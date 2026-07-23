//! `World` is the root state object owned by every simulation. It carries
//! the RNG, biome field, agent buffers, spatial hash, and tick counter.
//! Nothing outside this struct holds simulation state.

use bitvec::vec::BitVec;
use serde::{Deserialize, Serialize};

use crate::agent::{AgentBuffers, AgentId, LineageId, LINEAGE_NONE};
use crate::biome::BiomeField;
use crate::genome::Genome;
use crate::prelude::Vec2;
use crate::rng::Rng;
use crate::spatial::UniformSpatialHash;

/// World root struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub tick: u64,
    pub seed: u64,
    pub rng: Rng,
    pub biome: BiomeField,
    pub agents: AgentBuffers,
    /// Next lineage id to allocate. Monotonically increasing.
    /// Lineage id 0 is reserved as `LINEAGE_NONE` (no parent).
    pub next_lineage_id: LineageId,
    /// Per-species mean genome. Indexed by `SpeciesId`. Empty entries
    /// (extinct species) are kept in place so existing ids stay stable;
    /// `species_member_counts[id] == 0` marks them.
    pub species_centroids: Vec<crate::genome::Genome>,
    /// Per-species live member count. Tracked incrementally by
    /// `World::add_to_species` / `remove_from_species` on every spawn,
    /// kill, and `species_step` reassignment, so it is authoritative
    /// outside of `species_step` itself.
    pub species_member_counts: Vec<u32>,
    /// Parent species id for each species. `None` for founder species
    /// (initially only species 0). Indexed by `SpeciesId`.
    pub species_parents: Vec<Option<u32>>,
    /// Next species id to allocate.
    pub next_species_id: u32,
    /// Codex event bus + per-detector scratch. Part of the deterministic
    /// snapshot (not `#[serde(skip)]`).
    pub codex: crate::codex::CodexState,
    /// Dead-but-edible flesh left by deaths this run; scavenged by carnivores.
    pub carcasses: Vec<crate::carcass::Carcass>,
    /// Per-channel pheromone grids (deposited in `interact`, decayed each tick).
    pub pheromones: crate::pheromone::PheromoneField,
    /// DIT environmental-variability period (experiment). `0` = mechanism OFF
    /// (all pre-existing scenarios). `> 0` enables the gene-culture technique
    /// mechanism; `culture::ENV_STATIC_PERIOD` means active-but-static.
    /// `#[serde(default)]` helps self-describing formats only — bincode
    /// snapshots from before this field was added are rejected by the
    /// `FORMAT_VERSION` gate (see `snapshot.rs`).
    #[serde(default)]
    pub env_period: u32,
    /// When true, the biome-adaptation feeding bonus (EnvAffinity vs local
    /// climate) is active. Off by default; opt-in per scenario. Same
    /// bincode/`FORMAT_VERSION` caveat as `env_period`.
    #[serde(default)]
    pub biome_adaptation: bool,
    /// When true, agents are pulled toward their `TerrainAffinity` preferred
    /// terrain (terrain-based habitat selection), so species sort into biomes
    /// and trade at borders. Off by default; opt-in per scenario. Same
    /// bincode/`FORMAT_VERSION` caveat as `biome_adaptation`.
    #[serde(default)]
    pub terrain_habitat: bool,
    /// When true, the cultural invention tree is active: Communicator agents
    /// discover inventions (innovation roll) and copy them socially, with
    /// per-holder buffs/debuffs. Off by default; opt-in per scenario.
    /// Defaulted so old snapshots without this field still deserialize.
    #[serde(default)]
    pub inventions_enabled: bool,
    /// When true, the cognitive layer is active: each agent develops a realized
    /// IQ (heritable `CognitivePotential` × juvenile nutrition/social enrichment,
    /// `iq.rs`) that costs basal metabolism and (Phase 2+) gates meme
    /// acquisition. Off by default; opt-in per scenario. When false, IQ stays
    /// `0.0` for every agent, so the metabolic multiplier is exact identity.
    #[serde(default)]
    pub cognition_enabled: bool,
    /// When true, depleted biome cells recolonize from vegetated neighbours
    /// each biome step (`BiomeField::recolonize_step`), before regrowth. Off
    /// by default; opt-in per scenario. Defaulted so old snapshots without
    /// this field still deserialize.
    #[serde(default)]
    pub living_biome: bool,
    /// Season cycle length in ticks (half-period of `biome::season_phase`'s
    /// triangle wave). `0` = seasonal regrowth OFF (plain `regrow_step` runs
    /// unconditionally). `> 0` opts a scenario into a migrating productive
    /// band. Defaulted so old snapshots without this field still deserialize.
    #[serde(default)]
    pub season_period: u32,
    /// Discrete trade-good nodes on the map (biome trade goods feature).
    /// Empty and inert unless `resources_enabled`. Serialized.
    #[serde(default)]
    pub resources: Vec<crate::resource::Resource>,
    /// When true, the biome-trade-goods economy is active: nodes spawn, agents
    /// harvest and trade them, and reproduction requires a dowry basket. Off by
    /// default; opt-in per scenario. Draws zero RNG and changes no state when off.
    #[serde(default)]
    pub resources_enabled: bool,
    /// When true, the disaster scheduler is active: fire/drought/freeze
    /// disasters strike on a Poisson schedule and scar the biome into
    /// succession states (`disaster.rs`). Off by default; opt-in per
    /// scenario; a no-op (zero RNG draws) when off.
    #[serde(default)]
    pub disasters_enabled: bool,
    /// When true, `SenseHostility` joins the program structural-mutation
    /// pool (E7) so war-reactive behavior can evolve. Off by default:
    /// baseline pools are byte-identical with the flag off. The hostility
    /// record itself (and its detectors) is always on.
    #[serde(default)]
    pub war_enabled: bool,
    /// Disaster scheduler + active disasters + succession sites. Inert
    /// unless `disasters_enabled`. Serialized.
    #[serde(default)]
    pub disasters: crate::disaster::DisasterState,
    /// Hard cap on alive agents; `reproduce_all` skips mating at/above this.
    /// Defaults to `reproduce::MAX_POPULATION` (the design's 10k budget);
    /// scenarios/tests can pin it lower. Same bincode/`FORMAT_VERSION` caveat
    /// as `env_period`.
    #[serde(default = "default_max_population")]
    pub max_population: u32,
    /// World extent per axis (torus size). Defaults to `WORLD_SIZE_DEFAULT`
    /// (1024). Larger values opt a scenario into a bigger sandbox. Defaulted
    /// so old snapshots without this field still deserialize.
    #[serde(default = "default_world_size")]
    pub world_size: f32,
    /// Biome grid resolution per axis. Defaults to `BIOME_RES_DEFAULT` (128).
    #[serde(default = "default_biome_res")]
    pub biome_res: usize,
    /// Spatial-hash resolution per axis. Defaults to `HASH_RES_DEFAULT` (64).
    /// Kept so `world_size / hash_res` (the hash cell size, == perception cap)
    /// stays ~16 when the world scales.
    #[serde(default = "default_hash_res")]
    pub hash_res: usize,
    #[serde(skip)]
    pub spatial: UniformSpatialHash,
    /// Spatial hash over `carcasses` (indexed by carcass index), rebuilt each
    /// tick in `scavenge_pass` so carnivores don't linearly scan every carcass.
    #[serde(skip)]
    pub carcass_spatial: UniformSpatialHash,
    /// Spatial hash over `resources` (indexed by node index), rebuilt each
    /// tick in `harvest_pass`. `#[serde(skip)]` — reconstructed on load.
    #[serde(skip)]
    pub resource_spatial: UniformSpatialHash,
    #[serde(skip)]
    pub sensors: Vec<crate::sense::SensorRegister>,
    #[serde(skip)]
    pub desired_direction: Vec<crate::prelude::Vec2>,
    /// Per-agent action register from `decide()`. Scratch, recomputed each
    /// tick. Consumed by `interact` starting in M12.
    #[serde(skip)]
    pub actions: Vec<crate::program::ActionRegister>,
    /// Per-tick per-species aggregates shared by the codex detectors; rebuilt
    /// at the top of every `observe_all`. Reused across ticks (take/restore).
    #[serde(skip)]
    pub(crate) codex_agg: crate::codex::SpeciesAggTable,
    /// Per-agent BitVec marking who has already mated this tick.
    /// Cleared at the start of `reproduce_all`.
    #[serde(skip)]
    pub reproduced_this_tick: BitVec,
    /// Per-tick combat attribution scratch (reset each tick in `interact_all`).
    /// `combat_damaged[t]` is set when slot `t` takes combat damage; read by
    /// `age_and_starve` / the codex detectors to attribute deaths.
    #[serde(skip)]
    pub combat_damaged: Vec<bool>,
    /// Attacker species id for each combat-damaged slot (valid only where
    /// `combat_damaged[t]` is true this tick).
    #[serde(skip)]
    pub combat_attacker: Vec<u32>,
    /// Per-tick combat streak buffer for the viewer: `(attacker_pos,
    /// target_pos, attacker_hue)` records by `combat_pass` and cleared at
    /// the start of the next `interact_all`. The hue is the attacker's
    /// genome `ColorHue` slot, so streaks tint to match the attacker's body
    /// color. Scratch only — never read by the simulation, so it is skipped
    /// by serialization like the other per-tick combat buffers.
    #[serde(skip)]
    pub combat_streaks: Vec<(crate::prelude::Vec2, crate::prelude::Vec2, f32)>,
    /// Per-tick trade route buffer for the viewer: `(trader_pos,
    /// partner_pos, trader_hue)` records pushed by `trade_pass` and cleared at
    /// the start of the next `interact_all`. The hue is the initiating
    /// trader's genome `ColorHue` slot, so routes tint to match the trader's
    /// body color. Scratch only — never read by the simulation, so it is
    /// skipped by serialization like the other per-tick buffers.
    #[serde(skip)]
    pub trade_routes: Vec<(crate::prelude::Vec2, crate::prelude::Vec2, f32)>,
    /// Consecutive ticks each agent has been below the still-speed
    /// threshold (E6 ambush instrumentation). Updated after integrate, read
    /// by `combat_pass`. Observability only — skipped by serialization.
    #[serde(skip)]
    pub still_ticks: Vec<u32>,
    /// Last tick's `desired_direction` per agent (E6 signaling: a response
    /// is a receiver STEERING toward the caller, i.e. alignment improving
    /// tick-over-tick). Observability only — skipped by serialization.
    #[serde(skip)]
    pub prev_desired_direction: Vec<crate::prelude::Vec2>,
    /// Cumulative count of successful cross-species swaps over the run.
    /// Counts each initiator-side swap: `trade_pass` visits every agent as an
    /// initiator, so a reciprocal pair (each is the other's nearest partner)
    /// trades — and increments this — twice in one tick. It is a swap tally,
    /// not a distinct-exchange tally.
    /// Observability only (HUD trade counter / tests) — never read by the
    /// simulation, so it is skipped by serialization and does not affect
    /// state hashes; it resets to zero on snapshot load.
    #[serde(skip)]
    pub total_trades: u64,
}

/// Serde default for `World::max_population` (old snapshots lack the field).
fn default_max_population() -> u32 {
    crate::reproduce::MAX_POPULATION
}
fn default_world_size() -> f32 {
    crate::biome::WORLD_SIZE_DEFAULT
}
fn default_biome_res() -> usize {
    crate::biome::BIOME_RES_DEFAULT
}
fn default_hash_res() -> usize {
    crate::spatial::HASH_RES_DEFAULT
}

impl World {
    /// Build a world from a seed: deterministic biome + empty agent
    /// population + fresh spatial hash + tick 0.
    pub fn new(seed: u64) -> Self {
        Self {
            tick: 0,
            seed,
            rng: Rng::from_seed(seed),
            biome: BiomeField::generate(
                seed,
                crate::biome::BIOME_RES_DEFAULT,
                crate::biome::WORLD_SIZE_DEFAULT,
            ),
            agents: AgentBuffers::new(),
            // Start at 1 — id 0 is reserved as LINEAGE_NONE for founder parents.
            next_lineage_id: 1,
            // Species 0 is the founder; centroid will be initialized by
            // the first call to `species_step` once agents exist.
            species_centroids: vec![Genome::neutral()],
            species_member_counts: vec![0],
            species_parents: vec![None],
            next_species_id: 1,
            codex: crate::codex::CodexState::default(),
            carcasses: Vec::new(),
            pheromones: crate::pheromone::PheromoneField::new(),
            env_period: 0,
            biome_adaptation: false,
            terrain_habitat: false,
            inventions_enabled: false,
            cognition_enabled: false,
            living_biome: false,
            season_period: 0,
            resources: Vec::new(),
            resources_enabled: false,
            disasters_enabled: false,
            war_enabled: false,
            disasters: crate::disaster::DisasterState::default(),
            max_population: crate::reproduce::MAX_POPULATION,
            world_size: crate::biome::WORLD_SIZE_DEFAULT,
            biome_res: crate::biome::BIOME_RES_DEFAULT,
            hash_res: crate::spatial::HASH_RES_DEFAULT,
            spatial: UniformSpatialHash::with_dims(
                crate::biome::WORLD_SIZE_DEFAULT,
                crate::spatial::HASH_RES_DEFAULT,
            ),
            carcass_spatial: UniformSpatialHash::with_dims(
                crate::biome::WORLD_SIZE_DEFAULT,
                crate::spatial::HASH_RES_DEFAULT,
            ),
            resource_spatial: UniformSpatialHash::with_dims(
                crate::biome::WORLD_SIZE_DEFAULT,
                crate::spatial::HASH_RES_DEFAULT,
            ),
            sensors: Vec::new(),
            desired_direction: Vec::new(),
            actions: Vec::new(),
            reproduced_this_tick: BitVec::new(),
            codex_agg: crate::codex::SpeciesAggTable::default(),
            combat_damaged: Vec::new(),
            combat_attacker: Vec::new(),
            combat_streaks: Vec::new(),
            trade_routes: Vec::new(),
            still_ticks: Vec::new(),
            prev_desired_direction: Vec::new(),
            total_trades: 0,
        }
    }

    /// Build a world with explicit dimensions. The biome, pheromone grid, and
    /// spatial hashes are all regenerated at the requested resolution/extent.
    /// At default dimensions this is identical to `new`.
    pub fn with_dims(seed: u64, world_size: f32, biome_res: usize, hash_res: usize) -> Self {
        let mut w = Self::new(seed);
        w.world_size = world_size;
        w.biome_res = biome_res;
        w.hash_res = hash_res;
        w.biome = crate::biome::BiomeField::generate(seed, biome_res, world_size);
        w.pheromones = crate::pheromone::PheromoneField::with_dims(biome_res, world_size);
        w.spatial = crate::spatial::UniformSpatialHash::with_dims(world_size, hash_res);
        w.carcass_spatial = crate::spatial::UniformSpatialHash::with_dims(world_size, hash_res);
        w.resource_spatial = crate::spatial::UniformSpatialHash::with_dims(world_size, hash_res);
        w
    }

    /// Allocate a fresh, globally-unique lineage id. Never reuses values.
    #[inline]
    pub fn next_lineage(&mut self) -> LineageId {
        let id = self.next_lineage_id;
        self.next_lineage_id = self
            .next_lineage_id
            .checked_add(1)
            .expect("lineage id overflow: 2^64 births is implausible");
        id
    }

    /// Spawn a founder agent (no modelled parents) into the world. Lineage
    /// id is allocated here; species id is 0 (the founder species).
    pub fn spawn_agent(&mut self, position: Vec2, genome: Genome) -> AgentId {
        let lineage = self.next_lineage();
        let id = self.agents.spawn(
            position,
            genome,
            lineage,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            crate::program::starter_grazer(),
        );
        self.add_to_species(0);
        id
    }

    /// Spawn an agent with an explicit species, module kit, and program.
    /// Used by scenario archetypes (`spawn_agent` always uses species 0 +
    /// grazer defaults).
    pub fn spawn_seeded(
        &mut self,
        position: Vec2,
        genome: Genome,
        species_id: crate::agent::SpeciesId,
        modules: crate::module::ModuleList,
        program: crate::program::Program,
    ) -> AgentId {
        let lineage = self.next_lineage();
        let id = self.agents.spawn(
            position,
            genome,
            lineage,
            [LINEAGE_NONE; 2],
            species_id,
            modules,
            program,
        );
        self.add_to_species(species_id);
        id
    }

    /// Increment the species member count, growing the table if needed.
    /// Called by every spawn path.
    pub fn add_to_species(&mut self, species_id: u32) {
        let idx = species_id as usize;
        if idx >= self.species_member_counts.len() {
            // Caller created a species via the species_step split-off path
            // and is responsible for pushing centroid + parent first; this
            // helper only grows the count vec.
            self.species_member_counts.resize(idx + 1, 0);
        }
        self.species_member_counts[idx] =
            self.species_member_counts[idx].checked_add(1).expect("species member count overflow");
    }

    /// Decrement the species member count. Saturating: if the count is
    /// already zero (bookkeeping bug), do not underflow.
    pub fn remove_from_species(&mut self, species_id: u32) {
        let idx = species_id as usize;
        if idx >= self.species_member_counts.len() {
            return;
        }
        self.species_member_counts[idx] = self.species_member_counts[idx].saturating_sub(1);
    }

    /// World dimensions (for callers that want the runtime extent without
    /// reading `world_size` directly).
    #[inline]
    pub fn size(&self) -> f32 {
        self.world_size
    }

    /// Sanity helper used by tests and the headless CLI.
    pub fn alive_energy_total(&self) -> f32 {
        let mut total = 0.0;
        for id in self.agents.iter_alive() {
            total += self.agents.energy[id as usize];
        }
        total
    }

    /// Sum of plant biomass across the biome.
    pub fn plant_biomass_total(&self) -> f32 {
        self.biome.cells.iter().map(|c| c.plant_biomass).sum()
    }

    /// Resize scratch buffers to match agent capacity. Called by the tick.
    pub(crate) fn resize_scratch(&mut self) {
        let cap = self.agents.capacity();
        if self.sensors.len() < cap {
            self.sensors.resize(cap, crate::sense::SensorRegister::default());
        }
        if self.desired_direction.len() < cap {
            self.desired_direction.resize(cap, crate::prelude::Vec2::ZERO);
        }
        if self.actions.len() < cap {
            self.actions.resize(cap, crate::program::ActionRegister::default());
        }
        if self.reproduced_this_tick.len() < cap {
            self.reproduced_this_tick.resize(cap, false);
        }
        if self.combat_damaged.len() < cap {
            self.combat_damaged.resize(cap, false);
        }
        if self.combat_attacker.len() < cap {
            self.combat_attacker.resize(cap, crate::sense::NO_NEIGHBOR_SPECIES);
        }
        if self.still_ticks.len() < cap {
            self.still_ticks.resize(cap, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::SPAWN_ENERGY;

    #[test]
    fn world_construction_is_deterministic() {
        let a = World::new(42);
        let b = World::new(42);
        assert_eq!(a.tick, b.tick);
        assert_eq!(a.seed, b.seed);
        for i in 0..a.biome.cells.len() {
            assert_eq!(a.biome.cells[i].terrain, b.biome.cells[i].terrain);
            assert!((a.biome.cells[i].plant_biomass - b.biome.cells[i].plant_biomass).abs() < 1e-6);
        }
    }

    #[test]
    fn spawn_agent_sets_initial_energy() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(10.0, 10.0), Genome::neutral());
        assert!(w.agents.is_alive(id));
        assert_eq!(w.agents.energy[id as usize], SPAWN_ENERGY);
    }
}
