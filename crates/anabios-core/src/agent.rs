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
    /// Channels `invention::INVENTION_CHANNEL_BASE..` carry the invention
    /// tree's adoption levels.
    pub meme_vector: Vec<[f32; crate::program::MEME_CHANNELS]>,
    /// Per-agent trade-good holdings, indexed by `crate::resource::Good::index`.
    /// Zeroed on spawn; only the resource subsystem (harvest/trade/material
    /// learning costs) mutates it, and only when `World::resources_enabled` is on.
    pub inventory: Vec<[f32; crate::resource::GOOD_COUNT]>,
    /// Realized IQ in `[0,1]` — a gene×environment phenotype developed over the
    /// juvenile window by `iq::develop_all` (heritable `CognitivePotential`
    /// modulated by juvenile nutrition + social enrichment) and frozen at
    /// maturity. Stays `0.0` for every agent when `World::cognition_enabled` is
    /// false, so it is cost/gate-neutral there. Serialized (persistent state).
    pub iq: Vec<f32>,
    /// Running sum of per-tick juvenile enrichment samples (nutrition + social),
    /// and the sample count — together the developmental average feeding `iq`.
    pub iq_enrich_acc: Vec<f32>,
    pub iq_enrich_ticks: Vec<u32>,
    /// Per-agent subcortical affect activations (7 Panksepp systems, each in
    /// `[0,1]`), developed each tick by `affect::develop_all` and read by the
    /// affect bias hooks. Neutral default `[0.0; AFFECT_SYSTEMS]`; stays all-zero
    /// when `World::affect_enabled` is false, so it is cost/identity-neutral
    /// there. Serialized (persistent) — a path-dependent accumulator feeding
    /// hashed movement, so it must NOT be `#[serde(skip)]` (still-ticks v13 footgun).
    pub affect: Vec<crate::affect::AffectState>,
    /// Learned home point (E8): an EMA of position updated when
    /// `settlement_enabled`; inherited by offspring with drift. Spawned at
    /// the agent's spawn position; inert when the flag is off.
    pub anchor: Vec<crate::prelude::Vec2>,
    /// Per-good harvest experience (E8): each harvest of good k increments
    /// `harvest_exp[k]`; the harvest amount scales with it. Only mutated
    /// when `World::resources_enabled` is on.
    pub harvest_exp: Vec<[f32; crate::resource::GOOD_COUNT]>,
    /// Meme-variant lineage (E9): the variant id carried per meme channel
    /// (0 = untracked). Assigned at birth (communicator children) and on
    /// band transitions.
    pub meme_lineage: Vec<[u32; crate::program::MEME_CHANNELS]>,
    /// Biological sex (E12): `false` = female, `true` = male. Read only when
    /// `World::sexual_dimorphism_enabled`; all-female and unread otherwise.
    pub sex: BitVec,
    /// Livestock ownership (E13): the owning herder's agent id, or
    /// `AGENT_NULL` when wild. Set by `domestication::husbandry_step` (taming)
    /// and at birth (born-domesticated); cleared eagerly in `kill` when the
    /// owner dies (so a recycled owner slot can't silently inherit the herd),
    /// with the husbandry orphan sweep as a backstop. Read only when
    /// `World::domestication_enabled`.
    pub livestock_of: Vec<AgentId>,
    pub alive: BitVec,
    free_list: Vec<AgentId>,
    live_count: u32,
    /// Reusable scratch buffer for per-tick "snapshot the alive ids" loops.
    /// `#[serde(skip)]` — never part of the deterministic state hash.
    #[serde(skip)]
    pub scratch_ids: Vec<u32>,
    /// Snapshots (position, trade-goods inventory) of agents killed this tick,
    /// drained by `resource::conserve_goods_step` when `conserve_goods_on_death`
    /// is on. `#[serde(skip)]` — scratch, never part of the state hash.
    #[serde(skip)]
    pub deaths_scratch: Vec<(crate::prelude::Vec2, [f32; crate::resource::GOOD_COUNT])>,
    /// Whether livestock ownership is active (mirrors `World::domestication_enabled`).
    /// Gates the O(n) owner-referrer clear in `kill`, so worlds with the feature
    /// off pay nothing. `#[serde(skip)]` and re-derived from the (serialized)
    /// `domestication_enabled` flag at scenario instantiation and snapshot load,
    /// so it is not an independent piece of state that could drift across a
    /// save→load round-trip.
    #[serde(skip)]
    pub track_livestock: bool,
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
        sex: bool,
    ) -> AgentId {
        // Reuse a dead slot, or grow every column by one default row. Either
        // way we then write every field by index below — one write path, so the
        // free-list and grow cases can never silently diverge on a value.
        let id = match self.free_list.pop() {
            Some(id) => id,
            None => self.grow_one(),
        };
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
        self.inventory[i] = [0.0; crate::resource::GOOD_COUNT];
        self.iq[i] = 0.0;
        self.iq_enrich_acc[i] = 0.0;
        self.iq_enrich_ticks[i] = 0;
        self.affect[i] = [0.0; crate::affect::AFFECT_SYSTEMS];
        self.anchor[i] = position;
        self.harvest_exp[i] = [0.0; crate::resource::GOOD_COUNT];
        self.meme_lineage[i] = [0; crate::program::MEME_CHANNELS];
        self.sex.set(i, sex);
        self.livestock_of[i] = AGENT_NULL;
        self.alive.set(i, true);
        self.live_count += 1;
        id
    }

    /// Grow every per-agent column by one placeholder row and return the new
    /// slot's id. The sole caller (`spawn`) overwrites every field immediately,
    /// so the pushed values are throwaway defaults — this exists only so the
    /// real spawn values have a single write path. A column omitted here desyncs
    /// the buffer lengths and panics loudly on the next indexed write in
    /// `spawn`, rather than silently corrupting a slot.
    fn grow_one(&mut self) -> AgentId {
        let id = self.position.len() as AgentId;
        self.position.push(Vec2::ZERO);
        self.velocity.push(Vec2::ZERO);
        self.energy.push(0.0);
        self.age.push(0);
        self.genome.push(Genome::neutral());
        self.lineage_id.push(LINEAGE_NONE);
        self.parent_ids.push([LINEAGE_NONE; 2]);
        self.species_id.push(0);
        self.modules.push(ModuleList::default());
        self.program.push(Program::empty());
        self.meme_vector.push([0.0; crate::program::MEME_CHANNELS]);
        self.inventory.push([0.0; crate::resource::GOOD_COUNT]);
        self.iq.push(0.0);
        self.iq_enrich_acc.push(0.0);
        self.iq_enrich_ticks.push(0);
        self.affect.push([0.0; crate::affect::AFFECT_SYSTEMS]);
        self.anchor.push(Vec2::ZERO);
        self.harvest_exp.push([0.0; crate::resource::GOOD_COUNT]);
        self.meme_lineage.push([0; crate::program::MEME_CHANNELS]);
        self.sex.push(false);
        self.livestock_of.push(AGENT_NULL);
        self.alive.push(false);
        id
    }

    /// Kill the agent. Energy is zeroed and the slot is added to the free list.
    ///
    /// When livestock ownership is active, any animal that named this agent as
    /// its owner is released to the wild here. This must happen at the moment of
    /// death: the freed slot can be recycled by `spawn` (LIFO free list) before
    /// the husbandry orphan sweep next runs, and a recycled slot reads as alive
    /// again — so a purely "is the owner still alive?" sweep would silently
    /// rebind the herd to whatever unrelated agent inherited the slot. Clearing
    /// eagerly is order-independent and closes that window. Gated on
    /// `track_livestock` so the O(n) scan never runs when the feature is off.
    pub fn kill(&mut self, id: AgentId) {
        let i = id as usize;
        if i >= self.alive.len() || !self.alive[i] {
            return;
        }
        self.alive.set(i, false);
        self.energy[i] = 0.0;
        self.free_list.push(id);
        self.live_count -= 1;

        // Snapshot goods held at death (cheap; naturally a no-op when
        // `resources_enabled` is off, since inventory is all-zero then).
        // Only consumed by `resource::conserve_goods_step` when
        // `conserve_goods_on_death` is on; otherwise cleared unread. Do not
        // zero `self.inventory[i]` here — nothing reads a dead slot's
        // inventory before reuse zeroes it, so leaving it is a no-op for
        // every other reader and avoids any behavior change when the flag
        // is off.
        let inv = self.inventory[i];
        if inv.iter().any(|&g| g > 0.0) {
            self.deaths_scratch.push((self.position[i], inv));
        }

        if self.track_livestock {
            for owner in self.livestock_of.iter_mut() {
                if *owner == id {
                    *owner = AGENT_NULL;
                }
            }
        }
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
            false,
        );
        let id1 = a.spawn(
            Vec2::new(3.0, 4.0),
            neutral(),
            2,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
            false,
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
            false,
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
            false,
        );
        let id1 = a.spawn(
            Vec2::ZERO,
            neutral(),
            2,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
            false,
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
            false,
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
            false,
        );
        let _id1 = a.spawn(
            Vec2::ZERO,
            neutral(),
            2,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
            false,
        );
        let id2 = a.spawn(
            Vec2::ZERO,
            neutral(),
            3,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
            false,
        );
        a.kill(id0);
        let alive: Vec<AgentId> = a.iter_alive().collect();
        assert_eq!(alive, vec![1, id2]);
    }

    #[test]
    fn spawn_zeroes_inventory() {
        let mut a = AgentBuffers::new();
        let id = a.spawn(
            Vec2::ZERO,
            neutral(),
            1,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
            false,
        );
        assert_eq!(a.inventory[id as usize], [0.0; crate::resource::GOOD_COUNT]);
        assert_eq!(a.inventory.len(), a.capacity());
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
            false,
        );
        a.kill(id);
        a.kill(id);
        assert_eq!(a.live_count(), 0);
        assert_eq!(a.iter_alive().count(), 0);
    }

    #[test]
    fn spawn_zeroes_affect() {
        let mut a = AgentBuffers::new();
        let id = a.spawn(
            Vec2::ZERO,
            neutral(),
            1,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
            false,
        );
        assert_eq!(a.affect[id as usize], [0.0; crate::affect::AFFECT_SYSTEMS]);
        assert_eq!(a.affect.len(), a.capacity(), "affect grows in lockstep with capacity");
    }

    #[test]
    fn reused_slot_resets_affect() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(
            Vec2::ZERO,
            neutral(),
            1,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
            false,
        );
        a.affect[id0 as usize][crate::affect::SEEK] = 0.9;
        a.kill(id0);
        let id1 = a.spawn(
            Vec2::ZERO,
            neutral(),
            2,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
            false,
        );
        assert_eq!(id1, id0, "LIFO free list reuses slot 0");
        assert_eq!(
            a.affect[id1 as usize],
            [0.0; crate::affect::AFFECT_SYSTEMS],
            "reused (dead) slot resets affect to neutral"
        );
    }
}
