//! anabios-core — deterministic agent-based ecology simulation.
//!
//! This crate has no Godot, no file I/O, no wall-clock reads. Pure functions
//! over state buffers. Given the same seed and scenario, every run is
//! bit-identical.

pub mod agent;
pub mod behavior;
pub mod biome;
pub mod genome;
pub mod integrate;
pub mod rng;
pub mod scenario;
pub mod sense;
pub mod snapshot;
pub mod spatial;
pub mod tick;
pub mod world;

#[allow(dead_code)]
mod prelude;

pub use agent::AgentId;
pub use genome::{Genome, GenomeSlot};
pub use world::World;
// TODO(task-17): pub use scenario::Scenario;
