extends SceneTree
# Headless unit test for the terrain tile + decoration prop asset library.
# Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_terrain_sprites.gd
# Exits 0 on success, 1 on the first failed assertion.

const T = preload("res://scripts/terrain_sprites.gd")

# Canonical terrain base colours, kept in sync with `cell_color()` in
# crates/anabios-core/src/biome.rs. Tiles must stay in the same hue family so
# the tiled ground, the climate-lerp ground, and the minimap agree.
const CANON: Array = [
	[0.09, 0.19, 0.44],  # Water
	[0.21, 0.44, 0.19],  # Grass
	[0.07, 0.26, 0.11],  # Forest
	[0.68, 0.58, 0.33],  # Desert
	[0.42, 0.40, 0.45],  # Rock
	[0.72, 0.66, 0.36],  # Savanna
	[0.06, 0.34, 0.16],  # Rainforest
	[0.16, 0.34, 0.26],  # Taiga
	[0.62, 0.66, 0.62],  # Tundra
]

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _opaque_count(img: Image) -> int:
	var n := 0
	for y in img.get_height():
		for x in img.get_width():
			if img.get_pixel(x, y).a > 0.5:
				n += 1
	return n


func _mean_rgb(img: Image) -> Vector3:
	var acc := Vector3.ZERO
	for y in img.get_height():
		for x in img.get_width():
			var c := img.get_pixel(x, y)
			acc += Vector3(c.r, c.g, c.b)
	return acc / float(img.get_width() * img.get_height())


func _init() -> void:
	_check(T.TERRAIN_COUNT == CANON.size(), "one tile family per core TerrainType")

	# --- atlas: square, nearest-safe grid (Metal constraint) ---
	var atlas := T.build_atlas()
	var side := T.ATLAS_COLS * T.CELL_PX
	_check(atlas != null, "atlas builds")
	_check(
		atlas.get_width() == side and atlas.get_height() == side,
		"atlas is a square %dx%d grid (extreme-aspect atlases corrupt on Metal)" % [side, side]
	)
	_check(
		T.TERRAIN_COUNT * T.VARIANTS + T.PROP_COUNT <= T.ATLAS_COLS * T.ATLAS_COLS,
		"every tile variant and prop fits the atlas grid"
	)

	# --- ground tiles: 16x16, fully opaque, on-palette, variants distinct ---
	for t in T.TERRAIN_COUNT:
		for v in T.VARIANTS:
			var img: Image = T.tile_image(t, v)
			var label := "%s v%d" % [T.NAMES[t], v]
			_check(img.get_width() == 16 and img.get_height() == 16, "%s tile is 16x16" % label)
			_check(
				_opaque_count(img) == 256,
				"%s tile is fully opaque (ground tiles must have no holes)" % label
			)
			# Tiles agree with the biome.rs canon in HUE (so the minimap and the
			# ground name the same terrain) but may sit brighter: they carry 90%
			# of the land colour and the canon's dark greens read as dusk.
			var canon: Array = CANON[t]
			var mean: Vector3 = _mean_rgb(img)
			var mc := Color(mean.x, mean.y, mean.z)
			var cc := Color(canon[0], canon[1], canon[2])
			var dh := absf(mc.h - cc.h)
			dh = minf(dh, 1.0 - dh)
			_check(
				dh < 0.06 or cc.s < 0.12,
				"%s tile mean hue stays near the biome.rs canon (dh=%.3f)" % [label, dh]
			)
			_check(
				mc.v >= cc.v - 0.06,
				(
					"%s tile is no darker than the biome.rs canon (v=%.2f vs %.2f)"
					% [label, mc.v, cc.v]
				)
			)
		_check(
			T.tile_image(t, 0).get_data() != T.tile_image(t, 1).get_data(),
			"%s variants 0 and 1 differ" % T.NAMES[t]
		)
		# Variant 2 is specified as variant 0 mirrored, for cheap scatter.
		var flipped: Image = T.tile_image(t, 0)
		flipped.flip_x()
		_check(
			T.tile_image(t, 2).get_data() == flipped.get_data(),
			"%s variant 2 is variant 0 mirrored" % T.NAMES[t]
		)

	# --- decoration props: 16x16, visible figure on a transparent field ---
	for p in T.PROP_COUNT:
		var img: Image = T.prop_image(p)
		var label: String = T.PROP_NAMES[p]
		_check(img.get_width() == 16 and img.get_height() == 16, "%s prop is 16x16" % label)
		var opaque := _opaque_count(img)
		_check(
			opaque >= 12 and opaque <= 220,
			"%s prop is a figure on transparency (opaque=%d)" % [label, opaque]
		)

	# --- per-terrain prop suggestion: water bare, land terrains decorated ---
	_check(T.prop_for_terrain(T.WATER) == -1, "water suggests no prop")
	for t in range(1, T.TERRAIN_COUNT):
		var p: int = T.prop_for_terrain(t)
		_check(p >= 0 and p < T.PROP_COUNT, "%s suggests a valid prop" % T.NAMES[t])
		for i in 20:
			var v: int = T.prop_variant_for(t, float(i) / 20.0)
			_check(
				T.props_for_terrain(t).has(v), "%s variant %d is one of its props" % [T.NAMES[t], v]
			)
	_check(T.prop_variant_for(T.WATER, 0.5) == -1, "water grows no prop variant")
	var grass_kinds: Dictionary = {}
	for i in 60:
		grass_kinds[T.prop_variant_for(T.GRASS, float(i) / 60.0)] = true
	_check(
		grass_kinds.size() == T.props_for_terrain(T.GRASS).size(),
		"grass grows every one of its props"
	)

	# --- atlas layout: tile cells land where the mapping says ---
	var aimg := atlas.get_image()
	var cell: int = T.atlas_cell_for_tile(T.GRASS, 0)
	var cx := (cell % T.ATLAS_COLS) * T.CELL_PX
	var cy := int(cell / float(T.ATLAS_COLS)) * T.CELL_PX
	_check(
		aimg.get_pixel(cx + 8, cy + 8) == T.tile_image(T.GRASS, 0).get_pixel(8, 8),
		"atlas cell content matches the tile it maps"
	)

	if _failed:
		quit(1)
		return
	print("test_terrain_sprites: all passed")
	quit(0)
