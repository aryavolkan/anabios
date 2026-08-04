//! Codex detector tuning parameters — window sizes, thresholds, and rates for
//! every emergence detector.
//!
//! Extracted verbatim from `codex/mod.rs` so the god-file holds types and
//! orchestration, not ~130 interleaved constants. Re-exported from the codex
//! root (`pub use params::*` in `mod.rs`), so every existing reference —
//! `crate::codex::FOO` and the detectors' `use super::*` — resolves unchanged.
//! Grouped by the detector each constant serves.

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
/// Herbivore rise (over stage-entry level) required for stage 2.
pub const CASCADE_HERB_RISE: f32 = 0.3;
/// Plant-biomass drop (below stage-entry level) that completes the cascade.
pub const CASCADE_PLANT_DROP: f32 = 0.3;

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

/// Speed below this fraction of module max counts as "still" (ambush wait).
pub const STILL_SPEED_FRAC: f32 = 0.05;
/// Consecutive still ticks before firing that mark an ambush hit.
pub const AMBUSH_STILL_MIN: u32 = 40;
/// Rolling window (ticks) for per-species hit-share detectors.
pub const SIG_HIT_WINDOW: u64 = 400;
/// Minimum hits in the window for the ambush share to be meaningful.
pub const AMBUSH_MIN_HITS: u32 = 10;
/// Share of ambush-flagged hits that fires EvolvedAmbush (re-arm: 0.15).
pub const AMBUSH_MIN_SHARE: f32 = 0.3;
/// Species-level Metalworking adoption fraction required for EvolvedTool
/// (alongside ≥1 actual boosted hit in the window — tool use is adoption
/// put to work, robust to the sim's bursty combat).
pub const TOOL_ADOPTION_SHARE: f32 = 0.3;

/// Fast barrier crossings (agent-ticks on water/rock at ≥70% max speed)
/// over a 400-tick window that fire EvolvedFlight — with at least
/// `FLIGHT_MIN_PER_QUARTER` in each window quarter, so a one-off dash
/// through a lake doesn't count (sustained behavior only).
pub const FLIGHT_MIN_CROSSINGS: u32 = 50;
/// Per-quarter minimum for sustained flight behavior.
pub const FLIGHT_MIN_PER_QUARTER: u32 = 10;
/// A flier's mean module speed must exceed the world mean by this factor —
/// the evolutionary claim (relative adaptation), not just fast movement.
pub const FLIGHT_RELATIVE_SPEED: f64 = 1.5;
/// Speed fraction of module max that counts as "fast" for flight.
pub const FLIGHT_SPEED_FRAC: f32 = 0.7;

/// Ticks within which receivers must converge for a signal response.
pub const SIGNAL_WINDOW: u64 = 30;
/// Same-species receivers converging on the caller that count as a response.
pub const SIGNAL_CONVERGE_MIN: u32 = 3;
/// Cumulative responses that fire StructuredSignaling.
pub const SIGNAL_MIN_RESPONSES: u32 = 10;

/// Per-tick hostility-score decay for every species pair.
pub const WAR_DECAY: f32 = 0.995;
/// Hostility score that declares a war (rising edge).
pub const WAR_THRESHOLD: f32 = 12.0;
/// Hostility score per cross-faction combat HIT (deaths score 1.0 — wars
/// are fought with hits, deaths are decisive moments).
pub const WAR_HIT_SCORE: f32 = 0.1;
/// Consecutive ticks below half-threshold before a latched war ends.
pub const WAR_END_TICKS: u64 = 200;

/// Max mean-meme L2 distance for an alliance's shared culture.
pub const ALLIANCE_MEME_MAX: f32 = 0.3;
/// Window (ticks) of zero cross-kills required for an alliance. (The share
/// tally uses `COOPERATION_WINDOW`, the horizon `share_events` is pruned to.)
pub const ALLIANCE_WINDOW: u64 = 400;
/// Minimum cross-species energy shares in the window for an alliance.
pub const ALLIANCE_MIN_SHARES: u32 = 5;

/// Minimum members for a kin network.
pub const KIN_MIN_MEMBERS: u32 = 10;
/// Max RMS spatial spread (world units) for a cohesive kin network.
pub const KIN_SPREAD_MAX: f32 = 100.0;
/// Ticks of sustained size + cohesion for KinNetworkStable.
pub const KIN_WINDOW: u32 = 1500;

/// EMA rate for anchor learning (scaled by Territoriality).
pub const ANCHOR_LEARN_RATE: f32 = 0.01;
/// Gaussian jitter on the inherited child anchor (world units).
pub const ANCHOR_DRIFT_SIGMA: f32 = 4.0;
/// Homing pull strength at Territoriality = 1.
pub const ANCHOR_PULL: f32 = 0.5;
/// Max anchor RMS spread (world units) for a formed settlement.
pub const SETTLEMENT_SPREAD_MAX: f32 = 100.0;
/// Minimum anchored members for a settlement.
pub const SETTLEMENT_MIN_MEMBERS: u32 = 10;
/// Ticks of sustained anchor cohesion for SettlementFormed.
pub const SETTLEMENT_WINDOW: u32 = 400;

/// Market density deposited per successful swap at the initiator's cell.
pub const MARKET_DEPOSIT: f32 = 1.0;
/// Per-tick market-field decay.
pub const MARKET_DECAY: f32 = 0.999;
/// Sustained density that makes a cell a market node.
pub const MARKET_NODE_THRESHOLD: f32 = 40.0;
/// Ticks of sustained density for MarketEmerged.
pub const MARKET_WINDOW: u32 = 400;

/// Harvest-experience gain per harvest.
pub const HARVEST_EXP_RATE: f32 = 0.01;
/// Experience cap (bounds the harvest bonus).
pub const EXP_CAP: f32 = 10.0;
/// Harvest-amount bonus per experience point.
pub const SPECIALIZATION_GAIN: f32 = 0.1;
/// Experience share in one good that marks a producer specialist.
pub const SPECIALIST_SHARE: f32 = 0.6;
/// Minimum fraction of a species in each producer class for a split.
pub const SPECIALIZATION_MIN_CLASS: f32 = 0.2;

/// Ticks a lineage must stay above the adoption streak for TraditionPreserved.
pub const TRADITION_WINDOW: u32 = 2000;
/// Adoption fraction (of a species) that counts as "held" — a tradition is
/// the majority custom, not a subculture.
pub const TRADITION_ADOPTION_SHARE: f32 = 0.5;
/// Minimum age of the lineage root for a tradition (2× the window: the
/// custom predates its current carriers by generations).
pub const TRADITION_MIN_AGE: u64 = 4000;
/// Minimum descendants (distinct variants, across ≥2 factions) for radiation.
pub const RADIATION_MIN_DESCENDANTS: usize = 50;
/// Ticks of continuous era ≥ 2 for InstitutionalRatchet.
pub const RATCHET_WINDOW: u32 = 2000;
/// Minimum era for the ratchet.
pub const RATCHET_MIN_ERA: u8 = 2;
/// Transmission/inheritance jitter scale when both parties are
/// settlement-latched (institutional fidelity).
pub const SETTLED_FIDELITY: f32 = 0.25;
/// Ticks between per-agent band-transition sweeps (amortized).
pub const VARIANT_SWEEP_INTERVAL: u64 = 50;
/// Cap on the variant registry — beyond it, band transitions reuse the
/// parent variant (lineage collapses gracefully instead of growing memory).
pub const VARIANT_REGISTRY_CAP: usize = 20_000;

/// Minimum absolute lag (|mean skill − current optimum|) for a check to count
/// toward the maladaptation streak (E11). The seasonal optimum sweeps a 0.5-wide
/// band, so a quarter-band gap is a species chronically off the moving target.
pub const MALADAPT_LAG_MIN: f32 = 0.25;
/// Consecutive-lag ticks required before MaladaptationLag fires. The streak
/// advances by `CYCLE_CHECK_INTERVAL` per amortized check, so this is 500 ticks
/// (50 checks) of sustained lag — a persistent stress, not a transient dip.
pub const MALADAPT_WINDOW: u32 = 500;

/// Minimum rise in a species' mean dimorphism gene (slot 33) across the
/// genome-moment ring for SexualSelection (E12) — a clear directional trend,
/// not drift noise (matches the RapidAdaptation delta).
pub const SEXSEL_MIN_DELTA: f32 = 0.15;
/// Newest ring mean required for SexualSelection — the knob must end up
/// strongly expressed, not merely trending.
pub const SEXSEL_MIN_MEAN: f32 = 0.65;
/// Re-arm threshold below SEXSEL_MIN_MEAN (hysteresis band).
pub const SEXSEL_REARM_MEAN: f32 = 0.55;
/// Minimum species size for the SexRatioCollapse check (small species churn
/// their sex ratio harmlessly).
pub const SEXRATIO_MIN_TOTAL: u32 = 12;
/// A species this size with fewer than this many members of either sex is
/// one bad tick from reproductive extinction.
pub const SEXRATIO_MIN_MINORITY: u32 = 3;

/// Minimum living tamed members of a livestock species for a herd to count
/// toward LivestockHerd (E13).
pub const LIVESTOCK_HERD_MIN: u32 = 6;
/// Consecutive ticks at/above `LIVESTOCK_HERD_MIN` before LivestockHerd fires.
pub const LIVESTOCK_HERD_WINDOW: u64 = 500;

// --- M-F: affect-detector tuning (FeedingFrenzy, TerritorialRage,
// PanicCascade, MassGrief) ---

/// Per-system "high activation" thresholds (leaky-integrator value in [0,1]).
pub const HIGH_FEAR: f32 = 0.6;
pub const HIGH_SEEK: f32 = 0.6;
pub const HIGH_RAGE: f32 = 0.6;
pub const HIGH_PANIC: f32 = 0.6;

/// PANIC CASCADE: high-FEAR member count must jump by >= this within the window.
pub const CASCADE_MIN_SPREAD: u32 = 5;
/// Ticks over which the high-FEAR count is compared for a cascade rise.
pub const AFFECT_CASCADE_WINDOW: usize = 20;
/// Min species members before a cascade is meaningful.
pub const CASCADE_MIN_MEMBERS: u32 = 8;

/// FEEDING FRENZY: converged high-SEEK members required.
pub const FRENZY_MIN_MEMBERS: u32 = 6;
/// Max RMS spatial spread (world units) for the high-SEEK members to count as
/// "converged" on one patch.
pub const FRENZY_SPREAD_MAX: f32 = 90.0;

/// TERRITORIAL RAGE: species mean RAGE at/above this counts as an angry cluster.
pub const RAGE_CLUSTER_MEAN: f32 = 0.5;
/// Max RMS spread for the cluster to be territorial (co-located aggression).
pub const RAGE_CLUSTER_SPREAD_MAX: f32 = 120.0;
/// Ticks of sustained angry-cluster before TerritorialRage fires.
pub const RAGE_WINDOW: u32 = 60;
/// Min members for a rage cluster.
pub const RAGE_MIN_MEMBERS: u32 = 5;

/// MASS GRIEF: species mean PANIC at/above this after a die-off.
pub const GRIEF_MEAN_PANIC: f32 = 0.45;
/// Population-drop fraction (over the grief window) that qualifies as a die-off.
pub const GRIEF_DROP_FRAC: f32 = 0.3;
/// Ticks over which the die-off drop is measured.
pub const GRIEF_WINDOW: usize = 60;
/// Min pre-die-off population for grief to be meaningful.
pub const GRIEF_MIN_PEAK: u32 = 12;
