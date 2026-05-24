//! 50-float genome with named trait slots.
//!
//! Every value is clamped to `[0, 1]`. Slot meanings are hardcoded; values
//! mutate. Only a handful of slots drive behavior in M1 (see `behavior.rs`);
//! the rest are present and inert, awaiting later milestones.

use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeTuple;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::rng::Rng;

/// Number of trait slots in the genome.
pub const GENOME_LEN: usize = 50;

/// Per-trait Gaussian mutation sigma when `mutation_rate` is at maximum.
///
/// Effective sigma per mutation = `MUTATION_SIGMA_MAX * genome[mutation_rate]`.
pub const MUTATION_SIGMA_MAX: f32 = 0.08;

/// Named slot indices into the 50-float genome.
///
/// Slot meanings are stable. New slots are appended; existing indices never
/// shift (so saved genomes stay readable across versions).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenomeSlot {
    // Body modifiers (0..10)
    Size = 0,
    ColorHue = 1,
    ColorSat = 2,
    ColorVal = 3,
    LifespanBias = 4,
    BasalMetabolism = 5,
    MutationRate = 6,
    ImmuneStrength = 7,
    _BodyReserved8 = 8,
    _BodyReserved9 = 9,

    // Drive levels (10..20)
    Aggression = 10,
    Fearfulness = 11,
    Curiosity = 12,
    SocialAffinity = 13,
    KinPreference = 14,
    Territoriality = 15,
    _DriveReserved16 = 16,
    _DriveReserved17 = 17,
    _DriveReserved18 = 18,
    _DriveReserved19 = 19,

    // Behavioral biases (20..30)
    ExploreVsExploit = 20,
    RiskTolerance = 21,
    AmbushPreference = 22,
    CommunicationStrength = 23,
    Altruism = 24,
    SpeedMax = 25,
    PerceptionRadius = 26,
    DietCarnivory = 27,
    _BehaviorReserved28 = 28,
    _BehaviorReserved29 = 29,

    // Reproductive (30..40)
    ReproductionThreshold = 30,
    OffspringInvestment = 31,
    MateChoosiness = 32,
    SexualDimorphism = 33,
    _ReproReserved34 = 34,
    _ReproReserved35 = 35,
    _ReproReserved36 = 36,
    _ReproReserved37 = 37,
    _ReproReserved38 = 38,
    _ReproReserved39 = 39,

    // Sensory weighting (40..50)
    _SensoryReserved40 = 40,
    _SensoryReserved41 = 41,
    _SensoryReserved42 = 42,
    _SensoryReserved43 = 43,
    _SensoryReserved44 = 44,
    _SensoryReserved45 = 45,
    _SensoryReserved46 = 46,
    _SensoryReserved47 = 47,
    _SensoryReserved48 = 48,
    _SensoryReserved49 = 49,
}

impl GenomeSlot {
    #[inline]
    pub const fn idx(self) -> usize {
        self as usize
    }
}

/// Fixed-size 50-float genome.
///
/// All values are kept in `[0, 1]`; constructors and mutation respect this.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Genome(pub [f32; GENOME_LEN]);

// Manual Serde impls: serde 1.x only derives Serialize/Deserialize for arrays
// of length <= 32, so we hand-roll a tuple-shaped impl over GENOME_LEN floats.
impl Serialize for Genome {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tup = serializer.serialize_tuple(GENOME_LEN)?;
        for v in self.0.iter() {
            tup.serialize_element(v)?;
        }
        tup.end()
    }
}

impl<'de> Deserialize<'de> for Genome {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct GenomeVisitor;
        impl<'de> Visitor<'de> for GenomeVisitor {
            type Value = Genome;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a tuple of {GENOME_LEN} f32 values")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Genome, A::Error> {
                let mut out = [0.0_f32; GENOME_LEN];
                for (i, slot) in out.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                Ok(Genome(out))
            }
        }
        deserializer.deserialize_tuple(GENOME_LEN, GenomeVisitor)
    }
}

impl Genome {
    /// Construct a genome filled with `0.5` (a neutral baseline used by
    /// scenario seed templates).
    #[inline]
    pub fn neutral() -> Self {
        Self([0.5; GENOME_LEN])
    }

    /// Construct a uniformly random genome.
    pub fn random(rng: &mut Rng) -> Self {
        let mut g = [0.0_f32; GENOME_LEN];
        for slot in g.iter_mut() {
            *slot = rng.f32_unit();
        }
        Self(g)
    }

    /// Read a slot by name.
    #[inline]
    pub fn get(&self, slot: GenomeSlot) -> f32 {
        self.0[slot.idx()]
    }

    /// Write a slot by name. The value is clamped into `[0, 1]`.
    #[inline]
    pub fn set(&mut self, slot: GenomeSlot, value: f32) {
        self.0[slot.idx()] = value.clamp(0.0, 1.0);
    }

    /// L2 distance between two genomes. Used by speciation in M2; kept here
    /// because it is conceptually part of the genome's contract.
    pub fn distance(&self, other: &Genome) -> f32 {
        let mut acc = 0.0_f32;
        for i in 0..GENOME_LEN {
            let d = self.0[i] - other.0[i];
            acc += d * d;
        }
        acc.sqrt()
    }

    /// Apply per-slot Gaussian mutation in place. Sigma scales with the
    /// genome's own `MutationRate` slot. Values are clamped back into
    /// `[0, 1]` after perturbation.
    pub fn mutate_in_place(&mut self, rng: &mut Rng) {
        let sigma = MUTATION_SIGMA_MAX * self.get(GenomeSlot::MutationRate);
        if sigma <= 0.0 {
            return;
        }
        for i in 0..GENOME_LEN {
            let delta = rng.gaussian(0.0, sigma);
            self.0[i] = (self.0[i] + delta).clamp(0.0, 1.0);
        }
    }

    /// Uniform crossover: each slot is independently inherited from one of
    /// the two parents with equal probability. The RNG is consumed in slot
    /// order so the output is deterministic given the seed.
    pub fn crossover(a: &Genome, b: &Genome, rng: &mut Rng) -> Genome {
        let mut out = [0.0_f32; GENOME_LEN];
        for (i, slot) in out.iter_mut().enumerate() {
            // Bit-packed source select: one RNG draw, 32 binary decisions
            // per draw. Cheaper than calling f32_unit 50 times.
            // Simplified for clarity: just use f32_unit each slot.
            let from_a = rng.f32_unit() < 0.5;
            *slot = if from_a { a.0[i] } else { b.0[i] };
        }
        Genome(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_genome_is_all_half() {
        let g = Genome::neutral();
        for v in g.0.iter() {
            assert_eq!(*v, 0.5);
        }
    }

    #[test]
    fn random_genome_is_in_unit_range() {
        let mut rng = Rng::from_seed(1);
        let g = Genome::random(&mut rng);
        for v in g.0.iter() {
            assert!(*v >= 0.0 && *v < 1.0);
        }
    }

    #[test]
    fn random_genome_is_deterministic() {
        let mut a = Rng::from_seed(123);
        let mut b = Rng::from_seed(123);
        let ga = Genome::random(&mut a);
        let gb = Genome::random(&mut b);
        assert_eq!(ga, gb);
    }

    #[test]
    fn get_and_set_use_named_slots() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 0.9);
        g.set(GenomeSlot::PerceptionRadius, 0.3);
        assert!((g.get(GenomeSlot::SpeedMax) - 0.9).abs() < 1e-6);
        assert!((g.get(GenomeSlot::PerceptionRadius) - 0.3).abs() < 1e-6);
        assert_eq!(g.get(GenomeSlot::Size), 0.5);
    }

    #[test]
    fn set_clamps_out_of_range_values() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Aggression, -1.0);
        g.set(GenomeSlot::Curiosity, 2.0);
        assert_eq!(g.get(GenomeSlot::Aggression), 0.0);
        assert_eq!(g.get(GenomeSlot::Curiosity), 1.0);
    }

    #[test]
    fn distance_is_zero_for_identical_genomes() {
        let g = Genome::neutral();
        assert_eq!(g.distance(&g), 0.0);
    }

    #[test]
    fn distance_is_symmetric() {
        let mut a = Genome::neutral();
        let mut b = Genome::neutral();
        a.set(GenomeSlot::SpeedMax, 0.9);
        b.set(GenomeSlot::SpeedMax, 0.1);
        assert!((a.distance(&b) - b.distance(&a)).abs() < 1e-6);
    }

    #[test]
    fn mutate_keeps_values_in_range_and_respects_zero_rate() {
        let mut rng = Rng::from_seed(7);
        let mut g = Genome::neutral();
        g.set(GenomeSlot::MutationRate, 0.0);
        let before = g.0;
        g.mutate_in_place(&mut rng);
        assert_eq!(before, g.0, "mutation with rate 0 must be a no-op");

        g.set(GenomeSlot::MutationRate, 1.0);
        for _ in 0..1000 {
            g.mutate_in_place(&mut rng);
            for v in g.0.iter() {
                assert!(*v >= 0.0 && *v <= 1.0);
            }
        }
    }

    #[test]
    fn crossover_with_identical_parents_yields_same_genome() {
        let mut rng = Rng::from_seed(1);
        let g = Genome::neutral();
        let child = Genome::crossover(&g, &g, &mut rng);
        assert_eq!(child, g);
    }

    #[test]
    fn crossover_yields_per_slot_values_from_one_parent() {
        let mut rng = Rng::from_seed(7);
        let mut a = Genome::neutral();
        let mut b = Genome::neutral();
        for i in 0..GENOME_LEN {
            a.0[i] = 0.1;
            b.0[i] = 0.9;
        }
        let child = Genome::crossover(&a, &b, &mut rng);
        for i in 0..GENOME_LEN {
            let v = child.0[i];
            assert!(v == 0.1 || v == 0.9, "slot {i} was {v}");
        }
    }

    #[test]
    fn crossover_is_deterministic() {
        let a = Genome::neutral();
        let mut b = Genome::neutral();
        b.set(GenomeSlot::SpeedMax, 0.9);

        let mut rng1 = Rng::from_seed(42);
        let mut rng2 = Rng::from_seed(42);
        let c1 = Genome::crossover(&a, &b, &mut rng1);
        let c2 = Genome::crossover(&a, &b, &mut rng2);
        assert_eq!(c1, c2);
    }

    #[test]
    fn crossover_output_stays_in_unit_range() {
        let mut rng = Rng::from_seed(99);
        let mut a = Genome::neutral();
        let mut b = Genome::neutral();
        a.set(GenomeSlot::MutationRate, 1.0);
        b.set(GenomeSlot::Aggression, 1.0);
        let child = Genome::crossover(&a, &b, &mut rng);
        for v in child.0.iter() {
            assert!(*v >= 0.0 && *v <= 1.0);
        }
    }
}
