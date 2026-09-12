extends RefCounted
# Dual-grid water/land coast transition tiles (Phase 2 step 3 of
# docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md, D1/D2).
#
# The terrain shader (autotile.gdshaderinc) samples the four terrain cells
# around every half-offset grid corner, builds a 4-bit water mask
#
#   bit 0 (1) = TL water   bit 1 (2) = TR water
#   bit 2 (4) = BL water   bit 3 (8) = BR water
#
# and draws the tile whose atlas cell index equals the mask over the corner
# cell. Masks 0 (all land) and 15 (all water) are fully transparent.
#
# The art is generated, not hand-typed: every pixel takes a bilinear "water
# weight" from the four corners, and only a thin band around the 0.5
# iso-line is painted — sand on the land side, foam right at the waterline,
# shallow water just past it — so a tile stamps a shoreline over the ground
# and leaves the rest of the cell to the terrain tiles and water beneath.
# Bilinear iso-lines round single-corner masks into curved coves and cross
# the diagonal pairs at the centre; every tile is exactly the rotation of its
# mask family's base, so art and mask bits cannot drift apart.

const CELL_PX := 16
const ATLAS_COLS := 4  # 4x4 square atlas (Metal constraint): 16 cells = 16 masks.

# Palette: sand, wet sand, shallow water, foam.
const SAND := Color("d9c58a")
const WET_SAND := Color("bfa86e")
const SHALLOW := Color("2f6f9c")
const FOAM := Color("bcd8e6")

# Water-weight bands around the 0.5 waterline (see water_weight): the land
# side wears sand, the waterline foam, the water side a shallow tint.
const BAND_WET_SAND := 0.26
const BAND_SAND := 0.34
const BAND_FOAM := 0.06
const BAND_SHALLOW := 0.68
const WET_SAND_ALPHA := 0.6
const SHALLOW_ALPHA := 0.85


static func mask_of(tl: bool, tr: bool, bl: bool, br: bool) -> int:
	return int(tl) | (int(tr) << 1) | (int(bl) << 2) | (int(br) << 3)


# Rotate a mask 90° clockwise: the TL corner moves to TR, TR to BR, BR to
# BL, BL to TL.
static func rotate_mask_cw(mask: int) -> int:
	var tl := bool(mask & 1)
	var tr := bool(mask & 2)
	var bl := bool(mask & 4)
	var br := bool(mask & 8)
	return mask_of(bl, tl, br, tr)


# Bilinear water weight of pixel (x, y) (0..CELL_PX-1, pixel centres) for
# `mask`: 1.0 at a water corner, 0.0 at a land corner, the shoreline at 0.5.
static func water_weight(mask: int, x: int, y: int) -> float:
	var u := (float(x) + 0.5) / float(CELL_PX)
	var v := (float(y) + 0.5) / float(CELL_PX)
	var tl := 1.0 if mask & 1 else 0.0
	var tr := 1.0 if mask & 2 else 0.0
	var bl := 1.0 if mask & 4 else 0.0
	var br := 1.0 if mask & 8 else 0.0
	var top := lerpf(tl, tr, u)
	var bottom := lerpf(bl, br, u)
	return lerpf(top, bottom, v)


# The colour a pixel with water weight `w` wears; transparent outside the
# shoreline bands.
static func band_color(w: float) -> Color:
	if absf(w - 0.5) < BAND_FOAM:
		return FOAM
	if w >= 0.5 and w < BAND_SHALLOW:
		return Color(SHALLOW.r, SHALLOW.g, SHALLOW.b, SHALLOW_ALPHA)
	if w > BAND_SAND and w < 0.5:
		return SAND
	if w > BAND_WET_SAND and w <= BAND_SAND:
		return Color(WET_SAND.r, WET_SAND.g, WET_SAND.b, WET_SAND_ALPHA)
	return Color(0, 0, 0, 0)


# One 16x16 RGBA8 transition tile; fully transparent for masks 0 and 15.
static func tile_image(mask: int) -> Image:
	var img := Image.create(CELL_PX, CELL_PX, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	if mask <= 0 or mask >= 15:
		return img
	for y in CELL_PX:
		for x in CELL_PX:
			img.set_pixel(x, y, band_color(water_weight(mask, x, y)))
	return img


static func _blit_cell(atlas: Image, img: Image, cell: int) -> void:
	var cx := (cell % ATLAS_COLS) * CELL_PX
	var cy := int(cell / float(ATLAS_COLS)) * CELL_PX
	atlas.blit_rect(img, Rect2i(0, 0, CELL_PX, CELL_PX), Vector2i(cx, cy))


# The 4x4 atlas of all 16 masks, cell index == mask, row-major. Sampled by
# the terrain shader with `filter_nearest`; no flip (the ground shader reads
# it through UVs, not the plain MultiMesh quad).
static func build_atlas() -> ImageTexture:
	var atlas := Image.create(ATLAS_COLS * CELL_PX, ATLAS_COLS * CELL_PX, false, Image.FORMAT_RGBA8)
	atlas.fill(Color(0, 0, 0, 0))
	for mask in 16:
		_blit_cell(atlas, tile_image(mask), mask)
	return ImageTexture.create_from_image(atlas)


static func opaque_pixels(img: Image) -> int:
	var count := 0
	for y in img.get_height():
		for x in img.get_width():
			if img.get_pixel(x, y).a > 0.0:
				count += 1
	return count
