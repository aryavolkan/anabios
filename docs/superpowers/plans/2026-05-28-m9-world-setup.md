# M9 — World Setup + Accessibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a front-end shell to the viewer: a main menu where you choose a scenario and seed before launching, plus back-to-menu / restart controls in the viewer and basic accessibility (pause-on-focus-loss + UI scale). Turns the single hardcoded-scenario viewer into something you can actually configure and replay.

**Architecture:** A new `menu.tscn` becomes the main scene. It lists bundled scenarios, takes a seed, and stores the choice in a `GameConfig` autoload singleton, then switches to `main.tscn`. The viewer reads `GameConfig` on load instead of hardcoding the scenario path. gdext gains `load_scenario_with_seed` so the menu's seed overrides the scenario's own. Accessibility: the viewer pauses when the window loses focus, and a UI-scale setting in the menu scales the HUD/controls.

**Tech Stack:** Same as M8.

**Branch:** `m9-world-setup` from `main`.

**Working directory:** `/Users/aryasen/projects/anabios/`.

**Scope note (medium effort):** Menu + scenario picker + seed + back/restart + pause-on-focus-loss + UI scale. Deferred: full terrain/climate/species trait editor, saving custom `.anascen` files, the agent colorblind palette (genome-hue remap is non-trivial), and the cross-world codex DB.

---

## File structure after M9

New:
```
game/scenes/menu.tscn
game/scripts/menu.gd
game/scripts/game_config.gd        # autoload singleton
```
Modified:
```
game/project.godot                 # main scene → menu.tscn; register GameConfig autoload
game/scripts/main.gd               # read GameConfig; add back/restart; pause-on-focus-loss
game/scenes/main.tscn              # +Back/Restart buttons
crates/anabios-godot/src/lib.rs    # +load_scenario_with_seed
```

---

## Task 0: Branch

- [ ] `git checkout main && git pull && git checkout -b m9-world-setup`
- [ ] `cargo build -p anabios-godot` green.

---

## Task 1: gdext load_scenario_with_seed

**Files:** Modify `crates/anabios-godot/src/lib.rs`

- [ ] **Step 1.1:** Add a method that parses a scenario, overrides its seed, and instantiates:

```rust
    /// Load a TOML scenario but override its seed. Returns true on success.
    #[func]
    fn load_scenario_with_seed(&mut self, toml_text: GString, seed: i64) -> bool {
        let text = String::from(toml_text);
        match anabios_core::Scenario::parse_toml(&text) {
            Ok(mut s) => {
                s.seed = seed as u64;
                self.inner = Some(s.instantiate());
                true
            }
            Err(e) => {
                godot_error!("scenario parse failed: {e}");
                false
            }
        }
    }
```

- [ ] **Step 1.2:** Build + fmt + clippy + commit.

```bash
git commit -m "feat(godot): load_scenario_with_seed for menu seed override"
```

---

## Task 2: GameConfig autoload

**Files:** Create `game/scripts/game_config.gd`; modify `game/project.godot`

- [ ] **Step 2.1:** `game_config.gd`:

```gdscript
extends Node

# Selected scenario + seed, set by the menu and read by the viewer.
var scenario_path: String = "res://../scenarios/minimal.toml"
var seed: int = 12345
var ui_scale: float = 1.0
```

- [ ] **Step 2.2:** Register as an autoload in `project.godot`. Add:

```ini
[autoload]

GameConfig="*res://scripts/game_config.gd"
```

Also change `run/main_scene` to `res://scenes/menu.tscn`.

- [ ] **Step 2.3:** Commit.

```bash
git commit -m "feat(game): GameConfig autoload singleton; menu as main scene"
```

---

## Task 3: Main menu scene

**Files:** Create `game/scenes/menu.tscn`, `game/scripts/menu.gd`

- [ ] **Step 3.1:** `menu.gd` — populate an OptionButton from the two bundled scenarios, a SpinBox for seed, a SpinBox for UI scale, and a Start button that writes to GameConfig and changes scene to `main.tscn`.

```gdscript
extends Control

const SCENARIOS: Array[Dictionary] = [
	{ "label": "Minimal (200 herbivores)", "path": "res://../scenarios/minimal.toml" },
	{ "label": "Divergent (two founders)", "path": "res://../scenarios/divergent.toml" },
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
	GameConfig.scenario_path = SCENARIOS[idx]["path"]
	GameConfig.seed = int(seed_spin.value)
	GameConfig.ui_scale = scale_spin.value
	get_tree().change_scene_to_file("res://scenes/main.tscn")
```

- [ ] **Step 3.2:** `menu.tscn` — a centered VBox with a title Label, ScenarioPick OptionButton, Seed row (Label + SpinBox 0..2^31), Scale row (Label + SpinBox 0.5..2.0 step 0.1), Start button. Keep it simple.

- [ ] **Step 3.3:** Commit.

```bash
git commit -m "feat(game): main menu with scenario picker + seed + UI scale"
```

---

## Task 4: Viewer integration + back/restart + pause-on-focus-loss

**Files:** Modify `game/scripts/main.gd`, `game/scenes/main.tscn`

- [ ] **Step 4.1:** In `main.gd::_ready`, read `GameConfig.scenario_path` + `GameConfig.seed`, load via `sim.load_scenario_with_seed`. Apply `GameConfig.ui_scale` to the UI CanvasLayer (set the root Control's scale, or each control's theme scale — simplest: set `$UI` layer transform scale).

- [ ] **Step 4.2:** Add pause-on-focus-loss:

```gdscript
func _notification(what: int) -> void:
	if what == NOTIFICATION_APPLICATION_FOCUS_OUT:
		paused = true
	# Do NOT auto-resume on focus-in; let the user press play.
```

- [ ] **Step 4.3:** Add Back-to-menu and Restart buttons to the time controls (or a small top-right row). Back → `change_scene_to_file("res://scenes/menu.tscn")`. Restart → reload current scene (`get_tree().reload_current_scene()`).

- [ ] **Step 4.4:** Smoke test + commit.

```bash
godot --headless --quit --path game/ 2>&1 | tail -10
git commit -m "feat(game): viewer reads GameConfig; back/restart buttons; pause-on-focus-loss"
```

---

## Task 5: Smoke test + tag

- [ ] **Step 5.1:** Headless: `godot --headless --quit --path game/` — menu scene loads without error. (It won't auto-advance to the sim in headless, but should not crash.)
- [ ] **Step 5.2:** Interactive (local): launch, pick Divergent, set a seed, Start → viewer runs that scenario/seed; Back returns to menu; Restart reloads; switching away pauses.
- [ ] **Step 5.3:** `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`; tag `m9`.

```bash
git tag -a m9 -m "M9: world setup menu + back/restart + accessibility basics"
```

---

## Post-implementation expectations

- Launching the project shows a menu; you pick scenario + seed, press Start, and the viewer runs your choice
- Back-to-menu and Restart work from the viewer
- The window pauses when it loses focus
- UI scale setting adjusts the overlay size
- Determinism untouched (gdext seed override just sets `scenario.seed` before instantiate)

Deferred to M10+:
- Full terrain/climate/species trait editor + `.anascen` save/load
- Agent colorblind palette (genome-hue remap)
- Cross-world persistent codex DB
- Headless sweep tooling + W&B (the M10 milestone)
