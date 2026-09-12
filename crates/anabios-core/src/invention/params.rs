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
/// Hafted Spears: weapon-damage bonus; fraction of the final net damage the
/// attacker recovers as energy (hunt spoils — a transfer, never creation).
pub const SPEARS_DAMAGE: f32 = 0.25;
pub const SPEARS_SPOILS: f32 = 0.30;
/// Archery: weapon-reach multiplier bonus; weapon-damage bonus; small flat
/// per-tick upkeep (fletching and staves).
pub const ARCHERY_RANGE: f32 = 0.50;
pub const ARCHERY_DAMAGE: f32 = 0.15;
pub const ARCHERY_UPKEEP: f32 = 0.003;
/// Fortifications: incoming net-damage reduction; effective breeding-threshold
/// reduction (the birth-ledger subsidy — practices tax births, walls subsidize
/// them); locomotor speed penalty (sedentary).
pub const FORT_DEFENSE: f32 = 0.25;
pub const FORT_BIRTH_SUBSIDY: f32 = 0.15;
pub const FORT_SPEED_PENALTY: f32 = 0.10;
/// Steel Arms: weapon-damage bonus (stacks additively with Metalworking's
/// inside the same multiplier); added spoils fraction; extra module upkeep.
pub const STEEL_DAMAGE: f32 = 0.60;
pub const STEEL_SPOILS: f32 = 0.20;
pub const STEEL_UPKEEP: f32 = 0.10;
/// Pottery: graze-bite bonus when the local cell is depleted (stored food
/// reads as a bigger bite exactly when the land runs out), and the
/// biomass-fraction-of-capacity below which the bonus applies.
pub const POTTERY_BITE: f32 = 0.30;
pub const POTTERY_LOW_BIOMASS: f32 = 0.5;
/// Irrigation: graze-bite bonus on dry cells, the moisture below which a cell
/// counts as dry, and the extra crowding allowance it adds to Farming's free
/// neighbors (watered fields carry denser villages).
pub const IRRIGATION_BITE: f32 = 0.30;
pub const IRRIGATION_DRY_MOISTURE: f32 = 0.4;
pub const IRRIGATION_CROWDING_BONUS: u32 = 8;
/// Currency: trade-range multiplier bonus (coinage lets strangers deal at
/// arm's length) and the flat energy dividend each side of a swap pockets
/// (market efficiency); small flat per-tick upkeep.
pub const CURRENCY_RANGE: f32 = 0.50;
pub const CURRENCY_SWAP_ENERGY: f32 = 0.05;
pub const CURRENCY_UPKEEP: f32 = 0.002;
/// Printing: multiplier on meme copy / invention spread stacking with
/// Writing's (the knowledge branch compounds); small flat per-tick upkeep.
pub const PRINTING_SPREAD_MULT: f32 = 1.5;
pub const PRINTING_UPKEEP: f32 = 0.002;
/// Sanitation: susceptibility and recovery multipliers, stacking with
/// Medicine's (the welfare branch compounds); small flat per-tick upkeep.
pub const SANITATION_SUSCEPT_MULT: f32 = 0.5;
pub const SANITATION_RECOVERY_MULT: f32 = 1.5;
pub const SANITATION_UPKEEP: f32 = 0.002;
/// Gunpowder: weapon-damage and weapon-range bonuses (stack additively with
/// the earlier military branch inside the same multipliers); small flat
/// per-tick upkeep.
pub const GUNPOWDER_DAMAGE: f32 = 0.40;
pub const GUNPOWDER_RANGE: f32 = 0.30;
pub const GUNPOWDER_UPKEEP: f32 = 0.003;
/// Wells: multiplier on the holder's per-tick thirst GAIN (`needs::needs_step`)
/// — stored water means slower parching. Applies to the gain only, never to
/// `DRINK_RATE` or `dehydration_metabolism_multiplier`.
pub const WELLS_THIRST_MULT: f32 = 0.5;
/// Vaccination: susceptibility (transmission) and spillover-probability
/// multipliers, stacking with Medicine's and Sanitation's (the welfare
/// branch compounds further); small flat per-tick upkeep (era-4 tier,
/// matching Gunpowder's).
pub const VACCINATION_SUSCEPT_MULT: f32 = 0.5;
pub const VACCINATION_SPILLOVER_MULT: f32 = 0.5;
pub const VACCINATION_UPKEEP: f32 = 0.003;

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
