extends RefCounted
# Static landmark-building sprites for the settlement layer: one 16x16 block-art
# texture per trade/invention building, built with the shared ApeSprites cell
# painter (auto 1px outline, PAL palette) so buildings match the hut/farm look.
# Textures are flip_y()-ed for the MultiMesh QuadMesh's flipped V axis, same as
# settlement_layer's hut/farm textures. Buildings are drawn through a plain
# (no-shader) MultiMesh, keeping them off the Metal atlas path; the only
# motion is the two-frame flame flicker of the Fire and Metalworking
# landmarks (build_variant), swapped as whole textures by the layer.

const ApeSprites = preload("res://scripts/ape_sprites.gd")
const StructureSprites = preload("res://scripts/structure_sprites.gd")
const StructureTall = preload("res://scripts/structure_tall.gd")

enum {
	MARKET,
	WAREHOUSE,
	STONE_TOOLS,
	FIRE,
	FARMING,
	METALWORKING,
	WRITING,
	MEDICINE,
	HUSBANDRY,
	MACHINERY,
	ELECTRICITY,
	NUCLEAR,
	POTTERY,
	IRRIGATION,
	CURRENCY,
	PRINTING,
	SANITATION,
	GUNPOWDER,
	HAFTED_SPEARS,
	ARCHERY,
	FORTIFICATIONS,
	STEEL_ARMS,
	WELLS,
	VACCINATION
}
const KIND_COUNT := 24
const NAMES: PackedStringArray = [
	"Market",
	"Warehouse",
	"StoneTools",
	"Fire",
	"Farming",
	"Metalworking",
	"Writing",
	"Medicine",
	"Husbandry",
	"Machinery",
	"Electricity",
	"Nuclear",
	"Pottery",
	"Irrigation",
	"Currency",
	"Printing",
	"Sanitation",
	"Gunpowder",
	"HaftedSpears",
	"Archery",
	"Fortifications",
	"SteelArms",
	"Wells",
	"Vaccination"
]

# Invention key (from invention_catalog / species_stats.adopted_inventions) ->
# building kind. Keys are verbatim from anabios-core invention/mod.rs.
const INVENTION_BUILDING := {
	"stone_tools": STONE_TOOLS,
	"fire": FIRE,
	"farming": FARMING,
	"metalworking": METALWORKING,
	"writing": WRITING,
	"medicine": MEDICINE,
	"husbandry": HUSBANDRY,
	"machinery": MACHINERY,
	"electricity": ELECTRICITY,
	"nuclear_power": NUCLEAR,
	"pottery": POTTERY,
	"irrigation": IRRIGATION,
	"currency": CURRENCY,
	"printing": PRINTING,
	"sanitation": SANITATION,
	"gunpowder": GUNPOWDER,
	# Military branch (2026-09).
	"hafted_spears": HAFTED_SPEARS,
	"archery": ARCHERY,
	"fortifications": FORTIFICATIONS,
	"steel_arms": STEEL_ARMS,
	# Basic-needs / late-era rounds (2026-09).
	"wells": WELLS,
	"vaccination": VACCINATION,
}

# 16x16 block lists per kind, indexed by the enum.
const _BLOCKS: Array = [
	# MARKET — peaked striped awning over a stall counter with goods baskets
	[
		[7, 3, 2, 1, "W"],
		[5, 4, 2, 1, "R"],
		[7, 4, 2, 1, "W"],
		[9, 4, 2, 1, "R"],
		[3, 5, 2, 1, "W"],
		[5, 5, 2, 1, "R"],
		[7, 5, 2, 1, "W"],
		[9, 5, 2, 1, "R"],
		[11, 5, 2, 1, "W"],
		[2, 6, 2, 1, "R"],
		[4, 6, 2, 1, "W"],
		[6, 6, 2, 1, "R"],
		[8, 6, 2, 1, "W"],
		[10, 6, 2, 1, "R"],
		[12, 6, 2, 1, "W"],
		[2, 7, 12, 1, "b"],
		[3, 8, 1, 4, "b"],
		[12, 8, 1, 4, "b"],
		[4, 9, 2, 2, "o"],
		[7, 9, 2, 2, "y"],
		[10, 9, 2, 2, "n"],
		[3, 11, 10, 2, "B"],
		[3, 13, 10, 1, "b"],
	],
	# WAREHOUSE — broad storehouse, big ground-to-eave doors, stacked crates
	[
		[2, 6, 12, 8, "B"],
		[1, 4, 14, 2, "b"],
		[3, 3, 10, 1, "b"],
		[6, 7, 5, 7, "K"],
		[8, 7, 1, 7, "b"],
		[3, 9, 2, 2, "m"],
		[2, 11, 3, 3, "r"],
		[11, 9, 2, 2, "m"],
		[11, 11, 3, 3, "r"],
	],
	# STONE_TOOLS — worked-stone boulder + leaning tool rack
	[
		[3, 9, 5, 5, "g"],
		[4, 8, 3, 1, "G"],
		[10, 5, 1, 9, "b"],
		[10, 5, 3, 1, "s"],
		[11, 6, 2, 1, "B"],
		[4, 13, 1, 1, "s"],
		[8, 13, 1, 1, "s"],
	],
	# FIRE — stone hearth ring with a flame
	[
		[4, 11, 8, 3, "g"],
		[4, 10, 1, 1, "G"],
		[11, 10, 1, 1, "G"],
		[5, 11, 6, 1, "b"],
		[6, 8, 1, 2, "o"],
		[7, 6, 2, 5, "R"],
		[7, 5, 2, 2, "o"],
		[8, 4, 1, 2, "y"],
	],
	# FARMING — round grain silo (distinct from the generic farm patch)
	[
		[5, 5, 6, 9, "T"],
		[5, 5, 6, 1, "t"],
		[6, 3, 4, 1, "B"],
		[5, 4, 6, 1, "B"],
		[5, 8, 6, 1, "m"],
		[5, 11, 6, 1, "m"],
		[7, 11, 2, 3, "b"],
	],
	# METALWORKING — forge: chimney, fire mouth, anvil
	[
		[3, 8, 7, 6, "g"],
		[3, 8, 7, 1, "d"],
		[4, 3, 3, 5, "d"],
		[4, 2, 2, 1, "G"],
		[5, 1, 2, 1, "s"],
		[4, 10, 3, 3, "o"],
		[5, 11, 1, 1, "y"],
		[11, 10, 3, 2, "d"],
		[12, 9, 1, 1, "d"],
		[11, 12, 1, 2, "d"],
	],
	# WRITING — inscribed standing stele
	[
		[5, 12, 6, 2, "g"],
		[6, 3, 4, 9, "T"],
		[6, 3, 4, 1, "t"],
		[5, 2, 6, 1, "B"],
		[7, 5, 2, 1, "d"],
		[7, 7, 2, 1, "d"],
		[7, 9, 2, 1, "d"],
	],
	# MEDICINE — apothecary hut with hung herb bundles
	[
		[4, 7, 8, 7, "B"],
		[3, 5, 10, 2, "b"],
		[5, 4, 6, 1, "b"],
		[7, 10, 2, 4, "K"],
		[4, 7, 1, 3, "o"],
		[4, 7, 1, 1, "t"],
		[11, 7, 1, 3, "o"],
		[11, 7, 1, 1, "t"],
	],
	# HUSBANDRY — fenced corral with a penned animal
	[
		[2, 13, 13, 1, "m"],
		[2, 7, 1, 6, "B"],
		[5, 7, 1, 6, "B"],
		[8, 7, 1, 6, "B"],
		[11, 7, 1, 6, "B"],
		[14, 7, 1, 6, "B"],
		[2, 8, 13, 1, "b"],
		[2, 11, 13, 1, "b"],
		[6, 9, 3, 2, "t"],
		[9, 9, 1, 1, "t"],
	],
	# MACHINERY — workshop with a waterwheel over a connected flume
	[
		[2, 7, 6, 7, "B"],
		[1, 6, 8, 1, "b"],
		[9, 4, 6, 6, "d"],
		[9, 6, 6, 2, "s"],
		[11, 4, 2, 6, "s"],
		[11, 6, 2, 2, "g"],
		[9, 10, 6, 1, "g"],
		[9, 11, 6, 2, "s"],
	],
	# ELECTRICITY — glowing lamp post / pylon
	[
		[7, 4, 2, 10, "d"],
		[5, 13, 6, 1, "g"],
		[4, 5, 8, 1, "d"],
		[5, 2, 6, 3, "y"],
		[6, 1, 4, 1, "W"],
		[4, 3, 1, 1, "O"],
		[11, 3, 1, 1, "O"],
	],
	# NUCLEAR — cooling tower with steam
	[
		[4, 6, 8, 8, "G"],
		[5, 9, 6, 2, "g"],
		[4, 13, 8, 1, "d"],
		[5, 4, 6, 2, "W"],
		[6, 2, 4, 2, "w"],
		[7, 1, 2, 1, "e"],
	],
	# POTTERY — beehive kiln with a glowing fire mouth and stacked pots
	[
		[6, 3, 4, 1, "b"],
		[5, 4, 6, 1, "b"],
		[4, 5, 8, 6, "o"],
		[4, 10, 8, 1, "b"],
		[6, 7, 3, 3, "d"],
		[7, 8, 1, 1, "R"],
		[7, 2, 2, 1, "g"],
		[2, 11, 2, 3, "t"],
		[2, 10, 2, 1, "T"],
		[11, 12, 2, 2, "t"],
		[11, 11, 2, 1, "T"],
		[12, 8, 2, 3, "m"],
		[12, 7, 2, 1, "T"],
	],
	# IRRIGATION — post-and-lintel well feeding a water channel that branches
	# into tilled field plots on either side
	[
		[6, 1, 4, 1, "b"],
		[5, 2, 1, 3, "b"],
		[10, 2, 1, 3, "b"],
		[6, 3, 4, 3, "g"],
		[6, 3, 4, 1, "G"],
		[7, 6, 2, 6, "s"],
		[7, 6, 2, 1, "e"],
		[2, 10, 4, 4, "b"],
		[3, 11, 2, 1, "K"],
		[10, 10, 4, 4, "b"],
		[11, 11, 2, 1, "K"],
		[4, 10, 3, 1, "s"],
		[9, 10, 3, 1, "s"],
		[6, 12, 4, 2, "s"],
		[7, 13, 2, 1, "e"],
	],
	# CURRENCY — stone treasury vault with a gold coin emblem and coin stacks
	[
		[3, 7, 10, 7, "g"],
		[3, 7, 10, 1, "G"],
		[4, 4, 8, 3, "g"],
		[5, 3, 6, 1, "G"],
		[2, 6, 12, 1, "s"],
		[6, 10, 4, 4, "d"],
		[7, 11, 2, 2, "y"],
		[11, 1, 4, 4, "y"],
		[12, 2, 2, 2, "O"],
		[11, 1, 4, 1, "e"],
		[2, 11, 3, 2, "y"],
		[2, 10, 3, 1, "O"],
		[12, 11, 2, 2, "y"],
		[12, 10, 2, 1, "O"],
	],
	# PRINTING — wooden press frame with a screw, platen and a printed sheet
	[
		[4, 2, 1, 11, "b"],
		[11, 2, 1, 11, "b"],
		[4, 2, 8, 1, "b"],
		[4, 7, 8, 1, "B"],
		[6, 3, 2, 4, "g"],
		[6, 2, 2, 1, "G"],
		[4, 8, 8, 2, "d"],
		[5, 10, 6, 3, "W"],
		[6, 11, 1, 1, "K"],
		[8, 11, 1, 1, "K"],
		[11, 5, 4, 1, "b"],
		[14, 4, 1, 3, "B"],
		[3, 13, 10, 1, "b"],
	],
	# SANITATION — aqueduct arches carrying a water course down to a pool
	[
		[2, 6, 12, 2, "s"],
		[2, 6, 12, 1, "e"],
		[1, 8, 14, 1, "G"],
		[2, 9, 2, 5, "g"],
		[7, 9, 2, 5, "g"],
		[12, 9, 2, 5, "g"],
		[5, 11, 1, 2, "s"],
		[4, 13, 4, 2, "s"],
		[5, 14, 2, 1, "e"],
		[1, 15, 14, 1, "b"],
	],
	# GUNPOWDER — dark powder mill with stacked barrels and a lit fuse spark
	[
		[3, 7, 9, 7, "d"],
		[3, 7, 9, 1, "g"],
		[5, 4, 5, 3, "d"],
		[6, 3, 3, 1, "g"],
		[6, 10, 2, 4, "K"],
		[11, 10, 3, 4, "x"],
		[11, 10, 3, 1, "X"],
		[11, 12, 3, 1, "X"],
		[2, 11, 2, 3, "x"],
		[2, 11, 2, 1, "X"],
		[12, 9, 1, 1, "y"],
		[12, 8, 1, 1, "o"],
		[13, 7, 1, 1, "O"],
	],
	# HAFTED_SPEARS — a rack of two hafted spears crossed in an X over a
	# wooden base plank, stone blades at each tip
	[
		[2, 14, 13, 1, "b"],
		[3, 13, 1, 1, "t"],
		[4, 11, 1, 2, "t"],
		[5, 10, 1, 1, "t"],
		[6, 8, 1, 2, "t"],
		[7, 7, 1, 1, "t"],
		[8, 5, 1, 2, "t"],
		[9, 4, 1, 1, "t"],
		[10, 3, 1, 1, "t"],
		[11, 2, 1, 1, "t"],
		[10, 1, 3, 2, "s"],
		[11, 1, 1, 1, "G"],
		[12, 13, 1, 1, "t"],
		[11, 11, 1, 2, "t"],
		[10, 10, 1, 1, "t"],
		[9, 8, 1, 2, "t"],
		[8, 7, 1, 1, "t"],
		[7, 5, 1, 2, "t"],
		[6, 4, 1, 1, "t"],
		[5, 3, 1, 1, "t"],
		[4, 2, 1, 1, "t"],
		[3, 1, 3, 2, "s"],
		[4, 1, 1, 1, "G"],
	],
	# ARCHERY — a ringed target disc mounted on a post, with a strung bow
	# leaning beside it
	[
		[2, 2, 9, 9, "W"],
		[3, 3, 7, 7, "R"],
		[4, 4, 5, 5, "W"],
		[5, 5, 3, 3, "y"],
		[5, 10, 2, 4, "b"],
		[4, 13, 5, 1, "b"],
		[13, 2, 1, 3, "t"],
		[12, 5, 1, 2, "t"],
		[12, 7, 1, 2, "t"],
		[12, 9, 1, 2, "t"],
		[13, 11, 1, 3, "t"],
		[14, 2, 1, 12, "e"],
	],
	# FORTIFICATIONS — a crenellated stone rampart section with a dark gate
	[
		[1, 7, 14, 7, "g"],
		[1, 9, 14, 1, "d"],
		[1, 11, 14, 1, "d"],
		[1, 7, 14, 1, "G"],
		[1, 5, 2, 2, "g"],
		[5, 5, 2, 2, "g"],
		[9, 5, 2, 2, "g"],
		[13, 5, 2, 2, "g"],
		[3, 6, 2, 1, "G"],
		[7, 6, 2, 1, "G"],
		[11, 6, 2, 1, "G"],
		[5, 8, 6, 1, "G"],
		[6, 9, 4, 5, "K"],
	],
	# STEEL_ARMS — armory: a sword and shield mounted over a dark blockhouse
	[
		[3, 9, 10, 6, "d"],
		[2, 8, 12, 1, "K"],
		[7, 11, 2, 4, "K"],
		[5, 3, 6, 7, "s"],
		[5, 3, 1, 7, "G"],
		[10, 3, 1, 7, "d"],
		[7, 5, 2, 2, "y"],
		[11, 2, 1, 2, "s"],
		[10, 4, 1, 2, "s"],
		[9, 6, 1, 1, "s"],
		[9, 7, 3, 1, "K"],
		[10, 8, 1, 3, "b"],
		[10, 11, 1, 1, "y"],
	],
	# WELLS — freestanding roofed stone well: peaked plank roof on two posts,
	# a round stone ring (rim + wide body, not the post-and-lintel square of
	# IRRIGATION and no side channels), a rope from the crossbar and a bucket
	# hanging into the dark shaft opening
	[
		[7, 1, 2, 1, "b"],
		[6, 2, 4, 1, "B"],
		[5, 3, 6, 1, "b"],
		[5, 4, 1, 3, "b"],
		[10, 4, 1, 3, "b"],
		[4, 8, 8, 1, "G"],
		[3, 9, 10, 3, "g"],
		[4, 12, 9, 1, "b"],
		[5, 8, 2, 1, "K"],
		[9, 8, 2, 1, "K"],
		[7, 4, 1, 4, "t"],
		[6, 8, 3, 2, "d"],
		[6, 8, 3, 1, "s"],
	],
	# VACCINATION — small white/light clinic, flat pale roof, a prominent red
	# cross emblem on the facade (the dominant feature, unlike MEDICINE's
	# brown apothecary hut with hanging herb bundles) and a vial/bottle
	# accent standing on a shelf beside the door
	[
		[2, 5, 12, 1, "G"],
		[2, 6, 12, 1, "s"],
		[3, 7, 10, 7, "W"],
		[3, 7, 10, 1, "e"],
		[7, 11, 2, 3, "d"],
		[7, 7, 2, 4, "R"],
		[6, 8, 4, 2, "R"],
		[4, 9, 2, 2, "s"],
		[12, 12, 2, 1, "b"],
		[12, 9, 1, 3, "e"],
		[12, 10, 1, 2, "R"],
		[12, 8, 1, 1, "K"],
		[3, 14, 10, 1, "g"],
	],
]

# Every building draws at the structures' texel size (0.625 world units
# per texel on a 20-unit tile) as a 32x44 walled 2.5D front, never as a
# 16 px icon blown up to twice the grain of the stalls and huts beside it.
# The hub centrepieces are the 32 px MarketHall / Warehouse structures;
# every invention landmark is the WORKSHOP front below (slate roof, plaster
# wall between corner posts, two lit windows) with its 16 px invention
# icon hung on the wall as the shop sign.
const HI_RES_KINDS: PackedInt32Array = [MARKET, WAREHOUSE]
# 32x32 top-down workshop front in the structure palette. Row 12 (plain
# plaster between the posts, above the sign) is the row the tall variant
# extrudes.
const WORKSHOP_ROWS: Array = [
	"................................",
	"................................",
	"................................",
	"................................",
	"..............SSSS..............",
	"............ssssssss............",
	"...........ssssssssss...........",
	".........SSSSSSSSSSSSSS.........",
	".......ssssssssssssssssss.......",
	"......ssssssssssssssssssss......",
	"....SSSSSSSSSSSSSSSSSSSSSSSS....",
	"..SSSSSSSSSSSSSSSSSSSSSSSSSSSS..",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bboohhhhhhhhhhhhhhhhhhoobb...",
	"...bboohhhhhhhhhhhhhhhhhhoobb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...bbhhhhhhhhhhhhhhhhhhhhhhbb...",
	"...KKKKKKKKKKKKKKKKKKKKKKKKKK...",
	"...kkkkkkkkkkkkkkkkkkkkkkkkkk...",
	"....kkkkkkkkkkkkkkkkkkkkkkkk....",
]
const SIGN_POS := Vector2i(8, 13)  # the 16x16 icon's top-left on the wall
const WORKSHOP_EXTRUDE_ROW := 13
const WORKSHOP_TALL_ROWS := 6
const _HI_RES_STRUCTURE: Dictionary = {
	MARKET: StructureSprites.MARKET_HALL,
	WAREHOUSE: StructureSprites.WAREHOUSE,
}


static func is_hi_res(kind: int) -> bool:
	return HI_RES_KINDS.has(kind)


# Height over width of every kind's image: the tall ratio.
static func height_ratio(_kind: int) -> float:
	return float(StructureTall.TALL_PX) / float(StructureSprites.CELL_PX)


# The 16x16 invention icon from its block list, top-down.
static func icon_image(blocks: Array) -> Image:
	var icon: Image = ApeSprites._build_cell(blocks)
	icon.flip_y()
	return icon


# Top-down 32x44 workshop with the icon from `blocks` as its sign.
static func workshop_image(blocks: Array) -> Image:
	var front: Image = StructureSprites.paint_rows(WORKSHOP_ROWS)
	front.blend_rect(icon_image(blocks), Rect2i(0, 0, 16, 16), SIGN_POS)
	return StructureTall.extrude(front, WORKSHOP_TALL_ROWS, WORKSHOP_EXTRUDE_ROW)


# Top-down 32x44 art for `kind` at `phase`.
static func topdown_image(kind: int, phase: int = 0) -> Image:
	if is_hi_res(kind):
		return StructureTall.tall_image(_HI_RES_STRUCTURE[kind], 0)
	if phase % 2 == 1 and is_animated(kind):
		return workshop_image(_BLOCKS[kind] + _LIFT_BLOCKS[kind])
	return workshop_image(_BLOCKS[kind])


# Bottom-up (quad-ready) 32x44 image for `kind`.
static func build_image(kind: int) -> Image:
	var img: Image = topdown_image(kind)
	img.flip_y()
	return img


static func build(kind: int) -> ImageTexture:
	return ImageTexture.create_from_image(build_image(kind))


# Landmarks with a live flame. Phase 1 paints _LIFT_BLOCKS over the base art:
# only flame pixels move or brighten (an alpha-0 block erases one), so walls,
# roofs, footprint, enum, name and invention mapping are exactly the base's.
const ANIMATED_KINDS: PackedInt32Array = [FIRE, METALWORKING]
const _LIFT_BLOCKS := {
	# Fire: the side lick jumps a notch, the top amber pixel turns yellow and
	# the tip grows two pixels taller, ending pale.
	FIRE:
	[
		[6, 9, 1, 1, Color(0, 0, 0, 0)],
		[6, 7, 1, 1, "o"],
		[7, 5, 1, 1, "y"],
		[8, 3, 1, 1, "y"],
		[8, 2, 1, 1, "h"],
	],
	# Metalworking: the forge-mouth core whitens and an orange tongue climbs
	# one pixel up the forge wall.
	METALWORKING: [[5, 11, 1, 1, "h"], [5, 10, 1, 1, "y"], [5, 9, 1, 1, "o"]],
}


static func is_animated(kind: int) -> bool:
	return ANIMATED_KINDS.has(kind)


static func build_variant_image(kind: int, phase: int) -> Image:
	var img: Image = topdown_image(kind, phase)
	img.flip_y()
	return img


static func build_variant(kind: int, phase: int) -> ImageTexture:
	return ImageTexture.create_from_image(build_variant_image(kind, phase))


static func building_for_invention(key: String) -> int:
	return INVENTION_BUILDING.get(key, -1)


# market_colors() lerps a dim base (.r ~0.10, meaning NO market) toward amber
# (.r ~1.0, dense market), so the threshold must sit clearly above that 0.10
# floor to mean "a real market", not merely bare terrain.
const MARKET_MIN := 0.20
const WAREHOUSE_MIN_MEMBERS := 40


# Building kinds for the `want` highest-era held inventions, most-advanced
# first. `era_of` maps invention key -> era. Ties break by INVENTION_BUILDING
# insertion order (stable). Keys with no building are skipped.
static func signature_kinds(
	adopted: PackedStringArray, era_of: Dictionary, want: int
) -> PackedInt32Array:
	var order: Array = INVENTION_BUILDING.keys()
	var held: Array = []
	for key in adopted:
		if not INVENTION_BUILDING.has(key):
			continue
		held.append({"key": key, "era": int(era_of.get(key, 0)), "ord": order.find(key)})
	held.sort_custom(
		func(a, b):
			if a["era"] != b["era"]:
				return a["era"] > b["era"]
			return a["ord"] < b["ord"]
	)
	var out := PackedInt32Array()
	for i in mini(want, held.size()):
		out.push_back(INVENTION_BUILDING[held[i]["key"]])
	return out


# Trade building for a village: warehouse at large hubs, market at any hub,
# nothing below MARKET_MIN density. `density_r` is the .r channel of the
# market-field cell colour (base 0.10 -> amber 1.0).
static func trade_kind(density_r: float, members: int) -> int:
	if density_r < MARKET_MIN:
		return -1
	if members >= WAREHOUSE_MIN_MEMBERS:
		return WAREHOUSE
	return MARKET


# Row-major biome-grid cell index for a world position (clamped in-bounds).
static func market_cell(pos: Vector2, world_size: float, res: int) -> int:
	if res <= 0 or world_size <= 0.0:
		return -1
	var ix := clampi(int(pos.x / world_size * float(res)), 0, res - 1)
	var iy := clampi(int(pos.y / world_size * float(res)), 0, res - 1)
	return iy * res + ix


const GOOD_COUNT := 4
const GOOD_NAMES: PackedStringArray = ["Salt", "Obsidian", "Amber", "Spice"]

# 16x16 goods icons, indexed by sim Good index (Salt=0..Spice=3). Small, centered
# emblems drawn with the shared ApeSprites cell painter (auto 1px outline).
const _GOOD_BLOCKS: Array = [
	# SALT — faceted rock-salt cluster: a tall central prism with a lit face and
	# a shaded one, flanked by two shoulder crystals. (Flat "W" throughout, as it
	# was, made it a featureless white blob at hub scale — the brightest mark on
	# the map with no shape to read.)
	[
		[5, 8, 2, 4, "s"],
		[5, 7, 2, 1, "w"],
		[10, 8, 2, 4, "s"],
		[10, 7, 2, 1, "w"],
		[6, 5, 4, 7, "s"],
		[6, 5, 2, 7, "W"],
		[7, 4, 2, 1, "W"],
		[9, 6, 1, 5, "G"],
	],
	# OBSIDIAN — black glass shard, tapered to a point, with a specular sheen up
	# its face. Solid "K" alone read as a hole punched in the terrain.
	[
		[6, 9, 5, 3, "K"],
		[6, 6, 4, 3, "K"],
		[7, 4, 3, 2, "K"],
		[8, 3, 2, 1, "K"],
		[7, 5, 1, 6, "X"],
		[8, 4, 1, 7, "s"],
		[9, 7, 2, 4, "x"],
		[8, 11, 3, 1, "d"],
	],
	# AMBER — orange gem
	[[6, 6, 4, 4, "o"], [7, 5, 2, 1, "y"], [6, 9, 4, 1, "O"], [8, 6, 1, 1, "y"]],
	# SPICE — red-brown mound with specks
	[[5, 9, 6, 3, "r"], [6, 8, 4, 1, "R"], [7, 10, 1, 1, "y"], [9, 10, 1, 1, "y"]],
]


static func build_good_image(good_idx: int) -> Image:
	var img: Image = ApeSprites._build_cell(_GOOD_BLOCKS[good_idx])
	img.flip_y()
	return img


static func build_good(good_idx: int) -> ImageTexture:
	return ImageTexture.create_from_image(build_good_image(good_idx))


# 16x16 caravan cart: wooden body, pale canvas top, two dark wheels. Small so it
# reads as a vehicle beside the bigger hub buildings.
const _CART_BLOCKS: Array = [
	[3, 6, 10, 4, "t"],  # wooden body
	[3, 6, 10, 1, "b"],  # top rail of body
	[4, 3, 8, 3, "B"],  # pale canvas cover
	[4, 3, 8, 1, "W"],  # canvas highlight
	[3, 10, 2, 2, "K"],  # left wheel
	[11, 10, 2, 2, "K"],  # right wheel
	[5, 9, 6, 1, "d"],  # axle shadow
]

# 16x16 plank bridge, drawn along +x (the caravan layer rotates it onto the
# road): dark planks with lit seams and a rail along each edge.
const _BRIDGE_BLOCKS: Array = [
	[0, 4, 16, 8, "b"],  # deck
	[0, 5, 16, 1, "t"],  # lit seam
	[0, 8, 16, 1, "t"],
	[0, 11, 16, 1, "t"],
	[0, 3, 16, 1, "K"],  # near rail
	[0, 12, 16, 1, "K"],  # far rail
	[3, 4, 1, 8, "d"],  # plank joints
	[8, 4, 1, 8, "d"],
	[13, 4, 1, 8, "d"],
]


static func build_bridge_image() -> Image:
	var img: Image = ApeSprites._build_cell(_BRIDGE_BLOCKS)
	img.flip_y()
	return img


static func build_bridge() -> ImageTexture:
	return ImageTexture.create_from_image(build_bridge_image())


static func build_cart_image() -> Image:
	var img: Image = ApeSprites._build_cell(_CART_BLOCKS)
	img.flip_y()
	return img


static func build_cart() -> ImageTexture:
	return ImageTexture.create_from_image(build_cart_image())
