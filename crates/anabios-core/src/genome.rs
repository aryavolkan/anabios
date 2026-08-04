//! 50-float genome with named trait slots.
//!
//! Every value is clamped to `[0, 1]`. Slot meanings are hardcoded; values
//! mutate. Roughly twenty slots drive live behavior (body, metabolism,
//! lifespan, mutation rate, the Big Five personality, Altruism,
//! PerceptionRadius, ReproductionThreshold, the DIT learning propensities,
//! EnvAffinity); the rest are documented as reserved — see the per-slot
//! notes on `GenomeSlot`.

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

/// Std-dev for the Gaussian initial distribution of the 5 OCEAN personality
/// slots (stored space `[0,1]`, centered on the neutral 0.5).
pub const INIT_SIGMA: f32 = 0.2;

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
    /// Declared; not yet read by behavior. Reserved: future disease-resistance modifier.
    ImmuneStrength = 7,
    _BodyReserved8 = 8,
    _BodyReserved9 = 9,

    // Drive levels (10..20). Slots 10-13 and 21 are the Big Five (OCEAN)
    // personality traits (signed `[-1,+1]` via `2*g - 1`); renamed in place
    // from the former inert drive slots — indices are unchanged.
    /// Agreeableness: +1 cooperative/peaceful, −1 antagonistic (was Aggression).
    Agreeableness = 10,
    /// Neuroticism: +1 anxious/reactive, −1 stable/bold (was Fearfulness).
    Neuroticism = 11,
    /// Openness: +1 novelty-seeking, −1 routine (was Curiosity).
    Openness = 12,
    /// Extraversion: +1 social/seeking, −1 solitary (was SocialAffinity).
    Extraversion = 13,
    /// Declared; not yet read by behavior. Reserved: future kin-biased cooperation drive.
    KinPreference = 14,
    /// Declared; not yet read by behavior. Reserved: future territory-defense drive.
    Territoriality = 15,
    /// Heritable cognitive potential in `[0,1]` — the *nature* baseline for an
    /// agent's realized IQ (`iq.rs`). Unlike the personality slots this counts
    /// toward speciation distance (it is adaptive). Read only when
    /// `World::cognition_enabled`; inert otherwise.
    CognitivePotential = 16,
    /// Boldness (temperament, signed via accessor): +1 bold (low FEAR setpoint /
    /// high freeze threshold), −1 timid. Read by the affect layer. Counts toward
    /// speciation distance. Inert when `World::affect_enabled` is false.
    Boldness = 17,
    /// Aggressiveness (temperament): +1 high RAGE gain, −1 placid.
    Aggressiveness = 18,
    /// Nurturance (temperament): +1 high CARE gain, −1 neglectful.
    Nurturance = 19,

    // Behavioral biases (20..30)
    /// Declared; not yet read by behavior. Reserved: future foraging explore-vs-exploit bias.
    ExploreVsExploit = 20,
    /// Conscientiousness: +1 prudent/careful, −1 impulsive (was RiskTolerance).
    Conscientiousness = 21,
    /// Declared; not yet read by behavior. Reserved: future ambush-vs-pursuit hunting bias.
    AmbushPreference = 22,
    /// Declared; not yet read by behavior. Reserved: future Communicator-module effectiveness gain.
    CommunicationStrength = 23,
    Altruism = 24,
    /// Declared; not yet read by behavior (speed is set by Locomotor modules).
    /// Kept for serde index stability; reserved: future genome-level speed cap.
    SpeedMax = 25,
    PerceptionRadius = 26,
    /// Declared; not yet read by behavior (diet is set by Mouth module params).
    /// Kept for serde index stability; reserved: future genome-level diet bias.
    DietCarnivory = 27,
    /// Propensity (`> 0.5`) to individually *learn* the foraging technique by
    /// doing (learning-by-doing toward the current environmental optimum).
    /// Only active in DIT env mode (`World.env_period > 0`).
    IndividualLearning = 28,
    /// Propensity (`> 0.5`) to *socially copy* the foraging technique from a
    /// well-matched neighbour. Only active in DIT env mode (`World.env_period > 0`).
    SocialLearning = 29,

    // Reproductive (30..40)
    ReproductionThreshold = 30,
    /// Declared; not yet read by behavior. Reserved: future per-offspring energy-investment knob.
    OffspringInvestment = 31,
    /// Female mate-choice bar (E12): the female partner accepts a male iff his
    /// display quality ≥ `dimorphism::CHOOSINESS_QUALITY_SCALE × choosiness`.
    /// Read only when `World::sexual_dimorphism_enabled`; inert otherwise.
    MateChoosiness = 32,
    /// Heritable dimorphism magnitude `d ∈ [0,1]` (E12): scales male upkeep/
    /// damage/display and female metabolic efficiency via `dimorphism.rs`.
    /// Counts toward speciation distance (non-personality slot). Read only
    /// when `World::sexual_dimorphism_enabled`; inert otherwise.
    SexualDimorphism = 33,
    /// Sociality (temperament): +1 strongly bonded (PANIC/PLAY/CARE bond weight),
    /// −1 solitary. Read by the affect layer; counts toward speciation distance.
    Sociality = 34,
    /// Reactivity (temperament): +1 high arousal gain / low hijack threshold /
    /// slow decay, −1 phlegmatic. Read by the affect layer.
    Reactivity = 35,
    _ReproReserved36 = 36,
    _ReproReserved37 = 37,
    _ReproReserved38 = 38,
    _ReproReserved39 = 39,

    // Sensory weighting (40..50)
    /// A genetic strategy's fixed innate foraging technique in `[0, 1]`.
    /// Used (for non-cultural agents) only in DIT env mode (`World.env_period > 0`).
    InnateTechnique = 40,
    /// Genetic affinity in `[0,1]` for the local biome climate (`BiomeCell.env`).
    /// Read by the biome-adaptation feeding bonus when `World.biome_adaptation`
    /// is on. Counts toward speciation distance (drives biome-driven divergence).
    EnvAffinity = 41,
    /// Genetic affinity for a terrain TYPE (maps to a preferred trade good's home terrain).
    /// Read only under the terrain_habitat flag.
    TerrainAffinity = 42,
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

/// Display names indexed by slot number (`GenomeSlot as usize`), for UI
/// catalogs (the Godot bridge's helix/inspector panels). Reserved slots are
/// `"reserved_<n>"`. Alignment with the enum is locked by a unit test.
pub const SLOT_NAMES: [&str; GENOME_LEN] = [
    "Size",
    "ColorHue",
    "ColorSat",
    "ColorVal",
    "LifespanBias",
    "BasalMetabolism",
    "MutationRate",
    "ImmuneStrength",
    "reserved_8",
    "reserved_9",
    "Agreeableness",
    "Neuroticism",
    "Openness",
    "Extraversion",
    "KinPreference",
    "Territoriality",
    "CognitivePotential",
    "Boldness",
    "Aggressiveness",
    "Nurturance",
    "ExploreVsExploit",
    "Conscientiousness",
    "AmbushPreference",
    "CommunicationStrength",
    "Altruism",
    "SpeedMax",
    "PerceptionRadius",
    "DietCarnivory",
    "IndividualLearning",
    "SocialLearning",
    "ReproductionThreshold",
    "OffspringInvestment",
    "MateChoosiness",
    "SexualDimorphism",
    "Sociality",
    "Reactivity",
    "reserved_36",
    "reserved_37",
    "reserved_38",
    "reserved_39",
    "InnateTechnique",
    "EnvAffinity",
    "TerrainAffinity",
    "reserved_43",
    "reserved_44",
    "reserved_45",
    "reserved_46",
    "reserved_47",
    "reserved_48",
    "reserved_49",
];

/// Fixed-size 50-float genome.
///
/// All values are kept in `[0, 1]`; constructors and mutation respect this.
///
/// The inner array is private so the `[0, 1]` invariant can only be broken
/// through the explicitly-named `set_raw`/`as_mut_slice` escape hatches (used
/// by the trait-moment statistics that reuse `Genome` as a per-slot container),
/// never by a stray `.0` write. Named-slot writes go through `set`, which clamps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Genome([f32; GENOME_LEN]);

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

    /// Build a `Genome` from a raw slot array **without clamping**. For the
    /// derived "genomes" that are really per-slot statistics (species centroids,
    /// trait means/variances), not sampled individuals; real genomes come from
    /// `neutral`/`random`/`crossover`, which respect the `[0, 1]` invariant.
    #[inline]
    pub fn from_raw(slots: [f32; GENOME_LEN]) -> Self {
        Genome(slots)
    }

    /// Read a raw slot by index — the dynamic-index escape hatch for callers
    /// that index by a computed slot (e.g. `SenseGenome(slot)`), rather than a
    /// named `GenomeSlot`.
    #[inline]
    pub fn raw(&self, i: usize) -> f32 {
        self.0[i]
    }

    /// Write a raw slot by index, **without clamping**. Only for the trait-moment
    /// statistics that reuse `Genome` as a per-slot `f32` container (means and
    /// variances legitimately fall outside `[0, 1]`); real genomes must use `set`.
    #[inline]
    pub fn set_raw(&mut self, i: usize, value: f32) {
        self.0[i] = value;
    }

    /// Borrow the raw slot array (read-only) — for whole-genome scans such as
    /// species-centroid accumulation and the codex genome aggregator.
    #[inline]
    pub fn as_slice(&self) -> &[f32; GENOME_LEN] {
        &self.0
    }

    /// Mutable borrow of the raw slot array, **bypassing the `[0, 1]` clamp**.
    /// Same intent as `set_raw` (trait-moment statistics); not for real genomes.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [f32; GENOME_LEN] {
        &mut self.0
    }

    /// Openness in `[-1,+1]` (`2·slot − 1`). +1 novelty-seeking, −1 routine.
    pub fn openness(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Openness) - 1.0
    }
    /// Conscientiousness in `[-1,+1]`. +1 prudent/careful, −1 impulsive.
    pub fn conscientiousness(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Conscientiousness) - 1.0
    }
    /// Extraversion in `[-1,+1]`. +1 social/seeking, −1 solitary.
    pub fn extraversion(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Extraversion) - 1.0
    }
    /// Agreeableness in `[-1,+1]`. +1 cooperative/peaceful, −1 antagonistic.
    pub fn agreeableness(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Agreeableness) - 1.0
    }
    /// Neuroticism in `[-1,+1]`. +1 anxious/reactive, −1 stable/bold.
    pub fn neuroticism(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Neuroticism) - 1.0
    }

    /// Heritable cognitive potential in `[0,1]` — the genetic (nature) baseline
    /// realized IQ develops from (`iq.rs`).
    pub fn cognitive_potential(&self) -> f32 {
        self.get(GenomeSlot::CognitivePotential)
    }

    /// Boldness in `[-1,+1]` (`2·slot − 1`). +1 bold, −1 timid. Neutral `0.5`→`0.0`.
    pub fn boldness(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Boldness) - 1.0
    }
    /// Aggressiveness in `[-1,+1]`. +1 aggressive (high RAGE gain), −1 placid.
    pub fn aggressiveness(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Aggressiveness) - 1.0
    }
    /// Nurturance in `[-1,+1]`. +1 nurturing (high CARE gain), −1 neglectful.
    pub fn nurturance(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Nurturance) - 1.0
    }
    /// Sociality in `[-1,+1]`. +1 bonded, −1 solitary.
    pub fn sociality(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Sociality) - 1.0
    }
    /// Reactivity in `[-1,+1]`. +1 reactive (high arousal gain), −1 phlegmatic.
    pub fn reactivity(&self) -> f32 {
        2.0 * self.get(GenomeSlot::Reactivity) - 1.0
    }

    /// Overwrite the 5 OCEAN slots with `N(0.5, INIT_SIGMA)` clamped to `[0,1]`,
    /// giving a normally-distributed personality. Other slots are untouched.
    pub fn sample_personality_in_place(&mut self, rng: &mut Rng) {
        for slot in [
            GenomeSlot::Openness,
            GenomeSlot::Conscientiousness,
            GenomeSlot::Extraversion,
            GenomeSlot::Agreeableness,
            GenomeSlot::Neuroticism,
        ] {
            let v = rng.gaussian(0.5, INIT_SIGMA).clamp(0.0, 1.0);
            self.set(slot, v);
        }
    }

    /// Genome slots excluded from the speciation distance metric. The 5 Big
    /// Five personality slots vary within a species (they are behavioral
    /// temperament, not species identity), so counting them would fragment a
    /// population into spurious species and collapse same-species mating.
    /// These slots were neutral (0.5) at spawn in the pre-personality era, so
    /// their contribution to speciation was incidental; excluding them keeps
    /// species clustering driven by the ecological/morphological genes only.
    /// (The byte-level determinism contract is pinned by the golden test, not
    /// by this exclusion.)
    const PERSONALITY_SLOTS: [usize; 5] = [
        GenomeSlot::Agreeableness as usize,
        GenomeSlot::Neuroticism as usize,
        GenomeSlot::Openness as usize,
        GenomeSlot::Extraversion as usize,
        GenomeSlot::Conscientiousness as usize,
    ];

    /// O(1) lookup form of `PERSONALITY_SLOTS` — the 5-element `contains`
    /// probe showed up in the hot `distance()` inner loop.
    const PERSONALITY_MASK: [bool; GENOME_LEN] = {
        let mut m = [false; GENOME_LEN];
        let mut k = 0;
        while k < Self::PERSONALITY_SLOTS.len() {
            m[Self::PERSONALITY_SLOTS[k]] = true;
            k += 1;
        }
        m
    };

    /// L2 distance between two genomes, EXCLUDING the personality slots. Used by
    /// speciation in M2; kept here because it is conceptually part of the
    /// genome's contract.
    pub fn distance(&self, other: &Genome) -> f32 {
        let mut acc = 0.0_f32;
        for i in 0..GENOME_LEN {
            if Self::PERSONALITY_MASK[i] {
                continue;
            }
            let d = self.0[i] - other.0[i];
            acc += d * d;
        }
        acc.sqrt()
    }

    /// Apply per-slot Gaussian mutation in place. Sigma scales with the
    /// genome's own `MutationRate` slot. Values are clamped back into
    /// `[0, 1]` after perturbation.
    pub fn mutate_in_place(&mut self, rng: &mut Rng) {
        self.mutate_in_place_scaled(rng, 1.0);
    }

    /// `mutate_in_place` with the sigma scaled by `sigma_mult` (Nuclear
    /// Power's radiation debuff scales child mutation). The RNG draw count
    /// is identical to `mutate_in_place` — only the magnitudes change.
    pub fn mutate_in_place_scaled(&mut self, rng: &mut Rng, sigma_mult: f32) {
        let sigma = MUTATION_SIGMA_MAX * self.get(GenomeSlot::MutationRate) * sigma_mult;
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
    fn slot_names_align_with_the_enum() {
        assert_eq!(SLOT_NAMES[GenomeSlot::Size.idx()], "Size");
        assert_eq!(SLOT_NAMES[GenomeSlot::MutationRate.idx()], "MutationRate");
        assert_eq!(SLOT_NAMES[GenomeSlot::Agreeableness.idx()], "Agreeableness");
        assert_eq!(SLOT_NAMES[GenomeSlot::Openness.idx()], "Openness");
        assert_eq!(SLOT_NAMES[GenomeSlot::Territoriality.idx()], "Territoriality");
        assert_eq!(SLOT_NAMES[GenomeSlot::CognitivePotential.idx()], "CognitivePotential");
        assert_eq!(SLOT_NAMES[GenomeSlot::ExploreVsExploit.idx()], "ExploreVsExploit");
        assert_eq!(SLOT_NAMES[GenomeSlot::Conscientiousness.idx()], "Conscientiousness");
        assert_eq!(SLOT_NAMES[GenomeSlot::CommunicationStrength.idx()], "CommunicationStrength");
        assert_eq!(SLOT_NAMES[GenomeSlot::IndividualLearning.idx()], "IndividualLearning");
        assert_eq!(SLOT_NAMES[GenomeSlot::SocialLearning.idx()], "SocialLearning");
        assert_eq!(SLOT_NAMES[GenomeSlot::InnateTechnique.idx()], "InnateTechnique");
        assert_eq!(SLOT_NAMES[GenomeSlot::EnvAffinity.idx()], "EnvAffinity");
        assert_eq!(SLOT_NAMES[GenomeSlot::TerrainAffinity.idx()], "TerrainAffinity");
        assert_eq!(SLOT_NAMES[GenomeSlot::SexualDimorphism.idx()], "SexualDimorphism");
        // Reserved slots stay visibly reserved.
        assert!(SLOT_NAMES[8].starts_with("reserved_"));
        assert!(SLOT_NAMES[49].starts_with("reserved_"));
    }

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
        g.set(GenomeSlot::Agreeableness, -1.0);
        g.set(GenomeSlot::Openness, 2.0);
        assert_eq!(g.get(GenomeSlot::Agreeableness), 0.0);
        assert_eq!(g.get(GenomeSlot::Openness), 1.0);
    }

    #[test]
    fn personality_accessors_are_signed_minus1_to_plus1() {
        let mut g = Genome::neutral(); // all 0.5
        assert!((g.openness() - 0.0).abs() < 1e-6);
        assert!((g.agreeableness() - 0.0).abs() < 1e-6);
        g.set(GenomeSlot::Openness, 1.0);
        g.set(GenomeSlot::Neuroticism, 0.0);
        assert!((g.openness() - 1.0).abs() < 1e-6);
        assert!((g.neuroticism() - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn sample_personality_is_centered_clamped_and_varied() {
        let mut rng = crate::rng::Rng::from_seed(42);
        let mut sum = 0.0f32;
        let mut min = 1.0f32;
        let mut max = 0.0f32;
        let n = 2000;
        for _ in 0..n {
            let mut g = Genome::neutral();
            g.sample_personality_in_place(&mut rng);
            let v = g.get(GenomeSlot::Openness);
            assert!((0.0..=1.0).contains(&v));
            sum += v;
            min = min.min(v);
            max = max.max(v);
        }
        let mean = sum / n as f32;
        assert!((mean - 0.5).abs() < 0.05, "mean {mean} not ~0.5");
        assert!(max - min > 0.3, "spread too small: {min}..{max}");
        // Non-personality slot is untouched by the sampler.
        let mut g = Genome::neutral();
        g.sample_personality_in_place(&mut rng);
        assert_eq!(g.get(GenomeSlot::Size), 0.5);
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
        b.set(GenomeSlot::Agreeableness, 1.0);
        let child = Genome::crossover(&a, &b, &mut rng);
        for v in child.0.iter() {
            assert!(*v >= 0.0 && *v <= 1.0);
        }
    }

    #[test]
    fn temperament_slot_names_align_with_the_enum() {
        assert_eq!(SLOT_NAMES[GenomeSlot::Boldness.idx()], "Boldness");
        assert_eq!(SLOT_NAMES[GenomeSlot::Aggressiveness.idx()], "Aggressiveness");
        assert_eq!(SLOT_NAMES[GenomeSlot::Nurturance.idx()], "Nurturance");
        assert_eq!(SLOT_NAMES[GenomeSlot::Sociality.idx()], "Sociality");
        assert_eq!(SLOT_NAMES[GenomeSlot::Reactivity.idx()], "Reactivity");
    }

    #[test]
    fn temperament_accessors_are_signed_minus1_to_plus1() {
        let mut g = Genome::neutral(); // all 0.5 → 0.0 signed
        assert!(g.boldness().abs() < 1e-6);
        assert!(g.reactivity().abs() < 1e-6);
        g.set(GenomeSlot::Reactivity, 1.0);
        assert!((g.reactivity() - 1.0).abs() < 1e-6);
        g.set(GenomeSlot::Aggressiveness, 0.0);
        assert!((g.aggressiveness() + 1.0).abs() < 1e-6);
    }

    #[test]
    fn temperament_counts_toward_speciation_distance() {
        // Recorded M-A decision: temperament is adaptive → it counts toward
        // distance (unlike the OCEAN personality slots, which are masked out).
        let a = Genome::neutral();
        let mut b = Genome::neutral();
        b.set(GenomeSlot::Boldness, 1.0);
        assert!(a.distance(&b) > 0.0, "temperament (Boldness) must count toward distance");
        // A pure personality change still contributes nothing.
        let mut c = Genome::neutral();
        c.set(GenomeSlot::Openness, 1.0);
        assert_eq!(a.distance(&c), 0.0, "personality (Openness) stays excluded from distance");
    }
}
