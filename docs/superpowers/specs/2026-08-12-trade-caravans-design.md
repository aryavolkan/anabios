# Trade caravans — carts hauling goods between hubs

**Date:** 2026-08-12
**Branch:** `claude/2d-images-inventions-trade-198941` (follow-on to the trade-hubs feature)
**Status:** Design approved, ready for implementation plan

## Summary

Add animated **trade caravans** that travel between predetermined trade hubs in the
Godot viewer: a fixed route network connects nearby hubs, and short trains of cart
sprites shuttle along each route hauling trade-good icons. The cargo a caravan carries is
**proportional to the real goods traded** at the two hubs its route links.

This is **visual-only**: the simulation's behavior, determinism, and golden hashes are
untouched. The single sim-side addition is an **inert, non-hashed** per-hub per-good trade
tally — write-only scratch the sim never reads back, serialized-skipped exactly like the
existing `world.trade_routes` viewer scratch — surfaced to the viewer through one new
accessor.

## Context (current state, from the trade-hubs feature)

- Hubs: `World.trade_hubs: Vec<crate::hub::TradeHub>` (`pos`, `cell`, `goods`), placed at
  scenario instantiate when `resources_enabled`. Exposed via `sim.trade_hubs()` →
  `{ pos: Vector2, goods: PackedInt32Array }` per hub (index order stable).
- Barter is gated to hub proximity: `trade_pass` (`crates/anabios-core/src/interact.rs`)
  only lets a swap happen within `crate::hub::HUB_TRADE_RANGE` of a hub. On a successful
  swap it increments `world.total_trades`, calls `settlement::market_deposit`, and pushes a
  `world.trade_routes` record.
- `world.trade_routes: Vec<(Vec2, Vec2, f32)>` is `#[serde(skip)]` viewer scratch, cleared
  at the top of `interact_all` and repushed each tick, **never read by the sim** — the
  established pattern for viewer-only data with zero determinism impact.
- Viewer layers (`game/scripts/hub_layer.gd`, `settlement_layer.gd`) draw sprites through
  plain no-shader `MultiMesh` with a 9-way torus wrap-clone set; `building_sprites.gd`
  holds the block-art sprites (buildings + the 4 goods icons `build_good(idx)`), painted by
  `ApeSprites._build_cell` and `flip_y()`-ed for the MultiMesh's flipped V axis.
- Good index order is fixed: Salt=0, Obsidian=1, Amber=2, Spice=3
  (`crate::resource::Good::index()`). `GOOD_COUNT = 4`.
- The world is a torus (`wrap_torus`, `rem_euclid`); hubs can sit near the seam.

## Goals

1. A fixed route network linking each hub to its nearest neighbors.
2. Animated cart caravans shuttling along each route, plus a faint route line.
3. Caravan cargo (goods icons) proportional to the real per-good trade volume at the two
   hubs a route links.
4. Zero determinism/golden impact — visual-only, with only an inert non-hashed sim tally.

## Non-goals

- No change to trade behavior, agent movement, or the economy. Caravans do not move goods
  in the sim; they visualize trade that already happened at the hubs.
- No `FORMAT_VERSION` bump and no golden regeneration (the tally is serde-skipped and never
  hashed or read by the sim).
- No pathfinding — routes are straight torus-aware segments between hub positions.
- No new scenario — caravans appear on any `resources_enabled` scenario that has hubs
  (e.g. `trade-hubs.toml`, `geographic-trade.toml`).

## Design

### 1. Sim — inert per-hub per-good trade tally

New field on `World`:

```rust
/// Per-hub, per-good count of goods that changed hands at that hub. Index-aligned
/// to `trade_hubs`. Viewer scratch ONLY — never read by the simulation, so it is
/// `#[serde(skip)]` (not serialized, not in `state_hash`) exactly like `trade_routes`.
/// Self-healed to `trade_hubs.len()` at the top of `trade_pass`, so it survives a
/// snapshot load (which leaves it empty) without panicking.
#[serde(skip)]
pub hub_trade_tally: Vec<[u64; crate::resource::GOOD_COUNT]>,
```

In `trade_pass` (`interact.rs`):
- Before the per-agent loop (or lazily on first use), ensure the tally is sized:
  `if world.hub_trade_tally.len() != world.trade_hubs.len() { world.hub_trade_tally = vec![[0; GOOD_COUNT]; world.trade_hubs.len()]; }`
- On each successful swap, attribute it to the **nearest hub** to the initiator (the swap
  already happened within `HUB_TRADE_RANGE` of a hub, so a nearest-hub lookup returns the
  trading hub) and increment that hub's counter for each good that moved: the `give` good
  (A→B) and, for a bilateral swap, the `recv` good (B→A); a unilateral gift moves one good.
- New helper in `crate::hub`: `nearest_hub_index(hubs: &[TradeHub], pos: Vec2, world_size: f32) -> Option<usize>`
  (torus-aware, deterministic; `None` when no hubs). Mirrors `best_hub_direction`.

**Determinism invariant (must hold):** `hub_trade_tally` is written by the sim but never
read by any tick/decision logic — only by the Godot accessor. A `#[serde(skip)]` field is
safe precisely because it does not feed hashed state; this one does not. (See
`docs/determinism-contract.md` and the serde-skip footgun the `serde_skip_audit` test
guards.)

New accessor (`crates/anabios-godot/src/lib.rs`), index-aligned to `trade_hubs()`:

```rust
/// Per-hub trade tallies: one PackedInt32Array of GOOD_COUNT counts per hub, in
/// the same order as `trade_hubs()`. Empty until the first trade.
#[func]
fn hub_trade_tally(&self) -> Array<PackedInt32Array> { /* ... */ }
```

### 2. Viewer — caravan layer (`game/scripts/caravan_layer.gd`)

A `Node2D`, instantiated in `main.gd` next to the hub layer.

**Route network** (rebuilt when the hub set first appears / changes): for each hub, add an
edge to each of its nearest `CARAVAN_NEIGHBORS` (≈2) other hubs under torus min-image
distance; dedup undirected edges. Result: a small fixed set of routes, each a hub pair
`(a, b)` with the torus min-image segment from `hub[a].pos` to the nearest image of
`hub[b].pos`.

**Cargo mix per route** (recomputed on a periodic redraw, ≈ every 30–45 frames): sum the
two endpoint hubs' `hub_trade_tally` vectors element-wise, normalize over the 4 goods.
Allot `CARTS_PER_ROUTE` (≈3) carts to goods by **largest-remainder** apportionment of that
normalized mix, so a Salt-heavy route shows mostly Salt carts. A route whose endpoints have
no recorded trades yet shows carts with no goods icon (empty carts) until trade accrues.

**Rendering & animation:**
- **Route line:** a faint dashed line along each route's segment (subtle, low alpha),
  drawn at all 9 torus offsets so seam-crossing routes read correctly. Static (redrawn only
  when routes change).
- **Carts:** a short train of `CARTS_PER_ROUTE` cart sprites per route, evenly spaced,
  moving together along the segment with a time-based ping-pong parameter (out to `b`, back
  to `a`), so caravans visibly shuttle. Each cart draws the cart sprite plus its allotted
  goods icon riding on top. Rendered through the project's plain no-shader `MultiMesh` +
  9-way wrap-clone convention (one cart MultiMesh; the 4 existing goods-icon textures reused
  for the riding icons), with per-instance transforms updated each frame in `_process`.
- **New art:** `build_cart()` in `building_sprites.gd` — a small covered-wagon/cart
  block-art sprite via `ApeSprites._build_cell`, `flip_y()`-ed like the other MultiMesh
  textures. The goods icon reuses `build_good(idx)`.
- **Empty-hubs / single-hub safety:** no hubs or <2 hubs ⇒ no routes ⇒ nothing drawn (early
  return), no errors.

**Tuning constants** (in `caravan_layer.gd`): `CARAVAN_NEIGHBORS` (≈2), `CARTS_PER_ROUTE`
(≈3), caravan traversal period, route-line alpha, cart/goods scales.

### 3. Determinism, goldens, testing

- **No `FORMAT_VERSION` bump, no golden regeneration.** The tally is serde-skipped and
  inert; `state_hash` is unchanged.
- **Rust tests:**
  - `trade_pass` increments the correct hub's correct good on a hub-proximate swap (extend
    the existing hub-seeded `trade_pass` unit tests in `interact.rs`).
  - Adding/advancing the tally leaves `state_hash` unchanged across a run (proves it is
    non-hashed) and it survives a save→load→step round-trip without panicking (self-heal
    sizing).
  - `nearest_hub_index` returns the nearest hub under torus wrap; `None` with no hubs.
- **Godot:** headless boot renders the caravan layer without error; the sprite self-test
  (`test_building_sprites.gd`) covers `build_cart()`.
- Lint/format gates: `cargo fmt`, clippy `--all-targets -D warnings`, gdformat/gdlint.

## Files touched (anticipated)

**Sim (`crates/anabios-core/src/`):**
- `world.rs` — `hub_trade_tally` field (`#[serde(skip)]`) + constructor init.
- `hub.rs` — `nearest_hub_index`.
- `interact.rs` — self-heal tally sizing + per-swap tally increment in `trade_pass`; extend tests.

**Godot bridge (`crates/anabios-godot/src/lib.rs`):** `hub_trade_tally()` accessor.

**Viewer (`game/scripts/`):**
- `building_sprites.gd` — `build_cart()` (+ self-test coverage).
- `caravan_layer.gd` (new) — route network + animated caravans + route lines.
- `main.gd` — instantiate the caravan layer.

## Open questions

None blocking. Constant values (`CARAVAN_NEIGHBORS`, `CARTS_PER_ROUTE`, traversal period,
route-line alpha, scales) to be tuned during implementation against `trade-hubs.toml`.
