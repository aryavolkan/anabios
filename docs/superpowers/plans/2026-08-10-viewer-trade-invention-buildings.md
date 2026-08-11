# Trade & Invention Landmark Buildings — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Draw new pixel-art landmark buildings in each settlement village that announce, from real sim state, its most-advanced held invention and whether it is a trade hub.

**Architecture:** A new `building_sprites.gd` holds the 12 building block-art definitions, the invention-key→building map, and pure selection helpers (all static, unit-tested headlessly). `settlement_layer.gd` is extended to join `settlement_sites()` with `species_stats()` + `invention_catalog()` each throttled redraw, decide each village's signature landmark(s) and trade building, and draw them on their own deterministic ring using plain single-texture MultiMesh2D layers (no shader — deliberately off the Metal atlas-corruption path fixed in commit `7f0598b`). Reuses the existing village-memory linger/fade so buildings fade in on adoption and linger on collapse.

**Tech Stack:** Godot 4.7 / GDScript. Art via the shared `ApeSprites._build_cell(blocks)` block painter (16×16, auto 1px outline, `PAL` palette). Headless tests via `SceneTree` scripts run with `godot --headless --rendering-driver dummy`.

## Global Constraints

- **Presentation only.** No `anabios-core` changes, no new `#[func]` bindings, no new sim signals. Everything needed is already exposed: `settlement_sites()`, `species_stats()`, `invention_catalog()`, `market_colors()`, `resources_active()`, `biome_resolution()`, `world_size()`.
- **No determinism/golden impact** — do not run or modify the determinism/golden suite; it is unaffected.
- **Metal-safe rendering:** buildings are static, drawn as plain single-texture `MultiMeshInstance2D` with `TEXTURE_FILTER_NEAREST` and **no ShaderMaterial**. Never route a building texture through a `canvas_item` shader.
- **Palette:** reuse `ApeSprites.PAL` keys only, so buildings match the hut/farm look. Building textures are `flip_y()`-ed before use (QuadMesh flipped-V convention), exactly like `settlement_layer._hut_texture()`.
- **Gates before pushing:** `gdformat --check` and `gdlint` clean on every changed/new `.gd` file. Keep every file under the repo's 1000-line lint ceiling.
- **Invention keys (verbatim) and eras:** `stone_tools`(1), `fire`(1), `farming`(2), `metalworking`(2), `writing`(3), `medicine`(3), `husbandry`(3), `machinery`(4), `electricity`(4), `nuclear_power`(4).

---

### Task 1: Building sprite module (`building_sprites.gd`)

**Files:**
- Create: `game/scripts/building_sprites.gd`
- Test: `game/scripts/test_building_sprites.gd`

**Interfaces:**
- Consumes: `ApeSprites._build_cell(blocks: Array) -> Image` and `ApeSprites.PAL` (existing).
- Produces:
  - `enum { MARKET, WAREHOUSE, STONE_TOOLS, FIRE, FARMING, METALWORKING, WRITING, MEDICINE, HUSBANDRY, MACHINERY, ELECTRICITY, NUCLEAR }` and `const KIND_COUNT := 12`.
  - `const NAMES: PackedStringArray` (12, parallel to the enum).
  - `const INVENTION_BUILDING: Dictionary` — invention key (String) → building kind (int), all 10 keys.
  - `static func build_image(kind: int) -> Image` — the 16×16 painted, `flip_y()`-ed cell.
  - `static func build(kind: int) -> ImageTexture` — `ImageTexture.create_from_image(build_image(kind))`.
  - `static func building_for_invention(key: String) -> int` — `INVENTION_BUILDING.get(key, -1)`.

- [ ] **Step 1: Write the failing test**

Create `game/scripts/test_building_sprites.gd`:

```gdscript
extends SceneTree
# Headless checks for the building sprite module. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_building_sprites.gd
# Exits 0 on success, 1 on the first failed assertion.

const B = preload("res://scripts/building_sprites.gd")
const INV_KEYS := [
	"stone_tools", "fire", "farming", "metalworking", "writing",
	"medicine", "husbandry", "machinery", "electricity", "nuclear_power"
]


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		quit(1)


func _init() -> void:
	# Enum/name/count coherence.
	_check(B.KIND_COUNT == 12, "12 building kinds")
	_check(B.NAMES.size() == B.KIND_COUNT, "NAMES parallels the enum")
	# Every kind builds a 16x16 image with at least one opaque (figure) pixel.
	for k in B.KIND_COUNT:
		var img: Image = B.build_image(k)
		_check(img.get_width() == 16 and img.get_height() == 16, "%s is 16x16" % B.NAMES[k])
		var opaque := 0
		for y in 16:
			for x in 16:
				if img.get_pixel(x, y).a > 0.5:
					opaque += 1
		_check(opaque >= 8, "%s has a visible figure" % B.NAMES[k])
	# The invention map covers all ten keys and points at real kinds.
	for key in INV_KEYS:
		var kind: int = B.building_for_invention(key)
		_check(kind >= 0 and kind < B.KIND_COUNT, "invention '%s' maps to a building" % key)
	# Unknown keys return -1 (no building).
	_check(B.building_for_invention("not_a_thing") == -1, "unknown key -> -1")
	print("test_building_sprites: all passed")
	quit(0)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `godot --headless --rendering-driver dummy --path game -s res://scripts/test_building_sprites.gd`
Expected: FAIL — `building_sprites.gd` does not exist / preload error.

- [ ] **Step 3: Write minimal implementation**

Create `game/scripts/building_sprites.gd`. Block lists are `[x, y, w, h, PAL_key]`, painted back-to-front, auto-outlined by `_build_cell`. These are recognizable starters; refine them visually in Task 3's capture pass.

```gdscript
extends RefCounted
# Static landmark-building sprites for the settlement layer: one 16x16 block-art
# texture per trade/invention building, built with the shared ApeSprites cell
# painter (auto 1px outline, PAL palette) so buildings match the hut/farm look.
# Textures are flip_y()-ed for the MultiMesh QuadMesh's flipped V axis, same as
# settlement_layer's hut/farm textures. Buildings never animate and are drawn
# through a plain (no-shader) MultiMesh, keeping them off the Metal atlas path.

const ApeSprites = preload("res://scripts/ape_sprites.gd")

enum {
	MARKET, WAREHOUSE, STONE_TOOLS, FIRE, FARMING, METALWORKING,
	WRITING, MEDICINE, HUSBANDRY, MACHINERY, ELECTRICITY, NUCLEAR
}
const KIND_COUNT := 12
const NAMES: PackedStringArray = [
	"Market", "Warehouse", "StoneTools", "Fire", "Farming", "Metalworking",
	"Writing", "Medicine", "Husbandry", "Machinery", "Electricity", "Nuclear"
]

# Invention key (from invention_catalog / species_stats.adopted_inventions) ->
# building kind. Keys are verbatim from anabios-core invention/mod.rs.
const INVENTION_BUILDING := {
	"stone_tools": STONE_TOOLS,
	"fire": FIRE,
	"farming": FARMING,
	"metalworking": METALWORKING,
	"writing": WRITING,
	"medicine": MEDICINE,
	"husbandry": HUSBANDRY,
	"machinery": MACHINERY,
	"electricity": ELECTRICITY,
	"nuclear_power": NUCLEAR,
}

# 16x16 block lists per kind, indexed by the enum.
const _BLOCKS: Array = [
	# MARKET — striped awning stall with goods baskets
	[
		[3, 10, 10, 4, "B"], [3, 6, 1, 5, "b"], [12, 6, 1, 5, "b"],
		[2, 5, 12, 1, "R"], [2, 6, 12, 1, "W"],
		[4, 11, 2, 2, "o"], [7, 11, 2, 2, "y"], [10, 11, 2, 2, "n"],
	],
	# WAREHOUSE — broad storehouse, big doors, stacked crates
	[
		[2, 6, 12, 8, "B"], [1, 4, 14, 2, "b"], [3, 3, 10, 1, "b"],
		[6, 8, 4, 6, "K"], [8, 8, 1, 6, "b"],
		[3, 12, 2, 2, "r"], [12, 12, 2, 2, "r"],
	],
	# STONE_TOOLS — worked-stone boulder + leaning tool rack
	[
		[3, 9, 5, 5, "g"], [4, 8, 3, 1, "G"],
		[10, 5, 1, 9, "b"], [10, 5, 3, 1, "s"], [11, 6, 2, 1, "B"],
		[4, 13, 1, 1, "s"], [8, 13, 1, 1, "s"],
	],
	# FIRE — stone hearth ring with a flame
	[
		[4, 11, 8, 3, "g"], [4, 10, 1, 1, "G"], [11, 10, 1, 1, "G"],
		[5, 11, 6, 1, "b"],
		[6, 8, 1, 2, "o"], [7, 6, 2, 5, "R"], [7, 5, 2, 2, "o"], [8, 4, 1, 2, "y"],
	],
	# FARMING — round grain silo (distinct from the generic farm patch)
	[
		[5, 5, 6, 9, "T"], [5, 5, 6, 1, "t"],
		[6, 3, 4, 1, "B"], [5, 4, 6, 1, "B"],
		[5, 8, 6, 1, "m"], [5, 11, 6, 1, "m"], [7, 11, 2, 3, "b"],
	],
	# METALWORKING — forge: chimney, fire mouth, anvil
	[
		[3, 8, 7, 6, "g"], [3, 8, 7, 1, "d"],
		[4, 3, 3, 5, "d"], [4, 2, 2, 1, "G"], [5, 1, 2, 1, "s"],
		[4, 10, 3, 3, "o"], [5, 11, 1, 1, "y"],
		[11, 10, 3, 2, "d"], [12, 9, 1, 1, "d"], [11, 12, 1, 2, "d"],
	],
	# WRITING — inscribed standing stele
	[
		[5, 12, 6, 2, "g"], [6, 3, 4, 9, "T"], [6, 3, 4, 1, "t"], [5, 2, 6, 1, "B"],
		[7, 5, 2, 1, "d"], [7, 7, 2, 1, "d"], [7, 9, 2, 1, "d"],
	],
	# MEDICINE — apothecary hut with hung herb bundles
	[
		[4, 7, 8, 7, "B"], [3, 5, 10, 2, "b"], [5, 4, 6, 1, "b"],
		[7, 10, 2, 4, "K"],
		[4, 7, 1, 3, "o"], [4, 7, 1, 1, "t"], [11, 7, 1, 3, "o"], [11, 7, 1, 1, "t"],
	],
	# HUSBANDRY — fenced corral with a penned animal
	[
		[2, 13, 13, 1, "m"],
		[2, 7, 1, 6, "B"], [5, 7, 1, 6, "B"], [8, 7, 1, 6, "B"], [11, 7, 1, 6, "B"], [14, 7, 1, 6, "B"],
		[2, 8, 13, 1, "b"], [2, 11, 13, 1, "b"],
		[6, 9, 3, 2, "t"], [9, 9, 1, 1, "t"],
	],
	# MACHINERY — workshop with a waterwheel
	[
		[3, 7, 6, 7, "B"], [2, 6, 8, 1, "b"],
		[9, 5, 6, 6, "d"], [11, 5, 2, 6, "s"], [9, 7, 6, 2, "s"], [11, 7, 2, 2, "g"],
		[9, 12, 6, 2, "s"],
	],
	# ELECTRICITY — glowing lamp post / pylon
	[
		[7, 4, 2, 10, "d"], [5, 13, 6, 1, "g"], [4, 5, 8, 1, "d"],
		[5, 2, 6, 3, "y"], [6, 1, 4, 1, "W"], [4, 3, 1, 1, "O"], [11, 3, 1, 1, "O"],
	],
	# NUCLEAR — cooling tower with steam
	[
		[4, 6, 8, 8, "G"], [5, 9, 6, 2, "g"], [4, 13, 8, 1, "d"],
		[5, 4, 6, 2, "W"], [6, 2, 4, 2, "w"], [7, 1, 2, 1, "e"],
	],
]


static func build_image(kind: int) -> Image:
	var img: Image = ApeSprites._build_cell(_BLOCKS[kind])
	img.flip_y()
	return img


static func build(kind: int) -> ImageTexture:
	return ImageTexture.create_from_image(build_image(kind))


static func building_for_invention(key: String) -> int:
	return INVENTION_BUILDING.get(key, -1)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `godot --headless --rendering-driver dummy --path game -s res://scripts/test_building_sprites.gd`
Expected: PASS — `test_building_sprites: all passed`.

- [ ] **Step 5: Format, lint, commit**

```bash
cd /Users/aryasen/projects/anabios
gdformat game/scripts/building_sprites.gd game/scripts/test_building_sprites.gd
gdlint game/scripts/building_sprites.gd game/scripts/test_building_sprites.gd
git add game/scripts/building_sprites.gd game/scripts/test_building_sprites.gd
git commit -m "feat(viewer): building sprite module (trade + invention landmarks)"
```

---

### Task 2: Pure placement helpers

**Files:**
- Modify: `game/scripts/building_sprites.gd` (add three static helpers)
- Modify: `game/scripts/test_building_sprites.gd` (add helper assertions)

**Interfaces:**
- Consumes: the `KIND` enum from Task 1.
- Produces (static on `building_sprites.gd`):
  - `static func signature_kinds(adopted: PackedStringArray, era_of: Dictionary, want: int) -> PackedInt32Array` — the building kinds for the `want` highest-era held inventions, most-advanced first. `era_of` maps invention key → era (int). Ties break by the invention's order in `INVENTION_BUILDING` (stable). Skips keys with no building. Returns 0..`want` kinds.
  - `const MARKET_MIN := 0.35`, `const WAREHOUSE_MIN_MEMBERS := 40` (named thresholds, tuned in Task 3).
  - `static func trade_kind(density_r: float, members: int) -> int` — returns `WAREHOUSE` when `density_r >= MARKET_MIN and members >= WAREHOUSE_MIN_MEMBERS`, `MARKET` when `density_r >= MARKET_MIN`, else `-1`. `density_r` is the `.r` channel of the market-field cell colour (base 0.10 → amber 1.0).
  - `static func market_cell(pos: Vector2, world_size: float, res: int) -> int` — row-major biome-grid cell index for a world position: `clampi(iy,0,res-1) * res + clampi(ix,0,res-1)`, where `ix = int(pos.x / world_size * res)`, `iy = int(pos.y / world_size * res)`. Returns `-1` when `res <= 0` or `world_size <= 0`.

- [ ] **Step 1: Write the failing test (append to `test_building_sprites.gd`)**

Insert these checks just before the final `print(...)`/`quit(0)` in `_init()`:

```gdscript
	# --- signature_kinds: highest era first, skips unbuildable, honours want ---
	var era_of := {"fire": 1, "writing": 3, "farming": 2}
	var adopted := PackedStringArray(["fire", "writing", "farming"])
	var sig: PackedInt32Array = B.signature_kinds(adopted, era_of, 2)
	_check(sig.size() == 2, "want=2 returns two kinds")
	_check(sig[0] == B.WRITING, "highest era (writing) first")
	_check(sig[1] == B.FARMING, "second highest (farming) next")
	# want beyond held count returns only what's held.
	var one: PackedInt32Array = B.signature_kinds(PackedStringArray(["fire"]), era_of, 2)
	_check(one.size() == 1, "want clamps to held count")
	_check(one[0] == B.FIRE, "single held invention -> its building")
	# empty adoption -> no landmarks.
	_check(B.signature_kinds(PackedStringArray(), era_of, 2).size() == 0, "no inventions -> no landmark")

	# --- trade_kind: density + members thresholds ---
	_check(B.trade_kind(0.1, 10) == -1, "low density -> no trade building")
	_check(B.trade_kind(0.5, 10) == B.MARKET, "high density, small -> market")
	_check(B.trade_kind(0.5, 60) == B.WAREHOUSE, "high density, large -> warehouse")

	# --- market_cell: row-major index, clamped ---
	_check(B.market_cell(Vector2(0, 0), 100.0, 10) == 0, "origin -> cell 0")
	_check(B.market_cell(Vector2(55, 25), 100.0, 10) == 2 * 10 + 5, "mid maps row-major")
	_check(B.market_cell(Vector2(999, 999), 100.0, 10) == 9 * 10 + 9, "out-of-range clamps to last cell")
	_check(B.market_cell(Vector2(1, 1), 0.0, 10) == -1, "bad world_size -> -1")
```

- [ ] **Step 2: Run test to verify it fails**

Run: `godot --headless --rendering-driver dummy --path game -s res://scripts/test_building_sprites.gd`
Expected: FAIL — `signature_kinds` / `trade_kind` / `market_cell` not defined.

- [ ] **Step 3: Write minimal implementation (append to `building_sprites.gd`)**

```gdscript
const MARKET_MIN := 0.35
const WAREHOUSE_MIN_MEMBERS := 40


# Building kinds for the `want` highest-era held inventions, most-advanced
# first. `era_of` maps invention key -> era. Ties break by INVENTION_BUILDING
# insertion order (stable). Keys with no building are skipped.
static func signature_kinds(adopted: PackedStringArray, era_of: Dictionary, want: int) -> PackedInt32Array:
	var order: Array = INVENTION_BUILDING.keys()
	var held: Array = []
	for key in adopted:
		if not INVENTION_BUILDING.has(key):
			continue
		held.append({"key": key, "era": int(era_of.get(key, 0)), "ord": order.find(key)})
	held.sort_custom(func(a, b):
		if a["era"] != b["era"]:
			return a["era"] > b["era"]
		return a["ord"] < b["ord"])
	var out := PackedInt32Array()
	for i in mini(want, held.size()):
		out.push_back(INVENTION_BUILDING[held[i]["key"]])
	return out


# Trade building for a village: warehouse at large hubs, market at any hub,
# nothing below MARKET_MIN density. `density_r` is the .r channel of the
# market-field cell colour (base 0.10 -> amber 1.0).
static func trade_kind(density_r: float, members: int) -> int:
	if density_r < MARKET_MIN:
		return -1
	if members >= WAREHOUSE_MIN_MEMBERS:
		return WAREHOUSE
	return MARKET


# Row-major biome-grid cell index for a world position (clamped in-bounds).
static func market_cell(pos: Vector2, world_size: float, res: int) -> int:
	if res <= 0 or world_size <= 0.0:
		return -1
	var ix := clampi(int(pos.x / world_size * float(res)), 0, res - 1)
	var iy := clampi(int(pos.y / world_size * float(res)), 0, res - 1)
	return iy * res + ix
```

- [ ] **Step 4: Run test to verify it passes**

Run: `godot --headless --rendering-driver dummy --path game -s res://scripts/test_building_sprites.gd`
Expected: PASS — `test_building_sprites: all passed`.

- [ ] **Step 5: Format, lint, commit**

```bash
cd /Users/aryasen/projects/anabios
gdformat game/scripts/building_sprites.gd game/scripts/test_building_sprites.gd
gdlint game/scripts/building_sprites.gd game/scripts/test_building_sprites.gd
git add game/scripts/building_sprites.gd game/scripts/test_building_sprites.gd
git commit -m "feat(viewer): pure placement helpers for landmark buildings"
```

---

### Task 3: Wire landmark buildings into the settlement layer

**Files:**
- Modify: `game/scripts/settlement_layer.gd`
- Create: `game/scripts/dbg_building_dump.gd` (temporary headless sprite dump, removed at the end)

**Interfaces:**
- Consumes: `building_sprites.gd` (Task 1+2) — `build()`, `KIND_COUNT`, `NAMES`, `building_for_invention`, `signature_kinds`, `trade_kind`, `market_cell`; and the sim API `settlement_sites()`, `species_stats()`, `invention_catalog()`, `market_colors()`, `resources_active()`, `biome_resolution()`, `world_size()`.
- Produces: no external interface (leaf presentation node).

**Behaviour to implement in `settlement_layer.gd`:**

1. **Cache the era map once.** Add `var _era_of: Dictionary = {}`. In `_ready`, after the existing setup, populate it from the catalog:

```gdscript
	for inv in sim.invention_catalog():
		_era_of[String(inv["key"])] = int(inv["era"])
```

2. **One building MultiMesh layer per kind**, built like `_huts`/`_farms`. Add:

```gdscript
const Buildings = preload("res://scripts/building_sprites.gd")
const BUILDING_SCALE := 16.0
const LANDMARK2_MIN_MEMBERS := 32

var _building_mmis: Array[MultiMeshInstance2D] = []
```

In `_ready`, before `_make_wrap_clones()`, create the layers (z above agents, like huts):

```gdscript
	for k in Buildings.KIND_COUNT:
		var tex := Buildings.build(k)
		var mmi := _make_layer("Building_%s" % Buildings.NAMES[k], tex, 1)
		_building_mmis.append(mmi)
```

`_make_wrap_clones()` must also clone the building layers — extend its source list from `[_huts, _farms]` to `[_huts, _farms] + _building_mmis`.

3. **Per-village placement in `_redraw()`.** After the existing hut/farm transforms are gathered, build one transform list per building kind and write them. For each village (existing `_villages` loop already has `sid`, `members`, `pos`, `fade`, `ease`):
   - Join species stats once per redraw into a `sid -> stats` dictionary from `sim.species_stats()`.
   - `var adopted: PackedStringArray = stats.get("adopted_inventions", PackedStringArray())`
   - `var sig := Buildings.signature_kinds(adopted, _era_of, 2 if members >= LANDMARK2_MIN_MEMBERS else 1)`
   - Place each returned kind on a landmark-ring slot (golden-angle, distinct phase/radius from huts) with scale `BUILDING_SCALE * ease` and colour `Color(1,1,1,fade)`.
   - Trade building (only if `sim.resources_active()`): sample the market field:

```gdscript
	var field := sim.market_colors()
	var res := int(sim.biome_resolution())
	var wsz := sim.world_size()
	# ... per village:
	var tkind := -1
	if not field.is_empty():
		var ci := Buildings.market_cell(pos, wsz, res)
		if ci >= 0 and ci < field.size():
			tkind = Buildings.trade_kind(field[ci].r, members)
	if tkind >= 0:
		# place on a reserved trade slot near the anchor
```

   - Accumulate per-kind transform+colour lists, then write each building MultiMesh with the existing `_write(mm, xfs, cols)` helper.

4. **Landmark ring slots** — deterministic, distinct from huts. Example: signature landmarks at `pos + Vector2.from_angle(sid * 2.39996 + slot * 2.0) * 18.0`; the trade building at a reserved offset such as `pos + Vector2(0, -20)`. Exact radius/angle/scale are tuned in Step 4's capture pass so buildings don't overlap huts or each other.

- [ ] **Step 1: Implement the wiring** (era cache, building layers, wrap-clone extension, per-village placement) per the behaviour above. Keep `settlement_layer.gd` under 1000 lines; if it approaches the ceiling, move the block-art already lives in `building_sprites.gd`, so only placement stays here.

- [ ] **Step 2: Headless build-dump — verify every building sprite bakes cleanly**

Create `game/scripts/dbg_building_dump.gd`:

```gdscript
extends SceneTree
# Bake every building sprite to a scaled PNG contact sheet for eyeballing.
#   godot --headless --path game -s res://scripts/dbg_building_dump.gd -- OUTDIR
const Buildings = preload("res://scripts/building_sprites.gd")

func _init() -> void:
	var args := OS.get_cmdline_user_args()
	var outdir: String = args[0] if args.size() > 0 else "/tmp"
	var n := Buildings.KIND_COUNT
	var sheet := Image.create(n * 16, 16, false, Image.FORMAT_RGBA8)
	sheet.fill(Color(0.15, 0.15, 0.18, 1))
	for k in n:
		var cell: Image = Buildings.build_image(k)
		cell.flip_y()  # undo the storage flip for upright human viewing
		sheet.blit_rect(cell, Rect2i(0, 0, 16, 16), Vector2i(k * 16, 0))
	sheet.resize(n * 16 * 8, 16 * 8, Image.INTERPOLATE_NEAREST)
	sheet.save_png(outdir + "/buildings.png")
	print("wrote ", outdir, "/buildings.png")
	quit(0)
```

Run: `godot --headless --path game -s res://scripts/dbg_building_dump.gd -- <scratchpad>`
Open the PNG; confirm each of the 12 buildings is recognizable. Refine block lists in `building_sprites.gd` and re-run until satisfied. (`test_building_sprites` still guards shape/coverage.)

- [ ] **Step 3: Live capture — verify placement + no corruption**

Run the windowed screenshot harness on a scenario that reaches inventions + trade:

```bash
cd /Users/aryasen/projects/anabios
OUT=<scratchpad>
ANABIOS_SHOT="$OUT/buildings_live.png" ANABIOS_SHOT_TICKS=800 ANABIOS_SHOT_FRAMES=30 \
ANABIOS_SCENARIO="res://../scenarios/inventions.toml" ANABIOS_CAM_ZOOM=10 \
godot --path game --resolution 1280x800 res://scenes/main.tscn
```

Expected: villages show a landmark for their top invention; trade hubs show a market/warehouse; buildings fade in/out with settlements; nothing corrupts or pops. Repeat with `geographic-trade.toml` (trade emphasis) and `domestication.toml` (husbandry/livestock). Tune `MARKET_MIN`, `WAREHOUSE_MIN_MEMBERS`, `LANDMARK2_MIN_MEMBERS`, ring radius/angle, and `BUILDING_SCALE` until villages read clearly without clutter.

- [ ] **Step 4: Remove the debug dump and finalize**

```bash
cd /Users/aryasen/projects/anabios
rm -f game/scripts/dbg_building_dump.gd game/scripts/dbg_building_dump.gd.uid
```

- [ ] **Step 5: Re-run the unit test, format, lint**

```bash
cd /Users/aryasen/projects/anabios
godot --headless --rendering-driver dummy --path game -s res://scripts/test_building_sprites.gd
gdformat game/scripts/settlement_layer.gd
gdlint game/scripts/settlement_layer.gd game/scripts/building_sprites.gd
```
Expected: test passes; format/lint clean.

- [ ] **Step 6: Commit**

```bash
cd /Users/aryasen/projects/anabios
git add game/scripts/settlement_layer.gd
git commit -m "feat(viewer): draw trade & invention landmark buildings in settlements"
```

---

## Self-Review

**Spec coverage:**
- Signature landmark per village (highest-era held invention) → Task 2 `signature_kinds` + Task 3 placement. ✓
- Second landmark when `members ≥ 32` → Task 3 `LANDMARK2_MIN_MEMBERS`, `want=2`. ✓
- Market from market-density field; warehouse at large hubs; gated by `resources_active()` → Task 2 `trade_kind` + Task 3 field sampling. ✓
- 12 building sprites via `_build_cell` in PAL → Task 1 `_BLOCKS` + `build`. ✓
- Plain single-texture MultiMesh, no shader (Metal-safe) → Task 3 `_make_layer` (reuses hut path, no material). ✓
- Linger/fade reuse → Task 3 uses existing `_villages` memory, `fade`/`ease`. ✓
- One village per species; ≤2 landmarks + 1 trade building → Task 3 `want` clamp + single trade slot. ✓
- No core/determinism/binding changes → Global Constraints; all reads are existing `#[func]`s. ✓

**Placeholder scan:** No TBD/TODO; every code step has real code. Art block lists are concrete starters explicitly refined in Task 3 Step 2 (visual iteration is expected for pixel art, guarded by the shape/coverage test).

**Type consistency:** `build`/`build_image`, `KIND_COUNT`, `NAMES`, `INVENTION_BUILDING`, `building_for_invention`, `signature_kinds`, `trade_kind`, `market_cell`, `MARKET_MIN`, `WAREHOUSE_MIN_MEMBERS`, `LANDMARK2_MIN_MEMBERS` are used identically across tasks. `signature_kinds` returns `PackedInt32Array` of building kinds (consumed as such in Task 3). `market_cell` returns a row-major index consumed against `field.size()`. ✓
