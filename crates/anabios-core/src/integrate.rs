//! Integration step: applies desired directions to positions, wraps to the
//! torus, and drains energy proportional to movement plus a per-tick basal
//! metabolism cost.

use crate::agent::AgentBuffers;
#[cfg(test)]
use crate::biome::WORLD_SIZE;
use crate::genome::GenomeSlot;
use crate::prelude::{wrap_torus, Vec2};

/// Cost per world-unit of movement at `Size = 1.0`. Smaller agents pay less.
pub const MOVE_ENERGY_COST: f32 = 0.005;
/// Per-tick basal metabolism cost at `BasalMetabolism = 1.0`.
pub const BASAL_METABOLISM_COST: f32 = 0.05;

/// Maximum agent speed at `Locomotor.max_speed = 1.0`, in world units per
/// tick. Capping here keeps spatial-hash neighbor queries within their
/// `PERCEPTION_MAX_RADIUS` guarantee even when an agent has multiple
/// Locomotor modules (their max_speed contributions sum, then we clamp).
pub const SPEED_MAX_CAP: f32 = 4.0;

/// Apply `desired_direction[i]` to each alive agent, scaled by the agent's
/// effective Locomotor speed. Agents without a Locomotor still pay basal
/// metabolism but do not move.
pub fn integrate_all(agents: &mut AgentBuffers, desired_direction: &[Vec2], world_size: f32) {
    let mut ids = std::mem::take(&mut agents.scratch_ids);
    ids.clear();
    ids.extend(agents.iter_alive());
    for &id in &ids {
        let i = id as usize;

        // Action gating: no Locomotor → no motion.
        if !crate::module::has(&agents.modules[i], crate::module::ModuleType::Locomotor) {
            agents.velocity[i] = Vec2::ZERO;
            // Still pay basal metabolism.
            let basal = BASAL_METABOLISM_COST * agents.genome[i].get(GenomeSlot::BasalMetabolism);
            agents.energy[i] -= basal;
            continue;
        }

        let direction = desired_direction[i];
        let module_speed = crate::module::effective_speed_max(&agents.modules[i]).clamp(0.0, 1.0);
        // Openness scales effective speed (identity at neutral personality).
        let speed_factor = crate::personality::personality_speed_factor(&agents.genome[i]);
        let v = direction * (SPEED_MAX_CAP * module_speed * speed_factor);
        agents.velocity[i] = v;

        let new_pos = agents.position[i] + v;
        agents.position[i] = wrap_torus(new_pos, Vec2::splat(world_size));

        let move_dist = v.length();
        let size = agents.genome[i].get(GenomeSlot::Size).max(0.1);
        let move_cost = MOVE_ENERGY_COST * move_dist * size;
        let basal = BASAL_METABOLISM_COST * agents.genome[i].get(GenomeSlot::BasalMetabolism);
        agents.energy[i] -= move_cost + basal;
    }
    agents.scratch_ids = ids;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::SPAWN_ENERGY;
    use crate::genome::Genome;
    use crate::world::World;

    #[test]
    fn position_wraps_on_torus() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(WORLD_SIZE - 1.0, 0.5), Genome::neutral());
        // Force max-speed Locomotor so the unit direction produces a 4-unit step.
        for m in w.agents.modules[id as usize].iter_mut() {
            if let crate::module::Module::Locomotor { max_speed, .. } = m {
                *max_speed = 1.0;
            }
        }
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        // Move 3 ticks worth in one call by scaling the direction? No — direction
        // must be unit. Instead place agent close enough that one 4-unit step wraps.
        // WORLD_SIZE - 1.0 + 4.0 = WORLD_SIZE + 3.0 → wraps to 3.0.
        integrate_all(&mut w.agents, &desired, w.world_size);
        let p = w.agents.position[id as usize];
        assert!(p.x >= 0.0 && p.x < WORLD_SIZE);
        assert!((p.x - 3.0).abs() < 1e-3, "expected wrap-around to ~3.0, got {}", p.x);
    }

    #[test]
    fn motion_drains_energy_proportionally() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        for m in w.agents.modules[id as usize].iter_mut() {
            if let crate::module::Module::Locomotor { max_speed, .. } = m {
                *max_speed = 1.0;
            }
        }
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        let before = w.agents.energy[id as usize];
        integrate_all(&mut w.agents, &desired, w.world_size);
        let after = w.agents.energy[id as usize];
        assert!(after < before);
        // Speed is now SPEED_MAX_CAP * 1.0 = 4.0 units per tick.
        let expected_move_cost = MOVE_ENERGY_COST * 4.0 * 0.5; // size = 0.5 in neutral genome
        let expected_basal = BASAL_METABOLISM_COST * 0.5;
        let drained = before - after;
        assert!(
            (drained - (expected_move_cost + expected_basal)).abs() < 1e-3,
            "drained={drained}, expected~{}",
            expected_move_cost + expected_basal
        );
        // Sanity: still alive with non-zero energy.
        assert!(after < SPAWN_ENERGY);
    }

    #[test]
    fn agent_without_locomotor_does_not_move() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        // Strip Locomotor from the starter kit.
        w.agents.modules[id as usize]
            .retain(|m| !matches!(m, crate::module::Module::Locomotor { .. }));

        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        let pos_before = w.agents.position[id as usize];
        integrate_all(&mut w.agents, &desired, w.world_size);
        let pos_after = w.agents.position[id as usize];
        assert_eq!(pos_before, pos_after, "no Locomotor → no motion");
    }

    #[test]
    fn agent_with_locomotor_moves_proportionally_to_speed_param() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        // Replace starter kit Locomotor with a max-speed one.
        for m in w.agents.modules[id as usize].iter_mut() {
            if let crate::module::Module::Locomotor { max_speed, .. } = m {
                *max_speed = 1.0;
            }
        }

        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        integrate_all(&mut w.agents, &desired, w.world_size);
        let new_pos = w.agents.position[id as usize];
        // Moved roughly SPEED_MAX_CAP × 1.0 = 4.0 in +x.
        assert!((new_pos.x - 504.0).abs() < 0.1);
    }
}
