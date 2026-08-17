# Trade Hubs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add predetermined, worldgen-derived trade hubs at trade-good terrain borders; trade-motivated agents steer to the nearest hub, barter happens only at hubs, and the viewer draws marketplaces + goods icons.

**Architecture:** A new `hub` module in `anabios-core` computes fixed `TradeHub` locations from the biome grid at scenario instantiate. Movement gets one additive steering bias (`best_hub_direction`) in `decide_all`, active only when an agent has a real trade motive. `trade_pass` gates barter on hub proximity. The Godot viewer exposes hubs via a new accessor and renders them in a dedicated layer with polished sprites and new goods icons. Behavior-first: `FORMAT_VERSION` bumps and all goldens are regenerated to the new baseline.

**Tech Stack:** Rust (anabios-core sim + anabios-godot GDExtension bridge via godot-rust), GDScript (Godot 4.5 viewer), glam `Vec2`, serde/bincode snapshots.

## Global Constraints

- Everything trade/hub-related is inert unless `World.resources_enabled` is true (the trade-goods subsystem flag). No hubs, no gate changes, no seeking when it is off.
- Determinism: all new sim functions read no RNG and use fixed scan order with stable tie-breaks (mirror `biome::best_terrain_direction`). Torus wrap via `crate::prelude::wrap_torus`.
- `cargo fmt --check` (committed tree) and clippy `-D warnings` must pass; GDScript must pass gdformat/gdlint.
- Goldens are regenerated with `UPDATE_HASHES=1`, never hand-edited. The new golden values ARE the new baseline (we are NOT preserving prior values).
- All sim code lives under `crates/anabios-core/src/`; the Godot bridge is `crates/anabios-godot/src/lib.rs`; viewer scripts are under `game/scripts/`.
- Good↔terrain mapping is fixed (`resource::Good::from_terrain`): Desert→Salt, Rock→Obsidian, Forest/Rainforest/Taiga→Amber, Grass/Savanna/Tundra→Spice, Water→None. Good index order: Salt=0, Obsidian=1, Amber=2, Spice=3.

---

### Task 1: `hub` module — TradeHub type, placement, motive, steering & proximity helpers

**Files:**
- Create: `crates/anabios-core/src/hub.rs`
- Modify: `crates/anabios-core/src/lib.rs` (register `pub mod hub;`)

**Interfaces:**
- Consumes: `crate::biome::{BiomeField, TerrainType}`; `crate::resource::{Good, GOOD_COUNT, STOCK_TARGET, TRADE_UNIT, want}`; `crate::prelude::{Vec2, wrap_torus}`. `BiomeField` fields used: `cells: Vec<BiomeCell>`, `res: usize`, `world_size: f32`, `cell_size: f32`, method `at(col, row) -> &BiomeCell` (has `.terrain`). `Good::from_terrain(TerrainType) -> Option<Good>`, `Good::ALL: [Good; 4]`, `Good::index(self) -> usize`. `want(&[f32; GOOD_COUNT], usize) -> f32`.
- Produces (later tasks rely on these EXACT signatures):
  - `pub struct TradeHub { pub pos: Vec2, pub cell: usize, pub goods: Vec<Good> }` — derives `Debug, Clone, PartialEq, Serialize, Deserialize`.
  - `pub const HUB_SCAN_RADIUS_CELLS: i32 = 3;`
  - `pub const HUB_MIN_SPACING: f32 = 180.0;`
  - `pub const HUB_MAX_COUNT: usize = 6;`
  - `pub const HUB_PULL: f32 = 1.0;`
  - `pub const HUB_TRADE_RANGE: f32 = 30.0;`
  - `pub fn place_trade_hubs(biome: &BiomeField) -> Vec<TradeHub>`
  - `pub fn has_trade_motive(inv: &[f32; GOOD_COUNT]) -> bool`
  - `pub fn best_hub_direction(hubs: &[TradeHub], pos: Vec2, world_size: f32) -> Vec2`
  - `pub fn near_any_hub(hubs: &[TradeHub], pos: Vec2, world_size: f32, range: f32) -> bool`

- [ ] **Step 1: Register the module.** In `crates/anabios-core/src/lib.rs`, add `pub mod hub;` in alphabetical position (between `pub mod genome;` and `pub mod integrate;`).

- [ ] **Step 2: Write the failing tests.** Create `crates/anabios-core/src/hub.rs` with ONLY the `use` lines and this test module (implementation comes next), so it fails to compile / fails on missing items:

```rust
use crate::biome::{BiomeField, TerrainType};
use crate::prelude::{wrap_torus, Vec2};
use crate::resource::{want, Good, GOOD_COUNT, STOCK_TARGET, TRADE_UNIT};
use serde::{Deserialize, Serialize};

// (implementation added in Step 4)

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
```

- [ ] **Step 3: Run the tests to verify they fail.**

Run: `cd crates/anabios-core && cargo test --lib hub::`
Expected: FAIL — `place_trade_hubs`, `TradeHub`, etc. not found (compile error).

- [ ] **Step 4: Write the implementation.** Insert this ABOVE the `#[cfg(test)]` block in `crates/anabios-core/src/hub.rs`:

```rust
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
        let pos = Vec2::new(
            (col as f32 + 0.5) * biome.cell_size,
            (row as f32 + 0.5) * biome.cell_size,
        );
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
```

- [ ] **Step 5: Run the tests to verify they pass.**

Run: `cd crates/anabios-core && cargo test --lib hub::`
Expected: PASS (5 tests). Then `cargo fmt` and `cargo clippy -p anabios-core -- -D warnings`.

- [ ] **Step 6: Commit.**

```bash
git add crates/anabios-core/src/hub.rs crates/anabios-core/src/lib.rs
git commit -m "feat(hub): TradeHub + placement, motive, steering & proximity helpers"
```

---

### Task 2: `World.trade_hubs` field, FORMAT_VERSION bump, scenario placement wiring

**Files:**
- Modify: `crates/anabios-core/src/world.rs` (add field + constructor init)
- Modify: `crates/anabios-core/src/snapshot.rs:144` (`FORMAT_VERSION` 32 → 33)
- Modify: `crates/anabios-core/src/scenario.rs` (call `place_trade_hubs` after biome finalized)
- Test: `crates/anabios-core/src/hub.rs` (new tests appended to its `mod tests`)

**Interfaces:**
- Consumes: `crate::hub::{TradeHub, place_trade_hubs}` from Task 1; `crate::snapshot::{save_to_bytes, load_from_bytes}`; `crate::scenario::Scenario::{parse_toml, instantiate}`.
- Produces: `World.trade_hubs: Vec<crate::hub::TradeHub>` (public field, serialized), populated at instantiate when `resources_enabled`.

- [ ] **Step 1: Write the failing tests.** Append to the `mod tests` block in `crates/anabios-core/src/hub.rs`:

```rust
    #[test]
    fn world_trade_hubs_survive_snapshot_roundtrip() {
        use crate::snapshot::{load_from_bytes, save_to_bytes};
        use crate::world::World;
        let mut w = World::new(3);
        w.resources_enabled = true;
        // Paint a two-good split so placement yields hubs.
        let res = w.biome.res;
        for row in 0..res {
            for col in 0..res {
                let t = if col < res / 2 { TerrainType::Desert } else { TerrainType::Rock };
                w.biome.at_mut(col, row).terrain = t;
            }
        }
        w.trade_hubs = place_trade_hubs(&w.biome);
        assert!(!w.trade_hubs.is_empty(), "painted split must yield hubs");
        let bytes = save_to_bytes(&w).expect("save");
        let w2 = load_from_bytes(&bytes).expect("load");
        assert_eq!(w.trade_hubs, w2.trade_hubs, "hubs must round-trip identically");
    }

    #[test]
    fn scenario_instantiate_populates_hubs_from_biome() {
        use crate::scenario::Scenario;
        const TRADE: &str = include_str!("../../../scenarios/biome-trade.toml");
        let w = Scenario::parse_toml(TRADE).expect("parse").instantiate();
        assert!(w.resources_enabled, "biome-trade must enable resources");
        // apply() must have stored exactly what placement computes from the
        // finalized biome (proves the wiring ran, not the default empty vec).
        assert_eq!(w.trade_hubs, place_trade_hubs(&w.biome));
    }
```

- [ ] **Step 2: Run the tests to verify they fail.**

Run: `cd crates/anabios-core && cargo test --lib hub::world_trade_hubs hub::scenario_instantiate`
Expected: FAIL — `World` has no field `trade_hubs`.

- [ ] **Step 3: Add the World field.** In `crates/anabios-core/src/world.rs`, inside `pub struct World { ... }`, add near the other trade/market fields (e.g. right after the `market_field` declaration around line 218):

```rust
    /// Predetermined trade hubs placed from the biome at instantiate (empty and
    /// inert unless `resources_enabled`). Fixed after generation. Serialized.
    pub trade_hubs: Vec<crate::hub::TradeHub>,
```

- [ ] **Step 4: Initialize it in the constructor.** In `World::new` (the struct literal returned around lines 355-412), add alongside `market_field: Vec::new(),`:

```rust
            trade_hubs: Vec::new(),
```

- [ ] **Step 5: Bump FORMAT_VERSION.** In `crates/anabios-core/src/snapshot.rs:144` change:

```rust
pub const FORMAT_VERSION: u32 = 33;
```

- [ ] **Step 6: Wire placement into scenario apply.** In `crates/anabios-core/src/scenario.rs`, immediately AFTER the `match &self.world_map { ... }` block that finalizes `w.biome` (the block ending around line 761), add:

```rust
        // Predetermined trade hubs: placed from the finalized biome once the
        // trade-goods subsystem is active. Must run AFTER the world_map match so
        // it sees the real (Earth or climate) biome, not the default one.
        if w.resources_enabled {
            w.trade_hubs = crate::hub::place_trade_hubs(&w.biome);
        }
```

- [ ] **Step 7: Run the tests to verify they pass.**

Run: `cd crates/anabios-core && cargo test --lib hub::`
Expected: PASS (7 tests). Then `cargo fmt` + `cargo clippy -p anabios-core -- -D warnings`.

- [ ] **Step 8: Commit.**

```bash
git add crates/anabios-core/src/world.rs crates/anabios-core/src/snapshot.rs crates/anabios-core/src/scenario.rs crates/anabios-core/src/hub.rs
git commit -m "feat(hub): World.trade_hubs field, FORMAT_VERSION 33, scenario placement"
```

---

### Task 3: Hub-seeking steering bias in `decide_all`

**Files:**
- Modify: `crates/anabios-core/src/tick.rs` (capture hubs/flag; add bias in the `decide_all` loop, ~line 248 after the terrain_habitat block)
- Test: `crates/anabios-core/tests/trade.rs` (new behavior test)

**Interfaces:**
- Consumes: `crate::hub::{best_hub_direction, has_trade_motive, HUB_PULL}`; `World.trade_hubs`, `World.resources_enabled`, `agents.inventory[i]: [f32; GOOD_COUNT]`, `agents.position[i]: Vec2`.
- Produces: no new public API — an added additive contribution to `action.move_x/move_y`.

- [ ] **Step 1: Write the failing test.** Append to `crates/anabios-core/tests/trade.rs`:

```rust
/// A trade-motivated agent drifts toward a hub over time; with hubs present and
/// resources on, its mean distance to the nearest hub should not increase.
#[test]
fn motivated_agents_drift_toward_hubs() {
    use anabios_core::hub::{best_hub_direction, near_any_hub, HUB_TRADE_RANGE};
    let mut w = Scenario::parse_toml(TRADE).expect("parse").instantiate();
    assert!(w.resources_enabled && !w.trade_hubs.is_empty(), "need hubs to test");
    // Fraction of alive agents currently near some hub, before and after warming.
    let frac_near = |w: &anabios_core::world::World| {
        let (mut near, mut total) = (0usize, 0usize);
        for id in w.agents.iter_alive() {
            total += 1;
            if near_any_hub(&w.trade_hubs, w.agents.position[id as usize], w.world_size, HUB_TRADE_RANGE) {
                near += 1;
            }
        }
        if total == 0 { 0.0 } else { near as f64 / total as f64 }
    };
    let before = frac_near(&w);
    for _ in 0..800 {
        step(&mut w);
    }
    let after = frac_near(&w);
    // Sanity: the steering function returns a unit pull toward a hub for a
    // motivated agent placed away from all hubs.
    let probe = best_hub_direction(&w.trade_hubs, anabios_core::prelude_test::Vec2::new(0.0, 0.0), w.world_size);
    assert!(probe.length() > 0.5, "hub steering must produce a real direction");
    assert!(after + 0.001 >= before, "hub clustering should not decrease over time");
}
```

Note: if `prelude_test::Vec2` is not exported, use a hub position offset instead — replace the `probe` lines with `assert!(best_hub_direction(&w.trade_hubs, w.trade_hubs[0].pos + glam_offset, w.world_size).length() > 0.5)` where the test builds a nearby point; the intent is only "non-zero direction toward a hub."

- [ ] **Step 2: Run the test to verify it fails.**

Run: `cd crates/anabios-core && cargo test --test trade motivated_agents_drift_toward_hubs`
Expected: FAIL — agents don't yet steer toward hubs (or the pull is absent, `after < before`).

- [ ] **Step 3: Capture the needed state in `decide_all`.** In `crates/anabios-core/src/tick.rs`, in the local bindings block at the top of `decide_all` (near lines 160-167 where `terrain_habitat`, `ws`, etc. are captured), add:

```rust
    let trade_hubs = &world.trade_hubs;
    let resources_enabled = world.resources_enabled;
```

- [ ] **Step 4: Add the hub-seeking bias.** In the same `decide_all` per-agent closure, immediately AFTER the `if terrain_habitat { ... }` block (around line 248, before the `if settlement_enabled` block), add:

```rust
            // Trade-hub seeking (opt-in with the trade-goods subsystem): agents
            // with a real trade motive steer toward the nearest predetermined
            // hub so barter partners converge there. Motiveless agents forage
            // normally. Additive bias, normalized with the rest of the stack.
            if resources_enabled && crate::hub::has_trade_motive(&agents.inventory[i]) {
                let pull = crate::hub::best_hub_direction(trade_hubs, agents.position[i], ws);
                action.move_x += crate::hub::HUB_PULL * pull.x;
                action.move_y += crate::hub::HUB_PULL * pull.y;
            }
```

- [ ] **Step 5: Run the test to verify it passes.**

Run: `cd crates/anabios-core && cargo test --test trade motivated_agents_drift_toward_hubs`
Expected: PASS. Then `cargo fmt` + `cargo clippy -p anabios-core -- -D warnings`.

- [ ] **Step 6: Commit.**

```bash
git add crates/anabios-core/src/tick.rs crates/anabios-core/tests/trade.rs
git commit -m "feat(hub): trade-motivated agents steer toward nearest hub in decide_all"
```

---

### Task 4: Restrict barter to hub proximity in `trade_pass`

**Files:**
- Modify: `crates/anabios-core/src/interact.rs` (`trade_pass`, ~line 517)
- Test: `crates/anabios-core/tests/trade.rs`

**Interfaces:**
- Consumes: `crate::hub::{near_any_hub, HUB_TRADE_RANGE}`; `World.trade_hubs`, `World.world_size`, `world.agents.position[i]`.
- Produces: no new API — a proximity guard added at the top of the `trade_pass` per-agent loop.

- [ ] **Step 1: Write the failing test.** Append to `crates/anabios-core/tests/trade.rs`:

```rust
/// Barter only happens at hubs: two complementary agents far from every hub do
/// NOT trade; the same pair placed on a hub DOES.
#[test]
fn trade_only_happens_at_hubs() {
    use anabios_core::hub::TradeHub;
    use anabios_core::prelude_test::Vec2;

    // Minimal 2-agent world with resources on and one hub at the origin.
    let build = |on_hub: bool| {
        let toml = "name=\"t\"\nseed=1\nworld_size=256\nresources_enabled=true\n[[agents]]\narchetype=\"grazer\"\ncount=2\n";
        let mut w = Scenario::parse_toml(toml).expect("parse").instantiate();
        w.trade_hubs = vec![TradeHub { pos: Vec2::new(0.0, 0.0), cell: 0, goods: vec![] }];
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        let (a, b) = (ids[0] as usize, ids[1] as usize);
        // Complementary inventories so a swap is mutually beneficial.
        w.agents.inventory[a] = [4.0, 0.0, 0.0, 0.0];
        w.agents.inventory[b] = [0.0, 4.0, 0.0, 0.0];
        // Species must differ for cross-species trade; force it.
        w.agents.species_id[b] = w.agents.species_id[a] + 1;
        // Place them adjacent, either on the hub or far away.
        let p = if on_hub { Vec2::new(1.0, 0.0) } else { Vec2::new(120.0, 120.0) };
        w.agents.position[a] = p;
        w.agents.position[b] = p + Vec2::new(1.0, 0.0);
        w.agents.anchor[a] = p;
        w.agents.anchor[b] = p + Vec2::new(1.0, 0.0);
        w
    };

    let mut off_hub = build(false);
    let before = off_hub.total_trades;
    for _ in 0..30 {
        step(&mut off_hub);
    }
    assert_eq!(off_hub.total_trades, before, "no trade away from hubs");

    let mut on_hub = build(true);
    for _ in 0..30 {
        step(&mut on_hub);
    }
    assert!(on_hub.total_trades > 0, "trade must occur at a hub");
}
```

If `iter_alive`, `species_id`, or `total_trades` field names differ, adjust to the actual `agents`/`World` API discovered while implementing — the assertions (no trade off-hub, trade on-hub) are what matter.

- [ ] **Step 2: Run the test to verify it fails.**

Run: `cd crates/anabios-core && cargo test --test trade trade_only_happens_at_hubs`
Expected: FAIL — today trade happens anywhere, so the off-hub case also trades (`total_trades` increases).

- [ ] **Step 3: Add the proximity guard.** In `crates/anabios-core/src/interact.rs`, inside `trade_pass`, at the very top of the `for &id in alive_ids { ... }` loop body (right after `let i = id as usize;`), add:

```rust
        // Trade only at hubs: an agent not near any predetermined hub cannot
        // barter this tick. Empty `trade_hubs` (resources off, or a hubless map)
        // therefore disables all trade — intended.
        if !crate::hub::near_any_hub(
            &world.trade_hubs,
            world.agents.position[i],
            world.world_size,
            crate::hub::HUB_TRADE_RANGE,
        ) {
            continue;
        }
```

- [ ] **Step 4: Run the test to verify it passes.**

Run: `cd crates/anabios-core && cargo test --test trade trade_only_happens_at_hubs`
Expected: PASS. Then `cargo fmt` + `cargo clippy -p anabios-core -- -D warnings`.

- [ ] **Step 5: Commit.**

```bash
git add crates/anabios-core/src/interact.rs crates/anabios-core/tests/trade.rs
git commit -m "feat(hub): restrict barter to hub proximity in trade_pass"
```

---

### Task 5: Regenerate goldens to the new baseline

**Files:**
- Modify (regenerated, not hand-edited): golden hash values in `crates/anabios-core/tests/*.rs` that pin `state_hash` values for `resources_enabled` scenarios and any snapshot-format-sensitive suite.

**Interfaces:** none (test-fixture regeneration only).

- [ ] **Step 1: See which goldens moved.** Run the full suite once and note failures:

Run: `cd crates/anabios-core && cargo test 2>&1 | tee /tmp/golden-before.txt | tail -40`
Expected: FAILs in the suites that pin hashes for trade/resources scenarios and format-version-sensitive tests (e.g. `trade`, `inventions`, `settlement_economy`, `cognition`, `determinism`, `all_scenarios`). Self-consistency suites (`save_load_roundtrip`, `serde_skip_audit`) should still PASS.

- [ ] **Step 2: Regenerate the pinned hashes.**

Run: `cd crates/anabios-core && UPDATE_HASHES=1 cargo test`
Expected: the `UPDATE_HASHES` branches rewrite/print the new pinned values (per-test convention — some rewrite the file, some print values to copy in; follow each failing test's message, e.g. `affect.rs:115`).

- [ ] **Step 3: Verify the new baseline is stable.**

Run: `cd crates/anabios-core && cargo test`
Expected: PASS across the suite. If any test only *prints* new values rather than rewriting, paste them into that test file and re-run.

- [ ] **Step 4: Sanity-check determinism explicitly.**

Run: `cd crates/anabios-core && cargo test --test determinism --test save_load_roundtrip`
Expected: PASS — the new field replays identically and survives round-trip.

- [ ] **Step 5: Commit.**

```bash
git add crates/anabios-core/tests
git commit -m "test(hub): regenerate goldens for trade-hubs behavior (FORMAT_VERSION 33)"
```

---

### Task 6: Godot bridge accessor `trade_hubs()`

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs` (add `#[func] fn trade_hubs`)

**Interfaces:**
- Consumes: `w.trade_hubs: Vec<TradeHub>` (each `pos: Vec2`, `goods: Vec<Good>`), `Good::index()`.
- Produces: GDScript-callable `sim.trade_hubs() -> Array` of `{ pos: Vector2, goods: PackedInt32Array }` (good indices Salt=0..Spice=3).

- [ ] **Step 1: Add the accessor.** In `crates/anabios-godot/src/lib.rs`, alongside the other `#[func]` accessors (e.g. next to `settlement_sites`, ~line 1242), add:

```rust
    /// Predetermined trade hubs: fixed marketplace positions with the goods that
    /// meet there. Static after worldgen; the viewer draws a building + goods
    /// ring at each. Empty unless the scenario enables resources.
    #[func]
    fn trade_hubs(&self) -> Array<VarDictionary> {
        let mut out = Array::new();
        let Some(w) = self.inner.as_ref() else {
            return out;
        };
        for h in &w.trade_hubs {
            let mut d = VarDictionary::new();
            d.set("pos", Vector2::new(h.pos.x, h.pos.y));
            let mut goods = PackedInt32Array::new();
            for g in &h.goods {
                goods.push(g.index() as i32);
            }
            d.set("goods", goods);
            out.push(&d);
        }
        out
    }
```

If `PackedInt32Array` / `VarDictionary` / `Vector2` are not already imported in this file, add them to the `use godot::prelude::*;` (or equivalent) imports — check the existing `settlement_sites` accessor, which already uses `VarDictionary` and `Vector2`.

- [ ] **Step 2: Build the extension to verify it compiles.**

Run: `cd crates/anabios-godot && cargo build`
Expected: PASS. Then `cargo fmt` + `cargo clippy -p anabios-godot -- -D warnings`.

- [ ] **Step 3: Commit.**

```bash
git add crates/anabios-godot/src/lib.rs
git commit -m "feat(godot): expose trade_hubs() to the viewer"
```

---

### Task 7: Viewer — trade-good icons + building-sprite polish

**Files:**
- Modify: `game/scripts/building_sprites.gd` (add 4 goods-icon builders; polish `_BLOCKS`)
- Test: `game/scripts/test_building_sprites.gd` (extend the existing sprite self-test)

**Interfaces:**
- Consumes: `ApeSprites._build_cell(blocks)` (existing painter); `ApeSprites.PAL` palette chars.
- Produces:
  - `const GOOD_NAMES: PackedStringArray` (Salt/Obsidian/Amber/Spice, index-aligned to sim Good indices)
  - `static func build_good_image(good_idx: int) -> Image`
  - `static func build_good(good_idx: int) -> ImageTexture`
  - `const GOOD_COUNT := 4`

- [ ] **Step 1: Add goods icons.** In `game/scripts/building_sprites.gd`, after the existing building helpers, add a 4-entry goods block-art table and builders. Icons are ~12×12 within the shared cell; pick palette chars already defined in `ApeSprites.PAL` (Salt = pale/white crystal, Obsidian = dark shard, Amber = orange gem, Spice = red-brown pile):

```gdscript
const GOOD_COUNT := 4
const GOOD_NAMES: PackedStringArray = ["Salt", "Obsidian", "Amber", "Spice"]

# 16x16 goods icons, indexed by sim Good index (Salt=0..Spice=3). Small, centered
# emblems drawn with the shared ApeSprites cell painter (auto 1px outline).
const _GOOD_BLOCKS: Array = [
	# SALT — white crystal cluster
	[[6, 5, 4, 6, "W"], [7, 4, 2, 1, "w"], [5, 8, 1, 2, "W"], [10, 8, 1, 2, "W"]],
	# OBSIDIAN — black glass shard
	[[7, 4, 3, 8, "K"], [6, 6, 1, 4, "d"], [10, 7, 1, 3, "d"]],
	# AMBER — orange gem
	[[6, 6, 4, 4, "o"], [7, 5, 2, 1, "y"], [6, 9, 4, 1, "O"], [8, 6, 1, 1, "y"]],
	# SPICE — red-brown mound with specks
	[[5, 9, 6, 3, "r"], [6, 8, 4, 1, "R"], [7, 10, 1, 1, "y"], [9, 10, 1, 1, "y"]],
]


static func build_good_image(good_idx: int) -> Image:
	var img: Image = ApeSprites._build_cell(_GOOD_BLOCKS[good_idx])
	img.flip_y()
	return img


static func build_good(good_idx: int) -> ImageTexture:
	return ImageTexture.create_from_image(build_good_image(good_idx))
```

If any palette char above (`W w K d o y O r R`) is not in `ApeSprites.PAL`, substitute the nearest existing one — check `ape_sprites.gd` for the `PAL` dictionary keys before finalizing.

- [ ] **Step 2: Polish existing building sprites.** Still in `building_sprites.gd`, revise the `_BLOCKS` entries for clearer silhouettes at map scale (art-only; do NOT change the enum, `NAMES`, `INVENTION_BUILDING`, or any function). Focus on the trade pair first — `MARKET` (make the striped awning + stall read as a marketplace) and `WAREHOUSE` (bigger doors, clear stacked crates) — then tighten any tech sprite whose shape is muddy. Keep every rect within the 16×16 cell.

- [ ] **Step 3: Extend the sprite self-test.** In `game/scripts/test_building_sprites.gd`, add a check that every building AND every good builds a non-empty texture of the expected size:

```gdscript
	for k in Buildings.KIND_COUNT:
		var tex := Buildings.build(k)
		assert(tex != null and tex.get_width() == 16 and tex.get_height() == 16)
	for g in Buildings.GOOD_COUNT:
		var gtex := Buildings.build_good(g)
		assert(gtex != null and gtex.get_width() == 16 and gtex.get_height() == 16)
	print("building_sprites OK")
```

Match the exact assert/reporting style already used in that file (adapt if it uses a `_run()` harness rather than top-level code).

- [ ] **Step 4: Run the sprite self-test headless.**

Run: `cd game && godot --headless -s scripts/test_building_sprites.gd --quit-after 1`
Expected: prints `building_sprites OK` (or the file's existing success line) with no errors. If the project runs GDScript tests via a runner, use that instead (check for `test/test_runner.gd`).

- [ ] **Step 5: Commit.**

```bash
git add game/scripts/building_sprites.gd game/scripts/test_building_sprites.gd
git commit -m "feat(viewer): trade-good icons + polish building sprites"
```

---

### Task 8: Viewer — hub layer rendering + scene wiring

**Files:**
- Create: `game/scripts/hub_layer.gd`
- Modify: `game/scripts/main.gd` (instantiate the layer, ~line 204 next to `_settlement_layer`)

**Interfaces:**
- Consumes: `sim.trade_hubs()` (Task 6) → `Array` of `{ pos: Vector2, goods: PackedInt32Array }`; `sim.world_size()`, `sim.market_colors()`, `sim.resources_active()`, `sim.biome_resolution()`; `Buildings.build(kind)`, `Buildings.MARKET`, `Buildings.WAREHOUSE`, `Buildings.build_good(idx)`, `Buildings.market_cell(pos, world, res)`.
- Produces: a `HubLayer` Node2D drawn above agents with a MultiMesh per building + per good, torus-wrapped.

- [ ] **Step 1: Create the hub layer.** Write `game/scripts/hub_layer.gd`. Hubs are static after worldgen, so build the transforms once when `trade_hubs()` first returns data, then refresh only the Market→Warehouse choice periodically from market heat. Mirror `settlement_layer.gd`'s plain (no-shader) MultiMesh + 9-way wrap-clone pattern:

```gdscript
extends Node2D

# Trade-hub layer: draws a marketplace building at each predetermined hub
# (Warehouse where market heat is high, Market otherwise) plus a small ring of
# trade-good icons for the goods that meet there. Hubs are worldgen fixtures —
# positions never move — so geometry is built once, then only the building
# choice refreshes with the live market field. Presentation over read-only sim
# state; plain no-shader MultiMesh (Metal-safe), same as settlement_layer.

const ApeSprites = preload("res://scripts/ape_sprites.gd")
const Buildings = preload("res://scripts/building_sprites.gd")

const HUB_SCALE := 20.0
const GOOD_SCALE := 9.0
const GOOD_RING_RADIUS := 24.0
const REDRAW_EVERY := 30

var _market_mmi: MultiMeshInstance2D
var _warehouse_mmi: MultiMeshInstance2D
var _good_mmis: Array[MultiMeshInstance2D] = []
var _hubs: Array = []
var _frame: int = REDRAW_EVERY - 1

@onready var sim = get_node("/root/Main/Simulation")


func _ready() -> void:
	_market_mmi = _make_layer("Hub_Market", Buildings.build(Buildings.MARKET))
	_warehouse_mmi = _make_layer("Hub_Warehouse", Buildings.build(Buildings.WAREHOUSE))
	for g in Buildings.GOOD_COUNT:
		_good_mmis.append(_make_layer("Hub_Good_%s" % Buildings.GOOD_NAMES[g], Buildings.build_good(g)))
	_make_wrap_clones()


func _make_layer(pname: String, tex: ImageTexture) -> MultiMeshInstance2D:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_2D
	mm.use_colors = true
	mm.mesh = QuadMesh.new()
	var mmi := MultiMeshInstance2D.new()
	mmi.name = pname
	mmi.multimesh = mm
	mmi.texture = tex
	mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	mmi.z_index = 1
	add_child(mmi)
	return mmi


func _make_wrap_clones() -> void:
	var world: float = sim.world_size()
	for src in [_market_mmi, _warehouse_mmi] + _good_mmis:
		for gy in range(-1, 2):
			for gx in range(-1, 2):
				if gx == 0 and gy == 0:
					continue
				var clone := MultiMeshInstance2D.new()
				clone.multimesh = src.multimesh
				clone.texture = src.texture
				clone.texture_filter = src.texture_filter
				clone.z_index = src.z_index
				clone.position = Vector2(gx * world, gy * world)
				add_child(clone)


func _process(_delta: float) -> void:
	_frame += 1
	if _frame % REDRAW_EVERY != 0:
		return
	if _hubs.is_empty():
		_hubs = sim.trade_hubs()
		if _hubs.is_empty():
			return
	_redraw()


func _redraw() -> void:
	var market_field: PackedColorArray = (
		sim.market_colors() if sim.resources_active() else PackedColorArray()
	)
	var res := int(sim.biome_resolution())
	var world_sz: float = sim.world_size()
	var market_xf: Array = []
	var warehouse_xf: Array = []
	var good_xf: Array = []
	for g in Buildings.GOOD_COUNT:
		good_xf.append([])
	for hub in _hubs:
		var pos: Vector2 = hub["pos"]
		# Busy hub (hot market cell) -> warehouse, else market.
		var busy := false
		if not market_field.is_empty():
			var ci := Buildings.market_cell(pos, world_sz, res)
			if ci >= 0 and ci < market_field.size():
				busy = market_field[ci].r >= Buildings.MARKET_MIN
		var xf := Transform2D(0.0, Vector2(HUB_SCALE, HUB_SCALE), 0.0, pos)
		if busy:
			warehouse_xf.append(xf)
		else:
			market_xf.append(xf)
		# Goods ring: one icon per good that meets at this hub.
		var goods: PackedInt32Array = hub["goods"]
		for slot in goods.size():
			var gi: int = goods[slot]
			var ang: float = TAU * float(slot) / float(max(goods.size(), 1))
			var gp := pos + Vector2.from_angle(ang) * GOOD_RING_RADIUS
			good_xf[gi].append(Transform2D(0.0, Vector2(GOOD_SCALE, GOOD_SCALE), 0.0, gp))
	_write(_market_mmi.multimesh, market_xf)
	_write(_warehouse_mmi.multimesh, warehouse_xf)
	for g in Buildings.GOOD_COUNT:
		_write(_good_mmis[g].multimesh, good_xf[g])


func _write(mm: MultiMesh, xfs: Array) -> void:
	var m := xfs.size()
	if m > mm.instance_count:
		mm.instance_count = m
	mm.visible_instance_count = m
	for i in m:
		mm.set_instance_transform_2d(i, xfs[i])
		mm.set_instance_color(i, Color(1, 1, 1))
```

Note: `Buildings.MARKET_MIN` already exists (`building_sprites.gd:206`). Verify `sim.biome_resolution()` and `sim.market_colors()` accessor names against `settlement_layer.gd` (it uses both) and adjust if renamed.

- [ ] **Step 2: Wire the layer into the scene.** In `game/scripts/main.gd`, right after the settlement layer is added (lines 204-207), add:

```gdscript
	var hub_layer = preload("res://scripts/hub_layer.gd").new()
	hub_layer.name = "HubLayer"
	add_child(hub_layer)
	move_child(hub_layer, module_layers.get_index())
```

- [ ] **Step 3: Headless boot check.** Confirm the scene loads and the layer runs without error:

Run: `cd game && godot --headless res://scenes/main.tscn --quit-after 120 2>&1 | tail -30`
Expected: no `SCRIPT ERROR` / `Parse Error` lines; the run reaches quit cleanly. (Load a resources-enabled scenario in the menu path if the default scene doesn't auto-load one — check `main.gd` around line 94 `load_scenario_with_seed`.)

- [ ] **Step 4: Visual confirmation (optional but recommended).** Launch the app on a trade scenario (`geographic-trade.toml` or `biome-trade.toml` — NOT `inventions.toml`, which leaves `resources_enabled` off and so places no hubs) and confirm marketplaces with goods rings appear at hub crossroads and agents visibly gather there. Use the project's run path (see the `run` skill / `main.gd` menu).

- [ ] **Step 5: Commit.**

```bash
git add game/scripts/hub_layer.gd game/scripts/main.gd
git commit -m "feat(viewer): hub layer — marketplaces + goods rings at trade hubs"
```

---

## Self-Review

**Spec coverage:**
- Worldgen border-diversity placement (Approach A) → Task 1 (`place_trade_hubs`) + Task 2 (scenario wiring).
- Trade-motivated hub-seeking movement → Task 3.
- Trade only at hubs → Task 4.
- Behavior-first, reset goldens, FORMAT_VERSION bump → Task 2 (bump) + Task 5 (regen).
- Marketplace sprite at hubs (reuse Market/Warehouse) → Task 8.
- 4 goods icons → Task 7.
- Polish existing 12 sprites → Task 7 Step 2.
- Godot accessor → Task 6.
- Hubs turn on with `resources_enabled` → Task 2 Step 6 (gated on `w.resources_enabled`), exercised on `biome-trade.toml`/`geographic-trade.toml` (`inventions.toml` leaves resources off → no hubs). A dedicated `trade-hubs.toml` showcase was added as a follow-up.
- Unit tests (placement determinism/spacing/cap, single-good empty, motive, direction+wrap, proximity), trade-at-hub behavior, save/load round-trip → Tasks 1–4.
- Godot headless boot renders hub layer → Task 8 Step 3.
All spec sections map to a task. No gaps.

**Placeholder scan:** No "TBD"/"handle edge cases"/"similar to Task N". Each code step carries full code. Two `if names differ, adjust` notes (Task 3 test `prelude_test::Vec2`, Task 7 palette chars, Task 8 accessor names) are explicit verification instructions, not missing content — they name the exact fallback.

**Type consistency:** `TradeHub { pos, cell, goods }` used identically across Tasks 1/2/4 tests and Task 6 accessor. `place_trade_hubs(&BiomeField)`, `best_hub_direction(&[TradeHub], Vec2, f32)`, `near_any_hub(&[TradeHub], Vec2, f32, f32)`, `has_trade_motive(&[f32; GOOD_COUNT])` signatures match every call site. Constants `HUB_PULL`/`HUB_TRADE_RANGE` referenced consistently. Viewer `GOOD_COUNT`/`GOOD_NAMES`/`build_good` defined in Task 7, consumed in Task 8. `FORMAT_VERSION = 33` set once (Task 2).
