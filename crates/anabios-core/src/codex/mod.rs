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
mod cycles;
mod disturbance;
mod invention;
mod population;
mod practice;
mod spatial;
mod traits;

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

/// Per-species population window for cycle/plateau analysis. Deliberately
/// separate from the 200-tick `POP_HISTORY_WINDOW` so crash-detector
/// semantics (and golden behavior) stay untouched.
pub const CYCLE_WINDOW: usize = 400;
/// Cycle/plateau checks run every this many ticks (amortized).
pub const CYCLE_CHECK_INTERVAL: u64 = 10;
/// Zero-crossing intervals must land in this band to count as periodic.
pub const CYCLE_PERIOD_MIN: u64 = 40;
pub const CYCLE_PERIOD_MAX: u64 = 200;
/// Peak absolute deviation from the window mean, as a fraction of the mean.
pub const CYCLE_MIN_AMPLITUDE: f32 = 0.25;
/// Peak/trough ratio that upgrades a cycle to BoomAndBust.
pub const BOOM_AMPLITUDE: f32 = 3.0;
/// Minimum mean population for a carrying-capacity plateau to be meaningful.
pub const CARRYING_MIN_POP: f32 = 20.0;
/// Max coefficient of variation (std/mean) over the window for a plateau.
pub const CARRYING_MAX_CV: f32 = 0.05;

/// Carnivore-population window (ticks) for the cascade peak reference.
pub const CASCADE_WINDOW: usize = 150;
/// Carnivore drop below the window peak that opens a cascade candidate.
pub const CASCADE_CRASH_FRAC: f32 = 0.5;
/// Minimum carnivore peak for the crash to be ecologically meaningful.
pub const CASCADE_MIN_PREDATORS: u32 = 5;
/// Max ticks between cascade stages 0→1→2 before the candidate times out.
pub const CASCADE_LAG: u64 = 300;
/// Max ticks for the final plant-crash leg (stage 2→fire). Plant biomass
/// responds to grazer release far more slowly than prey responds to
/// predator release, so this leg gets its own, much longer budget.
pub const CASCADE_PLANT_LAG: u64 = 900;

/// Per-species occupied-cell window for RangeExpansion.
pub const RANGE_WINDOW: usize = 400;
/// Occupied-cell growth ratio (end/start) required for RangeExpansion.
pub const RANGE_GROWTH: f32 = 1.5;
/// Minimum occupied cells for the expansion to be meaningful.
pub const RANGE_MIN_CELLS: u32 = 20;
/// Minimum centroid displacement (world units) over the migration window.
pub const RANGE_MIN_DISPLACEMENT: f32 = 60.0;

/// Ticks of consecutive low overlap before SegregationEmerged fires.
pub const SEGREGATION_WINDOW: u32 = 200;
/// Max fraction of a species' occupied cells shared with other species.
pub const SEGREGATION_OVERLAP_MAX: f32 = 0.1;
/// Minimum members / occupied cells for segregation to be meaningful.
pub const SEGREGATION_MIN_MEMBERS: u32 = 20;
pub const SEGREGATION_MIN_CELLS: usize = 20;

/// Migrations in agreeing directions that count as corridor use.
pub const CORRIDOR_MIN_MIGRATIONS: usize = 4;
/// Cosine of the max pairwise angle for directions to "agree" (~14°).
pub const CORRIDOR_MAX_ANGLE_COS: f32 = 0.97;
/// Minimum tick span across the agreeing migrations — a corridor is a
/// sustained habit, not a burst of same-direction hops.
pub const CORRIDOR_MIN_SPAN: u64 = 400;
/// Sampled points along a migration's displacement; aggregated across the
/// recent legs, at least this many must land on barrier terrain (water/rock)
/// for the movement to count as a corridor — plain grassland drift is not.
pub const CORRIDOR_MIN_BARRIER_HITS: u32 = 6;
/// Points sampled along the displacement for the barrier check.
pub const CORRIDOR_BARRIER_SAMPLES: u32 = 16;
/// Per-species cap on remembered migration directions.
pub const CORRIDOR_DIR_CAP: usize = 4;

/// Fraction of a fire scar's tracked cells that must return to Climax for
/// the Succession event.
pub const SUCCESSION_RECOVERED_FRAC: f32 = 0.5;

/// Span (ticks) covered by the per-species genome-moment ring.
pub const MOMENT_SPAN: u64 = 400;
/// Samples per species in the genome-moment ring (at 10-tick cadence).
pub const MOMENT_RING: usize = 40;
/// Variance at/above which a slot counts as polymorphic (TraitFixation start).
pub const FIX_POLY_VAR: f32 = 0.02;
/// Variance at/below which a slot counts as collapsed/fixed.
pub const FIX_COLLAPSE_VAR: f32 = 0.005;
/// Minimum members for variance to be meaningful.
pub const FIX_MIN_MEMBERS: u32 = 10;
/// Samples (×10 ticks) compared for RapidAdaptation (100 ticks).
pub const RAPID_WINDOW: usize = 10;
/// Minimum absolute mean shift for RapidAdaptation.
pub const RAPID_MIN_DELTA: f32 = 0.15;
/// Shift must also exceed this multiple of the slot's recent σ.
pub const RAPID_SIGMA_MULT: f32 = 3.0;
/// Per-(species, slot) cooldown between RapidAdaptation events.
pub const RAPID_COOLDOWN: u64 = 400;
/// Max |Δmean| for two fixations to count as the same adaptive band.
pub const CONVERGE_MEAN_TOL: f32 = 0.15;
/// Bounded archive of past fixations scanned for convergence.
pub const FIXATION_ARCHIVE_CAP: usize = 500;

/// One per-species genome-moment sample (mean + variance over all 50 slots,
/// stored genome-shaped for the manual array serde impls).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitMoments {
    pub tick: u64,
    pub mean: crate::genome::Genome,
    pub var: crate::genome::Genome,
}
/// Herbivore rise (over stage-entry level) required for stage 2.
pub const CASCADE_HERB_RISE: f32 = 0.3;
/// Plant-biomass drop (below stage-entry level) that completes the cascade.
pub const CASCADE_PLANT_DROP: f32 = 0.3;
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
    /// First agent in the world to hold an invention (`value` = invention id).
    InventionDiscovered = 17,
    /// An invention crosses ≥50% adoption inside a species (`value` =
    /// invention id).
    InventionAdopted = 18,
    /// A maladaptive cultural practice is held for the first time anywhere
    /// (`value` = practice id). Fires once per practice.
    PracticeDiscovered = 19,
    /// A maladaptive practice crosses ≥50% penetration inside a species
    /// (`value` = practice id).
    PracticeAdopted = 20,
    /// First bilateral cross-species resource swap in the world (latched once).
    ResourceTraded = 21,
    /// An offspring was produced by spending a full dowry basket.
    DowryBirth = 22,
    /// A species' population oscillates with a regular period (zero-crossing
    /// analysis over `CYCLE_WINDOW`; `value` = mean period in ticks).
    PopulationCycleDetected = 23,
    /// A population cycle with peak/trough amplitude ≥ `BOOM_AMPLITUDE`
    /// (`value` = peak/trough ratio).
    BoomAndBust = 24,
    /// A species' population variance collapses at a sustained plateau
    /// (`value` = mean population over the window).
    CarryingCapacityReached = 25,
    /// Ordered trophic cascade: carnivore crash → herbivore boom → plant
    /// crash (`value` = carnivore drop fraction; world-scale, loc = 0,0).
    TrophicCascade = 26,
    /// A species' occupied-cell count grew ≥50% over `RANGE_WINDOW` with
    /// centroid displacement (`value` = growth ratio).
    RangeExpansion = 27,
    /// A species' occupied cells stay <10% shared with all other species
    /// over `SEGREGATION_WINDOW` (`value` = 1 − overlap).
    SegregationEmerged = 28,
    /// A species logged ≥`CORRIDOR_MIN_MIGRATIONS` migrations in agreeing
    /// directions — recurrent directed passage (`value` = direction angle).
    CorridorUse = 29,
    /// ≥50% of a fire's scar returned to Climax succession (loc = epicenter).
    Succession = 30,
    /// A genome slot collapsed from polymorphic to fixed within a species
    /// (`value` = slot id).
    TraitFixation = 31,
    /// A genome slot's mean moved ≥ max(0.15, 3σ) within 100 ticks
    /// (`value` = slot id).
    RapidAdaptation = 32,
    /// Two independent lineages (LCA = founder root) fixed the same slot in
    /// the same band (`value` = slot id; species = the newer fixer).
    ConvergentEvolution = 33,
}

/// Number of `EventType` variants. Derived from the last variant so it stays
/// correct as variants are appended; the viewer asserts its parallel name/color
/// arrays against this at boot to catch a forgotten GDScript-side update.
pub const EVENT_TYPE_COUNT: usize = EventType::ConvergentEvolution as usize + 1;

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
    /// Inventions discovered at least once anywhere in the world (latched;
    /// `InventionDiscovered` fires once per invention id).
    pub inventions_discovered: BTreeSet<u8>,
    /// (species, invention) pairs currently latched as adopted (≥50% of the
    /// species holds it). Re-arms when adoption drops below the threshold.
    pub inventions_adopted: BTreeSet<(u32, u8)>,
    /// Latch: the first cross-species `ResourceTraded` event has been emitted
    /// (biome trade goods feature). Kept with the other one-shot latches.
    pub first_cross_species_trade: bool,
    /// Maladaptive practices held at least once anywhere (latched;
    /// `PracticeDiscovered` fires once per practice id).
    pub practices_discovered: BTreeSet<u8>,
    /// (species, practice) pairs currently latched as adopted (≥50% penetration).
    /// Re-arms when penetration drops below the threshold.
    pub practices_adopted: BTreeSet<(u32, u8)>,
    /// Long per-species population window for cycle/plateau analysis
    /// (`CYCLE_WINDOW`; separate from the crash detector's `pop_history`).
    pub cycle_history: BTreeMap<u32, VecDeque<u32>>,
    /// Guild/world population windows for cycle/plateau analysis. Per-species
    /// lines churn under 200-tick reclustering; the ecologically meaningful
    /// oscillator is the trophic guild (0=herbivore, 1=carnivore series) and
    /// the world total. Keyed as three explicit fields for serde simplicity.
    pub herb_cycle_history: VecDeque<u32>,
    pub carn_cycle_history: VecDeque<u32>,
    pub total_cycle_history: VecDeque<u32>,
    /// Guild/world series currently latched as cycling (0=herb, 1=carn, 2=total).
    pub guild_cycle_active: BTreeSet<u8>,
    /// Guild/world series currently latched as boom-and-bust.
    pub guild_boom_active: BTreeSet<u8>,
    /// Guild/world series currently latched as plateaued.
    pub guild_carrying_active: BTreeSet<u8>,
    /// Species currently latched as cycling (re-arms when the checks fail).
    pub cycle_active: BTreeSet<u32>,
    /// Species currently latched as boom-and-bust cycling.
    pub boom_active: BTreeSet<u32>,
    /// Species currently latched as plateaued at carrying capacity.
    pub carrying_active: BTreeSet<u32>,
    /// Rolling carnivore-population window for the cascade peak reference.
    pub cascade_carn_history: VecDeque<u32>,
    /// Cascade state machine: 0 armed, 1 predator crashed, 2 herbivores booming.
    pub cascade_stage: u8,
    /// Tick the machine entered the current stage (for `CASCADE_LAG` timeouts).
    pub cascade_stage_tick: u64,
    /// Carnivore window peak when the cascade candidate opened (for `value`).
    pub cascade_carn_peak: u32,
    /// Herbivore population when the cascade candidate opened (stage 1 entry).
    pub cascade_herb_entry: u32,
    /// Plant biomass when the cascade candidate opened (stage 1 entry).
    pub cascade_plant_entry: f32,
    /// Per-species occupied-cell-count history for RangeExpansion.
    pub range_occ_history: BTreeMap<u32, VecDeque<u32>>,
    /// Species currently latched as range-expanding.
    pub range_active: BTreeSet<u32>,
    /// Per-species consecutive-tick streak of low spatial overlap.
    pub segregation_streak: BTreeMap<u32, u32>,
    /// Species observed spatially mixed (overlap ≥ threshold) at least once.
    /// SegregationEmerged requires this — it marks segregation that EMERGED
    /// from a mixed state, not species founded apart.
    pub segregation_was_mixed: BTreeSet<u32>,
    /// Species currently latched as segregated.
    pub segregation_active: BTreeSet<u32>,
    /// Recent migration records per species: `(tick, dir_x, dir_y,
    /// barrier_hits)` — fed by the migration detector (cap
    /// `CORRIDOR_DIR_CAP`).
    pub migration_dirs: BTreeMap<u32, VecDeque<(u64, f32, f32, u32)>>,
    /// Species currently latched as corridor users.
    pub corridor_active: BTreeSet<u32>,
    /// Per-species genome-moment history (10-tick cadence, ring of
    /// `MOMENT_RING`), pruned when a species leaves the active set for a
    /// full `MOMENT_SPAN`.
    pub genome_moments: BTreeMap<u32, VecDeque<TraitMoments>>,
    /// (species, slot) pairs currently latched as fixed.
    pub fixation_latches: BTreeSet<(u32, u8)>,
    /// (species, slot) → last RapidAdaptation fire tick (cooldown).
    pub rapid_cooldown: BTreeMap<(u32, u8), u64>,
    /// Bounded archive of past fixations `(species, slot, mean)` scanned by
    /// the convergence detector.
    pub fixation_archive: VecDeque<(u32, u8, f32)>,
    /// Index watermark into `fixation_archive`: entries before it have
    /// already been evaluated for convergence. Adjusted down by
    /// `archive_fixation` whenever the bounded archive drops its front, so it
    /// keeps tracking the same logical entry as older fixations age out.
    pub converge_watermark: usize,
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

    /// Append a fixation `(species, slot, mean)` to the bounded convergence
    /// archive. When the archive is full and drops its front, the convergence
    /// watermark is decremented in lockstep — otherwise it stays pinned at the
    /// cap and the convergence detector silently stops evaluating new entries.
    pub fn archive_fixation(&mut self, entry: (u32, u8, f32)) {
        if self.fixation_archive.len() == FIXATION_ARCHIVE_CAP {
            self.fixation_archive.pop_front();
            self.converge_watermark = self.converge_watermark.saturating_sub(1);
        }
        self.fixation_archive.push_back(entry);
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
#[derive(Debug, Clone)]
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
    /// Sum of per-member `effective_diet_carnivory` (0 = herbivore …
    /// 1 = carnivore); mean classifies the species' trophic level for the
    /// cascade detector.
    pub diet_sum: f64,
    /// Per-invention count of members at or above the held threshold (for
    /// `InventionAdopted`). All zero when the invention tree is inactive.
    pub invention_counts: [u32; crate::invention::INVENTION_COUNT],
    /// Per-practice count of members holding each maladaptive practice (for
    /// `PracticeAdopted`). All zero when cognition is inactive.
    pub practice_counts: [u32; crate::practice::PRACTICE_COUNT],
    /// Distinct biome cells occupied by members this tick (scratch for the
    /// E4 spatial detectors). Cell index = row * res + col.
    pub occ_cells: std::collections::BTreeSet<u32>,
    /// Per-slot genome sums / squared sums over members (for the E5
    /// genome-moment history). 50 slots each.
    pub genome_sums: [f64; 50],
    pub genome_sumsq: [f64; 50],
}

impl Default for SpeciesAgg {
    fn default() -> Self {
        Self {
            count: 0,
            sum_x: 0.0,
            sum_y: 0.0,
            member_idx: Vec::new(),
            has_comm: false,
            has_pheromone: false,
            module_mask: 0,
            node_mask: 0,
            terrain_counts: [0.0; TERRAIN_SLOTS],
            meme_sums: [0.0; MEME_CHANNELS],
            crowding_sum: 0.0,
            weapon_sum: 0.0,
            armor_sum: 0.0,
            diet_sum: 0.0,
            invention_counts: [0; crate::invention::INVENTION_COUNT],
            practice_counts: [0; crate::practice::PRACTICE_COUNT],
            occ_cells: std::collections::BTreeSet::new(),
            genome_sums: [0.0; 50],
            genome_sumsq: [0.0; 50],
        }
    }
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
        self.diet_sum = 0.0;
        self.invention_counts = [0; crate::invention::INVENTION_COUNT];
        self.practice_counts = [0; crate::practice::PRACTICE_COUNT];
        self.occ_cells.clear();
        self.genome_sums = [0.0; 50];
        self.genome_sumsq = [0.0; 50];
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
            e.occ_cells.insert(world.biome.cell_index(col, row) as u32);
            for (ch, s) in e.meme_sums.iter_mut().enumerate() {
                *s += world.agents.meme_vector[i][ch] as f64;
            }
            if sensors_ok {
                e.crowding_sum += world.sensors[i].crowding as f64;
            }
            e.diet_sum += module::effective_diet_carnivory(modules) as f64;
            for (slot, gv) in world.agents.genome[i].0.iter().enumerate() {
                let x = *gv as f64;
                e.genome_sums[slot] += x;
                e.genome_sumsq[slot] += x * x;
            }
            if world.inventions_enabled {
                let inv_mask = crate::invention::held_mask(&world.agents.meme_vector[i]);
                crate::invention::for_each_set_bit(inv_mask, |k| e.invention_counts[k] += 1);
            }
            if world.cognition_enabled {
                for (p, c) in e.practice_counts.iter_mut().enumerate() {
                    if crate::practice::has(&world.agents.meme_vector[i], p) {
                        *c += 1;
                    }
                }
            }
            e.weapon_sum +=
                module::effective_weapon(modules).map(|w| w.damage).unwrap_or(0.0) as f64;
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
    cycles::update_cycle_history(world, &agg);
    cycles::detect_cycles(world, &agg);
    cycles::detect_carrying_capacity(world, &agg);
    cycles::detect_trophic_cascade(world, &agg);
    disturbance::update_range_history(world, &agg);
    disturbance::detect_range_expansion(world, &agg);
    disturbance::detect_segregation(world, &agg);
    disturbance::detect_corridor_use(world, &agg);
    disturbance::detect_succession(world);
    // Fixation runs before convergence: convergence scans the archive
    // entries this cycle appended.
    traits::update_genome_moments(world, &agg);
    traits::detect_trait_fixation(world, &agg);
    traits::detect_rapid_adaptation(world, &agg);
    traits::detect_convergent_evolution(world, &agg);
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
    invention::detect_inventions(world, &agg);
    practice::detect_practices(world, &agg);
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

    /// The fused aggregation must reproduce the per-detector scans it replaced:
    /// counts, centroids, module/node masks, terrain histograms, sums.
    #[test]
    fn species_agg_matches_hand_rolled_expectations() {
        use crate::genome::Genome;
        use crate::module::{Module, ModuleList};
        use crate::prelude::Vec2;

        let mut w = World::new(21);
        // Species 0: two plain founders (starter kit).
        let a = w.spawn_agent(Vec2::new(100.0, 100.0), Genome::neutral());
        let _b = w.spawn_agent(Vec2::new(300.0, 100.0), Genome::neutral());
        // Species 1: one communicator+weapon agent.
        let c = w.spawn_agent(Vec2::new(600.0, 200.0), Genome::neutral());
        let sid1 = crate::prelude_test::reassign_to_new_species(&mut w, c);
        let mut kit: ModuleList = crate::module::communicator_kit();
        kit.push(Module::Weapon { damage: 4.0, energy_cost: 1.0 });
        w.agents.modules[c as usize] = kit;

        let mut agg = std::mem::take(&mut w.codex_agg);
        agg.build(&w);

        // Active set: both species, ascending.
        assert_eq!(agg.active(), &[0, sid1]);

        let e0 = agg.get(0).expect("species 0 entry");
        assert_eq!(e0.count, 2);
        assert_eq!(e0.member_idx.len(), 2);
        let (cx, cy) = e0.centroid();
        assert!((cx - 200.0).abs() < 1e-4 && (cy - 100.0).abs() < 1e-4);
        assert!(!e0.has_comm && !e0.has_pheromone);
        // Starter kit: Locomotor(0), Sensor(1), Mouth(2), Reproductive(?).
        let starter = crate::module::starter_kit();
        let mut want_mask = 0u16;
        for m in starter.iter() {
            want_mask |= 1u16 << (m.module_type() as u8);
        }
        assert_eq!(e0.module_mask, want_mask);
        // Node mask covers the starter grazer program's kinds.
        let mut want_nodes = 0u64;
        for n in crate::program::starter_grazer().nodes.iter().copied() {
            want_nodes |= 1u64 << crate::program::Program::node_kind(n);
        }
        assert_eq!(e0.node_mask, want_nodes);
        // Every member landed in exactly one terrain slot.
        assert_eq!(e0.terrain_counts.iter().sum::<f32>(), 2.0);
        assert_eq!(e0.weapon_sum, 0.0);

        let e1 = agg.get(sid1).expect("species 1 entry");
        assert_eq!(e1.count, 1);
        assert!(e1.has_comm, "communicator kit flags has_comm");
        assert_eq!(e1.weapon_sum, 4.0);
        let (cx1, cy1) = e1.centroid();
        assert!((cx1 - 600.0).abs() < 1e-4 && (cy1 - 200.0).abs() < 1e-4);

        // Rebuild reuses storage without leaking previous state.
        w.agents.kill(a);
        agg.build(&w);
        assert_eq!(agg.get(0).map(|e| e.count), Some(1));
        w.codex_agg = agg;
    }
}
