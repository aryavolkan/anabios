//! Struct-of-Arrays agent buffers.
//!
//! Each per-agent field is its own `Vec<T>` indexed by `AgentId`. Dead agent
//! slots stay allocated for index stability; `alive` is a bitvec used to mask
//! reads. Newly spawned agents reuse dead slots via a free list, so live
//! agent counts stay dense.

use bitvec::vec::BitVec;
use serde::{Deserialize, Serialize};

use crate::genome::Genome;
use crate::module::ModuleList;
use crate::prelude::Vec2;
use crate::program::Program;

/// Stable agent identifier. `u32::MAX` is reserved as a null sentinel.
pub type AgentId = u32;
pub const AGENT_NULL: AgentId = u32::MAX;

/// Maximum starting energy for newly-spawned agents.
pub const SPAWN_ENERGY: f32 = 50.0;

/// Unique lineage identifier. Each agent gets a fresh value at birth; never
/// reused even after death. Used for ancestry, kin recognition, and codex
/// lineage-hall entries.
pub type LineageId = u64;
/// Stable species identifier. Initially every agent is species 0; speciation
/// (M2) assigns new species ids over time.
pub type SpeciesId = u32;

/// Lineage id used for ancestors of seeded (founder) agents that have no
/// modelled parent. Stored in `parent_ids` slots to mean "no parent".
pub const LINEAGE_NONE: LineageId = 0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentBuffers {
    pub position: Vec<Vec2>,
    /// Most-recently-applied velocity. Recorded by the integrate stage.
    /// Not read by any sensor today (reserved for correlated-wander
    /// behavior); currently consumed only by the Godot binding for agent
    /// rotation. Included in the persistent snapshot to keep golden hashes
    /// stable across that change.
    pub velocity: Vec<Vec2>,
    pub energy: Vec<f32>,
    pub age: Vec<u32>,
    pub genome: Vec<Genome>,
    pub lineage_id: Vec<LineageId>,
    pub parent_ids: Vec<[LineageId; 2]>,
    pub species_id: Vec<SpeciesId>,
    pub modules: Vec<ModuleList>,
    pub program: Vec<Program>,
    /// Per-agent cultural state; transmitted by `culture_step`, read by
    /// `SenseMeme`. Zeroed on spawn; only Communicator agents change it.
    pub meme_vector: Vec<[f32; crate::program::MEME_CHANNELS]>,
    pub alive: BitVec,
    free_list: Vec<AgentId>,
    live_count: u32,
    /// Reusable scratch buffer for per-tick "snapshot the alive ids" loops.
    /// `#[serde(skip)]` — never part of the deterministic state hash.
    #[serde(skip)]
    pub scratch_ids: Vec<u32>,
}

impl AgentBuffers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of currently alive agents.
    #[inline]
    pub fn live_count(&self) -> u32 {
        self.live_count
    }

    /// Total slot capacity (alive + dead). Use only for sizing scratch
    /// buffers — iterate via `iter_alive()` instead of raw indices.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.position.len()
    }

    /// `true` iff the slot is currently alive.
    #[inline]
    pub fn is_alive(&self, id: AgentId) -> bool {
        let i = id as usize;
        i < self.alive.len() && self.alive[i]
    }

    /// Spawn an agent. Reuses a dead slot if available; otherwise extends
    /// every buffer by one. `lineage_id` must be globally unique across the
    /// world's lifetime (allocate via `World::next_lineage()`). `parent_ids`
    /// = `[LINEAGE_NONE; 2]` for founders; otherwise the lineage ids of the
    /// two parents.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &mut self,
        position: Vec2,
        genome: Genome,
        lineage_id: LineageId,
        parent_ids: [LineageId; 2],
        species_id: SpeciesId,
        modules: ModuleList,
        program: Program,
    ) -> AgentId {
        let id = if let Some(id) = self.free_list.pop() {
            let i = id as usize;
            self.position[i] = position;
            self.velocity[i] = Vec2::ZERO;
            self.energy[i] = SPAWN_ENERGY;
            self.age[i] = 0;
            self.genome[i] = genome;
            self.lineage_id[i] = lineage_id;
            self.parent_ids[i] = parent_ids;
            self.species_id[i] = species_id;
            self.modules[i] = modules;
            self.program[i] = program;
            self.meme_vector[i] = [0.0; crate::program::MEME_CHANNELS];
            self.alive.set(i, true);
            id
        } else {
            let i = self.position.len();
            self.position.push(position);
            self.velocity.push(Vec2::ZERO);
            self.energy.push(SPAWN_ENERGY);
            self.age.push(0);
            self.genome.push(genome);
            self.lineage_id.push(lineage_id);
            self.parent_ids.push(parent_ids);
            self.species_id.push(species_id);
            self.modules.push(modules);
            self.program.push(program);
            self.meme_vector.push([0.0; crate::program::MEME_CHANNELS]);
            self.alive.push(true);
            i as AgentId
        };
        self.live_count += 1;
        id
    }

    /// Kill the agent. Energy is zeroed and the slot is added to the free list.
    pub fn kill(&mut self, id: AgentId) {
        let i = id as usize;
        if i >= self.alive.len() || !self.alive[i] {
            return;
        }
        self.alive.set(i, false);
        self.energy[i] = 0.0;
        self.free_list.push(id);
        self.live_count -= 1;
    }

    /// Iterate live agent ids. Order is by raw index (ascending), which is
    /// deterministic.
    pub fn iter_alive(&self) -> impl Iterator<Item = AgentId> + '_ {
        self.alive.iter_ones().map(|i| i as AgentId)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neutral() -> Genome {
        Genome::neutral()
    }

    #[test]
    fn spawn_increases_capacity_and_live_count() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(
            Vec2::new(1.0, 2.0),
            neutral(),
            1,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
        );
        let id1 = a.spawn(
            Vec2::new(3.0, 4.0),
            neutral(),
            2,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
        );
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(a.capacity(), 2);
        assert_eq!(a.live_count(), 2);
        assert!(a.is_alive(0));
        assert!(a.is_alive(1));
    }

    #[test]
    fn kill_marks_slot_dead_and_decrements_live_count() {
        let mut a = AgentBuffers::new();
        let id = a.spawn(
            Vec2::ZERO,
            neutral(),
            1,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
        );
        a.kill(id);
        assert!(!a.is_alive(id));
        assert_eq!(a.live_count(), 0);
    }

    #[test]
    fn spawn_after_kill_reuses_slot() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(
            Vec2::ZERO,
            neutral(),
            1,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
        );
        let id1 = a.spawn(
            Vec2::ZERO,
            neutral(),
            2,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
        );
        a.kill(id0);
        let id2 = a.spawn(
            Vec2::new(5.0, 6.0),
            neutral(),
            3,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
        );
        assert_eq!(id2, id0, "slot 0 should have been reused");
        assert_eq!(a.live_count(), 2);
        assert!(a.is_alive(id1));
        assert!(a.is_alive(id2));
    }

    #[test]
    fn iter_alive_skips_dead_slots() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(
            Vec2::ZERO,
            neutral(),
            1,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
        );
        let _id1 = a.spawn(
            Vec2::ZERO,
            neutral(),
            2,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
        );
        let id2 = a.spawn(
            Vec2::ZERO,
            neutral(),
            3,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
        );
        a.kill(id0);
        let alive: Vec<AgentId> = a.iter_alive().collect();
        assert_eq!(alive, vec![1, id2]);
    }

    #[test]
    fn double_kill_is_a_noop() {
        let mut a = AgentBuffers::new();
        let id = a.spawn(
            Vec2::ZERO,
            neutral(),
            1,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
        );
        a.kill(id);
        a.kill(id);
        assert_eq!(a.live_count(), 0);
        assert_eq!(a.iter_alive().count(), 0);
    }
}
