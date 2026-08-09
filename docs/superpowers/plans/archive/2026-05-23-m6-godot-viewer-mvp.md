# M6 — Godot Viewer MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wrap `anabios-core` as a Godot extension (gdext) and stand up a minimal Godot 4.6 project where you can press play and watch agents move on screen. MVP = play/pause + speed control, click-to-inspect one agent, and a scrolling codex event ticker. No module sprites, no overlays, no camera follow modes, no codex chapter UI yet (those land in M7-M9).

**Architecture:** A new workspace member `anabios-godot` (cdylib) exposes a `Simulation` GodotClass extending `Node`. The Godot project `game/` hosts a main scene with a `Camera2D`, the `Simulation` node, a `MultiMeshInstance2D` for body rendering, and a Control overlay for UI. Each `_process(delta)` frame, GDScript ticks the sim N times based on speed setting, then reads `PackedVector2Array` of positions + `PackedColorArray` of colors and pushes them into the MultiMesh instance buffers. Click handling raycasts against agent positions; the inspector panel reads `get_agent_info(id) -> Dictionary`.

**Tech Stack:**
- Rust stable + gdext (godot crate v0.2 or current stable, whichever matches Godot 4.6)
- Godot 4.6+ (confirmed installed locally at `/opt/homebrew/bin/godot`, version `4.6.3.stable.official.7d41c59c4`)
- GDScript for UI
- macOS arm64 cdylib build for local dev; CI cross-compile in M7+

**Branch:** `m6-godot-viewer-mvp` branched from `main`.

**Working directory:** `/Users/aryasen/projects/anabios/`.

---

## File structure after M6

New files:
```
crates/anabios-godot/
├── Cargo.toml
└── src/
    └── lib.rs                            # Simulation GodotClass + ExtensionLibrary

game/
├── project.godot                         # Godot 4.6 project manifest
├── anabios.gdextension                   # points to the cdylib
├── default_env.tres                      # default environment
├── icon.svg                              # placeholder icon
├── scenes/
│   ├── main.tscn                         # root scene: Camera + Sim + MultiMesh + UI
│   └── inspector_panel.tscn              # the click-to-inspect panel
├── scripts/
│   ├── main.gd                           # tick scheduling + MultiMesh population
│   ├── camera_controller.gd              # pan + zoom
│   ├── time_controls.gd                  # play/pause/speed buttons
│   ├── inspector_panel.gd                # populates panel from get_agent_info
│   └── event_ticker.gd                   # codex event scrolling list
└── resources/
    └── body_multimesh.tres               # MultiMesh resource (mesh = circle, max ~10k)
```

Modified files:
```
Cargo.toml                                # add crates/anabios-godot member; cdylib dep
README.md                                 # add "Open game/ in Godot 4.6+" instructions
.gitignore                                # ignore .godot/, target dylibs
```

---

## Task 0: Branch + verify Godot version

- [ ] **Step 0.1:** `git checkout -b m6-godot-viewer-mvp`
- [ ] **Step 0.2:** `godot --version` should print `4.6.x.stable.official...` — if not, install Godot 4.6 from godotengine.org before proceeding.
- [ ] **Step 0.3:** `cargo test --workspace 2>&1 | tail -3` → all pass (baseline ~108 tests).

No commit.

---

## Task 1: anabios-godot crate skeleton

**Goal:** New cdylib workspace member with gdext deps, an `ExtensionLibrary` declaration, and a placeholder `Simulation` node that compiles into a dylib.

**Files:**
- Create: `crates/anabios-godot/Cargo.toml`
- Create: `crates/anabios-godot/src/lib.rs`
- Modify: `Cargo.toml` (workspace members + gdext workspace dep)

- [ ] **Step 1.1: Add workspace member + gdext dep**

In root `Cargo.toml`:

```toml
[workspace]
members = ["crates/anabios-core", "crates/anabios-godot", "crates/anabios-headless"]

[workspace.dependencies]
# ... existing entries ...
godot = "0.2"   # gdext bindings for Godot 4.6
```

(Verify the latest `godot` crate version compatible with Godot 4.6 at https://crates.io/crates/godot before pinning.)

- [ ] **Step 1.2: Create the crate manifest**

`crates/anabios-godot/Cargo.toml`:

```toml
[package]
name = "anabios-godot"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Godot 4.6 extension exposing anabios-core as a Simulation node."

[lib]
crate-type = ["cdylib"]

[dependencies]
anabios-core = { path = "../anabios-core" }
godot = { workspace = true }
```

- [ ] **Step 1.3: Implement the skeleton lib**

`crates/anabios-godot/src/lib.rs`:

```rust
//! anabios-godot — Godot 4.6 extension binding for anabios-core.
//!
//! Exposes a single `Simulation` node class that GDScript can construct,
//! advance with `step()`, and query for per-tick agent buffers + codex
//! events. UI logic lives in GDScript; this crate is purely the bridge.

use godot::prelude::*;

struct AnabiosExtension;

#[gdextension]
unsafe impl ExtensionLibrary for AnabiosExtension {}

/// One in-process anabios simulation, owned by a Godot `Simulation` node.
#[derive(GodotClass)]
#[class(base = Node)]
pub struct Simulation {
    base: Base<Node>,
    inner: Option<anabios_core::World>,
}

#[godot_api]
impl INode for Simulation {
    fn init(base: Base<Node>) -> Self {
        Self { base, inner: None }
    }
}

#[godot_api]
impl Simulation {
    /// Construct a new world from a seed. Idempotent — calling again resets.
    #[func]
    fn new_world(&mut self, seed: i64) {
        self.inner = Some(anabios_core::World::new(seed as u64));
    }

    /// Load a TOML scenario (passed as a Godot string). Returns true on
    /// success; false (and logs an error) on parse failure.
    #[func]
    fn load_scenario(&mut self, toml_text: GString) -> bool {
        let text = String::from(toml_text);
        match anabios_core::Scenario::parse_toml(&text) {
            Ok(s) => {
                self.inner = Some(s.instantiate());
                true
            }
            Err(e) => {
                godot_error!("scenario parse failed: {e}");
                false
            }
        }
    }

    /// Advance the simulation by N ticks.
    #[func]
    fn step_n(&mut self, n: i64) {
        if let Some(w) = self.inner.as_mut() {
            for _ in 0..n.max(0) {
                anabios_core::tick::step(w);
            }
        }
    }

    /// Current tick number.
    #[func]
    fn tick(&self) -> i64 {
        self.inner.as_ref().map(|w| w.tick as i64).unwrap_or(0)
    }

    /// Number of alive agents.
    #[func]
    fn alive_count(&self) -> i64 {
        self.inner.as_ref().map(|w| w.agents.live_count() as i64).unwrap_or(0)
    }
}
```

- [ ] **Step 1.4: Verify build**

```bash
cargo build -p anabios-godot 2>&1 | tail -5
```

Expected: produces `target/debug/libanabios_godot.dylib` (macOS) or `.so` / `.dll` on other platforms. **First build downloads gdext + builds the godot crate, can take 1-3 minutes.**

If gdext build fails with version mismatch errors against Godot 4.6, try `godot = "0.3"` or check the gdext compatibility matrix at https://godot-rust.github.io/.

- [ ] **Step 1.5: fmt + clippy + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml crates/anabios-godot/
git commit -m "feat(godot): anabios-godot cdylib crate with Simulation node skeleton"
```

Expected: workspace cargo test still passes; new crate compiles. (Clippy may flag base-field-not-used warnings on the `Simulation` struct — silence with `#[allow(dead_code)]` on the `base` field if needed; gdext requires it.)

---

## Task 2: Expose agent buffers as Packed arrays

**Goal:** Add `alive_positions() -> PackedVector2Array`, `alive_colors() -> PackedColorArray`, and `alive_count_per_species() -> PackedInt32Array`. These are the inputs GDScript needs to populate a `MultiMesh`.

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs`

- [ ] **Step 2.1: Add buffer accessors**

Append to the `#[godot_api] impl Simulation` block:

```rust
    /// Return alive-agent positions as a Vector2 array, indexed by raw
    /// iteration order (ascending agent id). Dead agents are skipped.
    #[func]
    fn alive_positions(&self) -> PackedVector2Array {
        let mut out = PackedVector2Array::new();
        if let Some(w) = self.inner.as_ref() {
            out.resize(w.agents.live_count() as usize);
            for (i, id) in w.agents.iter_alive().enumerate() {
                let p = w.agents.position[id as usize];
                out.set(i, Vector2::new(p.x, p.y));
            }
        }
        out
    }

    /// Return alive-agent colors derived from genome color slots, in the
    /// same order as `alive_positions`. Caller can pipe directly into a
    /// MultiMesh's instance-color buffer.
    #[func]
    fn alive_colors(&self) -> PackedColorArray {
        use anabios_core::genome::GenomeSlot;
        let mut out = PackedColorArray::new();
        if let Some(w) = self.inner.as_ref() {
            out.resize(w.agents.live_count() as usize);
            for (i, id) in w.agents.iter_alive().enumerate() {
                let g = &w.agents.genome[id as usize];
                let h = g.get(GenomeSlot::ColorHue);
                let s = g.get(GenomeSlot::ColorSat).clamp(0.4, 1.0);
                let v = g.get(GenomeSlot::ColorVal).clamp(0.5, 1.0);
                let c = hsv_to_color(h, s, v);
                out.set(i, c);
            }
        }
        out
    }

    /// Each agent's size in world units (used by MultiMesh scale).
    #[func]
    fn alive_sizes(&self) -> PackedFloat32Array {
        use anabios_core::genome::GenomeSlot;
        let mut out = PackedFloat32Array::new();
        if let Some(w) = self.inner.as_ref() {
            out.resize(w.agents.live_count() as usize);
            for (i, id) in w.agents.iter_alive().enumerate() {
                let g = &w.agents.genome[id as usize];
                let s = 0.5 + 2.5 * g.get(GenomeSlot::Size); // 0.5..3.0 units
                out.set(i, s);
            }
        }
        out
    }

    /// World extent (a square). UI uses this to set camera limits.
    #[func]
    fn world_size(&self) -> f32 {
        anabios_core::biome::WORLD_SIZE
    }
}

fn hsv_to_color(h: f32, s: f32, v: f32) -> Color {
    let h6 = (h % 1.0) * 6.0;
    let i = h6.floor() as i32;
    let f = h6 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::from_rgb(r, g, b)
}
```

- [ ] **Step 2.2: Build + fmt + clippy + commit**

```bash
cargo build -p anabios-godot
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-godot/src/lib.rs
git commit -m "feat(godot): expose alive_positions/colors/sizes packed arrays + world_size"
```

---

## Task 3: Expose inspector + codex queries

**Goal:** Add `get_agent_info(id) -> Dictionary` and `take_codex_events() -> Array` (drains events to GDScript).

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs`

- [ ] **Step 3.1: Add introspection + event drain**

Append to the impl block:

```rust
    /// Look up one alive agent by id. Returns a Dictionary with keys:
    ///   id, position, energy, age, genome (PackedFloat32Array),
    ///   lineage_id, species_id, program_len, module_count, alive
    /// Returns an empty dict if the id is not alive.
    #[func]
    fn get_agent_info(&self, id: i64) -> Dictionary {
        let mut d = Dictionary::new();
        let Some(w) = self.inner.as_ref() else { return d };
        let aid = id as u32;
        if !w.agents.is_alive(aid) {
            d.set("alive", false);
            return d;
        }
        let i = id as usize;
        let p = w.agents.position[i];
        d.set("id", id);
        d.set("alive", true);
        d.set("position", Vector2::new(p.x, p.y));
        d.set("energy", w.agents.energy[i]);
        d.set("age", w.agents.age[i] as i64);
        d.set("lineage_id", w.agents.lineage_id[i] as i64);
        d.set("species_id", w.agents.species_id[i] as i64);
        d.set("program_len", w.agents.program[i].len() as i64);
        d.set("module_count", w.agents.modules[i].len() as i64);
        let mut g = PackedFloat32Array::new();
        for v in w.agents.genome[i].0.iter() {
            g.push(*v);
        }
        d.set("genome", g);
        d
    }

    /// Drain the codex event buffer. Each event becomes a Dictionary:
    ///   { type: int (0=Extinction, 1=PopulationCrash, 2=SpeciationEvent),
    ///     tick: int, species_id: int, value: f32 }
    #[func]
    fn take_codex_events(&mut self) -> Array<Dictionary> {
        let mut out = Array::<Dictionary>::new();
        let Some(w) = self.inner.as_mut() else { return out };
        for ev in w.codex.drain_events() {
            let mut d = Dictionary::new();
            d.set("type", ev.event_type as u8 as i64);
            d.set("tick", ev.tick as i64);
            d.set("species_id", ev.species_id as i64);
            d.set("value", ev.value);
            out.push(d);
        }
        out
    }

    /// Find the closest alive agent to a world position, within `radius`
    /// world units. Returns the agent id, or -1 if no agent in range.
    /// Used by click-to-inspect.
    #[func]
    fn agent_near(&self, pos: Vector2, radius: f32) -> i64 {
        let Some(w) = self.inner.as_ref() else { return -1 };
        let p = glam::Vec2::new(pos.x, pos.y);
        let mut best_id: i64 = -1;
        let mut best_d = radius;
        for id in w.agents.iter_alive() {
            let q = w.agents.position[id as usize];
            let d = (q - p).length();
            if d < best_d {
                best_d = d;
                best_id = id as i64;
            }
        }
        best_id
    }
```

- [ ] **Step 3.2: Build + commit**

```bash
cargo build -p anabios-godot
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-godot/src/lib.rs
git commit -m "feat(godot): inspector dict, codex event drain, agent_near for click-to-inspect"
```

---

## Task 4: Godot project skeleton

**Goal:** Create the `game/` directory with `project.godot`, `.gdextension`, scenes, and scripts. Open in Godot once to let it generate `.godot/` metadata.

**Files:**
- Create: `game/project.godot`
- Create: `game/anabios.gdextension`
- Create: `game/icon.svg`
- Modify: `.gitignore` (add `.godot/`, `*.dylib`, `*.so`, `*.dll`)
- Modify: `README.md` (add Godot launch instructions)

- [ ] **Step 4.1: Add gitignore entries**

Append to `.gitignore`:

```
# Godot
.godot/
.import/
game/export.cfg
game/.editor/

# Native libraries built for gdext consumption
target/debug/libanabios_godot.*
target/release/libanabios_godot.*
```

- [ ] **Step 4.2: Write project.godot**

`game/project.godot`:

```ini
; Engine configuration file.
; Open in Godot 4.6+; do not edit manually.

config_version=5

[application]

config/name="anabios"
run/main_scene="res://scenes/main.tscn"
config/features=PackedStringArray("4.6", "Forward Plus")

[display]

window/size/viewport_width=1280
window/size/viewport_height=800
window/stretch/mode="canvas_items"

[input]

ui_accept={
"deadzone": 0.5,
"events": [Object(InputEventKey,"resource_local_to_scene":false,"resource_name":"","device":-1,"window_id":0,"alt_pressed":false,"shift_pressed":false,"ctrl_pressed":false,"meta_pressed":false,"pressed":false,"keycode":32,"physical_keycode":0,"key_label":0,"unicode":32,"location":0,"echo":false,"script":null)
]
}
```

- [ ] **Step 4.3: Write the .gdextension manifest**

`game/anabios.gdextension`:

```ini
[configuration]

entry_symbol = "gdext_rust_init"
compatibility_minimum = 4.2
reloadable = true

[libraries]

macos.debug    = "res://../target/debug/libanabios_godot.dylib"
macos.release  = "res://../target/release/libanabios_godot.dylib"
linux.debug.x86_64    = "res://../target/debug/libanabios_godot.so"
linux.release.x86_64  = "res://../target/release/libanabios_godot.so"
windows.debug.x86_64   = "res://../target/debug/anabios_godot.dll"
windows.release.x86_64 = "res://../target/release/anabios_godot.dll"
```

- [ ] **Step 4.4: Placeholder icon**

`game/icon.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">
  <rect width="128" height="128" fill="#1d2330"/>
  <circle cx="64" cy="64" r="36" fill="#7ec27e"/>
  <text x="50%" y="55%" font-size="40" fill="#0f172a" text-anchor="middle" font-family="sans-serif">a</text>
</svg>
```

- [ ] **Step 4.5: Update README**

Append to `README.md`:

```markdown
## Running the viewer

1. Build the gdext cdylib:
   ```bash
   cargo build -p anabios-godot
   ```
2. Open `game/project.godot` in Godot 4.6+.
3. Press F5 to run the main scene. Use space to play/pause; arrow keys / WASD to pan; mouse wheel to zoom.
```

- [ ] **Step 4.6: Commit skeleton**

```bash
git add .gitignore README.md game/project.godot game/anabios.gdextension game/icon.svg
git commit -m "chore(game): Godot 4.6 project skeleton + gdextension manifest"
```

- [ ] **Step 4.7: Sanity-launch Godot (local-only)**

Run: `godot --headless --quit --path game/ 2>&1 | tail -5`

Expected: Godot imports the project and exits cleanly. If it complains about the missing dylib, that's OK at this point — we just want it to read the project files without crashing.

---

## Task 5: Main scene + MultiMesh body rendering

**Goal:** A scene with `Camera2D + Simulation + MultiMeshInstance2D + UI overlay` that ticks the sim and pushes positions/colors/scales into the MultiMesh each frame.

**Files:**
- Create: `game/scenes/main.tscn`
- Create: `game/scripts/main.gd`
- Create: `game/resources/body_multimesh.tres`

- [ ] **Step 5.1: Create MultiMesh resource**

`game/resources/body_multimesh.tres`:

```tres
[gd_resource type="MultiMesh" load_steps=2 format=3]

[sub_resource type="QuadMesh" id="QuadMesh_body"]
size = Vector2(1, 1)

[resource]
transform_format = 1
use_colors = true
use_custom_data = false
instance_count = 10000
visible_instance_count = 0
mesh = SubResource("QuadMesh_body")
```

- [ ] **Step 5.2: Write the main.tscn**

`game/scenes/main.tscn`:

```tres
[gd_scene load_steps=4 format=3]

[ext_resource type="Script" path="res://scripts/main.gd" id="1_main"]
[ext_resource type="MultiMesh" path="res://resources/body_multimesh.tres" id="2_mm"]

[node name="Main" type="Node2D"]
script = ExtResource("1_main")

[node name="Camera2D" type="Camera2D" parent="."]
position = Vector2(512, 512)
zoom = Vector2(1, 1)

[node name="Simulation" type="Simulation" parent="."]

[node name="Bodies" type="MultiMeshInstance2D" parent="."]
multimesh = ExtResource("2_mm")

[node name="UI" type="CanvasLayer" parent="."]

[node name="HUD" type="Label" parent="UI"]
offset_left = 10.0
offset_top = 10.0
offset_right = 400.0
offset_bottom = 60.0
text = "tick=0 alive=0"
theme_override_colors/font_color = Color(0.95, 0.95, 0.95, 1)
theme_override_font_sizes/font_size = 16
```

- [ ] **Step 5.3: Write main.gd**

`game/scripts/main.gd`:

```gdscript
extends Node2D

# Number of sim ticks to run per rendered frame. Speeds: 1, 4, 16, 64.
@export var ticks_per_frame: int = 1
@export var paused: bool = false

@onready var sim: Simulation = $Simulation
@onready var bodies: MultiMeshInstance2D = $Bodies
@onready var hud: Label = $UI/HUD

func _ready() -> void:
    # Load the minimal scenario shipped in the workspace root.
    var scenario_path = "res://../scenarios/minimal.toml"
    var f = FileAccess.open(scenario_path, FileAccess.READ)
    if f == null:
        push_error("could not open " + scenario_path)
        return
    var text = f.get_as_text()
    f.close()
    if not sim.load_scenario(text):
        push_error("scenario load failed")
        return

func _process(_delta: float) -> void:
    if not paused:
        sim.step_n(ticks_per_frame)
    _refresh_bodies()
    hud.text = "tick=%d alive=%d" % [sim.tick(), sim.alive_count()]

func _refresh_bodies() -> void:
    var n: int = int(sim.alive_count())
    var mm: MultiMesh = bodies.multimesh
    if n > mm.instance_count:
        mm.instance_count = n
    mm.visible_instance_count = n

    var positions: PackedVector2Array = sim.alive_positions()
    var colors: PackedColorArray = sim.alive_colors()
    var sizes: PackedFloat32Array = sim.alive_sizes()
    for i in n:
        var t: Transform2D = Transform2D(0.0, Vector2(sizes[i], sizes[i]), 0.0, positions[i])
        mm.set_instance_transform_2d(i, t)
        mm.set_instance_color(i, colors[i])
```

- [ ] **Step 5.4: Smoke test in Godot**

```bash
cargo build -p anabios-godot
godot --headless --quit --path game/ 2>&1 | tail -5
```

The headless run should not crash. To actually see the viewer, the user opens `game/project.godot` in Godot's editor and presses F5.

- [ ] **Step 5.5: Commit**

```bash
git add game/scenes/main.tscn game/scripts/main.gd game/resources/body_multimesh.tres
git commit -m "feat(game): main scene with MultiMesh body rendering driven by Simulation node"
```

---

## Task 6: Camera controller (pan + zoom)

**Goal:** Drag-pan with middle mouse, zoom with scroll wheel, arrow-key panning fallback.

**Files:**
- Create: `game/scripts/camera_controller.gd`
- Modify: `game/scenes/main.tscn` (attach script to Camera2D)

- [ ] **Step 6.1: camera_controller.gd**

```gdscript
extends Camera2D

const ZOOM_STEP: float = 1.2
const ZOOM_MIN: float = 0.25
const ZOOM_MAX: float = 8.0
const PAN_SPEED_KEYS: float = 600.0  # pixels/sec at zoom 1

var _dragging: bool = false

func _input(event: InputEvent) -> void:
    if event is InputEventMouseButton:
        var mb := event as InputEventMouseButton
        if mb.button_index == MOUSE_BUTTON_WHEEL_UP and mb.pressed:
            zoom = (zoom * ZOOM_STEP).clamp(Vector2(ZOOM_MIN, ZOOM_MIN), Vector2(ZOOM_MAX, ZOOM_MAX))
        elif mb.button_index == MOUSE_BUTTON_WHEEL_DOWN and mb.pressed:
            zoom = (zoom / ZOOM_STEP).clamp(Vector2(ZOOM_MIN, ZOOM_MIN), Vector2(ZOOM_MAX, ZOOM_MAX))
        elif mb.button_index == MOUSE_BUTTON_MIDDLE:
            _dragging = mb.pressed
    elif event is InputEventMouseMotion and _dragging:
        var mm := event as InputEventMouseMotion
        position -= mm.relative / zoom.x

func _process(delta: float) -> void:
    var v := Vector2.ZERO
    if Input.is_key_pressed(KEY_W) or Input.is_key_pressed(KEY_UP):    v.y -= 1
    if Input.is_key_pressed(KEY_S) or Input.is_key_pressed(KEY_DOWN):  v.y += 1
    if Input.is_key_pressed(KEY_A) or Input.is_key_pressed(KEY_LEFT):  v.x -= 1
    if Input.is_key_pressed(KEY_D) or Input.is_key_pressed(KEY_RIGHT): v.x += 1
    if v != Vector2.ZERO:
        position += v.normalized() * (PAN_SPEED_KEYS * delta) / zoom.x
```

- [ ] **Step 6.2: Attach to Camera2D**

Edit `game/scenes/main.tscn`. Change the `[node name="Camera2D"]` block to include the script:

```tres
[ext_resource type="Script" path="res://scripts/camera_controller.gd" id="3_cam"]
```

And on the Camera2D node, add `script = ExtResource("3_cam")` as the first property.

- [ ] **Step 6.3: Commit**

```bash
git add game/scripts/camera_controller.gd game/scenes/main.tscn
git commit -m "feat(game): camera controller with pan + zoom"
```

---

## Task 7: Time controls (play/pause + speed)

**Goal:** Buttons in the UI overlay for pause/play and 1×/4×/16×/64× speed.

**Files:**
- Create: `game/scripts/time_controls.gd`
- Modify: `game/scenes/main.tscn` (add HBoxContainer with buttons)

- [ ] **Step 7.1: time_controls.gd**

```gdscript
extends HBoxContainer

@onready var main: Node2D = get_node("/root/Main")

func _ready() -> void:
    $PauseButton.pressed.connect(_on_pause_pressed)
    $Speed1.pressed.connect(_on_speed.bind(1))
    $Speed4.pressed.connect(_on_speed.bind(4))
    $Speed16.pressed.connect(_on_speed.bind(16))
    $Speed64.pressed.connect(_on_speed.bind(64))

func _on_pause_pressed() -> void:
    main.paused = not main.paused
    $PauseButton.text = "▶" if main.paused else "⏸"

func _on_speed(n: int) -> void:
    main.ticks_per_frame = n
```

- [ ] **Step 7.2: Update main.tscn UI block**

Add inside the `[node name="UI" type="CanvasLayer"]` block, after the HUD:

```tres
[node name="TimeControls" type="HBoxContainer" parent="UI"]
offset_left = 10.0
offset_top = 760.0
offset_right = 380.0
offset_bottom = 790.0
script = ExtResource("4_time")

[node name="PauseButton" type="Button" parent="UI/TimeControls"]
text = "⏸"

[node name="Speed1" type="Button" parent="UI/TimeControls"]
text = "1×"

[node name="Speed4" type="Button" parent="UI/TimeControls"]
text = "4×"

[node name="Speed16" type="Button" parent="UI/TimeControls"]
text = "16×"

[node name="Speed64" type="Button" parent="UI/TimeControls"]
text = "64×"
```

Add at the top of the scene file:

```tres
[ext_resource type="Script" path="res://scripts/time_controls.gd" id="4_time"]
```

- [ ] **Step 7.3: Commit**

```bash
git add game/scripts/time_controls.gd game/scenes/main.tscn
git commit -m "feat(game): play/pause + 1/4/16/64x speed controls"
```

---

## Task 8: Inspector panel (click-to-pin)

**Goal:** Click an agent to pin it in a small side panel showing energy, age, species, lineage, program/module counts.

**Files:**
- Create: `game/scripts/inspector_panel.gd`
- Modify: `game/scripts/main.gd` (route clicks to inspector)
- Modify: `game/scenes/main.tscn` (add the panel control)

- [ ] **Step 8.1: inspector_panel.gd**

```gdscript
extends PanelContainer

var pinned_id: int = -1

@onready var main: Node2D = get_node("/root/Main")
@onready var sim: Simulation = main.sim
@onready var label: Label = $VBoxContainer/Label

func pin(id: int) -> void:
    pinned_id = id
    visible = id >= 0

func _process(_delta: float) -> void:
    if pinned_id < 0:
        return
    var info: Dictionary = sim.get_agent_info(pinned_id)
    if info.is_empty() or not info.get("alive", false):
        label.text = "(agent %d is dead)" % pinned_id
        return
    label.text = (
        "id %d\n" +
        "species %d  lineage %d\n" +
        "energy %.1f  age %d\n" +
        "program %d nodes  modules %d"
    ) % [
        pinned_id, info["species_id"], info["lineage_id"],
        info["energy"], info["age"],
        info["program_len"], info["module_count"],
    ]
```

- [ ] **Step 8.2: main.gd handle clicks**

Append to `game/scripts/main.gd`:

```gdscript
@onready var inspector: PanelContainer = $UI/Inspector

func _unhandled_input(event: InputEvent) -> void:
    if event is InputEventMouseButton:
        var mb := event as InputEventMouseButton
        if mb.button_index == MOUSE_BUTTON_LEFT and mb.pressed:
            var world_pos: Vector2 = ($Camera2D as Camera2D).get_global_mouse_position()
            var hit_id: int = int(sim.agent_near(world_pos, 4.0))
            inspector.pin(hit_id)
```

- [ ] **Step 8.3: Add Inspector to main.tscn UI**

```tres
[ext_resource type="Script" path="res://scripts/inspector_panel.gd" id="5_insp"]

[node name="Inspector" type="PanelContainer" parent="UI"]
visible = false
offset_left = 1050.0
offset_top = 10.0
offset_right = 1270.0
offset_bottom = 200.0
script = ExtResource("5_insp")

[node name="VBoxContainer" type="VBoxContainer" parent="UI/Inspector"]
anchors_preset = 15
anchor_right = 1.0
anchor_bottom = 1.0
offset_left = 10.0
offset_top = 10.0
offset_right = -10.0
offset_bottom = -10.0

[node name="Label" type="Label" parent="UI/Inspector/VBoxContainer"]
text = "—"
theme_override_font_sizes/font_size = 14
```

- [ ] **Step 8.4: Commit**

```bash
git add game/scripts/inspector_panel.gd game/scripts/main.gd game/scenes/main.tscn
git commit -m "feat(game): click-to-pin inspector panel"
```

---

## Task 9: Codex event ticker

**Goal:** A scrolling list at the bottom of the screen showing the last N codex events (extinction / crash / speciation), updated every frame.

**Files:**
- Create: `game/scripts/event_ticker.gd`
- Modify: `game/scenes/main.tscn` (add ScrollContainer + Label)

- [ ] **Step 9.1: event_ticker.gd**

```gdscript
extends RichTextLabel

const MAX_LINES: int = 80
const TYPE_NAMES: PackedStringArray = ["Extinction", "PopCrash", "Speciation"]

var _lines: Array[String] = []

@onready var sim: Simulation = get_node("/root/Main/Simulation")

func _process(_delta: float) -> void:
    var events: Array = sim.take_codex_events()
    if events.is_empty():
        return
    for ev in events:
        var name: String = TYPE_NAMES[int(ev["type"])] if int(ev["type"]) < TYPE_NAMES.size() else "Event"
        _lines.append("t=%d %s species=%d value=%.2f" % [
            ev["tick"], name, ev["species_id"], ev["value"]
        ])
        while _lines.size() > MAX_LINES:
            _lines.pop_front()
    text = "\n".join(_lines)
    scroll_to_line(get_line_count() - 1)
```

- [ ] **Step 9.2: Add to main.tscn UI**

```tres
[ext_resource type="Script" path="res://scripts/event_ticker.gd" id="6_tick"]

[node name="EventTicker" type="RichTextLabel" parent="UI"]
offset_left = 400.0
offset_top = 720.0
offset_right = 1270.0
offset_bottom = 790.0
bbcode_enabled = false
scroll_following = true
theme_override_colors/default_color = Color(0.8, 0.85, 0.9, 1)
theme_override_font_sizes/normal_font_size = 12
script = ExtResource("6_tick")
```

- [ ] **Step 9.3: Commit**

```bash
git add game/scripts/event_ticker.gd game/scenes/main.tscn
git commit -m "feat(game): scrolling codex event ticker"
```

---

## Task 10: Smoke test + final + tag

- [ ] **Step 10.1: Headless project check**

```bash
cargo build -p anabios-godot --release
godot --headless --quit --path game/ 2>&1 | tail -10
```

Expected: no fatal errors. (Warnings about unused resources or autoload are OK.)

- [ ] **Step 10.2: Interactive smoke test** (run locally; not in CI)

Open `game/project.godot` in Godot, press F5. Expected:
- Window opens at 1280×800
- Roughly 200 colored discs visible on screen, moving
- HUD shows `tick=N alive=M` updating
- Pressing pause works
- 4× / 16× / 64× speeds visibly accelerate the sim
- Middle-drag pans; scroll wheels zoom
- Clicking near an agent fills the inspector panel
- Codex events scroll past in the bottom-right ticker

If something is broken, fix it before tagging. Report what was wrong + how it was fixed.

- [ ] **Step 10.3: Workspace check + tag**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git tag -a m6 -m "M6: Godot viewer MVP (play, pause, inspect, codex ticker)"
```

Do NOT push branch/tag — controller handles that.

---

## Post-implementation expectations

After M6 merges:
- `cargo build -p anabios-godot` produces a working gdext cdylib on macOS arm64
- Opening `game/project.godot` and pressing F5 shows the simulation live
- Time controls, pan/zoom, click-to-inspect, and codex event ticker all functional
- README documents how to run the viewer
- 200 agents from `scenarios/minimal.toml` are visible and ticking

Deferred to M7+:
- Per-module sprite layers (Sensor, Mouth, Weapon, etc.)
- Trail buffers (pheromone visualization, when pheromones land)
- Overlays (biome heatmap, species territories, lineage paint, trait paint)
- Camera modes (Follow Individual, Follow Species, Event Camera)
- Full codex chapter UI (this milestone just has a ticker; replay system is later)
- Scenario authoring UI
- Windows and Linux cdylib in CI

## Known risks

- **gdext version skew with Godot 4.6.** If `godot = "0.2"` doesn't compile against Godot 4.6.3, try the next minor (0.3 or 0.4 — check crates.io). Plan author developed against 4.6.3.stable.official; the API may have shifted slightly. If the implementer hits a compile error on `INode for Simulation` or `GodotClass` derives, consult the gdext docs at <https://godot-rust.github.io/> for the current pattern and adjust Task 1's code accordingly. Document the change in the task report.
- **GDScript autocomplete won't see `Simulation` class** until the editor has loaded the .gdextension at least once. After Task 4 step 4.7's headless launch, opening the editor will index it.
- **Click-to-inspect uses brute-force scan** (`agent_near` walks all alive agents per click). Fine for 2k; for larger populations, route through the spatial hash later.
- **HUD positioning is hardcoded** for a 1280×800 viewport. Resizing the window will not reflow — that's M7 work.
