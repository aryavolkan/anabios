//! Per-agent sensor sampling.
//!
//! `sense()` reads world state and writes each alive agent's `SensorRegister`.
//! All values are deterministic functions of the world buffers and the
//! agent's position.

use serde::{Deserialize, Serialize};

use crate::agent::AgentBuffers;
#[cfg(test)]
use crate::biome::CELL_SIZE;
use crate::biome::{BiomeCell, BiomeField};
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::{wrap_torus, Vec2};
use crate::spatial::{torus_distance_sq, UniformSpatialHash};

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
    /// War hostility of the nearest OTHER-species neighbor's species toward
    /// the agent's own (E7), in `[0,1]`; 0 when none / no hostility.
    /// `#[serde(skip)]` scratch — no snapshot impact.
    #[serde(skip)]
    pub hostility: f32,
    /// Tool-bearing culture-lineage threat of the nearest OTHER-species
    /// neighbor (anthropogenic arms race): `weapon damage / DAMAGE_MAX` in
    /// `[0,1]` when that neighbor belongs to a `culture_bearer` lineage and
    /// carries a weapon; exactly 0.0 otherwise — including whenever
    /// `anthro_race_enabled` is off (the culture mask is empty then), so
    /// flag-off worlds are byte-identical.
    /// `#[serde(skip)]` scratch — no snapshot impact.
    #[serde(skip)]
    pub culture_threat: f32,
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
            hostility: 0.0,
            culture_threat: 0.0,
        }
    }
}

/// Effective perception radius for an agent given its module list and
/// genome. Combines the max Sensor radius with the genome's
/// `PerceptionRadius` slot (the genome acts as a modulator on top of
/// module capability). Capped at `max_radius` (the world's spatial hash's
/// `perception_max_radius()`) for the spatial-hash one-ring guarantee.
pub fn perception_radius(
    modules: &crate::module::ModuleList,
    genome: &Genome,
    max_radius: f32,
) -> f32 {
    let sensor_radius = crate::module::effective_perception_radius(modules);
    if sensor_radius <= 0.0 {
        return 0.0;
    }
    let modulator = 0.25 + 0.75 * genome.get(GenomeSlot::PerceptionRadius);
    (max_radius * sensor_radius * modulator).min(max_radius)
}

/// Run the sense stage. `registers[i]` is populated for every alive agent;
/// dead slots are left unchanged. Caller owns `registers` and reuses it
/// across ticks to avoid per-tick allocation.
///
/// Each agent's register is a pure function of the (immutable) world inputs,
/// so the loop runs in parallel over rayon with index-disjoint writes —
/// results are bit-identical to the serial ascending-id loop.
#[allow(clippy::too_many_arguments)]
pub fn sense_all(
    agents: &AgentBuffers,
    biome: &BiomeField,
    pheromones: &crate::pheromone::PheromoneField,
    spatial: &UniformSpatialHash,
    hostility: &std::collections::BTreeMap<(u32, u32), crate::codex::HostilityRecord>,
    culture_mask: &[bool],
    registers: &mut [SensorRegister],
    world_size: f32,
    gene_tech_coupling: bool,
) {
    use rayon::prelude::*;
    debug_assert!(registers.len() >= agents.capacity());
    let cap = agents.capacity();
    let max_radius = spatial.perception_max_radius();

    registers[..cap].par_iter_mut().enumerate().for_each(|(i, reg)| {
        if !agents.is_alive(i as u32) {
            // Dead slots carry no stale state. `registers` is per-tick
            // scratch (`#[serde(skip)]`): a dead slot's stale register would
            // survive in a continuous run but read as default after a
            // snapshot load — and a newborn reusing the slot mid-tick would
            // then diverge between the two (the codex agg reads crowding for
            // alive newborns). Same invariant as the dead-slot zeroing of
            // `prev_desired_direction` in `codex::signatures`.
            *reg = SensorRegister::default();
            return;
        }
        *reg = sense_one(
            i as u32,
            agents,
            biome,
            pheromones,
            spatial,
            hostility,
            culture_mask,
            max_radius,
            world_size,
            gene_tech_coupling,
        );
    });
}

/// Running "closest neighbor" bookkeeping for one agent's sense scan.
///
/// Tracks the nearest neighbor overall, plus the nearest of the same and of a
/// different species. Directions and relative size/energy are derived once,
/// *after* the scan (see `sense_one`), so the hot per-neighbor path does no
/// `normalize` (a `sqrt`) and no division — only the ≤3 winners pay for those.
///
/// Distances are held **squared** (`torus_distance_sq`): ordering and the
/// radius reject are monotonic under squaring, so the winners are identical,
/// and the actual distances the register stores are recovered with a single
/// `sqrt` per winner in `sense_one` (`sqrt(dist_sq)` is bit-identical to the
/// former per-neighbor `torus_distance`, and `sqrt(∞) == ∞` keeps the
/// no-neighbor sentinel intact). The per-neighbor `sqrt` is gone.
///
/// The winner's *position* is captured here as it streams past (it is already
/// in-register for the distance test), so the post-scan direction math reads
/// the stored `Vec2` instead of re-fetching `agents.position[id]` — a cold,
/// random-index load at 10k agents that otherwise costs more than the `sqrt`s
/// the deferral saves. Values are bit-identical either way; this only changes
/// *where* the position comes from.
#[derive(Clone, Copy)]
struct NearestNeighbors {
    crowding: u32,
    nearest_dist_sq: f32,
    nearest_id: u32,
    nearest_species: u32,
    nearest_pos: Vec2,
    same_dist_sq: f32,
    same_id: u32,
    same_pos: Vec2,
    other_dist_sq: f32,
    other_id: u32,
    other_pos: Vec2,
}

impl NearestNeighbors {
    #[inline]
    fn new() -> Self {
        Self {
            crowding: 0,
            nearest_dist_sq: f32::INFINITY,
            nearest_id: NO_NEIGHBOR_ID,
            nearest_species: NO_NEIGHBOR_SPECIES,
            nearest_pos: Vec2::ZERO,
            same_dist_sq: f32::INFINITY,
            same_id: NO_NEIGHBOR_ID,
            same_pos: Vec2::ZERO,
            other_dist_sq: f32::INFINITY,
            other_id: NO_NEIGHBOR_ID,
            other_pos: Vec2::ZERO,
        }
    }

    /// Fold one in-range neighbor — id `oid`, at `pos`, *squared* distance
    /// `d_sq`, of `species` — into the running winners. Selection uses strict
    /// `<`, so the first neighbor seen at a given distance wins, preserving the
    /// ascending query order the former inline scan relied on for tie-breaks.
    #[inline]
    fn consider(&mut self, oid: u32, pos: Vec2, d_sq: f32, species: u32, self_species: u32) {
        self.crowding += 1;
        if d_sq < self.nearest_dist_sq {
            self.nearest_dist_sq = d_sq;
            self.nearest_id = oid;
            self.nearest_species = species;
            self.nearest_pos = pos;
        }
        if species == self_species {
            if d_sq < self.same_dist_sq {
                self.same_dist_sq = d_sq;
                self.same_id = oid;
                self.same_pos = pos;
            }
        } else if d_sq < self.other_dist_sq {
            self.other_dist_sq = d_sq;
            self.other_id = oid;
            self.other_pos = pos;
        }
    }

    #[inline]
    fn has_neighbor(&self) -> bool {
        self.nearest_id != NO_NEIGHBOR_ID
    }
}

/// Compute one alive agent's sensor register. Pure over the shared inputs.
#[allow(clippy::too_many_arguments)]
fn sense_one(
    id: u32,
    agents: &AgentBuffers,
    biome: &BiomeField,
    pheromones: &crate::pheromone::PheromoneField,
    spatial: &UniformSpatialHash,
    hostility: &std::collections::BTreeMap<(u32, u32), crate::codex::HostilityRecord>,
    culture_mask: &[bool],
    max_radius: f32,
    world_size: f32,
    gene_tech_coupling: bool,
) -> SensorRegister {
    let i = id as usize;
    let pos = agents.position[i];
    let genome = &agents.genome[i];
    // Electricity buff: powered sensors extend perception (identity at mask 0).
    let radius = perception_radius(&agents.modules[i], genome, max_radius)
        * crate::invention::perception_multiplier_coupled(
            crate::invention::held_mask(&agents.meme_vector[i]),
            genome,
            gene_tech_coupling,
        );
    let radius = radius.min(max_radius);
    if radius <= 0.0 {
        return SensorRegister::default();
    }

    let local_cell = biome.sample(pos);
    let plant_direction = best_plant_direction(biome, pos, radius);

    let self_species = agents.species_id[i];
    let self_size = genome.get(GenomeSlot::Size).max(1e-3);
    let self_energy = agents.energy[i].max(1e-3);

    // Scan the neighbor ring recording only squared distances + winner ids
    // (cheap compares, no `sqrt`). Distance `sqrt`s, directions, and
    // relative-metric math are all deferred to the ≤3 winners below, so a
    // crowded agent no longer pays a `sqrt`/`normalize` per neighbor it never
    // keeps. `radius_sq` rejects out-of-range neighbors (monotonic under
    // squaring, so the kept set is identical to the `d > radius` test).
    let radius_sq = radius * radius;
    let mut nn = NearestNeighbors::new();
    spatial.query(pos, radius, |oid| {
        if oid == id {
            return;
        }
        let other_pos = agents.position[oid as usize];
        let d_sq = torus_distance_sq(pos, other_pos, world_size);
        if d_sq > radius_sq {
            return;
        }
        nn.consider(oid, other_pos, d_sq, agents.species_id[oid as usize], self_species);
    });

    let has_neighbor = nn.has_neighbor();
    // Derive the stored directions + relative metrics once, from the winners.
    // Each is bit-identical to what the inline scan wrote: same winner
    // (selection is unchanged), same neighbor position (captured during the
    // scan) / genome / energy fed to the same function. `nearest_rel_energy`:
    // the hash holds only alive agents, so the neighbor's energy is >= 0.
    let (nearest_dir, nearest_rel_size, nearest_rel_energy) = if has_neighbor {
        let n = nn.nearest_id as usize;
        (
            torus_direction(pos, nn.nearest_pos, world_size),
            agents.genome[n].get(GenomeSlot::Size) / self_size,
            agents.energy[n] / self_energy,
        )
    } else {
        (Vec2::ZERO, 0.0, 0.0)
    };
    let same_dir = if nn.same_id != NO_NEIGHBOR_ID {
        torus_direction(pos, nn.same_pos, world_size)
    } else {
        Vec2::ZERO
    };
    let other_dir = if nn.other_id != NO_NEIGHBOR_ID {
        torus_direction(pos, nn.other_pos, world_size)
    } else {
        Vec2::ZERO
    };

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
        // Recover the actual distances with one `sqrt` per winner. Bit-
        // identical to the former per-neighbor `torus_distance`, and
        // `sqrt(∞) == ∞` preserves the no-neighbor sentinel.
        nearest_neighbor_dist: nn.nearest_dist_sq.sqrt(),
        nearest_neighbor_dir: nearest_dir,
        has_neighbor,
        nearest_neighbor_species: nn.nearest_species,
        nearest_neighbor_id: nn.nearest_id,
        nearest_same_dist: nn.same_dist_sq.sqrt(),
        nearest_same_dir: same_dir,
        nearest_same_id: nn.same_id,
        nearest_other_dist: nn.other_dist_sq.sqrt(),
        nearest_other_dir: other_dir,
        nearest_other_id: nn.other_id,
        nearest_rel_size,
        nearest_rel_energy,
        crowding: nn.crowding,
        pheromone,
        nearest_kinship: 0.0,
        hostility: 0.0,
        culture_threat: 0.0,
    };

    // War hostility of the nearest OTHER-species neighbor's species.
    reg.hostility = if nn.other_id != NO_NEIGHBOR_ID {
        crate::codex::war::hostility_lookup(
            hostility,
            self_species,
            agents.species_id[nn.other_id as usize],
        )
    } else {
        0.0
    };
    // Anthropogenic arms race: is the nearest OTHER-species neighbor a
    // tool-bearing culture-lineage agent? Threat scales with its weapon
    // damage. The empty mask (flag off / nothing tagged) yields 0.0 for
    // every agent, keeping flag-off worlds byte-identical.
    reg.culture_threat = if nn.other_id != NO_NEIGHBOR_ID {
        let o = nn.other_id as usize;
        let tagged = culture_mask.get(agents.species_id[o] as usize).copied().unwrap_or(false);
        if tagged {
            crate::module::effective_weapon(&agents.modules[o])
                .map(|w| (w.damage / crate::module::DAMAGE_MAX).clamp(0.0, 1.0))
                .unwrap_or(0.0)
        } else {
            0.0
        }
    } else {
        0.0
    };
    // Kinship of the overall-nearest neighbor (0 when there is none).
    reg.nearest_kinship = if has_neighbor {
        let n = nn.nearest_id as usize;
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
    let cell_reach = (radius / biome.cell_size).ceil() as i32 + 1;
    let (cx, cy) = biome.cell_coords(pos);
    // The distance is only used to reject cells outside `radius`; the returned
    // direction is chosen by biomass, not distance. Comparing squared lengths
    // gives the identical reject set (both sides ≥ 0, `x → x*x` monotonic) while
    // dropping a `sqrt` from every scanned biomass-positive cell.
    let radius_sq = radius * radius;

    for dy in -cell_reach..=cell_reach {
        // `row` and its cell-center y depend only on `dy`; hoist them out of the
        // inner `dx` loop (values unchanged — the same cell is read either way).
        let row = ((cy as i32 + dy).rem_euclid(biome.res as i32)) as usize;
        let center_y = (row as f32 + 0.5) * biome.cell_size;
        for dx in -cell_reach..=cell_reach {
            let col = ((cx as i32 + dx).rem_euclid(biome.res as i32)) as usize;
            let cell: &BiomeCell = biome.at(col, row);
            if cell.plant_biomass <= 0.0 {
                continue;
            }
            let cell_center = Vec2::new((col as f32 + 0.5) * biome.cell_size, center_y);
            let offset = wrap_torus(
                cell_center - pos + Vec2::splat(biome.world_size * 0.5),
                Vec2::splat(biome.world_size),
            ) - Vec2::splat(biome.world_size * 0.5);
            if offset.length_squared() > radius_sq {
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
fn torus_direction(from: Vec2, to: Vec2, world_size: f32) -> Vec2 {
    crate::spatial::torus_delta(to, from, world_size).normalize_or_zero()
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
        sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &w.culture_mask,
            &mut regs,
            w.world_size,
            false,
        );
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
        sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &w.culture_mask,
            &mut regs,
            w.world_size,
            false,
        );
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
        sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &w.culture_mask,
            &mut regs,
            w.world_size,
            false,
        );
        assert_eq!(regs[id as usize].local_plant_biomass, 0.0);
        assert!(!regs[id as usize].has_neighbor);
    }

    #[test]
    fn dead_slots_are_reset_to_default() {
        // Snapshot-restore invariant: dead slots carry no stale register, so
        // a newborn reusing the slot mid-tick reads the same default a
        // snapshot-loaded world would have.
        let mut w = World::new(1);
        let a = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let b = w.spawn_agent(Vec2::new(502.0, 500.0), Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &w.culture_mask,
            &mut regs,
            w.world_size,
            false,
        );
        assert!(regs[a as usize].crowding > 0, "neighbour seen while both alive");

        w.agents.kill(a);
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &w.culture_mask,
            &mut regs,
            w.world_size,
            false,
        );
        assert_eq!(
            format!("{:?}", regs[a as usize]),
            format!("{:?}", SensorRegister::default()),
            "dead slot's stale register is cleared"
        );
        assert_eq!(regs[b as usize].crowding, 0, "survivor no longer crowded");
    }

    #[test]
    fn isolated_agent_has_no_neighbor() {
        let mut w = World::new(1);
        let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &w.culture_mask,
            &mut regs,
            w.world_size,
            false,
        );
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
        sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &w.culture_mask,
            &mut regs,
            w.world_size,
            false,
        );
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
        sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &w.culture_mask,
            &mut regs,
            w.world_size,
            false,
        );
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
        sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &w.culture_mask,
            &mut regs,
            w.world_size,
            false,
        );
        assert_eq!(regs[me as usize].crowding, 2);
    }
}
