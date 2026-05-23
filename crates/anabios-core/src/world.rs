//! `World` is the root state object owned by every simulation. It carries
//! the RNG, biome field, agent buffers, spatial hash, and tick counter.
//! Nothing outside this struct holds simulation state.

use serde::{Deserialize, Serialize};

use crate::agent::{AgentBuffers, AgentId};
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
    #[serde(skip)]
    pub spatial: UniformSpatialHash,
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
            spatial: UniformSpatialHash::new(),
        }
    }

    /// Convenience: spawn an agent with starting energy at the given position.
    pub fn spawn_agent(&mut self, position: Vec2, genome: Genome) -> AgentId {
        self.agents.spawn(position, genome)
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
