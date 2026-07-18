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
use crate::spatial::torus_distance;
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

/// RMS distance (torus-aware) of `positions` from their coordinate mean, on a
/// torus of the given `world_size`. Returns 0.0 for fewer than 2 points.
pub fn species_spread(positions: &[glam::Vec2], world_size: f32) -> f32 {
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
        let d = crate::spatial::torus_distance(*p, centroid, world_size);
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
/// (`Σ min(a_t, b_t)`): 1.0 identical, 0.0 disjoint. Zero slots contribute
/// nothing, so fixed-array iteration matches the former sparse-map version.
pub fn histogram_overlap(a: &[f32; TERRAIN_SLOTS], b: &[f32; TERRAIN_SLOTS]) -> f32 {
    let mut overlap = 0.0f32;
    for t in 0..TERRAIN_SLOTS {
        overlap += a[t].min(b[t]);
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

/// Terrain histogram slot count. `TerrainType` has 5 variants; 8 gives
/// headroom so a new variant doesn't silently corrupt the niche detector.
pub const TERRAIN_SLOTS: usize = 8;

/// Per-species aggregates for one tick, built in a single `iter_alive` pass
/// at the top of `observe_all` and shared by every detector. Replaces the
/// ~7 hand-rolled per-detector population scans (and their per-tick
/// `BTreeMap` churn) with one pass over dense, reused storage.
///
/// Lives on `World` behind `#[serde(skip)]` — never part of the snapshot or
/// the state hash. All accumulations visit agents in ascending id order so
/// every f32/f64 sum is bit-identical to the scans this replaced.
#[derive(Debug, Clone, Default)]
pub struct SpeciesAggTable {
    /// Dense per-species entries, indexed by species id. Grows as needed.
    entries: Vec<SpeciesAgg>,
    /// Species with ≥1 alive member this tick, ascending.
    active: Vec<u32>,
}

/// Aggregates for one species for one tick.
#[derive(Debug, Clone, Default)]
pub struct SpeciesAgg {
    pub count: u32,
    pub sum_x: f64,
    pub sum_y: f64,
    /// Alive member agent indices, ascending.
    pub member_idx: Vec<usize>,
    pub has_comm: bool,
    pub has_pheromone: bool,
    /// Bitmask of present `ModuleType` discriminants (< 16 types).
    pub module_mask: u16,
    /// Bitmask of present program `node_kind` discriminants (≤ 42 kinds).
    pub node_mask: u64,
    /// Raw per-terrain member counts (normalized by `count` on read).
    pub terrain_counts: [f32; TERRAIN_SLOTS],
    pub meme_sums: [f64; MEME_CHANNELS],
    /// Sum of `SensorRegister::crowding`; 0 when the sensors scratch is
    /// undersized (standalone `observe_all` calls outside the tick).
    pub crowding_sum: f64,
    pub weapon_sum: f64,
    pub armor_sum: f64,
}

impl SpeciesAgg {
    fn reset(&mut self) {
        self.count = 0;
        self.sum_x = 0.0;
        self.sum_y = 0.0;
        self.member_idx.clear();
        self.has_comm = false;
        self.has_pheromone = false;
        self.module_mask = 0;
        self.node_mask = 0;
        self.terrain_counts = [0.0; TERRAIN_SLOTS];
        self.meme_sums = [0.0; MEME_CHANNELS];
        self.crowding_sum = 0.0;
        self.weapon_sum = 0.0;
        self.armor_sum = 0.0;
    }

    /// This tick's centroid (mean alive position), `(0,0)` when empty.
    /// f64 accumulator divided by member count — identical to the former
    /// `compute_centroids`.
    #[inline]
    pub fn centroid(&self) -> (f32, f32) {
        let nf = self.count.max(1) as f64;
        ((self.sum_x / nf) as f32, (self.sum_y / nf) as f32)
    }
}

impl SpeciesAggTable {
    /// Entry for `sid`, if the species has ≥1 alive member this tick.
    #[inline]
    pub fn get(&self, sid: u32) -> Option<&SpeciesAgg> {
        self.entries.get(sid as usize).filter(|e| e.count > 0)
    }

    /// Species ids with ≥1 alive member, ascending.
    #[inline]
    pub fn active(&self) -> &[u32] {
        &self.active
    }

    /// Rebuild from current world state. One `iter_alive` pass.
    pub fn build(&mut self, world: &World) {
        use crate::module::{self, ModuleType};
        for &sid in &self.active {
            if let Some(e) = self.entries.get_mut(sid as usize) {
                e.reset();
            }
        }
        self.active.clear();
        let sensors_ok = world.sensors.len() >= world.agents.capacity();
        for id in world.agents.iter_alive() {
            let i = id as usize;
            let sid = world.agents.species_id[i];
            let idx = sid as usize;
            if idx >= self.entries.len() {
                self.entries.resize(idx + 1, SpeciesAgg::default());
            }
            let e = &mut self.entries[idx];
            if e.count == 0 {
                self.active.push(sid);
            }
            e.count += 1;
            let pos = world.agents.position[i];
            e.sum_x += pos.x as f64;
            e.sum_y += pos.y as f64;
            e.member_idx.push(i);
            let modules = &world.agents.modules[i];
            if !e.has_comm && module::has(modules, ModuleType::Communicator) {
                e.has_comm = true;
            }
            if !e.has_pheromone && module::has(modules, ModuleType::Pheromone) {
                e.has_pheromone = true;
            }
            for m in modules.iter() {
                e.module_mask |= 1u16 << (m.module_type() as u8);
            }
            for node in world.agents.program[i].nodes.iter().copied() {
                e.node_mask |= 1u64 << crate::program::Program::node_kind(node);
            }
            let (col, row) = world.biome.cell_coords(pos);
            let terrain = world.biome.at(col, row).terrain as usize;
            e.terrain_counts[terrain.min(TERRAIN_SLOTS - 1)] += 1.0;
            for (ch, s) in e.meme_sums.iter_mut().enumerate() {
                *s += world.agents.meme_vector[i][ch] as f64;
            }
            if sensors_ok {
                e.crowding_sum += world.sensors[i].crowding as f64;
            }
            e.weapon_sum += module::effective_weapon(modules).map(|(d, _)| d).unwrap_or(0.0) as f64;
            e.armor_sum += module::effective_armor_protection(modules) as f64;
        }
        self.active.sort_unstable();
    }
}

/// West/east spatial-half meme divergence kernel shared by the DialectFormed
/// detector and the Godot coevo metric. Splits `idxs` at centroid x `cx`,
/// computes per-half per-channel meme means (f32, ascending index order),
/// and returns their L2 distance. `None` when either half has fewer than
/// `min_half` members.
pub fn west_east_meme_divergence(
    idxs: &[usize],
    cx: f32,
    min_half: u32,
    sample: impl Fn(usize) -> (f32, [f32; MEME_CHANNELS]),
) -> Option<f32> {
    let mut west_mean = [0.0f32; MEME_CHANNELS];
    let mut east_mean = [0.0f32; MEME_CHANNELS];
    let mut wn = 0u32;
    let mut en = 0u32;
    for &i in idxs {
        let (x, meme) = sample(i);
        if x < cx {
            for (ch, w) in west_mean.iter_mut().enumerate() {
                *w += meme[ch];
            }
            wn += 1;
        } else {
            for (ch, e) in east_mean.iter_mut().enumerate() {
                *e += meme[ch];
            }
            en += 1;
        }
    }
    if wn < min_half || en < min_half {
        return None;
    }
    for w in west_mean.iter_mut() {
        *w /= wn as f32;
    }
    for e in east_mean.iter_mut() {
        *e /= en as f32;
    }
    Some(meme_l2(&west_mean, &east_mean))
}

/// Run all detectors. Called by the tick orchestrator at the end of each
/// tick. SpeciationEvent is emitted directly from `species_step`.
pub fn observe_all(world: &mut World) {
    // One fused per-species aggregation pass (replaces the per-detector
    // population scans each detector used to hand-roll). Taken out of the
    // world so detectors can borrow `world` mutably while reading `agg`.
    let mut agg = std::mem::take(&mut world.codex_agg);
    agg.build(world);

    population::update_pop_history(world);
    population::detect_extinction(world, &agg);
    population::detect_population_crash(world, &agg);
    population::detect_migration(world, &agg);
    population::detect_novel_modules(world, &agg);
    population::detect_novel_behavior(world, &agg);
    // Predation runs before CombatRaid on purpose: it scans this tick's
    // combat-death entries, and CombatRaid then prunes entries older than the
    // window (which never includes the current tick).
    combat::detect_predation(world);
    combat::detect_combat_raid(world);
    combat::detect_arms_race(world, &agg);
    spatial::detect_territory_formation(world, &agg);
    spatial::detect_niche_partitioning(world, &agg);
    culture::detect_dialect_formed(world, &agg);
    culture::detect_meme_sweep(world, &agg);
    culture::detect_alarm_call(world);
    culture::detect_evolved_cooperation(world, &agg);
    combat::detect_pack_hunting(world, &agg);
    spatial::detect_herd_cohesion(world, &agg);

    world.codex_agg = agg;
}

pub(super) fn centroid_of(agg: &SpeciesAggTable, sid: u32) -> (f32, f32) {
    agg.get(sid).map(|e| e.centroid()).unwrap_or((0.0, 0.0))
}

/// Per-species edge-trigger latch. On the rising edge (`fired` and `sid` not
/// already active) marks `sid` active and returns the event to push; on a
/// falling edge (`!fired`) clears `sid`. Returns `None` when there is nothing to
/// emit. Centralizes the latch the species-keyed detectors previously hand-rolled.
pub(super) fn edge_trigger_species(
    active: &mut BTreeSet<u32>,
    sid: u32,
    fired: bool,
    make: impl FnOnce() -> CodexEvent,
) -> Option<CodexEvent> {
    if fired {
        if active.insert(sid) {
            return Some(make());
        }
    } else {
        active.remove(&sid);
    }
    None
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
