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
/// Neighbour count that saturates the social-enrichment signal.
pub const IQ_SOCIAL_REF: f32 = 8.0;

/// Basal-metabolism multiplier from realized IQ. Exact identity at `iq == 0`,
/// so a flag-off world (where IQ stays 0) pays no cost and stays byte-identical.
#[inline]
pub fn metabolism_multiplier(iq: f32) -> f32 {
    1.0 + IQ_METABOLIC_COST * iq
}

/// Per-tick cognitive development (tick stage). For each juvenile
/// (`age < IQ_MATURATION_AGE`) fold this tick's nutrition (local biome food) +
/// social enrichment (sensed crowding) into a running average and re-derive
/// realized IQ as a blend of the heritable baseline and that average.
/// Crystallized (skipped) at/after maturity. Consumes no RNG. No-op when
/// `cognition_enabled` is false.
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
        // Nutrition: the food richness of the cell this juvenile is growing up
        // in — its `plant_biomass` as a fraction of the local carrying capacity.
        // (The agent's own energy is a poor proxy: the spawn-energy buffer keeps
        // it saturated through the juvenile window regardless of feeding, so it
        // can't tell a starved upbringing from a fed one. Local food can — a
        // juvenile raised on barren or grazed-out ground develops a lower IQ.)
        let (col, row) = world.biome.cell_coords(world.agents.position[i]);
        let cell = world.biome.at(col, row);
        let cap = cell.terrain.carrying_capacity();
        let nutrition = if cap > 0.0 { (cell.plant_biomass / cap).clamp(0.0, 1.0) } else { 0.0 };
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
    use crate::biome::{TerrainType, BIOME_RES, CELL_SIZE};
    use crate::genome::{Genome, GenomeSlot};
    use crate::prelude::Vec2;

    /// A grass cell (positive carrying capacity) to stand a juvenile on so its
    /// local food level is controllable.
    fn grass_cell(w: &World) -> (usize, usize) {
        for row in 0..BIOME_RES {
            for col in 0..BIOME_RES {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    return (col, row);
                }
            }
        }
        (0, 0)
    }

    /// Develop one agent for a single tick with the given gene, local food
    /// fraction (`0..1` of the cell's carrying capacity), crowding, and age;
    /// return its realized IQ.
    fn develop_once(gene: f32, food: f32, crowding: u32, age: u32) -> f32 {
        let mut w = World::new(1);
        w.cognition_enabled = true;
        // Stand the juvenile on a grass cell and set that cell's food level.
        let (col, row) = grass_cell(&w);
        let cap = w.biome.at(col, row).terrain.carrying_capacity();
        let idx = w.biome.cell_index(col, row);
        w.biome.cells[idx].plant_biomass = food * cap;
        let spot = Vec2::new((col as f32 + 0.5) * CELL_SIZE, (row as f32 + 0.5) * CELL_SIZE);
        let id = w.spawn_agent(spot, Genome::neutral());
        let i = id as usize;
        w.agents.genome[i].set(GenomeSlot::CognitivePotential, gene);
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
        develop_all(&mut w);
        assert_eq!(w.agents.iq[id as usize], 0.0, "IQ untouched with the flag off");
        assert_eq!(w.agents.iq_enrich_ticks[id as usize], 0);
    }

    #[test]
    fn nurture_lifts_a_bright_gene_and_starvation_sinks_it() {
        let gene = 0.8;
        // Blend toward enrichment: full local food + social pulls above the gene,
        // barren + isolated drags below it.
        let rich = develop_once(gene, 1.0, 8, 0);
        let poor = develop_once(gene, 0.0, 0, 0);
        assert!((rich - 0.9).abs() < 1e-6, "bright+rich = lerp(0.8,1.0,0.5): {rich}");
        assert!((poor - 0.4).abs() < 1e-6, "bright+poor = lerp(0.8,0.0,0.5): {poor}");
        assert!(rich > poor);
    }

    #[test]
    fn nurture_helps_an_average_gene_too() {
        let rich = develop_once(0.5, 1.0, 8, 0);
        let poor = develop_once(0.5, 0.0, 0, 0);
        assert!((rich - 0.75).abs() < 1e-6, "avg+rich = lerp(0.5,1.0,0.5): {rich}");
        assert!((poor - 0.25).abs() < 1e-6, "avg+poor = lerp(0.5,0.0,0.5): {poor}");
    }

    #[test]
    fn nature_still_matters_in_the_same_environment() {
        // Identical rich environment: the brighter gene yields the higher IQ.
        let bright = develop_once(0.9, 1.0, 8, 0);
        let dull = develop_once(0.1, 1.0, 8, 0);
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
        w.sensors.resize(w.agents.capacity(), Default::default());
        w.sensors[i].crowding = 8;
        develop_all(&mut w);
        assert_eq!(w.agents.iq[i], 0.42, "mature IQ must not change");
        assert_eq!(w.agents.iq_enrich_ticks[i], 0, "no juvenile sample recorded");
    }
}
