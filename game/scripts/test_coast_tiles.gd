extends SceneTree
# Headless unit test for the dual-grid coast autotile tile set. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_coast_tiles.gd
# Exits 0 on success, 1 on the first failed assertion.

const T = preload("res://scripts/coast_tiles.gd")

const WATER_COLOR := "2f6f9c"
const SAND_COLOR := "d9c58a"

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


func _init() -> void:
	# --- atlas: square, nearest-safe grid (Metal constraint), one cell per mask ---
	var atlas := T.build_atlas()
	var side := T.ATLAS_COLS * T.CELL_PX
	_check(atlas != null, "atlas builds")
	_check(
		atlas.get_width() == side and atlas.get_height() == side,
		"atlas is a square %dx%d grid (extreme-aspect atlases corrupt on Metal)" % [side, side]
	)
	_check(T.ATLAS_COLS * T.ATLAS_COLS == 16, "atlas has exactly one cell per mask (0..15)")

	# --- masks 0 and 15: no overlay, fully transparent ---
	for trivial in [0, 15]:
		var img: Image = T.tile_image(trivial)
		_check(img.get_width() == 16 and img.get_height() == 16, "mask %d tile is 16x16" % trivial)
		_check(
			_opaque_count(img) == 0,
			"mask %d (uniform land/water) tile is fully transparent" % trivial
		)

	# --- mask_of: bit order is TL | TR<<1 | BL<<2 | BR<<3 ---
	_check(T.mask_of(false, false, false, false) == 0, "mask_of all-land is 0")
	_check(T.mask_of(true, true, true, true) == 15, "mask_of all-water is 15")
	_check(T.mask_of(true, false, false, false) == 1, "mask_of TL-only is bit 0 (1)")
	_check(T.mask_of(false, true, false, false) == 2, "mask_of TR-only is bit 1 (2)")
	_check(T.mask_of(false, false, true, false) == 4, "mask_of BL-only is bit 2 (4)")
	_check(T.mask_of(false, false, false, true) == 8, "mask_of BR-only is bit 3 (8)")
	_check(T.mask_of(true, true, false, false) == 3, "mask_of TL+TR is 3")
	_check(T.mask_of(false, false, true, true) == 12, "mask_of BL+BR is 12")
	_check(T.mask_of(true, false, false, true) == 9, "mask_of TL+BR diagonal is 9")
	_check(T.mask_of(false, true, true, false) == 6, "mask_of TR+BL diagonal is 6")

	# --- rotate_mask_cw: TL -> TR -> BR -> BL -> TL under a 90-degree turn ---
	_check(T.rotate_mask_cw(1) == 2, "TL-only rotates 90 degrees to TR-only")
	_check(T.rotate_mask_cw(2) == 8, "TR-only rotates 90 degrees to BR-only")
	_check(T.rotate_mask_cw(8) == 4, "BR-only rotates 90 degrees to BL-only")
	_check(T.rotate_mask_cw(4) == 1, "BL-only rotates 90 degrees to TL-only")
	# Every non-trivial mask returns to itself after 4 rotations.
	for m in range(1, 15):
		var r := m
		for _i in range(4):
			r = T.rotate_mask_cw(r)
		_check(r == m, "mask %d returns to itself after 4 rotations" % m)
	# The diagonal-pair masks are a 2-cycle (rotational symmetry).
	_check(T.rotate_mask_cw(9) == 6, "TL+BR diagonal rotates to TR+BL diagonal")
	_check(T.rotate_mask_cw(6) == 9, "TR+BL diagonal rotates back to TL+BR")
	# Every mask 1..14 is reachable by rotating some other mask (no orphans:
	# rotate_mask_cw is a bijection on 1..14 once 0 and 15 — its own fixed
	# points — are excluded).
	var seen := {}
	for m in range(1, 15):
		seen[T.rotate_mask_cw(m)] = true
	_check(seen.size() == 14, "rotate_mask_cw permutes the 14 non-trivial masks onto themselves")

	# --- corner-colour property: every non-trivial mask's tile is
	# water-coloured at the pixel nearest each water corner and
	# sand-coloured at the pixel nearest each land corner ---
	var corner_px := [Vector2i(0, 0), Vector2i(15, 0), Vector2i(0, 15), Vector2i(15, 15)]
	var corner_name := ["TL", "TR", "BL", "BR"]
	for mask in range(1, 15):
		var img: Image = T.tile_image(mask)
		_check(img.get_width() == 16 and img.get_height() == 16, "mask %d tile is 16x16" % mask)
		for i in 4:
			var bit_set := ((mask >> i) & 1) == 1
			var px: Vector2i = corner_px[i]
			var c := img.get_pixel(px.x, px.y)
			var want := Color(WATER_COLOR) if bit_set else Color(SAND_COLOR)
			_check(
				c.is_equal_approx(want),
				(
					"mask %d (%s) corner %s is %s-coloured"
					% [mask, mask, corner_name[i], "water" if bit_set else "sand"]
				)
			)

	# --- rotation consistency at the art level: rotating a tile's corner
	# colours 90 degrees (TL->TR->BR->BL) matches the tile at the rotated
	# mask, for every non-trivial mask ---
	for mask in range(1, 15):
		var img: Image = T.tile_image(mask)
		var rotated_mask := T.rotate_mask_cw(mask)
		var rimg: Image = T.tile_image(rotated_mask)
		# old TL corner colour should equal new TR corner colour, etc.
		_check(
			img.get_pixel(0, 0).is_equal_approx(rimg.get_pixel(15, 0)),
			"mask %d -> %d: TL colour moves to TR under rotation" % [mask, rotated_mask]
		)
		_check(
			img.get_pixel(15, 0).is_equal_approx(rimg.get_pixel(15, 15)),
			"mask %d -> %d: TR colour moves to BR under rotation" % [mask, rotated_mask]
		)
		_check(
			img.get_pixel(15, 15).is_equal_approx(rimg.get_pixel(0, 15)),
			"mask %d -> %d: BR colour moves to BL under rotation" % [mask, rotated_mask]
		)
		_check(
			img.get_pixel(0, 15).is_equal_approx(rimg.get_pixel(0, 0)),
			"mask %d -> %d: BL colour moves to TL under rotation" % [mask, rotated_mask]
		)

	# --- atlas layout: tile cells land where mask == cell index says ---
	var aimg := atlas.get_image()
	for mask in [1, 6, 11]:
		var cx: int = (mask % T.ATLAS_COLS) * T.CELL_PX
		var cy: int = int(mask / float(T.ATLAS_COLS)) * T.CELL_PX
		_check(
			aimg.get_pixel(cx + 8, cy + 8) == T.tile_image(mask).get_pixel(8, 8),
			"atlas cell %d content matches the tile it maps" % mask
		)

	if _failed:
		quit(1)
		return
	print("test_coast_tiles: all passed")
	quit(0)
