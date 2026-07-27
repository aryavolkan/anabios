//! Dependency-free, deterministic, torus-tileable gradient noise (Perlin
//! construction) with fBm and domain warping, for procedural world generation.
//! All randomness comes from `crate::rng::Rng`; RNG draw order is part of the
//! determinism contract.

use crate::rng::Rng;

#[inline]
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Perlin-style gradient noise on a `period x period` corner grid. Corners wrap
/// modulo `period`, so the field is seamless across the unit torus.
pub struct GradientNoise {
    period: usize,
    grad: Vec<(f32, f32)>,
}

impl GradientNoise {
    /// Draw one unit gradient per corner. Draw order (row-major) is part of the
    /// determinism contract.
    pub fn new(rng: &mut Rng, period: usize) -> Self {
        let mut grad = Vec::with_capacity(period * period);
        for _ in 0..period * period {
            let angle = rng.f32_range(0.0, std::f32::consts::TAU);
            grad.push((angle.cos(), angle.sin()));
        }
        Self { period, grad }
    }

    #[inline]
    fn grad_at(&self, cx: usize, cy: usize) -> (f32, f32) {
        self.grad[cy * self.period + cx]
    }

    /// Sample at `(u, v)`; inputs are wrapped into `[0,1)`, output remapped to
    /// `[0,1]`. Seamless: `sample(0,v) == sample(1,v)`.
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let p = self.period as f32;
        let x = u.rem_euclid(1.0) * p;
        let y = v.rem_euclid(1.0) * p;
        let x0 = (x.floor() as usize) % self.period;
        let y0 = (y.floor() as usize) % self.period;
        let x1 = (x0 + 1) % self.period;
        let y1 = (y0 + 1) % self.period;
        let fx = x - x.floor();
        let fy = y - y.floor();
        let dot = |cx: usize, cy: usize, dx: f32, dy: f32| {
            let (gx, gy) = self.grad_at(cx, cy);
            gx * dx + gy * dy
        };
        let n00 = dot(x0, y0, fx, fy);
        let n10 = dot(x1, y0, fx - 1.0, fy);
        let n01 = dot(x0, y1, fx, fy - 1.0);
        let n11 = dot(x1, y1, fx - 1.0, fy - 1.0);
        let sx = smoothstep(fx);
        let sy = smoothstep(fy);
        let nx0 = lerp(n00, n10, sx);
        let nx1 = lerp(n01, n11, sx);
        let n = lerp(nx0, nx1, sy); // ~[-0.7, 0.7]
        (n * 0.7 + 0.5).clamp(0.0, 1.0)
    }
}

/// Fractal Brownian motion: sum of gradient-noise octaves at increasing period
/// (frequency) and decreasing amplitude. Each octave is individually tileable,
/// so the sum is too.
pub struct Fbm {
    layers: Vec<(GradientNoise, f32)>, // (octave, amplitude)
    norm: f32,
}

impl Fbm {
    pub fn new(
        rng: &mut Rng,
        base_period: usize,
        octaves: usize,
        lacunarity: usize,
        persistence: f32,
    ) -> Self {
        let mut layers = Vec::with_capacity(octaves);
        let mut period = base_period;
        let mut amp = 1.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            layers.push((GradientNoise::new(rng, period), amp));
            norm += amp;
            period *= lacunarity;
            amp *= persistence;
        }
        Self { layers, norm }
    }

    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let mut acc = 0.0;
        for (noise, amp) in &self.layers {
            acc += noise.sample(u, v) * amp;
        }
        (acc / self.norm).clamp(0.0, 1.0)
    }
}

/// Inigo-Quilez domain warp: offset `(u,v)` by two fBm fields centered on 0.
pub fn warp(fx: &Fbm, fy: &Fbm, u: f32, v: f32, amp: f32) -> (f32, f32) {
    (u + amp * (fx.sample(u, v) - 0.5), v + amp * (fy.sample(u, v) - 0.5))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn gradient_noise_is_bounded_and_varies() {
        let mut rng = Rng::from_seed(1);
        let n = GradientNoise::new(&mut rng, 8);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..64 {
            for j in 0..64 {
                let s = n.sample(i as f32 / 64.0, j as f32 / 64.0);
                assert!((0.0..=1.0).contains(&s), "out of range: {s}");
                lo = lo.min(s);
                hi = hi.max(s);
            }
        }
        assert!(hi - lo > 0.3, "field too flat: {lo}..{hi}");
    }

    #[test]
    fn gradient_noise_is_seamless_on_torus() {
        let mut rng = Rng::from_seed(2);
        let n = GradientNoise::new(&mut rng, 8);
        for k in 0..16 {
            let t = k as f32 / 16.0;
            assert!((n.sample(0.0, t) - n.sample(1.0, t)).abs() < 1e-5, "u seam at {t}");
            assert!((n.sample(t, 0.0) - n.sample(t, 1.0)).abs() < 1e-5, "v seam at {t}");
        }
    }

    #[test]
    fn noise_is_deterministic() {
        let a = {
            let mut r = Rng::from_seed(7);
            GradientNoise::new(&mut r, 8).sample(0.3, 0.6)
        };
        let b = {
            let mut r = Rng::from_seed(7);
            GradientNoise::new(&mut r, 8).sample(0.3, 0.6)
        };
        assert_eq!(a, b);
    }

    #[test]
    fn fbm_is_bounded_and_varies() {
        let mut rng = Rng::from_seed(3);
        let f = Fbm::new(&mut rng, 4, 5, 2, 0.5);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..64 {
            let s = f.sample(i as f32 / 64.0, 0.5);
            assert!((0.0..=1.0).contains(&s));
            lo = lo.min(s);
            hi = hi.max(s);
        }
        assert!(hi - lo > 0.2, "fbm too flat: {lo}..{hi}");
    }

    #[test]
    fn warp_offsets_coordinates_deterministically() {
        let mut rng = Rng::from_seed(9);
        let fx = Fbm::new(&mut rng, 4, 3, 2, 0.5);
        let fy = Fbm::new(&mut rng, 4, 3, 2, 0.5);
        // Sample off the integer lattice: gradient noise is exactly 0 at
        // lattice points, so a warp there is a no-op by construction.
        let (u, v) = warp(&fx, &fy, 0.3, 0.7, 0.3);
        assert!((u - 0.3).abs() > 1e-6 || (v - 0.7).abs() > 1e-6, "warp did nothing");
    }
}
