//! Codex — discovery meta-game event bus and detectors.
//!
//! Detectors run at the end of each tick (after `species_step`). Each
//! detector is a pure observer over `&mut World` that writes any new
//! events into `CodexState.events`. Per-detector scratch lives on
//! `CodexState` so the hot path stays allocation-free.
//!
//! Determinism: all detector state uses `BTreeMap`/`BTreeSet` (ordered
//! iteration), never `HashMap`/`HashSet`. Events are appended in
//! deterministic detector order.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::world::World;

/// Maximum events buffered before the oldest are dropped. The headless
/// CLI's JSONL writer drains the buffer each tick, so in normal operation
/// this only matters if the consumer falls behind.
pub const CODEX_EVENT_CAPACITY: usize = 4096;

/// Recent population samples each species tracks for crash detection
/// (one sample per tick).
pub const POP_HISTORY_WINDOW: usize = 200;

/// PopulationCrash triggers when alive count drops by >= this fraction
/// across `POP_HISTORY_WINDOW` ticks.
pub const CRASH_FRACTION: f32 = 0.6;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    Extinction = 0,
    PopulationCrash = 1,
    SpeciationEvent = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexEvent {
    pub event_type: EventType,
    pub tick: u64,
    /// Species id most directly associated with the event (`u32::MAX` for
    /// global events).
    pub species_id: u32,
    /// Numeric payload (e.g. peak population for a crash, parent species
    /// id for a speciation event). Interpretation depends on type.
    pub value: f32,
}

/// Persistent state owned by `World`. Holds detector scratch and the
/// event ring buffer.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CodexState {
    /// Rolling per-species population history. Keys are `SpeciesId`s.
    pub pop_history: BTreeMap<u32, VecDeque<u32>>,
    /// Ring buffer of recent events. Oldest dropped when full.
    pub events: VecDeque<CodexEvent>,
}

impl CodexState {
    pub fn push_event(&mut self, ev: CodexEvent) {
        if self.events.len() >= CODEX_EVENT_CAPACITY {
            self.events.pop_front();
        }
        self.events.push_back(ev);
    }

    /// Drain the buffer — used by the CLI JSONL writer.
    pub fn drain_events(&mut self) -> std::collections::vec_deque::Drain<'_, CodexEvent> {
        self.events.drain(..)
    }
}

/// Run all detectors. Called by the tick orchestrator at the end of each
/// tick (after `species_step`, before `biome_step`).
///
/// Detectors run in fixed declaration order so the events buffer ordering
/// is reproducible. SpeciationEvent is emitted directly from
/// `species_step` at the moment of allocation — nothing to do here.
pub fn observe_all(world: &mut World) {
    update_pop_history(world);
    detect_extinction(world);
    detect_population_crash(world);
}

fn update_pop_history(world: &mut World) {
    let counts: Vec<u32> = world.species_member_counts.clone();
    for (sid, count) in counts.into_iter().enumerate() {
        let sid = sid as u32;
        let buf = world.codex.pop_history.entry(sid).or_default();
        if buf.len() == POP_HISTORY_WINDOW {
            buf.pop_front();
        }
        buf.push_back(count);
    }
}

fn detect_extinction(world: &mut World) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, buf) in world.codex.pop_history.iter() {
        if buf.len() < 2 {
            continue;
        }
        let prev = buf[buf.len() - 2];
        let cur = buf[buf.len() - 1];
        if prev > 0 && cur == 0 {
            to_push.push(CodexEvent {
                event_type: EventType::Extinction,
                tick,
                species_id: *sid,
                value: prev as f32,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

fn detect_population_crash(world: &mut World) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, buf) in world.codex.pop_history.iter() {
        if buf.len() < POP_HISTORY_WINDOW {
            continue;
        }
        let peak = *buf.iter().max().unwrap_or(&0);
        let cur = *buf.back().unwrap_or(&0);
        if peak == 0 || cur == 0 {
            continue;
        }
        let drop = 1.0 - (cur as f32 / peak as f32);
        if drop >= CRASH_FRACTION {
            to_push.push(CodexEvent {
                event_type: EventType::PopulationCrash,
                tick,
                species_id: *sid,
                value: drop,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_push_respects_capacity() {
        let mut s = CodexState::default();
        for i in 0..(CODEX_EVENT_CAPACITY + 100) {
            s.push_event(CodexEvent {
                event_type: EventType::Extinction,
                tick: i as u64,
                species_id: 0,
                value: 0.0,
            });
        }
        assert_eq!(s.events.len(), CODEX_EVENT_CAPACITY);
        assert_eq!(s.events.front().unwrap().tick, 100);
    }
}
