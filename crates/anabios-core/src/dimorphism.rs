//! Sexual dimorphism (E12): sex-linked stat expression, male display quality,
//! and the female mate-choice rule.
//!
//! Everything here is read **only when `World::sexual_dimorphism_enabled`**;
//! with the flag off every factor is exact identity and the mating pass takes
//! the pre-E12 code path byte-for-byte.
//!
//! Sex encoding: `AgentBuffers::sex` bit `false` = female, `true` = male.

use crate::genome::{Genome, GenomeSlot};

/// `sex` bit value for males (`false` = female).
pub const MALE: bool = true;
/// `sex` bit value for females.
pub const FEMALE: bool = false;

/// Male basal-metabolism surcharge per unit of the dimorphism gene `d`
/// (bigger, better-armed males cost more to run).
pub const DIMORPHISM_MALE_UPKEEP: f32 = 0.30;
/// Female basal-metabolism discount per unit of `d` (the dimorphic species'
/// females run leaner).
pub const DIMORPHISM_FEMALE_EFFICIENCY: f32 = 0.20;
/// Male weapon-damage bonus per unit of `d`.
pub const DIMORPHISM_MALE_DAMAGE: f32 = 0.40;
/// Male display-quality bonus (on the Size gene) per unit of `d`, read by the
/// female mate-choice rule.
pub const DIMORPHISM_MALE_DISPLAY: f32 = 0.60;
/// Slope of the female acceptance bar: a female with choosiness `c` accepts a
/// male iff his display quality ≥ `CHOOSINESS_QUALITY_SCALE × c`. Neutral
/// calibration (size 0.5, d 0.5, c 0.5): quality 0.575 ≥ 0.4 — neutral
/// populations mate freely; `c = 1` demands quality ≥ 0.8 (top-display males).
pub const CHOOSINESS_QUALITY_SCALE: f32 = 0.8;

/// The heritable dimorphism magnitude `d ∈ [0,1]` carried by one agent.
#[inline]
pub fn dimorphism_gene(genome: &Genome) -> f32 {
    genome.get(GenomeSlot::SexualDimorphism)
}

/// Basal-metabolism multiplier for one agent's sex + dimorphism gene.
/// Flag-off callers pass `enabled = false` and get exact identity.
#[inline]
pub fn metabolism_factor(genome: &Genome, male: bool, enabled: bool) -> f32 {
    if !enabled {
        return 1.0;
    }
    let d = dimorphism_gene(genome);
    if male {
        1.0 + DIMORPHISM_MALE_UPKEEP * d
    } else {
        1.0 - DIMORPHISM_FEMALE_EFFICIENCY * d
    }
}

/// Weapon-damage multiplier for one attacker's sex + dimorphism gene
/// (males hit harder in dimorphic species; females unchanged).
#[inline]
pub fn damage_factor(genome: &Genome, male: bool, enabled: bool) -> f32 {
    if !enabled {
        return 1.0;
    }
    if male {
        1.0 + DIMORPHISM_MALE_DAMAGE * dimorphism_gene(genome)
    } else {
        1.0
    }
}

/// Male display quality for mate choice: body size amplified by the
/// dimorphism gene, so the gene widens the male display distribution it also
/// pays for in upkeep.
#[inline]
pub fn male_display_quality(genome: &Genome) -> f32 {
    genome.get(GenomeSlot::Size) * (1.0 + DIMORPHISM_MALE_DISPLAY * dimorphism_gene(genome))
}

/// The female mate-choice rule: the female partner accepts the male iff his
/// display quality clears her choosiness bar. Deterministic (no RNG) so the
/// mating pass keeps its pre-E12 draw discipline. `female`/`male` are the two
/// partners' genomes, caller-identified by sex bit.
#[inline]
pub fn female_accepts(female: &Genome, male: &Genome) -> bool {
    let bar = CHOOSINESS_QUALITY_SCALE * female.get(GenomeSlot::MateChoosiness);
    male_display_quality(male) >= bar
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genome_with(size: f32, d: f32, choosiness: f32) -> Genome {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Size, size);
        g.set(GenomeSlot::SexualDimorphism, d);
        g.set(GenomeSlot::MateChoosiness, choosiness);
        g
    }

    #[test]
    fn factors_are_identity_when_disabled() {
        let g = genome_with(0.9, 1.0, 1.0);
        assert_eq!(metabolism_factor(&g, MALE, false), 1.0);
        assert_eq!(metabolism_factor(&g, FEMALE, false), 1.0);
        assert_eq!(damage_factor(&g, MALE, false), 1.0);
    }

    #[test]
    fn male_upkeep_and_female_efficiency_scale_with_d() {
        let hi = genome_with(0.5, 1.0, 0.5);
        let lo = genome_with(0.5, 0.0, 0.5);
        assert!((metabolism_factor(&hi, MALE, true) - 1.30).abs() < 1e-6);
        assert!((metabolism_factor(&hi, FEMALE, true) - 0.80).abs() < 1e-6);
        assert_eq!(metabolism_factor(&lo, MALE, true), 1.0);
        assert_eq!(metabolism_factor(&lo, FEMALE, true), 1.0);
        assert!((damage_factor(&hi, MALE, true) - 1.40).abs() < 1e-6);
        assert_eq!(damage_factor(&hi, FEMALE, true), 1.0);
    }

    #[test]
    fn neutral_pair_mates_freely() {
        let f = genome_with(0.5, 0.5, 0.5);
        let m = genome_with(0.5, 0.5, 0.5);
        // quality = 0.5 × 1.3 = 0.65 ≥ 0.8 × 0.5 = 0.4.
        assert!(female_accepts(&f, &m));
    }

    #[test]
    fn choosy_female_rejects_small_male_but_accepts_display_male() {
        let f = genome_with(0.5, 0.5, 0.9);
        let small = genome_with(0.3, 0.2, 0.5);
        let display = genome_with(0.9, 0.9, 0.5);
        assert!(!female_accepts(&f, &small));
        assert!(female_accepts(&f, &display));
    }

    #[test]
    fn zero_choosiness_accepts_any_male() {
        let f = genome_with(0.5, 0.5, 0.0);
        let tiny = genome_with(0.05, 0.0, 0.5);
        assert!(female_accepts(&f, &tiny));
    }
}
