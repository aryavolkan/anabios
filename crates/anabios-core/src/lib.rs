//! anabios-core — deterministic agent-based ecology simulation.
//!
//! This crate has no Godot, no file I/O, no wall-clock reads. Pure functions
//! over state buffers. Given the same seed and scenario, every run is
//! bit-identical.

pub mod age;
pub mod agent;
pub mod behavior;
pub mod biome;
pub mod carcass;
pub mod codex;
pub mod culture;
pub mod genome;
pub mod integrate;
pub mod interact;
pub mod invention;
pub mod iq;
pub mod kin;
pub mod mathf;
pub mod module;
pub mod personality;
pub mod pheromone;
pub mod program;
pub mod reproduce;
pub mod rng;
pub mod scenario;
pub mod sense;
pub mod snapshot;
pub mod spatial;
pub mod species;
pub mod tick;
pub mod world;

#[allow(dead_code)]
mod prelude;

#[doc(hidden)]
pub mod prelude_test {
    pub use glam::Vec2;

    /// Create a fresh species (bookkeeping tables consistent) and return its
    /// id. Test/bench-only way to make a second species before the first tick.
    pub fn fresh_species(w: &mut crate::world::World) -> u32 {
        let sid = w.species_centroids.len() as u32;
        // Grow all three parallel species tables explicitly so the helper is
        // self-contained (not relying on add_to_species's internal resize).
        w.species_centroids.push(crate::genome::Genome::neutral());
        w.species_parents.push(Some(0));
        w.species_member_counts.push(0);
        w.next_species_id = sid + 1;
        sid
    }

    /// Move an already-spawned agent into `sid`, keeping member counts
    /// consistent.
    pub fn reassign_to_species(w: &mut crate::world::World, agent: u32, sid: u32) {
        w.remove_from_species(w.agents.species_id[agent as usize]);
        w.agents.species_id[agent as usize] = sid;
        w.add_to_species(sid);
    }

    /// Move an already-spawned agent into a fresh species; returns the new
    /// species id.
    pub fn reassign_to_new_species(w: &mut crate::world::World, agent: u32) -> u32 {
        let sid = fresh_species(w);
        reassign_to_species(w, agent, sid);
        sid
    }
}

pub use agent::AgentId;
pub use agent::{LineageId, SpeciesId, LINEAGE_NONE};
pub use genome::{Genome, GenomeSlot};
pub use module::{Module, ModuleType};
pub use program::Program;
pub use scenario::Scenario;
pub use world::World;
