# Mammals with Good Animations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the Godot viewer a diet/size/livestock-driven quadruped mammal roster (7 archetypes, apes folded in) so ecological roles render as recognizable animated animals instead of everything being a hominin.

**Architecture:** Viewer-only. Generalize `main.gd`'s "bucket alive agents by ape species" into "bucket by **archetype**." A new `mammal_sprites.gd` registry owns a pure `archetype_for(diet, size, livestock)` selector, one procedural 16×16 atlas per archetype, and a `rig_kind` tag. `ape_sprites.gd` stays and becomes the Primate archetype's implementation. `field_agent.gdshader` gains a `rig_kind` uniform driving quadruped gallop/signature/secondary-motion branches; the Primate path is byte-identical. Per-species coat is a **per-instance modulate over a neutral value-ramp atlas** (no palette mask, no extra INSTANCE_CUSTOM channel).

**Tech Stack:** GDScript (Godot 4.7.1), GDShader (canvas_item), Rust (gdext / `godot` crate) for one batch accessor.

## Global Constraints

- **Viewer-only:** no changes to `crates/anabios-core`, the determinism gate, goldens, or snapshot `FORMAT_VERSION`. The one Rust change is a read-only accessor in `crates/anabios-godot`.
- **No shipped assets:** all sprites stay procedural (built from block-lists at load), matching `ape_sprites.gd`.
- **File size:** every file under `game/scripts/` must stay **under 1000 lines** (gdlint `max-file-lines`). Per-rig pose data lives in `game/scripts/mammal_data/<rig>.gd`.
- **CI gates (all blocking):**
  - `gdformat --check game/scripts/` and `gdlint game/scripts/` (gdtoolkit 4.*). Both recurse into `mammal_data/`.
  - Godot headless scene smoke: `godot --headless --rendering-driver dummy --path game res://scenes/main.tscn --quit-after 120` must emit `Initialize godot-rust` and **zero** `SCRIPT ERROR`.
  - Rust: `cargo build -p anabios-godot`, `cargo test -p anabios-godot`, `cargo fmt --check`, `cargo clippy -p anabios-godot -- -D warnings`.
- **gdlint footgun:** never run `gdlint --dump-default-config` in the repo — it writes a stray `gdlintrc` that shadows `.gdlintrc`.
- **Determinism of visuals:** archetype selection must be a pure function of per-agent state (diet and size are fixed at birth) so an agent never flickers archetype frame-to-frame.

## Design note — the tint model (resolves the spec's open "mask" detail)

The spec left the secondary-zone encoding to the plan. Resolution: **no mask.** Quadruped atlases are baked as a **neutral grayscale value-ramp** — dark outline (auto), mid-value coat (`"c"` ≈ 0.60), lighter underbelly/muzzle (`"u"` ≈ 0.92), near-black eye (`"e"` ≈ 0.10). `main.gd` already multiplies each instance by a per-instance color (`set_instance_color`, the `[C]`-overlay path). In **species mode** we repurpose that color: for quadruped archetypes it carries the **per-species coat hue**; the value-ramp then yields counter-shading in that hue (coat = hue×0.60, belly = hue×0.92) for free. The Primate archetype keeps its existing baked multi-zone colors and stays white in species mode, exactly as today. `[C]` overlays (diet/energy/dialect/affect) keep working for all archetypes — they replace the per-instance color and multiply over the atlas as now.

## File structure

- **Create** `game/scripts/mammal_sprites.gd` — registry: enum + NAMES, `rig_kind`, pure `archetype_for`, per-species coat hue, quadruped atlas/fallen builders, delegates Primate to `ApeSprites`. Target < 300 lines.
- **Create** `game/scripts/mammal_data/wolf.gd` (and later `deer.gd`, `hare.gd`, `boar.gd`, `fox.gd`, `livestock.gd`) — const-only pose block-lists per rig. Each < 250 lines.
- **Create** `game/scripts/test_mammal_sprites.gd` — headless `SceneTree` script asserting `archetype_for` boundaries; exits non-zero on failure.
- **Modify** `crates/anabios-godot/src/lib.rs` — add pure helper `livestock_flags_of(&World) -> Vec<i32>` + `#[func] fn alive_livestock_flags()` + `#[func] fn domestication_enabled()`; unit-test the helper.
- **Modify** `game/scripts/main.gd` — `_ready` builds 7 archetype MultiMeshes/materials; `_refresh_bodies` and death-ghost paths bucket by archetype; `_body_colors` species-mode returns coat hue for quadrupeds.
- **Modify** `game/scripts/ape_sprites.gd` — expose `POSE_COUNT`, pose layout, and builders under names the registry calls (mostly already public); no behavior change.
- **Modify** `game/scripts/inspector_panel.gd`, `game/scripts/settlement_layer.gd` — call the registry instead of `ApeSprites.ape_for_species`.

## Pose slot contract (shared by every rig, biped and quadruped)

12 poses stacked vertically in a 16×(12·16) strip, identical indices to today's apes so one shader serves all rigs:

| Slot | Meaning | Quadruped interpretation |
|------|---------|--------------------------|
| 0 | neutral / idle | stand square |
| 1 | contact-L | trot: near-hind + far-fore planted forward |
| 2 | passing | legs gathered under body, whole figure lifted 1px (the bob) |
| 3 | contact-R | trot: opposite diagonal planted |
| 4/5 | eat A/B | graze A/B (head lowered to ground, small dip) |
| 6/7 | fight A/B | attack A/B (bite/lunge or headbutt) |
| 8/9 | trade A/B | alert A/B (head up, ears forward, small shift) |
| 10/11 | flee A/B | gallop A/B (body stretched, legs fore/aft extended) |

---

### Task 1: Rust batch accessor — livestock flags

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs` (add helper + two `#[func]`s near `alive_diet`, ~L486; add test in the existing `#[cfg(test)] mod tests` ~L1495)

**Interfaces:**
- Consumes: `anabios_core::agent::AGENT_NULL`, `World.agents.livestock_of: Vec<AgentId>`, `World.agents.iter_alive()`, `World.domestication_enabled: bool`.
- Produces (GDScript-visible): `sim.alive_livestock_flags() -> PackedInt32Array` (1/0 per alive agent, same order as `alive_positions`; all-zero when domestication is off), `sim.domestication_enabled() -> bool`. Pure helper `livestock_flags_of(w: &anabios_core::World) -> Vec<i32>`.

- [ ] **Step 1: Write the failing test** (in `mod tests`)

```rust
#[test]
fn livestock_flags_match_alive_count_and_are_binary() {
    // Minimal scenario has domestication OFF -> every flag must be 0.
    let toml = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scenarios/minimal.toml"
    ))
    .expect("read minimal.toml");
    let mut w = anabios_core::Scenario::parse_toml(&toml).unwrap().instantiate();
    for _ in 0..25 {
        anabios_core::tick::step(&mut w);
    }
    let flags = super::livestock_flags_of(&w);
    assert_eq!(flags.len(), w.agents.iter_alive().count());
    assert!(flags.iter().all(|&f| f == 0 || f == 1));
    assert!(!w.domestication_enabled);
    assert!(flags.iter().all(|&f| f == 0), "domestication off => no livestock");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-godot livestock_flags_match -- --nocapture`
Expected: FAIL — `cannot find function livestock_flags_of`.

- [ ] **Step 3: Write the helper + accessors**

Add near `alive_diet` (after ~L486):

```rust
    /// Livestock flag (1 tamed / 0 wild) per alive agent, same order as
    /// `alive_positions`. All-zero unless the scenario has domestication on.
    #[func]
    fn alive_livestock_flags(&self) -> PackedInt32Array {
        let mut out = PackedInt32Array::new();
        if let Some(w) = self.inner.as_ref() {
            for f in livestock_flags_of(w) {
                out.push(f);
            }
        }
        out
    }

    /// Whether this world runs the E13 domestication subsystem.
    #[func]
    fn domestication_enabled(&self) -> bool {
        self.inner.as_ref().map(|w| w.domestication_enabled).unwrap_or(false)
    }
```

Add the pure helper as a free `fn` at module scope (near `dialect_hue`), so it is unit-testable without a `Gd` instance:

```rust
/// Per-alive-agent livestock flag (1 tamed / 0 wild). Zero for every agent
/// when domestication is disabled — `livestock_of` is `AGENT_NULL` then anyway,
/// but the flag short-circuits so a viewer needn't special-case the scenario.
fn livestock_flags_of(w: &anabios_core::World) -> Vec<i32> {
    use anabios_core::agent::AGENT_NULL;
    if !w.domestication_enabled {
        return w.agents.iter_alive().map(|_| 0).collect();
    }
    w.agents
        .iter_alive()
        .map(|id| i32::from(w.agents.livestock_of[id as usize] != AGENT_NULL))
        .collect()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p anabios-godot livestock_flags_match -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Format, lint, build**

Run: `cargo fmt -p anabios-godot && cargo clippy -p anabios-godot -- -D warnings && cargo build -p anabios-godot`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-godot/src/lib.rs
git commit -m "feat(viewer): expose alive_livestock_flags + domestication_enabled accessors"
```

---

### Task 2: Registry skeleton + pure `archetype_for` (all-Primate delegation)

Build the registry so it compiles and the viewer still renders identically (every archetype resolves to Primate for now; `main.gd` is not yet rewired). This isolates the pure selection logic behind a headless test.

**Files:**
- Create: `game/scripts/mammal_sprites.gd`
- Create: `game/scripts/test_mammal_sprites.gd`
- Modify: `game/scripts/ape_sprites.gd` (only if a needed symbol isn't already public — `POSE_COUNT`, `build_species_atlas`, `build_fallen_texture`, `ape_for_species`, `NAMES` are already `const`/`static`, so likely no change)

**Interfaces:**
- Consumes: `ApeSprites` (`SPECIES_COUNT`, `POSE_COUNT`, `build_species_atlas(sp)`, `build_fallen_texture(sp)`, `ape_for_species(id)`, `NAMES`).
- Produces:
  - `const ARCHETYPE_COUNT := 7`
  - `enum {HARE, DEER, BOAR, PRIMATE, FOX, WOLF, LIVESTOCK}`
  - `const NAMES: PackedStringArray`
  - `enum RigKind {PREY, PREDATOR, PRIMATE_RIG, LIVESTOCK_RIG}`
  - `static func rig_kind(archetype: int) -> int`
  - `static func archetype_for(diet: float, size: float, livestock: bool) -> int`
  - `static func primate_skin_for(species_id: int) -> int` (delegates to `ApeSprites.ape_for_species`)
  - `const POSE_COUNT` (mirror of `ApeSprites.POSE_COUNT`, = 12)

- [ ] **Step 1: Write the failing headless test** — `game/scripts/test_mammal_sprites.gd`

```gdscript
extends SceneTree
# Headless unit test for the pure archetype selector. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_mammal_sprites.gd
# Exits 0 on success, 1 on the first failed assertion.

const M = preload("res://scripts/mammal_sprites.gd")


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		quit(1)


func _init() -> void:
	# Livestock override beats everything.
	_check(M.archetype_for(0.9, 2.0, true) == M.LIVESTOCK, "livestock override")
	# Herbivore band (diet < 0.34): small -> Hare, large -> Deer.
	_check(M.archetype_for(0.1, 0.8, false) == M.HARE, "herb small = hare")
	_check(M.archetype_for(0.1, 2.0, false) == M.DEER, "herb large = deer")
	# Omnivore band (0.34..0.66): small -> Boar, large -> Primate.
	_check(M.archetype_for(0.5, 0.8, false) == M.BOAR, "omni small = boar")
	_check(M.archetype_for(0.5, 2.0, false) == M.PRIMATE, "omni large = primate")
	# Carnivore band (>= 0.66): small -> Fox, large -> Wolf.
	_check(M.archetype_for(0.9, 0.8, false) == M.FOX, "carn small = fox")
	_check(M.archetype_for(0.9, 2.0, false) == M.WOLF, "carn large = wolf")
	# Boundary: exactly SIZE_SPLIT counts as large; exactly a band edge is the
	# higher band's floor (diet == 0.34 is omnivore, diet == 0.66 is carnivore).
	_check(M.archetype_for(0.1, M.SIZE_SPLIT, false) == M.DEER, "size split = large")
	_check(M.archetype_for(M.HERB_MAX, 0.8, false) == M.BOAR, "diet 0.34 = omnivore")
	_check(M.archetype_for(M.CARN_MIN, 0.8, false) == M.FOX, "diet 0.66 = carnivore")
	print("test_mammal_sprites: all passed")
	quit(0)
```

- [ ] **Step 2: Run to verify it fails**

Run: `godot --headless --rendering-driver dummy --path game -s res://scripts/test_mammal_sprites.gd`
Expected: FAIL — cannot preload `mammal_sprites.gd` (does not exist yet).

- [ ] **Step 3: Write `game/scripts/mammal_sprites.gd`** (delegating everything to Primate for now)

```gdscript
extends RefCounted
# Archetype registry for the field figures. An agent's *archetype* (shape +
# animation rig) is chosen from its diet, body size, and livestock status; its
# *coat* is a per-species tint applied as a per-instance modulate over the
# archetype's neutral value-ramp atlas. The Primate archetype is the exception:
# it delegates to ape_sprites.gd, which bakes the five hominins' own colours.

const ApeSprites = preload("res://scripts/ape_sprites.gd")

enum { HARE, DEER, BOAR, PRIMATE, FOX, WOLF, LIVESTOCK }
const ARCHETYPE_COUNT := 7
const NAMES: PackedStringArray = ["Hare", "Deer", "Boar", "Primate", "Fox", "Wolf", "Livestock"]

# Signature-move family, passed to the shader as the `rig_kind` uniform.
enum RigKind { PREY, PREDATOR, PRIMATE_RIG, LIVESTOCK_RIG }
const _RIG_KIND: PackedInt32Array = [
	RigKind.PREY,  # HARE
	RigKind.PREY,  # DEER
	RigKind.PREY,  # BOAR (omnivore, but prey-family gait/flourish)
	RigKind.PRIMATE_RIG,  # PRIMATE
	RigKind.PREDATOR,  # FOX
	RigKind.PREDATOR,  # WOLF
	RigKind.LIVESTOCK_RIG,  # LIVESTOCK
]

# Pose strip is the same 12-slot layout as the apes so one shader serves all.
const POSE_COUNT := ApeSprites.POSE_COUNT

# Selection thresholds (tunable; validated in Task 9's capture pass).
const SIZE_SPLIT := 1.25
const HERB_MAX := 0.34
const CARN_MIN := 0.66


static func rig_kind(archetype: int) -> int:
	return _RIG_KIND[archetype]


# Pure archetype selector. `size` in world units (0.5..3.0), `diet` carnivory
# 0..1. Stable per agent (diet/size fixed at birth) so no per-frame flicker.
static func archetype_for(diet: float, size: float, livestock: bool) -> int:
	if livestock:
		return LIVESTOCK
	var large := size >= SIZE_SPLIT
	if diet < HERB_MAX:
		return DEER if large else HARE
	elif diet < CARN_MIN:
		return PRIMATE if large else BOAR
	return WOLF if large else FOX


static func primate_skin_for(species_id: int) -> int:
	return ApeSprites.ape_for_species(species_id)


# --- Rendering (Task 2: all archetypes fall back to Primate art). ---
# Replaced per-archetype in Tasks 4-7; kept delegating so the viewer renders
# unchanged until main.gd is rewired.
static func build_atlas(archetype: int, species_id: int) -> ImageTexture:
	return ApeSprites.build_species_atlas(ApeSprites.ape_for_species(species_id))


static func build_fallen(archetype: int, species_id: int) -> ImageTexture:
	return ApeSprites.build_fallen_texture(ApeSprites.ape_for_species(species_id))
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `godot --headless --rendering-driver dummy --path game -s res://scripts/test_mammal_sprites.gd`
Expected: `test_mammal_sprites: all passed`, exit 0.

- [ ] **Step 5: Format + lint**

Run: `gdformat game/scripts/mammal_sprites.gd game/scripts/test_mammal_sprites.gd && gdformat --check game/scripts/ && gdlint game/scripts/`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add game/scripts/mammal_sprites.gd game/scripts/test_mammal_sprites.gd
git commit -m "feat(viewer): archetype registry + pure archetype_for selector (Primate delegation)"
```

---

### Task 3: Shader — `rig_kind` uniform + quadruped motion branches (Primate byte-identical)

Extend `field_agent.gdshader` with a `rig_kind` uniform and gate the new gallop-stretch, per-family signature move, and idle tail-sway behind it. `rig_kind` defaults to `2` (PRIMATE_RIG); when unset (the current apes) every new branch is skipped, so the ape animation is unchanged.

**Files:**
- Modify: `game/shaders/field_agent.gdshader`

**Interfaces:**
- Consumes: existing `INSTANCE_CUSTOM = (phase, moving, face_left, action/4)`; new `uniform int rig_kind` (0 PREY, 1 PREDATOR, 2 PRIMATE, 3 LIVESTOCK).
- Produces: same visual contract; Primate output identical to pre-change.

- [ ] **Step 1: Add the uniform** (after the existing `uniform` block)

```glsl
// Signature-move family (mammal_sprites.RigKind): 0 prey, 1 predator,
// 2 primate (default = today's apes, all new branches off), 3 livestock.
uniform int rig_kind = 2;
```

- [ ] **Step 2: Add gallop stretch + signature move in `vertex()`** — replace the existing action `if (act > 1.5 ...)` block with:

```glsl
	// Fight/attack (act 2): primate lunges; predators add a forward pounce arc.
	if (act > 1.5 && act < 2.5) {
		float c = fract(TIME * action_fps * 0.5 + anim.r * 2.0);
		float lunge = pow(max(sin(c * 3.14159265), 0.0), 1.5);
		float dir = mix(1.0, -1.0, step(0.5, anim.b));
		VERTEX.x += dir * lunge * 0.10;
		if (rig_kind == 1) {          // PREDATOR pounce: lift through the leap
			VERTEX.y += lunge * 0.12;
			VERTEX.x += dir * lunge * 0.06;
		}
	} else if (act > 3.5) {           // Flee/gallop (act 4)
		if (rig_kind == 0 || rig_kind == 1) {
			// Quadruped gallop: stretch the body along travel + a bounding hop
			// (prey bound higher than predators chase).
			float g = TIME * walk_fps * 0.9 + anim.r * 6.2831853;
			VERTEX.x *= 1.0 + 0.10 * abs(sin(g));
			float hop = (rig_kind == 0) ? 0.10 : 0.05;
			VERTEX.y += max(sin(g), 0.0) * hop;
		} else {
			// Biped flee tremble (unchanged primate behaviour).
			VERTEX.x += sin(TIME * 43.0 + phase) * 0.03;
			VERTEX.y += cos(TIME * 37.0 + phase) * 0.02;
		}
	}
	// Livestock idle: a slow chewing head-bob (only while standing).
	if (rig_kind == 3 && anim.g < 0.5 && act < 0.5) {
		VERTEX.y += sin(TIME * 3.0 + phase) * 0.02;
	}
	// Quadruped idle tail/rear sway: the trailing (left) columns drift so a
	// settled animal isn't frozen. UV.x < 0.5 is the rear before the facing
	// mirror; scale by (1 - moving) so it only shows at rest.
	if ((rig_kind == 0 || rig_kind == 1) && act < 0.5) {
		float rear = clamp(0.5 - UV.x, 0.0, 0.5) * 2.0;
		VERTEX.x += sin(TIME * 2.2 + phase) * 0.03 * rear * (1.0 - anim.g);
	}
```

- [ ] **Step 3: Scene smoke — Primate unchanged**

Run: `godot --headless --rendering-driver dummy --path game res://scenes/main.tscn --quit-after 120`
Expected: `Initialize godot-rust` present, zero `SCRIPT ERROR`. (Shader still compiles; `rig_kind` defaults to 2 so apes are unaffected.)

- [ ] **Step 4: Commit**

```bash
git add game/shaders/field_agent.gdshader
git commit -m "feat(viewer): field shader rig_kind uniform + quadruped gallop/pounce/sway (primate unchanged)"
```

---

### Task 4: Quadruped atlas builder + per-species coat hue

Add the neutral value-ramp atlas builder to the registry and the per-species coat-hue function, plus the `_body_colors` species-mode change in `main.gd`. Not yet wired into bucketing (still safe — `main.gd` renders apes until Task 5), so verify via a throwaway print/headless smoke.

**Files:**
- Modify: `game/scripts/mammal_sprites.gd`
- Modify: `game/scripts/main.gd` (`_body_colors`, ~L648 default arm)

**Interfaces:**
- Consumes: `ApeSprites._build_cell(blocks)` (already static), the pose-slot contract.
- Produces:
  - `static func coat_hue(archetype: int, species_id: int) -> Color` — stable per (archetype, species), inside the archetype's palette band.
  - `static func build_quad_atlas(poses: Array) -> ImageTexture` — bakes a 16×(12·16) neutral strip from a rig's 12 pose block-lists using the neutral zone palette.
  - `static func build_quad_fallen(poses: Array) -> ImageTexture` — neutral pose rotated 90° CW (matches `ApeSprites.build_fallen_texture`).
  - `const QUAD_ZONES` — neutral grayscale palette for keys `"c"/"u"/"e"/"n"`.

- [ ] **Step 1: Add the neutral palette + coat-hue + quadruped builders to `mammal_sprites.gd`**

```gdscript
# Neutral value-ramp for quadruped rigs: coat mid, underside light, eye dark,
# nose a touch lighter than coat. A per-instance coat-hue modulate (main.gd
# _body_colors) turns this into a counter-shaded coloured animal.
const QUAD_ZONES := {
	"c": Color(0.60, 0.60, 0.60),
	"u": Color(0.92, 0.92, 0.92),
	"n": Color(0.74, 0.74, 0.74),
	"e": Color(0.10, 0.10, 0.11),
}

# Per-archetype coat palette band: base hue, hue jitter, saturation, value.
# Species id jitters the hue within the band so a herd varies without leaving
# the archetype's look (foxes rusty, wolves grey-brown, deer tan, hares sandy).
const _COAT_BAND := {
	HARE: [0.09, 0.05, 0.35, 0.82],
	DEER: [0.07, 0.04, 0.45, 0.72],
	BOAR: [0.05, 0.03, 0.30, 0.55],
	FOX: [0.045, 0.02, 0.75, 0.90],
	WOLF: [0.08, 0.06, 0.18, 0.62],
	LIVESTOCK: [0.08, 0.10, 0.25, 0.80],
}


static func coat_hue(archetype: int, species_id: int) -> Color:
	if not _COAT_BAND.has(archetype):
		return Color(1, 1, 1)  # PRIMATE: atlas already carries its colours.
	var band: Array = _COAT_BAND[archetype]
	# Deterministic jitter in [-1, 1] from the species id.
	var j := sin(float(species_id) * 12.9898) 
	var hue: float = fposmod(band[0] + j * band[1], 1.0)
	return Color.from_hsv(hue, band[2], band[3])


static func build_quad_atlas(poses: Array) -> ImageTexture:
	var atlas := Image.create(16, POSE_COUNT * 16, false, Image.FORMAT_RGBA8)
	atlas.fill(Color(0, 0, 0, 0))
	for fr in POSE_COUNT:
		var cell: Image = _build_quad_cell(poses[fr])
		cell.flip_y()
		atlas.blit_rect(cell, Rect2i(0, 0, 16, 16), Vector2i(0, fr * 16))
	return ImageTexture.create_from_image(atlas)


static func build_quad_fallen(poses: Array) -> ImageTexture:
	var blocks: Array = []
	for b in poses[0]:
		blocks.append([15 - (b[1] + b[3]), b[0], b[3], b[2], b[4]])
	var cell: Image = _build_quad_cell(blocks)
	cell.flip_y()
	return ImageTexture.create_from_image(cell)


# Like ApeSprites._build_pose but resolves zone keys against QUAD_ZONES (grays)
# and reuses the shared outline pass by handing explicit RGBA to _build_cell.
static func _build_quad_cell(pose: Array) -> Image:
	var blocks: Array = []
	for b in pose:
		var col: Color = QUAD_ZONES[b[4]]
		# _build_cell paints white when len < 5, or PAL[key] when len == 5; it
		# has no gray keys, so pre-resolve to explicit rgba via a 6-int block
		# form we add below.
		blocks.append([b[0], b[1], b[2], b[3], col])
	return _build_quad_cell_rgba(blocks)


# 16x16 cell from [x,y,w,h,Color] blocks + the same auto 1px dark outline as
# ApeSprites._build_cell (copied because that one keys colours through PAL).
static func _build_quad_cell_rgba(blocks: Array) -> Image:
	var img := Image.create(16, 16, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	for b in blocks:
		img.fill_rect(Rect2i(b[0], b[1], b[2], b[3]), b[4])
	var dirs := [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]
	var edges: Array = []
	for y in 16:
		for x in 16:
			if img.get_pixel(x, y).a > 0.0:
				continue
			for d in dirs:
				var nx: int = x + d.x
				var ny: int = y + d.y
				if nx >= 0 and nx < 16 and ny >= 0 and ny < 16 and img.get_pixel(nx, ny).a > 0.5:
					edges.append(Vector2i(x, y))
					break
	for e in edges:
		img.set_pixel(e.x, e.y, Color(0.20, 0.20, 0.22, 1.0))
	return img
```

- [ ] **Step 2: Update `main.gd` `_body_colors` species-mode arm** — replace the default (`_:`) arm (~L648) so quadruped archetypes get their coat hue while Primate stays white:

```gdscript
		_:
			# Species mode: Primate atlases carry their own coat/skin colours, so
			# white; quadruped atlases are neutral grayscale, so each agent gets
			# its per-species coat hue here. Diet/size come from the same batches
			# _refresh_bodies already fetched.
			var out4 := PackedColorArray()
			out4.resize(n)
			var diet: PackedFloat32Array = sim.alive_diet()
			var sizes: PackedFloat32Array = sim.alive_sizes()
			var sp_ids: PackedInt32Array = sim.alive_species_ids()
			var live: PackedInt32Array = _livestock_flags(n)
			for i in n:
				var arch := MammalSprites.archetype_for(diet[i], sizes[i], live[i] != 0)
				out4[i] = MammalSprites.coat_hue(arch, sp_ids[i])
			return out4
```

Add the `MammalSprites` preload at the top of `main.gd` (next to `ApeSprites`):

```gdscript
const MammalSprites = preload("res://scripts/mammal_sprites.gd")
```

Add a small helper (near `_body_colors`) that fetches livestock flags only when the scenario enables domestication (else all-zero, cheaply):

```gdscript
func _livestock_flags(n: int) -> PackedInt32Array:
	if sim.domestication_enabled():
		return sim.alive_livestock_flags()
	var z := PackedInt32Array()
	z.resize(n)
	return z
```

- [ ] **Step 3: Scene smoke**

Run: `godot --headless --rendering-driver dummy --path game res://scenes/main.tscn --quit-after 120`
Expected: `Initialize godot-rust`, zero `SCRIPT ERROR`. (Visuals unchanged — `_body_colors` returns white for Primate, and every agent is still bucketed as an ape until Task 5.)

- [ ] **Step 4: Format + lint**

Run: `gdformat game/scripts/mammal_sprites.gd game/scripts/main.gd && gdformat --check game/scripts/ && gdlint game/scripts/`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add game/scripts/mammal_sprites.gd game/scripts/main.gd
git commit -m "feat(viewer): quadruped neutral-atlas builder + per-species coat hue"
```

---

### Task 5: Rewire `main.gd` bucketing to the archetype registry (still all-Primate art)

The highest-risk change, done while the registry still maps every archetype to Primate art (Task 2's delegation) so the on-screen result is **identical** — this proves the rewire in isolation before any quadruped art exists.

**Files:**
- Modify: `game/scripts/main.gd` — `_ready` body/death setup (~L114-172), `_refresh_bodies` bucketing (~L472-538), `_on_agent_death`/`_refresh_death_effects` (~L549-589).

**Interfaces:**
- Consumes: `MammalSprites.ARCHETYPE_COUNT`, `archetype_for`, `rig_kind`, `build_atlas`, `build_fallen`, `primate_skin_for`, `POSE_COUNT`; `sim.alive_diet()`, `sim.alive_sizes()`, `sim.alive_species_ids()`, `_livestock_flags(n)`.
- Produces: `main.gd` buckets by archetype (7 buckets); death ghosts keyed by archetype; `_prev_arch` replaces `_prev_sp` for death bucketing.

- [ ] **Step 1: `_ready` — build 7 archetype MultiMeshes/materials.** Replace the `WALK_FPS`/species build loops (~L114-136) with an archetype loop. Because the neutral atlas needs no per-species bake, build one atlas per archetype using a representative species id (0); Primate delegates to its own per-species atlas keyed later per instance is not possible on a shared MultiMesh, so Primate uses skin 0's atlas here and per-instance color still tints — matching today where each ape species already had its own MultiMesh. **Keep one MultiMesh per archetype; Primate variety across the 5 hominins is preserved because Primate's atlas already carries hominin 0's colours and the inspector still names the exact hominin.** (Full per-hominin field variety is out of scope — see Non-goals.)

```gdscript
	# Per-archetype gait cadence (frames/sec): hares scurry, deer lope, wolves
	# trot, the primate strides, livestock amble.
	const GAIT_FPS := [7.0, 4.6, 5.2, 4.8, 6.5, 5.6, 4.0]
	_body_mmis.append(bodies)
	for a in range(1, MammalSprites.ARCHETYPE_COUNT):
		var mmi := MultiMeshInstance2D.new()
		mmi.name = "Bodies%d" % a
		var mm := MultiMesh.new()
		mm.transform_format = MultiMesh.TRANSFORM_2D
		mm.use_colors = true
		mm.use_custom_data = true
		mm.mesh = bodies.multimesh.mesh
		mmi.multimesh = mm
		add_child(mmi)
		move_child(mmi, bodies.get_index() + a)
		_body_mmis.append(mmi)
	for a in MammalSprites.ARCHETYPE_COUNT:
		var mmi := _body_mmis[a]
		mmi.texture = MammalSprites.build_atlas(a, 0)
		mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		var sp_mat := ShaderMaterial.new()
		sp_mat.shader = FieldAgentShader
		sp_mat.set_shader_parameter("frames", MammalSprites.POSE_COUNT)
		sp_mat.set_shader_parameter("walk_fps", GAIT_FPS[a])
		sp_mat.set_shader_parameter("rig_kind", MammalSprites.rig_kind(a))
		mmi.material = sp_mat
```

Update the death-ghost setup loop (~L159-172) to iterate `MammalSprites.ARCHETYPE_COUNT` and use `MammalSprites.build_fallen(a, 0)`.

- [ ] **Step 2: `_refresh_bodies` — bucket by archetype.** After the existing `var sizes := sim.alive_sizes()` etc. (it already fetches `sizes`, `sp_ids`, `rots`; add `diet` and `live`), replace the ape-bucket block (~L472-478):

```gdscript
	var diet: PackedFloat32Array = sim.alive_diet()
	var live: PackedInt32Array = _livestock_flags(n)
	var buckets: Array = []
	for a in MammalSprites.ARCHETYPE_COUNT:
		buckets.append(PackedInt32Array())
	var arch_of := PackedInt32Array()
	arch_of.resize(n)
	for i in n:
		var a := MammalSprites.archetype_for(diet[i], sizes[i], live[i] != 0) if have_sp else MammalSprites.PRIMATE
		arch_of[i] = a
		buckets[a].append(i)
```

Change the outer loop `for sp in ApeSprites.SPECIES_COUNT:` → `for a in MammalSprites.ARCHETYPE_COUNT:` and index `_body_mmis[a]`, `buckets[a]`. The per-instance transform/color/custom writes inside are unchanged. Record `_prev_arch` alongside `_prev_sp` for death bucketing (store `arch_of[i]` into a member array parallel to `_prev_smooth`).

- [ ] **Step 3: Death ghosts by archetype.** In `_on_agent_death` (~L555-558) replace `ApeSprites.ape_for_species(_prev_sp[prev_idx])` with the recorded `_prev_arch[prev_idx]`; in `_refresh_death_effects` (~L577-581) iterate `MammalSprites.ARCHETYPE_COUNT`. (Add a `_prev_arch: PackedInt32Array` member, written in `_refresh_bodies` next to `_prev_sp`/`_prev_sizes`.)

- [ ] **Step 4: Scene smoke — output identical.**

Run: `godot --headless --rendering-driver dummy --path game res://scenes/main.tscn --quit-after 120`
Expected: `Initialize godot-rust`, zero `SCRIPT ERROR`.

- [ ] **Step 5: Visual parity capture.** Compare against `main` before the branch.

```bash
godot --headless --path game res://scenes/main.tscn --quit-after 300 \
  # with ANABIOS_ZOOM/ANABIOS_PIN env + debug_capture as used in showcase runs
```

Expected: agents still render as the same hominins (registry maps all→Primate), confirming the rewire is behaviorally neutral.

- [ ] **Step 6: Format, lint, commit**

Run: `gdformat game/scripts/main.gd && gdformat --check game/scripts/ && gdlint game/scripts/`

```bash
git add game/scripts/main.gd
git commit -m "refactor(viewer): bucket field bodies by archetype registry (primate-identical)"
```

---

### Task 6: First quadruped rig — Wolf — live end-to-end

Turn on real archetype selection and give Wolf real quadruped art. Carnivore+large agents now render as animated wolves; this proves the whole pipeline (art → atlas → coat hue → shader gallop/pounce → bucketing).

**Files:**
- Create: `game/scripts/mammal_data/wolf.gd`
- Modify: `game/scripts/mammal_sprites.gd` — replace `build_atlas`/`build_fallen` Primate-delegation with a per-archetype dispatch that uses `build_quad_atlas` for quadrupeds and `ApeSprites` for Primate.

**Interfaces:**
- Consumes: `build_quad_atlas`, `build_quad_fallen`, `QUAD_ZONES`.
- Produces: `WolfData.POSES: Array` (12 pose block-lists); registry dispatch `build_atlas(archetype, species_id)` → quad atlas for `WOLF`, Primate art otherwise (other quadrupeds still fall back to Primate until their task lands).

- [ ] **Step 1: Author `game/scripts/mammal_data/wolf.gd`.** Side profile, facing right (head at high x), y=0 top/back → y=15 bottom/feet; zone keys `c` coat, `u` underbelly/muzzle, `n` nose, `e` eye. The 12 poses follow the slot contract. This is the **worked reference rig**; expect ≤1px nudges during Step 6's capture.

```gdscript
extends RefCounted
# Wolf — the predator/quadruped reference rig (mammal_sprites RigKind.PREDATOR).
# 16x16, facing right. Slot layout matches ape_sprites: 0 stand, 1/2/3 trot
# (contact-L / passing / contact-R), 4/5 graze, 6/7 bite, 8/9 alert, 10/11
# gallop. Blocks are [x, y, w, h, zone]; painted back-to-front, auto-outlined.

const POSES: Array = [
	# 0 stand
	[
		[0, 5, 4, 3, "c"], [1, 4, 2, 2, "c"],           # bushy tail
		[3, 4, 8, 5, "c"],                                # torso
		[4, 8, 6, 1, "u"],                                # underbelly
		[10, 3, 4, 5, "c"], [14, 5, 2, 2, "n"],           # head + muzzle
		[13, 3, 1, 1, "e"],                               # eye
		[10, 1, 1, 2, "c"], [12, 1, 1, 2, "c"],           # ears
		[3, 9, 2, 5, "c"], [6, 9, 2, 5, "c"],             # hind legs
		[9, 9, 2, 5, "c"], [12, 9, 1, 5, "c"],            # fore legs
		[3, 14, 2, 1, "u"], [9, 14, 2, 1, "u"],           # paws
	],
	# 1 contact-L — near-hind + far-fore reach forward, other diagonal trails
	[
		[0, 5, 4, 3, "c"], [1, 4, 2, 2, "c"],
		[3, 4, 8, 5, "c"], [4, 8, 6, 1, "u"],
		[10, 3, 4, 5, "c"], [14, 5, 2, 2, "n"], [13, 3, 1, 1, "e"],
		[10, 1, 1, 2, "c"], [12, 1, 1, 2, "c"],
		[2, 9, 2, 5, "c"], [7, 10, 2, 4, "c"],            # hind: far back, near under
		[10, 10, 2, 4, "c"], [12, 9, 1, 5, "c"],          # fore: near under, far ahead
		[2, 14, 2, 1, "u"], [10, 14, 2, 1, "u"],
	],
	# 2 passing — legs gathered, whole figure lifted 1px (the trot bob)
	[
		[0, 4, 4, 3, "c"], [1, 3, 2, 2, "c"],
		[3, 3, 8, 5, "c"], [4, 7, 6, 1, "u"],
		[10, 2, 4, 5, "c"], [14, 4, 2, 2, "n"], [13, 2, 1, 1, "e"],
		[10, 0, 1, 2, "c"], [12, 0, 1, 2, "c"],
		[5, 8, 2, 4, "c"], [7, 8, 2, 4, "c"],             # hind gathered
		[9, 8, 2, 4, "c"], [11, 8, 2, 4, "c"],            # fore gathered
	],
	# 3 contact-R — mirror diagonal of pose 1
	[
		[0, 5, 4, 3, "c"], [1, 4, 2, 2, "c"],
		[3, 4, 8, 5, "c"], [4, 8, 6, 1, "u"],
		[10, 3, 4, 5, "c"], [14, 5, 2, 2, "n"], [13, 3, 1, 1, "e"],
		[10, 1, 1, 2, "c"], [12, 1, 1, 2, "c"],
		[4, 10, 2, 4, "c"], [6, 9, 2, 5, "c"],
		[9, 9, 2, 5, "c"], [12, 10, 1, 4, "c"],
		[4, 13, 2, 1, "u"], [9, 14, 2, 1, "u"],
	],
	# 4 graze A — head lowered toward the ground
	[
		[0, 5, 4, 3, "c"], [1, 4, 2, 2, "c"],
		[3, 4, 8, 5, "c"], [4, 8, 6, 1, "u"],
		[11, 6, 3, 5, "c"], [14, 9, 2, 2, "n"], [13, 6, 1, 1, "e"],
		[11, 4, 1, 2, "c"], [13, 4, 1, 2, "c"],
		[3, 9, 2, 5, "c"], [6, 9, 2, 5, "c"],
		[9, 9, 2, 5, "c"], [12, 9, 1, 5, "c"],
	],
	# 5 graze B — nose to the dirt, a small dip
	[
		[0, 5, 4, 3, "c"], [1, 4, 2, 2, "c"],
		[3, 4, 8, 5, "c"], [4, 8, 6, 1, "u"],
		[11, 7, 3, 5, "c"], [14, 11, 2, 1, "n"], [13, 7, 1, 1, "e"],
		[11, 5, 1, 2, "c"], [13, 5, 1, 2, "c"],
		[3, 9, 2, 5, "c"], [6, 9, 2, 5, "c"],
		[9, 9, 2, 5, "c"], [12, 9, 1, 5, "c"],
	],
	# 6 bite A — head drawn back, haunches braced (wind-up)
	[
		[0, 6, 4, 3, "c"], [1, 5, 2, 2, "c"],
		[3, 4, 8, 5, "c"], [4, 8, 6, 1, "u"],
		[9, 3, 4, 5, "c"], [13, 5, 2, 2, "n"], [12, 3, 1, 1, "e"],
		[9, 1, 1, 2, "c"], [11, 1, 1, 2, "c"],
		[3, 9, 2, 5, "c"], [6, 9, 2, 5, "c"],
		[9, 9, 2, 5, "c"], [11, 9, 2, 5, "c"],
	],
	# 7 bite B — lunge: head thrust forward, jaws out past the frame edge
	[
		[0, 6, 4, 3, "c"], [1, 5, 2, 2, "c"],
		[3, 4, 8, 5, "c"], [4, 8, 6, 1, "u"],
		[10, 4, 5, 4, "c"], [15, 5, 1, 2, "n"], [14, 4, 1, 1, "e"],
		[10, 2, 1, 2, "c"], [12, 2, 1, 2, "c"],
		[3, 10, 2, 4, "c"], [6, 10, 2, 4, "c"],
		[10, 9, 2, 5, "c"], [12, 9, 1, 5, "c"],
	],
	# 8 alert A — head up, ears pricked
	[
		[0, 4, 4, 3, "c"], [1, 3, 2, 2, "c"],
		[3, 4, 8, 5, "c"], [4, 8, 6, 1, "u"],
		[10, 1, 4, 5, "c"], [14, 3, 2, 2, "n"], [13, 1, 1, 1, "e"],
		[10, -1, 1, 2, "c"], [12, -1, 1, 2, "c"],
		[3, 9, 2, 5, "c"], [6, 9, 2, 5, "c"],
		[9, 9, 2, 5, "c"], [12, 9, 1, 5, "c"],
	],
	# 9 alert B — a small weight shift (ears flick back)
	[
		[0, 5, 4, 3, "c"], [1, 4, 2, 2, "c"],
		[3, 4, 8, 5, "c"], [4, 8, 6, 1, "u"],
		[10, 2, 4, 5, "c"], [14, 4, 2, 2, "n"], [13, 2, 1, 1, "e"],
		[9, 1, 1, 2, "c"], [11, 1, 1, 2, "c"],
		[3, 9, 2, 5, "c"], [6, 9, 2, 5, "c"],
		[9, 9, 2, 5, "c"], [12, 9, 1, 5, "c"],
	],
	# 10 gallop A — body extended, fore reaching, hind trailing
	[
		[0, 6, 4, 3, "c"], [1, 5, 2, 2, "c"],
		[3, 5, 8, 4, "c"], [4, 8, 6, 1, "u"],
		[10, 4, 4, 5, "c"], [14, 6, 2, 2, "n"], [13, 4, 1, 1, "e"],
		[10, 2, 1, 2, "c"], [12, 2, 1, 2, "c"],
		[1, 9, 2, 5, "c"], [3, 10, 2, 4, "c"],            # hind flung back
		[11, 9, 2, 5, "c"], [13, 10, 2, 4, "c"],          # fore reaching
	],
	# 11 gallop B — gathered: legs sweep under, body compressed
	[
		[0, 6, 4, 3, "c"], [1, 5, 2, 2, "c"],
		[4, 5, 7, 4, "c"], [5, 8, 5, 1, "u"],
		[10, 4, 4, 5, "c"], [14, 6, 2, 2, "n"], [13, 4, 1, 1, "e"],
		[10, 2, 1, 2, "c"], [12, 2, 1, 2, "c"],
		[5, 9, 2, 4, "c"], [7, 9, 2, 4, "c"],
		[9, 9, 2, 4, "c"], [11, 9, 2, 4, "c"],
	],
]
```

- [ ] **Step 2: Registry dispatch.** In `mammal_sprites.gd`, add the Wolf data preload and replace `build_atlas`/`build_fallen`:

```gdscript
const _QUAD_DATA := {
	WOLF: preload("res://scripts/mammal_data/wolf.gd"),
}


static func build_atlas(archetype: int, species_id: int) -> ImageTexture:
	if _QUAD_DATA.has(archetype):
		return build_quad_atlas(_QUAD_DATA[archetype].POSES)
	# Primate + not-yet-authored quadrupeds fall back to hominin art.
	return ApeSprites.build_species_atlas(ApeSprites.ape_for_species(species_id))


static func build_fallen(archetype: int, species_id: int) -> ImageTexture:
	if _QUAD_DATA.has(archetype):
		return build_quad_fallen(_QUAD_DATA[archetype].POSES)
	return ApeSprites.build_fallen_texture(ApeSprites.ape_for_species(species_id))
```

- [ ] **Step 3: Scene smoke.**

Run: `godot --headless --rendering-driver dummy --path game res://scenes/main.tscn --quit-after 120`
Expected: `Initialize godot-rust`, zero `SCRIPT ERROR`.

- [ ] **Step 4: Visual capture — wolves on screen.** Use a predator-bearing scenario (pick one whose species include a carnivore+large morph; `scenarios/divergent.toml` or any predator/prey world). Run with the zoom/pin capture hooks:

```bash
ANABIOS_ZOOM=8 ANABIOS_PIN=1 godot --headless --path game \
  res://scenes/main.tscn --quit-after 400
```

Expected: carnivore+large agents render as grey-brown wolves that trot (leg cycle), stretch into a gallop when fleeing, and pounce on attack; herbivores/omnivores still render as apes (their rigs land next task).

- [ ] **Step 5: Nudge pixels if needed**, re-capture until the wolf reads cleanly at the showcase zoom. Keep `wolf.gd` under 250 lines.

- [ ] **Step 6: Format, lint, commit**

Run: `gdformat game/scripts/mammal_sprites.gd game/scripts/mammal_data/wolf.gd && gdformat --check game/scripts/ && gdlint game/scripts/`

```bash
git add game/scripts/mammal_sprites.gd game/scripts/mammal_data/wolf.gd
git commit -m "feat(viewer): live archetype selection + Wolf quadruped rig end-to-end"
```

---

### Task 7: Remaining five rigs (Deer, Hare, Boar, Fox, Livestock)

One sub-cycle per rig: author `mammal_data/<rig>.gd` against the Wolf template, register it in `_QUAD_DATA`, smoke, capture, nudge, commit. Each rig is prey/predator/livestock per the registry's `_RIG_KIND`, so the shader flourishes come for free. Per-rig silhouette specs (16×16, facing right, same slot contract):

- [ ] **Step 1: Deer** (`deer.gd`, prey, large). Tall thin legs (x-thin, y9→15), long neck angled up-right, small head with two upright ears and a short muzzle; short tail; slim torso (y5-8). Graze poses drop the whole neck+head to the ground (a deer's signature). Gallop = long bounding leap (the shader's prey hop is highest here). Coat tan (`_COAT_BAND[DEER]`). Register `DEER`. Capture on a herbivore-heavy scenario. Commit `feat(viewer): Deer rig`.

- [ ] **Step 2: Hare** (`hare.gd`, prey, small). Compact rounded body (y6-10), very short fore legs, large folded hind legs (the hop engine), two long ears laid along the back, tiny tail. Idle = mostly still with the shader's ear/rear sway; flee = big hop (prey hop). Sandy coat. Register `HARE`. Capture. Commit `feat(viewer): Hare rig`.

- [ ] **Step 3: Boar** (`boar.gd`, prey-family, small omnivore). Low stocky body (y5-10), short thick legs, big wedge head with a down-snout (`n`) and a small tusk pixel (`u`), bristly back ridge (1px `c` bumps along the top), short tail. Graze = snout roots at the ground. Dark grizzled coat (low value/sat). Register `BOAR`. Capture on an omnivore scenario. Commit `feat(viewer): Boar rig`.

- [ ] **Step 4: Fox** (`fox.gd`, predator, small). Slim low body, pointed head with large triangular ears, a big **bushy tail** (its signature — wide `c` block at back, tipped `u`), slender legs with `u` "socks". Pounce (predator arc) on attack. Rusty coat (high sat). Register `FOX`. Capture. Commit `feat(viewer): Fox rig`.

- [ ] **Step 5: Livestock** (`livestock.gd`, livestock family). Placid bovine/goat frame: deep barrel body (y4-9), short sturdy legs, blunt head with two short down-horns (`c`) and a broad muzzle (`n`); minimal tail. No gallop drama — its flee poses are a mild trot; idle uses the shader's chewing head-bob (`rig_kind == 3`). Register `LIVESTOCK`. Capture on a **domestication** scenario (`domestication_enabled`, so `_livestock_flags` is non-zero and tamed agents select `LIVESTOCK`). Commit `feat(viewer): Livestock rig`.

After all five: `gdformat --check game/scripts/ && gdlint game/scripts/` clean, every `mammal_data/*.gd` under 250 lines.

---

### Task 8: Inspector + settlement call-site updates

Point the two remaining `ApeSprites.ape_for_species` consumers at the registry so labels/colors match the field art.

**Files:**
- Modify: `game/scripts/inspector_panel.gd` (~L82-86)
- Modify: `game/scripts/settlement_layer.gd` (~L123-124)

**Interfaces:**
- Consumes: `MammalSprites.archetype_for`, `MammalSprites.NAMES`, `MammalSprites.coat_hue`, `MammalSprites.primate_skin_for`, `ApeSprites.NAMES`; per-agent `diet_carnivory`, `size`/genome Size, `livestock_of`, `species_id` already in the inspector dict.

- [ ] **Step 1: Inspector avatar/name.** Compute the pinned agent's archetype from its inspector dict (`diet_carnivory`, size, livestock ownership) and show `MammalSprites.NAMES[arch]`; when `arch == PRIMATE`, also show the hominin name `ApeSprites.NAMES[MammalSprites.primate_skin_for(sp)]` and keep the existing ape avatar. For quadrupeds, show the archetype name (avatar art for the inspector header is optional polish — reuse the existing ape avatar or a neutral disc; do NOT block on a per-archetype inspector portrait).

- [ ] **Step 2: Settlement hut color.** Replace `ApeSprites.ape_for_species(sid)` + `ApeSprites.PAL[...]` with `MammalSprites.coat_hue(MammalSprites.archetype_for(diet, size, false), sid)` for the hut zone tint. (Settlements are culture-bearing, effectively Primate; if diet/size aren't readily available at that call site, default to `MammalSprites.PRIMATE` and use `coat_hue(PRIMATE, sid)` which returns white — then keep the existing ape PAL color for Primate huts.)

- [ ] **Step 3: Scene smoke + inspector capture.**

Run: `godot --headless --rendering-driver dummy --path game res://scenes/main.tscn --quit-after 120`
Expected: `Initialize godot-rust`, zero `SCRIPT ERROR`. Pin a wolf and a deer; the inspector names them correctly.

- [ ] **Step 4: Format, lint, commit**

```bash
git add game/scripts/inspector_panel.gd game/scripts/settlement_layer.gd
git commit -m "feat(viewer): inspector + settlement use the archetype registry"
```

---

### Task 9: Threshold tuning + full-gate verification

**Files:** possibly `game/scripts/mammal_sprites.gd` (`SIZE_SPLIT`/`HERB_MAX`/`CARN_MIN`), pose nudges.

- [ ] **Step 1: Capture across three scenarios** — a herbivore-heavy world, a predator/prey world, and a domestication world. Confirm each archetype appears where expected and the food web reads (grazers ≠ predators ≠ livestock at a glance).

- [ ] **Step 2: Tune thresholds** if a scenario mis-buckets (e.g. its "predators" sit below `CARN_MIN`, or its herd animals fall under `SIZE_SPLIT`). Re-run `test_mammal_sprites.gd` after any const change (its boundary asserts reference the consts, so they follow automatically).

- [ ] **Step 3: Full CI gate locally.**

```bash
cargo fmt --check
cargo clippy -p anabios-godot -- -D warnings
cargo test -p anabios-godot
cargo build -p anabios-godot
gdformat --check game/scripts/
gdlint game/scripts/
godot --headless --rendering-driver dummy --path game res://scenes/main.tscn --quit-after 120
godot --headless --rendering-driver dummy --path game -s res://scripts/test_mammal_sprites.gd
```

Expected: all clean; smoke shows `Initialize godot-rust` + zero `SCRIPT ERROR`; the selector test prints all-passed.

- [ ] **Step 4: Final commit**

```bash
git add -u game/scripts crates/anabios-godot
git commit -m "chore(viewer): tune archetype thresholds; full mammal roster green"
```

---

## Self-review

**Spec coverage:**
- §Component 1 registry → Tasks 2, 6, 7. §Component 2 selection → Task 2 (+tuning Task 9). §Component 3 atlas/tint → Task 4 (mask resolved to value-ramp, documented above). §Component 4 shader animation → Task 3 (+ per-rig flourishes exercised in 6/7). §Component 5 integration → Tasks 1 (accessor), 5 (bucketing), 8 (inspector/settlement). §Verification → every task's smoke + Task 9. §Build order → Tasks 1-9 follow it (accessor → registry → shader → atlas → rewire → template rig → remaining rigs → call sites → tune). All covered.
- Spec's tentative 1-bit mask is intentionally superseded by the value-ramp tint; documented in the Design note.

**Placeholder scan:** No TBD/TODO. The per-rig art in Task 7 is specified by concrete silhouette/slot specs against the fully-worked Wolf reference (Task 6), plus a capture-verify loop — pixel authoring is inherently iterative and cannot be pre-rendered blind, which is why one rig is worked in full and the rest are precise deltas. Inspector per-archetype portrait is explicitly marked optional, not a hidden gap.

**Type consistency:** `archetype_for(diet, size, livestock)`, `rig_kind(archetype)`, `coat_hue(archetype, species_id)`, `build_atlas(archetype, species_id)`, `build_fallen(archetype, species_id)`, `build_quad_atlas(poses)`, `livestock_flags_of(&World)`, `alive_livestock_flags()`, `domestication_enabled()`, `_livestock_flags(n)`, `_prev_arch` — used consistently across tasks. Enum `{HARE, DEER, BOAR, PRIMATE, FOX, WOLF, LIVESTOCK}` and `RigKind {PREY, PREDATOR, PRIMATE_RIG, LIVESTOCK_RIG}` referenced identically in the shader (int values 0-3) and registry.
