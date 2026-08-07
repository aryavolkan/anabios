# M7 — Full Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade the M6 viewer from "colored discs on black" to a readable world: a biome terrain background that shows water/grass/forest/desert/rock and live plant biomass, agent bodies oriented toward their velocity, and per-module sprite glyphs so you can see each creature's body plan at a glance.

**Architecture:** Extend `anabios-godot` with biome and per-module accessors. The biome becomes a 128×128 `ImageTexture` rebuilt each frame (cheap — 16k pixels) drawn as a world-sized `Sprite2D` behind the agents. Each module type gets its own `MultiMeshInstance2D` layer; the gdext layer returns, per module type, a `PackedVector2Array` of glyph world-positions (agent position + perimeter slot offset) plus a matching color array. Bodies gain rotation from velocity direction.

**Tech Stack:** Same as M6 (Rust + gdext 0.2.4, Godot 4.6, GDScript).

**Branch:** `m7-full-rendering` branched from `main`.

**Working directory:** `/Users/aryasen/projects/anabios/`.

**Scope note (medium effort):** This milestone delivers biome + module glyphs + body orientation. Pheromone trail buffers (no pheromone field yet), overlay toggles, camera follow modes, and animated locomotor sprite atlases are deferred to M8+.

---

## File structure after M7

Modified files:
```
crates/anabios-godot/src/lib.rs   # +biome accessors, +alive_rotations, +module_glyphs
game/scenes/main.tscn             # +Biome Sprite2D, +per-module MultiMesh layers
game/scripts/main.gd              # build biome texture, populate module layers, rotate bodies
game/scripts/biome_renderer.gd    # NEW: builds + updates the biome ImageTexture
```

---

## Task 0: Branch

- [ ] `git checkout main && git pull && git checkout -b m7-full-rendering`
- [ ] `cargo build -p anabios-godot` succeeds (M6 baseline).

---

## Task 1: gdext biome accessors

**Goal:** Expose the biome grid so GDScript can build a texture.

**Files:** Modify `crates/anabios-godot/src/lib.rs`

- [ ] **Step 1.1: Add biome methods**

Append to the `#[godot_api] impl Simulation` block:

```rust
    /// Biome grid resolution per axis (cells = res²).
    #[func]
    fn biome_resolution(&self) -> i64 {
        anabios_core::biome::BIOME_RES as i64
    }

    /// One color per biome cell, row-major (`row * RES + col`). Terrain
    /// type sets the base hue; live plant biomass brightens grass/forest
    /// cells. Returns `RES²` colors, or empty if no world is loaded.
    #[func]
    fn biome_colors(&self) -> PackedColorArray {
        use anabios_core::biome::TerrainType;
        let mut out = PackedColorArray::new();
        let Some(w) = self.inner.as_ref() else { return out };
        for cell in w.biome.cells.iter() {
            let base = match cell.terrain {
                TerrainType::Water => Color::from_rgb(0.12, 0.22, 0.42),
                TerrainType::Grass => Color::from_rgb(0.20, 0.42, 0.18),
                TerrainType::Forest => Color::from_rgb(0.10, 0.30, 0.12),
                TerrainType::Desert => Color::from_rgb(0.62, 0.56, 0.34),
                TerrainType::Rock => Color::from_rgb(0.36, 0.36, 0.40),
            };
            // Brighten by plant biomass fraction of carrying capacity.
            let cap = cell.terrain.carrying_capacity();
            let frac = if cap > 0.0 { (cell.plant_biomass / cap).clamp(0.0, 1.0) } else { 0.0 };
            let lit = base.lerp(Color::from_rgb(0.40, 0.85, 0.35), frac * 0.6);
            out.push(lit);
        }
        out
    }
```

- [ ] **Step 1.2: Build + commit**

```bash
cargo build -p anabios-godot
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-godot/src/lib.rs
git commit -m "feat(godot): biome_resolution + biome_colors accessors"
```

---

## Task 2: gdext body rotation + module glyph accessors

**Goal:** Expose body rotation (from velocity) and per-module-type glyph positions/colors.

**Files:** Modify `crates/anabios-godot/src/lib.rs`

- [ ] **Step 2.1: Add rotation accessor**

```rust
    /// Body rotation (radians) per alive agent, derived from velocity
    /// direction. Agents that aren't moving keep rotation 0. Same order
    /// as `alive_positions`.
    #[func]
    fn alive_rotations(&self) -> PackedFloat32Array {
        let mut out = PackedFloat32Array::new();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                let v = w.agents.velocity[id as usize];
                let r = if v.length_squared() > 1e-6 { v.y.atan2(v.x) } else { 0.0 };
                out.push(r);
            }
        }
        out
    }
```

- [ ] **Step 2.2: Add module glyph accessor**

The 9 module types are indexed by `ModuleType as u8`. For a requested type, return one glyph position per matching module across all alive agents, placed at a perimeter slot around the owner's body, plus a parallel color array.

```rust
    /// Glyph world-positions for every module of `module_type` (0..9) across
    /// all alive agents. Each module is placed at one of 8 evenly-spaced
    /// perimeter slots around its owner, scaled by the owner's size. Used to
    /// drive a per-type MultiMesh layer.
    #[func]
    fn module_glyphs(&self, module_type: i64) -> PackedVector2Array {
        use anabios_core::genome::GenomeSlot;
        let mut out = PackedVector2Array::new();
        let Some(w) = self.inner.as_ref() else { return out };
        let want = module_type as u8;
        for id in w.agents.iter_alive() {
            let i = id as usize;
            let pos = w.agents.position[i];
            let size = 0.5 + 2.5 * w.agents.genome[i].get(GenomeSlot::Size);
            let radius = size * 0.7;
            for (slot, m) in w.agents.modules[i].iter().enumerate() {
                if m.module_type() as u8 != want {
                    continue;
                }
                let angle = (slot as f32) * std::f32::consts::TAU / 8.0;
                let gx = pos.x + radius * crate::math_cos(angle);
                let gy = pos.y + radius * crate::math_sin(angle);
                out.push(Vector2::new(gx, gy));
            }
        }
        out
    }

    /// Number of module types (for the GDScript layer loop).
    #[func]
    fn module_type_count(&self) -> i64 {
        9
    }
```

Add free functions at the bottom of the file (gdext can't see anabios-core's private `mathf`, so wrap libm directly):

```rust
#[inline]
fn math_cos(x: f32) -> f32 {
    libm::cosf(x)
}

#[inline]
fn math_sin(x: f32) -> f32 {
    libm::sinf(x)
}
```

Add `libm` to `crates/anabios-godot/Cargo.toml` dependencies:

```toml
libm = { workspace = true }
```

- [ ] **Step 2.3: Build + commit**

```bash
cargo build -p anabios-godot
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-godot/Cargo.toml crates/anabios-godot/src/lib.rs
git commit -m "feat(godot): alive_rotations + module_glyphs + module_type_count accessors"
```

---

## Task 3: Biome background renderer

**Goal:** Draw the biome as a world-sized textured sprite behind the agents.

**Files:**
- Create: `game/scripts/biome_renderer.gd`
- Modify: `game/scenes/main.tscn` (add a `Sprite2D` named `Biome` as the first child, behind `Bodies`)

- [ ] **Step 3.1: biome_renderer.gd**

```gdscript
extends Sprite2D

@onready var sim = get_node("/root/Main/Simulation")

var _img: Image
var _tex: ImageTexture
var _res: int = 0

func _ready() -> void:
	_res = int(sim.biome_resolution())
	if _res <= 0:
		return
	_img = Image.create(_res, _res, false, Image.FORMAT_RGBA8)
	_tex = ImageTexture.create_from_image(_img)
	texture = _tex
	centered = false
	# Scale the RES×RES texture up to cover the whole world.
	var world: float = sim.world_size()
	scale = Vector2(world / _res, world / _res)
	position = Vector2.ZERO
	# Draw behind everything else.
	z_index = -10

func _process(_delta: float) -> void:
	if _res <= 0:
		return
	var colors: PackedColorArray = sim.biome_colors()
	if colors.size() != _res * _res:
		return
	for row in _res:
		for col in _res:
			_img.set_pixel(col, row, colors[row * _res + col])
	_tex.update(_img)
```

- [ ] **Step 3.2: Add Biome node to main.tscn**

Add an ext_resource for the script and insert the node BEFORE `Bodies` (so it renders behind). Add near the top:

```tres
[ext_resource type="Script" path="res://scripts/biome_renderer.gd" id="7_biome"]
```

And add the node (place it right after the `Simulation` node, before `Bodies`):

```tres
[node name="Biome" type="Sprite2D" parent="."]
script = ExtResource("7_biome")
```

- [ ] **Step 3.3: Smoke test**

```bash
cargo build -p anabios-godot
godot --headless --quit --path game/ 2>&1 | tail -10
```

Expected: no errors. (Per-pixel `set_pixel` over 16k pixels per frame is acceptable for an MVP; optimize to `set_data` later if needed.)

- [ ] **Step 3.4: Commit**

```bash
git add game/scripts/biome_renderer.gd game/scenes/main.tscn
git commit -m "feat(game): biome terrain + plant-biomass background renderer"
```

---

## Task 4: Per-module sprite layers + body rotation

**Goal:** Add a `MultiMeshInstance2D` per module type, populated from `module_glyphs`. Rotate bodies by `alive_rotations`.

**Files:**
- Modify: `game/scripts/main.gd`
- Modify: `game/scenes/main.tscn` (add a `ModuleLayers` Node2D with 9 MultiMeshInstance2D children)

- [ ] **Step 4.1: Per-module colors + small mesh**

Each module type renders as a small colored square. Define a fixed palette in GDScript (9 entries). Glyphs are smaller than bodies (~0.6 world units).

- [ ] **Step 4.2: Update main.gd**

Replace `_refresh_bodies` and add module-layer population:

```gdscript
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
const GLYPH_SIZE: float = 0.7

@onready var module_layers: Node2D = $ModuleLayers

func _refresh_bodies() -> void:
	var n: int = int(sim.alive_count())
	var mm: MultiMesh = bodies.multimesh
	if n > mm.instance_count:
		mm.instance_count = n
	mm.visible_instance_count = n
	if n == 0:
		_clear_module_layers()
		return

	var positions: PackedVector2Array = sim.alive_positions()
	var colors: PackedColorArray = sim.alive_colors()
	var sizes: PackedFloat32Array = sim.alive_sizes()
	var rots: PackedFloat32Array = sim.alive_rotations()
	for i in n:
		var t: Transform2D = Transform2D(rots[i], Vector2(sizes[i], sizes[i]), 0.0, positions[i])
		mm.set_instance_transform_2d(i, t)
		mm.set_instance_color(i, colors[i])

	_refresh_module_layers()

func _refresh_module_layers() -> void:
	var type_count: int = int(sim.module_type_count())
	for t in type_count:
		var layer: MultiMeshInstance2D = module_layers.get_child(t)
		var glyphs: PackedVector2Array = sim.module_glyphs(t)
		var m: int = glyphs.size()
		var mm: MultiMesh = layer.multimesh
		if m > mm.instance_count:
			mm.instance_count = m
		mm.visible_instance_count = m
		var col: Color = MODULE_COLORS[t]
		for i in m:
			mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(GLYPH_SIZE, GLYPH_SIZE), 0.0, glyphs[i]))
			mm.set_instance_color(i, col)

func _clear_module_layers() -> void:
	for child in module_layers.get_children():
		(child as MultiMeshInstance2D).multimesh.visible_instance_count = 0
```

- [ ] **Step 4.3: Add ModuleLayers to main.tscn**

Add a `ModuleLayers` Node2D with 9 `MultiMeshInstance2D` children, each with its own MultiMesh sub-resource (QuadMesh, use_colors=true, transform_format=0, instance_count=10000). To avoid 9 separate .tres files, define the MultiMeshes as inline sub-resources in main.tscn.

Add 9 sub-resources at the top of the scene file:

```tres
[sub_resource type="QuadMesh" id="QuadMesh_glyph"]
size = Vector2(1, 1)
```

Then for each of the 9 layers, a MultiMesh sub-resource:

```tres
[sub_resource type="MultiMesh" id="MM_mod0"]
transform_format = 0
use_colors = true
instance_count = 10000
visible_instance_count = 0
mesh = SubResource("QuadMesh_glyph")
```

(Repeat `MM_mod0`..`MM_mod8`.)

Then the node tree:

```tres
[node name="ModuleLayers" type="Node2D" parent="."]

[node name="Layer0" type="MultiMeshInstance2D" parent="ModuleLayers"]
multimesh = SubResource("MM_mod0")

[node name="Layer1" type="MultiMeshInstance2D" parent="ModuleLayers"]
multimesh = SubResource("MM_mod1")
```

…through `Layer8`. Place `ModuleLayers` after `Bodies` so glyphs render on top of bodies.

- [ ] **Step 4.4: Smoke test + commit**

```bash
cargo build -p anabios-godot
godot --headless --quit --path game/ 2>&1 | tail -10
git add game/scripts/main.gd game/scenes/main.tscn
git commit -m "feat(game): per-module glyph layers + velocity-oriented bodies"
```

---

## Task 5: Interactive smoke test + tag

- [ ] **Step 5.1: Headless check**

```bash
cargo build -p anabios-godot --release
godot --headless --quit --path game/ 2>&1 | tail -10
```

Expected: no fatal errors.

- [ ] **Step 5.2: Interactive smoke test** (local)

Open `game/project.godot`, press F5. Expected:
- Biome terrain visible (blue water, green grass/forest, tan desert, grey rock), brightening where plants are dense
- Agents as colored discs oriented in their travel direction
- Small colored glyphs orbiting each agent (one per module)
- Everything from M6 still works (pause/speed/inspect/ticker)

Fix anything broken before tagging; report what.

- [ ] **Step 5.3: Workspace check + tag**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git tag -a m7 -m "M7: full rendering (biome background, module glyphs, oriented bodies)"
```

Do NOT push — controller handles that.

---

## Post-implementation expectations

- The viewer shows a legible biome with live plant density
- Agents are oriented by velocity with per-module glyphs revealing body plans
- M6 interactions all still work
- Determinism untouched (rendering-only changes to the gdext read path)

Deferred to M8+:
- Pheromone trail buffer (needs the pheromone field, a later sim milestone)
- Overlay toggles (biome heatmap modes, territory/lineage/trait paint)
- Camera follow modes
- Animated locomotor sprite atlases
- Per-pixel biome update optimization (`Image.set_data` from a packed byte buffer)
