# Frontend Batch (c) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut per-frame frontend cost — throttle the biome image rebuild, collapse 9 module-glyph passes into 1, cache co-evolution series per draw, and stop the pop/DIT panel label churn.

**Architecture:** Four localized changes: three pure-GDScript (`biome_renderer`, `coevolution_panel`, `population/dit panels`) and one read-only gdext binding addition (`module_glyphs_all`) consumed by `main.gd`. The binding stays a read-only view of `World` (no `&mut`, no new World fields); `anabios-core` is untouched.

**Tech Stack:** Rust (gdext binding, `anabios-godot`), GDScript (Godot 4.7).

## Global Constraints

- **Binding is read-only:** no `&mut World`, no new `World` fields. `anabios-core` untouched → core golden test still passes trivially (determinism unaffected).
- **No golden hash for the frontend.** Verification per task = headless boot clean (`/opt/homebrew/bin/godot --headless --path game res://scenes/main.tscn --quit-after <frames>` → no parse/`SCRIPT ERROR`/`nonexistent function`/`invalid call`; clean exit) + manual visual pass at the end.
- **Rust binding gate** (Task 2 only): `rustup run stable cargo fmt --all --check`; `rustup run stable cargo clippy -p anabios-godot --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc -p anabios-godot --no-deps --document-private-items`. Rebuild the debug dylib (`rustup run stable cargo build -p anabios-godot`) before the headless boot so Godot loads the new export.
- **Godot:** `/opt/homebrew/bin/godot` (4.7). macOS has no `timeout`; use `--quit-after <frames>`.
- **Rendering must look identical** to before (only timing/allocation changes).

---

### Task 1: `biome_renderer` — throttle + `set_data`

**Files:**
- Modify: `game/scripts/biome_renderer.gd`

**Interfaces:**
- Consumes: existing `sim.biome_colors()`, `sim.pheromone_colors(ch)`, `sim.env_optimum()`, `overlay.ground_channel()`, `overlay.ground_is_optimum()`.

- [ ] **Step 1: Add throttle state + a mode/channel change detector**

In `game/scripts/biome_renderer.gd`, add module-level vars after `var _res: int = 0`:

```gdscript
const REDRAW_EVERY := 6
var _frame: int = 0
var _last_mode: int = -999   # last (channel or -1 biome or -2 optimum) drawn
```

- [ ] **Step 2: Rewrite `_process` to throttle + build via a byte buffer**

Replace the entire `_process` body with:

```gdscript
func _process(_delta: float) -> void:
	if _res <= 0:
		return
	# Current ground selection encoded as a single int: -2 optimum, -1 biome, else channel.
	var mode: int = -1
	if overlay.ground_is_optimum():
		mode = -2
	else:
		var ch0: int = overlay.ground_channel()
		if ch0 >= 0:
			mode = ch0
	# Throttle: rebuild every REDRAW_EVERY frames, but immediately when the ground
	# selection changed (so [G]/overlay toggles feel instant).
	_frame += 1
	if mode == _last_mode and _frame % REDRAW_EVERY != 0:
		return
	_last_mode = mode

	var colors: PackedColorArray
	if mode == -2:
		var opt: float = sim.env_optimum()
		var c: Color = Color.from_hsv(clampf(opt, 0.0, 1.0) * 0.8, 0.7, 0.5) if opt >= 0.0 else Color(0.1, 0.1, 0.12)
		colors = PackedColorArray()
		colors.resize(_res * _res)
		colors.fill(c)
	elif mode >= 0:
		colors = sim.pheromone_colors(mode)
	else:
		colors = sim.biome_colors()
	if colors.size() != _res * _res:
		return

	# Build an RGBA8 byte buffer in one pass (faster than per-pixel set_pixel).
	var bytes := PackedByteArray()
	bytes.resize(_res * _res * 4)
	for i in colors.size():
		var col: Color = colors[i]
		var o: int = i * 4
		bytes[o] = int(clampf(col.r, 0.0, 1.0) * 255.0)
		bytes[o + 1] = int(clampf(col.g, 0.0, 1.0) * 255.0)
		bytes[o + 2] = int(clampf(col.b, 0.0, 1.0) * 255.0)
		bytes[o + 3] = int(clampf(col.a, 0.0, 1.0) * 255.0)
	_img.set_data(_res, _res, false, Image.FORMAT_RGBA8, bytes)
	_tex.update(_img)
```

(`_img`/`_tex` are already created `FORMAT_RGBA8` in `_ready`. `set_data` replaces the whole image in one call; the per-pixel `set_pixel` double loop is gone.)

- [ ] **Step 3: Headless boot check**

Run: `/opt/homebrew/bin/godot --headless --path game res://scenes/main.tscn --quit-after 150`
Expected: no `SCRIPT ERROR`/parse errors; clean exit. (Surface the command for the user if `godot` isn't on PATH.)

- [ ] **Step 4: Commit**

```bash
git add game/scripts/biome_renderer.gd
git commit -m "perf(godot): throttle biome_renderer rebuild + build image via set_data

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: `module_glyphs_all` — one pass instead of nine

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs` (add export)
- Modify: `game/scripts/main.gd` (`_refresh_module_layers`)

**Interfaces:**
- Produces: `#[func] fn module_glyphs_all(&self) -> Array<PackedVector2Array>` — index `t` holds all glyph world-positions for module type `t`, length = `module_type_count()`.

- [ ] **Step 1: Add the bundled export to the binding**

In `crates/anabios-godot/src/lib.rs`, next to the existing `module_glyphs` (around line 576), add:

```rust
    /// All module glyphs in ONE alive pass, bucketed by module type. Returns an
    /// array of length `module_type_count()`; entry `t` is a `PackedVector2Array`
    /// of world positions for every module of type `t`. Read-only view.
    #[func]
    fn module_glyphs_all(&self) -> Array<PackedVector2Array> {
        use anabios_core::genome::GenomeSlot;
        let type_count = anabios_core::module::ModuleType::COUNT;
        let mut out: Array<PackedVector2Array> = Array::new();
        for _ in 0..type_count {
            out.push(&PackedVector2Array::new());
        }
        let Some(w) = self.inner.as_ref() else { return out };
        for id in w.agents.iter_alive() {
            let i = id as usize;
            let pos = w.agents.position[i];
            let size = 0.5 + 2.5 * w.agents.genome[i].get(GenomeSlot::Size);
            let radius = size * 0.7;
            for (slot, m) in w.agents.modules[i].iter().enumerate() {
                let t = m.module_type() as usize;
                if t >= type_count {
                    continue;
                }
                let angle = (slot as f32) * std::f32::consts::TAU / 8.0;
                let gx = pos.x + radius * math_cos(angle);
                let gy = pos.y + radius * math_sin(angle);
                let mut arr = out.at(t);
                arr.push(Vector2::new(gx, gy));
                out.set(t, &arr);
            }
        }
        out
    }
```

**Note on `ModuleType::COUNT`:** the existing `module_type_count()` returns `9`. Check whether `anabios_core::module::ModuleType` exposes a `COUNT`/`count()`; if not, replace `let type_count = …::COUNT;` with `let type_count = 9usize;` to match `module_type_count()` exactly (the hardcoded 9 the frontend already relies on). Verify against `module_type_count`'s body.

**Note on `Array<PackedVector2Array>` mutation:** gdext's `Array::at(i)` returns a clone; the `push` + `set(t, &arr)` write-back above is O(len) per glyph. If that is measurably slow, build a `Vec<PackedVector2Array>` locally, push into the plain Vec, then convert to `Array` once at the end — but the write-back form is fine for typical agent counts and is the simplest correct version. Pick whichever compiles cleanly; both are read-only w.r.t. `World`.

- [ ] **Step 2: Consume it in `main.gd`**

Replace `_refresh_module_layers` in `game/scripts/main.gd`:

```gdscript
func _refresh_module_layers() -> void:
	var all: Array = sim.module_glyphs_all()
	var type_count: int = all.size()
	for t in type_count:
		var layer: MultiMeshInstance2D = module_layers.get_child(t)
		var glyphs: PackedVector2Array = all[t]
		var m: int = glyphs.size()
		var mm: MultiMesh = layer.multimesh
		if m > mm.instance_count:
			mm.instance_count = m
		mm.visible_instance_count = m
		var col: Color = MODULE_COLORS[t]
		for i in m:
			mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(GLYPH_SIZE, GLYPH_SIZE), 0.0, glyphs[i]))
			mm.set_instance_color(i, col)
```

(Preserves the grow-guard `if m > mm.instance_count` and the transform/color assignment exactly; only the data source changed from 9 calls to 1.)

- [ ] **Step 3: Rust gate + rebuild dylib**

```bash
rustup run stable cargo clippy -p anabios-godot --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc -p anabios-godot --no-deps --document-private-items
rustup run stable cargo build -p anabios-godot
```
Expected: clean.

- [ ] **Step 4: Headless boot check**

Run: `/opt/homebrew/bin/godot --headless --path game res://scenes/main.tscn --quit-after 200`
Expected: no script errors (esp. no "nonexistent function 'module_glyphs_all'"); clean exit.

- [ ] **Step 5: Commit**

```bash
rustup run stable cargo fmt --all
git add crates/anabios-godot/src/lib.rs game/scripts/main.gd
git commit -m "perf(godot): module_glyphs_all — one alive pass for all glyph layers

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: `coevolution_panel` — cache series per draw

**Files:**
- Modify: `game/scripts/coevolution_panel.gd` (`_draw`, `_draw_chart`)

**Interfaces:**
- Consumes: existing `sim.coevo_history_len()`, `sim.coevo_series(key)`.

- [ ] **Step 1: Fetch every series once in `_draw`, pass a cache to `_draw_chart`**

In `game/scripts/coevolution_panel.gd`, in `_draw` (after the `var ticks := sim.coevo_series("tick")` line, ~line 133), build a per-key cache of all series any chart needs, then pass it into `_draw_chart`:

```gdscript
	var ticks: PackedFloat32Array = sim.coevo_series("tick")
	# Fetch each series ONCE this draw (charts otherwise re-fetch per series and
	# again for auto-scale — O(history) copies each).
	var cache: Dictionary = {}
	for c in CHARTS:
		for s in c["series"]:
			var k: String = s["key"]
			if not cache.has(k):
				cache[k] = sim.coevo_series(k)
```

Then change the chart loop to pass `cache` and `n`:

```gdscript
	for c in CHARTS:
		_draw_chart(c, cache, n, pad, plot_w, y_cursor, chart_h)
		# ... (keep the existing y_cursor advance / other args as they are)
```
(Match the existing `_draw_chart(...)` call's positional args — insert `cache, n` after `c` and drop any now-redundant `n` the callee recomputes. Keep every other argument identical.)

- [ ] **Step 2: Read from the cache inside `_draw_chart`**

Change `_draw_chart`'s signature to accept `cache: Dictionary, n: int` (after `c`), remove its own `var n := sim.coevo_history_len()`, and replace the two `sim.coevo_series(...)` calls:
- the auto-scale loop `for v in sim.coevo_series(s["key"]):` → `for v in (cache[s["key"]] as PackedFloat32Array):`
- the draw fetch `var arr := sim.coevo_series(key)` → `var arr: PackedFloat32Array = cache[key]`

Leave all drawing math unchanged.

- [ ] **Step 3: Headless boot check**

Run: `/opt/homebrew/bin/godot --headless --path game res://scenes/main.tscn --quit-after 150`
Expected: clean (the panel is hidden by default, so `_draw` isn't exercised headless — this just confirms the script parses; visual correctness is the manual pass in Task 5).

- [ ] **Step 4: Commit**

```bash
git add game/scripts/coevolution_panel.gd
git commit -m "perf(godot): cache co-evolution series once per draw

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: `population_panel` / `dit_panel` — reuse labels + phase-offset

**Files:**
- Modify: `game/scripts/population_panel.gd`, `game/scripts/dit_panel.gd`

- [ ] **Step 1: `population_panel` — reuse labels**

Replace the body of `_process` in `game/scripts/population_panel.gd` (keep the `const`/`@onready`/`_frame` as-is):

```gdscript
func _process(_delta: float) -> void:
	_frame += 1
	if _frame % REFRESH_EVERY != 0:
		return
	var stats: Array = sim.species_stats()
	_sync_label_count(stats.size())
	var children: Array = list.get_children()
	for i in stats.size():
		var s: Dictionary = stats[i]
		(children[i] as Label).text = "sp %d   n=%d   E=%.0f" % [int(s["species_id"]), int(s["count"]), float(s["mean_energy"])]

func _sync_label_count(want: int) -> void:
	var have: int = list.get_child_count()
	while have < want:
		var lbl := Label.new()
		lbl.add_theme_font_size_override("font_size", 12)
		list.add_child(lbl)
		have += 1
	while have > want:
		list.get_child(have - 1).queue_free()
		have -= 1
```

- [ ] **Step 2: `dit_panel` — reuse labels + phase-offset**

Replace `_process` in `game/scripts/dit_panel.gd` (the DIT panel shows a header + one row per species; reuse child 0 as the header, children 1.. as rows; phase-offset to `% REFRESH_EVERY == 3`):

```gdscript
func _process(_delta: float) -> void:
	if not bool(sim.env_active()):
		visible = false
		return
	visible = true
	_frame += 1
	if _frame % REFRESH_EVERY != 3:   # phase-offset from population_panel (== 0)
		return
	var stats: Array = sim.species_stats()
	_sync_label_count(stats.size() + 1)   # +1 header
	var children: Array = list.get_children()
	(children[0] as Label).text = "DIT  env-optimum = %.2f" % float(sim.env_optimum())
	for i in stats.size():
		var s: Dictionary = stats[i]
		(children[i + 1] as Label).text = "sp %d   match=%.2f" % [int(s["species_id"]), float(s["mean_technique_match"])]

func _sync_label_count(want: int) -> void:
	var have: int = list.get_child_count()
	while have < want:
		var lbl := Label.new()
		lbl.add_theme_font_size_override("font_size", 12)
		list.add_child(lbl)
		have += 1
	while have > want:
		list.get_child(have - 1).queue_free()
		have -= 1
```

(The header keeps font size 13 in the original; using 12 for all rows is a negligible cosmetic change. If exact parity matters, set `children[0].add_theme_font_size_override("font_size", 13)` when creating — optional.)

- [ ] **Step 3: Headless boot check**

Run: `/opt/homebrew/bin/godot --headless --path game res://scenes/main.tscn --quit-after 200`
Expected: clean — population panel updates without errors; DIT panel path exercised only when a scenario has `env_active` (dit-* scenarios), but the script must parse clean regardless.

- [ ] **Step 4: Commit**

```bash
git add game/scripts/population_panel.gd game/scripts/dit_panel.gd
git commit -m "perf(godot): reuse pop/DIT panel labels + phase-offset refresh

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Verification

- [ ] **Step 1: Rust binding gate**
```bash
rustup run stable cargo fmt --all --check
rustup run stable cargo clippy -p anabios-godot --all-targets -- -D warnings
rustup run stable cargo test -p anabios-core --test determinism
```
Expected: all PASS (core golden unchanged — anabios-core untouched).

- [ ] **Step 2: Headless boot across overlay-relevant scenarios**
Rebuild dylib, then boot: `rustup run stable cargo build -p anabios-godot && /opt/homebrew/bin/godot --headless --path game res://scenes/main.tscn --quit-after 300`
Expected: clean boot, no script errors.

- [ ] **Step 3: Manual visual pass (acceptance gate)**
Windowed (`/opt/homebrew/bin/godot --path game`): confirm — biome overlay animates & `[G]` toggles instantly; pheromone/env-optimum overlays render; module glyphs render on agents; `[Y]` co-evolution panel curves draw + scrub; population + DIT panels update. Note any visual regression (blocks acceptance; nothing else does).

---

## Self-Review

**Spec coverage:**
- Item 1 biome throttle + set_data → Task 1. ✅ (throttle every 6 + force on mode change + set_data byte buffer)
- Item 2 module_glyphs_all → Task 2 (binding + main.gd; old module_glyphs kept). ✅
- Item 3 coevolution series caching → Task 3 (fetch-once dict cache). ✅
- Item 4 pop/dit label reuse + phase-offset → Task 4. ✅
- Read-only binding / determinism-safe / no golden refresh → Global Constraints + Task 5 golden check. ✅
- Headless boot per task + manual visual acceptance → each task's boot step + Task 5 Step 3. ✅
- Out-of-scope (alive_bundle, SimPanel, removing module_glyphs) → absent. ✅

**Placeholder scan:** No TBD/TODO. Task 2's `ModuleType::COUNT`-vs-`9` and the `Array` write-back-vs-`Vec` notes are explicit "verify and pick the one that compiles" instructions with both concrete forms given — not placeholders. Task 3 Step 1's "match the existing call's positional args" is precise given the callee change in Step 2.

**Type consistency:** `module_glyphs_all() -> Array<PackedVector2Array>` used identically in Task 2 Step 1 (def) and Step 2 (consumption, `all[t]` → `PackedVector2Array`). `_sync_label_count(want: int)` consistent within Task 4. `cache: Dictionary` threaded from Task 3 Step 1 → Step 2. Biome `_last_mode`/`REDRAW_EVERY` consistent within Task 1.
