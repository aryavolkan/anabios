# Godot Frontend Visual Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the anabios Godot sandbox's "field-instrument" visual identity to every surface — theme the entry menu, make the living world decodable (color-key legend, softer organic organisms, better framing), and raise flash/carcass visibility — building on the theme shipped in PR #25.

**Architecture:** All changes are view-only frontend code (GDScript, `.tscn`, `.tres`). The Rust `Simulation` node and its read-only `#[func]` accessors are NOT touched, so determinism golden hashes stay byte-identical (no golden refresh). New UI reuses the shared `UiTheme` (`res://scripts/ui_theme.gd`) and its color constants so the HUD keeps reading as one instrument. Verification is a headless boot through the env-gated `DebugCapture` autoload that renders the viewport to a PNG.

**Tech Stack:** Godot 4.7 (Forward+), GDScript, gdext `0.5` (`api-4-7`) Rust binding (already in place). No new dependencies.

## Global Constraints

- **Engine:** Godot **4.7+** only. The gdext binding uses `godot = { version = "0.5", features = ["api-4-7"] }` (PR #25). Do not change the Rust crate.
- **Determinism:** Do NOT modify `crates/anabios-core/**` or any `#[func]` in `crates/anabios-godot/src/lib.rs`. Only read from existing accessors. This keeps the golden determinism hashes in [[anabios-ci-gates]] byte-identical — no golden refresh.
- **One accent, applied consistently:** every new UI element derives its colors from `UiTheme` constants — `ACCENT = Color(0.30, 0.88, 0.70)`, `TEXT = Color(0.86, 0.92, 0.93)`, `TEXT_DIM = Color(0.56, 0.67, 0.69)`, `BG_PANEL`, `BG_ELEV`. Never hardcode a second accent hue.
- **Preload, don't `class_name`:** shared scripts are referenced via `const X = preload("res://scripts/x.gd")` (matches the existing `UiTheme` usage) so the global-class cache is never a runtime dependency.
- **No `.uid` churn in commits:** Godot regenerates `game/scripts/*.uid` and `game/**/*.import` on import. Stage only the `.gd`/`.tscn`/`.tres` files listed per task; leave `.uid`/`.import` untracked.

### Setup (run once before any task's verification)

Godot only loads the GDExtension when `.godot/extension_list.cfg` exists, which the editor/import step generates. In a fresh worktree:

```bash
cd game
godot --headless --path . --import   # may SIGSEGV in the editor-layout step AFTER import; the cache is still written — harmless
ls .godot/extension_list.cfg          # must exist: "res://anabios.gdextension"
```

### Verification recipe (V) — the test cycle for every task

There is no GDScript unit-test runner in this repo; the established gate (see [[anabios-godot-frontend]]) is a headless boot that surfaces every parse/instantiate/runtime error, plus a rendered PNG to inspect. Each task's "run the test" steps invoke this recipe:

```bash
cd game
OUT=/tmp/anabios_shot.png; rm -f "$OUT"
ANABIOS_SHOT="$OUT" ANABIOS_SHOT_FRAMES=90 ANABIOS_SHOT_TICKS=600 \
  <EXTRA_ENV> \
  godot --path . --windowed --resolution 1280x800 --position 40,60 res://scenes/main.tscn \
  > /tmp/anabios_run.log 2>&1
# GATE 1 (automated): zero script/instantiate errors
grep -icE 'ERROR|Nonexistent function|SCRIPT ERROR|not declared' /tmp/anabios_run.log   # expect: 0
# GATE 2 (visual): open "$OUT" and confirm the task's expected change
```

`DebugCapture` env knobs (already implemented in `game/scripts/debug_capture.gd`): `ANABIOS_SHOT` (output path + on switch), `ANABIOS_SHOT_FRAMES`, `ANABIOS_SHOT_TICKS` (force-unpause + jump N ticks), `ANABIOS_SCENARIO`/`ANABIOS_GROUND`/`ANABIOS_BODY` (override `GameConfig`), `ANABIOS_COEVO` (reveal `[Y]` chart). `<EXTRA_ENV>` in the recipe is where a task sets these.

The **menu** scene (Task 1) is not driven by `DebugCapture` (which only wires into `Main`). Boot it directly instead — that command is spelled out in Task 1.

---

## File Structure

- `game/scripts/ui_theme.gd` — **modify.** Shared `Theme` builder. Task 1 adds `OptionButton` / `LineEdit` / `PopupMenu` styling so form controls match.
- `game/scripts/menu.gd` — **modify.** Task 1 applies the shared theme + accent title to the entry screen.
- `game/scripts/palette.gd` — **create.** Single source of truth for the 9 module colors + names, shared by `main.gd` (glyph layers) and the legend key. Task 4.
- `game/scripts/main.gd` — **modify.** Task 2 assigns a soft-disc texture to the body/carcass/flash MultiMeshes; Task 4 reroutes `MODULE_COLORS` through `Palette`; Task 5 enlarges flash/carcass marks.
- `game/scripts/camera_controller.gd` — **modify.** Task 3 fits the world to the viewport on load and adds `[F]` reset-view.
- `game/scripts/legend_panel.gd` — **modify.** Task 4 rebuilds the legend to show a color key (module swatches + active body-mode key).

Each task is independently testable via recipe **V** and can be reviewed/rejected on its own.

---

### Task 1: Theme the entry menu

The in-game HUD is themed but `menu.tscn` (the actual `run/main_scene`) still uses stock Godot controls — an inconsistent first impression. Extend `UiTheme` to cover the menu's form controls, then apply it.

**Files:**
- Modify: `game/scripts/ui_theme.gd` (append control types inside `build()`, before `return theme`)
- Modify: `game/scripts/menu.gd:26-30` (`_ready`)

**Interfaces:**
- Consumes: `UiTheme.build() -> Theme`, `UiTheme.ACCENT`, `UiTheme._button_box(bg: Color, border: Color) -> StyleBoxFlat` (all already exist).
- Produces: nothing new for later tasks (self-contained).

- [ ] **Step 1: Extend `UiTheme.build()` with form-control styling**

In `game/scripts/ui_theme.gd`, insert the following immediately before the final `return theme` line inside `build()` (the `focus` StyleBoxFlat local is already defined above that point and is reused here):

```gdscript
	# --- Form controls (menu screen) ---
	theme.set_stylebox("normal", "OptionButton", _button_box(BG_ELEV, ACCENT_DIM))
	theme.set_stylebox("hover", "OptionButton", _button_box(BG_HOVER, ACCENT))
	theme.set_stylebox("pressed", "OptionButton", _button_box(BG_PRESSED, ACCENT))
	theme.set_stylebox("focus", "OptionButton", focus)
	theme.set_color("font_color", "OptionButton", TEXT)
	theme.set_color("font_hover_color", "OptionButton", ACCENT)
	theme.set_font_size("font_size", "OptionButton", 13)

	var field := _button_box(BG_ELEV, ACCENT_DIM)
	theme.set_stylebox("normal", "LineEdit", field)
	theme.set_stylebox("focus", "LineEdit", focus)
	theme.set_color("font_color", "LineEdit", TEXT)
	theme.set_color("caret_color", "LineEdit", ACCENT)
	theme.set_font_size("font_size", "LineEdit", 13)

	var popup := StyleBoxFlat.new()
	popup.bg_color = Color(0.05, 0.07, 0.085, 0.98)
	popup.set_corner_radius_all(4)
	popup.set_border_width_all(1)
	popup.border_color = ACCENT_DIM
	popup.set_content_margin_all(4)
	theme.set_stylebox("panel", "PopupMenu", popup)
	theme.set_color("font_color", "PopupMenu", TEXT)
	theme.set_color("font_hover_color", "PopupMenu", ACCENT)
	theme.set_color("font_accelerator_color", "PopupMenu", TEXT_DIM)
	theme.set_font_size("font_size", "PopupMenu", 13)
```

- [ ] **Step 2: Apply the theme in `menu.gd`**

In `game/scripts/menu.gd`, add the preload const at the top of the file (just under `extends Control`):

```gdscript
extends Control

const UiTheme = preload("res://scripts/ui_theme.gd")
```

Then at the START of `_ready()` (before the existing `for s in SCENARIOS:` loop), add:

```gdscript
func _ready() -> void:
	theme = UiTheme.build()
	$Background.color = Color(0.04, 0.055, 0.07, 1.0)
	$VBox/Title.add_theme_color_override("font_color", UiTheme.ACCENT)
	$VBox/Subtitle.add_theme_color_override("font_color", UiTheme.TEXT_DIM)
	# (existing body follows unchanged)
	for s in SCENARIOS:
		scenario_pick.add_item(s["label"])
	# ... rest unchanged ...
```

Leave the rest of `_ready()` exactly as-is.

- [ ] **Step 3: Boot the menu and check for errors**

The menu is not wired to `DebugCapture`, so boot it directly with a quit-after so the process exits:

```bash
cd game
godot --path . --windowed --resolution 1280x800 --position 40,60 res://scenes/menu.tscn --quit-after 120 > /tmp/menu_run.log 2>&1
grep -icE 'ERROR|Nonexistent function|SCRIPT ERROR|not declared' /tmp/menu_run.log   # expect: 0
```

Expected: `0`. (`--quit-after 120` renders ~120 frames then exits.)

- [ ] **Step 4: Screenshot the menu and inspect**

Reuse `DebugCapture` by pointing it at the menu scene (the autoload's `_ready` fires on any scene, and its capture path does not depend on `Main`):

```bash
cd game
OUT=/tmp/menu_shot.png; rm -f "$OUT"
ANABIOS_SHOT="$OUT" ANABIOS_SHOT_FRAMES=60 \
  godot --path . --windowed --resolution 1280x800 --position 40,60 res://scenes/menu.tscn \
  > /tmp/menu_run.log 2>&1
grep -c '\[capture\]' /tmp/menu_run.log   # expect: 1
```

Open `/tmp/menu_shot.png`. Confirm: dark instrument background, accent "anabios" title, dimmed subtitle, and the scenario dropdown + Start button rendered with the bordered instrument style (not stock gray).

- [ ] **Step 5: Commit**

```bash
git add game/scripts/ui_theme.gd game/scripts/menu.gd
git commit -m "feat(godot): theme the entry menu with the instrument look"
```

---

### Task 2: Soft-disc organism marks

Organisms render as hard 1×1 quads. A soft radial-gradient disc texture on the body MultiMesh (multiplied by each instance's color) turns them into rounded, organic marks that read better at small sizes — no geometry change, so instance colors still work.

**Files:**
- Modify: `game/scripts/main.gd` — add a `_disc_texture()` helper + assign it in `_ready()` to `$Bodies`, `$Carcasses`, `$Flashes`.

**Interfaces:**
- Consumes: node refs already declared as `@onready` (`bodies`, `carcasses`, `flashes` — the last as `flashes: MultiMeshInstance2D`).
- Produces: nothing for later tasks.

- [ ] **Step 1: Add the disc-texture helper**

Add this function to `game/scripts/main.gd` (place it next to the other private helpers, e.g. just above `_refresh_carcasses`):

```gdscript
# A soft white disc (alpha falls off to the edge). Multiplied by each MultiMesh
# instance color, it turns the flat body quads into rounded, organic marks.
func _disc_texture(res: int = 32) -> ImageTexture:
	var img := Image.create(res, res, false, Image.FORMAT_RGBA8)
	var c := (res - 1) * 0.5
	for y in res:
		for x in res:
			var d := Vector2(x - c, y - c).length() / c          # 0 center .. 1 edge
			var a := clampf(1.0 - smoothstep(0.75, 1.0, d), 0.0, 1.0)
			img.set_pixel(x, y, Color(1.0, 1.0, 1.0, a))
	return ImageTexture.create_from_image(img)
```

- [ ] **Step 2: Assign the texture in `_ready()`**

In `game/scripts/main.gd`, inside `_ready()` right after the `_apply_ui_theme()` call, add:

```gdscript
	var disc := _disc_texture()
	bodies.texture = disc
	carcasses.texture = disc
	flashes.texture = disc
```

- [ ] **Step 3: Boot with the sim running and check for errors**

Run recipe **V** with default env (minimal scenario):

```bash
cd game
OUT=/tmp/anabios_shot.png; rm -f "$OUT"
ANABIOS_SHOT="$OUT" ANABIOS_SHOT_FRAMES=90 ANABIOS_SHOT_TICKS=600 \
  godot --path . --windowed --resolution 1280x800 --position 40,60 res://scenes/main.tscn \
  > /tmp/anabios_run.log 2>&1
grep -icE 'ERROR|Nonexistent function|SCRIPT ERROR|not declared' /tmp/anabios_run.log   # expect: 0
```

Expected: `0`.

- [ ] **Step 4: Inspect the screenshot**

Open `/tmp/anabios_shot.png`. Confirm organisms now appear as soft round dots (not hard squares), still colored per species, with no dark square backing.

- [ ] **Step 5: Commit**

```bash
git add game/scripts/main.gd
git commit -m "feat(godot): render organisms as soft discs"
```

---

### Task 3: Fit the world to the viewport + reset-view key

Default `zoom = 1` leaves gray gutters around the 1024-unit world. Fit the world to fill the viewport on load, and add `[F]` to reset the view after panning/zooming.

**Files:**
- Modify: `game/scripts/camera_controller.gd` — add `_ready()` + `_fit_to_world()`, and an `[F]` branch in `_input`.

**Interfaces:**
- Consumes: `sim.world_size() -> float` (read-only `#[func]`, returns the `WORLD_SIZE` constant — safe to call any time).
- Produces: `_fit_to_world()` (private; not used by other tasks).

- [ ] **Step 1: Add fit-to-world on load**

Add to `game/scripts/camera_controller.gd` (the script has no `_ready` yet — add one; keep the existing `_input`/`_process`):

```gdscript
func _ready() -> void:
	_fit_to_world()

# Frame the whole world: fill the viewport (larger ratio wins, so there are no
# empty gutters) and center on the world's midpoint.
func _fit_to_world() -> void:
	var sim = get_node_or_null("/root/Main/Simulation")
	if sim == null:
		return
	var world: float = float(sim.world_size())
	if world <= 0.0:
		return
	var vp: Vector2 = get_viewport_rect().size
	var z: float = maxf(vp.x / world, vp.y / world)
	z = clampf(z, ZOOM_MIN, ZOOM_MAX)
	zoom = Vector2(z, z)
	position = Vector2(world * 0.5, world * 0.5)
```

- [ ] **Step 2: Add the `[F]` reset-view key**

In `game/scripts/camera_controller.gd`, extend `_input()` by adding this branch at the end of the function (after the existing `InputEventMouseMotion` branch, at the same indent level as the `if event is InputEventMouseButton:` block):

```gdscript
	elif event is InputEventKey and event.pressed and not (event as InputEventKey).echo:
		if (event as InputEventKey).keycode == KEY_F:
			_fit_to_world()
```

- [ ] **Step 3: Boot and check for errors**

Run recipe **V** (default env). Command identical to Task 2, Step 3.

Expected: `0`.

- [ ] **Step 4: Inspect the screenshot**

Open `/tmp/anabios_shot.png`. Confirm the biome fills the viewport edge-to-edge (no gray left/right gutters that were present before), still centered.

- [ ] **Step 5: Commit**

```bash
git add game/scripts/camera_controller.gd
git commit -m "feat(godot): fit world to viewport + [F] reset view"
```

---

### Task 4: Color-key legend

The legend lists keybinds but never explains what the colors mean. Add a color key: the 9 module-glyph colors (always drawn) plus a key for the active body-color mode. First extract the module palette to a shared script so the legend and `main.gd` agree.

**Files:**
- Create: `game/scripts/palette.gd`
- Modify: `game/scripts/main.gd:6-16` (replace the inline `MODULE_COLORS` const)
- Modify: `game/scripts/legend_panel.gd` (full rewrite of the script's UI to build a key)

**Interfaces:**
- Consumes: `overlay.ground_mode`, `overlay.body_mode` (ints), `UiTheme.ACCENT`, `UiTheme.TEXT_DIM`.
- Produces: `Palette.MODULE_COLORS: PackedColorArray` (9 entries), `Palette.MODULE_NAMES: PackedStringArray` (9 entries) — consumed by `main.gd` and `legend_panel.gd`.

- [ ] **Step 1: Create the shared palette**

Create `game/scripts/palette.gd`:

```gdscript
extends RefCounted

# Module glyph palette — indexed by module type id (0..8). Single source of
# truth shared by the body-glyph layers (main.gd) and the legend color key.
const MODULE_COLORS: PackedColorArray = [
	Color(0.6, 0.8, 1.0),   # 0 Locomotor
	Color(0.4, 0.9, 0.5),   # 1 Sensor
	Color(1.0, 0.7, 0.3),   # 2 Mouth
	Color(1.0, 0.3, 0.3),   # 3 Weapon
	Color(0.7, 0.7, 0.7),   # 4 Armor
	Color(0.9, 0.9, 0.4),   # 5 Storage
	Color(0.8, 0.5, 1.0),   # 6 Communicator
	Color(0.5, 1.0, 0.9),   # 7 Pheromone
	Color(1.0, 0.5, 0.8),   # 8 Reproductive
]
const MODULE_NAMES: PackedStringArray = [
	"Locomotor", "Sensor", "Mouth", "Weapon", "Armor",
	"Storage", "Communicator", "Pheromone", "Reproductive",
]
```

- [ ] **Step 2: Route `main.gd`'s module colors through `Palette`**

In `game/scripts/main.gd`, replace the inline `MODULE_COLORS` block (the `const MODULE_COLORS: PackedColorArray = [ ... 9 entries ... ]`) with a reference to the shared palette. Add the preload alongside the existing `UiTheme` preload, and swap the const:

```gdscript
const UiTheme = preload("res://scripts/ui_theme.gd")
const Palette = preload("res://scripts/palette.gd")

const MODULE_COLORS: PackedColorArray = Palette.MODULE_COLORS
```

Delete the old 9-line color literal. All existing `MODULE_COLORS[t]` reads keep working unchanged.

- [ ] **Step 3: Verify the palette refactor booted unchanged**

Run recipe **V** (default env). Command identical to Task 2, Step 3.

Expected: `0`. Open the screenshot; module glyphs render in the same colors as before (this step is a pure refactor — the key is added next).

- [ ] **Step 4: Rewrite `legend_panel.gd` to build the key**

Replace the ENTIRE contents of `game/scripts/legend_panel.gd` with:

```gdscript
extends PanelContainer

const UiTheme = preload("res://scripts/ui_theme.gd")
const Palette = preload("res://scripts/palette.gd")

const GROUND_NAMES := ["biome", "phero-0", "phero-1", "phero-2", "phero-3", "env-optimum"]
const BODY_NAMES := ["species", "dialect", "diet", "energy"]

@onready var overlay = get_node("/root/Main/OverlayManager")

var _controls: Label
var _key_box: VBoxContainer
var _expanded: bool = true
var _last_body: int = -1

func _ready() -> void:
	# Replace the scene's placeholder Label with our own layout.
	for c in get_children():
		c.queue_free()
	var vb := VBoxContainer.new()
	vb.add_theme_constant_override("separation", 6)
	add_child(vb)
	_controls = Label.new()
	_controls.add_theme_font_size_override("font_size", 13)
	vb.add_child(_controls)
	_key_box = VBoxContainer.new()
	_key_box.add_theme_constant_override("separation", 3)
	vb.add_child(_key_box)

func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_H:
		_expanded = not _expanded
		_key_box.visible = _expanded

func _process(_delta: float) -> void:
	if not _expanded:
		_controls.text = "[H] show controls"
		return
	var g: int = clampi(overlay.ground_mode, 0, GROUND_NAMES.size() - 1)
	var b: int = clampi(overlay.body_mode, 0, BODY_NAMES.size() - 1)
	_controls.text = (
		"[G] ground: %s\n[C] body: %s\n[Y] co-evolution chart · [F] reset view\n[H] hide · WASD/drag pan · wheel zoom · click inspect"
	) % [GROUND_NAMES[g], BODY_NAMES[b]]
	if b != _last_body:
		_last_body = b
		_rebuild_key(b)

func _rebuild_key(body_mode: int) -> void:
	for c in _key_box.get_children():
		c.queue_free()
	_key_box.add_child(_header("modules"))
	_key_box.add_child(_swatch_wrap(Palette.MODULE_COLORS, Palette.MODULE_NAMES))
	match body_mode:
		1:
			_key_box.add_child(_header("body: hue = dialect"))
		2:
			_key_box.add_child(_header("body: diet"))
			_key_box.add_child(_ramp_row(Color(0.3, 0.9, 0.4), Color(1.0, 0.3, 0.3), "herbivore", "carnivore"))
		3:
			_key_box.add_child(_header("body: energy"))
			_key_box.add_child(_ramp_row(Color(0.2, 0.3, 0.8), Color(1.0, 0.9, 0.3), "low", "high"))
		_:
			_key_box.add_child(_header("body: hue = lineage"))

func _header(text: String) -> Label:
	var l := Label.new()
	l.text = text
	l.add_theme_font_size_override("font_size", 11)
	l.add_theme_color_override("font_color", UiTheme.TEXT_DIM)
	return l

func _chip(col: Color) -> ColorRect:
	var r := ColorRect.new()
	r.color = col
	r.custom_minimum_size = Vector2(12, 12)
	return r

func _swatch_wrap(colors: PackedColorArray, names: PackedStringArray) -> HFlowContainer:
	var flow := HFlowContainer.new()
	flow.add_theme_constant_override("h_separation", 4)
	flow.add_theme_constant_override("v_separation", 2)
	for i in colors.size():
		var chip := _chip(colors[i])
		if i < names.size():
			chip.tooltip_text = names[i]
		flow.add_child(chip)
	return flow

func _ramp_row(a: Color, b: Color, left: String, right: String) -> HBoxContainer:
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 4)
	var la := Label.new()
	la.text = left
	la.add_theme_font_size_override("font_size", 11)
	row.add_child(la)
	# Five swatches interpolating a -> b as a compact ramp.
	for i in 5:
		row.add_child(_chip(a.lerp(b, float(i) / 4.0)))
	var lb := Label.new()
	lb.text = right
	lb.add_theme_font_size_override("font_size", 11)
	row.add_child(lb)
	return row
```

- [ ] **Step 5: Boot across body modes and check errors**

Run recipe **V** three times, forcing different body-color modes so every `_rebuild_key` branch executes (species, diet, energy):

```bash
cd game
for BODY in 0 2 3; do
  OUT=/tmp/legend_$BODY.png; rm -f "$OUT"
  ANABIOS_SHOT="$OUT" ANABIOS_SHOT_FRAMES=90 ANABIOS_SHOT_TICKS=400 ANABIOS_BODY="$BODY" \
    godot --path . --windowed --resolution 1280x800 --position 40,60 res://scenes/main.tscn \
    > /tmp/legend_$BODY.log 2>&1
  echo "BODY=$BODY errors=$(grep -icE 'ERROR|Nonexistent function|SCRIPT ERROR|not declared' /tmp/legend_$BODY.log)"
done
```

Expected: `errors=0` for all three.

- [ ] **Step 6: Inspect the screenshots**

Open `/tmp/legend_0.png`, `/tmp/legend_2.png`, `/tmp/legend_3.png`. Confirm the legend panel now shows: the keybind lines, a "modules" row of 9 color chips, and a body-mode key that changes — "body: hue = lineage" (species), a green→red "diet" ramp (herbivore→carnivore), and a blue→yellow "energy" ramp. The panel must stay inside its slot without overrunning the time-controls bar below it.

- [ ] **Step 7: Commit**

```bash
git add game/scripts/palette.gd game/scripts/main.gd game/scripts/legend_panel.gd
git commit -m "feat(godot): color-key legend (module + body-mode swatches)"
```

---

### Task 5: Raise combat-flash and carcass visibility

Now that live organisms are bigger (PR #25) and softer (Task 2), the combat flashes (scale 1.6) and carcasses read as too small/faint by comparison. Scale them up to match and make flashes pop.

**Files:**
- Modify: `game/scripts/main.gd` — `_refresh_carcasses` (scale floor/ceiling) and `_refresh_flashes` (scale + color).

**Interfaces:**
- Consumes: existing `sim.carcass_data()`, `sim.combat_flashes()`. No new accessors.
- Produces: nothing for later tasks.

- [ ] **Step 1: Enlarge carcasses**

In `game/scripts/main.gd`, in `_refresh_carcasses`, replace the flesh-scale line:

```gdscript
		var f: float = clampf(float(d["flesh"]) / 20.0, 0.2, 1.5)
```

with a larger, floored range so even small carcasses are visible:

```gdscript
		var f: float = clampf(float(d["flesh"]) / 20.0 * 4.0, 3.0, 7.0)
```

- [ ] **Step 2: Enlarge and brighten flashes**

In `game/scripts/main.gd`, in `_refresh_flashes`, replace the transform + color lines:

```gdscript
		mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(1.6, 1.6), 0.0, pts[i]))
		mm.set_instance_color(i, Color(1.0, 0.85, 0.2, 0.9))
```

with a larger, hotter flash:

```gdscript
		mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(6.0, 6.0), 0.0, pts[i]))
		mm.set_instance_color(i, Color(1.0, 0.92, 0.45, 0.95))
```

- [ ] **Step 3: Boot the predator-prey scenario (most combat) and check errors**

Combat flashes are frequent in predator-prey. Run recipe **V** with that scenario:

```bash
cd game
OUT=/tmp/flash_shot.png; rm -f "$OUT"
ANABIOS_SHOT="$OUT" ANABIOS_SHOT_FRAMES=90 ANABIOS_SHOT_TICKS=500 \
  ANABIOS_SCENARIO="res://../scenarios/predator-prey.toml" ANABIOS_GROUND=0 ANABIOS_BODY=2 \
  godot --path . --windowed --resolution 1280x800 --position 40,60 res://scenes/main.tscn \
  > /tmp/flash_run.log 2>&1
grep -icE 'ERROR|Nonexistent function|SCRIPT ERROR|not declared' /tmp/flash_run.log   # expect: 0
```

Expected: `0`.

- [ ] **Step 4: Inspect the screenshot**

Open `/tmp/flash_shot.png`. Confirm combat flashes appear as clearly visible bright-gold marks around the hunt cluster, and any carcasses read as soft pale discs — both noticeably larger than the pre-change specks, without swamping the live organisms.

- [ ] **Step 5: Commit**

```bash
git add game/scripts/main.gd
git commit -m "feat(godot): raise combat-flash and carcass visibility"
```

---

## Self-Review

**Spec coverage** — the spec is "extend the field-instrument identity to every surface + make the world decodable": menu (Task 1), organism legibility (Task 2 discs, Task 5 flashes/carcasses), framing (Task 3), decoding colors (Task 4). All covered.

**Placeholder scan** — every code step contains complete GDScript; every test step is a runnable command with an expected result (`0` errors, `1` capture line, or a named visual check). No TBD/TODO. The "test" cycle is the repo's real headless-boot gate (no unit-test runner exists — see [[anabios-godot-frontend]]), not a fabricated pytest.

**Type consistency** — `Palette.MODULE_COLORS` / `Palette.MODULE_NAMES` are defined in Task 4 Step 1 and consumed in Task 4 Steps 2 (`main.gd`) and 4 (`legend_panel.gd`) with matching names and `PackedColorArray`/`PackedStringArray` types. `UiTheme.ACCENT` / `TEXT_DIM` / `_button_box(bg, border)` match the existing `ui_theme.gd` signatures. `_fit_to_world()` is defined and called within Task 3 only. `sim.world_size()` returns `float` (constant) — the one accessor read, confirmed read-only.

**Ordering note** — Tasks are independent except Task 4 Step 4 (legend) references `[F]` from Task 3 in its help text; if Task 3 is skipped the text is still valid (just advertises a key that no-ops), so the tasks may still be reviewed independently. Recommended order is 1→5 as written.
