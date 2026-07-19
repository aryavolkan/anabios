# Biome Trade Goods Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add four biome-linked natural resources that spawn on the map, are harvested and carried by agents, swapped bilaterally between other-species agents, and required as a balanced dowry basket to reproduce.

**Architecture:** Copy the existing `carcass` precedent for map entities (`Vec<Resource>` + a `#[serde(skip)]` spatial hash rebuilt per tick) and the `invention` precedent for opt-in subsystems (a `World::resources_enabled` bool that draws zero RNG and adds zero behavior when off). Harvest and trade are new passes in `interact.rs`; the dowry gate hooks into `reproduce.rs`. All new persistent state is added in one version-bump task so the golden hashes are regenerated exactly once.

**Tech Stack:** Rust, `anabios-core` crate. Serde/bincode snapshots. `cargo test -p anabios-core`.

## Global Constraints

- **Determinism (hard requirement).** Single RNG stream `world.rng` (`Xoshiro256PlusPlus`); no `thread_rng`, no wall clock. Iterate agents via `world.agents.iter_alive()` (ascending id) only. Any cross-agent map uses `BTreeMap`/arrays, never `HashMap`.
- **Zero-RNG-when-off.** Every new behavior is gated on `world.resources_enabled`. When it is `false`, no new RNG is drawn and no agent state changes, so the *trajectory* of every existing scenario is byte-identical.
- **Serialized layout.** bincode is NOT self-describing. Adding/removing/reordering any serialized field on `World` or a nested serialized type changes the byte layout and MUST bump `snapshot::FORMAT_VERSION`. New enum variants (e.g. `EventType`, `Good`) MUST be appended at the end.
- **Golden hashes.** Two tests hardcode state hashes: `crates/anabios-core/tests/determinism.rs` (`GOLDEN`, minimal.toml) and `crates/anabios-core/tests/inventions.rs` (`INVENTIONS_GOLDEN`). Both move when serialized layout grows; regenerate with `UPDATE_HASHES=1` and document the bump. This happens exactly once, in Task 2.
- **Fixed constants (v1 balance; tuning deferred):** `GOOD_COUNT = 4`, `RESOURCE_STEP_INTERVAL = 10`, `NODE_SPAWN_ATTEMPTS = 64`, `NODE_TARGET_PER_GOOD = 40`, `NODE_MAX_TOTAL = 400`, `NODE_START_AMOUNT = 20.0`, `HARVEST_RANGE = 2.0`, `HARVEST_RATE = 1.0`, `INVENTORY_BASE_CAP = 12.0`, `INVENTORY_STORAGE_BONUS = 12.0`, `TRADE_RANGE = 2.0`, `TRADE_UNIT = 1.0`, `DOWRY_REQ = 2.0`.

---

## File Structure

- **Create** `crates/anabios-core/src/resource.rs` — `Good` enum, `Resource` struct, all constants, `Good::from_terrain`, `resource_step`, `want`, inventory-cap helper. One cohesive module for the whole subsystem's data + spawn logic.
- **Modify** `crates/anabios-core/src/lib.rs` — register `pub mod resource;`.
- **Modify** `crates/anabios-core/src/agent.rs` — `AgentBuffers.inventory` SoA array + spawn maintenance.
- **Modify** `crates/anabios-core/src/world.rs` — `resources`, `resource_spatial`, `resources_enabled` fields + init in `new`/`with_dims`.
- **Modify** `crates/anabios-core/src/snapshot.rs` — bump `FORMAT_VERSION` 5→6.
- **Modify** `crates/anabios-core/src/codex/mod.rs` — `CodexState.first_cross_species_trade` bool (Task 2); `EventType::ResourceTraded`/`DowryBirth` (Task 8).
- **Modify** `crates/anabios-core/src/interact.rs` — `harvest_pass` + `trade_pass`, wired into `interact_all`.
- **Modify** `crates/anabios-core/src/tick.rs` — call `resource::resource_step` on the biome cadence.
- **Modify** `crates/anabios-core/src/reproduce.rs` — dowry gate + consumption.
- **Modify** `crates/anabios-core/src/scenario.rs` — `resources_enabled` flag + wiring in `instantiate`.
- **Create** `scenarios/biome-trade.toml` — a runnable trade scenario.
- **Modify** `crates/anabios-core/tests/determinism.rs`, `crates/anabios-core/tests/inventions.rs` — regenerated golden hashes (Task 2).
- **Create** `crates/anabios-core/tests/trade.rs` — integration determinism + regression test (Task 9).

---

## Task 1: Resource module foundation (`Good`, `Resource`, constants, terrain map)

**Files:**
- Create: `crates/anabios-core/src/resource.rs`
- Modify: `crates/anabios-core/src/lib.rs` (add `pub mod resource;`)
- Test: unit tests inside `resource.rs`

**Interfaces:**
- Consumes: `crate::biome::TerrainType`, `crate::prelude::Vec2`.
- Produces:
  - `pub const GOOD_COUNT: usize = 4;`
  - `pub enum Good { Salt, Obsidian, Amber, Spice }` with `Good::ALL: [Good; GOOD_COUNT]`, `Good::index(self) -> usize`, `Good::from_terrain(TerrainType) -> Option<Good>`.
  - `pub struct Resource { pub pos: Vec2, pub kind: Good, pub amount: f32 }`
  - All constants listed in Global Constraints.
  - `pub fn want(inventory: &[f32; GOOD_COUNT], k: usize) -> f32` = `1.0 / (1.0 + inventory[k])`.
  - `pub fn inventory_total(inv: &[f32; GOOD_COUNT]) -> f32`.

- [ ] **Step 1: Write the failing test**

Add to a new file `crates/anabios-core/src/resource.rs`:

```rust
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
```

- [ ] **Step 2: Register the module**

In `crates/anabios-core/src/lib.rs`, add the module declaration next to the other `pub mod` lines (keep alphabetical grouping with neighbors like `pub mod reproduce;`):

```rust
pub mod resource;
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test -p anabios-core --lib resource::`
Expected: PASS (3 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/anabios-core/src/resource.rs crates/anabios-core/src/lib.rs
git commit -m "feat(resource): Good enum, Resource struct, terrain map + constants"
```

---

## Task 2: Persistent state + FORMAT_VERSION bump (the single hash-regeneration task)

Adds every new serialized field at once (`AgentBuffers.inventory`, `World.{resources,resources_enabled}`, `CodexState.first_cross_species_trade`), the `#[serde(skip)]` `World.resource_spatial`, bumps `FORMAT_VERSION`, and regenerates both golden-hash tests. After this task the codebase compiles and behaves identically with the feature off; only serialized bytes (and therefore the golden hashes) changed.

**Files:**
- Modify: `crates/anabios-core/src/agent.rs` (`AgentBuffers` struct + `spawn`)
- Modify: `crates/anabios-core/src/world.rs` (fields + `new` + `with_dims`)
- Modify: `crates/anabios-core/src/codex/mod.rs` (`CodexState` field)
- Modify: `crates/anabios-core/src/snapshot.rs` (`FORMAT_VERSION`)
- Modify: `crates/anabios-core/tests/determinism.rs`, `crates/anabios-core/tests/inventions.rs` (regenerated hashes)
- Test: `crates/anabios-core/src/agent.rs` unit test; existing snapshot roundtrip test.

**Interfaces:**
- Consumes: `crate::resource::{Resource, GOOD_COUNT}` (Task 1).
- Produces:
  - `AgentBuffers.inventory: Vec<[f32; crate::resource::GOOD_COUNT]>` (public field, one entry per agent slot, all-zero on spawn).
  - `World.resources: Vec<crate::resource::Resource>` (serialized).
  - `World.resource_spatial: crate::spatial::UniformSpatialHash` (`#[serde(skip)]`).
  - `World.resources_enabled: bool` (`#[serde(default)]`).
  - `CodexState.first_cross_species_trade: bool`.

- [ ] **Step 1: Write the failing test** (agent inventory is zeroed on spawn)

Add to the `tests` module in `crates/anabios-core/src/agent.rs`:

```rust
    #[test]
    fn spawn_zeroes_inventory() {
        let mut a = AgentBuffers::new();
        let id = a.spawn(
            Vec2::ZERO,
            neutral(),
            1,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            Program::empty(),
        );
        assert_eq!(a.inventory[id as usize], [0.0; crate::resource::GOOD_COUNT]);
        assert_eq!(a.inventory.len(), a.capacity());
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p anabios-core --lib agent::tests::spawn_zeroes_inventory`
Expected: FAIL to compile ("no field `inventory`").

- [ ] **Step 3: Add the `inventory` SoA array**

In `crates/anabios-core/src/agent.rs`, add the field to `AgentBuffers` immediately after `meme_vector`:

```rust
    /// Per-agent trade-good holdings, indexed by `crate::resource::Good::index`.
    /// Zeroed on spawn; only the resource subsystem (harvest/trade/dowry)
    /// mutates it, and only when `World::resources_enabled` is on.
    pub inventory: Vec<[f32; crate::resource::GOOD_COUNT]>,
```

In `spawn`, set the reused slot (the `if let Some(id) = self.free_list.pop()` branch), right after the `self.meme_vector[i] = ...` line:

```rust
            self.inventory[i] = [0.0; crate::resource::GOOD_COUNT];
```

And in the extend branch, right after `self.meme_vector.push(...)`:

```rust
            self.inventory.push([0.0; crate::resource::GOOD_COUNT]);
```

- [ ] **Step 4: Run the agent test to verify it passes**

Run: `cargo test -p anabios-core --lib agent::tests::spawn_zeroes_inventory`
Expected: PASS.

- [ ] **Step 5: Add `World` fields**

In `crates/anabios-core/src/world.rs`, add to the `World` struct after the `season_period` field (before `max_population`, keeping the `#[serde(skip)]` scratch group at the bottom untouched):

```rust
    /// Discrete trade-good nodes on the map (biome trade goods feature).
    /// Empty and inert unless `resources_enabled`. Serialized.
    #[serde(default)]
    pub resources: Vec<crate::resource::Resource>,
    /// When true, the biome-trade-goods economy is active: nodes spawn, agents
    /// harvest and trade them, and reproduction requires a dowry basket. Off by
    /// default; opt-in per scenario. Draws zero RNG and changes no state when off.
    #[serde(default)]
    pub resources_enabled: bool,
```

Add the skip-scratch spatial hash next to `carcass_spatial`:

```rust
    /// Spatial hash over `resources` (indexed by node index), rebuilt each tick
    /// in `harvest_pass`. `#[serde(skip)]` — reconstructed on load.
    #[serde(skip)]
    pub resource_spatial: UniformSpatialHash,
```

In `World::new`, initialize the three new fields (place the two serialized ones near `carcasses: Vec::new(),` and `season_period: 0,`, and the skip one near `carcass_spatial:`):

```rust
            resources: Vec::new(),
            resources_enabled: false,
```

```rust
            resource_spatial: UniformSpatialHash::with_dims(
                crate::biome::WORLD_SIZE_DEFAULT,
                crate::spatial::HASH_RES_DEFAULT,
            ),
```

In `World::with_dims`, after the `w.carcass_spatial = ...` line, add:

```rust
        w.resource_spatial = crate::spatial::UniformSpatialHash::with_dims(world_size, hash_res);
```

- [ ] **Step 6: Add the `CodexState` latch field**

In `crates/anabios-core/src/codex/mod.rs`, add to `CodexState` immediately before the `events` field:

```rust
    /// Latch: the first cross-species `ResourceTraded` event has been emitted
    /// (biome trade goods feature). Kept with the other one-shot latches.
    pub first_cross_species_trade: bool,
```

(The `#[derive(Default)]` on `CodexState` covers the new field automatically.)

- [ ] **Step 7: Bump `FORMAT_VERSION`**

In `crates/anabios-core/src/snapshot.rs`, change the constant and add a doc line:

```rust
/// v6: biome trade goods — AgentBuffers.inventory, World.{resources,
///     resources_enabled}, CodexState.first_cross_species_trade. Behavior
///     unchanged with resources_enabled off; only serialized layout grew.
pub const FORMAT_VERSION: u32 = 6;
```

- [ ] **Step 8: Build and confirm only the golden-hash tests fail**

Run: `cargo test -p anabios-core`
Expected: compiles; `snapshot::tests::roundtrip_preserves_state` and `loaded_world_continues_bit_identically` PASS (inventory + resources roundtrip through bincode; `resource_spatial` rebuilds via Default). The only failures are `determinism.rs::minimal_scenario_matches_golden_hashes` and `inventions.rs::inventions_scenario_matches_golden_hashes` (hash drift — expected, because the serialized `World` layout grew).

- [ ] **Step 9: Regenerate the golden hashes**

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --test determinism -- --nocapture`
Copy the three printed `(tick, 0x…)` lines into `GOLDEN` in `crates/anabios-core/tests/determinism.rs`, and add a changelog comment above `GOLDEN`:

```rust
// Refreshed 2026-07-19: biome trade goods added AgentBuffers.inventory,
// World.{resources,resources_enabled}, CodexState.first_cross_species_trade.
// Flag off = byte-identical trajectory; only serialized layout grew, so all
// three hashes moved.
```

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --test inventions -- --nocapture`
Copy the three printed lines into `INVENTIONS_GOLDEN` in `crates/anabios-core/tests/inventions.rs`, and add an equivalent one-line changelog comment above it.

- [ ] **Step 10: Run the full test suite to verify green**

Run: `cargo test -p anabios-core`
Expected: PASS (all tests, including the two regenerated golden-hash tests and `all_scenarios`).

- [ ] **Step 11: Commit**

```bash
git add crates/anabios-core/src/agent.rs crates/anabios-core/src/world.rs \
        crates/anabios-core/src/codex/mod.rs crates/anabios-core/src/snapshot.rs \
        crates/anabios-core/tests/determinism.rs crates/anabios-core/tests/inventions.rs
git commit -m "feat(resource): persistent inventory + resource state, FORMAT_VERSION 6"
```

---

## Task 3: Resource spawning (`resource_step`) + tick wiring

**Files:**
- Modify: `crates/anabios-core/src/resource.rs` (add `resource_step` + per-good counts)
- Modify: `crates/anabios-core/src/tick.rs` (call it on the biome cadence)
- Test: unit tests in `resource.rs`

**Interfaces:**
- Consumes: `World.{rng, world_size, biome, resources, resources_enabled}`, `Good::from_terrain`.
- Produces: `pub fn resource_step(world: &mut World)`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/anabios-core/src/resource.rs` (extend the existing `use` lines with `use crate::world::World;`):

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p anabios-core --lib resource::tests::resource_step`
Expected: FAIL to compile ("cannot find function `resource_step`").

- [ ] **Step 3: Implement `resource_step`**

Add to `crates/anabios-core/src/resource.rs` (module body, not the test module):

```rust
use crate::world::World;

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
```

Note the `use crate::world::World;` at module scope replaces the test-only import; keep a single module-level `use` and drop the duplicate inside the test module if the compiler warns.

- [ ] **Step 4: Wire it into the tick**

In `crates/anabios-core/src/tick.rs`, inside the `if world.tick.is_multiple_of(BIOME_STEP_INTERVAL)` block at the end of `step`, after the biome regrowth branch closes but still inside the interval block, add the resource step. Replace the existing block:

```rust
    // Stage 10: periodic biome regrowth (+ recolonization in a living biome).
    if world.tick.is_multiple_of(BIOME_STEP_INTERVAL) {
        if world.living_biome {
            world.biome.recolonize_step();
        }
        if world.season_period > 0 {
            let phase = crate::biome::season_phase(world.tick, world.season_period);
            world.biome.regrow_step_seasonal(phase);
        } else {
            world.biome.regrow_step();
        }
        // Stage 10b: resource node spawn/cleanup (opt-in; no-op when off).
        crate::resource::resource_step(world);
    }
```

(`RESOURCE_STEP_INTERVAL` equals `BIOME_STEP_INTERVAL` = 10, so gating on the same interval is exact.)

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p anabios-core --lib resource::`
Expected: PASS (all resource unit tests).

- [ ] **Step 6: Confirm determinism is intact**

Run: `cargo test -p anabios-core --test determinism`
Expected: PASS (minimal.toml never enables resources, so `resource_step` early-returns and draws no RNG; the golden hashes from Task 2 still hold).

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/resource.rs crates/anabios-core/src/tick.rs
git commit -m "feat(resource): resource_step spawns biome-linked nodes on the map"
```

---

## Task 4: Harvest pass

**Files:**
- Modify: `crates/anabios-core/src/interact.rs` (add `harvest_pass`, call from `interact_all`)
- Modify: `crates/anabios-core/src/resource.rs` (add `carrying_cap` helper)
- Test: unit test in `interact.rs`

**Interfaces:**
- Consumes: `World.{resources, resource_spatial, agents}`, `resource::{HARVEST_RANGE, HARVEST_RATE, carrying_cap, inventory_total}`, `spatial::torus_distance`.
- Produces: `harvest_pass(world, alive_ids)` (private), `resource::carrying_cap(modules) -> f32`.

- [ ] **Step 1: Add the carrying-cap helper (with test)**

In `crates/anabios-core/src/resource.rs` module body:

```rust
/// Per-agent carrying capacity: a flat base, plus a bonus for agents that
/// carry a `Storage` module (reuses the existing morphology).
pub fn carrying_cap(modules: &crate::module::ModuleList) -> f32 {
    let mut cap = INVENTORY_BASE_CAP;
    if crate::module::has(modules, crate::module::ModuleType::Storage) {
        cap += INVENTORY_STORAGE_BONUS;
    }
    cap
}
```

Add to the `resource.rs` test module:

```rust
    #[test]
    fn storage_module_raises_carrying_cap() {
        let base = crate::module::starter_kit();
        let mut with_storage = base.clone();
        with_storage.push(crate::module::Module::Storage { capacity: 1.0 });
        assert_eq!(carrying_cap(&base), INVENTORY_BASE_CAP);
        assert_eq!(carrying_cap(&with_storage), INVENTORY_BASE_CAP + INVENTORY_STORAGE_BONUS);
    }
```

- [ ] **Step 2: Write the failing harvest test**

Add to the `tests` module in `crates/anabios-core/src/interact.rs` (create the module if absent, mirroring other src test modules; add `use crate::world::World; use crate::prelude::Vec2; use crate::genome::Genome;`):

```rust
    #[test]
    fn harvest_fills_inventory_and_depletes_node() {
        use crate::resource::{Good, Resource, HARVEST_RATE};
        let mut w = World::new(3);
        w.resources_enabled = true;
        let pos = Vec2::new(200.0, 200.0);
        let id = w.spawn_agent(pos, Genome::neutral());
        w.resources.push(Resource { pos, kind: Good::Salt, amount: 5.0 });
        // Build the agent spatial hash (harvest_pass rebuilds resource_spatial itself).
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let alive: Vec<u32> = w.agents.iter_alive().collect();
        harvest_pass(&mut w, &alive);

        assert!((w.agents.inventory[id as usize][Good::Salt.index()] - HARVEST_RATE).abs() < 1e-6);
        assert!((w.resources[0].amount - (5.0 - HARVEST_RATE)).abs() < 1e-6);
    }

    #[test]
    fn harvest_respects_carrying_cap() {
        use crate::resource::{carrying_cap, Good, Resource};
        let mut w = World::new(3);
        w.resources_enabled = true;
        let pos = Vec2::new(200.0, 200.0);
        let id = w.spawn_agent(pos, Genome::neutral());
        let cap = carrying_cap(&w.agents.modules[id as usize]);
        // Pre-fill to the cap; no room to harvest more.
        w.agents.inventory[id as usize][Good::Amber.index()] = cap;
        w.resources.push(Resource { pos, kind: Good::Salt, amount: 5.0 });
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let alive: Vec<u32> = w.agents.iter_alive().collect();
        harvest_pass(&mut w, &alive);

        assert_eq!(w.agents.inventory[id as usize][Good::Salt.index()], 0.0, "at cap: no harvest");
        assert_eq!(w.resources[0].amount, 5.0, "node untouched");
    }
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p anabios-core --lib interact::tests::harvest`
Expected: FAIL to compile ("cannot find function `harvest_pass`").

- [ ] **Step 4: Implement `harvest_pass` and wire it in**

In `crates/anabios-core/src/interact.rs`, add the pass (mirrors `scavenge_pass`):

```rust
/// Harvest: any agent standing within `HARVEST_RANGE` of a resource node pulls
/// up to `HARVEST_RATE` of its good into inventory, bounded by carrying
/// capacity, depleting the node. Nodes are indexed in `resource_spatial`
/// (rebuilt here) so each agent's search touches only nearby nodes.
fn harvest_pass(world: &mut World, alive_ids: &[u32]) {
    use crate::resource::{carrying_cap, inventory_total, HARVEST_RANGE, HARVEST_RATE};
    if world.resources.is_empty() {
        return;
    }
    world.resource_spatial.rebuild_indexed(
        world.resources.len(),
        |ri| world.resources[ri].pos,
        |ri| world.resources[ri].amount > 0.0,
    );
    for &id in alive_ids {
        let i = id as usize;
        let cap = carrying_cap(&world.agents.modules[i]);
        let room = cap - inventory_total(&world.agents.inventory[i]);
        if room <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let mut best: Option<usize> = None;
        let mut best_d = HARVEST_RANGE;
        world.resource_spatial.query(pos, HARVEST_RANGE, |ri| {
            let ri = ri as usize;
            if world.resources[ri].amount <= 0.0 {
                return;
            }
            let d = crate::spatial::torus_distance(pos, world.resources[ri].pos, world.world_size);
            // Strict `<` plus lowest-index tie-break = deterministic nearest.
            if d < best_d || (d == best_d && best.is_some_and(|b| ri < b)) {
                best_d = d;
                best = Some(ri);
            }
        });
        if let Some(ri) = best {
            let taken = HARVEST_RATE.min(world.resources[ri].amount).min(room);
            if taken > 0.0 {
                let k = world.resources[ri].kind.index();
                world.agents.inventory[i][k] += taken;
                world.resources[ri].amount -= taken;
            }
        }
    }
}
```

Wire it into `interact_all` — call it right after `scavenge_pass`, gated so the flag-off path is byte-identical:

```rust
    scavenge_pass(world, &alive_ids);
    if world.resources_enabled {
        harvest_pass(world, &alive_ids);
    }
    deposit_pass(world, &alive_ids);
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p anabios-core --lib interact:: resource::`
Expected: PASS.

- [ ] **Step 6: Confirm determinism intact**

Run: `cargo test -p anabios-core --test determinism`
Expected: PASS (harvest_pass is skipped when the flag is off).

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/interact.rs crates/anabios-core/src/resource.rs
git commit -m "feat(resource): harvest_pass — agents gather biome-linked goods"
```

---

## Task 5: Trade pass (bilateral swap)

**Files:**
- Modify: `crates/anabios-core/src/interact.rs` (add `trade_pass`, call from `interact_all`)
- Test: unit tests in `interact.rs`

**Interfaces:**
- Consumes: `World.{agents, sensors}`, `resource::{want, TRADE_RANGE, TRADE_UNIT, GOOD_COUNT}`, `sense::{NO_NEIGHBOR_ID}`.
- Produces: `trade_pass(world, alive_ids)` (private), plus a pure helper `pick_swap(inv_a, inv_b) -> Option<(usize, usize)>` returning `(give_good, receive_good)` from A's perspective.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/anabios-core/src/interact.rs`:

```rust
    #[test]
    fn pick_swap_is_mutually_beneficial_and_complementary() {
        use crate::resource::GOOD_COUNT;
        // A rich in good 0, poor in good 1; B the mirror image.
        let mut a = [0.0f32; GOOD_COUNT];
        let mut b = [0.0f32; GOOD_COUNT];
        a[0] = 5.0; // A surplus Salt
        b[1] = 5.0; // B surplus Obsidian
        let (give, recv) = pick_swap(&a, &b).expect("a beneficial swap exists");
        assert_eq!(give, 0, "A gives its surplus good 0");
        assert_eq!(recv, 1, "A receives good 1 (B's surplus)");
    }

    #[test]
    fn pick_swap_returns_none_without_complementary_surplus() {
        use crate::resource::GOOD_COUNT;
        // Both empty → nothing to give.
        let a = [0.0f32; GOOD_COUNT];
        let b = [0.0f32; GOOD_COUNT];
        assert!(pick_swap(&a, &b).is_none());
    }

    #[test]
    fn trade_pass_swaps_and_conserves_units() {
        use crate::resource::{Good, GOOD_COUNT, TRADE_UNIT};
        let mut w = World::new(5);
        w.resources_enabled = true;
        let pos = Vec2::new(300.0, 300.0);
        let a = w.spawn_agent(pos, Genome::neutral());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
        // Different species so it counts as cross-species trade.
        w.agents.species_id[b as usize] = 1;
        w.agents.inventory[a as usize][Good::Salt.index()] = 5.0;
        w.agents.inventory[b as usize][Good::Obsidian.index()] = 5.0;

        // Sense fills nearest_other_id/dist (trade_pass reads those, like combat_pass).
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        crate::sense::sense_all(
            &w.agents, &w.biome, &w.pheromones, &w.spatial,
            &mut w.sensors, w.world_size,
        );

        let total_salt_before: f32 = (0..2).map(|id| w.agents.inventory[id][Good::Salt.index()]).sum();
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        trade_pass(&mut w, &alive);

        // A gave a unit of Salt, received a unit of Obsidian.
        assert!((w.agents.inventory[a as usize][Good::Salt.index()] - (5.0 - TRADE_UNIT)).abs() < 1e-6);
        assert!((w.agents.inventory[a as usize][Good::Obsidian.index()] - TRADE_UNIT).abs() < 1e-6);
        // Conservation of Salt across the two agents.
        let total_salt_after: f32 = (0..2).map(|id| w.agents.inventory[id][Good::Salt.index()]).sum();
        assert!((total_salt_before - total_salt_after).abs() < 1e-6);
        let _ = GOOD_COUNT;
    }

    #[test]
    fn trade_pass_skips_same_species() {
        use crate::resource::Good;
        let mut w = World::new(5);
        w.resources_enabled = true;
        let pos = Vec2::new(300.0, 300.0);
        let a = w.spawn_agent(pos, Genome::neutral());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
        // SAME species (both 0).
        w.agents.inventory[a as usize][Good::Salt.index()] = 5.0;
        w.agents.inventory[b as usize][Good::Obsidian.index()] = 5.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        crate::sense::sense_all(
            &w.agents, &w.biome, &w.pheromones, &w.spatial,
            &mut w.sensors, w.world_size,
        );
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        trade_pass(&mut w, &alive);
        assert_eq!(w.agents.inventory[a as usize][Good::Obsidian.index()], 0.0, "no same-species trade");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --lib interact::tests::trade interact::tests::pick_swap`
Expected: FAIL to compile ("cannot find function `pick_swap`/`trade_pass`").

- [ ] **Step 3: Implement `pick_swap` and `trade_pass`**

In `crates/anabios-core/src/interact.rs`:

```rust
/// Choose a mutually-beneficial one-for-one swap between two inventories, from
/// A's perspective: A gives good `give` (its lowest-want good it can spare a
/// `TRADE_UNIT` of that B still wants) and receives good `recv` (B's lowest-want
/// spare good that A wants). Returns `None` if no complementary, strictly-
/// beneficial swap exists. Ties in "lowest want" break toward the lower index.
fn pick_swap(
    inv_a: &[f32; crate::resource::GOOD_COUNT],
    inv_b: &[f32; crate::resource::GOOD_COUNT],
) -> Option<(usize, usize)> {
    use crate::resource::{want, GOOD_COUNT, TRADE_UNIT};
    // A's most-spareable good (lowest want) that it holds >= TRADE_UNIT of.
    let mut give: Option<usize> = None;
    let mut give_want = f32::INFINITY;
    for k in 0..GOOD_COUNT {
        if inv_a[k] >= TRADE_UNIT {
            let wk = want(inv_a, k);
            if wk < give_want {
                give_want = wk;
                give = Some(k);
            }
        }
    }
    let give = give?;
    // B's most-spareable good (lowest want) that it holds >= TRADE_UNIT of.
    let mut recv: Option<usize> = None;
    let mut recv_want_b = f32::INFINITY;
    for k in 0..GOOD_COUNT {
        if inv_b[k] >= TRADE_UNIT {
            let wk = want(inv_b, k);
            if wk < recv_want_b {
                recv_want_b = wk;
                recv = Some(k);
            }
        }
    }
    let recv = recv?;
    if give == recv {
        return None; // swapping the same good is pointless
    }
    // Mutual benefit: each side values what it receives more than what it gives.
    let a_gains = want(inv_a, recv) > want(inv_a, give);
    let b_gains = want(inv_b, give) > want(inv_b, recv);
    if a_gains && b_gains {
        Some((give, recv))
    } else {
        None
    }
}

/// Trade: each alive agent A (ascending) trades one `TRADE_UNIT` with its
/// nearest OTHER-species neighbor B (from the sensor register), if a mutually-
/// beneficial complementary swap exists and B is within `TRADE_RANGE`.
/// Conserves total units of each good. No RNG.
fn trade_pass(world: &mut World, alive_ids: &[u32]) {
    use crate::resource::TRADE_UNIT;
    for &id in alive_ids {
        let i = id as usize;
        let tgt = world.sensors[i].nearest_other_id;
        if tgt == crate::sense::NO_NEIGHBOR_ID {
            continue;
        }
        if world.sensors[i].nearest_other_dist >= crate::resource::TRADE_RANGE {
            continue;
        }
        let t = tgt as usize;
        if t == i || !world.agents.is_alive(tgt) {
            continue;
        }
        let inv_a = world.agents.inventory[i];
        let inv_b = world.agents.inventory[t];
        let Some((give, recv)) = pick_swap(&inv_a, &inv_b) else {
            continue;
        };
        // Execute the swap (totals conserved: each side's sum is unchanged).
        world.agents.inventory[i][give] -= TRADE_UNIT;
        world.agents.inventory[t][give] += TRADE_UNIT;
        world.agents.inventory[t][recv] -= TRADE_UNIT;
        world.agents.inventory[i][recv] += TRADE_UNIT;
    }
}
```

Wire it into `interact_all` right after `share_pass`, gated:

```rust
    share_pass(world, &alive_ids);
    if world.resources_enabled {
        trade_pass(world, &alive_ids);
    }
    world.agents.scratch_ids = alive_ids;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p anabios-core --lib interact::`
Expected: PASS (harvest + trade + pick_swap tests).

- [ ] **Step 5: Confirm determinism intact**

Run: `cargo test -p anabios-core --test determinism`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/interact.rs
git commit -m "feat(resource): trade_pass — mutually-beneficial cross-species swaps"
```

---

## Task 6: Reproduction dowry gate

**Files:**
- Modify: `crates/anabios-core/src/reproduce.rs` (dowry check + consumption)
- Test: unit tests in `reproduce.rs`

**Interfaces:**
- Consumes: `World.{resources_enabled, agents}`, `resource::{DOWRY_REQ, GOOD_COUNT, Good}`.
- Produces: `fn has_dowry(agents: &AgentBuffers, id: u32) -> bool` (module-private).

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/anabios-core/src/reproduce.rs`:

```rust
    #[test]
    fn dowry_blocks_then_permits_reproduction() {
        use crate::resource::{Good, DOWRY_REQ};
        let mut w = World::new(13);
        w.resources_enabled = true;
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        // No goods yet → no offspring despite ample energy.
        let before = w.agents.live_count();
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), before, "no dowry: no offspring");

        // Give both parents a full basket, then it must produce one offspring.
        for id in [id0, id1] {
            for g in Good::ALL {
                w.agents.inventory[id as usize][g.index()] = DOWRY_REQ;
            }
        }
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), before + 1, "full dowry: one offspring");
        // Dowry consumed from both parents.
        for id in [id0, id1] {
            for g in Good::ALL {
                assert_eq!(w.agents.inventory[id as usize][g.index()], 0.0, "dowry spent");
            }
        }
    }

    #[test]
    fn dowry_gate_is_inert_when_resources_disabled() {
        // With resources off, reproduction ignores inventory entirely (byte-identical path).
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let before = w.agents.live_count();
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), before + 1, "flag off: dowry not required");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --lib reproduce::tests::dowry`
Expected: FAIL (both parents reproduce without a dowry → `dowry_blocks_then_permits_reproduction` asserts `before` but gets `before + 1`).

- [ ] **Step 3: Implement the dowry gate**

In `crates/anabios-core/src/reproduce.rs`, add the helper near `is_eligible`:

```rust
/// True iff this agent holds at least `DOWRY_REQ` of every good — the basket
/// required to reproduce when the trade economy is active.
fn has_dowry(agents: &AgentBuffers, id: u32) -> bool {
    let inv = &agents.inventory[id as usize];
    crate::resource::Good::ALL.iter().all(|g| inv[g.index()] >= crate::resource::DOWRY_REQ)
}
```

In `reproduce_all`, gate parent A early. After the existing `if !is_eligible(&world.agents, a_id) { continue; }` (line ~63), add:

```rust
        if world.resources_enabled && !has_dowry(&world.agents, a_id) {
            continue;
        }
```

After the mate is chosen (`let Some(b_id) = mate else { continue };`, line ~82), add the B check:

```rust
        if world.resources_enabled && !has_dowry(&world.agents, b_id) {
            continue;
        }
```

After both parents pay energy (the two `world.agents.energy[..] -= cost;` lines, ~line 92), add dowry consumption:

```rust
        if world.resources_enabled {
            for g in crate::resource::Good::ALL {
                world.agents.inventory[i][g.index()] -= crate::resource::DOWRY_REQ;
                world.agents.inventory[j][g.index()] -= crate::resource::DOWRY_REQ;
            }
        }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p anabios-core --lib reproduce::`
Expected: PASS (new dowry tests + all existing reproduction tests, which never enable resources).

- [ ] **Step 5: Confirm determinism intact**

Run: `cargo test -p anabios-core --test determinism`
Expected: PASS (flag off → no dowry logic runs, no inventory reads/writes, no RNG change).

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/reproduce.rs
git commit -m "feat(resource): reproduction requires + consumes a dowry basket"
```

---

## Task 7: Scenario flag + `biome-trade.toml`

**Files:**
- Modify: `crates/anabios-core/src/scenario.rs` (`resources_enabled` field + wiring)
- Create: `scenarios/biome-trade.toml`
- Test: unit test in `scenario.rs`; `all_scenarios.rs` picks up the new TOML automatically.

**Interfaces:**
- Consumes: `Scenario` deserialization, `World.resources_enabled`.
- Produces: `Scenario.resources_enabled: bool`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/anabios-core/src/scenario.rs`:

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --lib scenario::tests::resources_flag`
Expected: FAIL to compile ("no field `resources_enabled` on `Scenario`").

- [ ] **Step 3: Add the flag and wire it**

In `crates/anabios-core/src/scenario.rs`, add to the `Scenario` struct after `season_period`:

```rust
    /// Opt-in: enable the biome-trade-goods economy (resource nodes spawn,
    /// agents harvest and trade them, reproduction needs a dowry basket).
    /// `false` (default) leaves the world unchanged.
    #[serde(default)]
    pub resources_enabled: bool,
```

In `instantiate`, after `w.season_period = self.season_period;`, add:

```rust
        w.resources_enabled = self.resources_enabled;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p anabios-core --lib scenario::tests::resources_flag`
Expected: PASS.

- [ ] **Step 5: Create the scenario TOML**

Create `scenarios/biome-trade.toml` — several species spread across the map (each cluster harvests whatever goods are local to it; trade forms where ranges meet). A smaller world keeps the demo dense.

```toml
# Biome trade goods: five species clustered across the map. Each harvests the
# goods native to its local biomes, then swaps surpluses with other species to
# assemble the balanced dowry basket every birth requires. Trade + reproduction
# emerge from geography.
name = "biome-trade"
seed = 20260719
resources_enabled = true
living_biome = true
max_population = 600

[[agents]]
count = 60
archetype = "grazer"
placement = { kind = "cluster", center_x = 200.0, center_y = 200.0, radius = 90.0 }
[agents.traits]
reproduction_threshold = 0.4
size = 0.4

[[agents]]
count = 60
archetype = "grazer"
placement = { kind = "cluster", center_x = 800.0, center_y = 200.0, radius = 90.0 }
[agents.traits]
reproduction_threshold = 0.4
size = 0.4

[[agents]]
count = 60
archetype = "grazer"
placement = { kind = "cluster", center_x = 200.0, center_y = 800.0, radius = 90.0 }
[agents.traits]
reproduction_threshold = 0.4
size = 0.4

[[agents]]
count = 60
archetype = "grazer"
placement = { kind = "cluster", center_x = 800.0, center_y = 800.0, radius = 90.0 }
[agents.traits]
reproduction_threshold = 0.4
size = 0.4

[[agents]]
count = 60
archetype = "grazer"
placement = { kind = "cluster", center_x = 512.0, center_y = 512.0, radius = 90.0 }
[agents.traits]
reproduction_threshold = 0.4
size = 0.4
```

- [ ] **Step 6: Run the scenario smoke test**

Run: `cargo test -p anabios-core --test all_scenarios`
Expected: PASS (the new TOML parses, instantiates, and runs without panicking or leaving world bounds).

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/scenario.rs scenarios/biome-trade.toml
git commit -m "feat(resource): resources_enabled scenario flag + biome-trade.toml"
```

---

## Task 8: Codex observability (`ResourceTraded`, `DowryBirth`)

**Files:**
- Modify: `crates/anabios-core/src/codex/mod.rs` (append `EventType` variants)
- Modify: `crates/anabios-core/src/interact.rs` (emit `ResourceTraded` in `trade_pass`)
- Modify: `crates/anabios-core/src/reproduce.rs` (emit `DowryBirth`)
- Test: unit test in `interact.rs` and `reproduce.rs`

**Interfaces:**
- Consumes: `CodexState.{first_cross_species_trade, push_event}` (field added in Task 2), `CodexEvent`.
- Produces: `EventType::ResourceTraded = 19`, `EventType::DowryBirth = 20`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/anabios-core/src/interact.rs`:

```rust
    #[test]
    fn first_cross_species_trade_emits_event() {
        use crate::codex::EventType;
        use crate::resource::Good;
        let mut w = World::new(5);
        w.resources_enabled = true;
        let pos = Vec2::new(300.0, 300.0);
        let a = w.spawn_agent(pos, Genome::neutral());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
        w.agents.species_id[b as usize] = 1;
        w.agents.inventory[a as usize][Good::Salt.index()] = 5.0;
        w.agents.inventory[b as usize][Good::Obsidian.index()] = 5.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        crate::sense::sense_all(
            &w.agents, &w.biome, &w.pheromones, &w.spatial, &mut w.sensors, w.world_size,
        );
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        trade_pass(&mut w, &alive);
        assert!(w.codex.first_cross_species_trade, "latch set after first trade");
        assert!(
            w.codex.events.iter().any(|e| e.event_type == EventType::ResourceTraded),
            "a ResourceTraded event was recorded"
        );
    }
```

Add to the `tests` module in `crates/anabios-core/src/reproduce.rs`:

```rust
    #[test]
    fn dowry_birth_emits_event() {
        use crate::codex::EventType;
        use crate::resource::{Good, DOWRY_REQ};
        let mut w = World::new(13);
        w.resources_enabled = true;
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        for id in [id0, id1] {
            for g in Good::ALL {
                w.agents.inventory[id as usize][g.index()] = DOWRY_REQ;
            }
        }
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        reproduce_all(&mut w);
        assert!(
            w.codex.events.iter().any(|e| e.event_type == EventType::DowryBirth),
            "a DowryBirth event was recorded"
        );
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --lib interact::tests::first_cross_species_trade reproduce::tests::dowry_birth`
Expected: FAIL to compile ("no variant `ResourceTraded`/`DowryBirth`").

- [ ] **Step 3: Append the `EventType` variants**

In `crates/anabios-core/src/codex/mod.rs`, append to `EventType` after `InventionAdopted = 18`:

```rust
    /// First bilateral cross-species resource swap in the world (latched once).
    ResourceTraded = 19,
    /// An offspring was produced by spending a full dowry basket.
    DowryBirth = 20,
```

- [ ] **Step 4: Emit `ResourceTraded` from `trade_pass`**

In `crates/anabios-core/src/interact.rs`, inside `trade_pass`, after the four inventory mutations that execute the swap, add:

```rust
        if !world.codex.first_cross_species_trade {
            world.codex.first_cross_species_trade = true;
            world.codex.push_event(crate::codex::CodexEvent {
                event_type: crate::codex::EventType::ResourceTraded,
                tick: world.tick,
                species_id: world.agents.species_id[i],
                value: give as f32,
                loc_x: world.agents.position[i].x,
                loc_y: world.agents.position[i].y,
            });
        }
```

- [ ] **Step 5: Emit `DowryBirth` from `reproduce_all`**

In `crates/anabios-core/src/reproduce.rs`, in the `if world.resources_enabled { ... }` dowry-consumption block added in Task 6, after the consumption loop (still inside the guard), record the event using the child position already computed (`child_pos`):

```rust
            world.codex.push_event(crate::codex::CodexEvent {
                event_type: crate::codex::EventType::DowryBirth,
                tick: world.tick,
                species_id: a_species,
                value: 0.0,
                loc_x: child_pos.x,
                loc_y: child_pos.y,
            });
```

(Place this after `child_pos` is bound — i.e. move the dowry-consumption block to just after `let child_pos = midpoint_torus(...)`, or reference the parents' position if simpler. If ordering is awkward, use `world.agents.position[i]` for `loc_x/loc_y` instead of `child_pos`.)

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p anabios-core --lib interact:: reproduce::`
Expected: PASS.

- [ ] **Step 7: Confirm determinism intact**

Run: `cargo test -p anabios-core --test determinism --test inventions`
Expected: PASS. Appending `EventType` variants does not change the serialized bytes of scenarios that never emit them (existing variant discriminants are unchanged), so neither golden-hash set moves.

- [ ] **Step 8: Commit**

```bash
git add crates/anabios-core/src/codex/mod.rs crates/anabios-core/src/interact.rs crates/anabios-core/src/reproduce.rs
git commit -m "feat(resource): codex ResourceTraded + DowryBirth events"
```

---

## Task 9: Integration test — trade scenario determinism + regression

**Files:**
- Create: `crates/anabios-core/tests/trade.rs`
- Test: this file.

**Interfaces:**
- Consumes: `Scenario`, `state_hash`, `step`, `EventType`.

- [ ] **Step 1: Write the tests**

Create `crates/anabios-core/tests/trade.rs`:

```rust
//! Integration tests for the biome trade-goods economy.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;

const TRADE: &str = include_str!("../../../scenarios/biome-trade.toml");

/// The trade scenario is deterministic: two independent runs match at tick 300.
#[test]
fn trade_scenario_is_deterministic() {
    let run = || {
        let mut w = Scenario::parse_toml(TRADE).expect("parse").instantiate();
        for _ in 0..300 {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(), run(), "trade scenario must replay identically");
}

/// The economy actually turns over: cross-species trades and dowry births occur.
#[test]
fn trade_scenario_produces_trades_and_dowry_births() {
    let mut w = Scenario::parse_toml(TRADE).expect("parse").instantiate();
    let mut saw_trade = false;
    let mut saw_dowry = false;
    for _ in 0..600 {
        step(&mut w);
        for e in w.codex.events.iter() {
            match e.event_type {
                EventType::ResourceTraded => saw_trade = true,
                EventType::DowryBirth => saw_dowry = true,
                _ => {}
            }
        }
        if saw_trade && saw_dowry {
            break;
        }
    }
    assert!(saw_trade, "expected at least one cross-species trade");
    assert!(saw_dowry, "expected at least one dowry-gated birth");
}

/// Regression guard: a resources-OFF scenario is unaffected by the feature.
/// (minimal.toml never enables resources; its golden hashes live in
/// determinism.rs. This asserts the flag genuinely defaults off end-to-end.)
#[test]
fn minimal_scenario_keeps_resources_off() {
    let minimal = include_str!("../../../scenarios/minimal.toml");
    let w = Scenario::parse_toml(minimal).expect("parse").instantiate();
    assert!(!w.resources_enabled);
    assert!(w.resources.is_empty());
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p anabios-core --test trade`
Expected: PASS. If `trade_scenario_produces_trades_and_dowry_births` fails (no trades/births within 600 ticks), it means the v1 balance constants are too tight for this seed — tune `NODE_TARGET_PER_GOOD`, `HARVEST_RATE`, `DOWRY_REQ`, or the scenario cluster layout until both events fire, then re-run. Document any constant change in `resource.rs`.

- [ ] **Step 3: Run the full crate test suite**

Run: `cargo test -p anabios-core`
Expected: PASS (every test, including both golden-hash suites and `all_scenarios`).

- [ ] **Step 4: Run fmt + clippy (CI gates)**

Run: `cargo fmt -p anabios-core -- --check && cargo clippy -p anabios-core --all-targets -- -D warnings`
Expected: clean. Fix any formatting/lint issues and re-run.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/tests/trade.rs
git commit -m "test(resource): trade scenario determinism + economy turnover"
```

---

## Self-Review

**Spec coverage** (each design section → task):
- Four biome-linked goods (§ "The four goods") → Task 1 (`Good`, `from_terrain`).
- Discrete map entities + spawning (§ Data model, Spawning) → Tasks 2 (`resources` state) + 3 (`resource_step`).
- Per-agent inventory + carrying cap + Storage bonus (§ Data model, Constants) → Tasks 2 (`inventory`) + 4 (`carrying_cap`).
- Harvesting (§ Harvesting) → Task 4.
- Bilateral need-based trade with diminishing marginal utility (§ Trade) → Task 5 (`want`, `pick_swap`, `trade_pass`).
- Reproduction dowry + consumption sink (§ Reproduction dowry) → Task 6.
- Determinism / FORMAT_VERSION / golden hashes / zero-RNG-off (§ Determinism) → Task 2 (bump + regen) + gating in Tasks 3–6 + Task 9 regression.
- Scenario flag + trade scenario (§ Config / scenario) → Task 7.
- Codex observability (§ Observability) → Task 8.
- Testing strategy (§ Testing) → unit tests in Tasks 1,3,4,5,6,7,8 + integration Task 9.
- Non-goals honored: no per-good secondary effects, no neural harvest/trade decisions, no same-species trade (Task 5 uses `nearest_other_id`).

**Type consistency:** `Good::index() -> usize`, `Good::ALL: [Good; GOOD_COUNT]`, `Good::from_terrain`, `want(&[f32; GOOD_COUNT], usize) -> f32`, `carrying_cap(&ModuleList) -> f32`, `inventory_total`, `resource_step(&mut World)`, `harvest_pass`/`trade_pass`/`pick_swap` signatures, `AgentBuffers.inventory: Vec<[f32; GOOD_COUNT]>`, `World.{resources, resource_spatial, resources_enabled}`, `CodexState.first_cross_species_trade`, `EventType::{ResourceTraded=19, DowryBirth=20}` — all consistent across tasks.

**Placeholder scan:** none — every code step shows full code; Task 9 Step 2 flags a concrete tuning action (not a placeholder) if the balance needs adjustment for the chosen seed.

**Determinism note carried through:** golden hashes regenerate exactly once (Task 2). Later tasks add only flag-gated behavior (Tasks 3–6) or append-only enum variants (Task 8), neither of which moves the regenerated hashes — Tasks 3–8 each re-run `--test determinism` to confirm.
