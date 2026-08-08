# Rivers Visible in the Viewer — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the continental world's rivers visible in the Godot viewer by baking `river_flow` into the biome color bridge as a river-blue blend.

**Architecture:** A pure `river_tint(rgb, river_flow) -> rgb` helper (unit-tested, no `godot` types) blends river cells toward a blue that trips the terrain shader's existing `is_water()` water treatment. `biome_colors()` calls it after its existing biomass/succession/pollution blends. Pure Godot-side presentation — no simulation, determinism, or format impact.

**Tech Stack:** Rust (`anabios-godot` gdext cdylib), Godot 4.6+ viewer, `game/shaders/terrain.gdshader` (unchanged, consumed as-is).

## Global Constraints

- **Presentation only.** `biome_colors()` is read only by the viewer; it is NOT part of `state_hash`, touches no golden suite, and changes no sim behavior. No `FORMAT_VERSION` bump. Do not touch `anabios-core`.
- **Non-river cells must be byte-identical to today:** `river_tint(rgb, 0.0)` returns `rgb` unchanged, and the river blend is applied only when `cell.river_flow > 0.0`.
- The blended river color must satisfy the shader's `is_water()` predicate at full flow so rivers get water shimmer: `b > r + 0.05 && b > g + 0.05 && b > 0.20 && max(r, g) < 0.45` (from `game/shaders/terrain.gdshader`).
- Gate before commit: `cargo test -p anabios-godot`, `cargo build -p anabios-godot`, `cargo fmt --check`, `cargo clippy -p anabios-godot` all green. Run commands in the FOREGROUND. Stage explicit paths only (never `git add -A`).

---

## File Structure

- **Modify** `crates/anabios-godot/src/lib.rs` — add the `RIVER_*` constants + pure `river_tint` free function; call it inside `biome_colors()` (~line 856, before `out.push(c)`); add unit tests to the existing `#[cfg(test)] mod tests` (~line 1495).

That is the only source file. `game/shaders/terrain.gdshader` and `game/scripts/biome_renderer.gd` are consumed unchanged.

---

## Task 1: `river_tint` helper + wire into `biome_colors` + unit tests

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs` (new consts + `river_tint` fn near the other pure helpers like `hsv_to_color` ~1339; call site in `biome_colors` ~856; tests in `mod tests` ~1526)

**Interfaces:**
- Consumes: `anabios_core::biome::BiomeCell.river_flow: f32` (already serialized, PR #126); `godot::builtin::Color` (has public `f32` fields `.r/.g/.b` and `Color::from_rgb`).
- Produces: `fn river_tint(rgb: (f32, f32, f32), river_flow: f32) -> (f32, f32, f32)`.

- [ ] **Step 1: Write the failing tests.** Add to `#[cfg(test)] mod tests` in `crates/anabios-godot/src/lib.rs`:

```rust
    #[test]
    fn river_tint_zero_flow_is_identity() {
        let grass = (0.21_f32, 0.44, 0.19);
        assert_eq!(river_tint(grass, 0.0), grass);
        assert_eq!(river_tint(grass, -1.0), grass); // guard: no NaN from sqrt(<0)
    }

    #[test]
    fn river_tint_is_blue_dominant_and_monotonic() {
        let grass = (0.21_f32, 0.44, 0.19);
        let (r, g, b) = river_tint(grass, 0.3);
        assert!(b > grass.2, "river must raise the blue channel");
        assert!(b > r && b > g, "river cell must be blue-dominant to trip is_water()");
        let (_, _, b_lo) = river_tint(grass, 0.1);
        let (_, _, b_hi) = river_tint(grass, 0.9);
        assert!(b_hi >= b_lo, "higher flow must be at least as blue");
    }

    #[test]
    fn river_tint_full_flow_satisfies_shader_is_water() {
        // Mirror game/shaders/terrain.gdshader is_water() on the strongest river.
        let (r, g, b) = river_tint((0.21, 0.44, 0.19), 1.0);
        assert!(
            b > r + 0.05 && b > g + 0.05 && b > 0.20 && r.max(g) < 0.45,
            "full-flow river must satisfy is_water(): got ({r}, {g}, {b})"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail.**

Run: `cargo test -p anabios-godot river_tint`
Expected: FAIL — `cannot find function river_tint in this scope`.

- [ ] **Step 3: Add the constants and the pure helper.** Place near the other pure helpers (e.g. just above `fn hsv_to_color`):

```rust
/// River presentation color and blend curve (viewer-only; see
/// docs/superpowers/specs/2026-08-07-viewer-rivers-design.md). Brighter/cyan-er
/// than the ocean blue (0.09, 0.19, 0.44) so rivers read against land and sea.
const RIVER_BLUE: (f32, f32, f32) = (0.18, 0.42, 0.72);
/// Blend floor so even low-flow creeks read as water; gain adds blue with
/// sqrt(flow) so trunk rivers go fully blue. river_flow is normalized
/// (accum/max_accum), hence the floor + sqrt rather than a linear ramp.
const RIVER_MIX_MIN: f32 = 0.55;
const RIVER_MIX_GAIN: f32 = 0.45;
const RIVER_MIX_MAX: f32 = 1.0;

/// Blend a cell color toward `RIVER_BLUE` by an amount that rises with
/// `river_flow`. `river_flow <= 0.0` returns `rgb` unchanged (non-river cells
/// are identical to before). Pure (no `godot` types) so it is unit-testable.
fn river_tint(rgb: (f32, f32, f32), river_flow: f32) -> (f32, f32, f32) {
    if river_flow <= 0.0 {
        return rgb;
    }
    let mix =
        (RIVER_MIX_MIN + RIVER_MIX_GAIN * river_flow.max(0.0).sqrt()).clamp(0.0, RIVER_MIX_MAX);
    let lerp = |a: f32, b: f32| a + (b - a) * mix;
    (lerp(rgb.0, RIVER_BLUE.0), lerp(rgb.1, RIVER_BLUE.1), lerp(rgb.2, RIVER_BLUE.2))
}
```

- [ ] **Step 4: Run the tests to verify they pass.**

Run: `cargo test -p anabios-godot river_tint`
Expected: PASS (all three).

- [ ] **Step 5: Wire it into `biome_colors()`.** In `biome_colors` (`crates/anabios-godot/src/lib.rs`), the loop already builds a `let mut c: Color` and blends succession + pollution into it. Immediately **before** `out.push(c);`, add:

```rust
            if cell.river_flow > 0.0 {
                let (r, g, b) = river_tint((c.r, c.g, c.b), cell.river_flow);
                c = Color::from_rgb(r, g, b);
            }
```

- [ ] **Step 6: Full gate.**

Run:
```bash
cargo test -p anabios-godot && cargo build -p anabios-godot && cargo fmt --check && cargo clippy -p anabios-godot -- -D warnings
```
Expected: all green. (`cargo build -p anabios-godot` proves the cdylib still compiles with the new call site.)

- [ ] **Step 7: Commit.**

```bash
git add crates/anabios-godot/src/lib.rs
git commit -m "feat(viewer): render rivers as blue channels in biome_colors"
```

---

## Task 2: Headless visual verification + constant tuning

Verify rivers actually read on-screen at the continental scale, and tune `RIVER_BLUE` / `RIVER_MIX_*` if the network is too faint or too bold. This task needs a working Godot 4.6+ install and the `anabios-godot` cdylib; the controller typically runs it (as it did for the PR #126 PPM).

**Files:**
- Modify (only if tuning is needed): `crates/anabios-godot/src/lib.rs` (the `RIVER_*` constants)

**Interfaces:**
- Consumes: `scenarios/continental.toml` (flagship, PR #126); `game/scripts/debug_capture.gd` (env-gated screenshot harness); `game/scripts/main.gd`.

- [ ] **Step 1: Build the cdylib.**

Run: `cargo build -p anabios-godot`
Expected: builds the gdext cdylib the Godot project loads.

- [ ] **Step 2: Headless screenshot of the continental world.** Use the repo's screenshot flow (inspect `game/scripts/debug_capture.gd` for the exact env vars / `scripts/emergence.sh` view/screenshot subcommand). Boot `scenarios/continental.toml` at a world-overview zoom and capture a PNG. Example shape (confirm the actual harness invocation from `debug_capture.gd`):

```bash
# The exact command comes from debug_capture.gd's env gate — read it first.
# It boots res://scenes/main.tscn headless, loads the scenario, screenshots, quits.
```

- [ ] **Step 3: Inspect the screenshot.** Confirm a **blue river network** is visible threading the continents, distinct from the darker ocean, with trunk rivers bolder than headwater creeks. Read the PNG (convert PPM→PNG with `sips -s format png` if the harness emits PPM).

- [ ] **Step 4: Tune if needed.** If rivers are too faint (lost against wet-forest green) raise `RIVER_MIX_MIN` / `RIVER_MIX_GAIN` or brighten `RIVER_BLUE`; if too bold/blocky, lower them. Re-run Steps 1–3. Keep the fully-blended color passing `is_water()` (the Task 1 unit test guards this — re-run `cargo test -p anabios-godot river_tint` after any constant change).

- [ ] **Step 5: Commit any tweak (skip if no change was needed).**

```bash
git add crates/anabios-godot/src/lib.rs
git commit -m "style(viewer): tune river-blue blend for legibility at continental scale"
```

---

## Self-Review Notes (for the executor)

- **Spec coverage:** river-blue blend baked into `biome_colors` (T1); pure `river_tint` helper + unit tests incl. the `is_water()` predicate check (T1); visual verification on `continental.toml` (T2); constant tuning (T2). All spec sections map to a task.
- **Determinism:** `biome_colors` is not in `state_hash` and no core file is touched, so there is no golden/format impact — nothing to regenerate. `river_tint(_, 0.0)` identity keeps non-river cells unchanged.
- **Verified facts:** `anabios-godot` already has a `#[cfg(test)] mod tests` that `cargo test -p anabios-godot` runs (despite the `cdylib` crate-type); `biome_colors` already uses `let mut c: Color`, `Color::from_rgb`, and `.lerp`, and `Color` exposes public `f32` `.r/.g/.b`; `river_flow` is a serialized `BiomeCell` field from PR #126.
- **T2 environment dependency:** Task 2 needs Godot + the cdylib and visual judgment — best run by the controller. If Godot is unavailable, T1 still delivers the code (unit-verified) and T2's visual confirmation is deferred, noted explicitly rather than silently skipped.
