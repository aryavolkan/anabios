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

mod affect;
mod affect_events;
mod agg;
mod anthro;
mod climate;
mod combat;
mod culture;
mod cycles;
mod dimorphism;
mod disturbance;
mod domestication;
mod event;
mod invention;
mod knowledge;
mod metrics;
mod params;
mod population;
mod practice;
pub(crate) mod settlement;
pub(crate) mod signatures;
mod spatial;
pub(crate) mod traditions;
mod traits;
pub(crate) mod war;

// Detector tuning parameters live in `params.rs`; re-export them at the codex
// root so `crate::codex::FOO` and the detectors' `use super::*` resolve exactly
// as when these constants lived here.
pub use params::*;

// Pure metric kernels live in `metrics.rs`; re-export so `crate::codex::FOO`
// (e.g. the Godot coevo panel's `west_east_meme_divergence`) and the detectors'
// `use super::*` resolve unchanged.
pub use metrics::*;

// The per-species aggregation machine lives in `agg.rs`; re-export
// `SpeciesAggTable`/`SpeciesAgg` (used by `World`, the detectors, and the
// Godot binding) so their `crate::codex::` paths resolve unchanged.
pub use agg::*;

// The event vocabulary + detector "fuel" records live in `event.rs`; re-export
// `EventType`, `CodexEvent`, and the input structs so `CodexState`, the
// detectors, and external `crate::codex::` references all resolve unchanged.
pub use event::*;

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
    /// Rolling window of share events (tick, donor_species, recipient_species)
    /// for EvolvedCooperation and the E7 alliance detector.
    pub share_events: VecDeque<(u64, u32, u32)>,
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
    /// Rolling per-species (species → (total, ambush, tool_boosted)) hit
    /// counters over `SIG_HIT_WINDOW`, rebuilt from `sig_hit_log`.
    pub sig_hit_log: VecDeque<SigHit>,
    /// Species currently latched as ambushers / tool users.
    pub ambush_active: BTreeSet<u32>,
    pub tool_active: BTreeSet<u32>,
    /// Per-species fast-barrier-crossing agent-ticks over the flight window.
    pub flight_log: BTreeMap<u32, VecDeque<(u64, u32)>>,
    /// Species currently latched as fliers (keyed by lineage ROOT — one
    /// flight event per lineage, not per speciation splinter).
    pub flight_active: BTreeSet<u32>,
    /// Cumulative signal responses per species (StructuredSignaling).
    pub signal_responses: BTreeMap<u32, u32>,
    /// Last tick a signal response was counted per species (rate limit: one
    /// response per `SIGNAL_WINDOW` so spawn-tick alignments can't dump a
    /// full counter at once).
    pub signal_last_response: BTreeMap<u32, u64>,
    /// Species currently latched as signalers.
    pub signal_active: BTreeSet<u32>,
    /// Per-species-pair war state (E7). Keyed by ordered pair (lo, hi).
    pub hostility: BTreeMap<(u32, u32), HostilityRecord>,
    /// Species pairs currently latched as allied.
    pub alliance_active: BTreeSet<(u32, u32)>,
    /// Per-species consecutive-tick streak of size + cohesion (KinNetwork).
    pub kin_streak: BTreeMap<u32, u32>,
    /// Species currently latched as stable kin networks.
    pub kin_active: BTreeSet<u32>,
    /// Per-species consecutive-tick streak of anchor cohesion (Settlement).
    pub settlement_streak: BTreeMap<u32, u32>,
    /// Species currently latched as settled.
    pub settlement_active: BTreeSet<u32>,
    /// Per-cell consecutive-tick streak of market density (Market).
    pub market_streak: BTreeMap<u32, u32>,
    /// Cells currently latched as market nodes.
    pub market_active: BTreeSet<u32>,
    /// Species currently latched as specialization-split.
    pub specialization_active: BTreeSet<u32>,
    /// Meme variant registry (E9), keyed by variant id.
    pub meme_variants: BTreeMap<u32, MemeVariant>,
    pub next_variant_id: u32,
    /// (variant, species) → consecutive-tick streak above adoption share.
    pub tradition_streaks: BTreeMap<(u32, u32), u32>,
    /// (variant, species) pairs currently latched as traditions.
    pub tradition_active: BTreeSet<(u32, u32)>,
    /// Ancestor variant ids whose radiation has been reported.
    pub radiation_active: BTreeSet<u32>,
    /// Per-species consecutive-tick streak at era ≥ `RATCHET_MIN_ERA`.
    pub ratchet_streak: BTreeMap<u32, u32>,
    /// Species currently latched as institutional-ratcheted.
    pub ratchet_active: BTreeSet<u32>,
    /// Per-species consecutive-lag streak (ticks) for MaladaptationLag (E11).
    /// Advances by `CYCLE_CHECK_INTERVAL` per check while lag ≥
    /// `MALADAPT_LAG_MIN`; resets to 0 when the species catches up.
    pub maladapt_streak: BTreeMap<u32, u32>,
    /// Species currently latched as climate-maladapted (re-arms on catch-up).
    pub maladapt_active: BTreeSet<u32>,
    /// Species currently latched as under directional sexual selection on the
    /// dimorphism gene (E12; re-arms below `SEXSEL_REARM_MEAN`).
    pub sexsel_active: BTreeSet<u32>,
    /// Species currently latched as sex-ratio-collapsed (E12; re-arms when
    /// the minority sex recovers to `SEXRATIO_MIN_MINORITY`).
    pub sexratio_active: BTreeSet<u32>,
    /// Livestock species with at least one tamed member so far (E13 latch;
    /// `AnimalDomesticated` fires once per livestock species). Written by
    /// `domestication::husbandry_step`.
    pub domesticated_species: BTreeSet<u32>,
    /// Per-species consecutive-tick streak at ≥`HERD_MIN` tamed members (E13).
    pub livestock_herd_streak: BTreeMap<u32, u64>,
    /// Species currently latched as livestock herds (re-arms on drop).
    pub livestock_herd_active: BTreeSet<u32>,
    /// Per-species accumulated knowledge (E14; rises while any member holds
    /// Writing, decays slowly otherwise). Written by `knowledge::knowledge_step`.
    pub knowledge_by_species: BTreeMap<u32, f32>,
    /// Species that have fired `KnowledgeRatchet` — a permanent
    /// once-per-species latch; fires the first tick
    /// `knowledge_by_species[sp] >= KNOWLEDGE_RATCHET_MIN`. See
    /// `codex::knowledge` for the v1-vs-deferred design note.
    pub knowledge_ratchet_fired: BTreeSet<u32>,
    /// Species currently latched as feeding-frenzy (re-arms on drop; M-F).
    pub frenzy_active: BTreeSet<u32>,
    /// Per-species sustained-angry-cluster streak (TerritorialRage; M-F).
    pub rage_streak: BTreeMap<u32, u32>,
    /// Species currently latched as territorial-rage (M-F).
    pub rage_active: BTreeSet<u32>,
    /// Rolling per-species high-FEAR member counts (PanicCascade contagion; M-F).
    pub fear_count_history: BTreeMap<u32, VecDeque<u32>>,
    /// Species currently latched as panic-cascading (M-F).
    pub cascade_active: BTreeSet<u32>,
    /// Species currently latched as mass-grieving (re-arms when PANIC subsides; M-F).
    pub grief_active: BTreeSet<u32>,
    /// Per (culture_root, prey_root) baseline indices latched when the pair
    /// qualifies (anthropogenic arms race): `(culture power, prey armor,
    /// prey speed, prey vigilance)` at qualification tick.
    pub hunted_baselines: BTreeMap<(u32, u32), (f32, f32, f32, f32)>,
    /// Hunter/hunted root pairs that have fired HuntedAdaptation (one shot
    /// per continuous qualification).
    pub hunted_active: BTreeSet<(u32, u32)>,
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

/// Terrain histogram slot count. `TerrainType` has 5 variants; 8 gives
/// headroom so a new variant doesn't silently corrupt the niche detector.
pub const TERRAIN_SLOTS: usize = 8;

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
    // E12 runs after the trait pass: SexualSelection reads this tick's
    // freshly-appended genome moments.
    dimorphism::detect_sexual_selection(world, &agg);
    dimorphism::detect_sex_ratio_collapse(world, &agg);
    domestication::detect_livestock_herd(world, &agg);
    knowledge::detect_knowledge_ratchet(world, &agg);
    signatures::detect_ambush_and_tool(world, &agg);
    signatures::detect_flight(world, &agg);
    signatures::detect_structured_signaling(world);
    war::detect_war(world);
    war::detect_alliance(world, &agg);
    war::detect_kin_network(world, &agg);
    settlement::detect_settlement(world, &agg);
    settlement::detect_market(world);
    settlement::detect_specialization(world, &agg);
    traditions::variant_sweep(world);
    traditions::detect_tradition(world, &agg);
    traditions::detect_radiation(world, &agg);
    traditions::detect_ratchet(world, &agg);
    population::detect_migration(world, &agg);
    population::detect_novel_modules(world, &agg);
    population::detect_novel_behavior(world, &agg);
    // Predation runs before CombatRaid on purpose: it scans this tick's
    // combat-death entries, and CombatRaid then prunes entries older than the
    // window (which never includes the current tick).
    combat::detect_predation(world);
    combat::detect_combat_raid(world);
    combat::detect_arms_race(world, &agg);
    if world.anthro_race_enabled {
        anthro::detect_hunted_adaptation(world, &agg);
    }
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
    climate::detect_maladaptation(world, &agg);
    affect::detect_mass_fright(world, &agg);
    if world.affect_enabled {
        affect_events::detect_feeding_frenzy(world, &agg);
        affect_events::detect_territorial_rage(world, &agg);
        affect_events::detect_panic_cascade(world, &agg);
        affect_events::detect_mass_grief(world, &agg);
    }

    world.codex_agg = agg;
}

pub(super) fn centroid_of(agg: &SpeciesAggTable, sid: u32) -> (f32, f32) {
    agg.get(sid).map(|e| e.centroid()).unwrap_or((0.0, 0.0))
}

/// Per-species edge-trigger latch. On the rising edge (`fired` and `sid` not
/// already active) marks `sid` active and returns the event to push; on a
/// falling edge (`!fired`) clears `sid`. Returns `None` when there is nothing to
/// emit. Centralizes the latch the species-keyed detectors previously hand-rolled.
pub(super) fn edge_trigger_species<K: Ord + Copy>(
    active: &mut BTreeSet<K>,
    key: K,
    fired: bool,
    make: impl FnOnce() -> CodexEvent,
) -> Option<CodexEvent> {
    if fired {
        if active.insert(key) {
            return Some(make());
        }
    } else {
        active.remove(&key);
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
