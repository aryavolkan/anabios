# Trade & Invention Landmark Buildings — Design Spec

**Date:** 2026-08-10
**Branch:** `claude/2d-artifacts-species-buildings`
**Scope:** Godot viewer only. Pure presentation over read-only sim state. No
`anabios-core` changes, no new sim signals, no determinism/golden impact.

## Goal

Every settlement village should visibly announce **what its species has
achieved**: its most-advanced invention (a signature landmark structure) and
whether it is a trading hub (a market or warehouse). Today the viewer draws
only generic huts + farm patches ([settlement_layer.gd](../../../game/scripts/settlement_layer.gd)),
which read as "a village exists" but say nothing about its tech or trade. This
adds a small vocabulary of building sprites, placed from real sim state, so a
glance at a village tells its story.

This is the second half of the "2D artifacts" work; the first half fixed the
Metal atlas corruption that garbled the creature sprites (commit `7f0598b`).

## Prior art / the Metal constraint

The just-fixed bug matters here: an extreme-aspect texture sampled through a
`canvas_item` **ShaderMaterial** on the MultiMesh2D canvas path corrupts into
torn streaks on Metal. Buildings **do not animate**, so they avoid that path
entirely — each building type is a plain single-texture `MultiMesh2D` with no
shader, nearest-filtered, exactly like the existing huts and farms, which
render cleanly. This is a deliberate, proven-safe choice, not an oversight.

## Sim-state surface (all already exposed, read-only)

Per throttled redraw (the existing `REDRAW_EVERY = 20`):

- `settlement_sites() -> [{species_id, pos, members}]` — one aggregated anchor
  per settled species (centroid of that species' anchored members). Already the
  anchor for huts/farms.
- `species_stats() -> [{species_id, count, tech_era, adopted_inventions,
  mean_energy, mean_technique_match}]` — `adopted_inventions` is a
  `PackedStringArray` of held invention **keys**. Join to a site by
  `species_id`.
- `invention_catalog() -> [{key, name, era, prereqs, ...}]` — static; used to
  map a held key to its `era` for "most-advanced" selection. Fetch once and
  cache (it never changes during a run).
- `market_colors() -> PackedColorArray` — per-biome-cell market-density field
  (the E8 markets overlay). Length = `biome_resolution()^2`. Empty unless the
  resource/market subsystem is on.
- `resources_active() -> bool` — gates the trade buildings; when false, skip
  market/warehouse entirely.
- `biome_resolution()`, `world_size()` — map a world position to a market-field
  cell index (same arithmetic the biome renderer uses).

No new binding methods are required.

## Placement logic (per village, per redraw)

For each site, joined to its species' stats:

1. **Signature landmark.** Among `adopted_inventions`, pick the key with the
   highest `era` (tie-break by canonical `INVENTIONS` order via the catalog
   index). Draw that invention's building on the village's primary landmark
   slot near the anchor. A village with zero held inventions draws no landmark
   (huts only, unchanged).
2. **Second landmark.** When `members >= LANDMARK2_MIN` (32), also draw the
   next-highest held invention on a secondary slot, so a thriving high-tech
   village reads as richer than a small one.
3. **Trade building.** Only when `resources_active()`. Sample `market_colors()`
   at the anchor's cell:
   - density `>= MARKET_MIN` → draw a **market**.
   - density `>= MARKET_MIN` **and** `members >= WAREHOUSE_MIN` → draw a
     **warehouse** instead (the hub outgrew a stall).
4. **Slots.** Landmark + trade buildings sit on their own deterministic
   golden-angle ring (a different phase/radius than the hut ring) so they never
   overlap huts. They reuse the existing village-memory `linger`/`fade`/`grow`
   easing so buildings fade in on adoption and linger on collapse rather than
   popping.

All thresholds are named constants, tuned in the capture pass (Task: visual
verification), not magic numbers inline.

## Art set (12 sprites)

Each is a 16×16 block-art texture built with the shared
`ApeSprites._build_cell(blocks)` painter (auto 1px dark outline), using the
existing `PAL` palette so buildings match the hut/farm look. Buildings are
scaled up on placement (like `HUT_SCALE`) so they read as architecture.

**Trade (2):**

- **Market** — striped awning over a stall counter with goods baskets; smaller
  footprint (it's a stall, not a building).
- **Warehouse** — a broad storehouse with big doors and stacked crates.

**Inventions (10), era-ordered:**

| # | Invention (key) | Landmark |
|---|---|---|
| 1 | Stone Tools | knapping rack — worked-stone block + leaning tools |
| 2 | Fire | stone hearth ring with a flame |
| 3 | Farming | granary/silo — round grain store (distinct from the generic farm patch) |
| 4 | Metalworking | forge — chimney + anvil, ember glow |
| 5 | Writing | scriptorium — standing stele / inscribed tablet |
| 6 | Medicine | apothecary — hut with hung herb bundles |
| 7 | Husbandry | corral — fenced pen |
| 8 | Machinery | workshop — gear / waterwheel |
| 9 | Electricity | lamp/pylon — glowing lamp post |
| 10 | Nuclear Power | reactor — cooling-tower dome |

The invention→building map is a single table keyed by invention key, so the
art and the lookup stay in one place.

## Rendering approach

- One plain `MultiMeshInstance2D` per building type (~12 layers), each with its
  own 16×16 `ImageTexture`, `TEXTURE_FILTER_NEAREST`, `use_colors = true`, no
  material. Each gets the same 8-way torus wrap-clones as huts (shared
  MultiMesh), via the existing `_make_wrap_clones()` machinery.
- z-order above agents (like huts, `z = 1`) so architecture looms over the
  crowd; the market stall can sit slightly lower.
- Per-instance color carries the linger `fade` alpha (and optionally a faint
  per-species tint like the huts already use).
- Building textures are built once at `_ready` (they are static), pre-flipped
  for the QuadMesh's flipped V axis (same `flip_y()` convention as the hut/farm
  textures).

## Files

- **`game/scripts/settlement_layer.gd`** (extend) — the join + per-village
  placement + the new MultiMesh layers. This file already owns village memory,
  linger/fade, hut/farm layers, and wrap clones; landmark buildings are the
  same job. If it grows past a comfortable size, split the pure texture-builder
  block lists into a sibling `building_sprites.gd` (mirroring how
  `mammal_data/*.gd` holds pose data) — decide during implementation based on
  final line count, staying under the repo's 1000-line lint ceiling.
- **`game/scripts/building_sprites.gd`** (likely new) — the 12 building block
  lists + the invention-key→building table + a `build(kind)` returning the
  `ImageTexture`, built via `ApeSprites._build_cell`. Keeps `settlement_layer`
  focused on placement, not art data.

No `.tres`/scene changes required (layers are code-created, like the hut/farm
layers already are).

## Testing / verification

- **Headless build check** — a small `SceneTree` dumper (like the one used to
  verify the creature atlases) that bakes every building texture to PNG so each
  sprite can be eyeballed for shape correctness before wiring placement.
- **Live capture** — the `ANABIOS_SHOT` windowed screenshot harness on a
  scenario that reaches inventions + trade (e.g. `inventions.toml`,
  `geographic-trade.toml`, `domestication.toml`): confirm the right landmark
  appears for a village's top invention, that markets appear on trade hubs, and
  that nothing corrupts or pops. This is the real acceptance test.
- **Determinism** — none needed; zero core changes, presentation only. The
  full determinism/golden suite is unaffected.
- **Gates** — `gdformat --check` and `gdlint` clean on the changed/new GDScript
  before pushing (the viewer CI gate).

## Non-goals

- No caravan / trade-route waystation markers (the `trade_routes()` lanes are
  already drawn as the existing TradeRoutes layer). Deferred.
- No per-invention *counts* or crowding: at most 2 landmarks + 1 trade building
  per village, because `settlement_sites` yields one anchor per species.
- No `anabios-core` changes, no new `#[func]` bindings, no new sim signals.
- No animation on buildings (deliberate — keeps them off the Metal shader path).

## Open tuning (resolved during the capture pass, not blockers)

- Exact `MARKET_MIN`, `WAREHOUSE_MIN`, `LANDMARK2_MIN`, landmark scale, and ring
  radius/phase — tuned visually so villages read clearly without clutter.
- Whether the second landmark or the market wins the more prominent slot when a
  village is both high-tech and a trade hub.
