extends SceneTree
# Headless check for the 32 px canopy trees and the per-chunk canopy planner.
# Run with:
#   godot --headless --rendering-driver dummy --path game -s res://scripts/test_flora_sprites.gd

const AgentLayer = preload("res://scripts/agent_layer.gd")
const Flora = preload("res://scripts/flora_sprites.gd")
const PropChunk = preload("res://scripts/prop_chunk.gd")
const SpriteSplit = preload("res://scripts/sprite_split.gd")
const StructureSprites = preload("res://scripts/structure_sprites.gd")

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
		# 2.5D split (sprite_split.gd): crown + trunk halves partition the art,
		# and the cut lands inside the trunk rows so the crown covers a figure
		# standing north of the tree.
		var row: int = Flora.trunk_row(k)
		var crown: Image = SpriteSplit.upper(img, row)
		var trunk: Image = SpriteSplit.lower(img, row)
		_check(
			Flora.opaque_pixels(crown) + Flora.opaque_pixels(trunk) == Flora.opaque_pixels(img),
			"%s split partitions the sprite" % Flora.NAMES[k]
		)
		_check(
			Flora.opaque_pixels(crown) > Flora.opaque_pixels(trunk),
			"%s crown is the larger half" % Flora.NAMES[k]
		)
		var trunk_row: String = rows[row]
		_check(
			trunk_row.contains("T") or trunk_row.contains("t"),
			"%s cut row is a trunk row" % Flora.NAMES[k]
		)
		_check(
			row > 0 and not (rows[row - 1] as String).contains("T"),
			"%s cut is the first trunk row" % Flora.NAMES[k]
		)
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

	# --- sprite_split on the structure set and edge cases ---
	var tent: Image = StructureSprites.build_variant_image(StructureSprites.TENT, 0)
	var trow: int = SpriteSplit.split_row(tent)
	var span: PackedInt32Array = SpriteSplit.opaque_span(tent)
	_check(trow > span[0] and trow < span[1], "tent cut lands inside the sprite")
	_check(
		(
			(
				StructureSprites.opaque_pixels(SpriteSplit.upper(tent, trow))
				+ StructureSprites.opaque_pixels(SpriteSplit.lower(tent, trow))
			)
			== StructureSprites.opaque_pixels(tent)
		),
		"tent split partitions the sprite"
	)
	var empty := Image.create(8, 8, false, Image.FORMAT_RGBA8)
	_check(SpriteSplit.opaque_span(empty) == PackedInt32Array([-1, -1]), "empty span")
	_check(SpriteSplit.split_row(empty) == 0, "empty image splits at row 0")
	# --- contact shadow texture: opaque core, transparent rim ---
	var sh: Image = AgentLayer.shadow_image(16)
	_check(sh.get_pixel(8, 8).a > 0.9, "shadow core is opaque")
	_check(sh.get_pixel(0, 0).a == 0.0, "shadow corner is transparent")

	if _failed:
		quit(1)
		return
	print("test_flora_sprites: all passed")
	quit(0)
