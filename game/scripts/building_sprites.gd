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
]


static func build_image(kind: int) -> Image:
	var img: Image = ApeSprites._build_cell(_BLOCKS[kind])
	img.flip_y()
	return img


static func build(kind: int) -> ImageTexture:
	return ImageTexture.create_from_image(build_image(kind))


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
	# SALT — white crystal cluster
	[[6, 5, 4, 6, "W"], [7, 4, 2, 1, "w"], [5, 8, 1, 2, "W"], [10, 8, 1, 2, "W"]],
	# OBSIDIAN — black glass shard
	[[7, 4, 3, 8, "K"], [6, 6, 1, 4, "d"], [10, 7, 1, 3, "d"]],
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


static func build_cart_image() -> Image:
	var img: Image = ApeSprites._build_cell(_CART_BLOCKS)
	img.flip_y()
	return img


static func build_cart() -> ImageTexture:
	return ImageTexture.create_from_image(build_cart_image())
