# Minimap in the viewer (sub-project C, spec 4)

**Date:** 2026-08-10
**Status:** Approved design → implementation
**Branch:** `claude/2d-world-scale-159e3b`

## Context: the larger arc

Sub-project **C** ("viewer at scale") of "actual world with good scale in 2D".
**A** (continental worldgen) shipped in PR #126; C specs **1** (rivers), **2** (hillshade),
and **3** (ocean bathymetry) also shipped there. This is **C spec 4 — a minimap** for
navigating the continent-scale world. Remaining C work (agent LOD, wider zoom-out) stays
in later specs.

## Motivation

At the flagship scale (`world_size 4096`) the viewport shows only a fraction of the world,
and a small population is a speck on one continent. There is no way to see where you are
in the whole world or to jump somewhere else quickly — panning across 4096 units by
WASD/drag is slow, and the fit-to-world zoom barely frames the map. A minimap gives a
persistent whole-world overview, marks the current camera view, and lets you click to jump.

## Current viewer structure (what this builds on)

- The HUD is a `CanvasLayer` named `UI` in `game/scenes/main.tscn`; panels (Inspector,
  Population, Legend, TimeControls, Codex, …) are `Control` children of it.
- `game/scripts/biome_renderer.gd` (a `Sprite2D`) holds the whole-world biome image as
  `var _tex: ImageTexture` (`res×res`, scaled to `world_size`, redrawn on a
  resolution-scaled cadence). This texture already exists and updates itself.
- The Camera2D (`camera_controller.gd`) exposes `position` (world center) and `zoom`
  (`Vector2`), and `sim.world_size()` gives the torus extent.

## Approach (locked): a Control that reuses the world texture

**Pure GDScript + scene — no Rust / `anabios-core` / shader / determinism impact.**

1. **New `game/scripts/minimap_panel.gd`** (`extends Control`), added as a child of the
   `UI` CanvasLayer in `main.tscn`, anchored **top-left, below the HUD label** (~200×200;
   exact size/position visual-tuned). **Always-on.**
2. **Image = the existing world texture.** `biome_renderer.gd` gains
   `func world_texture() -> ImageTexture: return _tex`. The minimap draws that texture
   scaled into its rect in `_draw()` (`draw_texture_rect`, GPU downscale — no rebuild).
   It **mirrors the active ground mode** (shows the pheromone/optimum/etc. overlay when
   one is on); "always biome" is a deliberate later tweak, not v1.
3. **Viewport rectangle.** Each frame, compute the camera's visible world rect —
   `view_world = viewport_pixels / camera.zoom`, centred on `camera.position` — map it
   through world→minimap coords (`minimap_px = world_pos / world_size * minimap_size`),
   and draw it as an outlined rect in `_draw()`. Redraw is triggered by `queue_redraw()`
   in `_process` (cheap; the minimap is tiny).
4. **Click-to-pan.** On a left click or drag inside the minimap rect, map the local point
   back to a world position (`world = local_px / minimap_size * world_size`) and set
   `camera.position` to it (respecting the camera's existing clamp/ease if any; a direct
   `position` write matches how the showcase director already scripts the camera). The
   click is consumed so it does not fall through to the world (agent-pick) handler.

### Torus handling
The minimap shows one world tile `[0, world_size]²`. The camera `position` is wrapped
into that tile; the viewport rectangle is drawn centred on the wrapped position and
clips at the minimap edges if the view straddles a wrap seam (acceptable for v1 — no
9-tile rect wrapping this pass).

## Determinism / blast radius

None. Files: `game/scripts/minimap_panel.gd` (new), `game/scripts/biome_renderer.gd`
(one getter), `game/scenes/main.tscn` (one `Control` node under `UI`). No Rust, no
shader, no `state_hash`, no goldens, no `FORMAT_VERSION`.

## Testing

Inherently visual — GDScript UI has no Rust unit surface. Verify via headless screenshots
(the `debug_capture` harness) on `scenarios/continental.toml` (seed 7):

1. **Overview + viewport rect:** the minimap renders the whole-world biome overview in the
   corner, with a viewport rectangle marking the current camera view. At a wide zoom the
   rect is large; at a tight zoom it is a small box — confirm the rect scales inversely
   with zoom and sits at the camera's world position.
2. **Click-to-pan (mapping check):** set the camera to two different positions
   (`ANABIOS_CAM_X/Y`) and confirm the viewport rectangle moves to the corresponding spot
   on the minimap — verifying the world↔minimap coordinate map. (The click handler shares
   that same map; a correct rect ⇒ correct click mapping.)

The controller runs and inspects these, as for C1–C3. Godot boot must be windowed
(`res://scenes/main.tscn`, the harness rejects `--headless`).

Sanity gate: `cargo build -p anabios-godot` still compiles (the getter is GDScript, but
confirm nothing else regressed), and a headless boot of `main.tscn` loads without script
errors (check the log for parse/`_draw` errors).

## Out of scope (later C specs)

Population-centroid dot on the minimap, always-biome rendering during ground overlays,
wider zoom-out (`ZOOM_MIN`), agent/event markers, 9-tile torus-wrapped viewport rect, a
visibility toggle.

## Milestone / delivery

A single small change on the branch (extends PR #126, or a fresh PR if #126 has merged):
`world_texture()` getter, `minimap_panel.gd` (draw texture + viewport rect + click-to-pan),
the `main.tscn` node, positioned/sized against a headless screenshot.
