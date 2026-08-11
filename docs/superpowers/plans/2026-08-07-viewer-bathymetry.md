# Ocean Bathymetry — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Shade ocean depth by real elevation (deep basins dark, coastal shelves bright) instead of coast proximity, by deriving `sea_level` from the elevation field and passing it to the terrain shader.

**Architecture:** A pure `water_line(cells)` helper derives the sea level (max elevation among Water cells) — exposed via a `#[func] sea_level()`. `biome_renderer` pushes it as a shader uniform. The shader's water branch reads the center cell's alpha (= real elevation, from C2) and drives the deep↔shallow mix by `elev / sea_level`. Pure Godot-side presentation.

**Tech Stack:** Rust (`anabios-godot` gdext cdylib), Godot 4.7 viewer, `game/shaders/terrain.gdshader`, `game/scripts/biome_renderer.gd`.

## Global Constraints

- **Presentation only.** No `anabios-core` change, not in `state_hash`, no golden suite, no `FORMAT_VERSION` bump. `sea_level` is DERIVED, not stored.
- **Water branch only.** The shader change is confined to the `if (water)` branch's depth mix; the surf rim and shimmer (both `coast`-keyed) and the land branch stay untouched.
- Gate: `cargo test -p anabios-godot`, `cargo build -p anabios-godot`, `cargo fmt --check`, `cargo clippy -p anabios-godot` green. Run commands FOREGROUND. Stage explicit paths only.
- Note: the branch was just merged with `main` (`20c5cbe`); build against the current merged tree.

---

## File Structure

- **Modify** `crates/anabios-godot/src/lib.rs` — add pure `fn water_line(cells: &[BiomeCell]) -> f32` (near other free helpers) + `#[func] fn sea_level(&self) -> f32` on the sim node; unit test in the existing `#[cfg(test)] mod tests` (~line 1564).
- **Modify** `game/scripts/biome_renderer.gd` — set a `sea_level` shader uniform in `_setup()` alongside `world_size` (~line 74).
- **Modify** `game/shaders/terrain.gdshader` — add `uniform float sea_level`; in the water branch, drive the deep↔shallow mix by elevation-derived `shallowness`.

---

## Task 1: Derive `sea_level` + elevation-based water depth

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs`
- Modify: `game/scripts/biome_renderer.gd`
- Modify: `game/shaders/terrain.gdshader`

**Interfaces:**
- Consumes: `anabios_core::biome::{BiomeCell, TerrainType, SEA_LEVEL, BiomeField, ClimateParams}`; the sim node's `self.inner: Option<World>` with `w.biome.cells`; the shader's water branch (`terrain.gdshader:74–90`), the center texel alpha (`texture(TEXTURE, UV).a` = elevation, from C2).
- Produces: `#[func] fn sea_level(&self) -> f32`; a `sea_level` shader uniform.

- [ ] **Step 1: Write the failing unit test.** Add to `#[cfg(test)] mod tests` in `crates/anabios-godot/src/lib.rs`:

```rust
    #[test]
    fn water_line_separates_sea_from_land() {
        use anabios_core::biome::{BiomeField, ClimateParams, TerrainType, SEA_LEVEL};
        // Empty field → SEA_LEVEL fallback (all-land / no-water edge case).
        assert_eq!(water_line(&[]), SEA_LEVEL);

        // Continental config with real oceans.
        let climate = ClimateParams { continentality: 0.9, sea_level: 0.45, ..Default::default() };
        let f = BiomeField::generate_with(7, 128, 4096.0, &climate);
        let wl = water_line(&f.cells);
        assert!(wl > 0.0 && wl <= climate.sea_level + 1e-6, "water_line in (0, sea_level]: {wl}");
        for c in &f.cells {
            if c.terrain == TerrainType::Water {
                assert!(c.elevation <= wl + 1e-6, "water cell above water_line");
            } else {
                assert!(c.elevation >= wl - 1e-6, "land/rock cell below water_line");
            }
        }

        // Default world: water_line ≈ SEA_LEVEL (shallowest water sits just below it).
        let d = BiomeField::generate(1, 96, 1024.0);
        let wld = water_line(&d.cells);
        assert!((wld - SEA_LEVEL).abs() < 0.05, "default water_line near SEA_LEVEL: {wld}");
    }
```

- [ ] **Step 2: Run it to verify it fails.**

Run: `cargo test -p anabios-godot water_line`
Expected: FAIL — `cannot find function water_line in this scope`.

- [ ] **Step 3: Add the `water_line` helper + `#[func] sea_level`.** Add the pure helper near the other free helpers (e.g. above `river_tint`):

```rust
/// Effective sea level for the viewer, DERIVED from the elevation field (no core
/// storage): the highest elevation still classified as water. `classify` makes a
/// cell Water iff `elevation < sea_level`, so the max water elevation is the tight
/// lower bound on `sea_level`, and every land/rock cell sits at or above it.
/// Falls back to `SEA_LEVEL` for an all-land field. Pure (no `godot` types) —
/// unit-tested.
fn water_line(cells: &[anabios_core::biome::BiomeCell]) -> f32 {
    let mut max_water = f32::MIN;
    for cell in cells {
        if cell.terrain == anabios_core::biome::TerrainType::Water && cell.elevation > max_water {
            max_water = cell.elevation;
        }
    }
    if max_water == f32::MIN {
        anabios_core::biome::SEA_LEVEL
    } else {
        max_water
    }
}
```

Add the exported accessor as a `#[func]` method on the sim node (near `fn world_size` / `fn biome_resolution`, inside the same `#[godot_api] impl` block):

```rust
    /// Viewer-derived sea level (max water-cell elevation) for the terrain
    /// shader's depth shading. Pure read; not simulation state.
    #[func]
    fn sea_level(&self) -> f32 {
        self.inner
            .as_ref()
            .map(|w| water_line(&w.biome.cells))
            .unwrap_or(anabios_core::biome::SEA_LEVEL)
    }
```

- [ ] **Step 4: Run the test to verify it passes.**

Run: `cargo test -p anabios-godot water_line`
Expected: PASS.

- [ ] **Step 5: Pass `sea_level` to the shader from GDScript.** In `game/scripts/biome_renderer.gd`, `_setup()` already does (inside `if _terrain_mat != null:`):

```gdscript
		_terrain_mat.set_shader_parameter("world_size", world)
```

Add immediately after it:

```gdscript
		_terrain_mat.set_shader_parameter("sea_level", sim.sea_level())
```

- [ ] **Step 6: Add the shader uniform.** In `game/shaders/terrain.gdshader`, next to the other uniforms (after `world_size`, ~line 20):

```glsl
uniform float sea_level = 0.35;        // viewer-derived water line, for depth shading
```

- [ ] **Step 7: Drive water depth by elevation.** In the `if (water)` branch of `fragment()` (`terrain.gdshader` ~74–79), replace:

```glsl
		vec3 deep = base * vec3(0.72, 0.82, 1.05);
		vec3 shallow = mix(base, vec3(0.30, 0.62, 0.72), 0.55);
		outc = mix(deep, shallow, coast);
```

with (real bathymetry — the center texel's alpha is the cell's elevation, packed in C2):

```glsl
		vec3 deep = base * vec3(0.72, 0.82, 1.05);
		vec3 shallow = mix(base, vec3(0.30, 0.62, 0.72), 0.55);
		// Real depth: elevation (packed in alpha) relative to the water line.
		// ~1 at the shoreline (elevation just below sea level), →0 in the deepest
		// basin, so open-ocean trenches read dark and shelves read bright teal.
		float elev = texture(TEXTURE, UV).a;
		float shallowness = clamp(elev / max(sea_level, 1e-3), 0.0, 1.0);
		outc = mix(deep, shallow, shallowness);
```

Leave the shimmer and surf lines below (both keyed off `coast`) unchanged.

- [ ] **Step 8: Gate.**

Run:
```bash
cargo test -p anabios-godot && cargo build -p anabios-godot && cargo fmt --check && cargo clippy -p anabios-godot -- -D warnings
```
Expected: all green (existing tests incl. C1 `river_tint` and C2 still pass — RGB and the land branch are unchanged).

- [ ] **Step 9: Commit.**

```bash
git add crates/anabios-godot/src/lib.rs game/scripts/biome_renderer.gd game/shaders/terrain.gdshader
git commit -m "feat(viewer): ocean bathymetry — depth-shade water by real elevation"
```

---

## Task 2: Headless verification + tune

**Files:**
- Modify (only if tuning is needed): `game/shaders/terrain.gdshader` (depth mix curve) — e.g. a `pow(shallowness, k)` shaping if the shelf/deep contrast needs adjusting.

**Interfaces:**
- Consumes: `scenarios/continental.toml` (seed 7); `game/scripts/debug_capture.gd`.

- [ ] **Step 1: Build the cdylib.**

Run: `cargo build -p anabios-godot`

- [ ] **Step 2: Capture a world-overview screenshot.** Boot the main scene windowed (harness rejects `--headless`; default boot is the menu, so target `res://scenes/main.tscn` directly):

```bash
SP=<scratchpad>
ANABIOS_SHOT="$SP/bathymetry.png" \
ANABIOS_SCENARIO="res://../scenarios/continental.toml" \
ANABIOS_SEED=7 ANABIOS_GROUND=0 \
ANABIOS_CAM_X=2048 ANABIOS_CAM_Y=2048 ANABIOS_ZOOM=0.3 \
ANABIOS_SHOT_FRAMES=90 \
godot --path game/ res://scenes/main.tscn --resolution 1280x1280
```

- [ ] **Step 3: Inspect.** Read the PNG. Confirm:
  - Open-ocean basins read visibly **darker/deeper** than coastal shelves/shallows (a real depth gradient across the sea, not just a coastline halo).
  - Coastlines, surf rim, and shimmer are unchanged; land (hillshade, rivers, colors) is unchanged.

- [ ] **Step 4: Tune if needed.** If the gradient is too subtle, apply a shaping curve (`shallowness = pow(clamp(elev / max(sea_level,1e-3),0,1), 0.6)`), or blend a little `coast` back for crisper shelves; if too dark mid-ocean, raise the deep floor. Re-run Steps 1–3.

- [ ] **Step 5: Commit any tweak (skip if Task 1 was right).**

```bash
git add game/shaders/terrain.gdshader
git commit -m "style(viewer): tune ocean depth gradient"
```

---

## Self-Review Notes (for the executor)

- **Spec coverage:** `water_line` derive + `#[func] sea_level` with unit test (T1 S1–4); GDScript uniform (T1 S5); shader uniform + elevation depth (T1 S6–7); visual verify + tune (T2). All spec sections map to a task.
- **Determinism:** no `anabios-core` change, `sea_level` derived not stored, `biome_colors`/state_hash untouched. Land branch and RGB unchanged, so C1 rivers and C2 hillshade are unaffected.
- **Verified facts (merged tree):** shader forces opaque output and the center texel alpha carries elevation (C2); the water branch mix is `outc = mix(deep, shallow, coast)` at `terrain.gdshader:79`; `biome_renderer._setup` sets `world_size` at line 74; the sim node exposes `#[func]` scalar accessors (`world_size`, `biome_resolution`) and holds `inner: Option<World>`; `SEA_LEVEL` const and `TerrainType::Water` exist in `anabios-core::biome`; `anabios-godot` has a `#[cfg(test)] mod tests` that `cargo test -p anabios-godot` runs.
- **Separator property:** `classify` makes a cell Water iff `elevation < sea_level`; so `max_water_elev < sea_level <= min_land_elev` — `water_line` (= max water elevation) cleanly separates the two, which the unit test asserts.
- **T2 environment dependency:** needs Godot + a windowed run + visual judgment — best run by the controller. If unavailable, T1 lands the (unit-tested) code and T2's visual confirmation is noted as deferred, not silently skipped.
