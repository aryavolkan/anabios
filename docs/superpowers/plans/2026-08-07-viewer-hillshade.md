# Real Elevation Hillshade — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drive the terrain shader's land hillshade from the real `elevation` field (packed into the biome texture's alpha) instead of color luminance, so mountain ranges get true relief.

**Architecture:** `biome_colors()` sets each cell color's alpha to `cell.elevation`. The shader's land branch samples the four neighbor alphas (= elevation) to build the height gradient it already uses for Lambert shading. Pure Godot-side presentation — no sim, determinism, or format impact. Output opacity is already forced to `1.0`, so alpha is free.

**Tech Stack:** Rust (`anabios-godot` gdext cdylib), Godot 4.7 viewer, `game/shaders/terrain.gdshader`.

## Global Constraints

- **Presentation only.** `biome_colors()` is viewer-read; NOT in `state_hash`, no golden suite, no `FORMAT_VERSION`, no `anabios-core` change.
- **RGB unchanged.** Only the alpha channel of each biome cell color changes (to elevation); the RGB blend (terrain/biomass/succession/pollution + C1 river-blue) is byte-identical.
- **Land branch only.** The shader change is confined to the `else` (land) branch of `fragment()`; the water branch, softening, coast, vignette, and passthrough (`biome_mode = 0`) are untouched.
- Gate: `cargo test -p anabios-godot`, `cargo build -p anabios-godot`, `cargo fmt --check`, `cargo clippy -p anabios-godot` green. Run commands FOREGROUND. Stage explicit paths only.

---

## File Structure

- **Modify** `crates/anabios-godot/src/lib.rs` — in `biome_colors()`, set `c.a = cell.elevation` before `out.push(c)`.
- **Modify** `game/shaders/terrain.gdshader` — in the land branch, replace the four `luma(cX)` heights with four elevation (alpha) taps; bump the `relief_strength` default (elevation gradient is smaller-scale than luma).

---

## Task 1: Elevation → alpha (bridge) + real hillshade (shader)

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs` (`biome_colors`, the `out.push(c)` site ~866)
- Modify: `game/shaders/terrain.gdshader` (land branch ~92–100; `relief_strength` uniform ~17)

**Interfaces:**
- Consumes: `anabios_core::biome::BiomeCell.elevation: f32` (serialized, PR #126, already `[0,1]`); the shader's existing `light_dir`, `relief_strength`, `TEXTURE`, `px` (= `TEXTURE_PIXEL_SIZE`).

- [ ] **Step 1: Bridge — pack elevation into alpha.** In `biome_colors()` (`crates/anabios-godot/src/lib.rs`), the loop ends with the C1 river block then `out.push(c);`. Immediately **before** `out.push(c);`, add:

```rust
            // Pack real elevation into alpha for the terrain shader's hillshade
            // (C2). The shader forces opaque output, so alpha never affects
            // rendering — it is a free data channel. RGB is unchanged.
            c.a = cell.elevation.clamp(0.0, 1.0);
```

- [ ] **Step 2: Build the cdylib to confirm the bridge compiles.**

Run: `cargo build -p anabios-godot`
Expected: builds clean (Godot loads this dylib).

- [ ] **Step 3: Shader — hillshade from elevation.** In `game/shaders/terrain.gdshader`, in the **land branch** (the `} else {` after the water branch), replace:

```glsl
		// Land relief: treat luminance as pseudo-height and light it directionally.
		float hL = luma(cL);
		float hR = luma(cR);
		float hU = luma(cU);
		float hD = luma(cD);
		vec2 grad = vec2(hR - hL, hD - hU);
		float shade = dot(normalize(light_dir), grad) * relief_strength;
```

with the real-elevation version (alpha taps at the same four neighbor offsets already used for the rgb taps):

```glsl
		// Land relief: sample the packed real elevation (biome texture alpha) at
		// the four neighbours and light the height gradient directionally. This
		// replaces the old luminance-as-height fake so mountain ranges (not
		// bright biomes) cast the relief.
		float eL = texture(TEXTURE, UV - vec2(px.x, 0.0)).a;
		float eR = texture(TEXTURE, UV + vec2(px.x, 0.0)).a;
		float eU = texture(TEXTURE, UV - vec2(0.0, px.y)).a;
		float eD = texture(TEXTURE, UV + vec2(0.0, px.y)).a;
		vec2 grad = vec2(eR - eL, eD - eU);
		float shade = dot(normalize(light_dir), grad) * relief_strength;
```

- [ ] **Step 4: Raise the `relief_strength` default.** The per-cell elevation gradient (`~0.01–0.03` between adjacent cells of a smooth fBm field) is much smaller than the old luma gradient, so the existing `relief_strength = 0.9` would produce nearly flat shading. Change the uniform default (`game/shaders/terrain.gdshader` ~line 17) to a stronger initial value (tuned in Task 2):

```glsl
uniform float relief_strength = 12.0;  // hillshade intensity on land (elevation gradient)
```

- [ ] **Step 5: Gate.**

Run:
```bash
cargo test -p anabios-godot && cargo build -p anabios-godot && cargo fmt --check && cargo clippy -p anabios-godot -- -D warnings
```
Expected: all green. (Existing `anabios-godot` tests, incl. the C1 `river_tint` tests, still pass — RGB is unchanged, so `river_tint` is unaffected.)

- [ ] **Step 6: Commit.**

```bash
git add crates/anabios-godot/src/lib.rs game/shaders/terrain.gdshader
git commit -m "feat(viewer): real elevation hillshade from biome-texture alpha"
```

---

## Task 2: Headless before/after verification + tune `relief_strength`

Confirm mountain relief now tracks topography, and tune `relief_strength` so ranges read without over-darkening plains. Needs Godot 4.7 + the cdylib; the controller runs it (as for C1).

**Files:**
- Modify (only if tuning is needed): `game/shaders/terrain.gdshader` (`relief_strength` default)

**Interfaces:**
- Consumes: `scenarios/continental.toml` (seed 7); `game/scripts/debug_capture.gd` env-gated harness.

- [ ] **Step 1: Build the cdylib.**

Run: `cargo build -p anabios-godot`

- [ ] **Step 2: Capture a world-overview screenshot.** Boot the main scene windowed with the continental world at a wide zoom (the harness rejects `--headless`; boot `res://scenes/main.tscn` directly — the default boot is the menu):

```bash
SP=<scratchpad>
ANABIOS_SHOT="$SP/hillshade.png" \
ANABIOS_SCENARIO="res://../scenarios/continental.toml" \
ANABIOS_SEED=7 ANABIOS_GROUND=0 \
ANABIOS_CAM_X=2048 ANABIOS_CAM_Y=2048 ANABIOS_ZOOM=0.3 \
ANABIOS_SHOT_FRAMES=90 \
godot --path game/ res://scenes/main.tscn --resolution 1280x1280
```

- [ ] **Step 3: Inspect.** Read the PNG. Confirm:
  - Mountain ranges show clear **directional relief** (ridges lit on the `light_dir` side, shadowed on the lee) that follows topography, not biome color.
  - Colors and rivers (C1) are unchanged; plains are not spuriously "lit" by biome-color edges.

- [ ] **Step 4: Tune `relief_strength` if needed.** Too flat → raise it; harsh/noisy (every cell edge shimmering) → lower it. Re-run Steps 1–3. Keep it a single uniform-default change in `game/shaders/terrain.gdshader`.

- [ ] **Step 5: Commit any tweak (skip if the Task 1 value was right).**

```bash
git add game/shaders/terrain.gdshader
git commit -m "style(viewer): tune hillshade relief_strength for continental relief"
```

---

## Self-Review Notes (for the executor)

- **Spec coverage:** elevation→alpha (T1 Step 1); shader land relief from real elevation (T1 Steps 3–4); visual before/after verification + tuning (T2). All spec sections map to a task.
- **Determinism:** `biome_colors()` isn't in `state_hash`; no core/golden/format touched. RGB unchanged, so the C1 `river_tint` tests and river rendering are unaffected.
- **Verified facts:** the shader forces opaque output in both branches (`COLOR = vec4(..., 1.0) * tint`, line 124), so packing elevation into texture alpha does not change rendering opacity; the land branch (`} else {`) is the only place height is used; the four neighbor offsets (`px.x`/`px.y`) already exist for the rgb taps; `elevation` is a serialized `[0,1]` `BiomeCell` field; the `debug_capture` harness boots `main.tscn` windowed and reaches repo scenarios via `res://../scenarios/`.
- **T2 environment dependency:** needs Godot + a windowed run (rejects `--headless`) and visual judgment — best run by the controller. If unavailable, T1 still lands the (compile-checked) code and T2's visual confirmation is noted as deferred, not silently skipped.
- **Tuning caveat:** `relief_strength = 12.0` is a reasoned first guess (elevation gradient ≈ 1–3% per cell vs. the old luma gradient); the real value comes from T2's screenshot.
