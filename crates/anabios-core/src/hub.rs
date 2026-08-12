use crate::biome::BiomeField;
#[cfg(test)]
use crate::biome::TerrainType;
use crate::prelude::{wrap_torus, Vec2};
use crate::resource::{want, Good, GOOD_COUNT, STOCK_TARGET, TRADE_UNIT};
use serde::{Deserialize, Serialize};

/// Cell radius scanned around a candidate cell when scoring good-diversity.
pub const HUB_SCAN_RADIUS_CELLS: i32 = 3;
/// Minimum world-space distance between two hubs (torus).
pub const HUB_MIN_SPACING: f32 = 180.0;
/// Hard cap on the number of hubs placed on a map.
pub const HUB_MAX_COUNT: usize = 6;
/// Steering weight of the hub-seeking bias (mirrors TERRAIN_HABITAT_PULL = 1.0).
pub const HUB_PULL: f32 = 1.0;
/// How close (world units) an agent must be to a hub to trade there.
pub const HUB_TRADE_RANGE: f32 = 30.0;

/// A predetermined marketplace location, fixed at worldgen. `goods` is the set
/// of distinct trade goods whose home terrain meets here (for the viewer icons).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeHub {
    pub pos: Vec2,
    pub cell: usize,
    pub goods: Vec<Good>,
}

/// Distinct trade goods whose home terrain appears within `HUB_SCAN_RADIUS_CELLS`
/// of cell `(cx, cy)`, torus-wrapped. Returned in Good-index order (deterministic).
fn neighborhood_goods(biome: &BiomeField, cx: i32, cy: i32) -> Vec<Good> {
    let res = biome.res as i32;
    let mut seen = [false; GOOD_COUNT];
    for dy in -HUB_SCAN_RADIUS_CELLS..=HUB_SCAN_RADIUS_CELLS {
        for dx in -HUB_SCAN_RADIUS_CELLS..=HUB_SCAN_RADIUS_CELLS {
            let col = (cx + dx).rem_euclid(res) as usize;
            let row = (cy + dy).rem_euclid(res) as usize;
            if let Some(g) = Good::from_terrain(biome.at(col, row).terrain) {
                seen[g.index()] = true;
            }
        }
    }
    Good::ALL.iter().copied().filter(|g| seen[g.index()]).collect()
}

/// Predetermined trade hubs: crossroads where >= 2 distinct trade-good terrains
/// meet. Border-diversity greedy scan with a minimum inter-hub spacing and a
/// hard cap. Deterministic (fixed scan order, tie-break by lowest cell index).
/// Reads no RNG. Empty on a low-diversity map (honest: no hubs -> sparse trade).
pub fn place_trade_hubs(biome: &BiomeField) -> Vec<TradeHub> {
    let res = biome.res;
    let mut candidates: Vec<(usize, usize, Vec<Good>)> = Vec::new(); // (score, cell, goods)
    for cell in 0..biome.cells.len() {
        let cx = (cell % res) as i32;
        let cy = (cell / res) as i32;
        let goods = neighborhood_goods(biome, cx, cy);
        if goods.len() >= 2 {
            candidates.push((goods.len(), cell, goods));
        }
    }
    // Highest diversity first; ties broken by lowest cell index (deterministic).
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let world = Vec2::splat(biome.world_size);
    let half = Vec2::splat(biome.world_size * 0.5);
    let mut hubs: Vec<TradeHub> = Vec::new();
    for (_, cell, goods) in candidates {
        if hubs.len() >= HUB_MAX_COUNT {
            break;
        }
        let col = cell % res;
        let row = cell / res;
        let pos =
            Vec2::new((col as f32 + 0.5) * biome.cell_size, (row as f32 + 0.5) * biome.cell_size);
        let too_close = hubs.iter().any(|h| {
            let off = wrap_torus(h.pos - pos + half, world) - half;
            off.length() < HUB_MIN_SPACING
        });
        if too_close {
            continue;
        }
        hubs.push(TradeHub { pos, cell, goods });
    }
    hubs
}

/// True when an agent has a real reason to visit a hub: at least a full unit of
/// surplus to offload, or a full unit of deficit to fill. Balanced agents (every
/// good within a unit of `STOCK_TARGET`) have no motive and forage as before.
pub fn has_trade_motive(inv: &[f32; GOOD_COUNT]) -> bool {
    let surplus = inv.iter().any(|&q| q >= STOCK_TARGET + TRADE_UNIT);
    let deficit = (0..GOOD_COUNT).any(|k| want(inv, k) >= TRADE_UNIT);
    surplus || deficit
}

/// Unit direction toward the nearest trade hub under torus wrap; `Vec2::ZERO`
/// when there are no hubs. Deterministic (strict `<` keeps the first on ties).
pub fn best_hub_direction(hubs: &[TradeHub], pos: Vec2, world_size: f32) -> Vec2 {
    let world = Vec2::splat(world_size);
    let half = Vec2::splat(world_size * 0.5);
    let mut best: Option<(f32, Vec2)> = None;
    for h in hubs {
        let off = wrap_torus(h.pos - pos + half, world) - half;
        let d2 = off.length_squared();
        if best.is_none_or(|(bd, _)| d2 < bd) {
            best = Some((d2, off));
        }
    }
    match best {
        Some((_, off)) => off.normalize_or_zero(),
        None => Vec2::ZERO,
    }
}

/// True if `pos` is within `range` world units of any hub (torus-aware).
pub fn near_any_hub(hubs: &[TradeHub], pos: Vec2, world_size: f32, range: f32) -> bool {
    let world = Vec2::splat(world_size);
    let half = Vec2::splat(world_size * 0.5);
    let r2 = range * range;
    hubs.iter().any(|h| {
        let off = wrap_torus(h.pos - pos + half, world) - half;
        off.length_squared() <= r2
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::{BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT};

    // Paint every cell of a generated field to a single terrain (one good) so
    // no neighborhood ever spans >= 2 distinct goods.
    fn uniform_field(terrain: TerrainType) -> BiomeField {
        let mut f = BiomeField::generate(9, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        for c in f.cells.iter_mut() {
            c.terrain = terrain;
        }
        f
    }

    // A field with distinct good-terrains in the four quadrants, so their
    // borders are genuine multi-good crossroads.
    fn quadrant_field() -> BiomeField {
        let mut f = BiomeField::generate(9, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        let res = f.res;
        let quad = [
            TerrainType::Desert, // Salt
            TerrainType::Rock,   // Obsidian
            TerrainType::Forest, // Amber
            TerrainType::Grass,  // Spice
        ];
        for row in 0..res {
            for col in 0..res {
                let q = (if col < res / 2 { 0 } else { 1 }) + (if row < res / 2 { 0 } else { 2 });
                f.at_mut(col, row).terrain = quad[q];
            }
        }
        f
    }

    #[test]
    fn placement_is_deterministic_and_bounded() {
        let f = quadrant_field();
        let a = place_trade_hubs(&f);
        let b = place_trade_hubs(&f);
        assert_eq!(a, b, "placement must be deterministic");
        assert!(a.len() <= HUB_MAX_COUNT, "must respect the cap");
        assert!(!a.is_empty(), "a diverse map must yield at least one hub");
        // Pairwise min-spacing (torus).
        let world = Vec2::splat(f.world_size);
        let half = Vec2::splat(f.world_size * 0.5);
        for i in 0..a.len() {
            for j in (i + 1)..a.len() {
                let off = wrap_torus(a[i].pos - a[j].pos + half, world) - half;
                assert!(off.length() >= HUB_MIN_SPACING, "hubs must be spaced apart");
            }
        }
    }

    #[test]
    fn placement_empty_on_single_good_map() {
        let f = uniform_field(Good::Salt.home_terrain());
        assert!(place_trade_hubs(&f).is_empty(), "one good -> no crossroads");
    }

    #[test]
    fn motive_true_on_surplus_or_deficit_false_when_balanced() {
        let balanced = [STOCK_TARGET; GOOD_COUNT];
        assert!(!has_trade_motive(&balanced));
        let mut surplus = [STOCK_TARGET; GOOD_COUNT];
        surplus[0] = STOCK_TARGET + TRADE_UNIT;
        assert!(has_trade_motive(&surplus));
        let mut deficit = [STOCK_TARGET; GOOD_COUNT];
        deficit[1] = STOCK_TARGET - TRADE_UNIT;
        assert!(has_trade_motive(&deficit));
    }

    #[test]
    fn hub_direction_points_at_nearest_including_wrap() {
        let ws = 100.0;
        let hubs = vec![
            TradeHub { pos: Vec2::new(10.0, 50.0), cell: 0, goods: vec![] },
            TradeHub { pos: Vec2::new(60.0, 50.0), cell: 1, goods: vec![] },
        ];
        // From x=95 the nearest hub is x=10 ACROSS the seam (dist 15), not x=60.
        let dir = best_hub_direction(&hubs, Vec2::new(95.0, 50.0), ws);
        assert!(dir.x > 0.9, "should steer +x across the wrap toward x=10");
        // No hubs -> zero.
        assert_eq!(best_hub_direction(&[], Vec2::new(1.0, 1.0), ws), Vec2::ZERO);
    }

    #[test]
    fn near_any_hub_respects_range_and_wrap() {
        let ws = 100.0;
        let hubs = vec![TradeHub { pos: Vec2::new(5.0, 5.0), cell: 0, goods: vec![] }];
        // x=98 is 7 away from x=5 across the seam -> within range 10.
        assert!(near_any_hub(&hubs, Vec2::new(98.0, 5.0), ws, 10.0));
        assert!(!near_any_hub(&hubs, Vec2::new(50.0, 50.0), ws, 10.0));
        assert!(!near_any_hub(&[], Vec2::new(5.0, 5.0), ws, 10.0));
    }
}
