//! `World` is the root state object owned by every simulation. It carries
//! the RNG, biome field, agent buffers, spatial hash, and tick counter.
//! Nothing outside this struct holds simulation state.

use bitvec::vec::BitVec;
use serde::{Deserialize, Serialize};

use crate::agent::{AgentBuffers, AgentId, LineageId, LINEAGE_NONE};
use crate::biome::{BiomeField, WORLD_SIZE};
use crate::genome::Genome;
use crate::prelude::Vec2;
use crate::rng::Rng;
use crate::spatial::UniformSpatialHash;

/// World root struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub tick: u64,
    pub seed: u64,
    pub rng: Rng,
    pub biome: BiomeField,
    pub agents: AgentBuffers,
    /// Next lineage id to allocate. Monotonically increasing.
    /// Lineage id 0 is reserved as `LINEAGE_NONE` (no parent).
    pub next_lineage_id: LineageId,
    /// Per-species mean genome. Indexed by `SpeciesId`. Empty entries
    /// (extinct species) are kept in place so existing ids stay stable;
    /// `species_member_counts[id] == 0` marks them.
    pub species_centroids: Vec<crate::genome::Genome>,
    /// **Only authoritative immediately after `species::species_step` has
    /// run.** Between any `agents.spawn` / `agents.kill` and the next
    /// `species_step` (which recomputes from `iter_alive`), these counts
    /// may be stale. M3 will track counts incrementally on spawn/kill;
    /// until then, do not read this field from gameplay code outside of
    /// `species_step` itself.
    pub species_member_counts: Vec<u32>,
    /// Parent species id for each species. `None` for founder species
    /// (initially only species 0). Indexed by `SpeciesId`.
    pub species_parents: Vec<Option<u32>>,
    /// Next species id to allocate.
    pub next_species_id: u32,
    #[serde(skip)]
    pub spatial: UniformSpatialHash,
    #[serde(skip)]
    pub sensors: Vec<crate::sense::SensorRegister>,
    #[serde(skip)]
    pub desired_velocity: Vec<crate::prelude::Vec2>,
    /// Per-agent BitVec marking who has already mated this tick.
    /// Cleared at the start of `reproduce_all`.
    // allow: filled by Task 6
    #[serde(skip)]
    pub reproduced_this_tick: BitVec,
}

impl World {
    /// Build a world from a seed: deterministic biome + empty agent
    /// population + fresh spatial hash + tick 0.
    pub fn new(seed: u64) -> Self {
        Self {
            tick: 0,
            seed,
            rng: Rng::from_seed(seed),
            biome: BiomeField::generate(seed),
            agents: AgentBuffers::new(),
            // Start at 1 — id 0 is reserved as LINEAGE_NONE for founder parents.
            next_lineage_id: 1,
            // Species 0 is the founder; centroid will be initialized by
            // the first call to `species_step` once agents exist.
            species_centroids: vec![Genome::neutral()],
            species_member_counts: vec![0],
            species_parents: vec![None],
            next_species_id: 1,
            spatial: UniformSpatialHash::new(),
            sensors: Vec::new(),
            desired_velocity: Vec::new(),
            reproduced_this_tick: BitVec::new(),
        }
    }

    /// Allocate a fresh, globally-unique lineage id. Never reuses values.
    #[inline]
    pub fn next_lineage(&mut self) -> LineageId {
        let id = self.next_lineage_id;
        self.next_lineage_id = self
            .next_lineage_id
            .checked_add(1)
            .expect("lineage id overflow: 2^64 births is implausible");
        id
    }

    /// Spawn a founder agent (no modelled parents) into the world. Lineage
    /// id is allocated here; species id is 0 (the founder species).
    pub fn spawn_agent(&mut self, position: Vec2, genome: Genome) -> AgentId {
        let lineage = self.next_lineage();
        self.agents.spawn(position, genome, lineage, [LINEAGE_NONE; 2], 0)
    }

    /// World dimensions (for callers that want the constant without
    /// importing the biome module directly).
    #[inline]
    pub fn size(&self) -> f32 {
        WORLD_SIZE
    }

    /// Sanity helper used by tests and the headless CLI.
    pub fn alive_energy_total(&self) -> f32 {
        let mut total = 0.0;
        for id in self.agents.iter_alive() {
            total += self.agents.energy[id as usize];
        }
        total
    }

    /// Sum of plant biomass across the biome.
    pub fn plant_biomass_total(&self) -> f32 {
        self.biome.cells.iter().map(|c| c.plant_biomass).sum()
    }

    /// Resize scratch buffers to match agent capacity. Called by the tick.
    pub(crate) fn resize_scratch(&mut self) {
        let cap = self.agents.capacity();
        if self.sensors.len() < cap {
            self.sensors.resize(cap, crate::sense::SensorRegister::default());
        }
        if self.desired_velocity.len() < cap {
            self.desired_velocity.resize(cap, crate::prelude::Vec2::ZERO);
        }
        if self.reproduced_this_tick.len() < cap {
            self.reproduced_this_tick.resize(cap, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::SPAWN_ENERGY;

    #[test]
    fn world_construction_is_deterministic() {
        let a = World::new(42);
        let b = World::new(42);
        assert_eq!(a.tick, b.tick);
        assert_eq!(a.seed, b.seed);
        for i in 0..a.biome.cells.len() {
            assert_eq!(a.biome.cells[i].terrain, b.biome.cells[i].terrain);
            assert!((a.biome.cells[i].plant_biomass - b.biome.cells[i].plant_biomass).abs() < 1e-6);
        }
    }

    #[test]
    fn spawn_agent_sets_initial_energy() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(10.0, 10.0), Genome::neutral());
        assert!(w.agents.is_alive(id));
        assert_eq!(w.agents.energy[id as usize], SPAWN_ENERGY);
    }
}
