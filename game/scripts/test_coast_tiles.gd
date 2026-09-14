extends SceneTree
# Headless unit test for the dual-grid coast autotile tile set. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_coast_tiles.gd
# Exits 0 on success, 1 on the first failed assertion.

const T = preload("res://scripts/coast_tiles.gd")

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

	# --- shoreline bands: every non-trivial tile is transparent at the pixel
	# nearest each corner (the ground shows through), paints sand only on the
	# land side of the waterline and shallow water only on the water side, and
	# carries all three bands somewhere ---
	var corner_px := [Vector2i(0, 0), Vector2i(15, 0), Vector2i(0, 15), Vector2i(15, 15)]
	var corner_name := ["TL", "TR", "BL", "BR"]
	for mask in range(1, 15):
		var img: Image = T.tile_image(mask)
		_check(img.get_width() == 16 and img.get_height() == 16, "mask %d tile is 16x16" % mask)
		for i in 4:
			var px: Vector2i = corner_px[i]
			# A diagonal pair's water corners are painted: the channel
			# core runs through them (their cell is water either way).
			if T.is_diagonal(mask) and bool(mask & (1 << i)):
				continue
			_check(
				img.get_pixel(px.x, px.y).a == 0.0,
				"mask %d corner %s is transparent" % [mask, corner_name[i]]
			)
		var sand := 0
		var foam := 0
		var shallow := 0
		var band_ok := true
		for y in 16:
			for x in 16:
				var c := img.get_pixel(x, y)
				if c.a == 0.0:
					continue
				var w := T.water_weight(mask, x, y)
				if c.is_equal_approx(T.FOAM):
					foam += 1
				elif c.is_equal_approx(T.SAND):
					sand += 1
					band_ok = band_ok and w < 0.5
				elif is_equal_approx(c.r, T.SHALLOW.r) and is_equal_approx(c.b, T.SHALLOW.b):
					shallow += 1
					band_ok = band_ok and w >= 0.5
				elif c.is_equal_approx(T.CHANNEL_DEEP):
					band_ok = band_ok and T.is_diagonal(mask) and w >= T.BAND_SHALLOW
		_check(
			sand > 0 and foam > 0 and shallow > 0, "mask %d carries sand, foam and shallow" % mask
		)
		_check(band_ok, "mask %d sand stays on the land side, shallow on the water side" % mask)
		_check(
			T.opaque_pixels(img) < 224,
			"mask %d is a band, not a slab (%d opaque)" % [mask, T.opaque_pixels(img)]
		)

	# --- water_weight is bilinear in the corner bits ---
	_check(T.water_weight(1, 0, 0) > 0.85, "TL-only weight is near 1 at the TL pixel")
	_check(T.water_weight(1, 15, 15) < 0.15, "TL-only weight is near 0 at the BR pixel")
	_check(absf(T.water_weight(3, 7, 7) - 0.53) < 0.05, "top-edge weight is ~0.5 mid-tile")
	# The diagonal pairs carry a channel: water along the diagonal, land at
	# the two other corners, and the core painted where the cell is land.
	_check(T.water_weight(9, 7, 7) > 0.9, "TL+BR pair is water at the centre")
	_check(T.water_weight(9, 15, 0) < 0.1, "TL+BR pair is land at the TR corner")
	_check(T.water_weight(6, 7, 7) > 0.9, "TR+BL pair is water at the centre")
	_check(T.water_weight(6, 0, 0) < 0.1, "TR+BL pair is land at the TL corner")
	var diag: Image = T.tile_image(9)
	_check(diag.get_pixel(7, 7).is_equal_approx(T.CHANNEL_DEEP), "channel core is painted water")
	_check(diag.get_pixel(15, 0).a == 0.0, "channel leaves the land corners clear")
	_check(T.is_diagonal(9) and T.is_diagonal(6) and not T.is_diagonal(3), "diagonal masks")

	# --- rotation consistency at the art level: rotating a tile 90 degrees
	# clockwise yields the tile of the rotated mask, pixel for pixel ---
	for mask in range(1, 15):
		var img: Image = T.tile_image(mask)
		var rotated_mask := T.rotate_mask_cw(mask)
		var rimg: Image = T.tile_image(rotated_mask)
		var same := true
		for y in 16:
			for x in 16:
				# (x, y) rotated 90 degrees clockwise lands at (15 - y, x).
				if not img.get_pixel(x, y).is_equal_approx(rimg.get_pixel(15 - y, x)):
					same = false
		_check(same, "mask %d rotated 90 degrees is mask %d" % [mask, rotated_mask])

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
