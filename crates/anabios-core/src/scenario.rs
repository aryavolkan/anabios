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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TraitOverrides {
    pub perception_radius: Option<f32>,
    pub size: Option<f32>,
    pub basal_metabolism: Option<f32>,
    pub lifespan_bias: Option<f32>,
    pub reproduction_threshold: Option<f32>,
    /// Altruistic sharing drive (`GenomeSlot::Altruism`). Required for M15
    /// `starter_cooperator` scenarios; absent from all pre-M15 scenarios so
    /// the golden-tick hash is unaffected.
    pub altruism: Option<f32>,
    /// DIT env-mode genome propensities (experiment). `InnateTechnique` is the
    /// genetic strategy's fixed technique; `IndividualLearning`/`SocialLearning`
    /// (`> 0.5`) enable learning-by-doing / social copying of the technique.
    pub innate_technique: Option<f32>,
    pub individual_learning: Option<f32>,
    pub social_learning: Option<f32>,
    /// Big Five personality overrides (stored `[0,1]`; `0.5` = neutral/0.0
    /// signed). When present, they pin the slot instead of the random draw.
    pub openness: Option<f32>,
    pub conscientiousness: Option<f32>,
    pub extraversion: Option<f32>,
    pub agreeableness: Option<f32>,
    pub neuroticism: Option<f32>,
    /// Preferred-terrain drive (`GenomeSlot::TerrainAffinity`); pairs with
    /// `World::terrain_habitat` (geographic trade routes).
    pub terrain_affinity: Option<f32>,
    /// E12 reproductive knobs (read only with `sexual_dimorphism_enabled`).
    pub sexual_dimorphism: Option<f32>,
    pub mate_choosiness: Option<f32>,
}

impl TraitOverrides {
    pub fn apply(&self, g: &mut Genome) {
        if let Some(v) = self.perception_radius {
            g.set(GenomeSlot::PerceptionRadius, v);
        }
        if let Some(v) = self.size {
            g.set(GenomeSlot::Size, v);
        }
        if let Some(v) = self.basal_metabolism {
            g.set(GenomeSlot::BasalMetabolism, v);
        }
        if let Some(v) = self.lifespan_bias {
            g.set(GenomeSlot::LifespanBias, v);
        }
        if let Some(v) = self.reproduction_threshold {
            g.set(GenomeSlot::ReproductionThreshold, v);
        }
        if let Some(v) = self.altruism {
            g.set(GenomeSlot::Altruism, v);
        }
        if let Some(v) = self.innate_technique {
            g.set(GenomeSlot::InnateTechnique, v);
        }
        if let Some(v) = self.individual_learning {
            g.set(GenomeSlot::IndividualLearning, v);
        }
        if let Some(v) = self.social_learning {
            g.set(GenomeSlot::SocialLearning, v);
        }
        if let Some(v) = self.openness {
            g.set(GenomeSlot::Openness, v);
        }
        if let Some(v) = self.conscientiousness {
            g.set(GenomeSlot::Conscientiousness, v);
        }
        if let Some(v) = self.extraversion {
            g.set(GenomeSlot::Extraversion, v);
        }
        if let Some(v) = self.agreeableness {
            g.set(GenomeSlot::Agreeableness, v);
        }
        if let Some(v) = self.neuroticism {
            g.set(GenomeSlot::Neuroticism, v);
        }
        if let Some(v) = self.terrain_affinity {
            g.set(GenomeSlot::TerrainAffinity, v);
        }
        if let Some(v) = self.sexual_dimorphism {
            g.set(GenomeSlot::SexualDimorphism, v);
        }
        if let Some(v) = self.mate_choosiness {
            g.set(GenomeSlot::MateChoosiness, v);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Placement {
    /// Uniform random across the world bounds.
    Uniform,
    /// Cluster around `center` within `radius`.
    Cluster { center_x: f32, center_y: f32, radius: f32 },
}

#[allow(clippy::derivable_impls)]
impl Default for Placement {
    fn default() -> Self {
        Placement::Uniform
    }
}

/// Resolve an archetype name to its starter program + module kit. Unknown
/// names fall back to the grazer defaults.
fn archetype_kit(name: &str) -> (crate::module::ModuleList, crate::program::Program) {
    use crate::module::{
        bruiser_kit, communicator_kit, fast_hunter_kit, marker_kit, predator_kit, slow_hunter_kit,
        spiner_kit, starter_kit,
    };
    use crate::program::{
        starter_asocial_forager, starter_asocial_prey, starter_bruiser, starter_communicator,
        starter_cooperator, starter_cultural_cooperator, starter_cultural_hunter,
        starter_culture_prey, starter_grazer, starter_herd, starter_marker, starter_pack_hunter,
        starter_sentinel, starter_spiner, starter_stalker,
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
            m.push(crate::module::Module::Communicator { range: 12.0, channel_id: 0 });
            (m, starter_asocial_forager())
        }
        // Invention-tree demo strategies: culture-bearing (starter_kit +
        // Communicator, keeping Reproductive so lineages persist across
        // generations) on the proven grazer foraging program; they differ
        // only by learning-propensity / personality genome slots (see
        // `archetype_genome`).
        "innovator" | "traditionalist" => {
            let mut m = starter_kit();
            m.push(crate::module::Module::Communicator { range: 12.0, channel_id: 0 });
            (m, starter_grazer())
        }
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
        w.practices_enabled = self.practices_enabled;
        w.disasters_enabled = self.disasters_enabled;
        if w.disasters_enabled {
            w.disasters = crate::disaster::DisasterState::init(&mut w.rng);
        }
        if let Some(cap) = self.max_population {
            w.max_population = cap;
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
                    let sid = w.next_species_id;
                    // Grow the species tables for this id (spawn_seeded's
                    // add_to_species only grows the member-count vec).
                    while w.species_centroids.len() <= sid as usize {
                        w.species_centroids.push(Genome::neutral());
                        // Placeholder parent; species_step overwrites on the
                        // first reclustering. Founder archetypes have no real
                        // parent species.
                        w.species_parents.push(Some(0));
                        w.species_member_counts.push(0);
                    }
                    w.next_species_id = sid + 1;
                    (sid, Some(archetype_kit(name)))
                }
                None => (0u32, None),
            };
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
}
