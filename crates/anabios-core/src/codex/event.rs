//! Codex event vocabulary and the "fuel" records detectors read.
//!
//! `EventType` enumerates every emergence the codex can record; `CodexEvent`
//! is one recorded occurrence. The remaining structs (`CombatDeath`,
//! `CombatHit`, `SigHit`, `HostilityRecord`, `MemeVariant`, `TraitMoments`)
//! are the per-tick / persistent inputs the detectors aggregate over.
//! Extracted verbatim from `codex/mod.rs`.

use super::*;

/// A meme variant: one quantized band of one meme channel, with parentage —
/// the lineage unit of cultural descent (E9).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemeVariant {
    pub id: u32,
    pub channel: u8,
    /// `floor(value × 10)` at birth, clamped to [0, 10].
    pub band: u8,
    pub parent: Option<u32>,
    /// The lineage root variant (self when `parent` is None) — stored at
    /// birth so tradition/radiation aggregation is O(1) per holder instead
    /// of a per-tick ancestor walk (quadratic blowup measured in profile).
    pub root: u32,
    pub born_tick: u64,
    pub born_species: u32,
}

/// Per-species-pair war state (E7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostilityRecord {
    /// Decaying kill score; war declares at `WAR_THRESHOLD`.
    pub score: f32,
    /// Decaying directional scores: kills BY the lo-keyed species and by the
    /// hi-keyed one. Tracked for inspection only — `detect_war` declares on
    /// the combined `score` (one-way killing already counts as war pressure),
    /// so these do not currently gate the decision.
    pub score_lo: f32,
    pub score_hi: f32,
    /// Total cross-kills over the record's life.
    pub kills: u32,
    /// Running mean of cross-kill locations (the "front").
    pub front_x: f32,
    pub front_y: f32,
    /// Last tick a cross-kill was recorded.
    pub last_kill_tick: u64,
    /// Tick the war latched (`u64::MAX` = not at war — tick 0 is a real
    /// declare tick, so 0 cannot be the sentinel).
    pub war_since: u64,
    /// Ticks spent below half-threshold (for `WarEnded`).
    pub below_ticks: u64,
}

impl Default for HostilityRecord {
    fn default() -> Self {
        Self {
            score: 0.0,
            score_lo: 0.0,
            score_hi: 0.0,
            kills: 0,
            front_x: 0.0,
            front_y: 0.0,
            last_kill_tick: 0,
            war_since: u64::MAX,
            below_ticks: 0,
        }
    }
}

/// One per-species genome-moment sample (mean + variance over all 50 slots,
/// stored genome-shaped for the manual array serde impls).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitMoments {
    pub tick: u64,
    pub mean: crate::genome::Genome,
    pub var: crate::genome::Genome,
}

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
    /// An agent completed learning an invention by spending its material
    /// basket (`value` = invention id). Fires per agent per acquisition —
    /// both for discovery breakthroughs and social-copy completions.
    MaterialLearning = 22,
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
    /// ≥30% of a species' weapon hits came from attackers lying in wait
    /// (≥40 still ticks before firing; `value` = ambush share).
    EvolvedAmbush = 34,
    /// ≥30% of a species' weapon hits were invention-boosted (Metalworking;
    /// `value` = boosted share).
    EvolvedTool = 35,
    /// A species repeatedly crosses barrier terrain at ≥70% of max speed
    /// (`value` = fast crossings in the window).
    EvolvedFlight = 36,
    /// Broadcasts reliably followed by ≥3 same-species receivers converging
    /// on the caller (`value` = cumulative responses).
    StructuredSignaling = 37,
    /// Inter-species hostility crossed the war threshold — a sustained war,
    /// distinct from the one-off CombatRaid burst (`value` = kills so far).
    WarOrRaid = 38,
    /// A latched war decayed below half-threshold for `WAR_END_TICKS`
    /// (`value` = war duration in ticks).
    WarEnded = 39,
    /// Species pair with shared meme signature, zero cross-kills, and
    /// sustained energy sharing over the alliance window (`value` = shares).
    AllianceFormed = 40,
    /// A species sustained ≥`KIN_MIN_MEMBERS` members and tight spatial
    /// cohesion over `KIN_WINDOW` ticks (`value` = window ticks).
    KinNetworkStable = 41,
    /// A species' member anchors cluster within `SETTLEMENT_SPREAD_MAX`
    /// over the settlement window (`value` = anchor RMS spread).
    SettlementFormed = 42,
    /// A biome cell sustained market density ≥ `MARKET_NODE_THRESHOLD`
    /// (`value` = density; loc = cell center).
    MarketEmerged = 43,
    /// A species split into ≥2 producer classes specialized on different
    /// goods (`value` = smaller class fraction).
    SpecializationSplit = 44,
    /// A meme variant held by ≥30% of a species for `TRADITION_WINDOW`
    /// ticks, outliving its founders (`value` = variant id).
    TraditionPreserved = 45,
    /// A variant's descendant tree reached ≥`RADIATION_MIN_DESCENDANTS`
    /// variants across ≥2 species (`value` = descendant count).
    CulturalRadiation = 46,
    /// A species held tech era ≥2 continuously for `RATCHET_WINDOW` ticks
    /// despite holder turnover (`value` = era).
    InstitutionalRatchet = 47,
    /// A species' mean skill stayed ≥`MALADAPT_LAG_MIN` from the drifting
    /// environmental optimum for `MALADAPT_WINDOW` ticks — chronic climate
    /// maladaptation, only meaningful under a moving optimum (`value` = lag).
    MaladaptationLag = 48,
    /// A species' mean dimorphism gene (slot 33) rose ≥`SEXSEL_MIN_DELTA`
    /// across the genome-moment ring to ≥`SEXSEL_MIN_MEAN` — directional
    /// sexual selection on the dimorphism knob (`value` = newest mean d).
    SexualSelection = 49,
    /// A species with ≥`SEXRATIO_MIN_TOTAL` members dropped below
    /// `SEXRATIO_MIN_MINORITY` of one sex — one bad tick from losing a sex
    /// and going reproductively extinct (`value` = minority fraction).
    SexRatioCollapse = 50,
    /// A member of this species was tamed by a Husbandry holder for the
    /// first time (`value` = the herder's species id).
    AnimalDomesticated = 51,
    /// A livestock species sustained ≥`LIVESTOCK_HERD_MIN` living tamed members for
    /// `LIVESTOCK_HERD_WINDOW` consecutive ticks (`value` = tamed count).
    LivestockHerd = 52,
    /// A species that had lost Writing re-acquired an invention faster than
    /// its founding rate because of retained per-species knowledge
    /// (`value` = accumulated knowledge at re-acquisition).
    KnowledgeRatchet = 53,
}

/// Number of `EventType` variants. Derived from the last variant so it stays
/// correct as variants are appended; the viewer asserts its parallel name/color
/// arrays against this at boot to catch a forgotten GDScript-side update.
pub const EVENT_TYPE_COUNT: usize = EventType::KnowledgeRatchet as usize + 1;

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
    /// Attacker had been still ≥ `AMBUSH_STILL_MIN` ticks at fire time (E6).
    #[serde(default)]
    pub ambush: bool,
    /// Damage was invention-boosted at fire time (Metalworking; E6).
    #[serde(default)]
    pub tool_boosted: bool,
}

/// Rolling record of one combat hit for the E6 named-behavior detectors.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SigHit {
    pub tick: u64,
    pub species: u32,
    pub ambush: bool,
    pub tool_boosted: bool,
}
