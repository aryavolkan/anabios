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

use crate::culture::{ALARM_MEME_CHANNEL, MEME_BROADCAST_THRESHOLD};
use crate::program::MEME_CHANNELS;
use crate::spatial::{torus_distance, PERCEPTION_MAX_RADIUS};
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

/// Samples retained per species for the weapon/armor trend windows.
pub const ARMS_WINDOW: usize = 20;
/// Minimum rise (window back − front) in a trait mean to count as "trending up".
pub const ARMS_MIN_DELTA: f32 = 0.5;

/// Ticks a species must stay clustered to count as a formed territory.
pub const TERRITORY_WINDOW: usize = 60;
/// Max RMS spread (world units) for a species to count as "clustered".
pub const TERRITORY_SPREAD_MAX: f32 = 120.0;
/// Min members before territory clustering is meaningful.
pub const TERRITORY_MIN_MEMBERS: u32 = 5;

/// Ticks two species must stay below the overlap threshold to partition.
pub const NICHE_WINDOW: u32 = 60;
/// Max terrain-distribution overlap for two species to count as partitioned.
pub const NICHE_OVERLAP_MAX: f32 = 0.35;
/// Min members per species for niche comparison to be meaningful.
pub const NICHE_MIN_MEMBERS: u32 = 5;

/// Ticks of consecutive divergence required before DialectFormed fires.
pub const DIALECT_WINDOW: usize = 50;
/// Minimum L2 distance between west/east meme half-means to count as divergent.
pub const DIALECT_DIVERGENCE_MIN: f32 = 0.4;
/// Minimum agents required in each spatial half for dialect detection.
pub const DIALECT_MIN_HALF: u32 = 3;
/// Cumulative alarm→flee co-occurrences needed to fire AlarmCall.
pub const ALARM_MIN_RESPONSES: u32 = 15;

/// Ticks of history tracked for the per-(species,channel) meme mean sweep.
pub const MEME_SWEEP_WINDOW: usize = 80;
/// Meme mean must start at or below this for a MemeSweep.
pub const MEME_SWEEP_LOW: f32 = 0.2;
/// Meme mean must reach at or above this for a MemeSweep.
pub const MEME_SWEEP_HIGH: f32 = 0.6;
/// Minimum members a species needs for MemeSweep to be meaningful.
pub const MEME_SWEEP_MIN_MEMBERS: u32 = 5;

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
    /// One species' mean weapon damage and another's mean armor both trend up.
    ArmsRace = 8,
    /// A pheromone-marking species maintains a tight, persistent spatial cluster.
    TerritoryFormation = 9,
    /// Two species occupy divergent terrain-type distributions (low overlap).
    NichePartitioning = 10,
    /// Two spatial halves of a communicating species hold divergent memes.
    DialectFormed = 11,
    /// A meme value rises from rare to dominant across a species.
    MemeSweep = 12,
    /// Alarm broadcasts reliably co-occur with nearby same-species fleeing.
    AlarmCall = 13,
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
    /// Rolling per-species mean weapon damage (window ARMS_WINDOW).
    pub weapon_history: BTreeMap<u32, VecDeque<f32>>,
    /// Rolling per-species mean armor protection (window ARMS_WINDOW).
    pub armor_history: BTreeMap<u32, VecDeque<f32>>,
    /// Edge-trigger state for ArmsRace.
    pub arms_race_active: bool,
    /// Rolling per-species RMS spatial spread (for TerritoryFormation).
    pub territory_spread: BTreeMap<u32, VecDeque<f32>>,
    /// Species currently latched as having a formed territory.
    pub territory_active: BTreeSet<u32>,
    /// Per species-pair consecutive-tick streak below the overlap threshold.
    pub niche_streak: BTreeMap<(u32, u32), u32>,
    /// Species pairs currently latched as niche-partitioned.
    pub niche_active: BTreeSet<(u32, u32)>,
    /// Rolling per-species east/west meme-divergence (for DialectFormed).
    pub dialect_divergence: BTreeMap<u32, VecDeque<f32>>,
    /// Species currently latched as having a formed dialect.
    pub dialect_active: BTreeSet<u32>,
    /// Rolling per (species, channel) mean meme value (for MemeSweep).
    pub meme_mean_history: BTreeMap<(u32, u8), VecDeque<f32>>,
    /// (species, channel) pairs currently latched as swept.
    pub meme_sweep_active: BTreeSet<(u32, u8)>,
    /// Cumulative alarm→flee co-occurrences (for AlarmCall).
    pub alarm_responses: u32,
    /// Latch: the AlarmCall event has been emitted.
    pub alarm_emitted: bool,
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

/// RMS distance (torus-aware) of `positions` from their coordinate mean.
/// Returns 0.0 for fewer than 2 points.
pub fn species_spread(positions: &[glam::Vec2]) -> f32 {
    if positions.len() < 2 {
        return 0.0;
    }
    let n = positions.len() as f32;
    let mut cx = 0.0f64;
    let mut cy = 0.0f64;
    for p in positions {
        cx += p.x as f64;
        cy += p.y as f64;
    }
    let centroid = glam::Vec2::new((cx / n as f64) as f32, (cy / n as f64) as f32);
    let mut sumsq = 0.0f64;
    for p in positions {
        let d = crate::spatial::torus_distance(*p, centroid);
        sumsq += (d as f64) * (d as f64);
    }
    ((sumsq / n as f64).sqrt()) as f32
}

/// Pure ArmsRace test: is there a species whose weapon-damage mean rose across
/// a full window while a *different* species' armor mean also rose? Returns
/// `(weaponized_species, weapon_rise)`.
pub fn arms_race_signal(
    weapon_history: &BTreeMap<u32, VecDeque<f32>>,
    armor_history: &BTreeMap<u32, VecDeque<f32>>,
) -> Option<(u32, f32)> {
    let rise = |buf: &VecDeque<f32>| -> Option<f32> {
        if buf.len() < ARMS_WINDOW {
            return None;
        }
        let delta = buf.back()? - buf.front()?;
        (delta >= ARMS_MIN_DELTA).then_some(delta)
    };
    for (wsid, wbuf) in weapon_history.iter() {
        let Some(wrise) = rise(wbuf) else { continue };
        for (asid, abuf) in armor_history.iter() {
            if asid == wsid {
                continue;
            }
            if rise(abuf).is_some() {
                return Some((*wsid, wrise));
            }
        }
    }
    None
}

/// Histogram intersection of two normalized terrain distributions
/// (`Σ min(a_t, b_t)`): 1.0 identical, 0.0 disjoint.
pub fn histogram_overlap(a: &BTreeMap<u8, f32>, b: &BTreeMap<u8, f32>) -> f32 {
    let mut overlap = 0.0f32;
    for (t, av) in a.iter() {
        if let Some(bv) = b.get(t) {
            overlap += av.min(*bv);
        }
    }
    overlap
}

/// Update per-species weapon/armor trend windows from the current population,
/// then edge-trigger ArmsRace when a co-rising trend appears.
fn detect_arms_race(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    // Accumulate per-species means (BTreeMap → deterministic).
    let mut wsum: BTreeMap<u32, (f64, u32)> = BTreeMap::new();
    let mut asum: BTreeMap<u32, (f64, u32)> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        let wd = crate::module::effective_weapon(&world.agents.modules[i])
            .map(|(d, _)| d)
            .unwrap_or(0.0);
        let ap = crate::module::effective_armor_protection(&world.agents.modules[i]);
        let w = wsum.entry(sid).or_insert((0.0, 0));
        w.0 += wd as f64;
        w.1 += 1;
        let a = asum.entry(sid).or_insert((0.0, 0));
        a.0 += ap as f64;
        a.1 += 1;
    }
    let push = |hist: &mut BTreeMap<u32, VecDeque<f32>>, sid: u32, mean: f32| {
        let buf = hist.entry(sid).or_default();
        if buf.len() == ARMS_WINDOW {
            buf.pop_front();
        }
        buf.push_back(mean);
    };
    for (sid, (sum, n)) in wsum.iter() {
        push(&mut world.codex.weapon_history, *sid, (*sum / *n as f64) as f32);
    }
    for (sid, (sum, n)) in asum.iter() {
        push(&mut world.codex.armor_history, *sid, (*sum / *n as f64) as f32);
    }

    let signal = arms_race_signal(&world.codex.weapon_history, &world.codex.armor_history);
    match signal {
        Some((sid, rise)) if !world.codex.arms_race_active => {
            let (lx, ly) = centroid_of(centroids, sid);
            world.codex.push_event(CodexEvent {
                event_type: EventType::ArmsRace,
                tick: world.tick,
                species_id: sid,
                value: rise,
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.arms_race_active = true;
        }
        None => world.codex.arms_race_active = false,
        _ => {}
    }
}

/// TerritoryFormation: a pheromone-marking species that stays clustered (spread
/// ≤ TERRITORY_SPREAD_MAX) for TERRITORY_WINDOW consecutive ticks. Edge-
/// triggered per species; re-arms when the cluster disperses.
fn detect_territory_formation(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Gather per-species member positions and whether the species marks (has a
    // Pheromone module member). BTreeMap → deterministic.
    let mut positions: BTreeMap<u32, Vec<glam::Vec2>> = BTreeMap::new();
    let mut marks: BTreeSet<u32> = BTreeSet::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        positions.entry(sid).or_default().push(world.agents.position[i]);
        if crate::module::has(&world.agents.modules[i], crate::module::ModuleType::Pheromone) {
            marks.insert(sid);
        }
    }
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, ps) in positions.iter() {
        if (ps.len() as u32) < TERRITORY_MIN_MEMBERS || !marks.contains(sid) {
            world.codex.territory_spread.remove(sid);
            world.codex.territory_active.remove(sid);
            continue;
        }
        let spread = species_spread(ps);
        let buf = world.codex.territory_spread.entry(*sid).or_default();
        if buf.len() == TERRITORY_WINDOW {
            buf.pop_front();
        }
        buf.push_back(spread);
        let clustered =
            buf.len() == TERRITORY_WINDOW && buf.iter().all(|&s| s <= TERRITORY_SPREAD_MAX);
        if clustered && !world.codex.territory_active.contains(sid) {
            let (lx, ly) = centroid_of(centroids, *sid);
            to_push.push(CodexEvent {
                event_type: EventType::TerritoryFormation,
                tick,
                species_id: *sid,
                value: *buf.back().unwrap(),
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.territory_active.insert(*sid);
        } else if !clustered {
            world.codex.territory_active.remove(sid);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// NichePartitioning: two ≥NICHE_MIN_MEMBERS species whose terrain-type
/// distributions overlap ≤ NICHE_OVERLAP_MAX for NICHE_WINDOW consecutive ticks.
fn detect_niche_partitioning(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Per-species normalized terrain histogram (terrain discriminant → fraction).
    let mut counts: BTreeMap<u32, BTreeMap<u8, f32>> = BTreeMap::new();
    let mut totals: BTreeMap<u32, u32> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        let (col, row) = crate::biome::BiomeField::cell_coords(world.agents.position[i]);
        let terrain = world.biome.at(col, row).terrain as u8;
        *counts.entry(sid).or_default().entry(terrain).or_insert(0.0) += 1.0;
        *totals.entry(sid).or_insert(0) += 1;
    }
    // Normalize and keep only species with enough members.
    let mut hist: BTreeMap<u32, BTreeMap<u8, f32>> = BTreeMap::new();
    for (sid, h) in counts.into_iter() {
        let n = *totals.get(&sid).unwrap_or(&0);
        if n < NICHE_MIN_MEMBERS {
            continue;
        }
        let nf = n as f32;
        hist.insert(sid, h.into_iter().map(|(t, c)| (t, c / nf)).collect());
    }
    let sids: Vec<u32> = hist.keys().copied().collect();
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for ai in 0..sids.len() {
        for bi in (ai + 1)..sids.len() {
            let (a, b) = (sids[ai], sids[bi]); // a < b (ascending keys)
            let overlap = histogram_overlap(&hist[&a], &hist[&b]);
            let key = (a, b);
            if overlap <= NICHE_OVERLAP_MAX {
                let s = world.codex.niche_streak.entry(key).or_insert(0);
                *s += 1;
                if *s >= NICHE_WINDOW && !world.codex.niche_active.contains(&key) {
                    let (lx, ly) = centroid_of(centroids, a);
                    to_push.push(CodexEvent {
                        event_type: EventType::NichePartitioning,
                        tick,
                        species_id: a,
                        value: overlap,
                        loc_x: lx,
                        loc_y: ly,
                    });
                    world.codex.niche_active.insert(key);
                }
            } else {
                world.codex.niche_streak.remove(&key);
                world.codex.niche_active.remove(&key);
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// L2 distance between two meme vectors.
pub fn meme_l2(a: &[f32; MEME_CHANNELS], b: &[f32; MEME_CHANNELS]) -> f32 {
    let mut s = 0.0f32;
    for ch in 0..MEME_CHANNELS {
        let d = a[ch] - b[ch];
        s += d * d;
    }
    s.sqrt()
}

/// DialectFormed: a Communicator-bearing species whose west/east spatial halves
/// maintain divergent meme vectors (L2 ≥ DIALECT_DIVERGENCE_MIN) for a full
/// DIALECT_WINDOW consecutive ticks. Edge-triggered per species; re-arms when
/// divergence drops (clears the buffer).
fn detect_dialect_formed(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Gather per-species member indices; tag species that have a Communicator member.
    let mut members: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut has_comm: BTreeSet<u32> = BTreeSet::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        members.entry(sid).or_default().push(i);
        if crate::module::has(&world.agents.modules[i], crate::module::ModuleType::Communicator) {
            has_comm.insert(sid);
        }
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, idxs) in members.iter() {
        // Only Communicator-bearing species.
        if !has_comm.contains(sid) {
            world.codex.dialect_divergence.remove(sid);
            world.codex.dialect_active.remove(sid);
            continue;
        }
        // Species centroid x.
        let (cx, _cy) = centroid_of(centroids, *sid);
        // Split into west (pos.x < cx) and east (pos.x >= cx).
        let mut west: Vec<usize> = Vec::new();
        let mut east: Vec<usize> = Vec::new();
        for &i in idxs {
            if world.agents.position[i].x < cx {
                west.push(i);
            } else {
                east.push(i);
            }
        }
        // Require each half to have at least DIALECT_MIN_HALF members.
        if (west.len() as u32) < DIALECT_MIN_HALF || (east.len() as u32) < DIALECT_MIN_HALF {
            world.codex.dialect_divergence.remove(sid);
            world.codex.dialect_active.remove(sid);
            continue;
        }
        // Compute per-channel mean meme for each half.
        let mut west_mean = [0.0f32; MEME_CHANNELS];
        let mut east_mean = [0.0f32; MEME_CHANNELS];
        let wn = west.len() as f32;
        let en = east.len() as f32;
        for &i in &west {
            for (ch, w) in west_mean.iter_mut().enumerate() {
                *w += world.agents.meme_vector[i][ch];
            }
        }
        for w in west_mean.iter_mut() {
            *w /= wn;
        }
        for &i in &east {
            for (ch, e) in east_mean.iter_mut().enumerate() {
                *e += world.agents.meme_vector[i][ch];
            }
        }
        for e in east_mean.iter_mut() {
            *e /= en;
        }
        let div = meme_l2(&west_mean, &east_mean);
        // Bounded window push.
        let buf = world.codex.dialect_divergence.entry(*sid).or_default();
        if buf.len() == DIALECT_WINDOW {
            buf.pop_front();
        }
        buf.push_back(div);
        let full_and_diverged =
            buf.len() == DIALECT_WINDOW && buf.iter().all(|&d| d >= DIALECT_DIVERGENCE_MIN);
        if full_and_diverged && !world.codex.dialect_active.contains(sid) {
            let (lx, ly) = centroid_of(centroids, *sid);
            to_push.push(CodexEvent {
                event_type: EventType::DialectFormed,
                tick,
                species_id: *sid,
                value: div,
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.dialect_active.insert(*sid);
        } else if !full_and_diverged {
            world.codex.dialect_active.remove(sid);
            if div < DIALECT_DIVERGENCE_MIN {
                // Clear buffer so a new window starts fresh.
                world.codex.dialect_divergence.remove(sid);
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// MemeSweep: for each Communicator species with ≥ MEME_SWEEP_MIN_MEMBERS,
/// track per-channel mean meme values over a MEME_SWEEP_WINDOW window. Fire
/// once per (species, channel) when the front of the window was ≤ MEME_SWEEP_LOW
/// and the back is ≥ MEME_SWEEP_HIGH (a sweep from rare to dominant). Re-arms
/// when the back drops below MEME_SWEEP_LOW again.
fn detect_meme_sweep(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Per-species meme sums for each channel; track species with Communicator members.
    let mut meme_sums: BTreeMap<u32, ([f64; MEME_CHANNELS], u32)> = BTreeMap::new();
    let mut has_comm: BTreeSet<u32> = BTreeSet::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        if crate::module::has(&world.agents.modules[i], crate::module::ModuleType::Communicator) {
            has_comm.insert(sid);
        }
        let entry = meme_sums.entry(sid).or_insert(([0.0f64; MEME_CHANNELS], 0));
        for ch in 0..MEME_CHANNELS {
            entry.0[ch] += world.agents.meme_vector[i][ch] as f64;
        }
        entry.1 += 1;
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, (sums, n)) in meme_sums.iter() {
        // Must have Communicator and enough members.
        if !has_comm.contains(sid) || *n < MEME_SWEEP_MIN_MEMBERS {
            continue;
        }
        let (lx, ly) = centroid_of(centroids, *sid);
        let nf = *n as f64;
        for (ch, &s) in sums.iter().enumerate() {
            let mean = (s / nf) as f32;
            let key = (*sid, ch as u8);
            let buf = world.codex.meme_mean_history.entry(key).or_default();
            if buf.len() == MEME_SWEEP_WINDOW {
                buf.pop_front();
            }
            buf.push_back(mean);
            let back = *buf.back().unwrap();
            // Re-arm: when back drops below MEME_SWEEP_LOW the latch is released.
            if back < MEME_SWEEP_LOW {
                world.codex.meme_sweep_active.remove(&key);
            }
            if buf.len() == MEME_SWEEP_WINDOW && !world.codex.meme_sweep_active.contains(&key) {
                let front = *buf.front().unwrap();
                if front <= MEME_SWEEP_LOW && back >= MEME_SWEEP_HIGH {
                    to_push.push(CodexEvent {
                        event_type: EventType::MemeSweep,
                        tick,
                        species_id: *sid,
                        value: back,
                        loc_x: lx,
                        loc_y: ly,
                    });
                    world.codex.meme_sweep_active.insert(key);
                }
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// AlarmCall: fires once (latched) when alarm-channel broadcasts co-occur
/// with nearby same-species agents fleeing a threat. Accumulates a cumulative
/// count of alarm→flee co-occurrences; emits when it reaches ALARM_MIN_RESPONSES.
fn detect_alarm_call(world: &mut World) {
    if world.codex.alarm_emitted {
        return;
    }
    // The detector reads this tick's actions/sensors/movement scratch. In a real
    // tick these are always sized (step() calls resize_scratch first); guard so
    // a standalone observe_all (e.g. detector unit tests) is a safe no-op.
    let cap = world.agents.capacity();
    if world.actions.len() < cap || world.sensors.len() < cap || world.desired_direction.len() < cap
    {
        return;
    }
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    let tick = world.tick;
    let mut first_caller_species: Option<u32> = None;
    let mut first_caller_pos: (f32, f32) = (0.0, 0.0);
    for &id in &alive_ids {
        let i = id as usize;
        // Only Communicator agents broadcasting on the alarm channel.
        if !crate::module::has(&world.agents.modules[i], crate::module::ModuleType::Communicator) {
            continue;
        }
        if world.actions[i].broadcast_intent[ALARM_MEME_CHANNEL] <= MEME_BROADCAST_THRESHOLD {
            continue;
        }
        let range = crate::module::effective_communicator_range(&world.agents.modules[i])
            .min(PERCEPTION_MAX_RADIUS);
        if range <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let caller_species = world.agents.species_id[i];
        let mut responses_this_caller: u32 = 0;
        world.spatial.query(pos, range, |oid| {
            if oid == id {
                return;
            }
            let j = oid as usize;
            if world.agents.species_id[j] != caller_species {
                return;
            }
            let nearest_other_dist = world.sensors[j].nearest_other_dist;
            if !nearest_other_dist.is_finite() {
                return;
            }
            let dd = world.desired_direction[j];
            let threat_dir = world.sensors[j].nearest_other_dir;
            if dd.dot(threat_dir) < 0.0 {
                responses_this_caller += 1;
            }
        });
        world.codex.alarm_responses += responses_this_caller;
        // Record the first alarm broadcaster this tick as the event's location,
        // regardless of whether it drew responses — so the payload never
        // defaults to (0,0) when the threshold tips on a zero-response caller.
        if first_caller_species.is_none() {
            first_caller_species = Some(caller_species);
            first_caller_pos = (pos.x, pos.y);
        }
        if world.codex.alarm_responses >= ALARM_MIN_RESPONSES {
            let (lx, ly) = first_caller_pos;
            let sid = first_caller_species.unwrap_or(caller_species);
            world.codex.push_event(CodexEvent {
                event_type: EventType::AlarmCall,
                tick,
                species_id: sid,
                value: world.codex.alarm_responses as f32,
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.alarm_emitted = true;
            return;
        }
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
    detect_arms_race(world, &centroids);
    detect_territory_formation(world, &centroids);
    detect_niche_partitioning(world, &centroids);
    detect_dialect_formed(world, &centroids);
    detect_meme_sweep(world, &centroids);
    detect_alarm_call(world);
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
