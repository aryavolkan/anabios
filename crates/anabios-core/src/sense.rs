//! Per-agent sensor sampling.
//!
//! `sense()` reads world state and writes each alive agent's `SensorRegister`.
//! All values are deterministic functions of the world buffers and the
//! agent's position.

use serde::{Deserialize, Serialize};

use crate::agent::AgentBuffers;
use crate::biome::{BiomeCell, BiomeField, CELL_SIZE, WORLD_SIZE};
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::{wrap_torus, Vec2};
use crate::spatial::{torus_distance, UniformSpatialHash, PERCEPTION_MAX_RADIUS};

/// Per-agent sensor outputs computed each tick.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SensorRegister {
    /// Plant biomass in the agent's own cell.
    pub local_plant_biomass: f32,
    /// Direction (unit) to the highest-biomass cell within perception, or
    /// zero if no edible cell exists in range.
    pub plant_direction: Vec2,
    /// Distance to the nearest other alive agent on the torus, or `f32::INFINITY`.
    pub nearest_neighbor_dist: f32,
    /// Direction (unit) to that nearest neighbor.
    pub nearest_neighbor_dir: Vec2,
    /// Whether the agent currently has any alive neighbor in perception.
    pub has_neighbor: bool,
}

/// Effective perception radius for an agent given its genome.
#[inline]
pub fn perception_radius(genome: &Genome) -> f32 {
    // Perception radius scales between 25% and 100% of the engine cap.
    let frac = 0.25 + 0.75 * genome.get(GenomeSlot::PerceptionRadius);
    PERCEPTION_MAX_RADIUS * frac
}

/// Run the sense stage. `registers[i]` is populated for every alive agent;
/// dead slots are left unchanged. Caller owns `registers` and reuses it
/// across ticks to avoid per-tick allocation.
pub fn sense_all(
    agents: &AgentBuffers,
    biome: &BiomeField,
    spatial: &UniformSpatialHash,
    registers: &mut [SensorRegister],
) {
    debug_assert!(registers.len() >= agents.capacity());

    for id in agents.iter_alive() {
        let i = id as usize;
        let pos = agents.position[i];
        let genome = &agents.genome[i];
        let radius = perception_radius(genome);

        let local_cell = biome.sample(pos);
        let plant_direction = best_plant_direction(biome, pos, radius);

        let mut nearest_dist = f32::INFINITY;
        let mut nearest_dir = Vec2::ZERO;
        let mut has_neighbor = false;
        spatial.query(pos, radius, |other_id| {
            if other_id == id {
                return;
            }
            let other_pos = agents.position[other_id as usize];
            let d = torus_distance(pos, other_pos);
            if d <= radius && d < nearest_dist {
                nearest_dist = d;
                nearest_dir = torus_direction(pos, other_pos);
                has_neighbor = true;
            }
        });

        registers[i] = SensorRegister {
            local_plant_biomass: local_cell.plant_biomass,
            plant_direction,
            nearest_neighbor_dist: nearest_dist,
            nearest_neighbor_dir: nearest_dir,
            has_neighbor,
        };
    }
}

/// Find the direction toward the best-biomass biome cell within `radius`.
/// Returns `Vec2::ZERO` if no cell in range has positive biomass.
fn best_plant_direction(biome: &BiomeField, pos: Vec2, radius: f32) -> Vec2 {
    let mut best_biomass = 0.0_f32;
    let mut best_offset = Vec2::ZERO;
    let cell_reach = (radius / CELL_SIZE).ceil() as i32 + 1;
    let (cx, cy) = BiomeField::cell_coords(pos);

    for dy in -cell_reach..=cell_reach {
        for dx in -cell_reach..=cell_reach {
            let col = ((cx as i32 + dx).rem_euclid(crate::biome::BIOME_RES as i32)) as usize;
            let row = ((cy as i32 + dy).rem_euclid(crate::biome::BIOME_RES as i32)) as usize;
            let cell: &BiomeCell = biome.at(col, row);
            if cell.plant_biomass <= 0.0 {
                continue;
            }
            let cell_center =
                Vec2::new((col as f32 + 0.5) * CELL_SIZE, (row as f32 + 0.5) * CELL_SIZE);
            let offset = wrap_torus(
                cell_center - pos + Vec2::splat(WORLD_SIZE * 0.5),
                Vec2::splat(WORLD_SIZE),
            ) - Vec2::splat(WORLD_SIZE * 0.5);
            let dist = offset.length();
            if dist > radius {
                continue;
            }
            if cell.plant_biomass > best_biomass {
                best_biomass = cell.plant_biomass;
                best_offset = offset;
            }
        }
    }

    if best_biomass <= 0.0 {
        Vec2::ZERO
    } else {
        best_offset.normalize_or_zero()
    }
}

/// Wrap-aware direction unit vector from `from` toward `to`.
fn torus_direction(from: Vec2, to: Vec2) -> Vec2 {
    let mut dx = to.x - from.x;
    let mut dy = to.y - from.y;
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
    Vec2::new(dx, dy).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;
    use crate::world::World;

    #[test]
    fn agent_on_grass_sees_local_biomass() {
        let mut w = World::new(7);
        // Find any grass cell and spawn an agent at its center.
        let mut spawn = Vec2::ZERO;
        'outer: for row in 0..crate::biome::BIOME_RES {
            for col in 0..crate::biome::BIOME_RES {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    spawn =
                        Vec2::new((col as f32 + 0.5) * CELL_SIZE, (row as f32 + 0.5) * CELL_SIZE);
                    break 'outer;
                }
            }
        }
        let _ = w.spawn_agent(spawn, Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.spatial, &mut regs);
        assert!(regs[0].local_plant_biomass > 0.0);
    }

    #[test]
    fn agent_finds_neighbor_within_perception() {
        let mut w = World::new(1);
        let pos_a = Vec2::new(100.0, 100.0);
        let pos_b = Vec2::new(104.0, 100.0);
        let _ = w.spawn_agent(pos_a, Genome::neutral());
        let _ = w.spawn_agent(pos_b, Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.spatial, &mut regs);
        assert!(regs[0].has_neighbor);
        assert!((regs[0].nearest_neighbor_dist - 4.0).abs() < 1e-3);
        assert!(regs[0].nearest_neighbor_dir.x > 0.9);
    }

    #[test]
    fn isolated_agent_has_no_neighbor() {
        let mut w = World::new(1);
        let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.spatial, &mut regs);
        assert!(!regs[0].has_neighbor);
        assert_eq!(regs[0].nearest_neighbor_dist, f32::INFINITY);
    }
}
