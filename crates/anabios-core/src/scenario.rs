//! Scenario initial conditions, loadable from TOML.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;
use crate::world::World;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Opt-in: enable the biome-trade-goods economy (resource nodes spawn,
    /// agents harvest and trade them, reproduction needs a dowry basket).
    /// `false` (default) leaves the world unchanged.
    #[serde(default)]
    pub resources_enabled: bool,
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
}

/// A request for `count` agents distributed via the given placement, each
/// initialized from the given trait overrides on top of a neutral genome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub count: u32,
    #[serde(default)]
    pub placement: Placement,
    #[serde(default)]
    pub traits: TraitOverrides,
    #[serde(default)]
    pub archetype: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

#[derive(Debug, Error)]
pub enum ScenarioError {
    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

impl Scenario {
    pub fn parse_toml(text: &str) -> Result<Self, ScenarioError> {
        Ok(toml::from_str(text)?)
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
        w.cognition_enabled = self.cognition_enabled;
        w.living_biome = self.living_biome;
        w.season_period = self.season_period;
        w.resources_enabled = self.resources_enabled;
        if let Some(cap) = self.max_population {
            w.max_population = cap;
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
                match &kit {
                    Some((modules, program)) => {
                        w.spawn_seeded(position, g, species_id, modules.clone(), program.clone());
                    }
                    None => {
                        w.spawn_agent(position, g);
                    }
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
}
