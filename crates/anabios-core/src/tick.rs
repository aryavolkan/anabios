//! Tick orchestration: the master `step()` function for M1.

use crate::age::age_and_starve;
use crate::behavior::decide;
use crate::integrate::integrate_all;
use crate::interact::interact_all;
use crate::sense::sense_all;
use crate::world::World;

/// How often (in ticks) the biome plant regrowth step runs.
pub const BIOME_STEP_INTERVAL: u64 = 10;

/// Advance the world by one tick.
pub fn step(world: &mut World) {
    world.resize_scratch();
    let cap = world.agents.capacity();

    // Stage 1: rebuild the spatial hash from current positions.
    world.spatial.rebuild(&world.agents.position, |i| world.agents.is_alive(i as u32));

    // Stage 2: sense.
    sense_all(&world.agents, &world.biome, &world.spatial, &mut world.sensors);

    // Stage 3: decide.
    decide_all(world);

    // Stage 4: integrate (motion + per-tick metabolism).
    integrate_all(&mut world.agents, &world.desired_velocity[..cap]);

    // Stage 5: interact (feeding).
    interact_all(&mut world.agents, &mut world.biome);

    // Stage 6: age + starve.
    age_and_starve(&mut world.agents);

    // Stage 7: periodic biome regrowth.
    if world.tick.is_multiple_of(BIOME_STEP_INTERVAL) {
        world.biome.regrow_step();
    }

    world.tick += 1;
}

fn decide_all(world: &mut World) {
    // Deterministic order: ascending id.
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
        let genome = world.agents.genome[i];
        let sensor = world.sensors[i];
        let energy = world.agents.energy[i];
        world.desired_velocity[i] = decide(&genome, &sensor, energy, &mut world.rng);
    }
    // Dead slots keep their old velocities; they're never read because
    // `integrate_all` only iterates alive ids.
}

#[cfg(test)]
mod tests {
    use crate::biome::TerrainType;
    use crate::genome::{Genome, GenomeSlot};
    use crate::prelude::Vec2;
    use crate::world::World;

    use super::step;

    #[test]
    fn empty_world_can_tick() {
        let mut w = World::new(1);
        for _ in 0..100 {
            step(&mut w);
        }
        assert_eq!(w.tick, 100);
    }

    #[test]
    fn agent_in_food_rich_world_survives_initial_ticks() {
        let mut w = World::new(13);
        // Find a grass cell to spawn near.
        let mut spawn = Vec2::ZERO;
        'outer: for row in 0..crate::biome::BIOME_RES {
            for col in 0..crate::biome::BIOME_RES {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    spawn = Vec2::new(
                        (col as f32 + 0.5) * crate::biome::CELL_SIZE,
                        (row as f32 + 0.5) * crate::biome::CELL_SIZE,
                    );
                    break 'outer;
                }
            }
        }
        let mut g = Genome::neutral();
        g.set(GenomeSlot::DietCarnivory, 0.0);
        g.set(GenomeSlot::LifespanBias, 1.0);
        let id = w.spawn_agent(spawn, g);
        for _ in 0..200 {
            step(&mut w);
            if !w.agents.is_alive(id) {
                break;
            }
        }
        assert!(w.agents.is_alive(id), "well-fed agent on grass should survive 200 ticks");
    }

    #[test]
    fn starving_agent_dies() {
        let mut w = World::new(1);
        // Pin the agent on a barren cell with no speed so it can't graze its
        // way back to life, then drain its energy to a sliver.
        let mut spawn = Vec2::ZERO;
        'outer: for row in 0..crate::biome::BIOME_RES {
            for col in 0..crate::biome::BIOME_RES {
                if w.biome.at(col, row).plant_biomass <= 0.0 {
                    spawn = Vec2::new(
                        (col as f32 + 0.5) * crate::biome::CELL_SIZE,
                        (row as f32 + 0.5) * crate::biome::CELL_SIZE,
                    );
                    break 'outer;
                }
            }
        }
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 0.0);
        let id = w.spawn_agent(spawn, g);
        w.agents.energy[id as usize] = 0.5;
        for _ in 0..200 {
            step(&mut w);
            if !w.agents.is_alive(id) {
                break;
            }
        }
        assert!(!w.agents.is_alive(id));
    }
}
