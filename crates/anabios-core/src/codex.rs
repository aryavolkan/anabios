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

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::spatial::torus_distance;
use crate::world::World;

/// Maximum events buffered before the oldest are dropped.
pub const CODEX_EVENT_CAPACITY: usize = 4096;

/// Recent population samples each species tracks for crash detection.
pub const POP_HISTORY_WINDOW: usize = 200;

/// PopulationCrash triggers when alive count drops by >= this fraction
/// across `POP_HISTORY_WINDOW` ticks.
pub const CRASH_FRACTION: f32 = 0.6;

/// Centroid samples each species tracks for migration detection.
pub const MIGRATION_WINDOW: usize = 200;

/// Migration triggers when a species centroid drifts >= this many world
/// units across `MIGRATION_WINDOW` ticks.
pub const MIGRATION_DISTANCE: f32 = 150.0;

/// Window (ticks) over which combat deaths accumulate for CombatRaid.
pub const COMBAT_RAID_WINDOW: u64 = 100;
/// Combat deaths within the window needed to declare a CombatRaid.
pub const COMBAT_RAID_THRESHOLD: usize = 3;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    Extinction = 0,
    PopulationCrash = 1,
    SpeciationEvent = 2,
    Migration = 3,
    NovelModuleAppeared = 4,
    NovelBehaviorPattern = 5,
    /// First agent death caused by another agent's weapon (vs starvation/age).
    Predation = 6,
    /// Sustained combat deaths crossing a rolling window threshold.
    CombatRaid = 7,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexEvent {
    pub event_type: EventType,
    pub tick: u64,
    /// Species id most directly associated with the event (`u32::MAX` for
    /// global events).
    pub species_id: u32,
    /// Numeric payload; interpretation depends on type (peak population,
    /// parent species id, module/node discriminant, drift fraction, …).
    pub value: f32,
    /// World location of the event (species centroid or agent position).
    /// `(0.0, 0.0)` when no natural location applies.
    pub loc_x: f32,
    pub loc_y: f32,
}

/// A death attributed to another agent's weapon. Fuel for the Predation /
/// CombatRaid detectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatDeath {
    pub tick: u64,
    pub victim_species: u32,
    pub attacker_species: u32,
    pub loc_x: f32,
    pub loc_y: f32,
}

/// Persistent state owned by `World`. Holds detector scratch and the
/// event ring buffer.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CodexState {
    /// Rolling per-species population history.
    pub pop_history: BTreeMap<u32, VecDeque<u32>>,
    /// Rolling per-species centroid history for migration detection.
    pub centroid_history: BTreeMap<u32, VecDeque<(f32, f32)>>,
    /// Per-species set of module-type discriminants already observed.
    pub seen_modules: BTreeMap<u32, BTreeSet<u8>>,
    /// Per-species set of program node-kind discriminants already observed.
    pub seen_node_kinds: BTreeMap<u32, BTreeSet<u8>>,
    /// Rolling window of combat-attributed deaths (pruned to COMBAT_RAID_WINDOW).
    pub combat_deaths: VecDeque<CombatDeath>,
    /// Latch: the first Predation event has been emitted.
    pub predation_emitted: bool,
    /// Edge-trigger state for CombatRaid (armed while below threshold).
    pub raid_active: bool,
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

    /// Drain the buffer — used by the CLI JSONL writer + Godot panel.
    pub fn drain_events(&mut self) -> std::collections::vec_deque::Drain<'_, CodexEvent> {
        self.events.drain(..)
    }

    /// Record a combat-attributed death for the Predation/CombatRaid detectors.
    pub fn record_combat_death(
        &mut self,
        tick: u64,
        victim_species: u32,
        attacker_species: u32,
        x: f32,
        y: f32,
    ) {
        self.combat_deaths.push_back(CombatDeath {
            tick,
            victim_species,
            attacker_species,
            loc_x: x,
            loc_y: y,
        });
    }
}

/// Run all detectors. Called by the tick orchestrator at the end of each
/// tick. SpeciationEvent is emitted directly from `species_step`.
pub fn observe_all(world: &mut World) {
    // Compute per-species centroids once (immutable agent borrow) so the
    // mutable-codex detectors can reuse them for event locations.
    let centroids = compute_centroids(world);

    update_pop_history(world);
    detect_extinction(world, &centroids);
    detect_population_crash(world, &centroids);
    detect_migration(world, &centroids);
    detect_novel_modules(world, &centroids);
    detect_novel_behavior(world, &centroids);
    // Predation runs before CombatRaid on purpose: it scans this tick's
    // combat-death entries, and CombatRaid then prunes entries older than the
    // window (which never includes the current tick).
    detect_predation(world);
    detect_combat_raid(world);
}

/// Mean alive position per species, ascending id order, f64 accumulator.
fn compute_centroids(world: &World) -> BTreeMap<u32, (f32, f32)> {
    let mut sums: BTreeMap<u32, (f64, f64, u32)> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        let p = world.agents.position[i];
        let e = sums.entry(sid).or_insert((0.0, 0.0, 0));
        e.0 += p.x as f64;
        e.1 += p.y as f64;
        e.2 += 1;
    }
    sums.into_iter()
        .map(|(sid, (sx, sy, n))| {
            let nf = n.max(1) as f64;
            (sid, ((sx / nf) as f32, (sy / nf) as f32))
        })
        .collect()
}

fn centroid_of(centroids: &BTreeMap<u32, (f32, f32)>, sid: u32) -> (f32, f32) {
    centroids.get(&sid).copied().unwrap_or((0.0, 0.0))
}

fn update_pop_history(world: &mut World) {
    let counts: Vec<u32> = world.species_member_counts.clone();
    for (sid, count) in counts.into_iter().enumerate() {
        let buf = world.codex.pop_history.entry(sid as u32).or_default();
        if buf.len() == POP_HISTORY_WINDOW {
            buf.pop_front();
        }
        buf.push_back(count);
    }
}

fn detect_extinction(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, buf) in world.codex.pop_history.iter() {
        if buf.len() < 2 {
            continue;
        }
        let prev = buf[buf.len() - 2];
        let cur = buf[buf.len() - 1];
        if prev > 0 && cur == 0 {
            let (lx, ly) = centroid_of(centroids, *sid);
            to_push.push(CodexEvent {
                event_type: EventType::Extinction,
                tick,
                species_id: *sid,
                value: prev as f32,
                loc_x: lx,
                loc_y: ly,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

fn detect_population_crash(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
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
            let (lx, ly) = centroid_of(centroids, *sid);
            to_push.push(CodexEvent {
                event_type: EventType::PopulationCrash,
                tick,
                species_id: *sid,
                value: drop,
                loc_x: lx,
                loc_y: ly,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

fn detect_migration(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Push current centroids into history.
    for (sid, c) in centroids.iter() {
        let buf = world.codex.centroid_history.entry(*sid).or_default();
        if buf.len() == MIGRATION_WINDOW {
            buf.pop_front();
        }
        buf.push_back(*c);
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    let mut clear: Vec<u32> = Vec::new();
    for (sid, buf) in world.codex.centroid_history.iter() {
        if buf.len() < MIGRATION_WINDOW {
            continue;
        }
        let first = *buf.front().unwrap();
        let last = *buf.back().unwrap();
        let d = torus_distance(glam::Vec2::new(first.0, first.1), glam::Vec2::new(last.0, last.1));
        if d >= MIGRATION_DISTANCE {
            to_push.push(CodexEvent {
                event_type: EventType::Migration,
                tick,
                species_id: *sid,
                value: d,
                loc_x: last.0,
                loc_y: last.1,
            });
            clear.push(*sid);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
    // Reset history for migrated species so they don't re-fire each tick.
    for sid in clear {
        world.codex.centroid_history.remove(&sid);
    }
}

fn detect_novel_modules(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Current module types present per species this tick.
    let mut current: BTreeMap<u32, BTreeSet<u8>> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        let set = current.entry(sid).or_default();
        for m in world.agents.modules[i].iter() {
            set.insert(m.module_type() as u8);
        }
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, types) in current {
        match world.codex.seen_modules.get_mut(&sid) {
            None => {
                // Debut: seed silently.
                world.codex.seen_modules.insert(sid, types);
            }
            Some(seen) => {
                for t in types {
                    if seen.insert(t) {
                        let (lx, ly) = centroid_of(centroids, sid);
                        to_push.push(CodexEvent {
                            event_type: EventType::NovelModuleAppeared,
                            tick,
                            species_id: sid,
                            value: t as f32,
                            loc_x: lx,
                            loc_y: ly,
                        });
                    }
                }
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

fn detect_novel_behavior(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    let mut current: BTreeMap<u32, BTreeSet<u8>> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        let set = current.entry(sid).or_default();
        for node in world.agents.program[i].nodes.iter().copied() {
            set.insert(crate::program::Program::node_kind(node));
        }
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, kinds) in current {
        match world.codex.seen_node_kinds.get_mut(&sid) {
            None => {
                world.codex.seen_node_kinds.insert(sid, kinds);
            }
            Some(seen) => {
                for k in kinds {
                    if seen.insert(k) {
                        let (lx, ly) = centroid_of(centroids, sid);
                        to_push.push(CodexEvent {
                            event_type: EventType::NovelBehaviorPattern,
                            tick,
                            species_id: sid,
                            value: k as f32,
                            loc_x: lx,
                            loc_y: ly,
                        });
                    }
                }
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Predation: emit once, the first tick a combat-attributed death is recorded.
/// Payload species = the attacker (predator) species.
fn detect_predation(world: &mut World) {
    if world.codex.predation_emitted {
        return;
    }
    let tick = world.tick;
    if let Some(cd) = world.codex.combat_deaths.iter().find(|d| d.tick == tick) {
        let ev = CodexEvent {
            event_type: EventType::Predation,
            tick,
            species_id: cd.attacker_species,
            value: 1.0,
            loc_x: cd.loc_x,
            loc_y: cd.loc_y,
        };
        world.codex.push_event(ev);
        world.codex.predation_emitted = true;
    }
}

/// CombatRaid: prune the combat-death window, then edge-trigger when the count
/// reaches COMBAT_RAID_THRESHOLD. Re-arms when it drops back below threshold.
fn detect_combat_raid(world: &mut World) {
    let tick = world.tick;
    let cutoff = tick.saturating_sub(COMBAT_RAID_WINDOW);
    while let Some(front) = world.codex.combat_deaths.front() {
        if front.tick < cutoff {
            world.codex.combat_deaths.pop_front();
        } else {
            break;
        }
    }
    let count = world.codex.combat_deaths.len();
    let raiding = count >= COMBAT_RAID_THRESHOLD;
    if raiding && !world.codex.raid_active {
        let last = world.codex.combat_deaths.back().expect("non-empty when raiding");
        let ev = CodexEvent {
            event_type: EventType::CombatRaid,
            tick,
            species_id: last.attacker_species,
            value: count as f32,
            loc_x: last.loc_x,
            loc_y: last.loc_y,
        };
        world.codex.push_event(ev);
        world.codex.raid_active = true;
    } else if !raiding {
        world.codex.raid_active = false;
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
                loc_x: 0.0,
                loc_y: 0.0,
            });
        }
        assert_eq!(s.events.len(), CODEX_EVENT_CAPACITY);
        assert_eq!(s.events.front().unwrap().tick, 100);
    }
}
