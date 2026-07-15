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

mod combat;
mod culture;
mod population;
mod spatial;

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

/// Rolling window (ticks) over which share events accumulate for EvolvedCooperation.
pub const COOPERATION_WINDOW: u64 = 100;
/// Minimum share events within the window to declare EvolvedCooperation.
pub const COOPERATION_MIN_SHARES: usize = 12;

/// Rolling window (ticks) over which combat hits accumulate for PackHunting.
pub const PACK_WINDOW: u64 = 8;
/// Distinct same-species attackers on one target needed to declare PackHunting.
pub const PACK_MIN_ATTACKERS: usize = 3;

/// Ticks of history tracked for the per-(species,channel) meme mean sweep.
pub const MEME_SWEEP_WINDOW: usize = 80;
/// Meme mean must start at or below this for a MemeSweep.
pub const MEME_SWEEP_LOW: f32 = 0.2;
/// Meme mean must reach at or above this for a MemeSweep.
pub const MEME_SWEEP_HIGH: f32 = 0.6;
/// Minimum members a species needs for MemeSweep to be meaningful.
pub const MEME_SWEEP_MIN_MEMBERS: u32 = 5;

/// Ticks a species must sustain high crowding to fire HerdCohesion.
pub const HERD_WINDOW: usize = 60;
/// Minimum mean same-species crowding (neighbors per member) to count as cohesive.
pub const HERD_CROWDING_MIN: f32 = 3.0;
/// Minimum members before herd cohesion detection is meaningful.
pub const HERD_MIN_MEMBERS: u32 = 5;

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
    /// A species sustains a high energy-sharing rate over a rolling window.
    EvolvedCooperation = 14,
    /// ≥ PACK_MIN_ATTACKERS distinct same-species agents deal combat damage to
    /// one target within PACK_WINDOW ticks.
    PackHunting = 15,
    /// A species maintains persistently high mean per-member same-species crowding
    /// over a full HERD_WINDOW window.
    HerdCohesion = 16,
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

/// A single combat hit (attacker deals damage to target). Fuel for the
/// PackHunting detector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatHit {
    pub tick: u64,
    pub target_id: u32,
    pub attacker_id: u32,
    pub species: u32,
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
    /// Rolling window of share events (tick, donor_species) for EvolvedCooperation.
    pub share_events: VecDeque<(u64, u32)>,
    /// Species currently latched as having evolved cooperation (re-arms on drop).
    pub cooperation_active: BTreeSet<u32>,
    /// Rolling window of combat hits for PackHunting.
    pub combat_hits: VecDeque<CombatHit>,
    /// Edge-trigger state for PackHunting (armed while no qualifying group).
    pub pack_active: bool,
    /// Rolling per-species mean same-species crowding window (for HerdCohesion).
    pub herd_crowding: BTreeMap<u32, VecDeque<f32>>,
    /// Species currently latched as exhibiting herd cohesion.
    pub herd_active: BTreeSet<u32>,
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

/// L2 distance between two meme vectors.
pub fn meme_l2(a: &[f32; MEME_CHANNELS], b: &[f32; MEME_CHANNELS]) -> f32 {
    let mut s = 0.0f32;
    for ch in 0..MEME_CHANNELS {
        let d = a[ch] - b[ch];
        s += d * d;
    }
    s.sqrt()
}

/// Run all detectors. Called by the tick orchestrator at the end of each
/// tick. SpeciationEvent is emitted directly from `species_step`.
pub fn observe_all(world: &mut World) {
    // Compute per-species centroids once (immutable agent borrow) so the
    // mutable-codex detectors can reuse them for event locations.
    let centroids = compute_centroids(world);

    population::update_pop_history(world);
    population::detect_extinction(world, &centroids);
    population::detect_population_crash(world, &centroids);
    population::detect_migration(world, &centroids);
    population::detect_novel_modules(world, &centroids);
    population::detect_novel_behavior(world, &centroids);
    // Predation runs before CombatRaid on purpose: it scans this tick's
    // combat-death entries, and CombatRaid then prunes entries older than the
    // window (which never includes the current tick).
    combat::detect_predation(world);
    combat::detect_combat_raid(world);
    combat::detect_arms_race(world, &centroids);
    spatial::detect_territory_formation(world, &centroids);
    spatial::detect_niche_partitioning(world, &centroids);
    culture::detect_dialect_formed(world, &centroids);
    culture::detect_meme_sweep(world, &centroids);
    culture::detect_alarm_call(world);
    culture::detect_evolved_cooperation(world, &centroids);
    combat::detect_pack_hunting(world, &centroids);
    spatial::detect_herd_cohesion(world, &centroids);
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

pub(super) fn centroid_of(centroids: &BTreeMap<u32, (f32, f32)>, sid: u32) -> (f32, f32) {
    centroids.get(&sid).copied().unwrap_or((0.0, 0.0))
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
