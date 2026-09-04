//! Scenario initial conditions, loadable from TOML.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;
use crate::world::World;

/// Serde default for flags that preserve prior behavior when absent by being
/// `true` (as opposed to the opt-in flags, which default `false`).
pub(crate) fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
// Reject unknown keys so a misspelled feature flag (`inventions_enable`,
// `sexual_dimorphism = true`) fails loudly at load instead of silently leaving
// the feature off and quietly invalidating an experiment. Every field has a
// `#[serde(default)]`, so absence is still fine — only *unrecognized* keys error.
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub name: String,
    pub seed: u64,
    #[serde(default)]
    pub agents: Vec<AgentSpec>,
    /// DIT environmental-variability period (experiment). `0` (default) = the
    /// env technique mechanism is OFF. `> 0` shifts the optimum every N ticks;
    /// `4294967295` (`u32::MAX`, `culture::ENV_STATIC_PERIOD`) = active-but-static.
    #[serde(default)]
    pub env_period: u32,
    /// Opt-in: enable the biome-adaptation feeding bonus (EnvAffinity vs local
    /// climate). `false` (default) leaves foraging behavior unchanged.
    #[serde(default)]
    pub biome_adaptation: bool,
    /// Opt-in: enable terrain-based habitat selection (agents pulled toward
    /// their `TerrainAffinity` preferred terrain, so species sort into
    /// biomes and trade at borders). `false` (default) leaves movement
    /// unchanged.
    #[serde(default)]
    pub terrain_habitat: bool,
    /// Opt-in: enable the cultural invention tree (discovery + social spread
    /// on the invention meme channels, with per-holder buffs/debuffs).
    /// `false` (default) leaves culture unchanged.
    #[serde(default)]
    pub inventions_enabled: bool,
    /// Opt-in: couple invention buffs and discovery to genome slots
    /// (`invention::GeneAffinity`), so adoption selects the genome and vice
    /// versa. `false` (default) is bit-identical to no coupling.
    #[serde(default)]
    pub gene_tech_coupling: bool,
    /// Opt-in: enforce each invention's hard genetic prerequisite
    /// (`invention::GeneReq`) on discovery and social copying.
    /// `false` (default) is bit-identical to no gate.
    #[serde(default)]
    pub gene_requirements: bool,
    /// Opt-in: enable the cognitive layer (per-agent realized IQ from the
    /// `CognitivePotential` gene + juvenile enrichment, with a metabolic cost).
    /// `false` (default) leaves metabolism and culture unchanged.
    #[serde(default)]
    pub cognition_enabled: bool,
    /// Opt-in: enable the subcortical affect layer (per-agent Panksepp activations
    /// developed each tick; SEEKING biases foraging in M-A). `false` (default)
    /// leaves behavior byte-identical.
    #[serde(default)]
    pub affect_enabled: bool,
    /// Opt-in: enable renewing biome (depleted cells recolonize from
    /// vegetated neighbours). `false` (default) leaves regrowth unchanged.
    #[serde(default)]
    pub living_biome: bool,
    /// Opt-in: season cycle length in ticks. `0` (default) = seasonal biome
    /// regrowth OFF (plain regrowth every biome step). `> 0` boosts regrowth
    /// in cells whose climate matches the current season phase, migrating
    /// the productive band over a `2 * season_period`-tick cycle.
    #[serde(default)]
    pub season_period: u32,
    /// Opt-in secular climate drift (E10): radians/tick of a slow non-stationary
    /// drift added to the environmental optimum on top of the seasonal cycle.
    /// `0.0` (default) leaves the optimum purely seasonal (byte-identical to
    /// pre-E10). A small value like `0.00005` gives a multi-100k-tick wander.
    #[serde(default)]
    pub climate_drift_rate: f32,
    /// Opt-in: enable per-cell nutrient-value variation (energy per bite scaled
    /// by `nutrient_quality`). `false` (default) leaves foraging energy unchanged.
    #[serde(default)]
    pub nutrient_variation: bool,
    /// Opt-in: enable per-cell soil fertility (scales carrying capacity and
    /// regrowth). `false` (default) leaves regrowth unchanged.
    #[serde(default)]
    pub soil_fertility: bool,
    /// Opt-in: enable the biome-trade-goods economy (resource nodes spawn,
    /// agents harvest and trade them, and invention learning requires — and
    /// consumes — per-tech material baskets).
    /// `false` (default) leaves the world unchanged.
    #[serde(default)]
    pub resources_enabled: bool,
    /// Opt-in: conserve trade goods on death (transfer to nearest living
    /// agent) so long-run trade doesn't freeze. Default off.
    #[serde(default)]
    pub conserve_goods_on_death: bool,
    /// Opt-in: enable natural disasters (fire/drought/freeze on a Poisson
    /// schedule, succession scars). `false` (default) leaves the world
    /// unchanged — zero RNG draws, no state.
    #[serde(default)]
    pub disasters_enabled: bool,
    /// Opt-in: `SenseHostility` joins the program mutation pool (E7) so
    /// war-reactive behavior can evolve. `false` (default) keeps the
    /// baseline pool byte-identical.
    #[serde(default)]
    pub war_enabled: bool,
    /// Opt-in: home-range anchoring (E8) — anchors learn, homing pull,
    /// anchor Sense nodes in the mutation pool. `false` (default) keeps
    /// the world byte-identical.
    #[serde(default)]
    pub settlement_enabled: bool,
    /// Opt-in: sexual dimorphism (E12) — binary sex, opposite-sex mating,
    /// female mate choice, sex-linked stat expression. `false` (default)
    /// keeps the world byte-identical (zero extra RNG draws).
    #[serde(default)]
    pub sexual_dimorphism_enabled: bool,
    /// Opt-in: domestication (E13) — Husbandry holders tame wild juvenile
    /// herbivores into penned, milk-yielding livestock that breeds
    /// born-tamed. `false` (default) keeps the world byte-identical.
    /// Effectively requires `inventions_enabled` (taming needs Husbandry).
    #[serde(default)]
    pub domestication_enabled: bool,
    /// Opt-in: knowledge accumulation — Writing-holding cultures build
    /// durable, transmissible tech memory that survives population
    /// bottlenecks. `false` (default) keeps the world byte-identical.
    /// Effectively requires `inventions_enabled` (Writing must exist).
    #[serde(default)]
    pub knowledge_enabled: bool,
    /// Maladaptive cultural practices (Inbreeding, Child Sacrifice). Unlike the
    /// opt-in flags above, this defaults to `true`: practices run whenever
    /// `cognition_enabled` is on, exactly as before the flag existed, so every
    /// existing scenario is unchanged. Set `false` to suppress practice
    /// *discovery* — the only source of practices in a fresh run (with none
    /// discovered there is nothing for copy-toward-best or inherit-jitter to
    /// amplify above threshold), so a fresh run effectively carries none. The O1
    /// autopsy found payoff-blind practice adoption is the dominant lever
    /// excluding culture; this flag makes that experiment reproducible. See
    /// `docs/superpowers/specs/2026-08-03-o1-exclusion-findings.md`.
    #[serde(default = "default_true")]
    pub practices_enabled: bool,
    /// Opt-in O2 payoff-biased social learning: cultural transmission copies
    /// from the highest-energy Communicator neighbour (model bias) and
    /// declines traits whose local holders are lower-energy than non-holders
    /// (content bias), so maladaptive practices are rejected while they still
    /// exist in the world. Off by default ⇒ byte-identical payoff-blind
    /// transmission. See
    /// `docs/superpowers/specs/2026-08-03-o2-payoff-biased-learning-design.md`.
    #[serde(default)]
    pub payoff_biased_learning: bool,
    /// Opt-in basic needs (thirst + sleep as Layer-0 drives): agents
    /// accumulate thirst (drink at water/river cells; dehydration raises
    /// basal drain) and fatigue (sleep on a hysteresis: movement + feeding
    /// suppressed, discounted metabolism, fatigue recovers). `false`
    /// (default) keeps the world byte-identical — zero RNG, no state.
    /// A scenario with no drinkable cells (no Water terrain and
    /// `river_threshold` 0) will dehydrate everyone; pair the flag with water.
    #[serde(default)]
    pub basic_needs_enabled: bool,
    /// Opt-in O3 reproductive-success payoff bias: cultural transmission
    /// declines a maladaptive-practice channel when its local holders show a
    /// higher observed birth-failure fraction than non-holders (content bias
    /// only — no model bias, which O2b measured as skill-suppressing). The
    /// only fitness proxy that can see a stillbirth/cull cost. Off by
    /// default ⇒ byte-identical transmission and no birth-outcome counting.
    /// See `docs/superpowers/specs/2026-09-02-o3-corrected-apparatus-repro-bias-design.md`.
    #[serde(default)]
    pub repro_biased_learning: bool,
    /// Opt-in unilateral (one-sided) exchange: when no mutually-beneficial
    /// barter swap exists, an agent may gift one `TRADE_UNIT` of a good it
    /// holds above `STOCK_TARGET + TRADE_UNIT` to a partner that still wants
    /// it — breaking the both-must-give constraint behind the measured trade
    /// freeze (`docs/superpowers/specs/2026-08-02-trade-freeze-diagnosis.md`).
    /// Off by default ⇒ byte-identical bilateral-only trade.
    #[serde(default)]
    pub unilateral_trade: bool,
    /// Opt-in anthropogenic arms race: `culture_bearer`-tagged founder
    /// lineages become perceptible to wild agents as tool-bearing threats
    /// (new sensor + evolvable program node + the `Vigilance` gene's FEAR
    /// gain), and the `HuntedAdaptation` codex detector pairs the culture
    /// lineage's power rise against prey defensive-trait rises. `false`
    /// (default) keeps the world byte-identical.
    #[serde(default)]
    pub anthro_race_enabled: bool,
    /// Opt-in disease subsystem: crowding-seeded SIS pathogen — spillover in crowded
    /// populations, proximity spread, energy-drain mortality via the existing starve
    /// path; `EpidemicOutbreak`/`MedicineContainment` codex events. `false` (default) keeps the
    /// world byte-identical.
    #[serde(default)]
    pub disease_enabled: bool,
    /// Opt-in population cap override (`World::max_population`). Absent =
    /// `reproduce::MAX_POPULATION` (10k design budget). Tests pin this lower
    /// to keep long smoke runs fast.
    #[serde(default)]
    pub max_population: Option<u32>,
    /// Opt-in larger world. Absent = default 1024/128/64. All three should be
    /// set together and keep `world_size / hash_res ≈ 16` (the perception cap).
    #[serde(default)]
    pub world_size: Option<f32>,
    #[serde(default)]
    pub biome_res: Option<usize>,
    #[serde(default)]
    pub hash_res: Option<usize>,
    /// Opt-in codex-observer cadence (`World::codex_interval`). Absent/`1` =
    /// run the emergence detectors every tick (the default; bit-identical).
    /// `N > 1` runs them every N ticks, a throughput lever for long headless
    /// sweeps that care about aggregate outcomes more than per-tick emergence
    /// timing. See `World::codex_interval` for the behavioural caveat under
    /// `war_enabled`.
    #[serde(default)]
    pub codex_interval: Option<u64>,
    /// Opt-in climate knobs (`[climate]` table). Absent = the compile-time
    /// defaults, bit-identical to every pre-knob scenario. Any present field
    /// regenerates the biome field with that override (seed and RNG draw
    /// order unchanged). See `biome::ClimateParams`.
    #[serde(default)]
    pub climate: Option<ScenarioClimate>,
    /// Opt-in real-world biome source. Absent = the procedural climate
    /// pipeline (`climate`, if any, or the compile-time default). `Earth`
    /// takes precedence over `climate` and builds the field from the
    /// embedded 256x256 rasters via `BiomeField::from_earth` (requires
    /// `biome_res == biome::EARTH_RES`).
    #[serde(default)]
    pub world_map: Option<WorldMapSource>,
}

/// Opt-in real-world biome source. Absent = the procedural climate pipeline.
/// `Earth` builds the field from the embedded 256x256 rasters via
/// `BiomeField::from_earth` (requires `biome_res == biome::EARTH_RES`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldMapSource {
    Earth,
}

/// Scenario-facing climate overrides; every field optional, defaults from
/// `biome::ClimateParams::default()`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioClimate {
    /// Global temperature shift (negative = ice age, positive = hothouse).
    pub temp_bias: Option<f32>,
    /// Global moisture shift (negative = arid world, positive = lush world).
    pub moisture_bias: Option<f32>,
    /// Elevation cutoff for open water (higher = more ocean / archipelagos).
    pub sea_level: Option<f32>,
    /// Elevation distribution widening (higher = deeper basins + more peaks).
    pub elev_contrast: Option<f32>,
    /// Continent shaping in `[0,1]`: 0 = today's fBm speckle; >0 pulls land into
    /// a few large masses separated by ocean.
    #[serde(default)]
    pub continentality: Option<f32>,
    /// Ridged mountain uplift added to elevation on land: 0 = scattered peaks;
    /// >0 raises connected linear ranges.
    #[serde(default)]
    pub mountain_uplift: Option<f32>,
    /// Orographic rain-shadow strength: 0 = no drying; >0 dries cells downwind
    /// of higher terrain.
    #[serde(default)]
    pub rain_shadow: Option<f32>,
    /// Minimum flow-accumulation (in upstream-cell units) for a cell to become a
    /// river. 0 = hydrology off (no rivers, `river_flow` stays 0).
    #[serde(default)]
    pub river_threshold: Option<f32>,
}

impl ScenarioClimate {
    /// Resolve against the compile-time defaults.
    pub fn resolve(&self) -> crate::biome::ClimateParams {
        let d = crate::biome::ClimateParams::default();
        crate::biome::ClimateParams {
            temp_bias: self.temp_bias.unwrap_or(d.temp_bias),
            moisture_bias: self.moisture_bias.unwrap_or(d.moisture_bias),
            sea_level: self.sea_level.unwrap_or(d.sea_level),
            elev_contrast: self.elev_contrast.unwrap_or(d.elev_contrast),
            continentality: self.continentality.unwrap_or(d.continentality),
            mountain_uplift: self.mountain_uplift.unwrap_or(d.mountain_uplift),
            rain_shadow: self.rain_shadow.unwrap_or(d.rain_shadow),
            river_threshold: self.river_threshold.unwrap_or(d.river_threshold),
        }
    }
}

/// A request for `count` agents distributed via the given placement, each
/// initialized from the given trait overrides on top of a neutral genome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSpec {
    pub count: u32,
    #[serde(default)]
    pub placement: Placement,
    #[serde(default)]
    pub traits: TraitOverrides,
    /// Named founder archetype: pairs a module kit with a starter behavior
    /// program (and, for some, genome defaults — see `archetype_genome`), and
    /// gives the spec a fresh species id. Valid names: `stalker`,
    /// `pack_hunter`, `spiner`, `bruiser`, `sentinel`, `herd`, `marker`,
    /// `communicator`, `cooperator`, `cultural_cooperator`, `asocial_forager`,
    /// `culture_prey`, `asocial_prey`, `skilled_forager`, `fast_hunter`,
    /// `slow_hunter`, `innate_forager`, `individual_learner`, `pure_imitator`,
    /// `critical_learner`, `cultural_forager`, `omnivore_forager` (the
    /// diet-matched asocial control for `cultural_forager`), `innovator`,
    /// `traditionalist`,
    /// `grazer`, `ape_hunter` (armed culture-bearer for the anthropogenic
    /// arms race), and the vertebrate classes `mammal_grazer`,
    /// `mammal_pursuer`, `reptile_ambusher`, `reptile_basker`. Unknown names
    /// fall back to the grazer kit + program.
    #[serde(default)]
    pub archetype: Option<String>,
    /// Inventions this spec's agents already HOLD at tick 0, named by their
    /// machine key (e.g. `["stone_tools", "fire", "farming"]`; matching is
    /// case-insensitive). Seeds the corresponding invention meme channels to
    /// fully adopted so the lineage begins partway up the tech tree — used to
    /// let a full-scale scenario reach the era-3 milestones (Writing,
    /// Husbandry -> domestication) without the slow cold-start climb. Only
    /// meaningful with `inventions_enabled`. Absent (the default) seeds
    /// nothing, keeping the golden scenarios byte-identical.
    #[serde(default)]
    pub starting_inventions: Vec<String>,
    /// Anthropogenic arms race: tag this spec's founder species as
    /// culture-bearing ("human"). Wild agents can then perceive its members
    /// as tool-bearing threats, and the `HuntedAdaptation` detector pairs
    /// this lineage's power rise against prey adaptation. Speciation
    /// splinters inherit the tag through the lineage-root walk. Requires
    /// `anthro_race_enabled = true` (validated at parse).
    #[serde(default)]
    pub culture_bearer: bool,
}

/// Declares the scenario `[traits]` table: one line per override, binding the
/// TOML field name to the genome slot it pins. The struct and `apply` are both
/// generated from this single list, so adding an override is a one-line change
/// that cannot go half-wired — the previous hand-written struct/`apply` pair
/// let a new field parse from TOML and then silently never reach the genome.
macro_rules! trait_overrides {
    ($( $(#[$meta:meta])* $field:ident => $slot:ident ),* $(,)?) => {
        #[derive(Debug, Clone, Serialize, Deserialize, Default)]
        #[serde(deny_unknown_fields)]
        pub struct TraitOverrides {
            $( $(#[$meta])* pub $field: Option<f32>, )*
        }

        impl TraitOverrides {
            /// Pin every override that is present onto `g`; absent ones leave
            /// the archetype default / random draw untouched.
            pub fn apply(&self, g: &mut Genome) {
                $(
                    if let Some(v) = self.$field {
                        g.set(GenomeSlot::$slot, v);
                    }
                )*
            }
        }
    };
}

trait_overrides! {
    perception_radius => PerceptionRadius,
    size => Size,
    basal_metabolism => BasalMetabolism,
    lifespan_bias => LifespanBias,
    reproduction_threshold => ReproductionThreshold,
    /// Altruistic sharing drive (`GenomeSlot::Altruism`). Required for M15
    /// `starter_cooperator` scenarios; absent from all pre-M15 scenarios so
    /// the golden-tick hash is unaffected.
    altruism => Altruism,
    /// DIT env-mode genome propensities (experiment). `InnateTechnique` is the
    /// genetic strategy's fixed technique; `IndividualLearning`/`SocialLearning`
    /// (`> 0.5`) enable learning-by-doing / social copying of the technique.
    innate_technique => InnateTechnique,
    individual_learning => IndividualLearning,
    social_learning => SocialLearning,
    /// Big Five personality overrides (stored `[0,1]`; `0.5` = neutral/0.0
    /// signed). When present, they pin the slot instead of the random draw.
    openness => Openness,
    conscientiousness => Conscientiousness,
    extraversion => Extraversion,
    agreeableness => Agreeableness,
    neuroticism => Neuroticism,
    /// Preferred-terrain drive (`GenomeSlot::TerrainAffinity`); pairs with
    /// `World::terrain_habitat` (geographic trade routes).
    terrain_affinity => TerrainAffinity,
    /// E12 reproductive knobs (read only with `sexual_dimorphism_enabled`).
    sexual_dimorphism => SexualDimorphism,
    mate_choosiness => MateChoosiness,
    /// Heritable cognitive baseline (`GenomeSlot::CognitivePotential`; read
    /// only with `cognition_enabled`).
    cognitive_potential => CognitivePotential,
    /// Affect-layer temperament genes (read only with `affect_enabled`):
    /// Boldness scales the FEAR response down; Aggressiveness sets RAGE gain;
    /// Reactivity raises survival-reflex hijack sensitivity. Nurturance and
    /// Sociality are declared for the (not yet wired) CARE/PANIC/PLAY systems
    /// and count toward speciation distance.
    boldness => Boldness,
    aggressiveness => Aggressiveness,
    reactivity => Reactivity,
    nurturance => Nurturance,
    sociality => Sociality,
    /// Render-color genes (HSV); picked up by the Godot bridge.
    color_hue => ColorHue,
    color_sat => ColorSat,
    color_val => ColorVal,
    /// Climate-match feeding bonus (`GenomeSlot::EnvAffinity`; read only with
    /// `biome_adaptation`).
    env_affinity => EnvAffinity,
    /// Genome mutation sigma scale (`GenomeSlot::MutationRate`). Lower values
    /// slow lineage drift/speciation, keeping breeding pools coherent.
    mutation_rate => MutationRate,
    /// Heritable wariness (`GenomeSlot::Vigilance`; read only with
    /// `anthro_race_enabled`).
    vigilance => Vigilance,
    /// Heritable thirst tolerance (`GenomeSlot::ThirstTolerance`; read only
    /// with `basic_needs_enabled`).
    thirst_tolerance => ThirstTolerance,
    /// Heritable sleep need (`GenomeSlot::SleepNeed`; read only with
    /// `basic_needs_enabled`).
    sleep_need => SleepNeed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Placement {
    /// Uniform random across the world bounds.
    Uniform,
    /// Cluster around `center` within `radius`.
    Cluster { center_x: f32, center_y: f32, radius: f32 },
    /// Cluster around a real-world lat/lon (equirectangular → sim coords),
    /// spread within `radius`. For real-map (`world_map`) scenarios.
    Geo { lat: f32, lon: f32, radius: f32 },
}

#[allow(clippy::derivable_impls)]
impl Default for Placement {
    fn default() -> Self {
        Placement::Uniform
    }
}

/// Retune the kit's Mouth to the primate omnivore band so the lineage renders
/// as (and counts as) an ape — the only archetype allowed to carry inventions.
fn make_omnivore(modules: &mut crate::module::ModuleList) {
    for m in modules.iter_mut() {
        if let crate::module::Module::Mouth { diet_affinity, .. } = m {
            *diet_affinity = 0.5;
        }
    }
}

/// Resolve an archetype name to its starter program + module kit. Unknown
/// names fall back to the grazer defaults.
fn archetype_kit(name: &str) -> (crate::module::ModuleList, crate::program::Program) {
    use crate::module::{
        bruiser_kit, communicator_kit, fast_hunter_kit, mammal_grazer_kit, mammal_pursuer_kit,
        marker_kit, predator_kit, reptile_ambusher_kit, reptile_basker_kit, slow_hunter_kit,
        spiner_kit, starter_kit,
    };
    use crate::program::{
        starter_asocial_forager, starter_asocial_prey, starter_bruiser, starter_communicator,
        starter_cooperator, starter_cultural_cooperator, starter_cultural_hunter,
        starter_culture_prey, starter_grazer, starter_herd, starter_mammal_herd,
        starter_mammal_pursuer, starter_marker, starter_pack_hunter, starter_reptile_ambusher,
        starter_reptile_basker, starter_sentinel, starter_spiner, starter_stalker,
    };
    match name {
        "stalker" => (predator_kit(), starter_stalker()),
        "pack_hunter" => (predator_kit(), starter_pack_hunter()),
        "spiner" => (spiner_kit(), starter_spiner()),
        "bruiser" => (bruiser_kit(), starter_bruiser()),
        "sentinel" => (starter_kit(), starter_sentinel()),
        "herd" => (starter_kit(), starter_herd()),
        "marker" => (marker_kit(), starter_marker()),
        "communicator" => (communicator_kit(), starter_communicator()),
        "cooperator" => (starter_kit(), starter_cooperator()),
        "cultural_cooperator" => (communicator_kit(), starter_cultural_cooperator()),
        "asocial_forager" => (starter_kit(), starter_asocial_forager()),
        "culture_prey" => (communicator_kit(), starter_culture_prey()),
        "asocial_prey" => (starter_kit(), starter_asocial_prey()),
        "skilled_forager" => (communicator_kit(), starter_asocial_forager()),
        "fast_hunter" => (fast_hunter_kit(), starter_cultural_hunter()),
        "slow_hunter" => (slow_hunter_kit(), starter_cultural_hunter()),
        // DIT env-mode strategies (experiment): the genetic strategy carries no
        // Communicator; the three cultural strategies do and differ only by their
        // learning-propensity genome slots (see `archetype_genome`).
        "innate_forager" => (starter_kit(), starter_asocial_forager()),
        "individual_learner" => (communicator_kit(), starter_asocial_forager()),
        "pure_imitator" => (communicator_kit(), starter_asocial_forager()),
        "critical_learner" => (communicator_kit(), starter_asocial_forager()),
        // Living-sandbox coevolution (Task 3.1): the culture cohort, matched
        // against `asocial_forager` (the control) on everything except the
        // Communicator module. Deliberately NOT `communicator_kit()` — that
        // kit drops Reproductive, which would cripple the culture lineage's
        // reproduction and bias the experiment for the wrong reason.
        "cultural_forager" => {
            let mut m = starter_kit();
            make_omnivore(&mut m); // reclass to ape (primate) diet
            m.push(crate::module::Module::Communicator { range: 12.0, channel_id: 0 });
            (m, starter_asocial_forager())
        }
        // O3 diet-matched control: the omnivore forager WITHOUT the
        // Communicator. `cultural_forager` differs from this archetype by
        // culture alone, so invasion contrasts against it isolate the
        // Communicator (the ape reclass above made `asocial_forager` a
        // diet-confounded control — omnivory, not culture, separated them).
        "omnivore_forager" => {
            let mut m = starter_kit();
            make_omnivore(&mut m);
            (m, starter_asocial_forager())
        }
        // Invention-tree demo strategies: culture-bearing (starter_kit +
        // Communicator, keeping Reproductive so lineages persist across
        // generations) on the proven grazer foraging program; they differ
        // only by learning-propensity / personality genome slots (see
        // `archetype_genome`).
        "innovator" | "traditionalist" => {
            let mut m = starter_kit();
            make_omnivore(&mut m); // reclass to ape (primate) diet
            m.push(crate::module::Module::Communicator { range: 12.0, channel_id: 0 });
            (m, starter_grazer())
        }
        // Anthropogenic arms race: an armed culture-bearer — the ape body
        // plan (omnivore diet ⇒ invention-eligible) plus a Weapon for
        // hunting and a Communicator for the tech tree, keeping
        // Reproductive so the lineage establishes. The "human hunter".
        "ape_hunter" => {
            let mut m = starter_kit();
            make_omnivore(&mut m); // reclass to ape (primate) diet
            m.push(crate::module::Module::Weapon { damage: 6.0, energy_cost: 1.0 });
            m.push(crate::module::Module::Communicator { range: 12.0, channel_id: 0 });
            (m, starter_cultural_hunter())
        }
        // Vertebrate-class archetypes: mammals (endotherm-approximated — high
        // metabolism, social, cognitive) and reptiles (ectotherm-approximated —
        // low metabolism, armored, ambush/bask). See `archetype_genome` for the
        // temperament/cognition defaults that complete the class body plan.
        "mammal_grazer" => (mammal_grazer_kit(), starter_mammal_herd()),
        "mammal_pursuer" => (mammal_pursuer_kit(), starter_mammal_pursuer()),
        "reptile_ambusher" => (reptile_ambusher_kit(), starter_reptile_ambusher()),
        "reptile_basker" => (reptile_basker_kit(), starter_reptile_basker()),
        _ => (starter_kit(), starter_grazer()),
    }
}

/// Apply DIT env-mode genome defaults for the four strategy archetypes (applied
/// before scenario `traits`, so an explicit trait override still wins). No-op for
/// every other archetype, keeping non-DIT scenarios untouched.
fn archetype_genome(name: &str, g: &mut Genome) {
    match name {
        // Genetic strategy: a fixed innate technique (mid-range; evolves across
        // generations), no learning.
        "innate_forager" => g.set(GenomeSlot::InnateTechnique, 0.5),
        // Individual learner: learns by doing, does not copy.
        "individual_learner" => g.set(GenomeSlot::IndividualLearning, 1.0),
        // Pure imitator (Rogers variant): copies, never individually learns.
        "pure_imitator" => g.set(GenomeSlot::SocialLearning, 1.0),
        // Critical learner: both copies and individually corrects.
        "critical_learner" => {
            g.set(GenomeSlot::IndividualLearning, 1.0);
            g.set(GenomeSlot::SocialLearning, 1.0);
        }
        // Invention-tree demo strategies. Innovators: high Openness (novelty
        // drive feeds the discovery roll) and fast learners (skill growth
        // feeds it too). Traditionalists: low Openness, slow individual
        // learners — they rarely invent, but still adopt what neighbours
        // discover (the social spread in `culture_step` is gene-free).
        "innovator" => {
            g.set(GenomeSlot::Openness, 0.9);
            g.set(GenomeSlot::IndividualLearning, 1.0);
            g.set(GenomeSlot::SocialLearning, 1.0);
        }
        "traditionalist" => {
            g.set(GenomeSlot::Openness, 0.2);
            g.set(GenomeSlot::IndividualLearning, 0.2);
            g.set(GenomeSlot::SocialLearning, 0.8);
        }
        // Mammal class defaults: the endotherm profile — a high basal
        // metabolism (the warm-blooded tax) buying a big brain, sociality,
        // and boldness. Cognitive potential is high; temperament leans
        // affiliative (high Agreeableness/Extraversion) with a measured
        // threat response (mild Boldness, low Reactivity).
        "mammal_grazer" => {
            g.set(GenomeSlot::BasalMetabolism, 0.8);
            g.set(GenomeSlot::CognitivePotential, 0.8);
            g.set(GenomeSlot::IndividualLearning, 0.8);
            g.set(GenomeSlot::SocialLearning, 0.9);
            g.set(GenomeSlot::Extraversion, 0.8);
            g.set(GenomeSlot::Agreeableness, 0.7);
            g.set(GenomeSlot::Boldness, 0.65);
            g.set(GenomeSlot::Aggressiveness, 0.3);
            g.set(GenomeSlot::Reactivity, 0.45);
            g.set(GenomeSlot::Nurturance, 0.8);
            g.set(GenomeSlot::Sociality, 0.8);
            g.set(GenomeSlot::ColorHue, 0.08); // warm brown
        }
        "mammal_pursuer" => {
            g.set(GenomeSlot::BasalMetabolism, 0.65);
            g.set(GenomeSlot::CognitivePotential, 0.75);
            g.set(GenomeSlot::IndividualLearning, 0.7);
            g.set(GenomeSlot::SocialLearning, 0.85);
            g.set(GenomeSlot::Extraversion, 0.75);
            g.set(GenomeSlot::Agreeableness, 0.55);
            g.set(GenomeSlot::Boldness, 0.95);
            g.set(GenomeSlot::Aggressiveness, 0.75);
            g.set(GenomeSlot::Reactivity, 0.4);
            g.set(GenomeSlot::Nurturance, 0.6);
            g.set(GenomeSlot::Sociality, 0.75);
            g.set(GenomeSlot::ColorHue, 0.0); // red-brown
        }
        // Reptile class defaults: the ectotherm profile — low basal metabolism
        // (the cold-blooded edge: cheap idle, no internal furnace), modest
        // cognition, asocial temperament, and a hair-trigger affect layer
        // (high Reactivity → fast freeze/fight/flight hijack) with high
        // Aggressiveness for the ambush strike.
        "reptile_ambusher" => {
            g.set(GenomeSlot::BasalMetabolism, 0.2);
            g.set(GenomeSlot::CognitivePotential, 0.35);
            g.set(GenomeSlot::IndividualLearning, 0.3);
            g.set(GenomeSlot::SocialLearning, 0.2);
            g.set(GenomeSlot::Extraversion, 0.2);
            g.set(GenomeSlot::Agreeableness, 0.3);
            g.set(GenomeSlot::Boldness, 0.35);
            g.set(GenomeSlot::Aggressiveness, 0.8);
            g.set(GenomeSlot::Reactivity, 0.85);
            g.set(GenomeSlot::Nurturance, 0.15);
            g.set(GenomeSlot::Sociality, 0.2);
            g.set(GenomeSlot::ColorHue, 0.33); // scaled green
        }
        "reptile_basker" => {
            g.set(GenomeSlot::BasalMetabolism, 0.15);
            g.set(GenomeSlot::CognitivePotential, 0.3);
            g.set(GenomeSlot::IndividualLearning, 0.25);
            g.set(GenomeSlot::SocialLearning, 0.15);
            g.set(GenomeSlot::Extraversion, 0.2);
            g.set(GenomeSlot::Agreeableness, 0.45);
            g.set(GenomeSlot::Boldness, 0.3);
            g.set(GenomeSlot::Aggressiveness, 0.2);
            g.set(GenomeSlot::Reactivity, 0.7);
            g.set(GenomeSlot::Nurturance, 0.1);
            g.set(GenomeSlot::Sociality, 0.25);
            g.set(GenomeSlot::ColorHue, 0.25); // olive
        }
        _ => {}
    }
}

/// Build the fail-fast message for an unknown `starting_inventions` entry,
/// listing every valid key so the message can't go stale as the tree grows.
fn unknown_invention_msg(name: &str) -> String {
    let valid =
        crate::invention::INVENTIONS.iter().map(|inv| inv.key).collect::<Vec<_>>().join(", ");
    format!("unknown starting invention '{name}' — valid keys: {valid}")
}

#[derive(Debug, Error)]
pub enum ScenarioError {
    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("{0}")]
    UnknownInvention(String),
    #[error(
        "starting_inventions requires `inventions_enabled = true` — without the \
         invention tree the seeded meme channels are never read"
    )]
    InventionsDisabled,
    #[error(
        "knowledge_enabled requires `inventions_enabled = true` — knowledge accumulation \
         tracks Writing-holding cultures, which don't exist without the invention tree"
    )]
    KnowledgeNeedsInventions,
    #[error(
        "culture_bearer requires `anthro_race_enabled = true` — without the arms-race \
         subsystem the culture-lineage tag is never read"
    )]
    CultureBearerNeedsAnthroRace,
    #[error(
        "hash_res must be >= 3 (got {0}): the spatial-hash neighbour query walks a \
         3-cell ring, which aliases onto the same cells at a lower resolution and \
         double-counts neighbours"
    )]
    InvalidHashRes(usize),
}

impl Scenario {
    pub fn parse_toml(text: &str) -> Result<Self, ScenarioError> {
        let scenario: Self = toml::from_str(text)?;
        // Validate up front so an authoring mistake fails at load with a clear
        // message, rather than panicking deep inside `instantiate` (unknown
        // name) or silently seeding channels nothing reads (tree disabled).
        if !scenario.inventions_enabled
            && scenario.agents.iter().any(|s| !s.starting_inventions.is_empty())
        {
            return Err(ScenarioError::InventionsDisabled);
        }
        if scenario.knowledge_enabled && !scenario.inventions_enabled {
            return Err(ScenarioError::KnowledgeNeedsInventions);
        }
        if !scenario.anthro_race_enabled && scenario.agents.iter().any(|s| s.culture_bearer) {
            return Err(ScenarioError::CultureBearerNeedsAnthroRace);
        }
        for spec in &scenario.agents {
            for name in &spec.starting_inventions {
                if crate::invention::id_from_name(name).is_none() {
                    return Err(ScenarioError::UnknownInvention(unknown_invention_msg(name)));
                }
            }
        }
        if let Some(hr) = scenario.hash_res {
            if hr < 3 {
                return Err(ScenarioError::InvalidHashRes(hr));
            }
        }
        Ok(scenario)
    }

    /// Build a `World` from this scenario. Determinism: world.rng is seeded
    /// from `seed`; agent positions for `Placement::Uniform` come from this
    /// RNG in agent-id order.
    pub fn instantiate(&self) -> World {
        let mut w = match (self.world_size, self.biome_res, self.hash_res) {
            (None, None, None) => World::new(self.seed),
            (ws, br, hr) => World::with_dims(
                self.seed,
                ws.unwrap_or(crate::biome::WORLD_SIZE_DEFAULT),
                br.unwrap_or(crate::biome::BIOME_RES_DEFAULT),
                hr.unwrap_or(crate::spatial::HASH_RES_DEFAULT),
            ),
        };
        w.env_period = self.env_period;
        w.biome_adaptation = self.biome_adaptation;
        w.terrain_habitat = self.terrain_habitat;
        w.inventions_enabled = self.inventions_enabled;
        w.gene_tech_coupling = self.gene_tech_coupling;
        w.gene_requirements = self.gene_requirements;
        w.cognition_enabled = self.cognition_enabled;
        w.affect_enabled = self.affect_enabled;
        w.living_biome = self.living_biome;
        w.season_period = self.season_period;
        w.climate_drift_rate = self.climate_drift_rate;
        w.nutrient_variation = self.nutrient_variation;
        w.soil_fertility = self.soil_fertility;
        w.resources_enabled = self.resources_enabled;
        if w.resources_enabled {
            w.market_field = vec![0.0; w.biome.cells.len()];
        }
        w.conserve_goods_on_death = self.conserve_goods_on_death;
        w.war_enabled = self.war_enabled;
        w.settlement_enabled = self.settlement_enabled;
        w.sexual_dimorphism_enabled = self.sexual_dimorphism_enabled;
        w.domestication_enabled = self.domestication_enabled;
        w.agents.track_livestock = self.domestication_enabled;
        w.knowledge_enabled = self.knowledge_enabled;
        w.practices_enabled = self.practices_enabled;
        w.payoff_biased_learning = self.payoff_biased_learning;
        w.basic_needs_enabled = self.basic_needs_enabled;
        w.repro_biased_learning = self.repro_biased_learning;
        w.unilateral_trade = self.unilateral_trade;
        w.anthro_race_enabled = self.anthro_race_enabled;
        w.disease_enabled = self.disease_enabled;
        w.disasters_enabled = self.disasters_enabled;
        if w.disasters_enabled {
            w.disasters = crate::disaster::DisasterState::init(&mut w.rng);
        }
        if let Some(cap) = self.max_population {
            w.max_population = cap;
        }
        match &self.world_map {
            Some(WorldMapSource::Earth) => {
                w.biome = crate::biome::BiomeField::from_earth(w.biome_res, w.world_size);
            }
            None => {
                if let Some(climate) = &self.climate {
                    w.biome = crate::biome::BiomeField::generate_with(
                        self.seed,
                        w.biome_res,
                        w.world_size,
                        &climate.resolve(),
                    );
                }
            }
        }
        // Predetermined trade hubs: placed from the finalized biome once the
        // trade-goods subsystem is active. Must run AFTER the world_map match so
        // it sees the real (Earth or climate) biome, not the default one.
        if w.resources_enabled {
            w.trade_hubs = crate::hub::place_trade_hubs(&w.biome);
        }
        if let Some(interval) = self.codex_interval {
            w.codex_interval = interval;
        }
        // Personality is sampled from a DEDICATED rng substream (seeded from the
        // world seed) so it never perturbs `world.rng` — the physics/placement/
        // reproduction stream stays bit-identical to a personality-free build.
        // The trajectory then diverges only through actual personality-driven
        // behavior, not through init draw-shifting.
        let mut personality_rng = crate::rng::Rng::from_seed(self.seed ^ 0x9E37_79B9_7F4A_7C15);
        for spec in self.agents.iter() {
            // Each archetype spec gets a FRESH species id from `next_species_id`,
            // reserving species 0 strictly for archetype-free (legacy) specs.
            // (Using the spec index as the id would let an archetype at index 0
            // silently alias the default species 0.)
            let (species_id, kit) = match &spec.archetype {
                Some(name) => {
                    // Fresh species row for this archetype. Placeholder parent
                    // `Some(0)`; species_step overwrites it on the first
                    // reclustering (founder archetypes have no real parent).
                    let sid = w.push_species(Genome::neutral(), Some(0));
                    (sid, Some(archetype_kit(name)))
                }
                None => (0u32, None),
            };
            // Anthropogenic arms race: record the culture-bearer founder's
            // species id. Splinters inherit via the lineage-root walk, so
            // nothing per-member is stored.
            if spec.culture_bearer {
                w.culture_roots.insert(species_id);
            }
            // Resolve any starting inventions to meme channels once per spec.
            // `parse_toml` already rejects unknown names; this panic guards
            // programmatically-built scenarios that bypass parsing.
            let seed_channels: Vec<usize> = spec
                .starting_inventions
                .iter()
                .map(|name| {
                    let inv = crate::invention::id_from_name(name)
                        .unwrap_or_else(|| panic!("{}", unknown_invention_msg(name)));
                    crate::invention::channel(inv)
                })
                .collect();
            for _ in 0..spec.count {
                let position = match spec.placement {
                    Placement::Uniform => {
                        let x = w.rng.f32_range(0.0, w.world_size);
                        let y = w.rng.f32_range(0.0, w.world_size);
                        Vec2::new(x, y)
                    }
                    Placement::Cluster { center_x, center_y, radius } => {
                        let theta = w.rng.f32_range(0.0, std::f32::consts::TAU);
                        let r = w.rng.f32_range(0.0, radius);
                        Vec2::new(
                            center_x + r * crate::mathf::cosf(theta),
                            center_y + r * crate::mathf::sinf(theta),
                        )
                    }
                    Placement::Geo { lat, lon, radius } => {
                        let center_x = (lon + 180.0) / 360.0 * w.world_size;
                        let center_y = (90.0 - lat) / 180.0 * w.world_size;
                        let theta = w.rng.f32_range(0.0, std::f32::consts::TAU);
                        let r = w.rng.f32_range(0.0, radius);
                        Vec2::new(
                            center_x + r * crate::mathf::cosf(theta),
                            center_y + r * crate::mathf::sinf(theta),
                        )
                    }
                };
                let mut g = Genome::neutral();
                // Normally-distributed Big Five personality (heritable, evolves).
                // Sampled from the dedicated substream, before archetype/trait
                // overrides so an explicit personality override in a scenario
                // wins over the random draw.
                g.sample_personality_in_place(&mut personality_rng);
                if let Some(name) = &spec.archetype {
                    archetype_genome(name, &mut g);
                }
                spec.traits.apply(&mut g);
                let id = match &kit {
                    Some((modules, program)) => {
                        w.spawn_seeded(position, g, species_id, modules.clone(), program.clone())
                    }
                    None => w.spawn_agent(position, g),
                };
                // Seed held inventions by setting their meme channels to fully
                // adopted. No RNG is drawn, so an empty list (the default)
                // leaves the trajectory byte-identical.
                for &ch in &seed_channels {
                    w.agents.meme_vector[id as usize][ch] = 1.0;
                }
                // Apes-only inventions: a non-ape seed holds nothing. No-op for apes and
                // when the flag is off, so every existing scenario/golden stays byte-identical
                // (the shipped seeded scenarios seed the ape `innovator`).
                crate::invention::enforce_ape_only(
                    &mut w.agents.meme_vector[id as usize],
                    &w.agents.genome[id as usize],
                    &w.agents.modules[id as usize],
                    w.inventions_enabled,
                );
            }
        }
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_toml() {
        let text = r#"
name = "test"
seed = 42

[[agents]]
count = 10
placement = { kind = "uniform" }
[agents.traits]
size = 0.5
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        assert_eq!(s.name, "test");
        assert_eq!(s.seed, 42);
        assert_eq!(s.agents.len(), 1);
        assert_eq!(s.agents[0].count, 10);
        assert!(matches!(s.agents[0].placement, Placement::Uniform));
        assert_eq!(s.agents[0].traits.size, Some(0.5));
    }

    #[test]
    fn gene_tech_coupling_defaults_off_and_scenario_applies() {
        // Omitting the field leaves it off (serde default) for baseline identity.
        let base = r#"
name = "base"
seed = 1

[[agents]]
count = 5
[agents.traits]
"#;
        let s0 = Scenario::parse_toml(base).expect("parse");
        assert!(!s0.gene_tech_coupling);
        assert!(!s0.instantiate().gene_tech_coupling);
        // Setting it propagates into the instantiated world.
        let coupled = r#"
name = "coupled"
seed = 1
gene_tech_coupling = true

[[agents]]
count = 5
[agents.traits]
"#;
        let s1 = Scenario::parse_toml(coupled).expect("parse");
        assert!(s1.gene_tech_coupling);
        assert!(s1.instantiate().gene_tech_coupling);
    }

    #[test]
    fn gene_requirements_defaults_off_and_scenario_applies() {
        // Omitting the field leaves it off (serde default) for baseline identity.
        let base = r#"
name = "base"
seed = 1

[[agents]]
count = 5
[agents.traits]
"#;
        let s0 = Scenario::parse_toml(base).expect("parse");
        assert!(!s0.gene_requirements);
        assert!(!s0.instantiate().gene_requirements);
        // Setting it propagates into the instantiated world.
        let gated = r#"
name = "gated"
seed = 1
gene_requirements = true

[[agents]]
count = 5
[agents.traits]
"#;
        let s1 = Scenario::parse_toml(gated).expect("parse");
        assert!(s1.gene_requirements);
        assert!(s1.instantiate().gene_requirements);
    }

    #[test]
    fn affect_enabled_defaults_off_and_scenario_applies() {
        // Omitting the field leaves it off (serde default) for baseline identity.
        let base = "name = \"base\"\nseed = 1\n\n[[agents]]\ncount = 5\n[agents.traits]\n";
        let s0 = Scenario::parse_toml(base).expect("parse");
        assert!(!s0.affect_enabled);
        assert!(!s0.instantiate().affect_enabled);
        // Setting it propagates into the instantiated world.
        let on = "name = \"on\"\nseed = 1\naffect_enabled = true\n\n[[agents]]\ncount = 5\n[agents.traits]\n";
        let s1 = Scenario::parse_toml(on).expect("parse");
        assert!(s1.affect_enabled);
        assert!(s1.instantiate().affect_enabled);
    }

    #[test]
    fn instantiate_creates_requested_agents() {
        let text = r#"
name = "test"
seed = 1

[[agents]]
count = 25
[agents.traits]
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        let w = s.instantiate();
        assert_eq!(w.agents.live_count(), 25);
    }

    #[test]
    fn instantiate_applies_max_population_override() {
        let with_cap = Scenario::parse_toml("name = \"t\"\nseed = 1\nmax_population = 42\n")
            .expect("parse")
            .instantiate();
        assert_eq!(with_cap.max_population, 42);
        let without =
            Scenario::parse_toml("name = \"t\"\nseed = 1\n").expect("parse").instantiate();
        assert_eq!(without.max_population, crate::reproduce::MAX_POPULATION);
    }

    #[test]
    fn instantiation_is_deterministic() {
        let text = r#"
name = "test"
seed = 999

[[agents]]
count = 50
[agents.traits]
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        let a = s.instantiate();
        let b = s.instantiate();
        for id in a.agents.iter_alive() {
            assert_eq!(a.agents.position[id as usize], b.agents.position[id as usize]);
        }
    }

    #[test]
    fn marker_archetype_has_pheromone_and_smell_modules() {
        let text = r#"
name = "t"
seed = 1
[[agents]]
count = 5
archetype = "marker"
placement = { kind = "uniform" }
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        let w = s.instantiate();
        let id = w.agents.iter_alive().next().expect("one agent");
        let mods = &w.agents.modules[id as usize];
        assert!(crate::module::has(mods, crate::module::ModuleType::Pheromone));
        assert!(crate::module::has_smell(mods));
    }

    #[test]
    fn archetype_seeds_distinct_species_with_kits() {
        let text = r#"
name = "pp"
seed = 3

[[agents]]
count = 4
archetype = "grazer"
placement = { kind = "uniform" }

[[agents]]
count = 2
archetype = "stalker"
placement = { kind = "uniform" }
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        let w = s.instantiate();
        assert_eq!(w.agents.live_count(), 6);
        // Fresh ids reserve species 0 for the archetype-free path, so the two
        // archetype specs become species 1 (grazers) and species 2 (stalkers).
        let grazers =
            w.agents.iter_alive().filter(|&id| w.agents.species_id[id as usize] == 1).count();
        assert_eq!(grazers, 4, "grazer archetype forms species 1");
        let stalkers: Vec<u32> =
            w.agents.iter_alive().filter(|&id| w.agents.species_id[id as usize] == 2).collect();
        assert_eq!(stalkers.len(), 2, "stalker archetype forms species 2");
        // Stalkers carry a Weapon module (predator kit).
        for id in stalkers {
            assert!(
                crate::module::effective_weapon(&w.agents.modules[id as usize]).is_some(),
                "stalker has a Weapon"
            );
        }
    }

    #[test]
    fn resources_flag_parses_and_wires_into_world() {
        let text = r#"
name = "t"
seed = 1
resources_enabled = true
[[agents]]
count = 3
placement = { kind = "uniform" }
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        assert!(s.resources_enabled);
        let w = s.instantiate();
        assert!(w.resources_enabled);
        // Default (absent) stays false.
        let off = Scenario::parse_toml("name=\"t\"\nseed=1\n").expect("parse").instantiate();
        assert!(!off.resources_enabled);
    }

    #[test]
    fn terrain_habitat_flag_and_affinity_override_parse_and_wire_into_world() {
        let text = r#"
name = "t"
seed = 1
terrain_habitat = true
[[agents]]
count = 3
placement = { kind = "uniform" }
[agents.traits]
terrain_affinity = 0.87
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        assert!(s.terrain_habitat);
        assert_eq!(s.agents[0].traits.terrain_affinity, Some(0.87));
        let w = s.instantiate();
        assert!(w.terrain_habitat);
        let id = w.agents.iter_alive().next().expect("one agent");
        assert_eq!(w.agents.genome[id as usize].get(GenomeSlot::TerrainAffinity), 0.87);
        // Default (absent) stays false, and the genome slot stays untouched.
        let off = Scenario::parse_toml("name=\"t\"\nseed=1\n").expect("parse").instantiate();
        assert!(!off.terrain_habitat);
    }

    #[test]
    fn new_weapon_archetypes_resolve_to_their_kits() {
        use crate::module::{has, ModuleType};
        let (sp_mods, _) = archetype_kit("spiner");
        assert!(has(&sp_mods, ModuleType::Spines), "spiner archetype carries Spines");
        assert!(has(&sp_mods, ModuleType::Reproductive), "spiner lineage can establish");
        let (br_mods, _) = archetype_kit("bruiser");
        assert!(has(&br_mods, ModuleType::Jaws), "bruiser archetype carries Jaws");
        assert!(has(&br_mods, ModuleType::Armor), "bruiser archetype is armored");
        assert!(has(&br_mods, ModuleType::Reproductive), "bruiser lineage can establish");
    }

    #[test]
    fn documented_archetype_names_all_resolve() {
        // Keep `AgentSpec::archetype`'s doc comment honest: every name listed
        // there must resolve to a dedicated match arm — not silently fall back
        // to the grazer kit + program (the trap this guards: renaming a match
        // arm and leaving the doc/name dangling).
        let dedicated = [
            "stalker",
            "pack_hunter",
            "spiner",
            "bruiser",
            "sentinel",
            "herd",
            "marker",
            "communicator",
            "cooperator",
            "cultural_cooperator",
            "asocial_forager",
            "culture_prey",
            "asocial_prey",
            "skilled_forager",
            "fast_hunter",
            "slow_hunter",
            "innate_forager",
            "individual_learner",
            "pure_imitator",
            "critical_learner",
            "cultural_forager",
            "innovator",
            "traditionalist",
            "mammal_grazer",
            "mammal_pursuer",
            "reptile_ambusher",
            "reptile_basker",
        ];
        let fallback = archetype_kit("definitely-not-an-archetype");
        for name in dedicated {
            assert!(
                archetype_kit(name) != fallback,
                "documented archetype '{name}' resolves to the grazer fallback"
            );
        }
        // `grazer` is documented as the default and DOES resolve to the fallback.
        assert_eq!(archetype_kit("grazer"), fallback);
    }

    #[test]
    fn vertebrate_archetypes_resolve_to_their_kits() {
        use crate::module::{has, has_smell, ModuleType};
        let (mg_mods, _) = archetype_kit("mammal_grazer");
        assert!(has(&mg_mods, ModuleType::Communicator), "mammal grazer is cultural");
        assert!(has(&mg_mods, ModuleType::Reproductive), "mammal grazer breeds");
        assert!(!has(&mg_mods, ModuleType::Weapon), "mammal grazer is unarmed");
        let (mp_mods, _) = archetype_kit("mammal_pursuer");
        assert!(has(&mp_mods, ModuleType::Weapon), "mammal pursuer is armed");
        assert!(has(&mp_mods, ModuleType::Communicator), "mammal pursuer coordinates");
        assert!(has(&mp_mods, ModuleType::Reproductive), "pursuer lineage can establish");
        let (ra_mods, _) = archetype_kit("reptile_ambusher");
        assert!(has(&ra_mods, ModuleType::Jaws), "reptile ambusher carries Jaws");
        assert!(has(&ra_mods, ModuleType::Armor), "reptile ambusher is scaled");
        assert!(has_smell(&ra_mods), "reptile ambusher smells its prey");
        assert!(has(&ra_mods, ModuleType::Reproductive), "ambusher lineage can establish");
        let (rb_mods, _) = archetype_kit("reptile_basker");
        assert!(has(&rb_mods, ModuleType::Armor), "reptile basker is armored");
        assert!(!has(&rb_mods, ModuleType::Jaws), "reptile basker is harmless");
        assert!(!has(&rb_mods, ModuleType::Weapon), "reptile basker is harmless");
        assert!(has(&rb_mods, ModuleType::Reproductive), "basker lineage can establish");
    }

    #[test]
    fn vertebrate_archetypes_carry_class_genome_profiles() {
        let mut g = Genome::neutral();
        archetype_genome("mammal_grazer", &mut g);
        assert_eq!(g.get(GenomeSlot::BasalMetabolism), 0.8, "endotherm tax");
        assert_eq!(g.get(GenomeSlot::CognitivePotential), 0.8, "big-brained");
        assert_eq!(g.get(GenomeSlot::Nurturance), 0.8, "parental investment");
        let mut g = Genome::neutral();
        archetype_genome("reptile_ambusher", &mut g);
        assert_eq!(g.get(GenomeSlot::BasalMetabolism), 0.2, "ectotherm edge");
        assert_eq!(g.get(GenomeSlot::Reactivity), 0.85, "hair-trigger hijack");
        assert_eq!(g.get(GenomeSlot::Aggressiveness), 0.8, "ambush strike");
    }

    #[test]
    fn trait_overrides_cover_affect_cognition_and_color_genes() {
        let text = r#"
name = "t"
seed = 1
affect_enabled = true
cognition_enabled = true
[[agents]]
count = 2
archetype = "reptile_ambusher"
placement = { kind = "uniform" }
[agents.traits]
boldness = 0.9
reactivity = 0.1
cognitive_potential = 0.77
color_hue = 0.5
env_affinity = 0.42
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        let w = s.instantiate();
        let id = w.agents.iter_alive().next().expect("one agent");
        let g = &w.agents.genome[id as usize];
        // Explicit trait overrides win over the archetype genome defaults
        // (reptile_ambusher would otherwise pin Reactivity to 0.85).
        assert_eq!(g.get(GenomeSlot::Boldness), 0.9);
        assert_eq!(g.get(GenomeSlot::Reactivity), 0.1, "override beats archetype default");
        assert_eq!(g.get(GenomeSlot::CognitivePotential), 0.77);
        assert_eq!(g.get(GenomeSlot::ColorHue), 0.5);
        assert_eq!(g.get(GenomeSlot::EnvAffinity), 0.42);
        // Untouched archetype defaults still apply.
        assert_eq!(g.get(GenomeSlot::BasalMetabolism), 0.2, "class default survives");
    }

    #[test]
    fn starting_inventions_seed_named_techs_and_leave_others_unheld() {
        use crate::invention::{has, FARMING, FIRE, STONE_TOOLS, WRITING};
        let text = r#"
name = "t"
seed = 1
inventions_enabled = true
[[agents]]
count = 1
archetype = "innovator"
starting_inventions = ["stone_tools", "fire", "farming"]
placement = { kind = "cluster", center_x = 100.0, center_y = 100.0, radius = 1.0 }
"#;
        let w = Scenario::parse_toml(text).expect("parse").instantiate();
        let id = w.agents.iter_alive().next().expect("one agent");
        let meme = &w.agents.meme_vector[id as usize];
        assert!(has(meme, STONE_TOOLS), "seeded Stone Tools is held at tick 0");
        assert!(has(meme, FIRE), "seeded Fire is held at tick 0");
        assert!(has(meme, FARMING), "seeded Farming is held at tick 0");
        assert!(!has(meme, WRITING), "unseeded Writing is NOT held");

        // Absent field => nothing seeded: the determinism-safe default that
        // keeps the golden (minimal) scenario byte-identical.
        let off = Scenario::parse_toml(
            "name=\"t\"\nseed=1\n[[agents]]\ncount=1\nplacement = { kind = \"uniform\" }\n",
        )
        .expect("parse")
        .instantiate();
        let oid = off.agents.iter_alive().next().expect("one agent");
        assert!(!has(&off.agents.meme_vector[oid as usize], STONE_TOOLS));
    }

    #[test]
    fn non_ape_starting_inventions_are_stripped() {
        use crate::invention::{has, STONE_TOOLS};
        let text = r#"
name = "t"
seed = 1
inventions_enabled = true
[[agents]]
count = 1
archetype = "asocial_forager"
starting_inventions = ["stone_tools"]
placement = { kind = "cluster", center_x = 100.0, center_y = 100.0, radius = 1.0 }
"#;
        let w = Scenario::parse_toml(text).expect("parse").instantiate();
        let id = w.agents.iter_alive().next().expect("one agent");
        assert!(
            !has(&w.agents.meme_vector[id as usize], STONE_TOOLS),
            "a non-ape (herbivore) must not hold a seeded invention"
        );
    }

    #[test]
    fn parse_toml_rejects_unknown_starting_invention() {
        let text = r#"
name = "t"
seed = 1
inventions_enabled = true
[[agents]]
count = 1
starting_inventions = ["stone_tools", "wheel"]
placement = { kind = "uniform" }
"#;
        let err = Scenario::parse_toml(text).expect_err("unknown invention must be rejected");
        let msg = err.to_string();
        assert!(msg.contains("wheel"), "error should name the bad invention, got: {msg}");
        assert!(msg.contains("husbandry"), "error should list valid keys, got: {msg}");
    }

    #[test]
    fn parse_toml_rejects_starting_inventions_with_tree_disabled() {
        // Seeding without `inventions_enabled` would silently write meme
        // channels nothing reads — reject so the author fixes the flag.
        let text = r#"
name = "t"
seed = 1
[[agents]]
count = 1
starting_inventions = ["stone_tools"]
placement = { kind = "uniform" }
"#;
        let err = Scenario::parse_toml(text).expect_err("disabled tree must be rejected");
        assert!(
            err.to_string().contains("inventions_enabled"),
            "error should name the missing flag, got: {err}"
        );
    }

    #[test]
    fn codex_interval_wires_into_world_and_is_observer_only_with_war_off() {
        use crate::snapshot::state_hash;
        use crate::tick::step;
        // Absent => every tick (World default 1).
        let base = "name=\"t\"\nseed=5\n[[agents]]\ncount=40\nplacement={kind=\"uniform\"}\n";
        let mut w1 = Scenario::parse_toml(base).expect("parse").instantiate();
        assert_eq!(w1.codex_interval, 1, "absent codex_interval => every tick");
        // Set a coarse cadence.
        let cadenced = format!("codex_interval=7\n{base}");
        let mut w7 = Scenario::parse_toml(&cadenced).expect("parse").instantiate();
        assert_eq!(w7.codex_interval, 7);
        for _ in 0..200 {
            step(&mut w1);
            step(&mut w7);
        }
        // War is off here, so the codex is a pure observer: cadencing it must
        // leave every agent trajectory identical (nothing feeds the codex back
        // into the sim when `war_enabled` is false).
        let ids: Vec<u32> = w1.agents.iter_alive().collect();
        assert_eq!(ids, w7.agents.iter_alive().collect::<Vec<u32>>(), "alive set diverged");
        for id in &ids {
            assert_eq!(
                w1.agents.position[*id as usize], w7.agents.position[*id as usize],
                "agent {id} trajectory changed by codex cadence (war off)"
            );
        }
        // …but the recorded codex genuinely differs (fewer observations), so the
        // whole-world hashes differ — proving the knob actually skips work.
        assert_ne!(state_hash(&w1), state_hash(&w7), "cadence should change the codex log");
    }

    #[test]
    fn parse_toml_rejects_unknown_top_level_key() {
        // A misspelled feature flag must fail at load rather than silently
        // leaving the feature off (the whole point of deny_unknown_fields).
        let text = "name = \"t\"\nseed = 1\ninventions_enable = true\n";
        let err = Scenario::parse_toml(text).expect_err("typo'd flag must be rejected");
        assert!(
            err.to_string().contains("inventions_enable"),
            "error should name the unknown key, got: {err}"
        );
    }

    #[test]
    fn parse_toml_rejects_unknown_trait_key() {
        // Unknown keys in nested tables (traits) are rejected too.
        let text = r#"
name = "t"
seed = 1
[[agents]]
count = 1
placement = { kind = "uniform" }
[agents.traits]
sixe = 0.5
"#;
        let err = Scenario::parse_toml(text).expect_err("typo'd trait must be rejected");
        assert!(err.to_string().contains("sixe"), "error should name the unknown key, got: {err}");
    }

    #[test]
    fn parse_toml_rejects_hash_res_below_three() {
        // hash_res < 3 makes the spatial-hash ring alias and double-count
        // neighbours; reject at load rather than silently mis-simulate in release.
        for bad in [0usize, 1, 2] {
            let text = format!("name = \"t\"\nseed = 1\nhash_res = {bad}\n");
            let err = Scenario::parse_toml(&text).expect_err("hash_res < 3 must be rejected");
            assert!(err.to_string().contains("hash_res"), "error should name hash_res, got: {err}");
        }
        // 3 and up are accepted.
        assert!(Scenario::parse_toml("name = \"t\"\nseed = 1\nhash_res = 3\n").is_ok());
    }

    #[test]
    fn starting_inventions_do_not_perturb_other_agents() {
        // Seeding spec 0 must not shift the placement/personality RNG streams,
        // so every agent (seeded or not) lands identically vs. an unseeded run.
        let base = r#"
name = "t"
seed = 7
inventions_enabled = true
[[agents]]
count = 3
archetype = "innovator"
placement = { kind = "cluster", center_x = 100.0, center_y = 100.0, radius = 5.0 }
[[agents]]
count = 3
archetype = "grazer"
placement = { kind = "cluster", center_x = 300.0, center_y = 300.0, radius = 5.0 }
"#;
        let seeded = base.replace(
            "center_x = 100.0, center_y = 100.0, radius = 5.0 }\n",
            "center_x = 100.0, center_y = 100.0, radius = 5.0 }\nstarting_inventions = [\"stone_tools\", \"fire\"]\n",
        );
        let a = Scenario::parse_toml(base).expect("parse").instantiate();
        let b = Scenario::parse_toml(&seeded).expect("parse").instantiate();
        let ids: Vec<u32> = a.agents.iter_alive().collect();
        assert_eq!(ids, b.agents.iter_alive().collect::<Vec<u32>>());
        for id in ids {
            assert_eq!(
                a.agents.position[id as usize], b.agents.position[id as usize],
                "agent {id} position unchanged by seeding another spec"
            );
            assert_eq!(
                a.agents.genome[id as usize], b.agents.genome[id as usize],
                "agent {id} genome unchanged by seeding another spec"
            );
        }
    }

    #[test]
    fn culture_cohort_archetypes_are_apes() {
        use crate::genome::Genome;
        // Default archetype genome (Size 0.5 = large); diet comes from the kit's Mouth.
        for name in ["innovator", "traditionalist", "cultural_forager"] {
            let (modules, _prog) = archetype_kit(name);
            let mut g = Genome::neutral();
            archetype_genome(name, &mut g);
            assert!(
                crate::invention::is_ape(&g, &modules),
                "{name} must be an ape (omnivore + large) so it can carry inventions"
            );
        }
        // Control: the asocial forager stays a non-ape herbivore.
        let (modules, _) = archetype_kit("asocial_forager");
        let mut g = Genome::neutral();
        archetype_genome("asocial_forager", &mut g);
        assert!(!crate::invention::is_ape(&g, &modules), "asocial_forager stays non-ape");
    }
}
