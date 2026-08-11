# Agents on the Minimap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Plot alive agents as small amber dots on the C4 minimap so the population's location and spread read at a glance.

**Architecture:** One block added to `minimap_panel._draw()` that maps `sim.alive_positions()` (world coords) into minimap space with the same `scale` the viewport rectangle uses, drawing a ~2px dot per agent. Pure GDScript — no Rust/scene/shader/determinism impact.

**Tech Stack:** Godot 4.7 GDScript (`game/scripts/minimap_panel.gd`).

## Global Constraints

- **Pure viewer.** No `anabios-core`/`anabios-godot`/shader/scene change, not in `state_hash`, no goldens. `sim.alive_positions() -> PackedVector2Array` is an existing bridge method (already called by the main view each frame).
- **Draw order:** dots under the viewport rectangle (rectangle outline stays readable on top). Reuse the existing `scale = ms / world` and `world` locals already computed in `_draw()`.
- Gate: `cargo build -p anabios-godot` compiles; windowed boot of `res://scenes/main.tscn` loads with no GDScript `_draw` error. Stage explicit paths only.

---

## Task 1: Agent dots in `minimap_panel._draw()`

**Files:**
- Modify: `game/scripts/minimap_panel.gd` (add `AGENT_DOT` const; agent-dot loop in `_draw()`)

**Interfaces:**
- Consumes: `sim.alive_positions() -> PackedVector2Array` (world coords); the `_draw()` locals `world: float` and `scale: Vector2` (`= ms / world`).

- [ ] **Step 1: Add the `AGENT_DOT` constant.** Next to the other color consts near the top of `game/scripts/minimap_panel.gd`:

```gdscript
const AGENT_DOT := Color(1.0, 0.75, 0.3, 0.85)
```

- [ ] **Step 2: Draw the agent dots.** In `_draw()`, the viewport-rectangle block computes `var scale := ms / world` then `var top_left := ...`. Insert the agent loop **between** `var scale := ms / world` and `var top_left := (center - view_world * 0.5) * scale` (so dots draw under the viewport rectangle that follows):

```gdscript
	# Agents: one small dot each at its world position, so the population's
	# location and spread read at a glance. Overlapping dots brighten into a
	# cluster (natural density cue). ≤ max_population draws, once per frame.
	var positions: PackedVector2Array = sim.alive_positions()
	for p in positions:
		var mp := Vector2(fposmod(p.x, world), fposmod(p.y, world)) * scale
		draw_rect(Rect2(mp - Vector2(1, 1), Vector2(2, 2)), AGENT_DOT)
```

- [ ] **Step 3: Compile + headless boot smoke.**

Run:
```bash
cargo build -p anabios-godot
godot --headless --path game/ res://scenes/main.tscn --quit-after 3 2>&1 | grep -iE 'SCRIPT ERROR|Parse Error|minimap' | head
```
Expected: cdylib builds; no `SCRIPT ERROR`/`Parse Error` for `minimap_panel.gd`. (`_draw` isn't exercised headless; the windowed screenshot in the verification step covers it.)

- [ ] **Step 4: Headless visual verification.** Windowed capture on the continental world at a wide zoom (harness rejects `--headless`):

```bash
SP=<scratchpad>
ANABIOS_SHOT="$SP/minimap_agents.png" ANABIOS_SCENARIO="res://../scenarios/continental.toml" \
  ANABIOS_SEED=7 ANABIOS_GROUND=0 ANABIOS_CAM_X=2048 ANABIOS_CAM_Y=2048 ANABIOS_ZOOM=0.3 \
  ANABIOS_SHOT_FRAMES=90 godot --path game/ res://scenes/main.tscn --resolution 1280x1280
```
Read the PNG. Confirm the population shows as an **amber dot cluster on the minimap** at the **same world location as the main-view agent cluster**, sitting inside the minimap's viewport rectangle. If dots are invisible (too small/faint) or overwhelming, tune the dot size (`Vector2(2,2)`) or `AGENT_DOT` alpha and re-capture.

- [ ] **Step 5: Commit.**

```bash
git add game/scripts/minimap_panel.gd
git commit -m "feat(viewer): plot agents as dots on the minimap"
```

---

## Self-Review Notes (for the executor)

- **Spec coverage:** agent dots mapped world→minimap with the shared `scale`, single amber color, drawn under the viewport rect (Step 2); visual verification vs. the main-view cluster (Step 4). All spec sections map to the one task.
- **Determinism:** GDScript only; no Rust/shader/scene/state_hash/goldens.
- **Verified facts:** `minimap_panel._draw()` (from C4) computes `world: float` and `scale := ms / world` before the viewport-rectangle draw, so the agent loop can reuse both; `sim.alive_positions()` is an existing `#[func]` bridge method returning world-space `PackedVector2Array` (the main view uses it each frame); the panel already `queue_redraw()`s every frame in `_process`.
- **Visual-verification dependency:** needs Godot + a windowed run — the controller runs it. If unavailable, the code lands (compile + headless-boot checked) and the visual confirmation is noted as deferred, not silently skipped.
