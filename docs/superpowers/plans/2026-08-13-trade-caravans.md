# Trade Caravans Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Draw animated trade caravans (carts hauling goods icons) shuttling between trade hubs in the Godot viewer, with cargo proportional to the real per-hub trade mix.

**Architecture:** Visual-only. The sim gains one inert `#[serde(skip)]` per-hub per-good tally (`hub_trade_tally`) that `trade_pass` increments and nothing in the sim ever reads — so determinism and golden hashes are untouched. A new Godot accessor surfaces the tally; a new `caravan_layer.gd` builds a fixed route network between nearby hubs and animates cart trains along it, allotting each cart's goods icon by the two endpoint hubs' trade mix.

**Tech Stack:** Rust (anabios-core + anabios-godot GDExtension), GDScript (Godot 4.5), glam `Vec2`, serde/bincode.

## Global Constraints

- **Visual-only / non-hashed invariant:** `hub_trade_tally` is `#[serde(skip)]`, written by `trade_pass` but NEVER read by any tick/decision logic (only by the Godot accessor). This is the safety property that keeps determinism and goldens unchanged — do not read it anywhere in the sim. No `FORMAT_VERSION` bump, no golden regeneration.
- Determinism: new sim helpers read no RNG; torus wrap via `crate::prelude::wrap_torus`; fixed scan order + stable tie-break (mirror `hub::best_hub_direction`).
- Good index order fixed: Salt=0, Obsidian=1, Amber=2, Spice=3 (`crate::resource::Good::index()`); `GOOD_COUNT = 4`.
- `cargo fmt` clean; `cargo clippy --all-targets -- -D warnings` clean (CI uses `--all-targets`); GDScript passes gdformat/gdlint.
- Viewer sprite textures are `flip_y()`-ed for the MultiMesh flipped-V axis; caravan carts/goods reuse that convention. Sprite/hub layers use plain no-shader `MultiMesh` + 9-way torus wrap clones (Metal-safe) — follow it.

---

### Task 1: `nearest_hub_index` helper

**Files:**
- Modify: `crates/anabios-core/src/hub.rs` (add fn + test)

**Interfaces:**
- Consumes: `TradeHub`, `crate::prelude::{Vec2, wrap_torus}` (already imported in hub.rs).
- Produces: `pub fn nearest_hub_index(hubs: &[TradeHub], pos: Vec2, world_size: f32) -> Option<usize>` — index of the nearest hub under torus wrap; `None` when `hubs` is empty. Deterministic (strict `<` keeps the earliest on ties).

- [ ] **Step 1: Write the failing test.** Append to the `mod tests` block in `crates/anabios-core/src/hub.rs`:

```rust
    #[test]
    fn nearest_hub_index_picks_closest_including_wrap() {
        let ws = 100.0;
        let hubs = vec![
            TradeHub { pos: Vec2::new(10.0, 50.0), cell: 0, goods: vec![] },
            TradeHub { pos: Vec2::new(60.0, 50.0), cell: 1, goods: vec![] },
        ];
        // x=95 is nearest to hub 0 (x=10) ACROSS the seam (dist 15), not hub 1 (dist 35).
        assert_eq!(nearest_hub_index(&hubs, Vec2::new(95.0, 50.0), ws), Some(0));
        assert_eq!(nearest_hub_index(&hubs, Vec2::new(55.0, 50.0), ws), Some(1));
        assert_eq!(nearest_hub_index(&[], Vec2::new(0.0, 0.0), ws), None);
    }
```

- [ ] **Step 2: Run it to verify failure.**

Run: `cd crates/anabios-core && cargo test --lib hub::nearest_hub_index`
Expected: FAIL — `nearest_hub_index` not found.

- [ ] **Step 3: Implement.** Add above the `#[cfg(test)]` block in `hub.rs`:

```rust
/// Index of the nearest trade hub to `pos` under torus wrap, or `None` when there
/// are no hubs. Deterministic (fixed order, strict `<` keeps the earliest on ties);
/// reads no RNG.
pub fn nearest_hub_index(hubs: &[TradeHub], pos: Vec2, world_size: f32) -> Option<usize> {
    let world = Vec2::splat(world_size);
    let half = Vec2::splat(world_size * 0.5);
    let mut best: Option<(f32, usize)> = None;
    for (i, h) in hubs.iter().enumerate() {
        let off = wrap_torus(h.pos - pos + half, world) - half;
        let d2 = off.length_squared();
        if best.is_none_or(|(bd, _)| d2 < bd) {
            best = Some((d2, i));
        }
    }
    best.map(|(_, i)| i)
}
```

- [ ] **Step 4: Run it to verify pass.**

Run: `cd crates/anabios-core && cargo test --lib hub::nearest_hub_index`
Expected: PASS. Then `cargo fmt` + `cargo clippy -p anabios-core --all-targets -- -D warnings`.

- [ ] **Step 5: Commit.**

```bash
git add crates/anabios-core/src/hub.rs
git commit -m "feat(hub): nearest_hub_index helper (torus-aware)"
```

---

### Task 2: `World.hub_trade_tally` + per-swap tally in `trade_pass`

**Files:**
- Modify: `crates/anabios-core/src/world.rs` (field + constructor init)
- Modify: `crates/anabios-core/src/interact.rs` (`trade_pass` self-heal sizing + increment; tests)

**Interfaces:**
- Consumes: `crate::hub::nearest_hub_index` (Task 1); `crate::resource::GOOD_COUNT`; the `give`/`recv` good indices already computed in `trade_pass`.
- Produces: `World.hub_trade_tally: Vec<[u64; crate::resource::GOOD_COUNT]>` (`#[serde(skip)]`, index-aligned to `trade_hubs`), incremented per successful swap at the nearest hub.

- [ ] **Step 1: Write the failing tests.** Append to the `mod tests` block in `crates/anabios-core/src/interact.rs` (this module already imports `World`, `Vec2`, `Genome`, and seeds hubs in its trade tests):

```rust
    #[test]
    fn trade_pass_tallies_goods_to_nearest_hub() {
        use crate::hub::TradeHub;
        use crate::resource::Good;
        let mut w = World::new(5);
        w.resources_enabled = true;
        let pos = Vec2::new(300.0, 300.0);
        let a = w.spawn_agent(pos, Genome::neutral());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
        w.push_species(w.agents.genome[b as usize], None);
        w.add_to_species(b, 1);
        w.remove_from_species(b, 0);
        w.agents.inventory[a as usize][Good::Salt.index()] = 5.0;
        w.agents.inventory[b as usize][Good::Obsidian.index()] = 5.0;
        w.trade_hubs = vec![TradeHub { pos, cell: 0, goods: vec![] }];
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        w.resize_scratch();
        crate::sense::sense_all(
            &w.agents, &w.biome, &w.pheromones, &w.spatial, &w.codex.hostility,
            &mut w.sensors, w.world_size, false,
        );
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        trade_pass(&mut w, &alive);
        // A bilateral Salt<->Obsidian swap moves both goods; both counters at hub 0 rise.
        assert_eq!(w.hub_trade_tally.len(), 1);
        assert!(w.hub_trade_tally[0][Good::Salt.index()] >= 1);
        assert!(w.hub_trade_tally[0][Good::Obsidian.index()] >= 1);
    }

    #[test]
    fn hub_trade_tally_is_not_hashed_and_survives_reload() {
        use crate::hub::TradeHub;
        use crate::resource::Good;
        use crate::snapshot::{load_from_bytes, save_to_bytes, state_hash};
        let mut w = World::new(5);
        w.resources_enabled = true;
        let pos = Vec2::new(300.0, 300.0);
        let a = w.spawn_agent(pos, Genome::neutral());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
        w.push_species(w.agents.genome[b as usize], None);
        w.add_to_species(b, 1);
        w.remove_from_species(b, 0);
        w.agents.inventory[a as usize][Good::Salt.index()] = 5.0;
        w.agents.inventory[b as usize][Good::Obsidian.index()] = 5.0;
        w.trade_hubs = vec![TradeHub { pos, cell: 0, goods: vec![] }];
        for _ in 0..20 {
            crate::tick::step(&mut w);
        }
        assert!(w.hub_trade_tally.iter().any(|t| t.iter().any(|&c| c > 0)), "tally populated");
        // Serde-skip: the tally is not in state_hash, so a reload (which drops it)
        // must hash identically, and must step without panicking (self-heal sizing).
        let bytes = save_to_bytes(&w).expect("save");
        let mut reload = load_from_bytes(&bytes).expect("load");
        assert_eq!(state_hash(&w), state_hash(&reload), "tally must not affect state_hash");
        crate::tick::step(&mut reload);
    }
```

- [ ] **Step 2: Run to verify failure.**

Run: `cd crates/anabios-core && cargo test --lib interact::tests::trade_pass_tallies interact::tests::hub_trade_tally`
Expected: FAIL — `World` has no field `hub_trade_tally`.

- [ ] **Step 3: Add the field.** In `crates/anabios-core/src/world.rs`, next to the `#[serde(skip)] pub trade_routes: ...` declaration (~line 309), add:

```rust
    /// Per-hub, per-good count of goods that changed hands at that hub, index-aligned
    /// to `trade_hubs`. Viewer scratch ONLY — never read by the simulation, so it is
    /// `#[serde(skip)]` (not serialized, not in `state_hash`) like `trade_routes`.
    /// `trade_pass` self-heals its length to `trade_hubs.len()`, so it survives a
    /// snapshot load (which leaves it empty) without panicking.
    #[serde(skip)]
    pub hub_trade_tally: Vec<[u64; crate::resource::GOOD_COUNT]>,
```

- [ ] **Step 4: Initialize it in the constructor.** In `World::new`, next to `trade_routes: Vec::new(),` (~line 432), add:

```rust
            hub_trade_tally: Vec::new(),
```

- [ ] **Step 5: Self-heal sizing + increment in `trade_pass`.** In `crates/anabios-core/src/interact.rs`, in `trade_pass`, right after the function opens (before the `for &id in alive_ids` loop), add:

```rust
    // Keep the (serde-skipped) per-hub tally sized to the hub set: empty after a
    // snapshot load, and stale if hubs ever change. Cheap no-op once sized.
    if world.hub_trade_tally.len() != world.trade_hubs.len() {
        world.hub_trade_tally = vec![[0u64; crate::resource::GOOD_COUNT]; world.trade_hubs.len()];
    }
```

Then, immediately AFTER `world.total_trades += 1;` (the successful-swap bookkeeping), add:

```rust
        // Viewer scratch: tally the traded good(s) to the nearest hub (the swap
        // happened within HUB_TRADE_RANGE of one). Never read by the sim.
        if let Some(h) =
            crate::hub::nearest_hub_index(&world.trade_hubs, world.agents.position[i], world.world_size)
        {
            world.hub_trade_tally[h][give] += 1;
            if recv != give {
                world.hub_trade_tally[h][recv] += 1;
            }
        }
```

(`give`/`recv` are the `usize` good indices already bound above in `trade_pass`; `i` is the initiator's index.)

- [ ] **Step 6: Run to verify pass.**

Run: `cd crates/anabios-core && cargo test --lib interact::`
Expected: PASS (the two new tests plus the existing interact unit tests). Then `cargo fmt` + `cargo clippy -p anabios-core --all-targets -- -D warnings`.

- [ ] **Step 7: Commit.**

```bash
git add crates/anabios-core/src/world.rs crates/anabios-core/src/interact.rs
git commit -m "feat(hub): inert per-hub per-good trade tally (viewer scratch)"
```

---

### Task 3: Godot `hub_trade_tally()` accessor

**Files:**
- Modify: `crates/anabios-godot/src/lib.rs`

**Interfaces:**
- Consumes: `w.hub_trade_tally: Vec<[u64; 4]>`.
- Produces: GDScript `sim.hub_trade_tally() -> Array` where each element is a `PackedInt32Array` of `GOOD_COUNT` counts, in the same order as `sim.trade_hubs()`. Empty until the first trade.

- [ ] **Step 1: Add the accessor.** In `crates/anabios-godot/src/lib.rs`, next to the `trade_hubs` accessor, add:

```rust
    /// Per-hub trade tallies: one `PackedInt32Array` of GOOD_COUNT counts per hub,
    /// in the same order as `trade_hubs()`. Empty until trading has occurred. This
    /// is viewer-only scratch (not part of the simulation state).
    #[func]
    fn hub_trade_tally(&self) -> Array<PackedInt32Array> {
        let mut out = Array::new();
        let Some(w) = self.inner.as_ref() else {
            return out;
        };
        for counts in &w.hub_trade_tally {
            let mut row = PackedInt32Array::new();
            for &c in counts.iter() {
                row.push(c as i32);
            }
            out.push(&row);
        }
        out
    }
```

If `Array<PackedInt32Array>` needs an import beyond what `trade_hubs` already uses, add it (the file already returns `PackedInt32Array` and `Array<VarDictionary>` elsewhere, so both are in scope).

- [ ] **Step 2: Build to verify.**

Run: `cd crates/anabios-godot && cargo build`
Expected: PASS. Then `cargo fmt` + `cargo clippy -p anabios-godot --all-targets -- -D warnings`.

- [ ] **Step 3: Commit.**

```bash
git add crates/anabios-godot/src/lib.rs
git commit -m "feat(godot): expose hub_trade_tally() to the viewer"
```

---

### Task 4: `build_cart()` caravan sprite

**Files:**
- Modify: `game/scripts/building_sprites.gd` (add cart sprite)
- Modify: `game/scripts/test_building_sprites.gd` (cover it)

**Interfaces:**
- Consumes: `ApeSprites._build_cell`, `ApeSprites.PAL`.
- Produces: `static func build_cart_image() -> Image` and `static func build_cart() -> ImageTexture` — a 16×16 covered-wagon/cart block-art sprite, `flip_y()`-ed like the other MultiMesh textures.

- [ ] **Step 1: Add the cart sprite.** In `game/scripts/building_sprites.gd`, after the goods-icon builders, add a cart block list + builders. Confirm each palette char exists in `ApeSprites.PAL` (grep it first); substitute the nearest existing key if not:

```gdscript
# 16x16 caravan cart: wooden body, pale canvas top, two dark wheels. Small so it
# reads as a vehicle beside the bigger hub buildings.
const _CART_BLOCKS: Array = [
	[3, 6, 10, 4, "t"],   # wooden body
	[3, 6, 10, 1, "b"],   # top rail of body
	[4, 3, 8, 3, "B"],    # pale canvas cover
	[4, 3, 8, 1, "W"],    # canvas highlight
	[3, 10, 2, 2, "K"],   # left wheel
	[11, 10, 2, 2, "K"],  # right wheel
	[5, 9, 6, 1, "d"],    # axle shadow
]


static func build_cart_image() -> Image:
	var img: Image = ApeSprites._build_cell(_CART_BLOCKS)
	img.flip_y()
	return img


static func build_cart() -> ImageTexture:
	return ImageTexture.create_from_image(build_cart_image())
```

- [ ] **Step 2: Extend the sprite self-test.** In `game/scripts/test_building_sprites.gd`, add a check that the cart builds a 16×16 texture, matching the file's existing harness style:

```gdscript
	var cart := Buildings.build_cart()
	assert(cart != null and cart.get_width() == 16 and cart.get_height() == 16)
```

- [ ] **Step 3: Run the sprite self-test headless.**

Run: `cd game && godot --headless --rendering-driver dummy --path . -s res://scripts/test_building_sprites.gd --quit-after 1`
Expected: prints the file's success line (e.g. `test_building_sprites: all passed`), no `SCRIPT ERROR`/`Parse Error`. Then gdformat/gdlint both files.

- [ ] **Step 4: Commit.**

```bash
git add game/scripts/building_sprites.gd game/scripts/test_building_sprites.gd
git commit -m "feat(viewer): caravan cart sprite"
```

---

### Task 5: `caravan_layer.gd` — route network + animated caravans

**Files:**
- Create: `game/scripts/caravan_layer.gd`
- Modify: `game/scripts/main.gd` (instantiate the layer)

**Interfaces:**
- Consumes: `sim.trade_hubs()` (`{pos, goods}` per hub), `sim.hub_trade_tally()` (`PackedInt32Array` per hub, index-aligned), `sim.world_size()`; `Buildings.build_cart()`, `Buildings.build_good(idx)`, `Buildings.GOOD_COUNT`.
- Produces: a `CaravanLayer` Node2D drawing faint dashed route lines + animated cart trains (each cart carrying a goods icon allotted by the route's real trade mix), torus-wrapped.

- [ ] **Step 1: Create the layer.** Write `game/scripts/caravan_layer.gd`. Carts + goods icons render through plain no-shader `MultiMesh` layers with 9-way wrap clones (like `hub_layer.gd`), animated each `_process` frame; the faint route lines are drawn in `_draw()` at all 9 torus offsets (static — redrawn only when the route network is rebuilt). Cargo per route is recomputed from `hub_trade_tally()` on a periodic cadence.

```gdscript
extends Node2D

# Trade caravans: a fixed route network links each hub to its nearest neighbours,
# and short cart trains shuttle along every route. Each cart carries a trade-good
# icon; a route's cart cargo is apportioned (largest-remainder) to the summed
# per-good trade tally of its two endpoint hubs, so busy Salt routes haul mostly
# Salt. Pure presentation over read-only sim state; the sim is unchanged.

const Buildings = preload("res://scripts/building_sprites.gd")

const CARAVAN_NEIGHBORS := 2      # edges added per hub (undirected, deduped)
const CARTS_PER_ROUTE := 3
const TRAVERSE_PERIOD := 6.0      # seconds for one out-and-back along a route
const CART_GAP_FRAC := 0.06       # fractional spacing between carts in a train
const CART_SCALE := 11.0
const GOOD_SCALE := 7.0
const GOOD_DY := -9.0             # goods icon rides above the cart
const REDRAW_MIX_EVERY := 40
const LINE_COLOR := Color(0.85, 0.80, 0.55, 0.18)
const LINE_DASH := 8.0

var _cart_mmi: MultiMeshInstance2D
var _good_mmis: Array[MultiMeshInstance2D] = []
var _hubs: Array = []
var _routes: Array = []           # each: {a, b, pa: Vector2, pb: Vector2, cargo: PackedInt32Array}
var _t: float = 0.0
var _frame: int = 0

@onready var sim = get_node("/root/Main/Simulation")


func _ready() -> void:
	_cart_mmi = _make_layer("Caravan_Cart", Buildings.build_cart(), 2)
	for g in Buildings.GOOD_COUNT:
		_good_mmis.append(_make_layer("Caravan_Good_%d" % g, Buildings.build_good(g), 3))
	_make_wrap_clones()


func _make_layer(pname: String, tex: ImageTexture, z: int) -> MultiMeshInstance2D:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_2D
	mm.use_colors = true
	mm.mesh = QuadMesh.new()
	var mmi := MultiMeshInstance2D.new()
	mmi.name = pname
	mmi.multimesh = mm
	mmi.texture = tex
	mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	mmi.z_index = z
	add_child(mmi)
	return mmi


func _make_wrap_clones() -> void:
	var world: float = sim.world_size()
	for src in [_cart_mmi] + _good_mmis:
		for gy in range(-1, 2):
			for gx in range(-1, 2):
				if gx == 0 and gy == 0:
					continue
				var clone := MultiMeshInstance2D.new()
				clone.multimesh = src.multimesh
				clone.texture = src.texture
				clone.texture_filter = src.texture_filter
				clone.z_index = src.z_index
				clone.position = Vector2(gx * world, gy * world)
				add_child(clone)


func _min_image(from: Vector2, to: Vector2, world: float) -> Vector2:
	var d := to - from
	d.x = fposmod(d.x + world * 0.5, world) - world * 0.5
	d.y = fposmod(d.y + world * 0.5, world) - world * 0.5
	return from + d


# Nearest-neighbour route network over the hub positions (torus-aware), each hub
# linked to its CARAVAN_NEIGHBORS closest others; undirected edges deduped.
func _build_routes() -> void:
	_routes.clear()
	var n := _hubs.size()
	if n < 2:
		return
	var world: float = sim.world_size()
	var seen := {}
	for i in n:
		var pi: Vector2 = _hubs[i]["pos"]
		var dists: Array = []
		for j in n:
			if j == i:
				continue
			var pj := _min_image(pi, _hubs[j]["pos"], world)
			dists.append({"j": j, "d": pi.distance_squared_to(pj), "pj": pj})
		dists.sort_custom(func(x, y): return x["d"] < y["d"])
		for k in mini(CARAVAN_NEIGHBORS, dists.size()):
			var j: int = dists[k]["j"]
			var key := "%d-%d" % [mini(i, j), maxi(i, j)]
			if seen.has(key):
				continue
			seen[key] = true
			_routes.append({
				"a": i, "b": j, "pa": pi, "pb": dists[k]["pj"],
				"cargo": PackedInt32Array()
			})


# Apportion CARTS_PER_ROUTE carts to goods by largest-remainder over the summed
# per-good tally of the route's two endpoint hubs. Empty (all -1) until trades.
func _recompute_cargo(tallies: Array) -> void:
	for r in _routes:
		var sums := PackedInt32Array()
		sums.resize(Buildings.GOOD_COUNT)
		var total := 0
		for hub_idx in [r["a"], r["b"]]:
			if hub_idx < tallies.size():
				var t: PackedInt32Array = tallies[hub_idx]
				for g in mini(t.size(), Buildings.GOOD_COUNT):
					sums[g] += t[g]
					total += t[g]
		var cargo := PackedInt32Array()
		cargo.resize(CARTS_PER_ROUTE)
		if total <= 0:
			for c in CARTS_PER_ROUTE:
				cargo[c] = -1
			r["cargo"] = cargo
			continue
		# Largest-remainder apportionment.
		var alloc := PackedInt32Array()
		alloc.resize(Buildings.GOOD_COUNT)
		var rema: Array = []
		var used := 0
		for g in Buildings.GOOD_COUNT:
			var exact := float(sums[g]) * float(CARTS_PER_ROUTE) / float(total)
			var base := int(floor(exact))
			alloc[g] = base
			used += base
			rema.append({"g": g, "r": exact - float(base)})
		rema.sort_custom(func(x, y): return x["r"] > y["r"])
		var leftover := CARTS_PER_ROUTE - used
		for m in leftover:
			alloc[rema[m % rema.size()]["g"]] += 1
		var idx := 0
		for g in Buildings.GOOD_COUNT:
			for _c in alloc[g]:
				if idx < CARTS_PER_ROUTE:
					cargo[idx] = g
					idx += 1
		r["cargo"] = cargo


func _process(delta: float) -> void:
	_t += delta
	_frame += 1
	if _routes.is_empty():
		_hubs = sim.trade_hubs()
		if _hubs.is_empty():
			return
		_build_routes()
		queue_redraw()  # paint the (static) route lines once
	if _frame % REDRAW_MIX_EVERY == 0:
		_recompute_cargo(sim.hub_trade_tally())
	_animate()


# Place each route's cart train along its segment with a ping-pong lead position,
# followers trailing by CART_GAP_FRAC; write per-instance transforms.
func _animate() -> void:
	var cart_xf: Array = []
	var good_xf: Array = []
	for g in Buildings.GOOD_COUNT:
		good_xf.append([])
	var lead := pingpong(_t / TRAVERSE_PERIOD, 1.0)
	for r in _routes:
		var pa: Vector2 = r["pa"]
		var pb: Vector2 = r["pb"]
		var cargo: PackedInt32Array = r["cargo"]
		for c in CARTS_PER_ROUTE:
			var f: float = clampf(lead - float(c) * CART_GAP_FRAC, 0.0, 1.0)
			var p := pa.lerp(pb, f)
			cart_xf.append(Transform2D(0.0, Vector2(CART_SCALE, CART_SCALE), 0.0, p))
			if c < cargo.size():
				var gi: int = cargo[c]
				if gi >= 0 and gi < Buildings.GOOD_COUNT:
					var gp := p + Vector2(0.0, GOOD_DY)
					good_xf[gi].append(
						Transform2D(0.0, Vector2(GOOD_SCALE, GOOD_SCALE), 0.0, gp)
					)
	_write(_cart_mmi.multimesh, cart_xf)
	for g in Buildings.GOOD_COUNT:
		_write(_good_mmis[g].multimesh, good_xf[g])


func _write(mm: MultiMesh, xfs: Array) -> void:
	var m := xfs.size()
	if m > mm.instance_count:
		mm.instance_count = m
	mm.visible_instance_count = m
	for i in m:
		mm.set_instance_transform_2d(i, xfs[i])
		mm.set_instance_color(i, Color(1, 1, 1))


# Faint dashed route lines, drawn at all 9 torus offsets so seam-crossing routes
# read correctly. Static: repainted only when the route network is (re)built.
func _draw() -> void:
	if _routes.is_empty():
		return
	var world: float = sim.world_size()
	for gy in range(-1, 2):
		for gx in range(-1, 2):
			var off := Vector2(gx * world, gy * world)
			for r in _routes:
				draw_dashed_line(
					(r["pa"] as Vector2) + off, (r["pb"] as Vector2) + off,
					LINE_COLOR, 1.0, LINE_DASH
				)
```

Note: verify `sim.world_size()` and `sim.trade_hubs()`/`sim.hub_trade_tally()` names against `hub_layer.gd` (it uses `world_size`/`trade_hubs`); adjust if renamed. `pingpong`, `fposmod`, `mini`, `maxi`, `clampf` are Godot 4 globals.

- [ ] **Step 2: Wire into the scene.** In `game/scripts/main.gd`, right after the hub layer is added (the `hub_layer` block near lines 208-211), add:

```gdscript
	var caravan_layer = preload("res://scripts/caravan_layer.gd").new()
	caravan_layer.name = "CaravanLayer"
	add_child(caravan_layer)
	move_child(caravan_layer, module_layers.get_index())
```

- [ ] **Step 3: Headless boot check.**

Run: `cd game && godot --headless res://scenes/main.tscn --quit-after 180 2>&1 | tail -30`
Expected: no `SCRIPT ERROR` / `Parse Error` / stack traces referencing `caravan_layer` or `main`; clean quit. (The default boot may load a hubless scenario, so caravans may draw nothing — you are verifying it loads and runs cleanly, not that caravans appear.)

- [ ] **Step 4: Visual confirmation (recommended).** Windowed run on the hub showcase and confirm carts shuttle between hubs along faint routes, carrying goods icons:

```bash
ANABIOS_SCENARIO=res://../scenarios/trade-hubs.toml ANABIOS_SEED=424242 godot --path game res://scenes/main.tscn
```

- [ ] **Step 5: gdformat/gdlint + commit.**

```bash
git add game/scripts/caravan_layer.gd game/scripts/main.gd
git commit -m "feat(viewer): caravan layer — carts hauling goods between hubs"
```

---

## Self-Review

**Spec coverage:**
- Inert non-hashed per-hub per-good tally → Task 2 (`#[serde(skip)] hub_trade_tally`, written not read; self-heal sizing; state_hash-unchanged + reload tests).
- Nearest-hub attribution → Task 1 (`nearest_hub_index`) used in Task 2.
- Accessor → Task 3.
- Route network (nearest ~2, deduped, torus) → Task 5 `_build_routes`.
- Animated cart trains + ping-pong + wrap → Task 5 `_animate` + wrap clones.
- Faint route line at 9 offsets → Task 5 `_draw`.
- Cargo proportional to two endpoint hubs' real trade (largest-remainder) → Task 5 `_recompute_cargo`.
- Cart sprite → Task 4 `build_cart`; goods icons reuse `build_good`.
- No FORMAT_VERSION bump / no golden regen → nothing changes serialized state (Task 2 field is serde-skipped); no task regenerates goldens.
- Godot headless boot + sprite self-test → Task 5 Step 3, Task 4 Step 3.
All spec sections map to a task.

**Placeholder scan:** No TBD/"handle edge cases"/"similar to Task N". Every code step carries full code. The two "verify names / substitute palette char if needed" notes are explicit verification instructions with named fallbacks, not missing content.

**Type consistency:** `hub_trade_tally: Vec<[u64; GOOD_COUNT]>` defined in Task 2, consumed by Task 3 accessor (`as i32` per count) and Task 5 (`PackedInt32Array` per hub). `nearest_hub_index(&[TradeHub], Vec2, f32) -> Option<usize>` defined Task 1, called Task 2. `build_cart()` defined Task 4, called Task 5. `_routes` entry shape (`a,b,pa,pb,cargo`) consistent across `_build_routes`/`_recompute_cargo`/`_animate`/`_draw`. `CARTS_PER_ROUTE`/`GOOD_COUNT` used consistently.
