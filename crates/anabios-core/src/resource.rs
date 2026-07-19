//! Biome trade goods: four unique natural resources that spawn in their home
//! terrain, are harvested and carried by agents, swapped between species, and
//! spent as a reproduction dowry. Opt-in per scenario via `World::resources_enabled`.

use serde::{Deserialize, Serialize};

use crate::biome::TerrainType;
use crate::prelude::Vec2;

/// Number of distinct trade goods. One per land terrain.
pub const GOOD_COUNT: usize = 4;

/// Biome plant regrowth cadence is reused for resource spawning.
pub const RESOURCE_STEP_INTERVAL: u64 = 10;
/// Random placement attempts per spawn step (fixed → deterministic RNG draw count).
pub const NODE_SPAWN_ATTEMPTS: usize = 64;
/// Target live node count per good; spawning stops adding a good at/above this.
pub const NODE_TARGET_PER_GOOD: usize = 40;
/// Hard cap on total live nodes.
pub const NODE_MAX_TOTAL: usize = 400;
/// Amount a fresh node carries.
pub const NODE_START_AMOUNT: f32 = 20.0;
/// Max distance an agent can harvest a node from (world units).
pub const HARVEST_RANGE: f32 = 2.0;
/// Max amount harvested from a node per tick per agent.
pub const HARVEST_RATE: f32 = 1.0;
/// Base per-agent carrying capacity (summed across all goods).
pub const INVENTORY_BASE_CAP: f32 = 12.0;
/// Extra carrying capacity granted by a `Storage` module.
pub const INVENTORY_STORAGE_BONUS: f32 = 12.0;
/// Max distance for a bilateral trade (world units).
pub const TRADE_RANGE: f32 = 2.0;
/// Units of a good moved in one direction per trade event.
pub const TRADE_UNIT: f32 = 1.0;
/// Units of EACH good a parent must hold and spend to reproduce.
pub const DOWRY_REQ: f32 = 2.0;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Good {
    Salt = 0,
    Obsidian = 1,
    Amber = 2,
    Spice = 3,
}

impl Good {
    /// All goods in index order.
    pub const ALL: [Good; GOOD_COUNT] = [Good::Salt, Good::Obsidian, Good::Amber, Good::Spice];

    /// Stable array index for this good.
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    /// The good that spawns in a given terrain. Water yields nothing.
    #[inline]
    pub fn from_terrain(t: TerrainType) -> Option<Good> {
        match t {
            TerrainType::Desert => Some(Good::Salt),
            TerrainType::Rock => Some(Good::Obsidian),
            TerrainType::Forest => Some(Good::Amber),
            TerrainType::Grass => Some(Good::Spice),
            TerrainType::Water => None,
        }
    }
}

/// A discrete resource node on the map.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Resource {
    pub pos: Vec2,
    pub kind: Good,
    pub amount: f32,
}

/// Marginal desire for good `k`: high when the agent holds little of it
/// (diminishing marginal utility). You value what you are short of.
#[inline]
pub fn want(inventory: &[f32; GOOD_COUNT], k: usize) -> f32 {
    1.0 / (1.0 + inventory[k])
}

/// Total units held across all goods.
#[inline]
pub fn inventory_total(inv: &[f32; GOOD_COUNT]) -> f32 {
    inv.iter().sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;

    #[test]
    fn terrain_maps_to_expected_good() {
        assert_eq!(Good::from_terrain(TerrainType::Desert), Some(Good::Salt));
        assert_eq!(Good::from_terrain(TerrainType::Rock), Some(Good::Obsidian));
        assert_eq!(Good::from_terrain(TerrainType::Forest), Some(Good::Amber));
        assert_eq!(Good::from_terrain(TerrainType::Grass), Some(Good::Spice));
        assert_eq!(Good::from_terrain(TerrainType::Water), None);
    }

    #[test]
    fn all_goods_have_matching_indices() {
        for (i, g) in Good::ALL.iter().enumerate() {
            assert_eq!(g.index(), i);
        }
    }

    #[test]
    fn want_falls_as_holdings_rise() {
        let mut inv = [0.0f32; GOOD_COUNT];
        let scarce = want(&inv, 0);
        inv[0] = 5.0;
        let plentiful = want(&inv, 0);
        assert!(scarce > plentiful, "scarcer good must be wanted more");
        assert!((scarce - 1.0).abs() < 1e-6, "empty holding => want 1.0");
    }
}
