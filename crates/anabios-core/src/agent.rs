//! Struct-of-Arrays agent buffers.
//!
//! Each per-agent field is its own `Vec<T>` indexed by `AgentId`. Dead agent
//! slots stay allocated for index stability; `alive` is a bitvec used to mask
//! reads. Newly spawned agents reuse dead slots via a free list, so live
//! agent counts stay dense.

use bitvec::vec::BitVec;
use serde::{Deserialize, Serialize};

use crate::genome::Genome;
use crate::prelude::Vec2;

/// Stable agent identifier. `u32::MAX` is reserved as a null sentinel.
pub type AgentId = u32;
pub const AGENT_NULL: AgentId = u32::MAX;

/// Maximum starting energy for newly-spawned agents.
pub const SPAWN_ENERGY: f32 = 50.0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentBuffers {
    pub position: Vec<Vec2>,
    /// Most-recently-applied velocity. Recorded by the integrate stage but
    /// not yet read by any sensor. Reserved for M3 correlated-wander
    /// behavior, which will read this as `last_velocity` to bias new
    /// directions toward recent motion. Included in the persistent
    /// snapshot to keep golden hashes stable across that change.
    pub velocity: Vec<Vec2>,
    pub energy: Vec<f32>,
    pub age: Vec<u32>,
    pub genome: Vec<Genome>,
    pub alive: BitVec,
    free_list: Vec<AgentId>,
    live_count: u32,
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

    /// Spawn an agent at the given position with the given genome. Reuses a
    /// dead slot if available, otherwise extends every buffer by one.
    pub fn spawn(&mut self, position: Vec2, genome: Genome) -> AgentId {
        let id = if let Some(id) = self.free_list.pop() {
            let i = id as usize;
            self.position[i] = position;
            self.velocity[i] = Vec2::ZERO;
            self.energy[i] = SPAWN_ENERGY;
            self.age[i] = 0;
            self.genome[i] = genome;
            self.alive.set(i, true);
            id
        } else {
            let i = self.position.len();
            self.position.push(position);
            self.velocity.push(Vec2::ZERO);
            self.energy.push(SPAWN_ENERGY);
            self.age.push(0);
            self.genome.push(genome);
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
        let id0 = a.spawn(Vec2::new(1.0, 2.0), neutral());
        let id1 = a.spawn(Vec2::new(3.0, 4.0), neutral());
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
        let id = a.spawn(Vec2::ZERO, neutral());
        a.kill(id);
        assert!(!a.is_alive(id));
        assert_eq!(a.live_count(), 0);
    }

    #[test]
    fn spawn_after_kill_reuses_slot() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(Vec2::ZERO, neutral());
        let id1 = a.spawn(Vec2::ZERO, neutral());
        a.kill(id0);
        let id2 = a.spawn(Vec2::new(5.0, 6.0), neutral());
        assert_eq!(id2, id0, "slot 0 should have been reused");
        assert_eq!(a.live_count(), 2);
        assert!(a.is_alive(id1));
        assert!(a.is_alive(id2));
    }

    #[test]
    fn iter_alive_skips_dead_slots() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(Vec2::ZERO, neutral());
        let _id1 = a.spawn(Vec2::ZERO, neutral());
        let id2 = a.spawn(Vec2::ZERO, neutral());
        a.kill(id0);
        let alive: Vec<AgentId> = a.iter_alive().collect();
        assert_eq!(alive, vec![1, id2]);
    }

    #[test]
    fn double_kill_is_a_noop() {
        let mut a = AgentBuffers::new();
        let id = a.spawn(Vec2::ZERO, neutral());
        a.kill(id);
        a.kill(id);
        assert_eq!(a.live_count(), 0);
        assert_eq!(a.iter_alive().count(), 0);
    }
}
