## Tall 2.5D variants of the walled structure sprites.
##
## 2.5D height follows the Liberated Pixel Cup / Kenney 3/4-view convention:
## a building shows a roof over a front wall about a figure high, so a hut
## stands roughly two figures tall. Kinds with walls get a 32x44 "tall"
## variant: the wall band under the eave is extruded by TALL_ROWS[kind]
## copies of the row just above the roof/wall split, roof and base art
## untouched, the spare rows left clear at the top. Flat kinds (fences,
## fields, wells, ...) keep their 32x32 art.

const StructureSprites = preload("res://scripts/structure_sprites.gd")
const SpriteSplit = preload("res://scripts/sprite_split.gd")

const CELL_PX := StructureSprites.CELL_PX
const TALL_PX := 44
const EXTRA_ROWS := TALL_PX - CELL_PX
const TALL_ROWS: Dictionary = {
	StructureSprites.HUT: 6,
	StructureSprites.HUT_B: 6,
	StructureSprites.HUT_C: 6,
	StructureSprites.HOUSE: 8,
	StructureSprites.HALL: 10,
	StructureSprites.GRANARY: 6,
	StructureSprites.FORGE: 4,
	StructureSprites.SCRIPTORIUM: 10,
	StructureSprites.MILL: 4,
	StructureSprites.TOWER: 12,
	StructureSprites.GATE: 6,
	StructureSprites.MARKET_HALL: 6,
	StructureSprites.WAREHOUSE: 8,
}


static func is_tall(kind: int) -> bool:
	return TALL_ROWS.has(kind)


# Rows of extruded wall a tall kind gains (0 for a flat kind).
static func tall_rows(kind: int) -> int:
	return int(TALL_ROWS.get(kind, 0))


# The 32x44 tall variant of `kind` at `phase` (see TALL_ROWS); a non-tall
# kind comes back as its plain 32x32 image.
static func tall_image(kind: int, phase: int = 0) -> Image:
	var img: Image = StructureSprites.build_variant_image(kind, phase)
	if not is_tall(kind):
		return img
	var cut: int = SpriteSplit.split_row(img)
	var dup: int = clampi(cut - 1, 0, CELL_PX - 1)
	var extra: int = tall_rows(kind)
	var lift: int = EXTRA_ROWS - extra
	var out := Image.create(CELL_PX, TALL_PX, false, Image.FORMAT_RGBA8)
	out.fill(Color(0, 0, 0, 0))
	for y in CELL_PX:
		var oy: int = y + lift if y < cut else y + EXTRA_ROWS
		for x in CELL_PX:
			out.set_pixel(x, oy, img.get_pixel(x, y))
	for i in extra:
		for x in CELL_PX:
			out.set_pixel(x, cut + lift + i, img.get_pixel(x, dup))
	return out


# Roof/wall split row of the tall variant: the plain split, pushed down by
# the extruded band so the whole wall stays under the figures.
static func tall_split_row(kind: int) -> int:
	var row: int = SpriteSplit.split_row(StructureSprites.build_variant_image(kind, 0))
	return row + EXTRA_ROWS if is_tall(kind) else row
