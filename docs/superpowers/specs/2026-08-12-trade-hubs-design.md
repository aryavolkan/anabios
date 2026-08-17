# Trade Hubs — worldgen marketplaces agents travel to

**Date:** 2026-08-12
**Branch:** `claude/2d-images-inventions-trade-198941`
**Status:** Design approved, ready for implementation plan

## Summary

Introduce **predetermined trade hubs**: a small set of fixed marketplace locations
placed at world-generation time where different trade-good terrains meet. Agents with
a real trade motive travel to the nearest hub, and **barter now happens only at hubs**.
This replaces today's trade-anywhere model (agents bartering wherever two complementary
neighbors randomly collide), which is prone to freezing because complementary pairs
rarely meet by chance.

Alongside the mechanic, complete the trade/invention 2D imagery: draw the marketplace
sprite at each hub, add trade-good icons so *what* is traded is visible, and polish the
existing building sprites.

This is a **behavior-first** change. We are not preserving determinism or byte-identity
against the current baseline — `FORMAT_VERSION` bumps and all goldens are regenerated to
the new behavior. The new golden values simply *are* the spec.

## Context (current state)

- **Trade is emergent, no hubs.** `interact.rs::trade_pass` has each agent barter one unit
  with its nearest other-species neighbor within `TRADE_RANGE`. A "market" is a decaying
  scalar field (`World.market_field`) that blooms wherever swaps keep happening
  (`settlement.rs::market_deposit` / `market_decay_step`).
- **4 trade goods**, one per land terrain: Salt/Obsidian/Amber/Spice
  (`resource.rs::Good`, `home_terrain`/`from_terrain`). `want()` flags an agent as having a
  surplus (above `STOCK_TARGET`) or a deficit.
- **Movement is drive-based steering, not pathfinding.** `tick.rs::decide_all` layers
  additive biases (personality, affect, habitat-climate, terrain-affinity, home-anchor,
  livestock, survival-hijack) onto the evolved program's raw move intent, then normalizes
  to a unit `desired_direction`; `integrate.rs::integrate_all` applies it with torus wrap.
- **Viewer is a live embedded sim** (GDExtension `Simulation` node), not replay files.
  `settlement_layer.gd` draws huts/farms at settlement anchors, market/warehouse buildings
  from the market field, and invention landmarks at inventive-species centroids.
  `building_sprites.gd` holds 12 16×16 block-art sprites (Market, Warehouse, + 10 tech).
- `resources_enabled` (World flag) gates the whole trade-goods subsystem.
- Current `FORMAT_VERSION = 32` (`snapshot.rs`).

## Goals

1. Fixed, worldgen-derived trade hubs at trade-good terrain borders.
2. Trade-motivated agents steer to the nearest hub; barter happens only at hubs.
3. Marketplace sprite drawn at each hub, with goods-icon rings showing traded goods.
4. Polish the 12 existing building sprites; add 4 trade-good icons.
5. Regenerate goldens to the new behavior.

## Non-goals

- No pathfinding / navmesh / A* — hub-seeking is a steering bias, consistent with the
  rest of the sim.
- No matching-primitive upgrade (agents still trade with a nearest neighbor; hubs just
  cluster them so neighbors are present). The known trade-freeze may improve as a side
  effect but "fix the freeze" is not a success criterion here.
- No dedicated new hub sprite — hubs reuse the polished Market/Warehouse art.
- No new scenario file required for the mechanic — hubs turn on with
  `resources_enabled`, so the existing trade scenarios (`biome-trade.toml`,
  `geographic-trade.toml`, `unilateral-trade.toml`) get them automatically.
  (Correction: `inventions.toml` sets `inventions_enabled` but NOT
  `resources_enabled`, so it is a tech-tree scenario and produces zero hubs — an
  earlier draft wrongly cited it here. A dedicated `trade-hubs.toml` showcase was
  added later as a follow-up, not by this spec.)
- No off-by-default gating for golden preservation — behavior is the new normal.

## Design

### 1. Data model

New type in the resource/settlement layer:

```rust
struct TradeHub {
    pos: Vec2,          // world-space hub center
    cell: usize,        // biome-grid cell index (row-major)
    goods: Vec<Good>,   // distinct good-terrains meeting here (viewer icons + flavor)
}
```

`World` gains `pub trade_hubs: Vec<TradeHub>`, serialized. Empty and inert unless
`resources_enabled`. Adding this field is what bumps `FORMAT_VERSION` (32 → 33) and
rehashes goldens.

Hub-seeking is **stateless** — the pull is recomputed each tick from the nearest hub,
so there is **no new per-agent field** and no per-agent serialized state.

### 2. Worldgen placement — border-diversity greedy scan (Approach A)

`place_trade_hubs(biome: &BiomeField, params) -> Vec<TradeHub>`, run once at worldgen end
after the `BiomeField` exists, only when `resources_enabled`. Pure function of the biome
grid → deterministic.

Algorithm:
1. Map each biome cell's terrain to its `Good` (via `Good::from_terrain`; cells with no
   trade-good terrain contribute nothing).
2. For each cell, compute a **diversity score** = count of *distinct* goods appearing in
   cells within `HUB_SCAN_RADIUS`. Cells whose neighborhood spans ≥2 distinct goods are
   candidate crossroads.
3. Greedily select candidates by descending score, skipping any candidate within
   `HUB_MIN_SPACING` of an already-selected hub, until `HUB_MAX_COUNT` is reached or
   candidates are exhausted. Deterministic tie-break by cell index.
4. For each selected cell, record the hub center (cell center in world space), cell index,
   and the set of distinct goods in its neighborhood.

Tuning constants (module-level, in the resource/settlement layer):
- `HUB_SCAN_RADIUS` — neighborhood radius in cells for the diversity score.
- `HUB_MIN_SPACING` — minimum world-space distance between hubs.
- `HUB_MAX_COUNT` — cap on hub count (target ~6; tune on `geographic-trade.toml`).

Honest degradation: a single-terrain / low-diversity map yields few or zero hubs, so trade
stays sparse — truthful rather than papered over.

### 3. Movement — hub-seeking steering bias

New additive bias `best_hub_direction(world, id) -> Vec2`, slotted into the `decide_all`
bias stack in `tick.rs` alongside the existing terrain-affinity and anchor pulls.

- **Active only** when the agent has a real trade motive: `want()` reports a surplus to
  offload or a deficit to fill. Balanced agents behave exactly as today (zero contribution).
- Direction = unit vector from the agent toward the **nearest** hub, torus-aware (min-image
  distance under world wrap). No hubs → zero vector.
- Contribution scaled by a module constant `HUB_PULL`, added into the bias sum and then
  normalized with the other biases (same treatment as `best_terrain_direction`).

### 4. Trade restricted to hubs

In `trade_pass`, a candidate swap is allowed **only** when the initiator is within
`HUB_TRADE_RANGE` of some hub. The partner search is otherwise unchanged (nearest
other-species neighbor within `TRADE_RANGE`); because trade-motivated agents cluster at
hubs, complementary partners are naturally present. Away from all hubs, no barter occurs.

`market_deposit` still fires on successful swaps, so `market_field` heat now blooms at
hubs — which is exactly where the viewer reads it for market/warehouse rendering.

When `resources_enabled` is false, `trade_hubs` is empty and `trade_pass` behaves as it
does today (no trade subsystem at all).

### 5. Viewer — hub rendering, goods icons, sprite polish

- **New accessor** `Simulation::trade_hubs()` (`anabios-godot/src/lib.rs`) → `Array` of
  `{ pos: Vector2, goods: PackedInt32Array }`.
- **Hub layer** (extend `settlement_layer.gd`, or a small sibling `hub_layer.gd`): draw the
  polished **Market** sprite at each hub center — **always on**, independent of village
  membership, since hubs are worldgen fixtures. The busiest hubs (highest local
  `market_field` heat) upgrade to the **Warehouse** sprite. Reuse the existing plain
  no-shader MultiMesh path (Metal-safe), with the 9-way torus wrap clones.
- **Goods icons:** 4 new ~10–12px sprites (Salt/Obsidian/Amber/Spice) built via
  `ApeSprites._build_cell`, drawn as a small ring around each hub for the goods in
  `hub.goods`. Same pre-`flip_y()` + nearest-filter convention as the buildings.
- **Polish pass** on the existing 12 building sprites: edit the `_BLOCKS` block-art in
  `building_sprites.gd` for clearer silhouettes / better read at a glance. Structural
  (enum, mapping, API) unchanged — art-only edits.

### 6. Determinism, goldens, testing

- Bump `FORMAT_VERSION` 32 → 33.
- **Regenerate all goldens** (determinism / inventions / cognition) to the new baseline
  as part of the change. Not preserving prior values.
- **Rust unit tests** (behavior, not preservation):
  - `place_trade_hubs`: on a synthetic multi-terrain biome, hubs are well-spaced
    (≥ `HUB_MIN_SPACING`), capped (≤ `HUB_MAX_COUNT`), sit at diverse cells, and the
    result is deterministic across two calls; a single-terrain biome yields ~0 hubs.
  - `best_hub_direction`: points at the nearest hub, including across the torus seam; zero
    when the agent has no trade motive or no hubs exist.
  - `trade_pass`: a swap succeeds when both agents are within `HUB_TRADE_RANGE` of a hub and
    is blocked when they are far from every hub.
  - Save → load → step round-trip still matches (basic replay sanity for the new `World`
    field; not a preservation gate).
- **Godot**: headless boot (`res://scenes/main.tscn --quit-after`) renders the hub layer +
  goods icons without error.
- Lint/format gates: `cargo fmt --check`, clippy `-D warnings`, gdformat/gdlint.

## Files touched (anticipated)

**Sim (`crates/anabios-core/src/`):**
- `resource.rs` or new `hub.rs` — `TradeHub`, `place_trade_hubs`, hub constants.
- `world.rs` — `trade_hubs` field + constructor init.
- `snapshot.rs` — `FORMAT_VERSION` bump.
- `tick.rs` — `best_hub_direction` bias into `decide_all`.
- `interact.rs` — hub-proximity gate in `trade_pass`.
- worldgen entry (where `BiomeField` is built) — call `place_trade_hubs`.

**Godot bridge (`crates/anabios-godot/src/lib.rs`):** `trade_hubs()` accessor.

**Viewer (`game/scripts/`):**
- `building_sprites.gd` — polish `_BLOCKS`; add 4 goods-icon builders.
- `settlement_layer.gd` (or new `hub_layer.gd`) — hub rendering + goods-icon rings.

**Goldens / scenarios:** regenerate golden fixtures; verify on the trade scenarios
(`biome-trade.toml` / `geographic-trade.toml`), which enable `resources_enabled`.

## Open questions

None blocking. Constant values (`HUB_SCAN_RADIUS`, `HUB_MIN_SPACING`, `HUB_MAX_COUNT`,
`HUB_PULL`, `HUB_TRADE_RANGE`) to be tuned during implementation against a trade
scenario such as `geographic-trade.toml`.
