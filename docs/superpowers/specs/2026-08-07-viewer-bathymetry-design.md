# Ocean bathymetry in the viewer (sub-project C, spec 3)

**Date:** 2026-08-07
**Status:** Approved design → implementation
**Branch:** `claude/2d-world-scale-159e3b`

## Context: the larger arc

Sub-project **C** ("viewer at scale") of "actual world with good scale in 2D".
**A** (continental worldgen, PR #126) stores `elevation`/`river_flow` per `BiomeCell`.
**C spec 1** (rivers visible) and **C spec 2** (real elevation hillshade) shipped in
PR #126. This is **C spec 3 — ocean bathymetry**: shade water depth by real elevation.
Remaining C work (minimap + wider zoom-out, agent LOD) stays in later specs.

## Motivation

The water branch of `game/shaders/terrain.gdshader` shades depth by **coast proximity**
(`coast` = fraction of the 4 neighbours that are land): cells read "shallow" only near a
coastline, "deep" everywhere else. It ignores real depth, so a deep central basin and a
shallow shelf far from shore look identical. With continent masks (A), ocean basins now
have genuine depth variation — a floor near `DEEP_OCEAN_ELEV ≈ 0.15`, higher shelves
toward coasts — that the viewer discards. After C2, every water cell already carries its
real `elevation` in the biome texture's alpha channel, so the data is already at the
shader; it just isn't used for depth.

## Current water rendering (what exists)

In `terrain.gdshader`'s `if (water)` branch: `outc = mix(deep, shallow, coast)` (deeper =
darker/more saturated in open sea, brighter teal toward coasts), plus a directional
`shimmer` in seamless world coordinates and a crisp `surf` rim at the waterline (both
keyed off `coast`). `sea_level` is **not** known to the shader, and **not stored** on
`World`/`BiomeField` — it is only used at generation (`classify`: a cell is Water iff
`elevation < sea_level`).

## Approach (locked): shade water by real elevation depth

1. **Bridge — derive `sea_level` (no core change).** Add a pure free function
   `fn water_line(cells: &[BiomeCell]) -> f32` returning the **maximum `elevation`
   among Water-terrain cells** (the tight lower bound on `sea_level`; every land cell
   sits at or above it). Falls back to `biome::SEA_LEVEL` (the default) when a world has
   no water. Wrap it in a `#[func] fn sea_level(&self) -> f32` on the sim node. This is a
   pure derived read of existing data — no `World`/`BiomeField` field, no serialization,
   no `FORMAT_VERSION` bump, not in `state_hash`.
2. **GDScript — pass it to the shader.** In `game/scripts/biome_renderer.gd`'s setup
   (where `world_size` is already pushed to the material), set a `sea_level` shader
   uniform once from `sim.sea_level()`.
3. **Shader — depth from elevation.** Add `uniform float sea_level`. In the water branch,
   read the center cell's alpha (= real `elevation`, from C2) and compute
   `shallowness = clamp(elev / max(sea_level, 1e-3), 0.0, 1.0)` — ~1 at the shoreline
   (elevation just below sea level), →0 in the deepest basin. Drive the existing
   deep↔shallow colour `mix` by `shallowness` instead of `coast`, so open-ocean trenches
   read dark and coastal shelves read bright teal. **Keep** the `coast`-based surf rim and
   the shimmer unchanged (coastline crispness is still a proximity effect).

**Why derive `sea_level` rather than store it:** storing it means a `World`/`BiomeField`
field + `FORMAT_VERSION` bump + golden regen for a purely cosmetic viewer need. Deriving
it from the elevation field is exact-to-one-cell and keeps this change presentation-only,
consistent with C1/C2. Storing it as first-class state is a deliberate future step only if
another consumer needs it.

## Determinism / blast radius

Pure Godot-side presentation. Files: `crates/anabios-godot/src/lib.rs` (the `water_line`
free fn + `#[func] sea_level`), `game/scripts/biome_renderer.gd` (one uniform),
`game/shaders/terrain.gdshader` (water branch depth). No `anabios-core`, no goldens, no
format bump. Improves every scenario's viewer.

## Testing

1. **Unit test (pure Rust, godot crate `#[cfg(test)] mod tests`).** `water_line` over a
   `BiomeField::generate_with(seed, res, world, &climate)` world at a known `sea_level`:
   - `water_line` is within one elevation-quantum below `climate.sea_level` (i.e.
     `water_line <= climate.sea_level` and `> 0`).
   - **Separator property:** every `Water` cell's `elevation <= water_line`, and every
     land cell's `elevation >= water_line` (so it cleanly divides sea from land).
   - A knobs-off default world (`ClimateParams::default()`, `sea_level = SEA_LEVEL`)
     yields `water_line ≈ SEA_LEVEL`.
   - Edge case: an all-land field (construct or pick a config with no Water) returns the
     `SEA_LEVEL` fallback rather than a sentinel/NaN.
2. **Visual (headless).** Screenshot `scenarios/continental.toml` (seed 7) via the
   `debug_capture` harness at a world-overview zoom: open ocean is visibly darker/deeper
   than coastal shelves and shallows; coastlines, surf, and shimmer are unchanged. The
   controller runs and inspects it, as for C1/C2.

Gate: `cargo test -p anabios-godot`, `cargo build -p anabios-godot`, `cargo fmt --check`,
`cargo clippy -p anabios-godot` green.

## Out of scope (later C specs)

Minimap + wider zoom-out, agent level-of-detail, animated caustics / depth fog, storing
`sea_level` as first-class simulation state.

## Milestone / delivery

A single small change on the branch (extends PR #126, or a fresh PR if #126 has merged
by then): `water_line` + `#[func] sea_level` (with a unit test), one shader uniform set
from GDScript, the water-branch depth term, tuned against a headless screenshot.
