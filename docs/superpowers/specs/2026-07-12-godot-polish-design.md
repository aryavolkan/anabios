# Godot sandbox polish — design

**Date:** 2026-07-12
**Project:** anabios (`game/` + `crates/anabios-godot/`)
**Status:** approved, ready for implementation
**Sequence:** first of two polish specs (Godot first, then the web frontend artifact).

## Goal

Close the gap between what the anabios simulation *does* and what the Godot sandbox
lets you *see and reach*. Today the renderer draws bodies, biome, and per-module
glyphs, with an inspector, codex feed, camera, and time controls — but:

- The menu exposes only **2 of 14 scenarios** (`minimal`, `divergent`); every
  milestone (M12–M15) and DIT scenario is unreachable.
- Core mechanics are **invisible**: pheromone fields (M13), carcasses / weapon fire
  (M12), meme/dialect divergence (M14), and the entire DIT env-optimum experiment
  leave no on-screen trace.
- The inspector is minimal (energy / age / program-len / module-count only).

This spec covers all four requested dimensions — **scenario breadth, mechanic
visibility, analytics & inspector, visual & UX refinement** — built as one coherent
*layered observability system* on the existing `Simulation` node.

## Guiding constraint: determinism is untouched

Every substrate addition is a **read-only accessor** on the existing `World`. No new
`World` fields, no changes to any sim step. Therefore `state_hash` and the
`determinism.rs` golden hashes **do not change**, and no golden refresh is required.
This is the load-bearing invariant of the whole spec: if a change would alter a hash,
it is out of scope. (See `anabios-ci-gates` — CI runs `fmt --check`, clippy `-D
warnings`, rustdoc `-D warnings`, and the workspace tests on the stable toolchain;
the local gate must be run with `rustup run stable` to match.)

## Architecture

The design adds:

1. A set of read-only `#[func]` exports to `crates/anabios-godot/src/lib.rs`.
2. A GDScript **overlay manager** (`game/scripts/overlay_manager.gd`) owning two
   independent, hotkey-cycled display modes: a **ground layer** and a **body color
   mode**.
3. Two new HUD panels (population, DIT) plus a richer inspector and a legend overlay.
4. A full scenario menu with per-scenario default overlay/color selection.

Data flow is unchanged in shape: `main.gd::_process` steps the sim, then pulls typed
arrays from `Simulation` and uploads them to `MultiMesh` / `ImageTexture`. The new
work is *additional* pulls plus a selector deciding which array feeds the body
`MultiMesh` and which feeds the ground `Sprite2D`.

### New Rust exports (`Simulation`)

All read-only; all mirror existing export patterns (`biome_colors`, `alive_*`,
`get_agent_info`). Names and shapes:

| Function | Signature → returns | Purpose |
|---|---|---|
| `pheromone_channel_count` | `() -> i64` (=4) | overlay cycling bound |
| `pheromone_colors` | `(channel: i64) -> PackedColorArray` | ground pheromone overlay (BIOME_RES² row-major heat ramp) |
| `env_active` | `() -> bool` | true iff `world.env_period > 0` |
| `env_optimum` | `() -> f32` | `env_optimum_at(tick, env_period)` in `[0,1]`; `-1.0` when inactive |
| `carcass_data` | `() -> Array<Dictionary>` | `{pos: Vector2, flesh: f32, age: i64, species_id: i64}` per carcass |
| `combat_flashes` | `() -> PackedVector2Array` | positions of agents with `combat_damaged` set this tick |
| `species_stats` | `() -> Array<Dictionary>` | `{species_id, count, mean_energy, mean_technique_match}` per live species |
| `agent_detail` | `(id: i64) -> Dictionary` | superset of `get_agent_info` + `diet_carnivory: f32`, `skill: f32` (meme[5]), `technique: f32` (meme[6]), `indiv_learn: bool`, `social_learn: bool`, `module_names: PackedStringArray` |

Notes:
- `mean_technique_match` uses `culture::technique_match(meme[TECH_CHANNEL],
  env_optimum())`; it is `0.0`/ignored when `env_active()` is false (the DIT panel is
  hidden then anyway).
- `diet_carnivory` calls the existing `module::effective_diet_carnivory`.
- `indiv_learn` / `social_learn` read genome slots `IndividualLearning` (28) /
  `SocialLearning` (29) against the `> 0.5` convention.
- `combat_damaged` is a per-tick `Vec<bool>` already on `World`; the exporter reads it
  before the next step clears it. `main.gd` renders flashes as short-lived sprites.
- `pheromone_colors` reuses `PheromoneField.cells` (row-major `BIOME_RES²`, 4
  channels); a value→color heat ramp (dark→hot) with alpha ∝ concentration.

### Overlay manager (`game/scripts/overlay_manager.gd`)

A plain `Node` holding two enums and dispatching what the renderers draw. It does not
own the meshes; it tells `main.gd`/`biome_renderer.gd` which mode is active.

- **GroundMode**: `BIOME` (default; current biome render) → `PHEROMONE_0..3` →
  `ENV_OPTIMUM` (a flat tint whose hue encodes the current global optimum; only
  reachable when `env_active`). Cycled with **`G`** (forward). Channels that a
  scenario never emits still render (all-zero → transparent), which is fine.
- **BodyMode**: `SPECIES` (default; current per-agent color) → `DIALECT`
  (meme-vector → hue via a fixed projection of `meme[0..MEME_CHANNELS]`) → `DIET`
  (herbivore green ↔ carnivore red on `diet_carnivory`) → `ENERGY` (cold→hot ramp).
  Cycled with **`C`**.
- `biome_renderer.gd` is refactored to read `GroundMode`: on `BIOME` it keeps today's
  path; on `PHEROMONE_n` it uploads `pheromone_colors(n)`; on `ENV_OPTIMUM` it uploads
  a single-hue fill. Same `Image`/`ImageTexture.update` upload path in all cases.
- `main.gd::_refresh_bodies` chooses the color array by `BodyMode` (species colors
  from `alive_colors`, or a computed array from diet/energy/meme pulled per frame).

### Panels

- **Inspector** (`inspector_panel.gd`): switch to `agent_detail`; add lines for diet,
  learned skill, technique, learning flags, and a comma-joined module-name list. Show
  the technique/skill/DIT lines only when the values are meaningful (env active or
  nonzero), to avoid noise on foundational scenarios.
- **Population panel** (`population_panel.gd`, new, top-right): one row per live
  species — color swatch + `sp N: count` — from `species_stats()`. Live bars only, no
  time series (YAGNI). Refreshes on a throttle (every ~6 frames) to avoid per-frame
  layout churn.
- **DIT panel** (`dit_panel.gd`, new): visible only when `env_active()`. Shows the
  current env-optimum (numeric + a small marker bar in `[0,1]`) and, per species, the
  mean technique-match — the on-screen readout of the gene–culture experiment.
- **Legend / keybind overlay** (`legend_panel.gd`, new; toggle **`H`**): lists the
  active GroundMode + BodyMode and the hotkeys (`G`, `C`, `H`, camera keys, speeds).

### Scenario menu (`menu.gd`)

Replace the 2-entry list with all 14 scenarios, grouped by section header in the
`OptionButton` and each carrying a `desc` and a `defaults` block:

- **Foundations**: `minimal`, `divergent`
- **Milestones**: `predator-prey`, `territories`, `dialects`, `cooperation`,
  `gene-culture`, `gene-culture-skill`, `gene-culture-hunt`, `gene-culture-alarm`
- **DIT boundary**: `dit-env-slow`, `dit-env-fast`, `dit-env-static`, `dit-rogers`

`GameConfig` carries the chosen scenario's default GroundMode/BodyMode into `main.tscn`.
Defaults: env/DIT scenarios → `ENV_OPTIMUM` ground + `DIALECT` body + DIT panel shown;
`territories`/`dialects` → `PHEROMONE_0` ground; everything else → `BIOME` + `SPECIES`.
Manual `G`/`C` always override. Paths stay `res://../scenarios/<name>.toml`.

## Components & boundaries

| Unit | Responsibility | Depends on |
|---|---|---|
| `Simulation` (Rust) | read-only typed views of `World` | `anabios-core` (unchanged) |
| `overlay_manager.gd` | own GroundMode/BodyMode state + cycling input | — |
| `biome_renderer.gd` | upload ground texture for current GroundMode | `Simulation`, overlay_manager |
| `main.gd` | step sim; upload body mesh for current BodyMode; carcass/flash sprites | `Simulation`, overlay_manager |
| `population_panel.gd` / `dit_panel.gd` | render `species_stats` / env readouts | `Simulation` |
| `inspector_panel.gd` | render `agent_detail` for pinned agent | `Simulation` |
| `legend_panel.gd` | show active modes + keybinds | overlay_manager |
| `menu.gd` / `GameConfig` | scenario list + default modes → scene load | — |

Each panel reads only the `Simulation` methods it needs and can be understood/tested
in isolation. The overlay manager is the single source of truth for display mode.

## Build order (phased, each independently shippable)

- **P1 — Scenario breadth** (pure GDScript): full menu + descriptions + `GameConfig`
  default-mode plumbing (modes can be inert until P3). Immediately makes all 14
  scenarios reachable.
- **P2 — Rust exports**: add the `#[func]`s above; run determinism + workspace tests to
  confirm golden hashes unchanged; run stable-toolchain fmt/clippy/doc.
- **P3 — Ground overlays + body color modes**: `overlay_manager.gd`, refactor
  `biome_renderer.gd`, extend `main.gd` for body modes + carcass/flash sprites.
- **P4 — Panels**: richer inspector, population panel, DIT panel.
- **P5 — UX polish**: legend/keybind overlay, per-scenario default wiring, color-ramp
  and layout cleanup.

## Testing & verification

- **Determinism (gating):** after P2, `cargo test -p anabios-core --test determinism`
  must pass with **no** hash change; `cargo test --workspace` green. These prove the
  exports are truly read-only.
- **CI-accurate local gate:** `rustup run stable cargo fmt --all --check`, `clippy
  --workspace --all-targets -- -D warnings`, rustdoc `-D warnings` (escape `[0,1]` /
  `[N]` in doc comments to avoid broken intra-doc-link errors), workspace tests.
- **Godot smoke:** the project has no headless test runner for rendering; verification
  is manual — load each of the 14 scenarios, cycle `G`/`C`, confirm no script errors in
  the Godot output and that env scenarios show the DIT panel. Document this checklist in
  the plan.
- **No new mutation grammar / sim nodes** are introduced, so nothing can perturb
  `minimal.toml` behavior.

## Out of scope (YAGNI)

- No lineage/phylogeny tree; no time-series charts (population panel is live bars only).
- No recording, replay, or data export.
- No new simulation mechanics, modules, genome slots, or scenario formats.
- No changes to the C skill mechanism, the DIT boundary tests, or any A/B harness.
- The web frontend artifact is a **separate** spec (the second polish cycle).
