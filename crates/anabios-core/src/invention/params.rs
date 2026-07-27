//! Invention effect magnitudes and discovery/spread tuning constants.
//!
//! Split out of `invention/mod.rs` so the tree definition and effect logic stay
//! readable (mirrors `codex/params.rs`). Re-exported via `pub use params::*`, so
//! every constant is still reachable as `crate::invention::<NAME>`.

// --- Effect magnitudes -----------------------------------------------------

/// Stone Tools: graze-bite bonus.
pub const STONE_TOOLS_BITE: f32 = 0.25;
/// Fire: energy-per-biomass bonus; extra basal metabolism fraction.
pub const FIRE_ENERGY: f32 = 0.40;
pub const FIRE_METABOLISM: f32 = 0.10;
/// Farming: graze-bite bonus; energy drained per tick per crowding neighbour
/// above the free allowance (sedentary density stress).
pub const FARMING_BITE: f32 = 0.60;
pub const FARMING_CROWDING_FREE: u32 = 8;
pub const FARMING_STRESS_PER_NEIGHBOR: f32 = 0.002;
/// Metalworking: weapon-damage bonus; extra module upkeep fraction.
pub const METALWORKING_DAMAGE: f32 = 0.50;
pub const METALWORKING_UPKEEP: f32 = 0.10;
/// Writing: multiplier on meme copy rate and invention spread rate; small
/// flat per-tick upkeep.
pub const WRITING_SPREAD_MULT: f32 = 2.0;
pub const WRITING_UPKEEP: f32 = 0.003;
/// Medicine: lifespan bonus; small flat per-tick upkeep.
pub const MEDICINE_LIFESPAN: f32 = 0.50;
pub const MEDICINE_UPKEEP: f32 = 0.003;
/// Husbandry: scavenge-energy bonus; extra basal metabolism fraction.
pub const HUSBANDRY_SCAVENGE: f32 = 0.40;
pub const HUSBANDRY_METABOLISM: f32 = 0.08;
/// Machinery: speed + graze-bite bonuses; pollution deposited into the local
/// biome cell per tick (regrowth penalty, decays per biome step).
pub const MACHINERY_SPEED: f32 = 0.25;
pub const MACHINERY_BITE: f32 = 0.25;
pub const MACHINERY_POLLUTION_DEPOSIT: f32 = 0.002;
/// Electricity: perception-radius bonus; discovery-rate multiplier; upkeep.
pub const ELECTRICITY_PERCEPTION: f32 = 0.30;
pub const ELECTRICITY_DISCOVERY: f32 = 1.5;
pub const ELECTRICITY_UPKEEP: f32 = 0.005;
/// Nuclear Power: flat per-tick energy income; child mutation-sigma
/// multiplier (radiation); heavy flat upkeep.
pub const NUCLEAR_INCOME: f32 = 0.06;
pub const NUCLEAR_MUTATION: f32 = 1.5;
pub const NUCLEAR_UPKEEP: f32 = 0.012;

/// Biome pollution: per-cell cap, regrowth-penalty cap, and per-biome-step
/// decay. Regrowth is multiplied by `1 - min(pollution, POLLUTION_MAX_EFFECT)`.
pub const POLLUTION_CAP: f32 = 0.8;
pub const POLLUTION_MAX_EFFECT: f32 = 0.7;
pub const POLLUTION_DECAY: f32 = 0.95;

// --- Discovery / spread tuning ----------------------------------------------

/// Base per-agent per-tick discovery probability at Openness = 1, skill = 1,
/// era 1 (scaled down by era and by the agent's traits/skill).
pub const BASE_DISCOVERY: f32 = 3e-5;
/// Hard cap on the summed per-tick discovery probability (all candidates).
pub const DISCOVERY_CAP: f32 = 0.05;
/// Spread: per-tick lerp rate toward the best-holding neighbour's level
/// (the skill channel's `SKILL_SOCIAL_RATE` analogue).
pub const INVENTION_SPREAD_RATE: f32 = 0.03;
/// Knowledge atrophy: per-tick decay of an invention level whose prereqs the
/// agent does NOT hold (foundations lost → the dependent tech fades).
pub const ATROPHY_RATE: f32 = 0.001;
