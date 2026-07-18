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

/// Sentinel value in `SensorRegister.nearest_neighbor_species` meaning
/// "no neighbor". `Default` initializes the field to this value.
pub const NO_NEIGHBOR_SPECIES: u32 = u32::MAX;

/// Sentinel in `SensorRegister` id fields meaning "no such neighbor".
pub const NO_NEIGHBOR_ID: u32 = u32::MAX;

/// Per-agent sensor outputs computed each tick.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
    /// Species id of the nearest neighbor, or `NO_NEIGHBOR_SPECIES` when
    /// there is no neighbor. The sentinel is chosen so the default-
    /// initialized state of an uninhabited sensor register doesn't
    /// accidentally look like "compatible with species 0".
    pub nearest_neighbor_species: u32,
    /// Id of the nearest neighbor of any species, or `NO_NEIGHBOR_ID`.
    pub nearest_neighbor_id: u32,
    /// Distance to the nearest neighbor of the SAME species, or `f32::INFINITY`.
    pub nearest_same_dist: f32,
    /// Unit direction to the nearest same-species neighbor.
    pub nearest_same_dir: Vec2,
    /// Id of the nearest same-species neighbor, or `NO_NEIGHBOR_ID`.
    pub nearest_same_id: u32,
    /// Distance to the nearest neighbor of a DIFFERENT species.
    pub nearest_other_dist: f32,
    /// Unit direction to the nearest other-species neighbor.
    pub nearest_other_dir: Vec2,
    /// Id of the nearest other-species neighbor, or `NO_NEIGHBOR_ID`.
    pub nearest_other_id: u32,
    /// `other.size / self.size` of the overall-nearest neighbor; 0.0 if none.
    pub nearest_rel_size: f32,
    /// `other.energy / self.energy` of the overall-nearest neighbor; 0.0 if none.
    pub nearest_rel_energy: f32,
    /// Count of alive neighbors within perception radius.
    pub crowding: u32,
    /// Local pheromone concentration per channel (0 unless the agent has a
    /// `Smell` sensor). Read by `Node::SensePheromone`.
    pub pheromone: [f32; crate::program::PHEROMONE_CHANNELS],
    /// Kinship of the overall-nearest neighbor in `[0,1]`; 0.0 when there is
    /// no neighbor. Computed by `sense_all` after the neighbor scan.
    /// `#[serde(skip)]` scratch — no snapshot impact.
    #[serde(skip)]
    pub nearest_kinship: f32,
}

impl Default for SensorRegister {
    fn default() -> Self {
        Self {
            local_plant_biomass: 0.0,
            plant_direction: Vec2::ZERO,
            nearest_neighbor_dist: f32::INFINITY,
            nearest_neighbor_dir: Vec2::ZERO,
            has_neighbor: false,
            nearest_neighbor_species: NO_NEIGHBOR_SPECIES,
            nearest_neighbor_id: NO_NEIGHBOR_ID,
            nearest_same_dist: f32::INFINITY,
            nearest_same_dir: Vec2::ZERO,
            nearest_same_id: NO_NEIGHBOR_ID,
            nearest_other_dist: f32::INFINITY,
            nearest_other_dir: Vec2::ZERO,
            nearest_other_id: NO_NEIGHBOR_ID,
            nearest_rel_size: 0.0,
            nearest_rel_energy: 0.0,
            crowding: 0,
            pheromone: [0.0; crate::program::PHEROMONE_CHANNELS],
            nearest_kinship: 0.0,
        }
    }
}

/// Effective perception radius for an agent given its module list and
/// genome. Combines the max Sensor radius with the genome's
/// `PerceptionRadius` slot (the genome acts as a modulator on top of
/// module capability). Capped at `PERCEPTION_MAX_RADIUS` for the
/// spatial-hash one-ring guarantee.
pub fn perception_radius(modules: &crate::module::ModuleList, genome: &Genome) -> f32 {
    let sensor_radius = crate::module::effective_perception_radius(modules);
    if sensor_radius <= 0.0 {
        return 0.0;
    }
    let modulator = 0.25 + 0.75 * genome.get(GenomeSlot::PerceptionRadius);
    (PERCEPTION_MAX_RADIUS * sensor_radius * modulator).min(PERCEPTION_MAX_RADIUS)
}

/// Run the sense stage. `registers[i]` is populated for every alive agent;
/// dead slots are left unchanged. Caller owns `registers` and reuses it
/// across ticks to avoid per-tick allocation.
///
/// Each agent's register is a pure function of the (immutable) world inputs,
/// so the loop runs in parallel over rayon with index-disjoint writes —
/// results are bit-identical to the serial ascending-id loop.
pub fn sense_all(
    agents: &AgentBuffers,
    biome: &BiomeField,
    pheromones: &crate::pheromone::PheromoneField,
    spatial: &UniformSpatialHash,
    registers: &mut [SensorRegister],
) {
    use rayon::prelude::*;
    debug_assert!(registers.len() >= agents.capacity());
    let cap = agents.capacity();

    registers[..cap].par_iter_mut().enumerate().for_each(|(i, reg)| {
        if !agents.is_alive(i as u32) {
            return;
        }
        *reg = sense_one(i as u32, agents, biome, pheromones, spatial);
    });
}

/// Compute one alive agent's sensor register. Pure over the shared inputs.
fn sense_one(
    id: u32,
    agents: &AgentBuffers,
    biome: &BiomeField,
    pheromones: &crate::pheromone::PheromoneField,
    spatial: &UniformSpatialHash,
) -> SensorRegister {
    let i = id as usize;
    let pos = agents.position[i];
    let genome = &agents.genome[i];
    let radius = perception_radius(&agents.modules[i], genome);
    if radius <= 0.0 {
        return SensorRegister::default();
    }

    let local_cell = biome.sample(pos);
    let plant_direction = best_plant_direction(biome, pos, radius);

    let self_species = agents.species_id[i];
    let self_size = genome.get(GenomeSlot::Size).max(1e-3);
    let self_energy = agents.energy[i].max(1e-3);

    let mut nearest_dist = f32::INFINITY;
    let mut nearest_dir = Vec2::ZERO;
    let mut has_neighbor = false;
    let mut nearest_species: u32 = NO_NEIGHBOR_SPECIES;
    let mut nearest_id: u32 = NO_NEIGHBOR_ID;
    let mut nearest_rel_size = 0.0_f32;
    let mut nearest_rel_energy = 0.0_f32;
    let mut same_dist = f32::INFINITY;
    let mut same_dir = Vec2::ZERO;
    let mut same_id: u32 = NO_NEIGHBOR_ID;
    let mut other_dist = f32::INFINITY;
    let mut other_dir = Vec2::ZERO;
    let mut other_id: u32 = NO_NEIGHBOR_ID;
    let mut crowding: u32 = 0;

    spatial.query(pos, radius, |oid| {
        if oid == id {
            return;
        }
        let other_pos = agents.position[oid as usize];
        let d = torus_distance(pos, other_pos);
        if d > radius {
            return;
        }
        crowding += 1;
        let dir = torus_direction(pos, other_pos);
        let other_species = agents.species_id[oid as usize];
        if d < nearest_dist {
            nearest_dist = d;
            nearest_dir = dir;
            has_neighbor = true;
            nearest_species = other_species;
            nearest_id = oid;
            nearest_rel_size = agents.genome[oid as usize].get(GenomeSlot::Size) / self_size;
            // The hash holds only alive agents (rebuilt with the alive
            // predicate before sense), so the neighbor's energy is >= 0.
            nearest_rel_energy = agents.energy[oid as usize] / self_energy;
        }
        if other_species == self_species {
            if d < same_dist {
                same_dist = d;
                same_dir = dir;
                same_id = oid;
            }
        } else if d < other_dist {
            other_dist = d;
            other_dir = dir;
            other_id = oid;
        }
    });

    // Pheromone perception is gated by a Smell sensor module.
    let pheromone = if crate::module::has_smell(&agents.modules[i]) {
        let pos = agents.position[i];
        let mut ch_vals = [0.0f32; crate::program::PHEROMONE_CHANNELS];
        for (ch, v) in ch_vals.iter_mut().enumerate() {
            *v = pheromones.sample(pos, ch);
        }
        ch_vals
    } else {
        [0.0; crate::program::PHEROMONE_CHANNELS]
    };

    let mut reg = SensorRegister {
        local_plant_biomass: local_cell.plant_biomass,
        plant_direction,
        nearest_neighbor_dist: nearest_dist,
        nearest_neighbor_dir: nearest_dir,
        has_neighbor,
        nearest_neighbor_species: nearest_species,
        nearest_neighbor_id: nearest_id,
        nearest_same_dist: same_dist,
        nearest_same_dir: same_dir,
        nearest_same_id: same_id,
        nearest_other_dist: other_dist,
        nearest_other_dir: other_dir,
        nearest_other_id: other_id,
        nearest_rel_size,
        nearest_rel_energy,
        crowding,
        pheromone,
        nearest_kinship: 0.0,
    };

    // Kinship of the overall-nearest neighbor (0 when there is none).
    reg.nearest_kinship = if has_neighbor {
        let n = nearest_id as usize;
        crate::kin::kinship(
            agents.lineage_id[i],
            &agents.parent_ids[i],
            &agents.genome[i],
            agents.lineage_id[n],
            &agents.parent_ids[n],
            &agents.genome[n],
        )
    } else {
        0.0
    };
    reg
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
    use crate::genome::GenomeSlot;
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
        sense_all(&w.agents, &w.biome, &w.pheromones, &w.spatial, &mut regs);
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
        sense_all(&w.agents, &w.biome, &w.pheromones, &w.spatial, &mut regs);
        assert!(regs[0].has_neighbor);
        assert!((regs[0].nearest_neighbor_dist - 4.0).abs() < 1e-3);
        assert!(regs[0].nearest_neighbor_dir.x > 0.9);
        assert_eq!(regs[0].nearest_neighbor_species, 0);
    }

    #[test]
    fn agent_without_sensor_perceives_nothing() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.agents.modules[id as usize]
            .retain(|m| !matches!(m, crate::module::Module::Sensor { .. }));
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.pheromones, &w.spatial, &mut regs);
        assert_eq!(regs[id as usize].local_plant_biomass, 0.0);
        assert!(!regs[id as usize].has_neighbor);
    }

    #[test]
    fn isolated_agent_has_no_neighbor() {
        let mut w = World::new(1);
        let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.pheromones, &w.spatial, &mut regs);
        assert!(!regs[0].has_neighbor);
        assert_eq!(regs[0].nearest_neighbor_dist, f32::INFINITY);
        assert_eq!(regs[0].nearest_neighbor_species, NO_NEIGHBOR_SPECIES);
        assert_eq!(regs[0].nearest_neighbor_id, NO_NEIGHBOR_ID);
        assert_eq!(regs[0].nearest_rel_size, 0.0);
        assert_eq!(regs[0].nearest_rel_energy, 0.0);
        assert_eq!(regs[0].crowding, 0);
    }

    #[test]
    fn distinguishes_same_and_other_species() {
        let mut w = World::new(1);
        let me = w.spawn_agent(Vec2::new(100.0, 100.0), Genome::neutral());
        let kin = w.spawn_agent(Vec2::new(106.0, 100.0), Genome::neutral()); // same species 0
        let foe = w.spawn_agent(Vec2::new(103.0, 100.0), Genome::neutral());
        w.agents.species_id[foe as usize] = 1; // make foe another species
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.pheromones, &w.spatial, &mut regs);
        let r = regs[me as usize];
        assert_eq!(r.nearest_same_id, kin);
        assert!((r.nearest_same_dist - 6.0).abs() < 1e-3);
        assert!(r.nearest_same_dir.x > 0.9);
        assert_eq!(r.nearest_other_id, foe);
        assert!((r.nearest_other_dist - 3.0).abs() < 1e-3);
        assert!(r.nearest_other_dir.x > 0.9);
        // Overall nearest is the foe (3 < 6).
        assert_eq!(r.nearest_neighbor_id, foe);
    }

    #[test]
    fn relative_size_and_energy_of_nearest() {
        let mut w = World::new(1);
        let mut big = Genome::neutral();
        big.set(GenomeSlot::Size, 1.0);
        let mut small = Genome::neutral();
        small.set(GenomeSlot::Size, 0.5);
        let me = w.spawn_agent(Vec2::new(200.0, 200.0), small);
        let other = w.spawn_agent(Vec2::new(204.0, 200.0), big);
        w.agents.energy[me as usize] = 20.0;
        w.agents.energy[other as usize] = 40.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.pheromones, &w.spatial, &mut regs);
        let r = regs[me as usize];
        assert!(
            (r.nearest_rel_size - 2.0).abs() < 1e-3,
            "1.0/0.5 = 2.0, got {}",
            r.nearest_rel_size
        );
        assert!(
            (r.nearest_rel_energy - 2.0).abs() < 1e-3,
            "40/20 = 2.0, got {}",
            r.nearest_rel_energy
        );
    }

    #[test]
    fn crowding_counts_neighbors_in_radius() {
        let mut w = World::new(1);
        let me = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let _ = w.spawn_agent(Vec2::new(303.0, 300.0), Genome::neutral());
        let _ = w.spawn_agent(Vec2::new(300.0, 303.0), Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.pheromones, &w.spatial, &mut regs);
        assert_eq!(regs[me as usize].crowding, 2);
    }
}
