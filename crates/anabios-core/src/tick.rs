//! Tick orchestration: the master `step()` function for M1.

use crate::age::age_and_starve;
use crate::behavior::decide;
use crate::integrate::integrate_all;
use crate::interact::interact_all;
use crate::prelude::Vec2;
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
    integrate_all(&mut world.agents, &world.desired_direction[..cap]);

    // Stage 5: interact (feeding, combat, predation).
    interact_all(world);

    // M3: module upkeep — every alive agent pays for its modules.
    crate::module::upkeep_all(&mut world.agents);

    // Stage 6: reproduce. Mutates the alive set; do not rely on `cap` after
    // this point.
    crate::reproduce::reproduce_all(world);

    // Stage 7: age + starve.
    age_and_starve(world);

    // Stage 7b: carcass aging + removal (design step 9 analogue).
    crate::carcass::carcass_step(world);

    // Stage 8c: pheromone field decay (design §3.7 step 9).
    world.pheromones.decay_step();

    // Stage 8: periodic species clustering.
    if world.tick.is_multiple_of(crate::species::SPECIES_STEP_INTERVAL) {
        crate::species::species_step(world);
    }

    // Stage 9: codex detectors (extinction, population crash, etc.).
    crate::codex::observe_all(world);

    // Stage 10: periodic biome regrowth.
    if world.tick.is_multiple_of(BIOME_STEP_INTERVAL) {
        world.biome.regrow_step();
    }

    world.tick += 1;
}

fn decide_all(world: &mut World) {
    // Deterministic order: ascending id. Programs are evaluated against the
    // shared `world.eval_stack` scratch buffer.
    // Collect ids first to release the borrow on `world.agents` before the loop
    // body borrows `world` mutably (decide reads buffers, then we write back).
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
        let action = decide(
            &world.agents.program[i],
            &world.agents.genome[i],
            &world.sensors[i],
            world.agents.energy[i],
            world.agents.age[i],
            &mut world.eval_stack,
        );
        // Normalize the movement intent to a unit direction (identical to the
        // pre-M11 logic that lived inside `decide`).
        let v = Vec2::new(action.move_x, action.move_y);
        let len = v.length();
        world.desired_direction[i] = if len < 1e-4 { Vec2::ZERO } else { v / len };
        world.actions[i] = action;
    }
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
        let g = Genome::neutral();
        let id = w.spawn_agent(spawn, g);
        // Strip Locomotor so the agent can't graze its way back to life.
        w.agents.modules[id as usize]
            .retain(|m| !matches!(m, crate::module::Module::Locomotor { .. }));
        w.agents.energy[id as usize] = 0.5;
        for _ in 0..200 {
            step(&mut w);
            if !w.agents.is_alive(id) {
                break;
            }
        }
        assert!(!w.agents.is_alive(id));
    }

    #[test]
    fn decide_populates_action_buffer_with_target() {
        use crate::program::{Node, Program, NO_TARGET};
        let mut w = World::new(1);
        // A program that always fires with intent 1.0.
        let prog = Program::from_slice(&[Node::Const(1.0), Node::FireWeapon]);
        let a = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
        let b = w.spawn_agent(Vec2::new(404.0, 400.0), Genome::neutral());
        w.agents.program[a as usize] = prog;
        // One tick runs sense -> decide; afterwards actions[a] reflects the program.
        step(&mut w);
        assert!(w.actions[a as usize].fire_intent > 0.0);
        // a's nearest neighbor is b, so target should be b (not NO_TARGET).
        assert_eq!(w.actions[a as usize].target_id, b);
        assert_ne!(w.actions[a as usize].target_id, NO_TARGET);
    }
}
