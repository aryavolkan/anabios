//! Deterministic RNG wrapper.
//!
//! The simulation uses a single Xoshiro256++ stream owned by `World`. Every
//! stochastic operation at tick time pulls from this stream in a fixed
//! order. No code reads `rand::thread_rng()` or `std::time` for randomness.
//! (Scenario instantiation additionally draws personalities from a dedicated
//! substream seeded from the world seed — see `Scenario::instantiate` — so
//! it never perturbs the tick-time stream.)

use rand::distributions::Standard;
use rand::prelude::Distribution;
use rand::{Rng as _, RngCore as _, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};

/// Deterministic RNG used throughout the simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rng {
    inner: Xoshiro256PlusPlus,
}

impl Rng {
    /// Construct from a 64-bit seed. Same seed → bit-identical stream.
    #[inline]
    pub fn from_seed(seed: u64) -> Self {
        Self { inner: Xoshiro256PlusPlus::seed_from_u64(seed) }
    }

    /// Uniform `f32` in `[0, 1)`.
    #[inline]
    pub fn f32_unit(&mut self) -> f32 {
        Standard.sample(&mut self.inner)
    }

    /// Uniform `f32` in `[low, high)`.
    #[inline]
    pub fn f32_range(&mut self, low: f32, high: f32) -> f32 {
        debug_assert!(low < high, "f32_range: low must be < high");
        self.inner.gen_range(low..high)
    }

    /// Gaussian sample with given mean and standard deviation, generated via
    /// the Box–Muller transform so it stays deterministic across platforms
    /// (the standard library has no fixed-output normal distribution).
    pub fn gaussian(&mut self, mean: f32, std_dev: f32) -> f32 {
        // Two uniforms in (0, 1] for Box–Muller.
        let u1 = (1.0 - self.f32_unit()).max(f32::MIN_POSITIVE);
        let u2 = self.f32_unit();
        let mag = (-2.0_f32 * crate::mathf::lnf(u1)).sqrt();
        let z0 = mag * crate::mathf::cosf(std::f32::consts::TAU * u2);
        mean + std_dev * z0
    }

    /// Uniform `u32`.
    #[inline]
    pub fn u32(&mut self) -> u32 {
        self.inner.next_u32()
    }

    /// Uniform index `< n`.
    #[inline]
    pub fn index(&mut self, n: usize) -> usize {
        debug_assert!(n > 0, "index: n must be > 0");
        self.inner.gen_range(0..n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_yields_same_stream() {
        let mut a = Rng::from_seed(42);
        let mut b = Rng::from_seed(42);
        for _ in 0..1024 {
            assert_eq!(a.u32(), b.u32());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::from_seed(1);
        let mut b = Rng::from_seed(2);
        let first_a = a.u32();
        let first_b = b.u32();
        assert_ne!(first_a, first_b);
    }

    #[test]
    fn f32_unit_in_range() {
        let mut r = Rng::from_seed(7);
        for _ in 0..10_000 {
            let x = r.f32_unit();
            assert!((0.0..1.0).contains(&x));
        }
    }

    #[test]
    fn gaussian_has_reasonable_moments() {
        let mut r = Rng::from_seed(11);
        let n = 50_000;
        let samples: Vec<f32> = (0..n).map(|_| r.gaussian(0.0, 1.0)).collect();
        let mean = samples.iter().sum::<f32>() / n as f32;
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n as f32;
        assert!(mean.abs() < 0.05, "mean drifted: {mean}");
        assert!((var - 1.0).abs() < 0.05, "variance drifted: {var}");
    }

    #[test]
    fn snapshot_roundtrip_preserves_stream() {
        let mut a = Rng::from_seed(99);
        for _ in 0..17 {
            a.u32();
        }
        let bytes = bincode::serialize(&a).expect("serialize");
        let mut b: Rng = bincode::deserialize(&bytes).expect("deserialize");
        for _ in 0..1024 {
            assert_eq!(a.u32(), b.u32());
        }
    }
}
