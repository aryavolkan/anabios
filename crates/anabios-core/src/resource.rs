//! Biome trade goods: four unique natural resources that spawn in their home
//! terrain, are harvested and carried by agents, swapped between species, and
//! spent as a reproduction dowry. Opt-in per scenario via `World::resources_enabled`.

use serde::{Deserialize, Serialize};

use crate::biome::TerrainType;
use crate::prelude::Vec2;
use crate::world::World;

/// Number of distinct trade goods. One per land terrain.
pub const GOOD_COUNT: usize = 4;

/// Biome plant regrowth cadence is reused for resource spawning.
pub const RESOURCE_STEP_INTERVAL: u64 = 10;
/// Random placement attempts per spawn step (fixed → deterministic RNG draw count).
pub const NODE_SPAWN_ATTEMPTS: usize = 64;
/// Target live node count per good; spawning stops adding a good at/above this.
/// Unchanged at 80: turnover testing (basket-accumulating `pick_swap` +
/// `DOWRY_REQ` = 2.0) showed this ceiling isn't the bottleneck — a scarce
/// terrain's live node count near any one cluster is already far below it,
/// so lowering it (tried 60) starved the trade scenario without freeing up
/// any other resource; the real limiter is map-area fraction per terrain.
pub const NODE_TARGET_PER_GOOD: usize = 80;
/// Hard cap on total live nodes.
pub const NODE_MAX_TOTAL: usize = 400;
/// Amount a fresh node carries. Raised from 200: basket-accumulating
/// `pick_swap` (deficit valuation, see `want` below) fixed the old
/// equal-holdings absorbing state, but `DOWRY_REQ` doubling (1.0 -> 2.0)
/// doubles the raw goods each agent needs, and that dominates. Turnover
/// testing showed 200 (and everything below it down to 40) fails to produce
/// a `DowryBirth` in the trade scenario's 600-tick window; 225 clears it with
/// margin (first dowry birth ~tick 507, several agents holding a full basket
/// by the end) — a modest supply increase, not the hoped-for decrease.
pub const NODE_START_AMOUNT: f32 = 225.0;
/// Max distance an agent can harvest a node from (world units).
pub const HARVEST_RANGE: f32 = 2.0;
/// Max amount harvested from a node per tick per agent. Unchanged at 5.0:
/// turnover testing showed the per-tick harvest rate has much less leverage
/// on `DowryBirth` reachability than total node supply (`NODE_START_AMOUNT`)
/// does — raising it alone (to 6.0, 7.0) while lowering start amount did not
/// recover a passing run, so it stays at its prior value.
pub const HARVEST_RATE: f32 = 5.0;
/// Base per-agent carrying capacity (summed across all goods).
pub const INVENTORY_BASE_CAP: f32 = 12.0;
/// Extra carrying capacity granted by a `Storage` module.
pub const INVENTORY_STORAGE_BONUS: f32 = 12.0;
/// Max distance for a bilateral trade (world units).
pub const TRADE_RANGE: f32 = 2.0;
/// Units of a good moved in one direction per trade event.
pub const TRADE_UNIT: f32 = 1.0;
/// Units of EACH good a parent must hold and spend to reproduce — the balanced
/// basket. Reachable because `pick_swap` (see `interact.rs`) values goods by
/// their deficit below this target, so agents accumulate toward a full basket
/// instead of stalling at "equal holdings" (the old strict-`>` / diminishing-
/// utility rule capped trade-only goods near 1 unit and forced this to 1.0).
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

/// Trade valuation of good `k`: how far the agent is BELOW the dowry target
/// `DOWRY_REQ` for that good, clamped at 0. An agent values a good it still
/// needs to complete its basket and stops valuing it once that slot is full —
/// so bilateral trade drives agents to ACCUMULATE a full balanced basket
/// rather than merely equalize holdings against a neighbor.
#[inline]
pub fn want(inventory: &[f32; GOOD_COUNT], k: usize) -> f32 {
    (DOWRY_REQ - inventory[k]).max(0.0)
}

/// Total units held across all goods.
#[inline]
pub fn inventory_total(inv: &[f32; GOOD_COUNT]) -> f32 {
    inv.iter().sum()
}

/// Per-agent carrying capacity: a flat base, plus a bonus for agents that
/// carry a `Storage` module (reuses the existing morphology).
pub fn carrying_cap(modules: &crate::module::ModuleList) -> f32 {
    let mut cap = INVENTORY_BASE_CAP;
    if crate::module::has(modules, crate::module::ModuleType::Storage) {
        cap += INVENTORY_STORAGE_BONUS;
    }
    cap
}

/// Spawn new resource nodes in their home terrain and remove depleted ones.
/// Gated on `resources_enabled` — draws ZERO RNG and touches nothing when off.
/// Called on the biome cadence (`RESOURCE_STEP_INTERVAL`).
pub fn resource_step(world: &mut World) {
    if !world.resources_enabled {
        return;
    }
    // Drop depleted nodes first (retain preserves order → deterministic).
    world.resources.retain(|r| r.amount > 0.0);

    // Per-good live counts.
    let mut counts = [0usize; GOOD_COUNT];
    for r in &world.resources {
        counts[r.kind.index()] += 1;
    }

    // Fixed attempt budget → fixed RNG draw count per step (2 draws/attempt),
    // independent of how many actually land, keeping the stream deterministic.
    for _ in 0..NODE_SPAWN_ATTEMPTS {
        if world.resources.len() >= NODE_MAX_TOTAL {
            // Still draw so the RNG stream does not depend on the early exit.
            let _ = world.rng.f32_range(0.0, world.world_size);
            let _ = world.rng.f32_range(0.0, world.world_size);
            continue;
        }
        let x = world.rng.f32_range(0.0, world.world_size);
        let y = world.rng.f32_range(0.0, world.world_size);
        let pos = crate::prelude::Vec2::new(x, y);
        let Some(good) = Good::from_terrain(world.biome.sample(pos).terrain) else {
            continue;
        };
        let k = good.index();
        if counts[k] >= NODE_TARGET_PER_GOOD {
            continue;
        }
        world.resources.push(Resource { pos, kind: good, amount: NODE_START_AMOUNT });
        counts[k] += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;

    #[test]
    fn resource_step_is_inert_when_disabled() {
        let mut w = World::new(7);
        // resources_enabled defaults false.
        for _ in 0..50 {
            resource_step(&mut w);
        }
        assert!(w.resources.is_empty(), "no nodes spawn while feature is off");
    }

    #[test]
    fn resource_step_spawns_nodes_in_matching_terrain() {
        let mut w = World::new(7);
        w.resources_enabled = true;
        for _ in 0..50 {
            resource_step(&mut w);
        }
        assert!(!w.resources.is_empty(), "nodes spawn when enabled");
        // Every node's terrain matches its good.
        for r in &w.resources {
            let terrain = w.biome.sample(r.pos).terrain;
            assert_eq!(Good::from_terrain(terrain), Some(r.kind), "node good matches its terrain");
            assert!(r.amount > 0.0);
        }
        // Per-good counts respect the target ceiling.
        for g in Good::ALL {
            let n = w.resources.iter().filter(|r| r.kind == g).count();
            assert!(n <= NODE_TARGET_PER_GOOD, "{g:?} count {n} exceeds target");
        }
    }

    #[test]
    fn resource_step_removes_depleted_nodes() {
        let mut w = World::new(7);
        w.resources_enabled = true;
        resource_step(&mut w);
        let before = w.resources.len();
        assert!(before > 0);
        // Deplete the first node; the next step must drop it.
        w.resources[0].amount = 0.0;
        resource_step(&mut w);
        assert!(w.resources.len() < before || !w.resources.iter().any(|r| r.amount <= 0.0));
    }

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
    fn want_is_dowry_deficit() {
        let mut inv = [0.0f32; GOOD_COUNT];
        // Empty slot: want equals the full dowry target.
        assert!((want(&inv, 0) - DOWRY_REQ).abs() < 1e-6);
        // Partially filled: want is the remaining deficit.
        inv[0] = DOWRY_REQ - 0.5;
        assert!((want(&inv, 0) - 0.5).abs() < 1e-6);
        // Full (or over-full) slot: want is zero (satiated), never negative.
        inv[0] = DOWRY_REQ + 3.0;
        assert_eq!(want(&inv, 0), 0.0);
    }

    #[test]
    fn storage_module_raises_carrying_cap() {
        let base = crate::module::starter_kit();
        let mut with_storage = base.clone();
        with_storage.push(crate::module::Module::Storage { capacity: 1.0 });
        assert_eq!(carrying_cap(&base), INVENTORY_BASE_CAP);
        assert_eq!(carrying_cap(&with_storage), INVENTORY_BASE_CAP + INVENTORY_STORAGE_BONUS);
    }
}
