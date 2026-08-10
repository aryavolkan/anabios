# Minimap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an always-on HUD minimap that shows the whole world (reusing the biome texture), marks the current camera viewport, and jumps the camera on click.

**Architecture:** A new `Control` (`minimap_panel.gd`) under the `UI` CanvasLayer draws `biome_renderer`'s world `ImageTexture` scaled into a corner, overlays a viewport rectangle computed from the Camera2D's `position`/`zoom`, and maps clicks back to world coordinates to recenter the camera. Pure GDScript + scene — no Rust, no shader, no determinism impact.

**Tech Stack:** Godot 4.7 GDScript + `.tscn` scene. `anabios-godot` is untouched.

## Global Constraints

- **Pure viewer.** No `anabios-core`/`anabios-godot`/shader change, not in `state_hash`, no goldens, no `FORMAT_VERSION`.
- **No click fall-through.** The minimap is a `Control` with `mouse_filter = MOUSE_FILTER_STOP` handling clicks in `_gui_input`, so a click on it does NOT reach `main.gd:_unhandled_input` (the world agent-pick handler).
- **Node paths (confirmed):** `/root/Main/Simulation` (sim, exposes `world_size()`), `/root/Main/Camera2D` (Camera2D, `position`/`zoom`), `/root/Main/Biome` (biome_renderer Sprite2D), `/root/Main/UI` (HUD CanvasLayer). Panels reference nodes via `@onready var x = get_node("/root/Main/...")`.
- Gate: `cargo build -p anabios-godot` still compiles; a windowed boot of `res://scenes/main.tscn` loads with no GDScript parse/`_draw` errors. Stage explicit paths only.

---

## File Structure

- **Create** `game/scripts/minimap_panel.gd` — the minimap Control (draw texture + viewport rect + click-to-pan).
- **Modify** `game/scripts/biome_renderer.gd` — add `func world_texture() -> ImageTexture: return _tex`.
- **Modify** `game/scenes/main.tscn` — register the script as an `ext_resource` and add a `Minimap` `Control` node under `UI` (and bump `load_steps`).

---

## Task 1: Minimap Control + world-texture getter + scene node

**Files:**
- Create: `game/scripts/minimap_panel.gd`
- Modify: `game/scripts/biome_renderer.gd`
- Modify: `game/scenes/main.tscn`

**Interfaces:**
- Consumes: `sim.world_size() -> float`; `Camera2D.position: Vector2`, `Camera2D.zoom: Vector2`; `biome_renderer.world_texture() -> ImageTexture`; `get_viewport_rect().size`.
- Produces: a `Minimap` node in the HUD; `biome_renderer.world_texture()`.

- [ ] **Step 1: Add the world-texture getter to `biome_renderer.gd`.** After `_setup()` (or near the other funcs):

```gdscript
# The whole-world biome ImageTexture (res×res), shared with the minimap so it
# reflects biome/biomass updates without a second rebuild. May be null before
# the first _setup(); callers must guard.
func world_texture() -> ImageTexture:
	return _tex
```

- [ ] **Step 2: Create `game/scripts/minimap_panel.gd`.**

```gdscript
extends Control

# Whole-world overview in the HUD corner: draws biome_renderer's world texture
# scaled into this Control, overlays the current camera viewport as a rectangle,
# and recenters the camera on click/drag. Pure viewer — no sim state touched.

@onready var sim = get_node("/root/Main/Simulation")
@onready var cam: Camera2D = get_node("/root/Main/Camera2D")
@onready var biome = get_node("/root/Main/Biome")

const BORDER := Color(0.8, 0.85, 0.9, 0.5)
const VIEWRECT := Color(1.0, 1.0, 1.0, 0.9)


func _ready() -> void:
	# STOP so clicks on the minimap are consumed by _gui_input and never fall
	# through to the world agent-pick handler in main.gd:_unhandled_input.
	mouse_filter = Control.MOUSE_FILTER_STOP


func _process(_dt: float) -> void:
	queue_redraw()  # viewport rect tracks the camera each frame; the panel is tiny


func _draw() -> void:
	var world: float = float(sim.world_size())
	if world <= 0.0:
		return
	var ms: Vector2 = size
	var tex = biome.world_texture()
	if tex != null:
		draw_texture_rect(tex, Rect2(Vector2.ZERO, ms), false)
	# Viewport rectangle: the world extent currently visible, centred on the
	# (torus-wrapped) camera position, mapped world→minimap. Clips at edges when
	# the view straddles a wrap seam (acceptable v1).
	var vp: Vector2 = get_viewport_rect().size
	var view_world := Vector2(vp.x / cam.zoom.x, vp.y / cam.zoom.y)
	var center := Vector2(fposmod(cam.position.x, world), fposmod(cam.position.y, world))
	var scale := ms / world
	var top_left := (center - view_world * 0.5) * scale
	draw_rect(Rect2(top_left, view_world * scale), VIEWRECT, false, 2.0)
	# Panel border.
	draw_rect(Rect2(Vector2.ZERO, ms), BORDER, false, 1.0)


func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_LEFT and event.pressed:
			_jump_to(event.position)
	elif event is InputEventMouseMotion:
		if event.button_mask & MOUSE_BUTTON_MASK_LEFT:
			_jump_to(event.position)


# Map a local minimap point to a world position and recenter the camera there.
func _jump_to(local: Vector2) -> void:
	var world: float = float(sim.world_size())
	if world <= 0.0 or size.x <= 0.0 or size.y <= 0.0:
		return
	cam.position = Vector2(local.x / size.x, local.y / size.y) * world
```

- [ ] **Step 3: Register the script + add the node in `game/scenes/main.tscn`.**

Bump the header `load_steps` by 1 (currently `[gd_scene load_steps=20 format=3]` → `load_steps=21`).

Add an `ext_resource` line alongside the other script resources (id must be unique — `15_minimap`):

```
[ext_resource type="Script" path="res://scripts/minimap_panel.gd" id="15_minimap"]
```

Add the node under the `UI` CanvasLayer (place it near the other `parent="UI"` nodes; top-left, below the HUD label at `offset_top 70`, ~200×200):

```
[node name="Minimap" type="Control" parent="UI"]
offset_left = 10.0
offset_top = 70.0
offset_right = 210.0
offset_bottom = 270.0
script = ExtResource("15_minimap")
```

- [ ] **Step 4: Compile check + headless boot smoke test.**

Run:
```bash
cargo build -p anabios-godot
godot --headless --path game/ res://scenes/main.tscn --quit-after 3 2>&1 | grep -iE 'SCRIPT ERROR|Parse Error|error' | grep -iv 'ERROR: .*audio' | head
```
Expected: cdylib builds; the boot prints **no** `SCRIPT ERROR` / `Parse Error` for `minimap_panel.gd` (a headless boot runs `_ready`/`_process` but not `_draw`; `_draw` is exercised in Task 2). Some unrelated audio/driver `ERROR:` lines under headless are benign — the grep filters the common audio one; ignore other driver-init noise.

- [ ] **Step 5: Commit.**

```bash
git add game/scripts/minimap_panel.gd game/scripts/biome_renderer.gd game/scenes/main.tscn
git commit -m "feat(viewer): world minimap with viewport rect + click-to-pan"
```

---

## Task 2: Headless visual verification + placement/size tune

Confirm the minimap renders and the viewport rectangle tracks the camera; tune size/position/colors. Needs Godot 4.7 windowed; the controller runs it (as for C1–C3).

**Files:**
- Modify (only if tuning): `game/scenes/main.tscn` (Minimap offsets) and/or `game/scripts/minimap_panel.gd` (colors/border).

**Interfaces:**
- Consumes: `scenarios/continental.toml` (seed 7); `game/scripts/debug_capture.gd`; `ANABIOS_CAM_X/Y`, `ANABIOS_ZOOM`.

- [ ] **Step 1: Build the cdylib.**

Run: `cargo build -p anabios-godot`

- [ ] **Step 2: Capture two shots at different camera framings.** Windowed (harness rejects `--headless`):

```bash
SP=<scratchpad>
# Wide view — large viewport rect on the minimap.
ANABIOS_SHOT="$SP/minimap_wide.png" ANABIOS_SCENARIO="res://../scenarios/continental.toml" \
  ANABIOS_SEED=7 ANABIOS_GROUND=0 ANABIOS_CAM_X=2048 ANABIOS_CAM_Y=2048 ANABIOS_ZOOM=0.3 \
  ANABIOS_SHOT_FRAMES=90 godot --path game/ res://scenes/main.tscn --resolution 1280x1280
# Tight view, different corner — small rect, moved on the minimap.
ANABIOS_SHOT="$SP/minimap_tight.png" ANABIOS_SCENARIO="res://../scenarios/continental.toml" \
  ANABIOS_SEED=7 ANABIOS_GROUND=0 ANABIOS_CAM_X=1200 ANABIOS_CAM_Y=1000 ANABIOS_ZOOM=1.5 \
  ANABIOS_SHOT_FRAMES=90 godot --path game/ res://scenes/main.tscn --resolution 1280x1280
```

- [ ] **Step 3: Inspect both PNGs.** Confirm:
  - The minimap shows the whole-world biome overview in the top-left corner, not overlapping the HUD tick label or other panels.
  - `minimap_wide` has a large viewport rectangle centred near world (2048,2048) → minimap centre; `minimap_tight` has a small rectangle offset toward (1200,1000) → upper-left of the minimap. The rectangle position and size track the camera (position moves, size scales inversely with zoom) — this verifies the world↔minimap map that click-to-pan reuses.

- [ ] **Step 4: Tune if needed.** Adjust the Minimap `offset_*` (size/position) if it crowds a panel, or the border/rect colors for legibility. Re-run Steps 1–3.

- [ ] **Step 5: Commit any tweak (skip if Task 1 placement was right).**

```bash
git add game/scenes/main.tscn game/scripts/minimap_panel.gd
git commit -m "style(viewer): tune minimap placement/legibility"
```

---

## Self-Review Notes (for the executor)

- **Spec coverage:** world-overview via reused texture (`world_texture()` getter + `draw_texture_rect`, T1 S1–2); viewport rectangle from camera position/zoom (T1 S2 `_draw`); click-to-pan with no fall-through (T1 S2 `_gui_input` + `MOUSE_FILTER_STOP`); HUD placement (T1 S3); visual verification of overview + rect tracking (T2). All spec sections map to a task.
- **Determinism:** GDScript + scene only; no Rust/shader/state_hash/goldens.
- **Verified facts:** node paths `/root/Main/{Simulation,Camera2D,Biome,UI}` (from `main.tscn` + `main.gd:825` using `$Camera2D`); panels use `@onready get_node("/root/Main/...")`; agent-pick is in `main.gd:_unhandled_input`, so a `MOUSE_FILTER_STOP` Control handling `_gui_input` consumes the click first; `main.tscn` header is `load_steps=20`, ext_resource ids `1_main`..`14_tech`/`13_coevo` are taken (use `15_minimap`); `biome_renderer` holds `var _tex: ImageTexture` built in `_setup()`.
- **Torus caveat:** the viewport rect uses the wrapped camera centre and clips at minimap edges when the view straddles a seam — acceptable per spec (no 9-tile wrap this pass).
- **T2 environment dependency:** needs Godot + a windowed run + visual judgment — best run by the controller. If unavailable, T1 lands the code (compile + headless-boot checked) and T2's visual confirmation is noted as deferred, not silently skipped.
- **Camera clamp caveat:** `cam.position` is written directly (matching how the showcase director scripts the camera). If `camera_controller` clamps position in `_process`, a click near a world edge may settle slightly inside the clamp — verify in T2 and note if the clamp interferes.
