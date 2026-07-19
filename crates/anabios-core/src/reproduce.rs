//! Reproduction stage.
//!
//! Two same-species agents in close proximity (≤ `MATING_RANGE`) with energy
//! above `reproduction_threshold * SPAWN_ENERGY * 1.5` may produce one
//! offspring per tick. Each parent pays `PARENT_ENERGY_COST_FRAC *
//! SPAWN_ENERGY` energy; the offspring is seeded with `SPAWN_ENERGY` energy.
//! The fraction is tuned to be **energy-conserving** within the family-pair
//! exchange (parents collectively pay exactly the offspring's spawn energy).
//!
//! Reproduction is hard-capped at `World::max_population` (default
//! `MAX_POPULATION` = 10_000, scenario-overridable) to prevent runaway
//! growth in over-fertile scenarios; this is a coarse backstop, not a
//! carrying-capacity model.

use crate::agent::{AgentBuffers, SPAWN_ENERGY};
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;
use crate::spatial::{torus_distance, UniformSpatialHash};
use crate::world::World;

/// Maximum distance between two parents at the moment of mating, in world units.
pub const MATING_RANGE: f32 = 2.0;

/// Fraction of `SPAWN_ENERGY` each parent pays to produce an offspring.
/// 0.5 means parents collectively pay `SPAWN_ENERGY` total (energy-conserving).
pub const PARENT_ENERGY_COST_FRAC: f32 = 0.5;

/// Default hard upper bound on alive agents. Reproduction skips at/above the
/// cap. The live value is `World::max_population` (per-world overridable);
/// this constant is the design's 10k-agent budget (design §8; the
/// `tick_bench` 10k case seeds founders directly to exercise that scale).
pub const MAX_POPULATION: u32 = 10_000;

/// Run the reproduce stage. Each alive agent at most mates once per tick.
/// Order: ascending agent id. Each agent A checks its same-cell neighbours
/// in ascending id order and mates with the first eligible B such that
/// `B.id > A.id`; this avoids double-counting and keeps the algorithm
/// deterministic.
pub fn reproduce_all(world: &mut World) {
    // Pull scratch buffer length up to current capacity.
    if world.reproduced_this_tick.len() < world.agents.capacity() {
        world.reproduced_this_tick.resize(world.agents.capacity(), false);
    }
    world.reproduced_this_tick.fill(false);

    // Snapshot the alive ids to a local vec; reproduction mutates the
    // alive set via spawn() and we don't want to iterate over newborns
    // this tick.
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());

    for &a_id in &alive_ids {
        if world.agents.live_count() >= world.max_population {
            // Backstop: stop producing offspring above the cap. Iteration
            // order is deterministic (ascending id), so the cutoff is too.
            break;
        }
        let i = a_id as usize;
        if world.reproduced_this_tick[i] {
            continue;
        }
        if !is_eligible(&world.agents, a_id) {
            continue;
        }

        let a_pos = world.agents.position[i];
        let a_species = world.agents.species_id[i];
        let a_genome = world.agents.genome[i];
        let a_lineage = world.agents.lineage_id[i];

        // Find an eligible mate with a strictly higher id.
        let mate = find_mate(
            &world.spatial,
            &world.agents,
            &world.reproduced_this_tick,
            a_id,
            a_pos,
            a_species,
            world.world_size,
        );
        let Some(b_id) = mate else { continue };

        let j = b_id as usize;
        let b_pos = world.agents.position[j];
        let b_genome = world.agents.genome[j];
        let b_lineage = world.agents.lineage_id[j];

        // Pay energy from both parents.
        let cost = SPAWN_ENERGY * PARENT_ENERGY_COST_FRAC;
        world.agents.energy[i] -= cost;
        world.agents.energy[j] -= cost;

        // Build child genome: crossover + mutate.
        let mut child_genome = Genome::crossover(&a_genome, &b_genome, &mut world.rng);
        child_genome.mutate_in_place(&mut world.rng);

        // Mark both parents as reproduced this tick before spawning so the
        // newborn's slot (which gets a fresh bitvec bit) isn't accidentally
        // touched.
        world.reproduced_this_tick.set(i, true);
        world.reproduced_this_tick.set(j, true);

        // Spawn at midpoint of parents on the torus (account for wrap).
        let child_pos = midpoint_torus(a_pos, b_pos, world.world_size);

        let a_modules = world.agents.modules[i].clone();
        let b_modules = world.agents.modules[j].clone();
        let child_modules =
            crate::module::crossover_and_mutate(&a_modules, &b_modules, &mut world.rng);

        let a_program = world.agents.program[i].clone();
        let b_program = world.agents.program[j].clone();
        let child_program =
            crate::program::crossover_and_mutate(&a_program, &b_program, &mut world.rng);

        let lineage = world.next_lineage();
        let child_id = world.agents.spawn(
            child_pos,
            child_genome,
            lineage,
            [a_lineage, b_lineage],
            a_species,
            child_modules,
            child_program,
        );
        world.add_to_species(a_species);

        // Ensure the bitvec covers the new slot, mark the child as
        // "reproduced this tick" so they cannot immediately mate again.
        if world.reproduced_this_tick.len() <= child_id as usize {
            world.reproduced_this_tick.resize(child_id as usize + 1, false);
        }
        world.reproduced_this_tick.set(child_id as usize, true);

        // Meme inheritance: child = parent average + jitter, ONLY if the child
        // has a Communicator module. This gates RNG draws so that non-communicator
        // lineages (e.g. minimal.toml) draw zero meme RNG, keeping the golden
        // hash stream unchanged.
        if crate::module::has(
            &world.agents.modules[child_id as usize],
            crate::module::ModuleType::Communicator,
        ) {
            let a_meme = world.agents.meme_vector[i];
            let b_meme = world.agents.meme_vector[j];
            world.agents.meme_vector[child_id as usize] =
                crate::culture::inherit_meme(&a_meme, &b_meme, &mut world.rng);
        }
    }
    world.agents.scratch_ids = alive_ids;
}

fn is_eligible(agents: &AgentBuffers, id: u32) -> bool {
    let i = id as usize;
    if !agents.is_alive(id) {
        return false;
    }
    // Action gating: must have Reproductive module to mate.
    if !crate::module::has(&agents.modules[i], crate::module::ModuleType::Reproductive) {
        return false;
    }
    // Conscientiousness raises the effective breeding threshold.
    let threshold = SPAWN_ENERGY
        * agents.genome[i].get(GenomeSlot::ReproductionThreshold)
        * 1.5
        * crate::personality::personality_reproduction_factor(&agents.genome[i]);
    agents.energy[i] >= threshold
}

fn find_mate(
    spatial: &UniformSpatialHash,
    agents: &AgentBuffers,
    reproduced: &bitvec::vec::BitVec,
    a_id: u32,
    a_pos: Vec2,
    a_species: u32,
    world_size: f32,
) -> Option<u32> {
    let mut best: Option<u32> = None;
    spatial.query(a_pos, MATING_RANGE, |other_id| {
        if other_id <= a_id {
            return;
        }
        let j = other_id as usize;
        if reproduced[j] {
            return;
        }
        if !is_eligible(agents, other_id) {
            return;
        }
        if agents.species_id[j] != a_species {
            return;
        }
        let d = torus_distance(a_pos, agents.position[j], world_size);
        if d > MATING_RANGE {
            return;
        }
        // Take the lowest-id eligible mate. The spatial query already visits
        // cells in a fixed order and within each cell ids are scattered in
        // ascending-id order, so this is robust to any future change in
        // bucket traversal.
        match best {
            None => best = Some(other_id),
            Some(cur) if other_id < cur => best = Some(other_id),
            _ => {}
        }
    });
    best
}

fn midpoint_torus(a: Vec2, b: Vec2, world_size: f32) -> Vec2 {
    let mut dx = b.x - a.x;
    let mut dy = b.y - a.y;
    if dx > world_size * 0.5 {
        dx -= world_size;
    } else if dx < -world_size * 0.5 {
        dx += world_size;
    }
    if dy > world_size * 0.5 {
        dy -= world_size;
    } else if dy < -world_size * 0.5 {
        dy += world_size;
    }
    let mid_x = (a.x + dx * 0.5).rem_euclid(world_size);
    let mid_y = (a.y + dy * 0.5).rem_euclid(world_size);
    Vec2::new(mid_x, mid_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;
    use crate::genome::Genome;
    use crate::world::World;

    fn find_grass_cell_center(w: &World) -> Vec2 {
        let res = w.biome.res;
        let cell_size = w.biome.cell_size;
        for row in 0..res {
            for col in 0..res {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    return Vec2::new(
                        (col as f32 + 0.5) * cell_size,
                        (row as f32 + 0.5) * cell_size,
                    );
                }
            }
        }
        panic!("no grass cell in biome");
    }

    fn fertile_genome() -> Genome {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.4);
        g.set(GenomeSlot::Size, 0.4);
        g.set(GenomeSlot::BasalMetabolism, 0.4);
        g
    }

    #[test]
    fn two_adjacent_well_fed_agents_produce_offspring() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());

        // Give both ample energy.
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;

        // Build the spatial hash so find_mate can see them.
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();

        assert_eq!(after, before + 1, "expected exactly one offspring");
        // Each parent paid energy.
        assert!(w.agents.energy[id0 as usize] < SPAWN_ENERGY * 2.0);
        assert!(w.agents.energy[id1 as usize] < SPAWN_ENERGY * 2.0);
    }

    #[test]
    fn cross_species_pair_does_not_mate() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        // Force different species.
        w.agents.species_id[id1 as usize] = 1;
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;

        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();
        assert_eq!(after, before, "different species must not produce offspring");
    }

    #[test]
    fn population_cap_blocks_reproduction() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        // At the cap: no offspring.
        w.max_population = 2;
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), 2, "at cap: no offspring");

        // One slot free: exactly one offspring, then the cap bites again.
        w.max_population = 3;
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), 3, "one free slot: exactly one offspring");
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), 3, "cap holds on the next pass too");
    }

    #[test]
    fn low_energy_pair_does_not_mate() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        // Below threshold.
        w.agents.energy[id0 as usize] = 1.0;
        w.agents.energy[id1 as usize] = 1.0;

        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();
        assert_eq!(after, before, "low-energy agents must not mate");
    }

    #[test]
    fn offspring_inherits_parent_lineages() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        let lin0 = w.agents.lineage_id[id0 as usize];
        let lin1 = w.agents.lineage_id[id1 as usize];

        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        reproduce_all(&mut w);

        // The newborn is the only agent with non-zero parent ids.
        let mut found = false;
        for id in w.agents.iter_alive() {
            let p = w.agents.parent_ids[id as usize];
            if p != [crate::agent::LINEAGE_NONE; 2] {
                assert_eq!(
                    {
                        let mut s = p;
                        s.sort();
                        s
                    },
                    {
                        let mut s = [lin0, lin1];
                        s.sort();
                        s
                    }
                );
                found = true;
            }
        }
        assert!(found, "offspring with parent ids not found");
    }

    #[test]
    fn agent_without_reproductive_does_not_mate() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());

        // Strip Reproductive from id0 only.
        w.agents.modules[id0 as usize]
            .retain(|m| !matches!(m, crate::module::Module::Reproductive { .. }));

        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();
        assert_eq!(after, before, "missing Reproductive must block mating");
    }
}
