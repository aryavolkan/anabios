//! Reproduction stage.
//!
//! Two same-species agents in close proximity (≤ MATING_RANGE) with energy
//! above `reproduction_threshold * SPAWN_ENERGY * 1.5` may produce one
//! offspring per tick. Each parent pays `OFFSPRING_INVESTMENT * SPAWN_ENERGY
//! / 2` energy; the offspring is seeded with `SPAWN_ENERGY` from the world.
//! (Energy is approximately conserved within the family-pair exchange.)

use crate::agent::{AgentBuffers, SPAWN_ENERGY};
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;
use crate::spatial::{torus_distance, UniformSpatialHash};
use crate::world::World;

/// Maximum distance between two parents at the moment of mating, in world units.
pub const MATING_RANGE: f32 = 2.0;

/// Fraction of `SPAWN_ENERGY` that each parent pays to produce an offspring.
pub const PARENT_ENERGY_COST_FRAC: f32 = 0.25;

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
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();

    for &a_id in &alive_ids {
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
        let child_pos = midpoint_torus(a_pos, b_pos);

        let lineage = world.next_lineage();
        let child_id =
            world.agents.spawn(child_pos, child_genome, lineage, [a_lineage, b_lineage], a_species);

        // Ensure the bitvec covers the new slot, mark the child as
        // "reproduced this tick" so they cannot immediately mate again.
        if world.reproduced_this_tick.len() <= child_id as usize {
            world.reproduced_this_tick.resize(child_id as usize + 1, false);
        }
        world.reproduced_this_tick.set(child_id as usize, true);
    }
}

fn is_eligible(agents: &AgentBuffers, id: u32) -> bool {
    let i = id as usize;
    if !agents.is_alive(id) {
        return false;
    }
    let threshold = SPAWN_ENERGY * agents.genome[i].get(GenomeSlot::ReproductionThreshold) * 1.5;
    agents.energy[i] >= threshold
}

fn find_mate(
    spatial: &UniformSpatialHash,
    agents: &AgentBuffers,
    reproduced: &bitvec::vec::BitVec,
    a_id: u32,
    a_pos: Vec2,
    a_species: u32,
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
        let d = torus_distance(a_pos, agents.position[j]);
        if d > MATING_RANGE {
            return;
        }
        // First eligible mate wins; we iterate ids in deterministic order
        // because the spatial hash flattens cells in ascending bucket order.
        // To be safe, take the lowest id we've seen.
        match best {
            None => best = Some(other_id),
            Some(cur) if other_id < cur => best = Some(other_id),
            _ => {}
        }
    });
    best
}

fn midpoint_torus(a: Vec2, b: Vec2) -> Vec2 {
    use crate::biome::WORLD_SIZE;
    let mut dx = b.x - a.x;
    let mut dy = b.y - a.y;
    if dx > WORLD_SIZE * 0.5 {
        dx -= WORLD_SIZE;
    } else if dx < -WORLD_SIZE * 0.5 {
        dx += WORLD_SIZE;
    }
    if dy > WORLD_SIZE * 0.5 {
        dy -= WORLD_SIZE;
    } else if dy < -WORLD_SIZE * 0.5 {
        dy += WORLD_SIZE;
    }
    let mid_x = (a.x + dx * 0.5).rem_euclid(WORLD_SIZE);
    let mid_y = (a.y + dy * 0.5).rem_euclid(WORLD_SIZE);
    Vec2::new(mid_x, mid_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;
    use crate::genome::Genome;
    use crate::world::World;

    fn find_grass_cell_center(w: &World) -> Vec2 {
        use crate::biome::{BIOME_RES, CELL_SIZE};
        for row in 0..BIOME_RES {
            for col in 0..BIOME_RES {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    return Vec2::new(
                        (col as f32 + 0.5) * CELL_SIZE,
                        (row as f32 + 0.5) * CELL_SIZE,
                    );
                }
            }
        }
        panic!("no grass cell in biome");
    }

    fn fertile_genome() -> Genome {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.4);
        g.set(GenomeSlot::DietCarnivory, 0.0);
        g.set(GenomeSlot::SpeedMax, 0.4);
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
}
