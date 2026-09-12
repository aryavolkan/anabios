# Anabios 2D Visual-System Pass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a performant pixel-art biome-prop layer plus more legible pixel event and landmark animation while preserving Anabios as a simulation-driven viewer.

**Architecture:** Existing field atlases already supply the complete active state contract: idle, four-frame locomotion, and paired action poses through the shared 16×16 / 128×128 texture grid. New static terrain decoration will be a separate `Node2D` child of `Biome`, using one nearest-filtered plain `MultiMeshInstance2D` per prop kind with wrap clones. Existing pooled `GPUParticles2D` and settlement MultiMesh layers will receive pixel-art textures and brightness animation; neither will add per-agent nodes or change simulation state.

**Tech Stack:** Godot 4.7.2, GDScript, MultiMeshInstance2D, GPUParticles2D, Image/ImageTexture, Fennara screenshots and scene validation, gdformat, gdlint.

**Spec:** `docs/superpowers/specs/2026-09-11-visual-system-design.md`

## Global Constraints

- Keep all production sprite cells at 16×16 and any atlas at 128×128 in an 8×8 square grid.
- Use nearest-neighbour filtering for pixel-art props and landmarks.
- Do not replace MultiMesh rendering with per-agent nodes or add simulation authority to Godot.
- Keep `main.gd` unchanged; `Biome` owns biome props, `ViewerEffects` owns effect pools, and `SettlementLayer` owns landmark presentation.
- Preserve event cooldown/budget behavior and keep camera shake disabled unless an existing cooldown-gated effect already uses it.
- Run scene validation and inspect actual rendered captures for each visual subsystem.

---

## File Structure

- Create: `game/scripts/biome_props.gd` — deterministic 16×16 prop art, terrain-colour classification, batched prop layers, and torus wrap clones.
- Create: `game/scripts/test_biome_props.gd` — headless checks for sprite visibility, colour classification, deterministic sampling, and output bounds.
- Modify: `game/scripts/biome_renderer.gd` — own and refresh the prop layer only when the terrain view is visible.
- Create: `game/scripts/pixel_fx_sprites.gd` — 16×16 ember, impact, and discovery burst art for effect pools.
- Create: `game/scripts/test_pixel_fx_sprites.gd` — headless checks for sprite dimensions, opacity, and texture generation.
- Modify: `game/scripts/event_fx.gd` — add the `burst` effect kind to discovery/adoption and combat event specs.
- Modify: `game/scripts/viewer_effects.gd` — add a bounded pixel-burst particle pool and consume `burst` specs.
- Modify: `game/scripts/test_event_fx.gd` — extend the allowed effect kinds and assert burst dispatch for combat and discovery.
- Modify: `game/scripts/building_sprites.gd` — provide flame-on and flame-low 16×16 texture variants for Fire and Metalworking landmarks.
- Modify: `game/scripts/settlement_layer.gd` — alternate only animated landmark textures at a fixed low cadence, including their wrap clones.
- Modify: `game/scripts/test_building_sprites.gd` — assert animated variants retain visible landmark silhouettes and differ in flame pixels.

### Task 1: Deterministic biome-prop layer

**Files:**
- Create: `game/scripts/biome_props.gd`
- Create: `game/scripts/test_biome_props.gd`
- Modify: `game/scripts/biome_renderer.gd:1-80, 125-175`

**Interfaces:**
- Consumes: `Simulation.biome_colors() -> PackedColorArray`, `Simulation.biome_resolution() -> int`, and `Simulation.world_size() -> float`.
- Produces: `BiomeProps.setup(sim: Node) -> void`, `BiomeProps.refresh(colors: PackedColorArray, resolution: int, world_size: float) -> void`, and `BiomeProps.set_terrain_visible(is_visible: bool) -> void`.

- [ ] **Step 1: Write the failing prop-registry test.**

```gdscript
const Props = preload("res://scripts/biome_props.gd")

func _check_registry() -> void:
	_check(Props.KIND_COUNT == 5, "five terrain prop silhouettes")
	for kind in Props.KIND_COUNT:
		var image: Image = Props.build_image(kind)
		_check(image.get_width() == 16 and image.get_height() == 16, "prop is 16x16")
		_check(Props.opaque_pixels(image) >= 8, "prop has a readable silhouette")
	_check(Props.kind_for_color(Color(0.08, 0.24, 0.55)) == Props.NONE, "water has no prop")
	_check(Props.kind_for_color(Color(0.18, 0.56, 0.20)) == Props.SHRUB, "green terrain grows shrub")
	_check(Props.kind_for_color(Color(0.56, 0.48, 0.34)) == Props.ROCK, "dry terrain gets rock")
```

- [ ] **Step 2: Run the new test and verify it fails because the registry is absent.**

Run:
```bash
godot --headless --rendering-driver dummy --path game -s res://scripts/test_biome_props.gd
```

Expected: a preload error for `res://scripts/biome_props.gd`, with no later success line.

- [ ] **Step 3: Implement the prop registry and bounded sampler.**

```gdscript
extends Node2D

const ApeSprites = preload("res://scripts/ape_sprites.gd")
enum { NONE = -1, REEDS, SHRUB, CONIFER, ROCK, LOG }
const KIND_COUNT := 5
const SAMPLE_STRIDE := 12
const MAX_PER_KIND := 140

static func kind_for_color(c: Color) -> int:
	if c.b > c.r * 1.25 and c.b > c.g * 1.10:
		return NONE
	if c.g > c.r * 1.18 and c.g > c.b * 1.10:
		return SHRUB
	if c.g > 0.34 and c.r < 0.40:
		return CONIFER
	if c.r > c.g * 1.12:
		return ROCK
	return LOG

static func sample_seed(x: int, y: int) -> int:
	return int((x * 73856093) ^ (y * 19349663))
```

Author five explicit block lists using the existing `ApeSprites._build_cell()` painter: water reeds, a rounded shrub, tall conifer, faceted rock, and fallen log. Flip each output with `Image.flip_y()` before wrapping it in `ImageTexture`, matching the existing plain MultiMesh convention. In `refresh`, scan every `SAMPLE_STRIDE` grid cell, keep only seeds whose `posmod(sample_seed(x, y), 11) == 0`, classify the colour, and append at most `MAX_PER_KIND` transforms per kind. Convert grid coordinates with `Vector2((x + 0.5) * world_size / resolution, (y + 0.5) * world_size / resolution)`.

Create one `MultiMeshInstance2D` per kind with `TEXTURE_FILTER_NEAREST`, `z_index = 3` (above Biome terrain, below farms), and eight offset clones that share the original MultiMesh. Make `set_terrain_visible(false)` hide the complete prop layer while a data overlay owns the terrain.

- [ ] **Step 4: Wire the layer into `BiomeRenderer` without editing `main.gd`.**

```gdscript
const BiomeProps = preload("res://scripts/biome_props.gd")
var _props: Node2D

func _ready() -> void:
	# existing setup
	_props = BiomeProps.new()
	_props.name = "BiomeProps"
	add_child(_props)
	_props.setup(sim)

func _process(_delta: float) -> void:
	# after choosing mode and colours
	_props.set_terrain_visible(mode == -1)
	if mode == -1:
		_props.refresh(colors, _res, sim.world_size())
```

Make `BiomeProps.refresh` return early when its `(resolution, world_size, terrain checksum)` signature has not changed. The checksum must sample the first, middle, and final terrain colours so steady terrain does not rewrite MultiMeshes every redraw.

- [ ] **Step 5: Run the prop test and formatter.**

Run:
```bash
godot --headless --rendering-driver dummy --path game -s res://scripts/test_biome_props.gd
gdformat --check game/scripts/biome_props.gd game/scripts/biome_renderer.gd game/scripts/test_biome_props.gd
gdlint game/scripts/biome_props.gd game/scripts/biome_renderer.gd game/scripts/test_biome_props.gd
```

Expected: `test_biome_props: all passed`; no `SCRIPT ERROR`, `FAIL:`, formatter, or lint output.

- [ ] **Step 6: Capture the main scene and validate it.**

Use Fennara `validate_scene` on `scenes/main.tscn`, then use `screenshot_scene` to inspect a terrain view. Verify props are crisp, sparse, under agents, present across a torus edge, and hidden after changing the ground overlay.

- [ ] **Step 7: Commit the self-contained biome layer.**

```bash
git add game/scripts/biome_props.gd game/scripts/test_biome_props.gd game/scripts/biome_renderer.gd
git commit -m "viewer: add batched biome pixel props"
```

### Task 2: Pixel-art event bursts

**Files:**
- Create: `game/scripts/pixel_fx_sprites.gd`
- Create: `game/scripts/test_pixel_fx_sprites.gd`
- Modify: `game/scripts/event_fx.gd`
- Modify: `game/scripts/viewer_effects.gd:20-50, 70-180, 260-330`
- Modify: `game/scripts/test_event_fx.gd:28-31, 280-330`

**Interfaces:**
- Produces: `PixelFxSprites.build(kind: int) -> ImageTexture`, with `EMBER = 0`, `IMPACT = 1`, and `DISCOVERY = 2`.
- Consumes: Event spec entries `{ "kind": "burst", "sprite": int, "color": Color }`.
- Produces: `ViewerEffects.spawn_pixel_burst(pos: Vector2, sprite: int, color: Color) -> void`.

- [ ] **Step 1: Write a failing texture test and event mapping assertions.**

```gdscript
const PixelFx = preload("res://scripts/pixel_fx_sprites.gd")

func _check_pixel_fx() -> void:
	for kind in PixelFx.KIND_COUNT:
		var texture: ImageTexture = PixelFx.build(kind)
		var image: Image = texture.get_image()
		_check(image.get_width() == 16 and image.get_height() == 16, "fx is 16x16")
		_check(PixelFx.opaque_pixels(image) >= 6, "fx silhouette is visible")
	_check(EventFx.spec(7).any(func(s): return s["kind"] == "burst"), "combat has impact burst")
	_check(EventFx.spec(17).any(func(s): return s["kind"] == "burst"), "discovery has burst")
```

- [ ] **Step 2: Run both tests and verify the new assertions fail.**

Run:
```bash
godot --headless --rendering-driver dummy --path game -s res://scripts/test_pixel_fx_sprites.gd
godot --headless --rendering-driver dummy --path game -s res://scripts/test_event_fx.gd
```

Expected: missing `pixel_fx_sprites.gd` preload and event assertions failing for absent `burst` specs.

- [ ] **Step 3: Implement three 16×16 pixel effect textures.**

```gdscript
extends RefCounted

const ApeSprites = preload("res://scripts/ape_sprites.gd")
enum { EMBER, IMPACT, DISCOVERY }
const KIND_COUNT := 3

static func build(kind: int) -> ImageTexture:
	var image: Image = ApeSprites._build_cell(_BLOCKS[kind])
	image.flip_y()
	return ImageTexture.create_from_image(image)
```

Use compact block silhouettes: a three-pixel ember diamond, a four-direction impact star, and a seven-point discovery sparkle. Use `ApeSprites.PAL` keys `o`, `R`, `y`, `W`, and `s`; all cells must stay transparent outside their outlined mark.

- [ ] **Step 4: Add a bounded burst pool and dispatch it from event specs.**

```gdscript
const PixelFxSprites = preload("res://scripts/pixel_fx_sprites.gd")
const PIXEL_BURST_POOL := 10
var _pixel_bursts: Array[GPUParticles2D] = []
var _pixel_burst_idx := 0

func spawn_pixel_burst(pos: Vector2, sprite: int, color: Color) -> void:
	var burst: GPUParticles2D = _pixel_bursts[_pixel_burst_idx]
	_pixel_burst_idx = (_pixel_burst_idx + 1) % _pixel_bursts.size()
	burst.texture = PixelFxSprites.build(sprite)
	burst.modulate = color
	burst.position = pos
	burst.restart()
```

Initialize exactly ten one-shot particles in `setup()`: amount 8, lifetime 0.45 seconds, z index 6, additive canvas material, 360-degree spread, and a 64×64 visibility rect. In `apply_event_fx`, add a `"burst"` branch after `"motes"`; skip it when the event position is `Vector2.ZERO`, exactly like other positional effects. Add `burst` to the test’s `KINDS` list. Add an amber Impact burst to CombatRaid (7) and War (38), and an invention-coloured Discovery burst to Discovery (17) and Adoption (18).

- [ ] **Step 5: Run tests, diagnostics, and formatting.**

Run:
```bash
godot --headless --rendering-driver dummy --path game -s res://scripts/test_pixel_fx_sprites.gd
godot --headless --rendering-driver dummy --path game -s res://scripts/test_event_fx.gd
gdformat --check game/scripts/pixel_fx_sprites.gd game/scripts/event_fx.gd game/scripts/viewer_effects.gd game/scripts/test_pixel_fx_sprites.gd game/scripts/test_event_fx.gd
gdlint game/scripts/pixel_fx_sprites.gd game/scripts/event_fx.gd game/scripts/viewer_effects.gd game/scripts/test_pixel_fx_sprites.gd game/scripts/test_event_fx.gd
```

Expected: both tests print their `all passed` success lines and there are no parser errors.

- [ ] **Step 6: Inspect visual output and commit.**

Use Fennara runtime capture or a short managed scene run to trigger one combat event and one discovery. Confirm bursts are visible at close zoom, expire, preserve the existing ring/mote effects, and do not accumulate beyond the ten-element pool. Then commit:

```bash
git add game/scripts/pixel_fx_sprites.gd game/scripts/test_pixel_fx_sprites.gd game/scripts/event_fx.gd game/scripts/viewer_effects.gd game/scripts/test_event_fx.gd
git commit -m "viewer: add pixel event bursts"
```

### Task 3: Landmark fire flicker

**Files:**
- Modify: `game/scripts/building_sprites.gd:455-464`
- Modify: `game/scripts/settlement_layer.gd:39-110, 120-145`
- Modify: `game/scripts/test_building_sprites.gd:25-70`

**Interfaces:**
- Produces: `BuildingSprites.is_animated(kind: int) -> bool` and `BuildingSprites.build_variant(kind: int, phase: int) -> ImageTexture`.
- Consumes: `phase` equal to 0 or 1; only `FIRE` and `METALWORKING` vary.
- Produces: `SettlementLayer._tick_landmark_animation() -> void`, swapping source and clone textures in lockstep.

- [ ] **Step 1: Add failing assertions for landmark variants.**

```gdscript
_check(B.is_animated(B.FIRE), "fire landmark animates")
_check(B.is_animated(B.METALWORKING), "forge landmark animates")
_check(not B.is_animated(B.MARKET), "market stays static")
var low: Image = B.build_variant(B.FIRE, 0).get_image()
var high: Image = B.build_variant(B.FIRE, 1).get_image()
_check(low.get_size() == Vector2i(16, 16), "flicker frame stays 16x16")
_check(low.get_data() != high.get_data(), "flicker frames differ")
```

- [ ] **Step 2: Run the building test and verify it fails for missing APIs.**

Run:
```bash
godot --headless --rendering-driver dummy --path game -s res://scripts/test_building_sprites.gd
```

Expected: an invalid-method failure naming `is_animated` or `build_variant`.

- [ ] **Step 3: Implement two deterministic variants without changing landmark mapping.**

```gdscript
const ANIMATED_KINDS: PackedInt32Array = [FIRE, METALWORKING]

static func is_animated(kind: int) -> bool:
	return ANIMATED_KINDS.has(kind)

static func build_variant(kind: int, phase: int) -> ImageTexture:
	var image: Image = build_image(kind)
	if phase % 2 == 1:
		_apply_flame_lift(image, kind)
	return ImageTexture.create_from_image(image)
```

`_apply_flame_lift` must only recolour and move the existing flame pixels: Fire trades its top amber pixel for a taller pale-yellow tip; Metalworking brightens the forge-mouth core and moves one orange pixel upward. Do not change any non-animated building’s pixels, enum value, name, invention mapping, or landmark placement.

- [ ] **Step 4: Alternate animated textures at 3 Hz and include wrap clones.**

```gdscript
const FLICKER_PERIOD := 1.0 / 3.0
var _flicker_elapsed := 0.0
var _flicker_phase := 0
var _building_clones: Dictionary = {}

func _process(delta: float) -> void:
	_flicker_elapsed += delta
	if _flicker_elapsed >= FLICKER_PERIOD:
		_flicker_elapsed = 0.0
		_flicker_phase = 1 - _flicker_phase
		_tick_landmark_animation()
	# existing redraw cadence
```

Build phase 0 textures during `_ready()`. Keep a per-kind array containing the source `MultiMeshInstance2D` and its eight clones when `_make_wrap_clones()` creates them. `_tick_landmark_animation()` must update `texture` on all instances for Fire and Metalworking only, so origin and torus copies never show mismatched frames.

- [ ] **Step 5: Run tests, diagnostics, and visual verification.**

Run:
```bash
godot --headless --rendering-driver dummy --path game -s res://scripts/test_building_sprites.gd
godot --headless --rendering-driver dummy --path game -s res://scripts/test_event_fx.gd
gdformat --check game/scripts/building_sprites.gd game/scripts/settlement_layer.gd game/scripts/test_building_sprites.gd
gdlint game/scripts/building_sprites.gd game/scripts/settlement_layer.gd game/scripts/test_building_sprites.gd
```

Then capture both flicker phases at identical camera framing. Confirm the Fire and Metalworking pixels change cleanly while every other landmark remains static, and no clone lags a frame behind.

- [ ] **Step 6: Commit the landmark animation.**

```bash
git add game/scripts/building_sprites.gd game/scripts/settlement_layer.gd game/scripts/test_building_sprites.gd
git commit -m "viewer: animate fire and forge landmarks"
```

### Task 4: End-to-end visual and regression pass

**Files:**
- Modify only if a verification result exposes a scoped rendering defect in Tasks 1–3.

**Interfaces:**
- Consumes: the three committed visual subsystems.
- Produces: final rendered evidence and a clean scoped worktree.

- [ ] **Step 1: Generate a pixel-art reference board for design QA.**

Use the built-in image generator once with this prompt:

```text
Use case: stylized-concept
Asset type: 2D evolution-simulation pixel-art reference board
Primary request: a compact set of five 16-bit pixel-art silhouettes: reed clump, scrub, conifer, rock, fallen log; plus ember, impact star, discovery sparkle, and a tiny hearth flame in two states
Style/medium: crisp 16×16-friendly pixel art, dark near-black outline, simple readable silhouettes, no gradients, transparent background
Color palette: earth brown, moss green, muted pine, slate grey, amber and pale-yellow fire, cyan only for water context
Constraints: each subject isolated with generous spacing; no text, no UI, no watermark, no anti-aliasing
```

Use the result only as a visual comparison; production block lists remain deterministic source code.

- [ ] **Step 2: Run all affected headless tests and inspect their logs.**

Run:
```bash
for test in test_mammal_sprites.gd test_building_sprites.gd test_emote_sprites.gd test_event_fx.gd test_biome_props.gd test_pixel_fx_sprites.gd; do
  godot --headless --rendering-driver dummy --path game -s "res://scripts/$test" 2>&1 | tee "/tmp/${test%.gd}.log"
  test ${pipestatus[1]} -eq 0
  ! rg -n "SCRIPT ERROR|FAIL:" "/tmp/${test%.gd}.log"
done
gdformat --check game/scripts
gdlint game/scripts
```

Expected: every test prints `all passed`; no `SCRIPT ERROR`, `FAIL:`, formatter, or linter finding occurs.

- [ ] **Step 3: Validate and capture the player-facing scene.**

Use Fennara `validate_scene` for `scenes/main.tscn`. Start a bounded runtime session for the main scene, capture terrain, populated field, settlement, and combat/discovery moments, then make an ordered sheet. Confirm current field creature animations remain crisp and unchanged, biome props remain sparse, bursts read distinctly, and Fire/Metalworking landmarks flicker without atlas tears.

- [ ] **Step 4: Commit any verification-only corrections and report results.**

```bash
git status --short
git add game/scripts/biome_props.gd game/scripts/test_biome_props.gd game/scripts/biome_renderer.gd game/scripts/pixel_fx_sprites.gd game/scripts/test_pixel_fx_sprites.gd game/scripts/event_fx.gd game/scripts/viewer_effects.gd game/scripts/test_event_fx.gd game/scripts/building_sprites.gd game/scripts/settlement_layer.gd game/scripts/test_building_sprites.gd
git commit -m "viewer: polish visual-system pass"
```

Run the `git add` and commit only when `git status --short` names one or more of those scoped files; otherwise do not create an empty commit. Report the specific commits, test output, scene validation result, and rendered evidence.

## Plan Self-Review

- Spec coverage: Tasks 1–3 implement each new system named in the spec; Task 4 verifies field animation compatibility, actual render quality, and the generated pixel-art direction board.
- Placeholders: none; every file, public interface, test command, asset count, effect budget, and verification condition is stated.
- Type consistency: `BiomeProps` uses `PackedColorArray`, `int`, `float`, `Vector2`, and `Node`; burst specs use `int` sprite IDs and `Color`; building variants use `int` kind/phase and return `ImageTexture` throughout.
