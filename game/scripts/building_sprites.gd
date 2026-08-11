extends RefCounted
# Static landmark-building sprites for the settlement layer: one 16x16 block-art
# texture per trade/invention building, built with the shared ApeSprites cell
# painter (auto 1px outline, PAL palette) so buildings match the hut/farm look.
# Textures are flip_y()-ed for the MultiMesh QuadMesh's flipped V axis, same as
# settlement_layer's hut/farm textures. Buildings never animate and are drawn
# through a plain (no-shader) MultiMesh, keeping them off the Metal atlas path.

const ApeSprites = preload("res://scripts/ape_sprites.gd")

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
	NUCLEAR
}
const KIND_COUNT := 12
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
	"Nuclear"
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
}

# 16x16 block lists per kind, indexed by the enum.
const _BLOCKS: Array = [
	# MARKET — striped awning stall with goods baskets
	[
		[3, 10, 10, 4, "B"],
		[3, 6, 1, 5, "b"],
		[12, 6, 1, 5, "b"],
		[2, 5, 12, 1, "R"],
		[2, 6, 12, 1, "W"],
		[4, 11, 2, 2, "o"],
		[7, 11, 2, 2, "y"],
		[10, 11, 2, 2, "n"],
	],
	# WAREHOUSE — broad storehouse, big doors, stacked crates
	[
		[2, 6, 12, 8, "B"],
		[1, 4, 14, 2, "b"],
		[3, 3, 10, 1, "b"],
		[6, 8, 4, 6, "K"],
		[8, 8, 1, 6, "b"],
		[3, 12, 2, 2, "r"],
		[12, 12, 2, 2, "r"],
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
	# MACHINERY — workshop with a waterwheel
	[
		[3, 7, 6, 7, "B"],
		[2, 6, 8, 1, "b"],
		[9, 5, 6, 6, "d"],
		[11, 5, 2, 6, "s"],
		[9, 7, 6, 2, "s"],
		[11, 7, 2, 2, "g"],
		[9, 12, 6, 2, "s"],
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
]


static func build_image(kind: int) -> Image:
	var img: Image = ApeSprites._build_cell(_BLOCKS[kind])
	img.flip_y()
	return img


static func build(kind: int) -> ImageTexture:
	return ImageTexture.create_from_image(build_image(kind))


static func building_for_invention(key: String) -> int:
	return INVENTION_BUILDING.get(key, -1)
