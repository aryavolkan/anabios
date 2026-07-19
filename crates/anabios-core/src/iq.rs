//! Cognitive layer: a per-agent **realized IQ** that develops from a heritable
//! gene modulated by the juvenile environment (nature + nurture), costs basal
//! metabolism, and (Phase 2+) will gate which cultural traits an agent can
//! acquire.
//!
//! Realized IQ is a *phenotype*, not a gene. It starts at 0 and, during a
//! juvenile window, is refined each tick toward a blend of the heritable
//! `CognitivePotential` baseline and two environmental signals — **nutrition**
//! (how well-fed the juvenile is) and **social enrichment** (how socially
//! embedded it is) — then freezes at maturity. So a genetically-bright agent
//! raised starving underperforms an average one raised rich.
//!
//! The whole layer is gated on `World::cognition_enabled`: with the flag off,
//! IQ stays `0.0` for every agent (metabolic multiplier is exact identity, no
//! gating) and `develop_all` is a strict no-op that consumes no RNG.

use crate::agent::SPAWN_ENERGY;
use crate::world::World;

/// Age (ticks) at which realized IQ crystallizes; development runs only while
/// `age < IQ_MATURATION_AGE`.
pub const IQ_MATURATION_AGE: u32 = 100;
/// Blend between nature (`CognitivePotential`) and nurture (juvenile
/// enrichment): `iq = lerp(gene, enrichment, IQ_PLASTICITY)`. `0.5` = half
/// heritable, half developmental.
pub const IQ_PLASTICITY: f32 = 0.5;
/// Basal-metabolism surcharge at `iq = 1` (brains are expensive). This cost is
/// what keeps IQ from freely maxing out — it makes cognition an evolvable
/// tradeoff rather than a free lunch.
pub const IQ_METABOLIC_COST: f32 = 0.25;
/// Energy level that saturates the nutrition signal (a juvenile at or above
/// spawn energy counts as fully nourished).
pub const IQ_NUTRITION_REF: f32 = SPAWN_ENERGY;
/// Neighbour count that saturates the social-enrichment signal.
pub const IQ_SOCIAL_REF: f32 = 8.0;

/// Basal-metabolism multiplier from realized IQ. Exact identity at `iq == 0`,
/// so a flag-off world (where IQ stays 0) pays no cost and stays byte-identical.
#[inline]
pub fn metabolism_multiplier(iq: f32) -> f32 {
    1.0 + IQ_METABOLIC_COST * iq
}

/// Per-tick cognitive development (tick stage, run after feeding/upkeep and
/// before reproduce so energy reflects this tick's foraging). For each juvenile
/// (`age < IQ_MATURATION_AGE`) fold this tick's nutrition + social enrichment
/// into a running average and re-derive realized IQ as a blend of the heritable
/// baseline and that average. Crystallized (skipped) at/after maturity.
/// Consumes no RNG. No-op when `cognition_enabled` is false.
pub fn develop_all(world: &mut World) {
    if !world.cognition_enabled {
        return;
    }
    let mut ids = std::mem::take(&mut world.agents.scratch_ids);
    ids.clear();
    ids.extend(world.agents.iter_alive());
    for &id in &ids {
        let i = id as usize;
        if world.agents.age[i] >= IQ_MATURATION_AGE {
            continue; // crystallized — IQ is fixed for life
        }
        // Nutrition: how well-fed this juvenile is (post-feeding energy).
        let nutrition = (world.agents.energy[i] / IQ_NUTRITION_REF).clamp(0.0, 1.0);
        // Social enrichment: local neighbour density from this tick's sense.
        // Per-agent bounds check — on a growth tick the sensors buffer can be
        // shorter than capacity (same discipline as invention crowding stress).
        let social = if i < world.sensors.len() {
            (world.sensors[i].crowding as f32 / IQ_SOCIAL_REF).clamp(0.0, 1.0)
        } else {
            0.0
        };
        world.agents.iq_enrich_acc[i] += 0.5 * nutrition + 0.5 * social;
        world.agents.iq_enrich_ticks[i] += 1;
        let enrich = world.agents.iq_enrich_acc[i] / world.agents.iq_enrich_ticks[i] as f32;
        let gene = world.agents.genome[i].cognitive_potential();
        world.agents.iq[i] = gene + (enrich - gene) * IQ_PLASTICITY;
    }
    world.agents.scratch_ids = ids;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::{Genome, GenomeSlot};
    use crate::prelude::Vec2;

    /// Develop one agent for a single tick with the given gene / energy /
    /// crowding and return its realized IQ.
    fn develop_once(gene: f32, energy: f32, crowding: u32, age: u32) -> f32 {
        let mut w = World::new(1);
        w.cognition_enabled = true;
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let i = id as usize;
        w.agents.genome[i].set(GenomeSlot::CognitivePotential, gene);
        w.agents.energy[i] = energy;
        w.agents.age[i] = age;
        w.sensors.resize(w.agents.capacity(), Default::default());
        w.sensors[i].crowding = crowding;
        develop_all(&mut w);
        w.agents.iq[i]
    }

    #[test]
    fn metabolism_multiplier_is_identity_at_zero() {
        assert_eq!(metabolism_multiplier(0.0), 1.0);
        assert_eq!(metabolism_multiplier(1.0), 1.0 + IQ_METABOLIC_COST);
        assert_eq!(metabolism_multiplier(0.5), 1.0 + 0.5 * IQ_METABOLIC_COST);
    }

    #[test]
    fn develop_is_noop_when_flag_off() {
        let mut w = World::new(2);
        // cognition_enabled defaults to false.
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.agents.energy[id as usize] = SPAWN_ENERGY;
        develop_all(&mut w);
        assert_eq!(w.agents.iq[id as usize], 0.0, "IQ untouched with the flag off");
        assert_eq!(w.agents.iq_enrich_ticks[id as usize], 0);
    }

    #[test]
    fn nurture_lifts_a_bright_gene_and_starvation_sinks_it() {
        let gene = 0.8;
        // Blend toward enrichment: full nutrition + social pulls above the gene,
        // starved + isolated drags below it.
        let rich = develop_once(gene, SPAWN_ENERGY, 8, 0);
        let poor = develop_once(gene, 0.0, 0, 0);
        assert!((rich - 0.9).abs() < 1e-6, "bright+rich = lerp(0.8,1.0,0.5): {rich}");
        assert!((poor - 0.4).abs() < 1e-6, "bright+poor = lerp(0.8,0.0,0.5): {poor}");
        assert!(rich > poor);
    }

    #[test]
    fn nurture_helps_an_average_gene_too() {
        let rich = develop_once(0.5, SPAWN_ENERGY, 8, 0);
        let poor = develop_once(0.5, 0.0, 0, 0);
        assert!((rich - 0.75).abs() < 1e-6, "avg+rich = lerp(0.5,1.0,0.5): {rich}");
        assert!((poor - 0.25).abs() < 1e-6, "avg+poor = lerp(0.5,0.0,0.5): {poor}");
    }

    #[test]
    fn nature_still_matters_in_the_same_environment() {
        // Identical rich environment: the brighter gene yields the higher IQ.
        let bright = develop_once(0.9, SPAWN_ENERGY, 8, 0);
        let dull = develop_once(0.1, SPAWN_ENERGY, 8, 0);
        assert!(bright > dull, "nature contributes: bright={bright} dull={dull}");
    }

    #[test]
    fn iq_is_frozen_after_maturation() {
        // An agent at/after the maturation age is skipped entirely.
        let mut w = World::new(3);
        w.cognition_enabled = true;
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let i = id as usize;
        w.agents.iq[i] = 0.42; // pretend it crystallized earlier
        w.agents.age[i] = IQ_MATURATION_AGE;
        w.agents.energy[i] = SPAWN_ENERGY;
        w.sensors.resize(w.agents.capacity(), Default::default());
        w.sensors[i].crowding = 8;
        develop_all(&mut w);
        assert_eq!(w.agents.iq[i], 0.42, "mature IQ must not change");
        assert_eq!(w.agents.iq_enrich_ticks[i], 0, "no juvenile sample recorded");
    }
}
