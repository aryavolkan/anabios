//! Domesticable animals (E13): Husbandry holders tame wild juvenile
//! herbivores, pen them (movement override), and draw passive milk yields;
//! penned stock breeds and newborns of tamed parents are born domesticated.
//!
//! Everything here is gated on `World::domestication_enabled`: with the flag
//! off the tick stage early-returns, the decide-stage pen override is
//! skipped, and zero RNG is drawn, so every pre-E13 scenario stays
//! byte-identical. Taming additionally requires the holder to actually hold
//! the Husbandry invention, so `inventions_enabled` is effectively required
//! for anything to happen.

use std::collections::BTreeMap;

use crate::agent::{AgentId, AGENT_NULL};
use crate::genome::GenomeSlot;
use crate::prelude::Vec2;
use crate::world::World;

/// Maximum tick age at which a wild animal can still be tamed (the juvenile
/// window — adults are set in their ways).
pub const TAME_MAX_AGE: u32 = 100;
/// Distance within which a Husbandry holder can reach a candidate.
pub const TAME_RANGE: f32 = 4.0;
/// Per-tick tame probability once a candidate is in reach.
pub const TAME_CHANCE: f32 = 0.05;
/// Only herbivores are domesticable (diet carnivory strictly below this).
pub const TAME_CARNIVORY_MAX: f32 = 0.5;
/// Herd cap per herder.
pub const MAX_LIVESTOCK_PER_OWNER: u32 = 8;
/// Pen radius: livestock farther than this from its owner is pulled back;
/// inside it stands and grazes.
pub const PEN_RADIUS: f32 = 6.0;
/// Livestock at/above this age yields milk.
pub const MILK_MIN_AGE: u32 = 100;
/// Per-tick milk energy per unit of body size transferred to the owner.
pub const MILK_RATE: f32 = 0.02;
/// What the animal pays per unit of milk (conversion loss).
pub const MILK_COST_MULT: f32 = 1.25;
/// The animal must hold this much energy to be milked (surplus only).
pub const MILK_MIN_ENERGY: f32 = 30.0;

/// Pen-pull direction for one livestock agent (pure parts for the rayon
/// decide closure): unit torus-aware delta toward the owner when beyond
/// `PEN_RADIUS`, else `ZERO` (stand and graze the pen cell).
pub fn pen_pull_parts(owner_pos: Vec2, pos: Vec2, ws: f32) -> Vec2 {
    let d = crate::settlement::anchor_delta(owner_pos, pos, ws);
    let len = d.length();
    if len > PEN_RADIUS {
        d / len
    } else {
        Vec2::ZERO
    }
}

/// True iff `candidate` is a valid taming target for `owner_species`: alive,
/// wild, juvenile, other species, herbivorous.
fn is_tame_candidate(
    agents: &crate::agent::AgentBuffers,
    candidate: usize,
    owner_species: u32,
) -> bool {
    agents.livestock_of[candidate] == AGENT_NULL
        && agents.age[candidate] < TAME_MAX_AGE
        && agents.species_id[candidate] != owner_species
        && crate::module::effective_diet_carnivory(&agents.modules[candidate]) < TAME_CARNIVORY_MAX
}

/// Tick stage 6e: orphan sweep + herd counts, milk yields, taming rolls.
/// Ascending-id iteration everywhere; RNG draws only on taming attempts with
/// a candidate in reach. No-op (zero draws, zero state) when the flag is off.
pub fn husbandry_step(world: &mut World) {
    if !world.domestication_enabled {
        return;
    }
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());

    // Pass 1: orphan sweep (dead owner → wild again) + per-owner herd counts.
    let mut herd_counts: BTreeMap<AgentId, u32> = BTreeMap::new();
    for &id in &alive_ids {
        let owner = world.agents.livestock_of[id as usize];
        if owner == AGENT_NULL {
            continue;
        }
        if !world.agents.is_alive(owner) {
            world.agents.livestock_of[id as usize] = AGENT_NULL;
        } else {
            *herd_counts.entry(owner).or_insert(0) += 1;
        }
    }

    // Pass 2: milk yields — adult livestock with surplus pays its owner.
    for &id in &alive_ids {
        let i = id as usize;
        let owner = world.agents.livestock_of[i];
        if owner == AGENT_NULL || world.agents.age[i] < MILK_MIN_AGE {
            continue;
        }
        if world.agents.energy[i] <= MILK_MIN_ENERGY {
            continue;
        }
        let size = world.agents.genome[i].get(GenomeSlot::Size).max(0.1);
        let amount = MILK_RATE * size;
        world.agents.energy[i] -= amount * MILK_COST_MULT;
        world.agents.energy[owner as usize] += amount;
    }

    // Pass 3: taming — each Husbandry holder under its herd cap reaches for
    // the nearest eligible juvenile herbivore and rolls.
    for &id in &alive_ids {
        let i = id as usize;
        if herd_counts.get(&id).copied().unwrap_or(0) >= MAX_LIVESTOCK_PER_OWNER {
            continue;
        }
        let held = crate::invention::held_mask(&world.agents.meme_vector[i]);
        if held & crate::invention::bit(crate::invention::HUSBANDRY) == 0 {
            continue;
        }
        let pos = world.agents.position[i];
        let species = world.agents.species_id[i];
        // Nearest eligible candidate: min distance, tie-break lowest id —
        // independent of spatial bucket traversal order, so deterministic.
        let mut best: Option<(u32, f32)> = None;
        world.spatial.query(pos, TAME_RANGE, |other_id| {
            let j = other_id as usize;
            if !world.agents.is_alive(other_id) || !is_tame_candidate(&world.agents, j, species) {
                return;
            }
            let d = crate::spatial::torus_distance(pos, world.agents.position[j], world.world_size);
            if d > TAME_RANGE {
                return;
            }
            match best {
                None => best = Some((other_id, d)),
                Some((cur, cd)) if d < cd || (d == cd && other_id < cur) => {
                    best = Some((other_id, d));
                }
                _ => {}
            }
        });
        let Some((candidate, _)) = best else { continue };
        if world.rng.f32_unit() >= TAME_CHANCE {
            continue;
        }
        world.agents.livestock_of[candidate as usize] = id;
        // Keep the invariant "a binding exists ⇒ the kill-time release is armed"
        // local to the write, so worlds built outside `instantiate`/load still
        // release orphaned herds. Only reachable with the feature on.
        world.agents.track_livestock = true;
        herd_counts.insert(id, herd_counts.get(&id).copied().unwrap_or(0) + 1);
        // First tame of this livestock species anywhere → codex event.
        let livestock_species = world.agents.species_id[candidate as usize];
        if world.codex.domesticated_species.insert(livestock_species) {
            let p = world.agents.position[candidate as usize];
            world.codex.push_event(crate::codex::CodexEvent {
                event_type: crate::codex::EventType::AnimalDomesticated,
                tick: world.tick,
                species_id: livestock_species,
                value: species as f32,
                loc_x: p.x,
                loc_y: p.y,
            });
        }
    }

    world.agents.scratch_ids = alive_ids;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;

    /// A Husbandry-holding herder + a wild juvenile herbivore of another
    /// species, spawned `dist` apart on a fresh world with the flag on.
    fn herder_and_calf(dist: f32) -> (World, AgentId, AgentId) {
        let mut w = World::new(5);
        w.inventions_enabled = true;
        w.domestication_enabled = true;
        let herder = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        // Species 1 (the calf's): grow the species tables like scenario
        // instantiation does for archetype species.
        w.push_species(Genome::neutral(), Some(0));
        let calf = w.spawn_seeded(
            Vec2::new(500.0 + dist, 500.0),
            Genome::neutral(),
            1,
            crate::module::starter_kit(),
            crate::program::starter_grazer(),
        );
        w.agents.meme_vector[herder as usize]
            [crate::invention::INVENTION_CHANNEL_BASE + crate::invention::HUSBANDRY] = 1.0;
        (w, herder, calf)
    }

    #[test]
    fn holder_tames_juvenile_herbivore_within_range() {
        // Force the roll: run enough ticks that TAME_CHANCE (0.05) hits.
        let (mut w, herder, calf) = herder_and_calf(2.0);
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        for _ in 0..200 {
            husbandry_step(&mut w);
            if w.agents.livestock_of[calf as usize] != AGENT_NULL {
                break;
            }
        }
        assert_eq!(w.agents.livestock_of[calf as usize], herder, "calf is tamed");
        assert!(
            w.codex
                .events
                .iter()
                .any(|e| e.event_type == crate::codex::EventType::AnimalDomesticated),
            "first tame fires AnimalDomesticated"
        );
    }

    #[test]
    fn out_of_range_calf_is_not_tamed() {
        let (mut w, _herder, calf) = herder_and_calf(10.0);
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        for _ in 0..50 {
            husbandry_step(&mut w);
        }
        assert_eq!(w.agents.livestock_of[calf as usize], AGENT_NULL);
    }

    #[test]
    fn adult_and_same_species_are_not_tamed() {
        let (mut w, herder, calf) = herder_and_calf(2.0);
        w.agents.age[calf as usize] = TAME_MAX_AGE; // adult
        let own = w.spawn_agent(Vec2::new(502.0, 500.0), Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        for _ in 0..200 {
            husbandry_step(&mut w);
        }
        assert_eq!(w.agents.livestock_of[calf as usize], AGENT_NULL, "adults are untameable");
        assert_eq!(w.agents.livestock_of[own as usize], AGENT_NULL, "own species is untameable");
        let _ = herder;
    }

    #[test]
    fn non_holder_tames_nothing() {
        let (mut w, herder, calf) = herder_and_calf(2.0);
        w.agents.meme_vector[herder as usize]
            [crate::invention::INVENTION_CHANNEL_BASE + crate::invention::HUSBANDRY] = 0.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        for _ in 0..200 {
            husbandry_step(&mut w);
        }
        assert_eq!(w.agents.livestock_of[calf as usize], AGENT_NULL);
    }

    #[test]
    fn milk_flows_from_surplus_adult_to_owner() {
        let (mut w, herder, calf) = herder_and_calf(2.0);
        w.agents.livestock_of[calf as usize] = herder;
        w.agents.age[calf as usize] = MILK_MIN_AGE;
        w.agents.energy[calf as usize] = 50.0;
        w.agents.energy[herder as usize] = 40.0;
        husbandry_step(&mut w);
        let size = 0.5f32; // neutral genome
        let amount = MILK_RATE * size;
        assert!(
            (w.agents.energy[calf as usize] - (50.0 - amount * MILK_COST_MULT)).abs() < 1e-5,
            "calf pays cost-mult of the yield"
        );
        assert!(
            (w.agents.energy[herder as usize] - (40.0 + amount)).abs() < 1e-5,
            "herder receives the yield"
        );
    }

    #[test]
    fn no_milk_from_juvenile_or_starving_stock() {
        let (mut w, herder, calf) = herder_and_calf(2.0);
        w.agents.livestock_of[calf as usize] = herder;
        // Juvenile: no yield.
        w.agents.age[calf as usize] = MILK_MIN_AGE - 1;
        w.agents.energy[calf as usize] = 50.0;
        w.agents.energy[herder as usize] = 40.0;
        husbandry_step(&mut w);
        assert_eq!(w.agents.energy[herder as usize], 40.0, "juveniles give no milk");
        // Adult but at/below the surplus floor: no yield.
        w.agents.age[calf as usize] = MILK_MIN_AGE;
        w.agents.energy[calf as usize] = MILK_MIN_ENERGY;
        husbandry_step(&mut w);
        assert_eq!(w.agents.energy[herder as usize], 40.0, "starving stock gives no milk");
    }

    #[test]
    fn orphan_goes_wild_when_owner_dies() {
        let (mut w, herder, calf) = herder_and_calf(2.0);
        w.agents.livestock_of[calf as usize] = herder;
        w.agents.kill(herder);
        husbandry_step(&mut w);
        assert_eq!(w.agents.livestock_of[calf as usize], AGENT_NULL, "orphaned calf is wild again");
    }

    #[test]
    fn herd_cap_blocks_further_taming() {
        let (mut w, herder, calf) = herder_and_calf(2.0);
        // Fill the herder's herd to the cap with phantom stock (spawned adults
        // of species 1 pre-marked as livestock).
        for k in 0..MAX_LIVESTOCK_PER_OWNER {
            let id = w.spawn_seeded(
                Vec2::new(520.0 + k as f32, 520.0),
                Genome::neutral(),
                1,
                crate::module::starter_kit(),
                crate::program::starter_grazer(),
            );
            w.agents.livestock_of[id as usize] = herder;
        }
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        for _ in 0..200 {
            husbandry_step(&mut w);
        }
        assert_eq!(w.agents.livestock_of[calf as usize], AGENT_NULL, "at cap: no new tames");
    }

    #[test]
    fn pen_pull_only_beyond_radius() {
        let owner = Vec2::new(500.0, 500.0);
        assert_eq!(
            pen_pull_parts(owner, Vec2::new(500.0 + PEN_RADIUS - 1.0, 500.0), 1024.0),
            Vec2::ZERO,
            "inside the pen: stand and graze"
        );
        let pull = pen_pull_parts(owner, Vec2::new(500.0 + PEN_RADIUS + 5.0, 500.0), 1024.0);
        assert!((pull - Vec2::new(-1.0, 0.0)).length() < 1e-6, "outside: pulled back toward owner");
    }

    #[test]
    fn flag_off_is_inert() {
        let (mut w, _herder, calf) = herder_and_calf(2.0);
        w.domestication_enabled = false;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let hash_before = crate::snapshot::state_hash(&w);
        husbandry_step(&mut w);
        assert_eq!(w.agents.livestock_of[calf as usize], AGENT_NULL);
        assert_eq!(
            crate::snapshot::state_hash(&w),
            hash_before,
            "flag off: zero state change, zero RNG draws"
        );
    }
}
