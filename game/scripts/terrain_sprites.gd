extends RefCounted
# Terrain tile + decoration prop asset library for the pixel-art ground.
#
# One 16x16 tile family per core TerrainType (biome.rs), each in the SAME hue
# family as `cell_color()`'s canonical base so tiled ground, climate-lerp
# ground, and the minimap agree. Two hand-authored variants per terrain plus a
# mirrored third break tiling repetition. Decoration props (trees, bush,
# cactus, boulder, shrub) are figures on transparency for a scatter layer.
#
# All art packs into ONE square nearest-filtered atlas (ATLAS_COLS x
# ATLAS_COLS cells of CELL_PX): extreme-aspect atlases corrupt on the Metal
# renderer, so the grid stays square like ape_sprites' pose atlases.

enum { WATER, GRASS, FOREST, DESERT, ROCK, SAVANNA, RAINFOREST, TAIGA, TUNDRA }
const TERRAIN_COUNT := 9
const NAMES: PackedStringArray = [
	"Water", "Grass", "Forest", "Desert", "Rock", "Savanna", "Rainforest", "Taiga", "Tundra"
]
# Variants 0 and 1 are hand-authored; variant 2 is variant 0 mirrored.
const VARIANTS := 3

enum { BUSH, OAK, CACTUS, BOULDER, ACACIA, JUNGLE, PINE, SHRUB, FLOWERS, TUFT, MUSHROOM, STUMP }
const PROP_COUNT := 12
const PROP_NAMES: PackedStringArray = [
	"Bush",
	"Oak",
	"Cactus",
	"Boulder",
	"Acacia",
	"Jungle",
	"Pine",
	"Shrub",
	"Flowers",
	"Tuft",
	"Mushroom",
	"Stump",
]

const ATLAS_COLS := 8
const CELL_PX := 16

# --- ground tile pixel maps -------------------------------------------------
# Char keys per terrain: first entry is the base fill; accents stay within the
# hue family except rare feature pixels (flowers, stones) kept to a few px.
# The land maps are mottled ground (2-3 px dark and light clumps kept a
# pixel apart, tufts, cracks on rock, a flower or two on grass) rather than
# single-pixel speckle, so a field reads as turf at 2x instead of confetti.
# Rock is a neutral grey (the canon's violet cast read as purple ground).
# The green families sit a step brighter than the biome.rs canon on purpose:
# tiles now carry 90% of the land colour (terrain.gdshader tile_mix) and the
# canon's dark forest read as dusk in every capture; the minimap still uses
# the canon colours, so the two agree in hue, not value.

const _TILE_PALS: Array = [
	{"w": "173070", "k": "12265c", "h": "2a4a94"},
	{"a": "4a8a3a", "d": "3d7530", "l": "5fa84a", "f": "e6cc60", "w": "f2f0dc"},
	{"b": "236a2e", "d": "1a5223", "l": "2f8a3c", "m": "3fa04a"},
	{"e": "ad9454", "d": "947c42", "r": "c4ac68", "t": "7a6a4a"},
	{"r": "6f7278", "d": "585b62", "l": "868a90", "k": "45484e"},
	{"s": "b8a85c", "d": "9c8c48", "l": "ccbc70", "t": "857a3a"},
	{"j": "1f7a3a", "d": "166030", "l": "2c9a4c", "m": "45b45e"},
	{"g": "3a7a5c", "d": "2c604a", "l": "4a9270", "n": "245240"},
	{"u": "9ea89e", "d": "8a948a", "l": "b2bcb2", "k": "76806f"},
]

const _TILE_MAPS: Array = [
	[
		[
			"wwwwwwwwwwwwwwww",
			"wwhhhwwwwwwwwwww",
			"wwwwwwwwwwkwwwww",
			"wwwwwwwwwwwwwwww",
			"wwwwwwhhwwwwwwhh",
			"wkwwwwwwwwwwwwww",
			"wwwwwwwwwwwwwwww",
			"wwwhhhwwwwkwwwww",
			"wwwwwwwwwwwwwwww",
			"wwwwwwwwwhhwwwww",
			"wkwwwwwwwwwwwwkw",
			"wwwwwwwwwwwwwwww",
			"wwhhwwwwwhhhwwww",
			"wwwwwwwwwwwwwwww",
			"wwwwwkwwwwwwwwww",
			"wwwwwwwwwwhhwwww",
		],
		[
			"wwwwwwwwhhwwwwww",
			"wwwwwwwwwwwwwwww",
			"wwkwwwwwwwwwhhhw",
			"wwwwwwwwwwwwwwww",
			"wwwwhhwwwkwwwwww",
			"wwwwwwwwwwwwwwww",
			"whhhwwwwwwwwwkww",
			"wwwwwwwwhhwwwwww",
			"wwwwwkwwwwwwwwww",
			"wwwwwwwwwwwhhhww",
			"wwwwwwwwwwwwwwww",
			"wwhhwwwkwwwwwwww",
			"wwwwwwwwwwwwwwww",
			"wwwwwwwwwwkwwhhw",
			"wwwwwwwwwwwwwwww",
			"wwwkwwwwhhwwwwww",
		],
	],
	[
		[
			"aaaaaaaadadaaaaa",
			"aaaaaaaaaaaaaaaa",
			"aaaaaaaaaaaaaaaa",
			"aaaaadddaaaaadaa",
			"daaaaaaaaaaaaaad",
			"aaaaaaaaaaaaaaaa",
			"daallaaaaaaaaaad",
			"aaalaaadaaadaaad",
			"aaaaaaadaaaadaaa",
			"aaaaaaaaaaaaaaaa",
			"llaaadaaaaaallaa",
			"aaaadadaaaaalaaa",
			"aaaaaaaaaaaaaaaa",
			"aaaaaddaafaaaaaw",
			"aaaaaadaaaaallaa",
			"aaaaaaaaadaalaaa",
		],
		[
			"aaafaaaaaaaaaaaa",
			"alaaaadaaaaalaaa",
			"alaaaaaaaaaalaaa",
			"aaaadaawaaaaaaaa",
			"aaaaadaaaaaaaaaa",
			"aaaaaaaaaaaaaaaa",
			"adalaaaaaaaadaaa",
			"aaalaaaaaaadadaa",
			"aaaaaaaaadaaaaaa",
			"aaaaaaaadadaaaaa",
			"aaaaaaaaaaaaalla",
			"aaaaaaaaaaaaaaaa",
			"aadaaaaaaaadaaaa",
			"aaaaadaadaaadaaa",
			"aaaaaaaaaaaaaaaa",
			"aaaaaaaaaaaaaaaa",
		],
	],
	[
		[
			"bblbbbbbbbbbbbbb",
			"bbbbbbbdbbbbbdbb",
			"bblbddbdbbbbbbdb",
			"bbbbbdbbbbbbbbbb",
			"ddbbbbbbbbddbbbb",
			"bdbbbbbbbbbdbbbb",
			"bbbbbbbbbbbbbbbb",
			"blbbbbbbbbbbbddd",
			"blbbdbbbbbbbbbbb",
			"bbbbdbbbbbbbbbdb",
			"dbbbbbbbbbbbbbbb",
			"bdbbbbbbbbbbbbbd",
			"bbbbbbbdbbbbbbbb",
			"mbbbbbdbdbmbbbbb",
			"bbbbbbbbbbbbbbbb",
			"bllbbbbbbbbbbbbb",
		],
		[
			"bbbbbbbbbbbbbbbb",
			"bbbbbbbdbbbbbbbb",
			"bbbbbbbbdbddbbdd",
			"bbbbbbbbbbbbbbbd",
			"bbbbbblbbbbmbbbb",
			"bbbbbbbbbbbbbbbb",
			"bbbmbbllbbbbbbbb",
			"bbbbbbblbbbddbbb",
			"bbbbbdbbbbbbdbbb",
			"bbbbdbdbbbbbbbbb",
			"bbbbbbbbbbbbdbbb",
			"bbbbbbbbbbbbdbbb",
			"dbbbbbbddbbbbbdd",
			"bbbbbbbbbbbbbbbb",
			"bbbbbbbblbbdbbdb",
			"bbbbbbbbbbdbdbbd",
		],
	],
	[
		[
			"eeeeeeerrreeeeee",
			"eeeeeeeeeeeeeeee",
			"eeeeeeeeeeedeeee",
			"eeeeeeeeeeeeerre",
			"reeeeeeeeeeeeeee",
			"reeeeeeeeeeeeete",
			"eeeddeeeeeeeeeee",
			"eeeedeeeeeeeeeee",
			"eeeeeeeeeeeeeeee",
			"eeeeeeeeeeeerree",
			"eereeedeeeeeeeee",
			"eereeeeeeeeeeeee",
			"eeeeeeeeeeeeeeee",
			"eeeeeeeeeeeeeeer",
			"reeeeeeeeeeeeeee",
			"eeeeeeeeeeeeeeee",
		],
		[
			"eeeeeeeeeeeeeeee",
			"eeeeeereeeeeeeee",
			"eeeeeeeeeeeeeeee",
			"eeeeeeeeeeeeeeee",
			"eeeeeeeeeeeeeete",
			"eeeeeeeeeeeeeeee",
			"eeeeeeeeeeerrree",
			"eeeeeeeeeeeeeeee",
			"eeerreeeereeeeee",
			"ereereeeeeeeeeee",
			"eereeeeeeeeeeeee",
			"eeeeeeeeedddeeee",
			"eeeeeeeeeeeeeeee",
			"eeeeeeeeeeeeddee",
			"reeeeeeeeeeeeder",
			"reeeedddeeeeeeee",
		],
	],
	[
		[
			"rrrrrrrrrrdddrrr",
			"llrrrrrrrrrrrrrr",
			"lrrrdrrrrllrrrrr",
			"rrrrrdrrrlrrkdrr",
			"rrrrrrrrrrrrrrdd",
			"rrrrrrrrrrrrrdrr",
			"rrrrrrrrrrrrrrdr",
			"rddrrrrrrrrrrrrd",
			"ddrrrrrrrrrrrrrr",
			"rdrrrrdrrrrrrrrr",
			"rrrrrrrrrrrllrrr",
			"rrrrrrrrrrrrlrrr",
			"rrrrrrrrrrrrrrlr",
			"rrrrrrrrrrrrrrrr",
			"rrrrrrrrrrrrrrrr",
			"rrrrrrrrrrrrrrrr",
		],
		[
			"rrrrrrrrrrrrrrrr",
			"lrrrrrrrrdddrrrr",
			"lrrrrrrrrrrrrrrr",
			"rrrrrrrrrrrrrrkr",
			"rrrrrrrrrrrrrrrr",
			"rdrrrrrrrrrrrrrr",
			"rrrrrrrrrrrrrllr",
			"rrrrrddrrrrrrrrr",
			"rlrrrdrrrrrrrrrr",
			"rrlrrrrrrrrrrlrr",
			"rrrrrrrrrrrrrrlr",
			"rrrrrrrrrrrrrrrr",
			"rrrrrrrrrrrrrrdr",
			"rrrrrrrrdddrrrrd",
			"ddddrrrrrrrrrrrr",
			"rrrrrrrrrrrrrrrr",
		],
	],
	[
		[
			"ssssssststsdddss",
			"sssddsssssssssss",
			"ssssssssssssssss",
			"ssssdssssssllsss",
			"sssssdsssssslssl",
			"sssssssssssssssl",
			"slllssssssssssss",
			"ssssssssssssssss",
			"ssssssssssdddsss",
			"ssssssssssssssss",
			"dsssssssssssssdd",
			"ssssssslssssssss",
			"ssssssssssssssss",
			"sssssssssssstsss",
			"sssstsssssststss",
			"ssststsstsssssss",
		],
		[
			"dsssssssssssssss",
			"ssddssssssstssss",
			"ssdsssdssststsss",
			"ssssssdsssssssss",
			"ssssssssssslssss",
			"dssssssssssssssd",
			"sssssssdddssssss",
			"ssssssssssssssss",
			"ssssssssssssssss",
			"ssssssssssssssll",
			"ssslllsssssllssl",
			"sssssssssssslsss",
			"ssssstssssssssss",
			"sssststsssssssss",
			"sssssssstsssssss",
			"dsssssststssssss",
		],
	],
	[
		[
			"djjjjdjjjjjjjjjj",
			"jdjjjdjjjjjjjjjj",
			"jjjjjjjdjjjddjjj",
			"jdjjjjjjdjjjdjjj",
			"djdjjjjjjjjjjjjj",
			"jjjjjjjjjjjjjjjj",
			"jjjjjjjjjjmjjjjj",
			"jjljjjddjjjjjjjj",
			"jjljjjdjjjjdjjlj",
			"jjjjjjjjjjjjjjjl",
			"jjjjjjljjjjjjjjj",
			"jlljjjjjjjjjjjjj",
			"jjljjjjjmjjjjjjj",
			"jjjjdjjjjjjjjddj",
			"jjjjdjjjjjjjjjdj",
			"jjjjjjjjjjjjjjjj",
		],
		[
			"jjjjjjjjddjjjjmj",
			"jjjjjjjjdjjjjjjj",
			"jjjjjjjjjjjjjjjj",
			"jjjjjllljjjjjjjm",
			"jjlljjjjjjjjjjjj",
			"djjljjjjjjjjjjjd",
			"jjjjjjjjjjjjjjjj",
			"jjjdjjjjjjjjjjjj",
			"jjdjdjlljjjjjjdd",
			"jjjjjjjljjjjjjjd",
			"jjjjjjjjjjjjjjjj",
			"jjjdddjjjjjjjjjj",
			"jjjjjjjjjdjjjjjj",
			"lljjjjjjjjdjjjjl",
			"jjjdjddjjjjjjjjj",
			"jjjdjjdjjjjdjjjj",
		],
	],
	[
		[
			"ggggggddgggggddg",
			"gggggggdggggggdg",
			"gggggggggggggggg",
			"ggdgggggggggggdd",
			"gdgdgddggggggggd",
			"gggggdgggggddggg",
			"ggggggggggggdggg",
			"glllgggggllggggg",
			"gggggggggglggggg",
			"ggggggggggggnggg",
			"ggggggggdggggggg",
			"ggggglgdgdgggggg",
			"nggggggggggggggg",
			"ggggggggddgggggg",
			"ggggggggdggggggg",
			"gggggggggggggggg",
		],
		[
			"ggggggngdggggggg",
			"ggggggggdggggggg",
			"ggggggggggdggggg",
			"gggggggggggdgllg",
			"gggggggggggggggg",
			"lgggggggddgggggl",
			"lggggggggdgggggg",
			"gggggggggggggggg",
			"ggggggddgggggggg",
			"gggdgggggggdgggg",
			"dgdgdgggggdgdgdd",
			"gggggggggggggggg",
			"dddggllggggggggg",
			"ggggglgggggggggg",
			"gggggggggggggggn",
			"gggggggggggggggg",
		],
	],
	[
		[
			"uuuuuuuuuuuuuuuu",
			"uuuuuuuluuuuuuuu",
			"uuuuuuuluuuuuudu",
			"uuuuuuuuuuuuudud",
			"uuuuuuuuuuuuuuuu",
			"uuuuuuuuuuuuuuuu",
			"uuuuuuuuuuuuuuuu",
			"dddulukuuullluuu",
			"uuuuuuuuuuuuuuuu",
			"uuuuuuuuuuuuuuuu",
			"ukuuuuuuuuuuuuuu",
			"uuuullluuuuuuuuu",
			"uuuuuuuuuuuuuuuu",
			"udduuuuuuuuuuuuu",
			"uduuudduluuuuuuu",
			"uuuuuuduuuuuuduu",
		],
		[
			"uuuuuuuuuuuuuuuu",
			"uuuuuuukuuuuuuuu",
			"uuullluuuuuuuuuu",
			"duuuuuuuuuuuuuuu",
			"uduuuuuuuuuuuuuu",
			"uuuuuuuuuuuuuuuu",
			"luuuuuuuuuuuuuul",
			"uuuuuuuuuuuuuuuu",
			"uuuuuuuuuuuudddu",
			"uuuuuuuluuuuuuuu",
			"uuuuuuuuluuuuduu",
			"uuuuuuuuuuuududu",
			"luuuuuulluuuuuuu",
			"luuukuuuuuuuuuuu",
			"uuuuuuuuuuuuuuuu",
			"uuuuuuuuudduuduu",
		],
	],
]

# --- decoration prop pixel maps ('.' = transparent) -------------------------

const _PROP_PALS: Array = [
	{"g": "2c5f28", "m": "3f7a34", "l": "55964a", "r": "b03a3a"},
	{"t": "6b4a2e", "g": "1d5426", "m": "2a6b30", "l": "3f8a3c"},
	{"g": "3f7a34", "d": "2c5f28", "l": "55964a"},
	{"d": "575260", "b": "6b6673", "l": "807a8a"},
	{"m": "6b7a2c", "d": "55632a", "t": "7a5a34"},
	{"d": "07331a", "m": "1d7a38", "l": "36a35c", "t": "5a4426", "v": "3fae62"},
	{"e": "1f4634", "m": "37795a", "g": "4f9a6a", "t": "5f4128"},
	{"t": "6a5a44", "s": "b8c2b8"},
	{"g": "3f7a34", "l": "55964a", "r": "d24a4a", "y": "f0c850", "w": "f2f0dc"},
	{"g": "3f7a34", "d": "2c5f28", "l": "55964a"},
	{"r": "c24a3a", "w": "f2ebd8", "t": "d9c8a0", "d": "7a4a30"},
	{"t": "8a6a42", "b": "5a3c22", "r": "a88a5a", "g": "3f7a34"},
]

const _PROP_MAPS: Array = [
	[
		"................",
		"................",
		"................",
		"................",
		"................",
		"......mm........",
		"....mmllmm......",
		"...mllmmllm.....",
		"..gmlmmrmmlg....",
		"..gmmlmmmlmg....",
		"...gmmrmmmg.....",
		"....ggmmgg......",
		"................",
		"................",
		"................",
		"................",
	],
	[
		"................",
		"....gggggg......",
		"...gmmmmmmg.....",
		"..gmmllmmmmg....",
		"..gmllmmmmlg....",
		"..gmmmmlmmmg....",
		"...gmmmmmmg.....",
		"....ggmmgg......",
		"......tt........",
		"......tt........",
		"......tt........",
		".....ttt........",
		"................",
		"................",
		"................",
		"................",
	],
	[
		"................",
		"......dl........",
		"......gl........",
		"......gl........",
		"..dl..gl..dl....",
		"..gl..gl..gl....",
		"..gl..gl..gl....",
		"..gld.gl.dgl....",
		"...dggglggd.....",
		"......gl........",
		"......gl........",
		"......gl........",
		"......gl........",
		"................",
		"................",
		"................",
	],
	[
		"................",
		"................",
		"................",
		"................",
		"................",
		".....dbbbd......",
		"....dbbllbd.....",
		"...dbblllbbd....",
		"...dbbbllbbd....",
		"...dbbbbbbdd....",
		"....ddbbbdd.....",
		".....ddddd......",
		"................",
		"................",
		"................",
		"................",
	],
	[
		"................",
		"................",
		"................",
		"...ddmmmmmdd....",
		"..dmmmmmmmmmd...",
		"...ddmmmmmdd....",
		".......t........",
		".......t........",
		"......tt........",
		"......t.........",
		".....tt.........",
		"................",
		"................",
		"................",
		"................",
		"................",
	],
	[
		"....dmmmmd......",
		"..dmmmllmmmd....",
		".dmmlmmmmlmmd...",
		".dmmmmlmmmmd....",
		"..dmmmmmmmd.....",
		"...dmmlmmd......",
		"....dmmmd.......",
		"......tt...v....",
		"......tt...v....",
		"......tt...v....",
		"......tt........",
		".....ttt........",
		"................",
		"................",
		"................",
		"................",
	],
	[
		"................",
		".......m........",
		"......gme.......",
		".....gmmme......",
		"......mme.......",
		".....gmmmee.....",
		"....gmmmmmee....",
		".....mmmee......",
		"....gmmmmee.....",
		"...gmmmmmmeee...",
		"..gmmmmmmmmeee..",
		"......tt........",
		"......tt........",
		"......tt........",
		"................",
		"................",
	],
	[
		"................",
		"................",
		"................",
		"................",
		"................",
		"....t...t.......",
		"....t..t..t.....",
		".....t.t.t......",
		"......ttt.......",
		".......t........",
		".......t........",
		"....ssttss......",
		"...ssssssss.....",
		"................",
		"................",
		"................",
	],
	[
		"................",
		"................",
		"................",
		"................",
		"................",
		"......r.........",
		".....rrr...y....",
		"..w...r...yyy...",
		".www..g....y....",
		"..w...g....g....",
		"..g...g.l..g....",
		"..g..lg.g..g....",
		".lggglgggglggl..",
		"................",
		"................",
		"................",
	],
	[
		"................",
		"................",
		"................",
		"................",
		"................",
		"................",
		".......l........",
		"..l....l..l.....",
		"..l.l..l..l.l...",
		"...ll.ll.ll.l...",
		"...dl.dl.dl.d...",
		"....ddddddd.....",
		"................",
		"................",
		"................",
		"................",
	],
	[
		"................",
		"................",
		"................",
		"................",
		"......rrrr......",
		".....rwrrrr.....",
		"....rrrrrwrr....",
		"....rrwrrrrr....",
		"....dddddddd....",
		".....ttttt......",
		".....tttt.......",
		".....ttttrr.....",
		".....tttrwrr....",
		"....ddtddddd....",
		"................",
		"................",
	],
	[
		"................",
		"................",
		"................",
		"................",
		"................",
		"......rrrrr.....",
		".....rttttrr....",
		".....rtrrrtr....",
		".....rtrrrtr....",
		".....bttttrb....",
		".....bbbbbbb....",
		".....bbbbbbb....",
		"....gbbbbbbbg...",
		"....gg.....gg...",
		"................",
		"................",
	],
]

# Scatter props per terrain (water stays bare): the first entry is the
# terrain's head prop, the rest are the clutter a cell may grow instead
# (flowers and tufts on grass, mushrooms and stumps under the trees), picked
# per cell by prop_variant_for() so the ground reads as lived-in rather than
# one bush repeated. A kind listed twice is twice as likely (mushrooms stay
# an accent under the trees, not a polka dot over the whole wood).
const _TERRAIN_PROPS: Array = [
	[],
	[BUSH, FLOWERS, TUFT],
	[OAK, OAK, STUMP, MUSHROOM],
	[CACTUS],
	[BOULDER],
	[ACACIA, TUFT],
	[JUNGLE, JUNGLE, MUSHROOM],
	[PINE, PINE, STUMP],
	[SHRUB, TUFT],
]

# ---------------------------------------------------------------------------


static func _sprite(rows: Array, pal: Dictionary) -> Image:
	var img := Image.create(CELL_PX, CELL_PX, false, Image.FORMAT_RGBA8)
	if rows.size() != CELL_PX:
		push_error("pixel map has %d rows, want %d" % [rows.size(), CELL_PX])
		return img
	for y in rows.size():
		var row: String = rows[y]
		if row.length() != CELL_PX:
			push_error("pixel map row %d has %d chars, want %d" % [y, row.length(), CELL_PX])
			continue
		for x in row.length():
			var c := row[x]
			if c != ".":
				img.set_pixel(x, y, Color(pal[c]))
	return img


static func tile_image(terrain: int, variant: int) -> Image:
	var maps: Array = _TILE_MAPS[terrain]
	if variant < 2:
		return _sprite(maps[variant], _TILE_PALS[terrain])
	var img := _sprite(maps[0], _TILE_PALS[terrain])
	img.flip_x()
	return img


static func prop_image(kind: int) -> Image:
	return _sprite(_PROP_MAPS[kind], _PROP_PALS[kind])


static func prop_for_terrain(terrain: int) -> int:
	if terrain < 0 or terrain >= _TERRAIN_PROPS.size():
		return -1
	var kinds: Array = _TERRAIN_PROPS[terrain]
	return -1 if kinds.is_empty() else int(kinds[0])


# All the props a terrain may grow (empty for water).
static func props_for_terrain(terrain: int) -> Array:
	if terrain < 0 or terrain >= _TERRAIN_PROPS.size():
		return []
	return _TERRAIN_PROPS[terrain]


# The prop a cell with hash `h` grows on `terrain`: an even pick from the
# terrain's list on a re-scrambled hash (h itself already decided that the
# cell grows anything at all).
static func prop_variant_for(terrain: int, h: float) -> int:
	var kinds: Array = props_for_terrain(terrain)
	if kinds.is_empty():
		return -1
	var v := fposmod(h * 977.0, 1.0)
	return int(kinds[mini(int(v * kinds.size()), kinds.size() - 1)])


# Atlas cell index for a tile variant: tiles fill the grid first, row-major.
static func atlas_cell_for_tile(terrain: int, variant: int) -> int:
	return terrain * VARIANTS + variant


# Atlas cell index for a prop: props follow all tile variants.
static func atlas_cell_for_prop(kind: int) -> int:
	return TERRAIN_COUNT * VARIANTS + kind


# Every tile variant and prop packed into one square nearest-filtered atlas.
static func build_atlas() -> ImageTexture:
	var side := ATLAS_COLS * CELL_PX
	var atlas := Image.create(side, side, false, Image.FORMAT_RGBA8)
	for t in TERRAIN_COUNT:
		for v in VARIANTS:
			_blit_cell(atlas, tile_image(t, v), atlas_cell_for_tile(t, v))
	for p in PROP_COUNT:
		_blit_cell(atlas, prop_image(p), atlas_cell_for_prop(p))
	return ImageTexture.create_from_image(atlas)


static func _blit_cell(atlas: Image, img: Image, cell: int) -> void:
	var cx := (cell % ATLAS_COLS) * CELL_PX
	var cy := int(cell / float(ATLAS_COLS)) * CELL_PX
	atlas.blit_rect(img, Rect2i(0, 0, CELL_PX, CELL_PX), Vector2i(cx, cy))
