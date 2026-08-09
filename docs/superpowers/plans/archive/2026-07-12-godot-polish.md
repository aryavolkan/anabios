# Godot Sandbox Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every anabios simulation mechanic visible and every scenario reachable in the Godot sandbox, by adding read-only view exports plus a layered overlay/panel system — with zero change to simulation determinism.

**Architecture:** Add read-only `#[func]` accessors to the existing `Simulation` node (`crates/anabios-godot/src/lib.rs`), then a GDScript overlay manager driving two hotkey-cycled display modes (ground layer + body color), plus population/DIT/legend panels and a 14-scenario menu. No new `World` fields, no sim-logic changes.

**Tech Stack:** Rust (gdext / godot 4.6 bindings), GDScript, anabios-core.

## Global Constraints

- **Determinism invariant (load-bearing):** Every Rust addition is a read-only accessor. No new `World` fields, no mutation of sim state, no change to any `tick::step` path. After every Rust task, `cargo test -p anabios-core --test determinism` MUST pass with byte-identical golden hashes (currently `[(0, 0x58807132956798b1), (100, 0xa020c143eccfb4eb), (1000, 0xfd21efef4e1619e4)]`). If a change alters a hash, it is wrong — revert.
- **CI-accurate gate (run before each Rust commit):** the CI toolchain differs from local default, so use `rustup run stable`:
  - `rustup run stable cargo fmt --all --check` (COMMIT fmt output — CI checks the committed tree)
  - `rustup run stable env RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets`
  - `rustup run stable env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
  - In doc comments, escape `` `[0,1]` `` and `` `[N]` `` in backticks or rustdoc treats them as broken intra-doc links (`-D warnings` fails the build).
- **Godot:** engine 4.6; scenario paths are `res://../scenarios/<name>.toml`; no headless render-test runner exists, so GDScript tasks are verified by launching the project and confirming no script errors in Godot's Output panel.
- **Branch:** `godot-polish` (already created). **Commits** end with:
  `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`
- **Reference constants (verified in source):** `BIOME_RES` grid is row-major `row*RES+col`; `PHEROMONE_CHANNELS = 4`; `MEME_CHANNELS = 8`; `SKILL_CHANNEL = 5`; `TECH_CHANNEL = 6`; genome slots `IndividualLearning = 28`, `SocialLearning = 29`, `InnateTechnique = 40`; `ModuleType` 0..8 = Locomotor, Sensor, Mouth, Weapon, Armor, Storage, Communicator, Pheromone, Reproductive.

---

## File Structure

**Rust (modify):**
- `crates/anabios-godot/src/lib.rs` — add read-only `#[func]` exports + two pure helper fns (`phero_intensity`, `dialect_hue`) with unit tests.

**GDScript (create):**
- `game/scripts/overlay_manager.gd` — GroundMode/BodyMode state + hotkey cycling.
- `game/scripts/population_panel.gd` — per-species live bars.
- `game/scripts/dit_panel.gd` — env-optimum + per-species technique-match (env scenarios only).
- `game/scripts/legend_panel.gd` — active modes + keybind help.

**GDScript (modify):**
- `game/scripts/menu.gd` — full 14-scenario list + defaults.
- `game/scripts/game_config.gd` — carry default ground/body mode.
- `game/scripts/biome_renderer.gd` — read GroundMode; upload biome/pheromone/optimum.
- `game/scripts/main.gd` — body color modes; carcass + combat-flash sprites; wire overlay manager.
- `game/scripts/inspector_panel.gd` — render `agent_detail`.

**Scene (modify):**
- `game/scenes/main.tscn` — add OverlayManager node + Population/DIT/Legend panels + carcass/flash MultiMeshInstance2D.

---

## Phase P1 — Scenario breadth

### Task 1: Full scenario menu + default-mode plumbing

**Files:**
- Modify: `game/scripts/game_config.gd`
- Modify: `game/scripts/menu.gd`

**Interfaces:**
- Produces: `GameConfig.default_ground: int`, `GameConfig.default_body: int` (ints matching the enums defined in Task 7; until then they are inert integers — `0` = BIOME / SPECIES).

- [ ] **Step 1: Extend GameConfig with default-mode fields**

Replace the contents of `game/scripts/game_config.gd` with:

```gdscript
extends Node

# Set by the menu, read by main.tscn on load.
var scenario_path: String = "res://../scenarios/minimal.toml"
var seed: int = 0
var ui_scale: float = 1.0
# Default display modes for the chosen scenario (see overlay_manager.gd enums).
# 0 = BIOME / SPECIES until the overlay system lands in P3.
var default_ground: int = 0
var default_body: int = 0
```

- [ ] **Step 2: Rewrite the menu scenario list**

Replace the `SCENARIOS` const and `_on_start` in `game/scripts/menu.gd`. Mode integers use the P3 enums (Ground: 0 BIOME, 1..4 PHEROMONE_0..3, 5 ENV_OPTIMUM; Body: 0 SPECIES, 1 DIALECT, 2 DIET, 3 ENERGY):

```gdscript
const SCENARIOS: Array[Dictionary] = [
	# Foundations
	{ "label": "Foundations — Minimal (200 herbivores)", "path": "res://../scenarios/minimal.toml", "ground": 0, "body": 0 },
	{ "label": "Foundations — Divergent (two founders)", "path": "res://../scenarios/divergent.toml", "ground": 0, "body": 0 },
	# Milestones
	{ "label": "M12 — Predator / prey", "path": "res://../scenarios/predator-prey.toml", "ground": 0, "body": 2 },
	{ "label": "M13 — Territories (pheromones)", "path": "res://../scenarios/territories.toml", "ground": 1, "body": 0 },
	{ "label": "M14 — Dialects (memes)", "path": "res://../scenarios/dialects.toml", "ground": 1, "body": 1 },
	{ "label": "M15 — Cooperation & kin", "path": "res://../scenarios/cooperation.toml", "ground": 0, "body": 0 },
	{ "label": "Gene–culture (baseline)", "path": "res://../scenarios/gene-culture.toml", "ground": 0, "body": 1 },
	{ "label": "Gene–culture — Skill", "path": "res://../scenarios/gene-culture-skill.toml", "ground": 0, "body": 1 },
	{ "label": "Gene–culture — Hunt", "path": "res://../scenarios/gene-culture-hunt.toml", "ground": 0, "body": 2 },
	{ "label": "Gene–culture — Alarm", "path": "res://../scenarios/gene-culture-alarm.toml", "ground": 1, "body": 1 },
	# DIT boundary
	{ "label": "DIT — Env slow (culture tracks)", "path": "res://../scenarios/dit-env-slow.toml", "ground": 5, "body": 1 },
	{ "label": "DIT — Env fast (culture stale)", "path": "res://../scenarios/dit-env-fast.toml", "ground": 5, "body": 1 },
	{ "label": "DIT — Env static (culture redundant)", "path": "res://../scenarios/dit-env-static.toml", "ground": 5, "body": 1 },
	{ "label": "DIT — Rogers (imitators invade)", "path": "res://../scenarios/dit-rogers.toml", "ground": 5, "body": 1 },
]

@onready var scenario_pick: OptionButton = $VBox/ScenarioPick
@onready var seed_spin: SpinBox = $VBox/SeedRow/SeedSpin
@onready var scale_spin: SpinBox = $VBox/ScaleRow/ScaleSpin
@onready var start_btn: Button = $VBox/StartButton

func _ready() -> void:
	for s in SCENARIOS:
		scenario_pick.add_item(s["label"])
	seed_spin.value = GameConfig.seed
	scale_spin.value = GameConfig.ui_scale
	start_btn.pressed.connect(_on_start)

func _on_start() -> void:
	var idx: int = scenario_pick.selected
	if idx < 0:
		idx = 0
	var s: Dictionary = SCENARIOS[idx]
	GameConfig.scenario_path = s["path"]
	GameConfig.seed = int(seed_spin.value)
	GameConfig.ui_scale = scale_spin.value
	GameConfig.default_ground = int(s["ground"])
	GameConfig.default_body = int(s["body"])
	get_tree().change_scene_to_file("res://scenes/main.tscn")
```

- [ ] **Step 3: Verify in Godot**

Run: open `game/project.godot` in Godot 4.6 and press Play (F5). The menu dropdown lists all 14 scenarios in three groups.
Expected: no errors in the Output panel; selecting any entry and pressing Start loads `main.tscn` and runs (modes are still inert — that is expected until P3).

- [ ] **Step 4: Commit**

```bash
git add game/scripts/game_config.gd game/scripts/menu.gd
git commit -m "$(printf 'feat(godot): reachable menu for all 14 scenarios + default-mode plumbing\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

## Phase P2 — Read-only Rust exports

### Task 2: Pheromone exports + intensity helper

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs`

**Interfaces:**
- Produces: `Simulation.pheromone_channel_count() -> i64`; `Simulation.pheromone_colors(channel: i64) -> PackedColorArray` (`BIOME_RES²`, row-major); free fn `phero_intensity(f32) -> f32` in `[0,1]`.

- [ ] **Step 1: Write the failing unit test for the intensity ramp**

Add to the bottom of `crates/anabios-godot/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phero_intensity_saturates_monotonically() {
        assert_eq!(phero_intensity(0.0), 0.0);
        assert!(phero_intensity(1.0) > phero_intensity(0.1));
        assert!(phero_intensity(100.0) <= 1.0);
        assert!(phero_intensity(-5.0) == 0.0);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `rustup run stable cargo test -p anabios-godot phero_intensity`
Expected: FAIL — `cannot find function 'phero_intensity'`.

- [ ] **Step 3: Implement the helper and the two exports**

Add the helper near `hsv_to_color` (bottom of file):

```rust
/// Map a raw pheromone concentration to a saturating intensity in `[0,1]`
/// (deposits are unbounded; decay is slow, so a plain clamp would wash out).
fn phero_intensity(v: f32) -> f32 {
    let x = v.max(0.0);
    1.0 - (-x).exp()
}
```

Add inside `impl Simulation` (after `biome_colors`):

```rust
    /// Number of pheromone channels (for the overlay cycling loop).
    #[func]
    fn pheromone_channel_count(&self) -> i64 {
        anabios_core::program::PHEROMONE_CHANNELS as i64
    }

    /// One color per pheromone cell on `channel`, row-major (`row * RES + col`),
    /// as a dark to hot ramp with alpha proportional to intensity. Returns
    /// `RES²` colors, or empty if no world is loaded or the channel is invalid.
    #[func]
    fn pheromone_colors(&self, channel: i64) -> PackedColorArray {
        let mut out = PackedColorArray::new();
        let Some(w) = self.inner.as_ref() else { return out };
        let ch = channel as usize;
        if ch >= anabios_core::program::PHEROMONE_CHANNELS {
            return out;
        }
        for cell in w.pheromones.cells.iter() {
            let t = phero_intensity(cell[ch]);
            let c = Color::from_rgb(0.05 + 0.95 * t, 0.10 * t, 0.30 * (1.0 - t));
            out.push(Color { a: t, ..c });
        }
        out
    }
```

- [ ] **Step 4: Run the unit test and the determinism gate**

Run: `rustup run stable cargo test -p anabios-godot phero_intensity`
Expected: PASS.
Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: PASS with unchanged golden hashes (read-only accessor cannot shift state).

- [ ] **Step 5: CI gate + commit**

```bash
rustup run stable cargo fmt --all
rustup run stable env RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
rustup run stable env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
git add crates/anabios-godot/src/lib.rs
git commit -m "$(printf 'feat(godot): read-only pheromone field exports\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 3: Env-optimum + species-stats exports

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs`

**Interfaces:**
- Consumes: `phero_intensity` (unused here); existing `w.agents.iter_alive()`, `w.env_period`.
- Produces: `Simulation.env_active() -> bool`; `Simulation.env_optimum() -> f32` (`[0,1]`, or `-1.0` inactive); `Simulation.species_stats() -> Array<Dictionary>` with keys `species_id: i64, count: i64, mean_energy: f32, mean_technique_match: f32`.

- [ ] **Step 1: Implement the three exports**

Add inside `impl Simulation`:

```rust
    /// True iff the DIT environment mechanism is active (`env_period > 0`).
    #[func]
    fn env_active(&self) -> bool {
        self.inner.as_ref().map(|w| w.env_period > 0).unwrap_or(false)
    }

    /// Current global optimal technique in `[0,1]`, or `-1.0` when the env
    /// mechanism is inactive.
    #[func]
    fn env_optimum(&self) -> f32 {
        match self.inner.as_ref() {
            Some(w) if w.env_period > 0 => {
                anabios_core::culture::env_optimum_at(w.tick, w.env_period)
            }
            _ => -1.0,
        }
    }

    /// Per-live-species aggregate stats. `mean_technique_match` is the mean of
    /// `technique_match(meme[TECH_CHANNEL], optimum)` when the env mechanism is
    /// active, else `0.0`.
    #[func]
    fn species_stats(&self) -> Array<Dictionary> {
        use anabios_core::culture::{env_optimum_at, technique_match, TECH_CHANNEL};
        let mut out = Array::<Dictionary>::new();
        let Some(w) = self.inner.as_ref() else { return out };
        let active = w.env_period > 0;
        let opt = if active { env_optimum_at(w.tick, w.env_period) } else { 0.0 };
        // Aggregate over live agents (indexed by species_id).
        let mut count: std::collections::BTreeMap<u32, i64> = std::collections::BTreeMap::new();
        let mut energy: std::collections::BTreeMap<u32, f32> = std::collections::BTreeMap::new();
        let mut matchsum: std::collections::BTreeMap<u32, f32> = std::collections::BTreeMap::new();
        for id in w.agents.iter_alive() {
            let i = id as usize;
            let sp = w.agents.species_id[i];
            *count.entry(sp).or_insert(0) += 1;
            *energy.entry(sp).or_insert(0.0) += w.agents.energy[i];
            if active {
                let tech = w.agents.meme_vector[i][TECH_CHANNEL];
                *matchsum.entry(sp).or_insert(0.0) += technique_match(tech, opt);
            }
        }
        for (sp, n) in count.iter() {
            let mut d = Dictionary::new();
            let nf = *n as f32;
            d.set("species_id", *sp as i64);
            d.set("count", *n);
            d.set("mean_energy", energy[sp] / nf);
            d.set("mean_technique_match", if active { matchsum[sp] / nf } else { 0.0 });
            out.push(&d);
        }
        out
    }
```

- [ ] **Step 2: Build + determinism gate**

Run: `rustup run stable cargo build -p anabios-godot`
Expected: compiles.
Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: PASS, hashes unchanged.

- [ ] **Step 3: CI gate + commit**

```bash
rustup run stable cargo fmt --all
rustup run stable env RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
rustup run stable env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
git add crates/anabios-godot/src/lib.rs
git commit -m "$(printf 'feat(godot): env-optimum + per-species stats exports\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 4: Carcass + combat-flash exports

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs`

**Interfaces:**
- Produces: `Simulation.carcass_data() -> Array<Dictionary>` keys `pos: Vector2, flesh: f32, age: i64, species_id: i64`; `Simulation.combat_flashes() -> PackedVector2Array` (positions of agents whose `combat_damaged` bit is set this tick).

- [ ] **Step 1: Implement both exports**

Add inside `impl Simulation`:

```rust
    /// One entry per carcass currently in the world.
    #[func]
    fn carcass_data(&self) -> Array<Dictionary> {
        let mut out = Array::<Dictionary>::new();
        let Some(w) = self.inner.as_ref() else { return out };
        for c in w.carcasses.iter() {
            let mut d = Dictionary::new();
            d.set("pos", Vector2::new(c.pos.x, c.pos.y));
            d.set("flesh", c.flesh);
            d.set("age", c.age as i64);
            d.set("species_id", c.species_id as i64);
            out.push(&d);
        }
        out
    }

    /// World positions of alive agents that took combat damage on the most
    /// recent tick (the flag is reset at the start of the next combat pass).
    #[func]
    fn combat_flashes(&self) -> PackedVector2Array {
        let mut out = PackedVector2Array::new();
        let Some(w) = self.inner.as_ref() else { return out };
        for id in w.agents.iter_alive() {
            let i = id as usize;
            if w.combat_damaged.get(i).copied().unwrap_or(false) {
                let p = w.agents.position[i];
                out.push(Vector2::new(p.x, p.y));
            }
        }
        out
    }
```

- [ ] **Step 2: Build + determinism gate**

Run: `rustup run stable cargo build -p anabios-godot`
Expected: compiles.
Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: PASS, hashes unchanged.

- [ ] **Step 3: CI gate + commit**

```bash
rustup run stable cargo fmt --all
rustup run stable env RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
rustup run stable env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
git add crates/anabios-godot/src/lib.rs
git commit -m "$(printf 'feat(godot): carcass + combat-flash exports\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 5: Rich agent detail + dialect-hue helper

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs`

**Interfaces:**
- Produces: free fn `dialect_hue(&[f32]) -> f32` in `[0,1)`; `Simulation.agent_detail(id: i64) -> Dictionary` — superset of `get_agent_info` plus keys `diet_carnivory: f32, skill: f32, technique: f32, indiv_learn: bool, social_learn: bool, dialect_hue: f32, module_names: PackedStringArray`.

- [ ] **Step 1: Write the failing unit test for dialect_hue**

Add to the `tests` module created in Task 2:

```rust
    #[test]
    fn dialect_hue_is_bounded_and_varies() {
        let a = [0.0_f32; 8];
        let b = [0.9_f32, 0.1, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert!((0.0..1.0).contains(&dialect_hue(&a)));
        assert!((0.0..1.0).contains(&dialect_hue(&b)));
        assert!(dialect_hue(&a) != dialect_hue(&b));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `rustup run stable cargo test -p anabios-godot dialect_hue`
Expected: FAIL — `cannot find function 'dialect_hue'`.

- [ ] **Step 3: Implement the helper**

Add near `phero_intensity`:

```rust
/// Project a meme vector onto a stable hue in `[0,1)` so divergent dialects
/// render as distinct body colors. Weighted low channels dominate.
fn dialect_hue(meme: &[f32]) -> f32 {
    let mut acc = 0.0_f32;
    for (k, v) in meme.iter().enumerate() {
        acc += v * (0.37 + 0.11 * k as f32);
    }
    acc.rem_euclid(1.0)
}
```

- [ ] **Step 4: Implement agent_detail**

Add inside `impl Simulation`:

```rust
    /// Full inspector view of one alive agent. Superset of `get_agent_info`.
    #[func]
    fn agent_detail(&self, id: i64) -> Dictionary {
        use anabios_core::culture::{SKILL_CHANNEL, TECH_CHANNEL};
        use anabios_core::genome::GenomeSlot;
        let mut d = self.get_agent_info(id);
        let Some(w) = self.inner.as_ref() else { return d };
        let aid = id as u32;
        if !w.agents.is_alive(aid) {
            return d;
        }
        let i = id as usize;
        let g = &w.agents.genome[i];
        let meme = &w.agents.meme_vector[i];
        d.set("diet_carnivory", anabios_core::module::effective_diet_carnivory(&w.agents.modules[i]));
        d.set("skill", meme[SKILL_CHANNEL]);
        d.set("technique", meme[TECH_CHANNEL]);
        d.set("indiv_learn", g.get(GenomeSlot::IndividualLearning) > 0.5);
        d.set("social_learn", g.get(GenomeSlot::SocialLearning) > 0.5);
        d.set("dialect_hue", dialect_hue(meme));
        let mut names = PackedStringArray::new();
        for m in w.agents.modules[i].iter() {
            names.push(&format!("{:?}", m.module_type()));
        }
        d.set("module_names", names);
        d
    }
```

- [ ] **Step 5: Run tests + determinism gate**

Run: `rustup run stable cargo test -p anabios-godot dialect_hue`
Expected: PASS.
Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: PASS, hashes unchanged.

- [ ] **Step 6: CI gate + commit**

```bash
rustup run stable cargo fmt --all
rustup run stable env RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
rustup run stable env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
git add crates/anabios-godot/src/lib.rs
git commit -m "$(printf 'feat(godot): rich agent_detail export (diet, memes, learning, modules)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

## Phase P3 — Overlays + body color modes

### Task 6: Overlay manager node + scene wiring

**Files:**
- Create: `game/scripts/overlay_manager.gd`
- Modify: `game/scenes/main.tscn`

**Interfaces:**
- Produces: an `OverlayManager` node at `/root/Main/OverlayManager` exposing `ground_mode: int`, `body_mode: int`, `ground_channel() -> int`, `ground_is_optimum() -> bool`, `ground_is_biome() -> bool`, and signals nothing — consumers read its vars each frame. Enum values: Ground `BIOME=0, PHEROMONE_0=1..PHEROMONE_3=4, ENV_OPTIMUM=5`; Body `SPECIES=0, DIALECT=1, DIET=2, ENERGY=3`.

- [ ] **Step 1: Create the overlay manager script**

Create `game/scripts/overlay_manager.gd`:

```gdscript
extends Node

# Ground layer modes. PHEROMONE_0..3 are contiguous (1..4).
const GROUND_BIOME := 0
const GROUND_PHEROMONE_0 := 1
const GROUND_ENV_OPTIMUM := 5
const GROUND_MAX := 6  # count of ground modes

# Body color modes.
const BODY_SPECIES := 0
const BODY_DIALECT := 1
const BODY_DIET := 2
const BODY_ENERGY := 3
const BODY_MAX := 4

var ground_mode: int = GROUND_BIOME
var body_mode: int = BODY_SPECIES

@onready var sim = get_node("/root/Main/Simulation")

func _ready() -> void:
	ground_mode = GameConfig.default_ground
	body_mode = GameConfig.default_body

func ground_is_biome() -> bool:
	return ground_mode == GROUND_BIOME

func ground_is_optimum() -> bool:
	return ground_mode == GROUND_ENV_OPTIMUM

# Pheromone channel for the current ground mode, or -1 if not a pheromone mode.
func ground_channel() -> int:
	if ground_mode >= GROUND_PHEROMONE_0 and ground_mode <= GROUND_PHEROMONE_0 + 3:
		return ground_mode - GROUND_PHEROMONE_0
	return -1

func _unhandled_key_input(event: InputEvent) -> void:
	if not (event is InputEventKey) or not event.pressed or event.echo:
		return
	var k := event as InputEventKey
	if k.keycode == KEY_G:
		_cycle_ground()
	elif k.keycode == KEY_C:
		_cycle_body()

func _cycle_ground() -> void:
	ground_mode = (ground_mode + 1) % GROUND_MAX
	# Skip ENV_OPTIMUM when the env mechanism is inactive.
	if ground_mode == GROUND_ENV_OPTIMUM and not bool(sim.env_active()):
		ground_mode = GROUND_BIOME

func _cycle_body() -> void:
	body_mode = (body_mode + 1) % BODY_MAX
```

- [ ] **Step 2: Add the OverlayManager node to main.tscn**

In `game/scenes/main.tscn`, add a script ext_resource and a node. Add after the existing `[ext_resource ... id="8_codex"]` line:

```
[ext_resource type="Script" path="res://scripts/overlay_manager.gd" id="9_overlay"]
```

Add after the `[node name="Simulation" ...]` block (before `[node name="Biome" ...]`):

```
[node name="OverlayManager" type="Node" parent="."]
script = ExtResource("9_overlay")
```

- [ ] **Step 3: Verify in Godot**

Run: launch project, start any scenario, press `G` and `C` a few times.
Expected: no script errors in Output. (Nothing changes visually yet — renderers wire up in Tasks 7–8.)

- [ ] **Step 4: Commit**

```bash
git add game/scripts/overlay_manager.gd game/scenes/main.tscn
git commit -m "$(printf 'feat(godot): overlay manager (ground + body display modes, G/C hotkeys)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 7: Ground renderer reads overlay mode

**Files:**
- Modify: `game/scripts/biome_renderer.gd`

**Interfaces:**
- Consumes: `OverlayManager.ground_is_biome()`, `.ground_channel()`, `.ground_is_optimum()`; `Simulation.pheromone_colors(ch)`, `.env_optimum()`, `.biome_colors()`.

- [ ] **Step 1: Rewrite biome_renderer to dispatch on ground mode**

Replace `game/scripts/biome_renderer.gd` with:

```gdscript
extends Sprite2D

@onready var sim = get_node("/root/Main/Simulation")
@onready var overlay = get_node("/root/Main/OverlayManager")

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
	var world: float = sim.world_size()
	scale = Vector2(world / _res, world / _res)
	position = Vector2.ZERO
	z_index = -10

func _process(_delta: float) -> void:
	if _res <= 0:
		return
	var colors: PackedColorArray
	var ch: int = overlay.ground_channel()
	if overlay.ground_is_optimum():
		# Flat tint whose hue encodes the current global optimum in [0,1].
		var opt: float = sim.env_optimum()
		var c := Color.from_hsv(clampf(opt, 0.0, 1.0) * 0.8, 0.7, 0.5) if opt >= 0.0 else Color(0.1, 0.1, 0.12)
		colors = PackedColorArray()
		colors.resize(_res * _res)
		colors.fill(c)
	elif ch >= 0:
		colors = sim.pheromone_colors(ch)
	else:
		colors = sim.biome_colors()
	if colors.size() != _res * _res:
		return
	for row in _res:
		for col in _res:
			_img.set_pixel(col, row, colors[row * _res + col])
	_tex.update(_img)
```

- [ ] **Step 2: Verify in Godot**

Run: launch, start `M13 — Territories`, press `G` to cycle ground modes.
Expected: ground cycles biome → pheromone channels (heat colors where agents deposit) → back; no errors. Start a DIT scenario and confirm `G` reaches an env-optimum tint that shifts hue over time.

- [ ] **Step 3: Commit**

```bash
git add game/scripts/biome_renderer.gd
git commit -m "$(printf 'feat(godot): ground overlay dispatch (biome / pheromone / env-optimum)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 8: Body color modes + carcass/flash sprites

**Files:**
- Modify: `game/scripts/main.gd`
- Modify: `game/scenes/main.tscn`

**Interfaces:**
- Consumes: `OverlayManager.body_mode`; `Simulation.alive_colors/positions/sizes`, `agent_detail` is NOT used here (per-frame cost); instead new bulk exports are not added — DIALECT/DIET/ENERGY are computed from data already pulled: DIET/ENERGY need per-agent scalars. To avoid N Dictionary calls per frame, this task adds two more read-only bulk exports.

**Note:** This task adds two small bulk exports to `crates/anabios-godot/src/lib.rs` first (they belong with the body-mode renderer that consumes them), then the GDScript.

- [ ] **Step 1: Add bulk body-scalar exports (Rust)**

Add inside `impl Simulation` in `crates/anabios-godot/src/lib.rs`:

```rust
    /// Carnivory diet score per alive agent (0 herbivore .. 1 carnivore),
    /// same order as `alive_positions`.
    #[func]
    fn alive_diet(&self) -> PackedFloat32Array {
        let mut out = PackedFloat32Array::new();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                out.push(anabios_core::module::effective_diet_carnivory(&w.agents.modules[id as usize]));
            }
        }
        out
    }

    /// Dialect hue per alive agent in `[0,1)`, same order as `alive_positions`.
    #[func]
    fn alive_dialect_hue(&self) -> PackedFloat32Array {
        let mut out = PackedFloat32Array::new();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                out.push(dialect_hue(&w.agents.meme_vector[id as usize]));
            }
        }
        out
    }

    /// Energy per alive agent, same order as `alive_positions`.
    #[func]
    fn alive_energy(&self) -> PackedFloat32Array {
        let mut out = PackedFloat32Array::new();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                out.push(w.agents.energy[id as usize]);
            }
        }
        out
    }
```

- [ ] **Step 2: Rust gate (build + determinism + CI)**

Run: `rustup run stable cargo build -p anabios-godot` → compiles.
Run: `rustup run stable cargo test -p anabios-core --test determinism` → hashes unchanged.
Run the fmt/clippy/doc trio from the Global Constraints.

- [ ] **Step 3: Add carcass + flash MultiMeshInstance2D nodes to main.tscn**

Add two sub-resources near the other MultiMesh sub-resources in `game/scenes/main.tscn`:

```
[sub_resource type="MultiMesh" id="MM_carcass"]
transform_format = 0
use_colors = true
instance_count = 4096
visible_instance_count = 0
mesh = SubResource("QuadMesh_glyph")

[sub_resource type="MultiMesh" id="MM_flash"]
transform_format = 0
use_colors = true
instance_count = 4096
visible_instance_count = 0
mesh = SubResource("QuadMesh_glyph")
```

Add two nodes after the `[node name="Bodies" ...]` block:

```
[node name="Carcasses" type="MultiMeshInstance2D" parent="."]
multimesh = SubResource("MM_carcass")
z_index = -5

[node name="Flashes" type="MultiMeshInstance2D" parent="."]
multimesh = SubResource("MM_flash")
z_index = 5
```

- [ ] **Step 3.5: Verify body-mode helper mapping before wiring**

There is no automated test for GDScript; confirm by reading that `alive_diet`, `alive_dialect_hue`, `alive_energy`, `carcass_data`, `combat_flashes` are spelled identically in `lib.rs` and the code below.

- [ ] **Step 4: Wire body color modes + carcasses + flashes into main.gd**

In `game/scripts/main.gd`, add an onready for the overlay and the two mesh nodes, and add color-mode + carcass/flash logic. Add these `@onready`s after the existing ones:

```gdscript
@onready var overlay = $OverlayManager
@onready var carcasses: MultiMeshInstance2D = $Carcasses
@onready var flashes: MultiMeshInstance2D = $Flashes
```

Replace the body-color upload loop in `_refresh_bodies` (the `for i in n:` block) with a mode-aware version. Replace:

```gdscript
	for i in n:
		var t: Transform2D = Transform2D(rots[i], Vector2(sizes[i], sizes[i]), 0.0, positions[i])
		mm.set_instance_transform_2d(i, t)
		mm.set_instance_color(i, colors[i])
```

with:

```gdscript
	var body_colors: PackedColorArray = _body_colors(n)
	for i in n:
		var t: Transform2D = Transform2D(rots[i], Vector2(sizes[i], sizes[i]), 0.0, positions[i])
		mm.set_instance_transform_2d(i, t)
		mm.set_instance_color(i, body_colors[i])
```

Add these helper functions to `main.gd`:

```gdscript
func _body_colors(n: int) -> PackedColorArray:
	match overlay.body_mode:
		overlay.BODY_DIALECT:
			var hues: PackedFloat32Array = sim.alive_dialect_hue()
			var out := PackedColorArray()
			out.resize(n)
			for i in n:
				out[i] = Color.from_hsv(hues[i], 0.7, 0.95)
			return out
		overlay.BODY_DIET:
			var diet: PackedFloat32Array = sim.alive_diet()
			var out2 := PackedColorArray()
			out2.resize(n)
			for i in n:
				out2[i] = Color(0.3, 0.9, 0.4).lerp(Color(1.0, 0.3, 0.3), clampf(diet[i], 0.0, 1.0))
			return out2
		overlay.BODY_ENERGY:
			var en: PackedFloat32Array = sim.alive_energy()
			var out3 := PackedColorArray()
			out3.resize(n)
			for i in n:
				var t := clampf(en[i] / 50.0, 0.0, 1.0)
				out3[i] = Color(0.2, 0.3, 0.8).lerp(Color(1.0, 0.9, 0.3), t)
			return out3
		_:
			return sim.alive_colors()

func _refresh_carcasses() -> void:
	var data: Array = sim.carcass_data()
	var mm: MultiMesh = carcasses.multimesh
	var m: int = data.size()
	mm.visible_instance_count = m
	for i in m:
		var d: Dictionary = data[i]
		var pos: Vector2 = d["pos"]
		var f: float = clampf(float(d["flesh"]) / 20.0, 0.2, 1.5)
		mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(f, f), 0.0, pos))
		mm.set_instance_color(i, Color(0.77, 0.80, 0.86, 0.55))

func _refresh_flashes() -> void:
	var pts: PackedVector2Array = sim.combat_flashes()
	var mm: MultiMesh = flashes.multimesh
	var m: int = pts.size()
	mm.visible_instance_count = m
	for i in m:
		mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(1.6, 1.6), 0.0, pts[i]))
		mm.set_instance_color(i, Color(1.0, 0.85, 0.2, 0.9))
```

Call the two refreshers from `_process`, after `_refresh_bodies()`:

```gdscript
	_refresh_carcasses()
	_refresh_flashes()
```

- [ ] **Step 5: Verify in Godot**

Run: launch, start `M12 — Predator / prey`. Press `C` to cycle body colors (species → dialect → diet → energy). Watch for carcasses (translucent) appearing on kills and yellow flashes on combat.
Expected: body colors change per mode; carcasses and flashes render; no errors.

- [ ] **Step 6: Commit**

```bash
rustup run stable cargo fmt --all
git add crates/anabios-godot/src/lib.rs game/scripts/main.gd game/scenes/main.tscn
git commit -m "$(printf 'feat(godot): body color modes + carcass/combat-flash rendering\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

## Phase P4 — Panels

### Task 9: Richer inspector

**Files:**
- Modify: `game/scripts/inspector_panel.gd`

**Interfaces:**
- Consumes: `Simulation.agent_detail(id)` (keys from Task 5).

- [ ] **Step 1: Rewrite the inspector to render agent_detail**

Replace the `_process` in `game/scripts/inspector_panel.gd` with:

```gdscript
func _process(_delta: float) -> void:
	if pinned_id < 0:
		return
	var info: Dictionary = sim.agent_detail(pinned_id)
	if info.is_empty() or not info.get("alive", false):
		label.text = "(agent %d is dead)" % pinned_id
		return
	var lines: PackedStringArray = [
		"id %d   species %d   lineage %d" % [pinned_id, info["species_id"], info["lineage_id"]],
		"energy %.1f   age %d" % [info["energy"], info["age"]],
		"program %d   modules %d" % [info["program_len"], info["module_count"]],
		"diet %.2f (0=herb 1=carn)" % info["diet_carnivory"],
		"skill %.2f   technique %.2f" % [info["skill"], info["technique"]],
		"learn: indiv=%s social=%s" % [str(info["indiv_learn"]), str(info["social_learn"])],
		"modules: %s" % ", ".join(info["module_names"]),
	]
	label.text = "\n".join(lines)
```

- [ ] **Step 2: Verify in Godot**

Run: launch any scenario, left-click an agent.
Expected: inspector shows diet, skill/technique, learning flags, and module names; no errors.

- [ ] **Step 3: Commit**

```bash
git add game/scripts/inspector_panel.gd
git commit -m "$(printf 'feat(godot): richer inspector (diet, memes, learning, module names)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 10: Population panel

**Files:**
- Create: `game/scripts/population_panel.gd`
- Modify: `game/scenes/main.tscn`

**Interfaces:**
- Consumes: `Simulation.species_stats()`.

- [ ] **Step 1: Create the population panel script**

Create `game/scripts/population_panel.gd`:

```gdscript
extends PanelContainer

const REFRESH_EVERY := 6

@onready var sim = get_node("/root/Main/Simulation")
@onready var list: VBoxContainer = $VBox
var _frame: int = 0

func _process(_delta: float) -> void:
	_frame += 1
	if _frame % REFRESH_EVERY != 0:
		return
	var stats: Array = sim.species_stats()
	for child in list.get_children():
		child.queue_free()
	for s in stats:
		var lbl := Label.new()
		lbl.add_theme_font_size_override("font_size", 12)
		lbl.text = "sp %d   n=%d   E=%.0f" % [int(s["species_id"]), int(s["count"]), float(s["mean_energy"])]
		list.add_child(lbl)
```

- [ ] **Step 2: Add the panel to main.tscn**

Add ext_resource (with a fresh id, e.g. `10_pop`):

```
[ext_resource type="Script" path="res://scripts/population_panel.gd" id="10_pop"]
```

Add node under UI (after the Inspector block):

```
[node name="PopulationPanel" type="PanelContainer" parent="UI"]
script = ExtResource("10_pop")
offset_left = 1050.0
offset_top = 210.0
offset_right = 1270.0
offset_bottom = 420.0

[node name="VBox" type="VBoxContainer" parent="UI/PopulationPanel"]
```

- [ ] **Step 3: Verify in Godot**

Run: launch `M12 — Predator / prey`.
Expected: top-right panel lists each species with live count and mean energy, updating as populations shift; no errors.

- [ ] **Step 4: Commit**

```bash
git add game/scripts/population_panel.gd game/scenes/main.tscn
git commit -m "$(printf 'feat(godot): live per-species population panel\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 11: DIT panel (env scenarios only)

**Files:**
- Create: `game/scripts/dit_panel.gd`
- Modify: `game/scenes/main.tscn`

**Interfaces:**
- Consumes: `Simulation.env_active()`, `.env_optimum()`, `.species_stats()` (`mean_technique_match`).

- [ ] **Step 1: Create the DIT panel script**

Create `game/scripts/dit_panel.gd`:

```gdscript
extends PanelContainer

const REFRESH_EVERY := 6

@onready var sim = get_node("/root/Main/Simulation")
@onready var list: VBoxContainer = $VBox
var _frame: int = 0

func _process(_delta: float) -> void:
	if not bool(sim.env_active()):
		visible = false
		return
	visible = true
	_frame += 1
	if _frame % REFRESH_EVERY != 0:
		return
	for child in list.get_children():
		child.queue_free()
	var header := Label.new()
	header.add_theme_font_size_override("font_size", 13)
	header.text = "DIT  env-optimum = %.2f" % float(sim.env_optimum())
	list.add_child(header)
	for s in sim.species_stats():
		var lbl := Label.new()
		lbl.add_theme_font_size_override("font_size", 12)
		lbl.text = "sp %d   match=%.2f" % [int(s["species_id"]), float(s["mean_technique_match"])]
		list.add_child(lbl)
```

- [ ] **Step 2: Add the panel to main.tscn**

Add ext_resource (id `11_dit`):

```
[ext_resource type="Script" path="res://scripts/dit_panel.gd" id="11_dit"]
```

Add node under UI:

```
[node name="DitPanel" type="PanelContainer" parent="UI"]
script = ExtResource("11_dit")
visible = false
offset_left = 1050.0
offset_top = 430.0
offset_right = 1270.0
offset_bottom = 620.0

[node name="VBox" type="VBoxContainer" parent="UI/DitPanel"]
```

- [ ] **Step 3: Verify in Godot**

Run: launch `DIT — Env slow`. Confirm the DIT panel appears with a shifting env-optimum and per-species match. Launch `M12 — Predator / prey` and confirm the panel stays hidden.
Expected: panel visible only for env scenarios; no errors.

- [ ] **Step 4: Commit**

```bash
git add game/scripts/dit_panel.gd game/scenes/main.tscn
git commit -m "$(printf 'feat(godot): DIT panel (env-optimum + per-species technique match)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

## Phase P5 — UX polish

### Task 12: Legend / keybind overlay

**Files:**
- Create: `game/scripts/legend_panel.gd`
- Modify: `game/scenes/main.tscn`

**Interfaces:**
- Consumes: `OverlayManager.ground_mode`, `.body_mode` (ints); toggled by `H`.

- [ ] **Step 1: Create the legend panel script**

Create `game/scripts/legend_panel.gd`:

```gdscript
extends PanelContainer

const GROUND_NAMES := ["biome", "phero-0", "phero-1", "phero-2", "phero-3", "env-optimum"]
const BODY_NAMES := ["species", "dialect", "diet", "energy"]

@onready var overlay = get_node("/root/Main/OverlayManager")
@onready var label: Label = $Label

func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_H:
		visible = not visible

func _process(_delta: float) -> void:
	if not visible:
		return
	var g: int = clampi(overlay.ground_mode, 0, GROUND_NAMES.size() - 1)
	var b: int = clampi(overlay.body_mode, 0, BODY_NAMES.size() - 1)
	label.text = (
		"[G] ground: %s\n[C] body: %s\n[H] hide this\nWASD/drag pan · wheel zoom · click inspect"
	) % [GROUND_NAMES[g], BODY_NAMES[b]]
```

- [ ] **Step 2: Add the panel to main.tscn**

Add ext_resource (id `12_legend`):

```
[ext_resource type="Script" path="res://scripts/legend_panel.gd" id="12_legend"]
```

Add node under UI (bottom-left, above time controls):

```
[node name="LegendPanel" type="PanelContainer" parent="UI"]
script = ExtResource("12_legend")
offset_left = 10.0
offset_top = 620.0
offset_right = 380.0
offset_bottom = 750.0

[node name="Label" type="Label" parent="UI/LegendPanel"]
text = "—"
theme_override_font_sizes/font_size = 13
```

- [ ] **Step 3: Verify in Godot + full scenario sweep**

Run: launch, confirm the legend shows active modes and toggles with `H`. Then load **all 14 scenarios** in turn; for each, confirm no script errors in Output, `G`/`C` cycle correctly, and DIT scenarios show the DIT panel.
Expected: clean run across the whole scenario set.

- [ ] **Step 4: Commit**

```bash
git add game/scripts/legend_panel.gd game/scenes/main.tscn
git commit -m "$(printf 'feat(godot): legend/keybind overlay + full-scenario smoke pass\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

## Self-Review

**Spec coverage:**
- Scenario breadth → Task 1. ✓
- Mechanic visibility: pheromones → Tasks 2, 7; carcasses/flashes → Tasks 4, 8; dialect coloring → Tasks 5, 8; env-optimum → Tasks 3, 7. ✓
- Analytics & inspector: richer inspector → Tasks 5, 9; population panel → Tasks 3, 10; DIT panel → Tasks 3, 11. ✓
- Visual & UX refinement: overlay manager + modes → Tasks 6–8; legend → Task 12. ✓
- Determinism invariant → gated in every Rust task (2, 3, 4, 5, 8). ✓
- Smart per-scenario defaults → Task 1 (data) + Task 6 (`_ready` reads them). ✓

**Type consistency:** export names used by GDScript match `lib.rs` definitions — `pheromone_colors`, `pheromone_channel_count`, `env_active`, `env_optimum`, `species_stats` (keys `species_id/count/mean_energy/mean_technique_match`), `carcass_data` (keys `pos/flesh/age/species_id`), `combat_flashes`, `agent_detail` (keys incl. `diet_carnivory/skill/technique/indiv_learn/social_learn/dialect_hue/module_names`), `alive_diet`, `alive_dialect_hue`, `alive_energy`. Overlay enum constants (`GROUND_*`, `BODY_*`) referenced consistently in Tasks 6–8, 12.

**Placeholder scan:** no TBD/TODO; every code step carries full code.

## Manual verification checklist (no headless render tests exist)

Run once after Task 12, load each scenario, confirm no Output errors:
`minimal, divergent, predator-prey, territories, dialects, cooperation, gene-culture, gene-culture-skill, gene-culture-hunt, gene-culture-alarm, dit-env-slow, dit-env-fast, dit-env-static, dit-rogers`.
