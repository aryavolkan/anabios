extends SceneTree
# Headless checks for the decorative biome-prop layer. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_biome_props.gd
# Exits 0 on success, 1 on the first failed assertion.

const Props = preload("res://scripts/biome_props.gd")

const RES := 64
const WORLD := 640.0
const WATER := Color(0.08, 0.24, 0.55)
const GRASS := Color(0.18, 0.56, 0.20)
const DRY := Color(0.56, 0.48, 0.34)
const TAIGA := Color(0.16, 0.34, 0.26)

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		# quit() only REQUESTS an exit; the flag keeps a later quit(0) honest.
		_failed = true


# Synthetic terrain: grass on the west half, dry scrub on the east, a taiga
# band along the south, and a square pond in the middle of the grass.
func _terrain() -> PackedColorArray:
	var colors := PackedColorArray()
	colors.resize(RES * RES)
	for y in RES:
		for x in RES:
			var c := GRASS if x < int(RES / 2.0) else DRY
			if y >= RES - 12:
				c = TAIGA
			if x >= 12 and x < 24 and y >= 12 and y < 24:
				c = WATER
			colors[y * RES + x] = c
	return colors


func _init() -> void:
	_check_registry()
	_check_reeds_only()
	_check_classifier()
	_check_sampling()
	_check_layer()
	if _failed:
		quit(1)
		return
	print("test_biome_props: all passed")
	quit(0)


func _check_reeds_only() -> void:
	var props := Props.new()
	props.setup(null)
	props.set_reeds_only(true)
	for k in Props.KIND_COUNT:
		_check(props.layers()[k].visible == (k == Props.REEDS), "reeds-only hides kind %d" % k)
	var hidden_clones := 0
	for clone in props.clones():
		if not clone.visible:
			hidden_clones += 1
	_check(hidden_clones == 8 * (Props.KIND_COUNT - 1), "reeds-only hides the other clones")
	props.set_reeds_only(false)
	for k in Props.KIND_COUNT:
		_check(props.layers()[k].visible, "all kinds back when reeds-only is off")
	props.free()


func _check_registry() -> void:
	_check(Props.KIND_COUNT == 5, "five terrain prop silhouettes")
	_check(Props.NAMES.size() == Props.KIND_COUNT, "NAMES parallels the enum")
	for kind in Props.KIND_COUNT:
		var image: Image = Props.build_image(kind)
		_check(image.get_width() == 16 and image.get_height() == 16, "prop %d is 16x16" % kind)
		_check(Props.opaque_pixels(image) >= 8, "prop %d has a readable silhouette" % kind)
		var tex: ImageTexture = Props.build(kind)
		_check(tex != null and tex.get_width() == 16, "prop %d builds a texture" % kind)


func _check_classifier() -> void:
	_check(Props.kind_for_color(WATER) == Props.NONE, "water has no prop")
	_check(Props.kind_for_color(GRASS) == Props.SHRUB, "green terrain grows shrub")
	_check(Props.kind_for_color(DRY) == Props.ROCK, "dry terrain gets rock")
	_check(Props.kind_for_color(TAIGA) == Props.CONIFER, "dull dark green grows conifer")
	_check(Props.kind_for_color(Color(0.62, 0.66, 0.62)) == Props.ROCK, "pale tundra gets a rock")
	_check(Props.kind_for_color(Color(0.72, 0.66, 0.36)) == Props.LOG, "savanna gets a fallen log")
	_check(Props.kind_for_color(Color(0.42, 0.40, 0.45)) == Props.ROCK, "rock terrain gets a rock")
	_check(
		Props.kind_for_color(Color(0.14, 0.42, 0.18)) == Props.CONIFER, "lush forest grows conifer"
	)
	_check(Props.is_water(WATER), "is_water agrees with the terrain shader on water")
	_check(not Props.is_water(GRASS), "grass is not water")
	# Seeds are pure functions of the cell, and neighbours differ.
	var cell_seed: int = Props.sample_seed(3, 7)
	_check(cell_seed == Props.sample_seed(3, 7), "seed is stable")
	_check(Props.sample_seed(3, 7) != Props.sample_seed(4, 7), "seed varies across x")
	_check(Props.sample_seed(3, 7) != Props.sample_seed(3, 8), "seed varies across y")


func _check_sampling() -> void:
	var colors := _terrain()
	var a: Array = Props.sample_cells(colors, RES)
	var b: Array = Props.sample_cells(colors, RES)
	_check(a.size() == Props.KIND_COUNT, "one cell list per kind")
	_check(a == b, "sampling is deterministic")
	var total := 0
	for kind in Props.KIND_COUNT:
		var cells: PackedVector2Array = a[kind]
		_check(cells.size() <= Props.MAX_PER_KIND, "kind %d respects MAX_PER_KIND" % kind)
		total += cells.size()
		for cell in cells:
			var x := int(cell.x)
			var y := int(cell.y)
			_check(x >= 0 and x < RES and y >= 0 and y < RES, "cell in bounds")
			var c: Color = colors[y * RES + x]
			_check(not Props.is_water(c), "no prop stands in water")
			match kind:
				Props.SHRUB:
					_check(c == GRASS, "shrubs only on grass")
				Props.ROCK:
					_check(c == DRY, "rocks only on dry ground")
				Props.CONIFER:
					_check(c == TAIGA, "conifers only on taiga")
				Props.REEDS:
					_check(Props.has_water_neighbour(colors, RES, x, y), "reeds hug the shore")
	_check(total > 0, "a varied terrain places some props (got %d)" % total)
	_check(total < int(RES * RES / 20.0), "props stay sparse (got %d)" % total)
	_check((a[Props.SHRUB] as PackedVector2Array).size() > 0, "grass half grows shrubs")
	_check((a[Props.ROCK] as PackedVector2Array).size() > 0, "dry half grows rocks")
	# Uniform water: nothing at all.
	var sea := PackedColorArray()
	sea.resize(RES * RES)
	sea.fill(WATER)
	var none: Array = Props.sample_cells(sea, RES)
	var sea_total := 0
	for kind in Props.KIND_COUNT:
		sea_total += (none[kind] as PackedVector2Array).size()
	_check(sea_total == 0, "an ocean world has no props")
	# A mismatched buffer is rejected rather than read out of bounds.
	var short := PackedColorArray()
	short.resize(10)
	var bad: Array = Props.sample_cells(short, RES)
	var bad_total := 0
	for kind in Props.KIND_COUNT:
		bad_total += (bad[kind] as PackedVector2Array).size()
	_check(bad_total == 0, "a mismatched colour buffer places nothing")


func _check_layer() -> void:
	var props: Node2D = Props.new()
	props.min_rescan_msec = 0
	props.setup(null)
	_check(props.layers().size() == Props.KIND_COUNT, "one MultiMesh layer per kind")
	_check(props.clones().size() == 8 * Props.KIND_COUNT, "eight torus clones per kind")
	var colors := _terrain()
	props.refresh(colors, RES, WORLD)
	_check(props.rewrites == 1, "first refresh writes the layers")
	var cells: Array = Props.sample_cells(colors, RES)
	var shown := 0
	for kind in Props.KIND_COUNT:
		var mm: MultiMesh = props.layers()[kind].multimesh
		_check(
			mm.visible_instance_count == (cells[kind] as PackedVector2Array).size(),
			"kind %d shows every sampled cell" % kind
		)
		shown += mm.visible_instance_count
		for i in mm.visible_instance_count:
			var o: Vector2 = mm.get_instance_transform_2d(i).origin
			_check(o.x >= 0.0 and o.x < WORLD and o.y >= 0.0 and o.y < WORLD, "prop inside world")
	_check(shown > 0, "layer shows props")
	# Steady terrain does not rewrite the MultiMeshes.
	props.refresh(colors, RES, WORLD)
	props.refresh(colors, RES, WORLD)
	_check(props.rewrites == 1, "unchanged terrain skips the rewrite")
	# Changed terrain does.
	colors[0] = DRY
	props.refresh(colors, RES, WORLD)
	_check(props.rewrites == 2, "changed terrain rewrites")
	# A world resize repositions the wrap clones.
	props.refresh(colors, RES, WORLD * 2.0)
	_check(props.rewrites == 3, "a resized world rewrites")
	var far := 0
	for clone in props.clones():
		if absf((clone as Node2D).position.x) == WORLD * 2.0:
			far += 1
	_check(far > 0, "clones follow the world size")
	# The parent Biome sprite is scaled world/res; the layer cancels that.
	_check(
		is_equal_approx(props.scale.x, RES / (WORLD * 2.0)), "layer cancels the biome sprite scale"
	)
	props.set_terrain_visible(false)
	_check(not props.visible, "a data overlay hides the props")
	props.set_terrain_visible(true)
	_check(props.visible, "the terrain view shows the props")
	props.free()
