# Agents on the minimap (sub-project C, spec 5)

**Date:** 2026-08-10
**Status:** Approved design → implementation
**Branch:** `claude/2d-world-scale-159e3b`

## Context: the larger arc

Sub-project **C** ("viewer at scale"). C specs 1–4 (rivers, hillshade, ocean
bathymetry, minimap) shipped in PR #126. This is **C spec 5 — agents on the minimap**,
the "agent LOD / don't lose the population" deliverable.

## Motivation

Classic sprite level-of-detail already exists: `main.gd:462-469` applies a
zoom-compensated body-size floor so agents stay legible in the main view when zoomed
out, and bodies render via cheap MultiMesh (no perf-LOD need). The real remaining gap on
a 4096-unit world is **discoverability** — you cannot tell at a glance *where* the small
population is, and the C4 minimap draws only the biome, not the agents. Plotting agents
on the minimap closes that gap and composes with C4's click-to-jump: glance → see the
cluster → click there.

## Approach (locked): agent dots in `minimap_panel._draw()`

**Extend `game/scripts/minimap_panel.gd` only — no Rust/core/scene/shader change.**

In `_draw()`, after drawing the world texture and before the viewport rectangle (so the
rectangle outline stays readable over the dots), fetch `sim.alive_positions()` (a
`PackedVector2Array` of world coordinates the bridge already exposes and the main view
already uses each frame) and draw each agent as a small ~2px dot, mapped world→minimap
with the same `scale = minimap_size / world_size` the viewport rectangle uses
(`fposmod`-wrapped for torus safety):

```gdscript
var positions: PackedVector2Array = sim.alive_positions()
for p in positions:
    var mp := Vector2(fposmod(p.x, world), fposmod(p.y, world)) * scale
    draw_rect(Rect2(mp - Vector2(1, 1), Vector2(2, 2)), AGENT_DOT)
```

- **Single warm dot color** with modest alpha — `AGENT_DOT = Color(1.0, 0.75, 0.3, 0.85)`
  (amber) — so overlapping dots read as a brighter cluster, a natural density cue without
  a separate heatmap. Reads against the biome overview's greens/blues/greys.
- `draw_rect` per agent (cheaper than `draw_circle`); at the flagship cap (`max_population
  800`) that is ≤800 tiny draws once per frame, alongside the rest of the minimap redraw
  (the panel already `queue_redraw()`s each frame). Trivial for Godot 2D.

## Determinism / blast radius

None — pure GDScript, one function edited (`minimap_panel.gd`). No Rust, shader, scene,
`state_hash`, or golden impact. `sim.alive_positions()` is an existing bridge method.

## Testing

Visual — GDScript UI has no Rust unit surface. Headless screenshot on
`scenarios/continental.toml` (seed 7) via the `debug_capture` harness at a wide zoom:
the population appears as an amber dot cluster on the minimap at the **same world location
as the main-view agent cluster**, inside the minimap's viewport rectangle when framed
there. The controller runs and inspects it, as for C1–C4.

Sanity: `cargo build -p anabios-godot` still compiles; a windowed boot of
`res://scenes/main.tscn` loads with no GDScript `_draw` error.

## Out of scope (later)

Per-species dot coloring (via `alive_colors()`), density-heatmap binning, event/codex
markers on the minimap, a "jump to population" hotkey.

## Milestone / delivery

A single small change on the branch (extends PR #126, or a fresh PR if #126 has merged):
add the agent-dot loop + `AGENT_DOT` const to `minimap_panel._draw()`, verified against a
headless screenshot.
