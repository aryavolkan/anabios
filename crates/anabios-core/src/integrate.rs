//! Integration step: applies desired velocities to positions, wraps to the
//! torus, and drains energy proportional to movement plus a per-tick basal
//! metabolism cost.

use crate::agent::AgentBuffers;
use crate::biome::WORLD_SIZE;
use crate::genome::GenomeSlot;
use crate::prelude::{wrap_torus, Vec2};

/// Cost per world-unit of movement at `Size = 1.0`. Smaller agents pay less.
pub const MOVE_ENERGY_COST: f32 = 0.005;
/// Per-tick basal metabolism cost at `BasalMetabolism = 1.0`.
pub const BASAL_METABOLISM_COST: f32 = 0.05;

/// Apply `desired_velocity[i]` to each alive agent.
pub fn integrate_all(agents: &mut AgentBuffers, desired_velocity: &[Vec2]) {
    for id in agents.iter_alive().collect::<Vec<_>>() {
        let i = id as usize;
        let v = desired_velocity[i];
        agents.velocity[i] = v;

        let new_pos = agents.position[i] + v;
        agents.position[i] = wrap_torus(new_pos, Vec2::splat(WORLD_SIZE));

        let move_dist = v.length();
        let size = agents.genome[i].get(GenomeSlot::Size).max(0.1);
        let move_cost = MOVE_ENERGY_COST * move_dist * size;
        let basal_cost = BASAL_METABOLISM_COST * agents.genome[i].get(GenomeSlot::BasalMetabolism);
        agents.energy[i] -= move_cost + basal_cost;
    }
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
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(3.0, 0.0);
        integrate_all(&mut w.agents, &desired);
        let p = w.agents.position[id as usize];
        assert!(p.x >= 0.0 && p.x < WORLD_SIZE);
        assert!((p.x - 2.0).abs() < 1e-3, "expected wrap-around to ~2.0, got {}", p.x);
    }

    #[test]
    fn motion_drains_energy_proportionally() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(4.0, 0.0);
        let before = w.agents.energy[id as usize];
        integrate_all(&mut w.agents, &desired);
        let after = w.agents.energy[id as usize];
        assert!(after < before);
        // Spawn energy should not have been touched outside the cost.
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
}
