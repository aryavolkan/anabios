extends SceneTree
# Headless checks for the era structure sprite atlas. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_structure_sprites.gd
# Exits 0 on success, 1 on the first failed assertion.

const SpriteSplit = preload("res://scripts/sprite_split.gd")
const S = preload("res://scripts/structure_sprites.gd")
const T = preload("res://scripts/structure_tall.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		# quit() only REQUESTS an exit; the flag keeps a later quit(0) honest.
		_failed = true


func _col_has_opaque(img: Image, x: int) -> bool:
	for y in img.get_height():
		if img.get_pixel(x, y).a > 0.5:
			return true
	return false


func _row_has_opaque(img: Image, y: int) -> bool:
	for x in img.get_width():
		if img.get_pixel(x, y).a > 0.5:
			return true
	return false


func _init() -> void:
	# --- enum / NAMES / constants coherence -------------------------------
	_check(S.KIND_COUNT == 28, "28 structure kinds")
	_check(S.NAMES.size() == S.KIND_COUNT, "NAMES parallels the enum")
	_check(S.CELL_PX == 32, "cell size is 32px")
	_check(S.ATLAS_COLS == 8, "atlas is an 8x8 grid")

	# --- every kind builds a 32x32 image with a readable silhouette -------
	var images: Array = []
	for k in S.KIND_COUNT:
		var img: Image = S.kind_image(k)
		_check(img.get_width() == 32 and img.get_height() == 32, "%s image is 32x32" % S.NAMES[k])
		var opaque := S.opaque_pixels(img)
		_check(opaque >= 60, "%s has a readable silhouette (%d opaque px)" % [S.NAMES[k], opaque])
		images.append(img)

	# --- every pair of kinds is visually distinct --------------------------
	for i in S.KIND_COUNT:
		for j in range(i + 1, S.KIND_COUNT):
			var a: Image = images[i]
			var b: Image = images[j]
			_check(
				a.get_data() != b.get_data(),
				"%s and %s are distinct sprites" % [S.NAMES[i], S.NAMES[j]]
			)

	# --- opaque_pixels agrees with a manual count on a synthetic image -----
	var synth := Image.create(4, 4, false, Image.FORMAT_RGBA8)
	synth.fill(Color(0, 0, 0, 0))
	synth.set_pixel(0, 0, Color(1, 1, 1, 1))
	synth.set_pixel(1, 1, Color(1, 1, 1, 0.6))
	synth.set_pixel(2, 2, Color(1, 1, 1, 0.4))
	_check(S.opaque_pixels(synth) == 2, "opaque_pixels counts alpha > 0.5 only")

	# --- atlas: 256x256, cell k's centre pixel matches kind_image(k)'s -----
	var atlas_tex := S.build_atlas()
	var atlas_img := atlas_tex.get_image()
	_check(atlas_img.get_width() == 256 and atlas_img.get_height() == 256, "atlas is 256x256")
	for k in S.KIND_COUNT:
		var cx := (k % S.ATLAS_COLS) * S.CELL_PX
		var cy := int(k / float(S.ATLAS_COLS)) * S.CELL_PX
		var atlas_centre := atlas_img.get_pixel(cx + 16, cy + 16)
		var cell_centre: Color = images[k].get_pixel(16, 16)
		_check(
			atlas_centre == cell_centre,
			"%s atlas cell centre matches kind_image centre" % S.NAMES[k]
		)

	# --- tiling: fence/palisade segments span the full cell so they tile ---
	var fence_h: Image = images[S.FENCE_H]
	_check(_col_has_opaque(fence_h, 0), "FenceH leftmost column is opaque (tiles)")
	_check(_col_has_opaque(fence_h, 31), "FenceH rightmost column is opaque (tiles)")

	var fence_v: Image = images[S.FENCE_V]
	_check(_row_has_opaque(fence_v, 0), "FenceV top row is opaque (tiles)")
	_check(_row_has_opaque(fence_v, 31), "FenceV bottom row is opaque (tiles)")

	var pal_h: Image = images[S.PALISADE_H]
	_check(_col_has_opaque(pal_h, 0), "PalisadeH leftmost column is opaque (tiles)")
	_check(_col_has_opaque(pal_h, 31), "PalisadeH rightmost column is opaque (tiles)")

	var pal_v: Image = images[S.PALISADE_V]
	_check(_row_has_opaque(pal_v, 0), "PalisadeV top row is opaque (tiles)")
	_check(_row_has_opaque(pal_v, 31), "PalisadeV bottom row is opaque (tiles)")

	# --- animated kinds: hearth, forge, burnt ruin flicker; others don't ---
	_check(S.is_animated(S.HEARTH), "hearth animates")
	_check(S.is_animated(S.FORGE), "forge animates")
	_check(S.is_animated(S.RUIN_BURNT), "burnt ruin animates")
	_check(not S.is_animated(S.HUT), "hut stays static")
	_check(not S.is_animated(S.TENT), "tent stays static")
	_check(S.ANIMATED_KINDS.size() == 5, "exactly five animated kinds")

	for k in S.ANIMATED_KINDS:
		var low: Image = S.build_variant(k, 0).get_image()
		var high: Image = S.build_variant(k, 1).get_image()
		_check(low.get_size() == Vector2i(32, 32), "%s phase 0 stays 32x32" % S.NAMES[k])
		_check(high.get_size() == Vector2i(32, 32), "%s phase 1 stays 32x32" % S.NAMES[k])
		_check(
			low.get_data() == S.kind_image(k).get_data(), "%s phase 0 is the base art" % S.NAMES[k]
		)
		_check(low.get_data() != high.get_data(), "%s phase 1 differs from phase 0" % S.NAMES[k])
		_check(
			S.build_variant(k, 3).get_image().get_data() == high.get_data(),
			"%s odd phases share the lifted frame" % S.NAMES[k]
		)

	# A non-animated kind's "variant" is identical at every phase.
	_check(
		S.build_variant(S.HUT, 1).get_image().get_data() == S.kind_image(S.HUT).get_data(),
		"static kinds ignore the phase"
	)

	# --- era_of matches the design's era table ------------------------------
	for k in [S.TENT, S.TENT_B, S.WINDBREAK, S.HEARTH]:
		_check(S.era_of(k) == 0, "%s is era 0 (camp)" % S.NAMES[k])
	for k in [
		S.HUT,
		S.HUT_B,
		S.HUT_C,
		S.FENCE_H,
		S.FENCE_V,
		S.FIELD,
		S.GRANARY,
		S.WELL,
		S.RUIN_BURNT,
		S.STALL,
	]:
		_check(S.era_of(k) == 1, "%s is era 1 (thatch)" % S.NAMES[k])
	for k in [
		S.HALL,
		S.PALISADE_H,
		S.PALISADE_V,
		S.PALISADE_CORNER,
		S.GATE,
		S.TOWER,
		S.MILL,
		S.FORGE,
		S.SCRIPTORIUM,
		S.BANNER
	]:
		_check(S.era_of(k) == 2, "%s is era 2 (timber/stone)" % S.NAMES[k])

	# --- tall variants: 32x44 for walled kinds, roof and base art intact ---
	_check(T.TALL_PX == 44 and T.EXTRA_ROWS == 12, "tall cell is 32x44")
	_check(
		T.is_tall(S.HUT) and T.is_tall(S.HOUSE) and not T.is_tall(S.FENCE_H),
		"walled kinds are tall"
	)
	var hut: Image = S.kind_image(S.HUT)
	var tall: Image = T.tall_image(S.HUT)
	_check(tall.get_width() == 32 and tall.get_height() == 44, "tall hut is 32x44")
	var cut: int = SpriteSplit.split_row(hut)
	_check(T.tall_split_row(S.HUT) == cut + T.EXTRA_ROWS, "tall split sits under the extruded wall")
	var lift: int = T.EXTRA_ROWS - T.tall_rows(S.HUT)
	var same := true
	for y in cut:
		for x in 32:
			same = same and tall.get_pixel(x, y + lift) == hut.get_pixel(x, y)
	for y in range(cut, 32):
		for x in 32:
			same = same and tall.get_pixel(x, y + T.EXTRA_ROWS) == hut.get_pixel(x, y)
	_check(same, "the roof and the base rows are the 32 px art")
	var band := true
	for i in T.tall_rows(S.HUT):
		for x in 32:
			band = band and tall.get_pixel(x, cut + lift + i) == hut.get_pixel(x, cut - 1)
	_check(band, "the extruded band repeats the row above the split")
	var clear := true
	for y in lift:
		for x in 32:
			clear = clear and tall.get_pixel(x, y).a == 0.0
	_check(clear, "spare rows above a short extrusion stay clear")
	_check(
		T.tall_rows(S.TOWER) == T.EXTRA_ROWS and T.tall_rows(S.FENCE_H) == 0,
		"tower uses the full band"
	)
	_check(T.tall_image(S.FENCE_H).get_height() == 32, "a flat kind stays 32x32")

	if _failed:
		quit(1)
		return
	print("test_structure_sprites: all passed")
	quit(0)
