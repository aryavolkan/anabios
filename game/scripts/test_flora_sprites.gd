extends SceneTree
# Headless check for the 32 px canopy trees and the per-chunk canopy planner.
# Run with:
#   godot --headless --rendering-driver dummy --path game -s res://scripts/test_flora_sprites.gd

const Flora = preload("res://scripts/flora_sprites.gd")
const PropChunk = preload("res://scripts/prop_chunk.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	_check(Flora.NAMES.size() == Flora.KIND_COUNT, "one name per kind")
	for k in Flora.KIND_COUNT:
		var rows: Array = Flora._ROWS[k]
		_check(rows.size() == 32, "%s has 32 rows" % Flora.NAMES[k])
		for r in rows:
			_check((r as String).length() == 32, "%s rows are 32 wide" % Flora.NAMES[k])
		var img: Image = Flora.kind_image(k)
		_check(img.get_width() == 32 and img.get_height() == 32, "%s is 32x32" % Flora.NAMES[k])
		_check(Flora.opaque_pixels(img) >= 200, "%s has a readable canopy" % Flora.NAMES[k])
		# Canopy above trunk: the top half holds more opaque pixels than the
		# bottom quarter (trunk + shadow), i.e. the art is upright as painted.
		var top := 0
		var bottom := 0
		for y in 32:
			for x in 32:
				if img.get_pixel(x, y).a > 0.0:
					if y < 16:
						top += 1
					elif y >= 24:
						bottom += 1
		_check(top > bottom, "%s canopy sits above its trunk" % Flora.NAMES[k])
	for a in Flora.KIND_COUNT:
		for b in range(a + 1, Flora.KIND_COUNT):
			_check(
				Flora.kind_image(a).get_data() != Flora.kind_image(b).get_data(),
				"%s and %s differ" % [Flora.NAMES[a], Flora.NAMES[b]]
			)
	_check(Flora.kind_for_terrain(0) == -1, "water grows no tree")
	_check(Flora.kind_for_terrain(2) == Flora.OAK, "forest grows oaks")
	_check(Flora.kind_for_terrain(7) == Flora.PINE, "taiga grows pines")
	_check(Flora.kind_for_terrain(99) == -1, "unknown terrain grows nothing")

	# --- canopy planner: dense on forest, empty on water, deterministic ---
	var res := 128
	var forest := PackedByteArray()
	forest.resize(66 * 66)
	forest.fill(2)
	var trees: Array = PropChunk.plan_canopy(0, 0, forest, res, 1024.0)
	_check(trees.size() == Flora.KIND_COUNT, "one bucket per kind")
	var oaks: PackedVector2Array = trees[Flora.OAK]
	var expected := int(64 * 64 * Flora.DENSITY[2])
	_check(
		oaks.size() > expected / 2 and oaks.size() < expected * 2,
		"forest chunk grows about %d oaks (got %d)" % [expected, oaks.size()]
	)
	var sorted := true
	for i in range(1, oaks.size()):
		if oaks[i].y < oaks[i - 1].y:
			sorted = false
	_check(sorted, "canopy positions are y-sorted")
	var again: Array = PropChunk.plan_canopy(0, 0, forest, res, 1024.0)
	_check(again[Flora.OAK] == oaks, "canopy plan is deterministic")
	var water := PackedByteArray()
	water.resize(66 * 66)
	water.fill(0)
	var none: Array = PropChunk.plan_canopy(1, 1, water, res, 1024.0)
	var total := 0
	for k in Flora.KIND_COUNT:
		total += (none[k] as PackedVector2Array).size()
	_check(total == 0, "water chunk grows no trees")

	if _failed:
		quit(1)
		return
	print("test_flora_sprites: all passed")
	quit(0)
