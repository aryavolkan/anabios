extends RefCounted
# 32x32 canopy trees for the per-chunk prop layer (Phase 2 step 4 / D1 of
# docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md): the
# reference boards read as forest because trees are big, dense and overlap;
# the 16 px props in terrain_sprites.gd stay for bushes, rocks and reeds and
# this set adds one canopy tree per forest family, drawn at 0.5 world units
# per texel (16 world units, two biome cells) and y-sorted within a chunk so
# nearer trees overlap farther ones. Row-string block art, 1 px dark outline
# baked into the rows, ground shadow in the bottom rows.

const CELL_PX := 32

# Kinds 4 and 5 are rock outcrops: the same 32 px, y-sorted, split-layered
# pipeline as the trees, planted on Rock terrain so mountains read as grey
# crags rather than flat purple tiles (the reference boards' outcrops).
# Kinds 6..9 are variants of the oak, pine and jungle families (a broad
# lobed oak, a tall narrow oak, a wide layered pine, a twin-crowned jungle
# tree): the planner picks one per cell from the cell hash so a forest
# mixes silhouettes instead of repeating one lollipop.
enum { OAK, PINE, ACACIA, JUNGLE, BOULDERS, CRAG, OAK_B, OAK_C, PINE_B, JUNGLE_B }
const KIND_COUNT := 10
const NAMES: PackedStringArray = [
	"Oak", "Pine", "Acacia", "Jungle", "Boulders", "Crag", "OakB", "OakC", "PineB", "JungleB"
]
# Kind -> family head (the kind kind_for_terrain() names for its terrain).
const FAMILY: PackedInt32Array = [
	OAK, PINE, ACACIA, JUNGLE, BOULDERS, BOULDERS, OAK, OAK, PINE, JUNGLE
]

# Terrain ids (biome.rs / terrain_sprites.gd): Water 0, Grass 1, Forest 2,
# Desert 3, Rock 4, Savanna 5, Rainforest 6, Taiga 7, Tundra 8.
const _TERRAIN_KIND: PackedInt32Array = [-1, OAK, OAK, -1, BOULDERS, ACACIA, JUNGLE, PINE, PINE]
# Fraction of cells of each terrain that grow a canopy tree (the 16 px props
# keep their own, lower table). Forests are dense; grass and tundra sparse.
const DENSITY: PackedFloat32Array = [0.0, 0.03, 0.42, 0.0, 0.10, 0.05, 0.5, 0.36, 0.02]

const PAL := {
	"k": "14100f",  # outline
	"T": "4a2f1e",  # trunk dark
	"t": "7a4c2c",  # trunk
	"d": "1f5a2a",  # canopy dark
	"g": "2f8a3c",  # canopy
	"l": "4fb050",  # canopy lit
	"D": "1b4a3a",  # pine dark
	"p": "2d7a55",  # pine
	"P": "48a070",  # pine lit
	"a": "6f8a30",  # acacia dark
	"A": "9ab84a",  # acacia lit
	"j": "1a6b34",  # jungle dark
	"J": "3fa050",  # jungle
	"L": "6fd06a",  # jungle lit
	"s": "0b1a12",  # shadow
	"r": "8b8f97",  # rock
	"R": "b9bcc2",  # rock lit
	"q": "5b5e66",  # rock shade
	"m": "5f8a3a",  # moss
}

const _ROWS := {
	OAK:
	[
		"................................",
		"..............kkkk..............",
		"...........kkkddddkkk...........",
		".........kkdddggggdddkk.........",
		"........kddgggggggggddk.........",
		".......kdggggllggggggdk.........",
		"......kdgggglllllggggggdk.......",
		".....kdggglllllllgggggggdk......",
		"....kdgggglllllllggggggggdk.....",
		"....kdggglllllllgggggggggdk.....",
		"...kdgggggllllggggggggggggdk....",
		"...kdggggggggggggggggggggddk....",
		"...kddgggggggggggggggggggddk....",
		"...kddggggggggggggggggggdddk....",
		"....kdddgggggggggggggggdddk.....",
		"....kddddggggggggggggddddk......",
		".....kddddddgggggggddddddk......",
		"......kkddddddddddddddkkk.......",
		"........kkkddddddddkkk..........",
		"...........kkTTtTkk.............",
		".............kTttTk.............",
		".............kTttTk.............",
		".............kTttTk.............",
		".............kTtttTk............",
		"............kTTtttTTk...........",
		"...........kTTTtttTTTk..........",
		"..........kkkkkkkkkkkkk.........",
		".......ssssssssssssssssss.......",
		".....ssssssssssssssssssssss.....",
		".......ssssssssssssssssss.......",
		"................................",
		"................................",
	],
	PINE:
	[
		"................................",
		"...............kk...............",
		"..............kPPk..............",
		"..............kPpk..............",
		".............kPppDk.............",
		".............kPppDk.............",
		"............kPpppDDk............",
		"...........kPPppppDDk...........",
		"..........kkPppppDDDkk..........",
		"............kPpppDDk............",
		"...........kPPppppDDk...........",
		"..........kPPpppppDDDk..........",
		".........kPPppppppDDDDk.........",
		"........kkkPpppppDDDkkkk........",
		"..........kPPpppppDDDk..........",
		".........kPPppppppDDDDk.........",
		"........kPPPppppppDDDDDk........",
		".......kPPpppppppppDDDDDk.......",
		"......kkkkPppppppDDDDkkkkk......",
		".........kPPpppppppDDDk.........",
		"........kPPppppppppDDDDk........",
		".......kPPpppppppppDDDDDk.......",
		"......kPPPppppppppppDDDDDk......",
		".....kkkkkkkkkTttTkkkkkkkkk.....",
		".............kTttTk.............",
		".............kTttTk.............",
		"............kTTttTTk............",
		"...........kkkkkkkkkk...........",
		".........ssssssssssssss.........",
		".......ssssssssssssssssss.......",
		".........ssssssssssssss.........",
		"................................",
	],
	ACACIA:
	[
		"................................",
		"................................",
		"........kkkkkkkkkkkkkkkk........",
		"......kkAAAAAAAAAAAAAAAAkk......",
		".....kAAAAAAAAAAAAAAAAAAAAk.....",
		"....kAAAaaaAAAAAAAAaaaAAAAAk....",
		"....kaaaaaaaaaaaaaaaaaaaaaak....",
		".....kaaaaaaaaaaaaaaaaaaaak.....",
		"......kkaaaaaaaaaaaaaaaakk......",
		"........kkkaaaaaaaaaakkk........",
		"...........kkkTtTkkk............",
		".............kTtTk..............",
		".............kTttk..............",
		"..............kTtk..............",
		"..............kTtk..............",
		"..............kTtk..............",
		"..............kTtk..............",
		".............kTTtTk.............",
		"............kTTtttTk............",
		"...........kkkkkkkkkk...........",
		"........ssssssssssssssss........",
		"......ssssssssssssssssssss......",
		"........ssssssssssssssss........",
		"................................",
		"................................",
		"................................",
		"................................",
		"................................",
		"................................",
		"................................",
		"................................",
		"................................",
	],
	JUNGLE:
	[
		"................................",
		"..........kkk.....kkk...........",
		".........kLLLk...kLLLk..........",
		"........kLJJJLk.kLJJJLk.........",
		".......kLJJjjJLkLJjjJJLk........",
		"......kLJJjjjjJJJJjjjjJLk.......",
		".....kLJJJjjjjjjjjjjjJJJLk......",
		"....kLJJJJjjjjjjjjjjjjJJJLk.....",
		"....kJJJJjjjjjjjjjjjjjjJJJk.....",
		"...kLJJJjjjjjjjjjjjjjjjjJJLk....",
		"...kJJJjjjjjjjjjjjjjjjjjjJJk....",
		"...kJJjjjjjjjjjjjjjjjjjjjjJk....",
		"....kJjjjjjjjjjjjjjjjjjjjjJk....",
		"....kkJjjjjjjjjjjjjjjjjjjJkk....",
		"......kkJjjjjjjjjjjjjjjJkk......",
		".........kkjjjjjjjjjjjkk........",
		"...........kkkjjjjjkkk..........",
		"..............kTtTk.............",
		"..............kTtTk.............",
		"..............kTtTk.............",
		"..............kTtTk.............",
		".............kTTtTTk............",
		"............kTTTtTTTk...........",
		"...........kkkkkkkkkkk..........",
		".........ssssssssssssss.........",
		".......ssssssssssssssssss.......",
		".........ssssssssssssss.........",
		"................................",
		"................................",
		"................................",
		"................................",
		"................................",
	],
	BOULDERS:
	[
		"................................",
		"................................",
		"................................",
		"..........kkkkkk................",
		"........kkRRRRRRkk..............",
		".......kRRRRRRRRRRk.............",
		"......kRRRRrrrrrRRrk............",
		".....kRRRrrrrrrrrrrrk...........",
		".....kRRrrrrrrrrrrrrk.kkkkk.....",
		"....kRRrrrrrmrrrrrrrkkRRRRkk....",
		"....kRrrrrrmmrrrrrrrkRRrrrrRk...",
		"....kRrrrrrrrrrrrqqrkRrrrrrrrk..",
		"...kRrrrrrrrrrrrqqqrkRrrrrrrrk..",
		"...kRrrrrrrrrrrqqqqqkrrrrrrqrk..",
		"...krrrrrrrrrrqqqqqqkrrrrrqqrk..",
		"...krrrrrrrrrqqqqqqqkrrrrqqqqk..",
		"...kqrrrrrrrqqqqqqqqkrrrqqqqqk..",
		"....kqrrrrrqqqqqqqqqkrrqqqqqqk..",
		"....kqqrrrqqqqqqqqqqkqqqqqqqk...",
		".....kqqqqqqqqqqqqqkkqqqqqqk....",
		"......kkqqqqqqqqqqkk.kkqqqkk....",
		"........kkkkkkkkkk.....kkk......",
		".....ssssssssssssssssssssss.....",
		"...ssssssssssssssssssssssssss...",
		".....ssssssssssssssssssssss.....",
		"................................",
		"................................",
		"................................",
		"................................",
		"................................",
		"................................",
		"................................",
	],
	CRAG:
	[
		"................................",
		"................................",
		"............kk..................",
		"...........kRRk.................",
		"..........kRRRRk................",
		".........kRRRRRrk...............",
		"........kRRRRrrrrk..............",
		"........kRRRrrrrrrk.............",
		".......kRRRrrrrrrrrk............",
		".......kRRrrrrrrrrrk............",
		"......kRRrrrrrrrrrrrk...........",
		"......kRrrrrrrrrrqqrk...........",
		".....kRrrrrrrrrrqqqqrk..........",
		".....kRrrrrrrrrqqqqqqk..........",
		"....kRrrrrrrrrqqqqqqqqk.........",
		"....krrrrrrrrqqqqqqqqqk.........",
		"...kRrrrrrrrqqqqqqqqqqqk........",
		"...krrrrrrrqqqqqqqqqqqqk........",
		"..kmrrrrrrqqqqqqqqqqqqqqk.......",
		"..kmmrrrrqqqqqqqqqqqqqqqk.......",
		"..kqmrrrqqqqqqqqqqqqqqqqk.......",
		"..kqqqrqqqqqqqqqqqqqqqqqk.......",
		"...kqqqqqqqqqqqqqqqqqqqk........",
		"....kkqqqqqqqqqqqqqqqkk.........",
		"......kkkkkkkkkkkkkkk...........",
		"....ssssssssssssssssssss........",
		"..ssssssssssssssssssssssss......",
		"....ssssssssssssssssssss........",
		"................................",
		"................................",
		"................................",
		"................................",
	],
	OAK_B:
	[
		"................................",
		"................................",
		"................................",
		"..........kkkk......kkk.........",
		"........kkddddkk..kkdddkk.......",
		".......kdgglllgdkkdgllggdk......",
		"......kdgglllllgddggllgggdk.....",
		".....kdggglllllggggglllgggdk....",
		"....kdgggglllllgggggllgggggdk...",
		"....kdggggglllgggggggggggggdk...",
		"...kdgggggggggggggggggggggggdk..",
		"..kdgggllgggggggggggggglllggggdk",
		"..kdgglllgggggggggggggglllggggdk",
		"..kdggglllggggggggggggggllgggddk",
		"..kddggggggggggggggggggggggggddk",
		"...kddgggggggggggggggggggggdddk.",
		"...kdddggggggggggggggggggggdddk.",
		"....kddddgggggggggggggggddddk...",
		".....kdddddddggggggddddddddk....",
		"......kkkdddddddddddddddkkk.....",
		".........kkkddddddddkkk.........",
		"............kkTTtTkk............",
		"..............kTttTk............",
		"..............kTttTk............",
		".............kTTtttTk...........",
		"............kTTTtttTTk..........",
		"...........kkkkkkkkkkkk.........",
		"........ssssssssssssssssss......",
		"......ssssssssssssssssssssss....",
		"........ssssssssssssssssss......",
		"................................",
		"................................",
	],
	OAK_C:
	[
		"................................",
		".............kkkk...............",
		"...........kkddddkk.............",
		"..........kdgglllgdk............",
		".........kdggllllggdk...........",
		"........kdgglllllgggdk..........",
		"........kdgggllllggggdk.........",
		".......kdgggglllggggggdk........",
		".......kdggggggggggggggdk.......",
		".......kdggglllgggggggggdk......",
		".......kdgglllggggggggggdk......",
		".......kddggllgggggggggddk......",
		"........kddgggggggggggddk.......",
		"........kdddgggggggggdddk.......",
		".........kddddgggggddddk........",
		"..........kkdddddddddkk.........",
		"............kkkdddkkk...........",
		"..............kTtTk.............",
		"..............kTtTk.............",
		"..............kTtTk.............",
		"..............kTtTk.............",
		"..............kTttk.............",
		"..............kTttk.............",
		".............kTTttTk............",
		"............kTTTttTTk...........",
		"...........kkkkkkkkkkk..........",
		".........ssssssssssssss.........",
		".......ssssssssssssssssss.......",
		".........ssssssssssssss.........",
		"................................",
		"................................",
		"................................",
	],
	PINE_B:
	[
		"................................",
		"................................",
		"...............kk...............",
		"..............kPPk..............",
		"..............kPpk..............",
		".............kPppDk.............",
		"............kPPppDDk............",
		"...........kPPpppDDDk...........",
		"..........kkPppppDDDkk..........",
		"............kPpppDDk............",
		"...........kPPppppDDk...........",
		"..........kPPpppppDDDk..........",
		".........kPPppppppDDDDk.........",
		"........kPPPpppppppDDDDk........",
		".......kkkkPpppppppDDDkkkk......",
		"..........kPPpppppDDDDk.........",
		".........kPPppppppDDDDDk........",
		"........kPPPppppppppDDDDk.......",
		".......kPPPppppppppppDDDDk......",
		"......kPPPpppppppppppDDDDDk.....",
		".....kkkkkPpppppppppDDDDkkkkk...",
		"........kPPpppppppppDDDDDk......",
		".......kPPPppppppppppDDDDDk.....",
		"......kPPPpppppppppppppDDDDk....",
		".....kkkkkkkkkkTttTkkkkkkkkkk...",
		"..............kTttTk............",
		".............kTTttTTk...........",
		"............kkkkkkkkkk..........",
		"..........ssssssssssssss........",
		"........ssssssssssssssssss......",
		"..........ssssssssssssss........",
		"................................",
	],
	JUNGLE_B:
	[
		"................................",
		"................................",
		".......kkk..........kkk.........",
		"......kLLLk..kkkk..kLLLk........",
		".....kLJJJLkkLLLLkkLJJJLk.......",
		"....kLJjjjJLkLJJLkLJjjjJLk......",
		"....kJjjjjjJLJjjJLJjjjjjJk......",
		"...kLJjjjjjjJjjjjJjjjjjjJLk.....",
		"...kJjjjjjjjjjjjjjjjjjjjjJk.....",
		"...kJjjjjjjjjjjjjjjjjjjjjjk.....",
		"....kJjjjjjjjjjjjjjjjjjjjJk.....",
		"....kkJjjjjjjjjjjjjjjjjjJkk.....",
		"......kkJjjjjjjjjjjjjjJkk.......",
		"........kkjjjjjjjjjjjkk.........",
		"..........kkkjjjjjkkk...........",
		".............kkTtTkk............",
		"..............kTtTk.............",
		"..............kTtTk.............",
		"..............kTtTk.............",
		"..............kTtTk.............",
		"..............kTtTk.............",
		".............kTTtTTk............",
		"............kTTTtTTTk...........",
		"...........kkkkkkkkkkk..........",
		".........ssssssssssssss.........",
		".......ssssssssssssssssss.......",
		".........ssssssssssssss.........",
		"................................",
		"................................",
		"................................",
		"................................",
		"................................",
	],
}

static var _cache: Dictionary = {}


static func kind_for_terrain(terrain: int) -> int:
	if terrain < 0 or terrain >= _TERRAIN_KIND.size():
		return -1
	return _TERRAIN_KIND[terrain]


# Per-cell variety within a family on the planner's cell hash: the rock
# family alternates boulder clusters and crags, oaks split three ways,
# pines and jungle trees two ways.
static func variant_for(kind: int, h: float) -> int:
	var v := fposmod(h * 977.0, 1.0)
	match kind:
		BOULDERS:
			return CRAG if v < 0.4 else BOULDERS
		OAK:
			return OAK if v < 0.4 else (OAK_B if v < 0.72 else OAK_C)
		PINE:
			return PINE if v < 0.55 else PINE_B
		JUNGLE:
			return JUNGLE if v < 0.5 else JUNGLE_B
	return kind


# The family head of a kind (OAK for every oak variant, ...).
static func family_of(kind: int) -> int:
	return FAMILY[kind] if kind >= 0 and kind < FAMILY.size() else kind


static func kind_image(kind: int) -> Image:
	var rows: Array = _ROWS[kind]
	var img := Image.create(CELL_PX, CELL_PX, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	for y in CELL_PX:
		var row: String = rows[y]
		for x in CELL_PX:
			var c := row[x]
			if c == "s":
				img.set_pixel(x, y, Color(0.04, 0.10, 0.07, 0.45))
			elif c != ".":
				img.set_pixel(x, y, Color(PAL[c]))
	return img


# The first trunk row (a row painting trunk keys), where the 2.5D crown /
# trunk cut lands (sprite_split.gd, D9): the crown above covers figures, the
# trunk below draws under them.
static func trunk_row(kind: int) -> int:
	var rows: Array = _ROWS[kind]
	for y in rows.size():
		var row: String = rows[y]
		if row.contains("T") or row.contains("t"):
			return y
	return CELL_PX


static func kind_texture(kind: int) -> ImageTexture:
	if _cache.has(kind):
		return _cache[kind]
	var tex := ImageTexture.create_from_image(kind_image(kind))
	_cache[kind] = tex
	return tex


static func opaque_pixels(img: Image) -> int:
	var count := 0
	for y in img.get_height():
		for x in img.get_width():
			if img.get_pixel(x, y).a > 0.0:
				count += 1
	return count
