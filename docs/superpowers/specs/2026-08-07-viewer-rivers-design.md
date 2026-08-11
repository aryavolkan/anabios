# Rivers visible in the viewer (sub-project C, spec 1)

**Date:** 2026-08-07
**Status:** Approved design → implementation
**Branch:** `claude/2d-world-scale-159e3b`

## Context: the larger arc

Sub-project **C** ("viewer at scale") of the four-part "actual world with good scale
in 2D" effort. **A** (continental worldgen) shipped in PR #126: continents, mountain
ranges, rain-shadow, and rivers, with `elevation` and `river_flow` now stored on every
`BiomeCell`. This spec is the **first C deliverable — making rivers visible**. The rest
of C (real elevation hillshade, minimap + wider zoom-out, agent level-of-detail) is
deferred to follow-up specs (C2+).

## Motivation

The signature feature of the new world — its river network — is **invisible in the
viewer**. Two facts combine to hide it:

1. Rivers are a *passable moisture field* (`river_flow`), **not** a `Water` terrain
   (deliberately, so the small population is never stranded). So river cells classify
   as ordinary wet Grass/Forest and render green, not blue.
2. `biome_colors()` (the Rust→Godot color bridge in `crates/anabios-godot/src/lib.rs`)
   **predates** `river_flow` and never reads it.

Result: on the continental scenario the rivers carved in PR #126 don't show at all.
This spec makes them read as flowing water threading the continents.

## Current rendering pipeline (what exists)

- `biome_colors()` returns one `Color` per biome cell (row-major), baking terrain
  base color + live plant biomass + succession + pollution into each cell.
- `game/scripts/biome_renderer.gd` writes those colors into a `res×res` `ImageTexture`,
  scaled to `world_size`, drawn as a `Sprite2D` with a 3×3 torus wrap and LINEAR
  filtering. Redraw cadence scales with resolution (handles `biome_res 512`).
- `game/shaders/terrain.gdshader` presents that texture: softens pixel steps, derives
  a luminance hillshade, and — via `is_water(vec3 c)` (dark, blue-dominant test) —
  applies water treatment (depth gradient, coastal shallows, animated shimmer) to
  water-colored cells. When a DATA overlay is active the shader is set to passthrough
  (`biome_mode = 0`), so overlays stay faithful.

## Approach (locked)

**Bake rivers into `biome_colors()`** — the same mechanism already used for biomass,
succession, and pollution. After those blends, any cell with `river_flow > 0.0` blends
its color toward a **river-blue**, by an amount that rises with `river_flow`
(headwater creeks faint → trunk rivers bold). Because the result is blue, the existing
shader's `is_water()` gives river cells the same shimmer/shallows treatment as the
ocean for free — they read as flowing water.

**Why this over a separate river data-texture + shader path:** smallest change, exactly
matches the existing "bake presentation into the cell color" architecture, needs **no**
new bridge function, texture, or shader edit, and rivers inherit the water shader
automatically. The separate-texture path (a `river_flow_values()` bridge + a second
sampler + shader edits for explicit river geometry/width) is the natural C2 upgrade if
finer control (toggle, line width) is wanted later.

**Rivers-always-on in the biome view** (no dedicated toggle this pass) — consistent with
biomass/succession/pollution, which are likewise always baked in. A toggle is a C2 item.

### Styling
- **River-blue** is brighter / more cyan than the deep-ocean blue `(0.09, 0.19, 0.44)`
  so rivers read against both green land and the darker sea — e.g. around
  `(0.18, 0.42, 0.72)` (final value tuned via the screenshot below; it must still pass
  the shader's `is_water()` test: blue clearly above red and green, blue > 0.20,
  `max(r,g) < 0.45`).
- **Blend strength scales with `river_flow`.** `river_flow` is normalized
  (`accum / max_accum`), so only the mouth approaches 1.0 and most river cells carry
  small values; a linear blend would leave the network nearly invisible. Use a curve
  with a solid floor so even creeks read, e.g.
  `mix = clamp(RIVER_MIX_MIN + RIVER_MIX_GAIN * sqrt(river_flow), 0, RIVER_MIX_MAX)`.
  Exact constants tuned during implementation against the headless screenshot.

## Determinism / blast radius

Pure Godot-side **presentation**. `biome_colors()` is read only by the viewer; it is
**not** part of `state_hash`, touches no golden suite, and changes no simulation
behavior. No `FORMAT_VERSION` bump. The only files touched are
`crates/anabios-godot/src/lib.rs` (the `biome_colors` bridge + a new pure helper) and,
if needed, a one-line constant. `river_flow` is already serialized (PR #126), so no
core changes.

## Testing

1. **Unit test (pure Rust, godot crate).** Extract the river blend as a pure free
   function `fn river_tint(rgb: (f32, f32, f32), river_flow: f32) -> (f32, f32, f32)`
   (no `godot` types, so it lives in the crate's existing `#[cfg(test)] mod tests`).
   `biome_colors()` calls it and wraps the result in `Color`. Assert:
   - `river_flow == 0.0` → returns the input rgb unchanged (identity; non-river cells
     are byte-identical to today).
   - `river_flow > 0.0` → the result is bluer than the input (blue channel up, and
     `b > r` and `b > g` so it trips the shader's `is_water()`).
   - Monotonic: a higher `river_flow` yields a result at least as blue as a lower one.
   - The fully-blended (`river_flow == 1.0`) color satisfies the shader's `is_water()`
     predicate (`b > r + 0.05 && b > g + 0.05 && b > 0.20 && max(r,g) < 0.45`).
2. **Visual verification (headless).** Boot the viewer on `scenarios/continental.toml`
   via the env-gated `debug_capture` screenshot harness (`game/scripts/debug_capture.gd`;
   the repo's `scripts/emergence.sh`/screenshot flow), zoomed to the world overview, and
   confirm a blue river network is visible over the continents, distinct from the ocean.
   The controller runs this and inspects the screenshot (as was done for the PR #126 PPM).

Gate: `cargo test -p anabios-godot` green, `cargo build -p anabios-godot` (cdylib)
succeeds, `cargo fmt --check`, `cargo clippy -p anabios-godot`.

## Constants (initial, tuned in impl via the screenshot)

```
RIVER_BLUE      = (0.18, 0.42, 0.72)   // brighter/cyan-er than ocean (0.09,0.19,0.44)
RIVER_MIX_MIN   = 0.55                  // floor so creeks read
RIVER_MIX_GAIN  = 0.45                  // extra blue with sqrt(river_flow)
RIVER_MIX_MAX   = 1.0
```

## Out of scope (later C specs)

Real elevation hillshade (feed the stored `elevation` field to the shader), minimap +
whole-world zoom-out, agent level-of-detail for sparse populations, river geometry/width
as explicit lines, and a dedicated river-overlay toggle.

## Milestone / delivery

A single small PR on the branch: pure `river_tint` helper + unit tests, wire it into
`biome_colors()`, tune the constants against a headless screenshot, done.
