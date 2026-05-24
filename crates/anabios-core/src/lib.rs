//! anabios-core — deterministic agent-based ecology simulation.
//!
//! This crate has no Godot, no file I/O, no wall-clock reads. Pure functions
//! over state buffers. Given the same seed and scenario, every run is
//! bit-identical.

pub mod age;
pub mod agent;
pub mod behavior;
pub mod biome;
pub mod codex;
pub mod genome;
pub mod integrate;
pub mod interact;
pub mod mathf;
pub mod module;
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
}

pub use agent::AgentId;
pub use agent::{LineageId, SpeciesId, LINEAGE_NONE};
pub use genome::{Genome, GenomeSlot};
pub use module::{Module, ModuleType};
pub use program::Program;
pub use scenario::Scenario;
pub use world::World;
