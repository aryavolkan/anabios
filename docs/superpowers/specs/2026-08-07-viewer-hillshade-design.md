# Real elevation hillshade in the viewer (sub-project C, spec 2)

**Date:** 2026-08-07
**Status:** Approved design → implementation
**Branch:** `claude/2d-world-scale-159e3b`

## Context: the larger arc

Sub-project **C** ("viewer at scale") of "actual world with good scale in 2D".
**A** (continental worldgen, PR #126) stores `elevation` and `river_flow` on every
`BiomeCell`. **C spec 1** (rivers visible, PR #126) made `river_flow` render. This is
**C spec 2 — real elevation hillshade**. Remaining C work (minimap + wider zoom-out,
agent LOD, ocean bathymetry) stays in later specs.

## Motivation

The terrain shader (`game/shaders/terrain.gdshader`) lights land by treating **color
luminance as height** (lines 92–100): `grad = (luma(cR)-luma(cL), luma(cD)-luma(cU))`,
Lambert-shaded by `light_dir`. This is a fake — a bright Desert reads as "high", a dark
Forest as "low" — so the mountain ranges carved in PR #126 get **no real relief** (they
render as flat grey). The true `elevation` field is stored on every cell but never
reaches the shader.

This spec feeds real elevation to the shader so hillshade reflects actual topography.

## Current data path (what exists)

`biome_colors()` (Rust→Godot bridge, `crates/anabios-godot/src/lib.rs`) returns one
`Color` per cell (RGB = terrain/biomass/succession/pollution/river blend; alpha `1.0`).
`game/scripts/biome_renderer.gd` writes them into a `res×res` RGBA8 `ImageTexture`
(scaled to `world_size`, 3×3 torus wrap, LINEAR filter). `terrain.gdshader` presents it:
softens, derives the luminance hillshade on land, applies water treatment to
color-detected water, and — crucially — **forces opaque output**:
`COLOR = vec4(clamp(outc,0,1), 1.0) * tint` (line 124, both branches). So the texture's
alpha channel is **currently unused for rendering** and free to carry data.

## Approach (locked): pack elevation into the texture alpha

1. **Bridge:** in `biome_colors()`, set each returned cell color's **alpha to
   `cell.elevation`** (clamped `[0,1]`). RGB is unchanged (terrain + biomass +
   succession + pollution + C1 river-blue all as today). Safe because the shader forces
   opaque output — alpha never affected rendering.
2. **Shader (land branch only):** replace the four `luma(cX)` pseudo-heights with the
   neighbors' **alpha = real elevation**. Sample the alpha at the four neighbor taps
   (the shader already reads `cL/cR/cU/cD` for softening/relief; add their `.a`), and
   compute `grad = (elevR - elevL, elevD - elevU)`, `shade = dot(normalize(light_dir),
   grad) * relief_strength`, exactly as now but with true height. Mountain ranges gain
   real directional relief; flat plains stop being lit by biome color.

**Why alpha-packing over a separate elevation texture:** minimal change — one texture,
reuses the existing neighbor taps, no new bridge function / `ImageTexture` / sampler /
sync-cadence code. Output opacity is already forced to `1.0`, so overloading alpha is
invisible to rendering. The separate-texture route (a `elevation_values()` bridge + a
second synced `ImageTexture` + a `sampler2D` uniform) is cleaner in the abstract but
~40 lines of plumbing for no visible gain here; it remains the fallback if alpha-packing
ever conflicts with a future overlay need.

### Alpha in the other ground modes
Data overlays (pheromone/optimum/succession/market) run with `biome_mode = 0`
(passthrough) and their own color arrays; the shader ignores alpha there too (output is
still forced opaque), so packing elevation into the *biome* array's alpha does not affect
overlay rendering. Elevation-in-alpha is only *read* in the land branch, which only runs
in biome mode.

## Determinism / blast radius

Pure Godot-side presentation. `biome_colors()` is viewer-read only — not in `state_hash`,
no golden suite, no `FORMAT_VERSION`, no `anabios-core` change. Files touched:
`crates/anabios-godot/src/lib.rs` (alpha assignment) and `game/shaders/terrain.gdshader`
(land relief). Improves **every** scenario's viewer (all worlds store real elevation),
behavior-neutral.

## Testing

Hillshade is inherently visual: GLSL is not unit-testable, and the bridge change is a
one-line alpha assignment. Verification is therefore primarily a **before/after
screenshot** of `scenarios/continental.toml` (seed 7) via the env-gated `debug_capture`
harness (`game/scripts/debug_capture.gd`; boot `res://scenes/main.tscn` windowed,
`ANABIOS_GROUND=0`, world-overview zoom). Acceptance:

- Mountain ranges show clear **directional relief** driven by topography (ridgelines
  lit on the light-facing side, shadowed on the lee) — not by biome color.
- Colors and rivers (C1) are unchanged where elevation is flat.
- Plains no longer show spurious luminance-driven "relief" from biome color edges.

`relief_strength` (currently `0.9`) is retuned against the screenshot, since the
per-cell elevation gradient scale differs from the luma gradient it replaces.

Code sanity gate: `cargo test -p anabios-godot` (unchanged tests still pass),
`cargo build -p anabios-godot` (cdylib compiles), `cargo fmt --check`,
`cargo clippy -p anabios-godot`.

Optional (only if cheap): extract the per-cell color into a pure
`fn cell_color(cell) -> (f32,f32,f32,f32)` helper so the alpha-carries-elevation
contract gets one pure-Rust unit test (mirrors the C1 `river_tint` extraction). Not
required — the alpha assignment is a one-liner and the real risk is the shader look,
which is visual.

## Out of scope (later C specs)

Ocean bathymetry (depth-shaded water from real elevation — needs `sea_level` as a shader
uniform), normal-mapped or soft-shadow relief, minimap + wider zoom-out, agent LOD.

## Milestone / delivery

A single small change on the branch (extends PR #126): set alpha = elevation in
`biome_colors()`, switch the shader's land relief to sample that alpha, tune
`relief_strength` against a headless screenshot. Verify the whole `river_flow`/`elevation`
viewer story reads on `continental.toml`.
