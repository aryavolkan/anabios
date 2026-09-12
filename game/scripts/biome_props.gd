extends Node2D

# Decorative biome props: a sparse, deterministic scatter of 16x16 pixel
# silhouettes (shore reeds, shrub, conifer, rock, fallen log) resolved from the
# terrain colours the sim already publishes. Pure presentation — the sim knows
# nothing of them, they carry no collision or resources, and the same terrain
# always yields the same scatter. One nearest-filtered plain MultiMesh layer
# per kind (plus eight torus wrap clones sharing its MultiMesh) keeps the cost
# flat on the Metal-safe plain-MultiMesh path: no per-prop nodes, no shader.
# Owned by BiomeRenderer, which refreshes it only while the terrain view is up.

const ApeSprites = preload("res://scripts/ape_sprites.gd")

enum { NONE = -1, REEDS, SHRUB, CONIFER, ROCK, LOG }
const KIND_COUNT := 5
const NAMES: PackedStringArray = ["Reeds", "Shrub", "Conifer", "Rock", "Log"]

# Candidate lattice: every SAMPLE_STRIDE-th cell on both axes, thinned by a
# per-cell hash so the survivors read as scattered rather than gridded. The
# hash modulus scales with the grid so a 512² continent gets about as many
# props as a 128² sandbox, spread evenly instead of piling up in scan order.
const SAMPLE_STRIDE := 3
const TARGET_TOTAL := 560
const MIN_MOD := 4
const MAX_PER_KIND := 140
# Draw order relative to the parent Biome sprite (z -10): above the terrain,
# below farms (-6), agents (0) and huts (1).
const Z_INDEX := 3
# World units per 16px cell, per kind. Shrubs/rocks sit under hut scale (16)
# so the village still looms; conifers stand a notch taller.
const SCALE_OF: PackedFloat32Array = [8.0, 8.0, 12.0, 8.0, 9.0]
# Terrain checksum samples (first, middle and last cell included) — with the
# colour quantised so living-biome creep does not rescan every redraw.
const CHECKSUM_SAMPLES := 33
const CHECKSUM_STEPS := 16.0

# Extra hues the shared ApeSprites palette lacks (it has no greens); any other
# block key falls through to ApeSprites.PAL.
const PROP_PAL := {
	"r1": "8c9a4e",  # reed stalk
	"r2": "6f7d3c",  # reed shade
	"g1": "2f5a2a",  # shrub shade
	"g2": "4e8a3c",  # shrub body
	"g3": "7ab353",  # shrub light
	"n1": "24463a",  # pine shade
	"n2": "3a6b52",  # pine body
	"n3": "5a8f6a",  # pine light
}

# Each prop is a list of [x, y, w, h, key] blocks on a 16x16 grid, drawn
# back-to-front (later blocks overwrite earlier ones); the cell painter adds
# the 1px outline.
const _BLOCKS: Array = [
	# REEDS — a shoreline clump of olive stalks with brown cattail heads
	[
		[3, 14, 10, 1, "r2"],
		[7, 3, 1, 12, "r1"],
		[4, 6, 1, 9, "r1"],
		[10, 5, 1, 10, "r1"],
		[12, 8, 1, 7, "r2"],
		[2, 9, 1, 6, "r2"],
		[7, 1, 1, 3, "B"],
		[10, 3, 1, 3, "B"],
		[4, 4, 1, 3, "B"],
	],
	# SHRUB — rounded bush, dark base, lit crown, short stem
	[
		[7, 13, 2, 2, "b"],
		[3, 9, 10, 4, "g1"],
		[4, 7, 8, 6, "g1"],
		[4, 7, 7, 4, "g2"],
		[5, 5, 6, 3, "g2"],
		[6, 4, 3, 2, "g3"],
		[5, 6, 2, 1, "g3"],
		[9, 7, 2, 1, "g3"],
	],
	# CONIFER — tiered pine, lit centre streak, brown trunk
	[
		[7, 13, 2, 3, "b"],
		[2, 11, 12, 2, "n1"],
		[3, 9, 10, 2, "n2"],
		[4, 7, 8, 2, "n1"],
		[5, 5, 6, 2, "n2"],
		[6, 3, 4, 2, "n1"],
		[7, 1, 2, 2, "n2"],
		[7, 3, 1, 8, "n3"],
	],
	# ROCK — faceted grey boulder with a lit top and a shaded flank
	[
		[3, 13, 11, 1, "b"],
		[4, 8, 9, 5, "g"],
		[5, 6, 6, 2, "G"],
		[6, 5, 3, 1, "s"],
		[5, 7, 2, 2, "s"],
		[10, 8, 3, 3, "d"],
		[4, 11, 9, 2, "d"],
	],
	# LOG — fallen trunk, lit top edge, pale cut end, one branch stub
	[
		[2, 12, 12, 1, "d"],
		[2, 8, 12, 4, "b"],
		[2, 8, 12, 1, "B"],
		[3, 9, 10, 1, "r"],
		[13, 7, 2, 5, "t"],
		[13, 8, 1, 3, "B"],
		[5, 6, 1, 2, "B"],
		[10, 12, 2, 1, "b"],
	],
]

# Count of refresh() calls that actually rewrote the layers (tests + debugging).
var rewrites := 0
# Wall-clock throttle between rescans (a living biome's colours creep every
# tick; a threshold crossing in the checksum samples is the only trigger, but
# even that must not re-scatter twice a second). Tests set it to 0.
var min_rescan_msec := 2000

var _layers: Array[MultiMeshInstance2D] = []
var _clones: Array[MultiMeshInstance2D] = []
var _sig: Array = []
var _last_rewrite_msec := -1000000
var _lattice := PackedVector2Array()
var _lattice_res := -1


static func build_image(kind: int) -> Image:
	var blocks: Array = []
	for b in _BLOCKS[kind]:
		var key: String = b[4]
		var col: Variant = key
		if PROP_PAL.has(key):
			col = Color(PROP_PAL[key])
		blocks.append([b[0], b[1], b[2], b[3], col])
	var img: Image = ApeSprites._build_cell(blocks)
	img.flip_y()
	return img


static func build(kind: int) -> ImageTexture:
	return ImageTexture.create_from_image(build_image(kind))


static func opaque_pixels(image: Image) -> int:
	var n := 0
	for y in image.get_height():
		for x in image.get_width():
			if image.get_pixel(x, y).a > 0.5:
				n += 1
	return n


# Same thresholds as terrain.gdshader's is_water() (and BiomeRenderer's
# is_water_at), so props stop exactly where the ground shader draws water.
# Alpha is ignored: biome_colors() packs elevation into it.
static func is_water(c: Color) -> bool:
	return c.b > c.r + 0.05 and c.b > c.g + 0.05 and c.b > 0.20 and maxf(c.r, c.g) < 0.45


# Terrain colour -> prop kind, keyed to the hue families cell_color() emits:
# greens split by darkness (grass/pioneer flush -> shrub; forest, taiga and
# rainforest -> conifer), low-chroma greys (rock terrain, tundra) and dry
# desert/bare-earth tones -> rock, warm savanna -> fallen log, water -> none.
# Reeds are not a colour: sample_cells() places them on shoreline cells.
static func kind_for_color(c: Color) -> int:
	if is_water(c):
		return NONE
	if c.g > c.r * 1.18 and c.g > c.b * 1.10:
		return SHRUB if c.r >= 0.17 or c.g >= 0.50 else CONIFER
	var lo := minf(c.r, minf(c.g, c.b))
	var hi := maxf(c.r, maxf(c.g, c.b))
	if hi - lo < 0.10:
		return ROCK
	if c.r > c.g * 1.12:
		return ROCK
	return LOG


static func sample_seed(x: int, y: int) -> int:
	return int((x * 73856093) ^ (y * 19349663))


# Second, independent hash per cell for jitter/size/mirror bits: the lattice
# hash's low bits are zero by construction (posmod == 0), so they can't vary.
static func detail_seed(x: int, y: int) -> int:
	return int(((x + 1) * 83492791) ^ ((y + 1) * 2654435761))


static func has_water_neighbour(colors: PackedColorArray, res: int, x: int, y: int) -> bool:
	for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
		var nx := posmod(x + d.x, res)
		var ny := posmod(y + d.y, res)
		if is_water(colors[ny * res + nx]):
			return true
	return false


# Hash modulus for a grid: the fraction of lattice candidates that survive.
static func sample_mod(res: int) -> int:
	var side := ceili(res / float(SAMPLE_STRIDE))
	return maxi(MIN_MOD, roundi(side * side / float(TARGET_TOTAL)))


# Candidate cells for a grid, independent of terrain: colours only decide the
# kind, so a refresh classifies a few hundred cells instead of the whole grid.
static func lattice(res: int) -> PackedVector2Array:
	var out := PackedVector2Array()
	if res <= 0:
		return out
	var m := sample_mod(res)
	for y in range(0, res, SAMPLE_STRIDE):
		for x in range(0, res, SAMPLE_STRIDE):
			if posmod(sample_seed(x, y), m) == 0:
				out.append(Vector2(x, y))
	return out


# Per-kind grid cells (Array of PackedVector2Array, indexed by kind) for a
# colour grid. Pure and deterministic; an empty result for a size mismatch.
static func sample_cells(colors: PackedColorArray, res: int) -> Array:
	return _classify(colors, res, lattice(res))


static func _classify(colors: PackedColorArray, res: int, cells: PackedVector2Array) -> Array:
	var out: Array = []
	for _k in KIND_COUNT:
		out.append(PackedVector2Array())
	if res <= 0 or colors.size() != res * res:
		return out
	var picked: Array = []  # per kind: [[detail hash, cell], ...]
	for _k in KIND_COUNT:
		picked.append([])
	for cell in cells:
		var x := int(cell.x)
		var y := int(cell.y)
		var c: Color = colors[y * res + x]
		var kind := kind_for_color(c)
		if kind == NONE:
			continue
		if has_water_neighbour(colors, res, x, y):
			kind = REEDS
		picked[kind].append([detail_seed(x, y), cell])
	for k in KIND_COUNT:
		var list: Array = picked[k]
		if list.size() > MAX_PER_KIND:
			# Uniform thinning by hash, not scan order: a truncated scan would
			# strip the bottom of the map bare.
			list.sort_custom(func(a: Array, b: Array) -> bool: return a[0] < b[0])
			list.resize(MAX_PER_KIND)
		var cells_k := PackedVector2Array()
		for entry in list:
			cells_k.append(entry[1])
		out[k] = cells_k
	return out


# Quantised sample of the terrain colours: first, middle and last cells plus
# evenly spaced ones between, so steady terrain never rewrites the layers.
static func checksum(colors: PackedColorArray) -> int:
	var n := colors.size()
	if n == 0:
		return 0
	var samples := mini(CHECKSUM_SAMPLES, n)
	var acc := 0
	for i in samples:
		var idx := roundi(float(n - 1) * float(i) / float(maxi(1, samples - 1)))
		var c: Color = colors[idx]
		acc = hash(
			[
				acc,
				int(c.r * CHECKSUM_STEPS),
				int(c.g * CHECKSUM_STEPS),
				int(c.b * CHECKSUM_STEPS),
			]
		)
	return acc


# Build the per-kind layers and their wrap clones once. `sim` (optional) seeds
# the clone offsets; refresh() re-derives them from the world size it is given.
func setup(sim: Node) -> void:
	if not _layers.is_empty():
		return
	var world: float = float(sim.world_size()) if sim != null else 0.0
	for k in KIND_COUNT:
		var mmi := _make_layer("Prop_%s" % NAMES[k], build(k))
		_layers.append(mmi)
		for gy in range(-1, 2):
			for gx in range(-1, 2):
				if gx == 0 and gy == 0:
					continue
				var clone := MultiMeshInstance2D.new()
				clone.name = "%s_Wrap%d_%d" % [mmi.name, gx + 1, gy + 1]
				clone.multimesh = mmi.multimesh
				clone.texture = mmi.texture
				clone.texture_filter = mmi.texture_filter
				clone.z_index = mmi.z_index
				clone.position = Vector2(gx * world, gy * world)
				add_child(clone)
				_clones.append(clone)


func _make_layer(pname: String, tex: ImageTexture) -> MultiMeshInstance2D:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_2D
	mm.use_colors = true
	mm.mesh = QuadMesh.new()
	var mmi := MultiMeshInstance2D.new()
	mmi.name = pname
	mmi.multimesh = mm
	mmi.texture = tex
	mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	mmi.z_index = Z_INDEX
	add_child(mmi)
	return mmi


func layers() -> Array[MultiMeshInstance2D]:
	return _layers


func clones() -> Array[MultiMeshInstance2D]:
	return _clones


# Props belong to the terrain view; a data overlay (pheromone, markets, ...)
# owns the ground and would read wrong with scenery on top.
func set_terrain_visible(shown: bool) -> void:
	visible = shown


# Re-scatter from the current terrain colours. No-op unless the grid size,
# world size or quantised terrain checksum changed (and the rescan throttle
# has elapsed for a colour-only change).
func refresh(colors: PackedColorArray, resolution: int, world_size: float) -> void:
	if _layers.is_empty() or resolution <= 0 or world_size <= 0.0:
		return
	if colors.size() != resolution * resolution:
		return
	var sig := [resolution, world_size, checksum(colors)]
	if sig == _sig:
		return
	var now := Time.get_ticks_msec()
	var geometry_changed: bool = _sig.is_empty() or sig[0] != _sig[0] or sig[1] != _sig[1]
	if not geometry_changed and now - _last_rewrite_msec < min_rescan_msec:
		return
	_sig = sig
	_last_rewrite_msec = now
	rewrites += 1
	# The parent Biome sprite is scaled world/res (biome pixels -> world
	# units); cancel it so the instance transforms below are world units.
	scale = Vector2(resolution, resolution) / world_size
	var i := 0
	for _k in KIND_COUNT:
		for gy in range(-1, 2):
			for gx in range(-1, 2):
				if gx == 0 and gy == 0:
					continue
				_clones[i].position = Vector2(gx * world_size, gy * world_size)
				i += 1
	if _lattice_res != resolution:
		_lattice = lattice(resolution)
		_lattice_res = resolution
	var cells: Array = _classify(colors, resolution, _lattice)
	var cell_w := world_size / float(resolution)
	for k in KIND_COUNT:
		_write(_layers[k].multimesh, cells[k], k, cell_w)


# Place one kind's props: cell centre plus a hashed jitter (never leaving the
# cell, so a shore prop never drifts into the water), a hashed size wobble
# and a hashed mirror so the same silhouette does not tile visibly.
func _write(mm: MultiMesh, cells: PackedVector2Array, kind: int, cell_w: float) -> void:
	var m := cells.size()
	if m > mm.instance_count:
		mm.instance_count = m
	mm.visible_instance_count = m
	var base: float = SCALE_OF[kind]
	for i in m:
		var cx := int(cells[i].x)
		var cy := int(cells[i].y)
		var h := detail_seed(cx, cy)
		var jx := (float((h >> 8) & 255) / 255.0 - 0.5) * 0.8
		var jy := (float((h >> 16) & 255) / 255.0 - 0.5) * 0.8
		var size := base * (0.85 + 0.30 * float((h >> 24) & 255) / 255.0)
		var flip := -1.0 if ((h >> 3) & 1) == 1 else 1.0
		var pos := Vector2((cx + 0.5 + jx) * cell_w, (cy + 0.5 + jy) * cell_w)
		mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(size * flip, size), 0.0, pos))
		mm.set_instance_color(i, Color(1, 1, 1))
