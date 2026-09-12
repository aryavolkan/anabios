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

enum { BUSH, OAK, CACTUS, BOULDER, ACACIA, JUNGLE, PINE, SHRUB }
const PROP_COUNT := 8
const PROP_NAMES: PackedStringArray = [
	"Bush", "Oak", "Cactus", "Boulder", "Acacia", "Jungle", "Pine", "Shrub"
]

const ATLAS_COLS := 8
const CELL_PX := 16

# --- ground tile pixel maps -------------------------------------------------
# Char keys per terrain: first entry is the base fill; accents stay within the
# hue family except rare feature pixels (flowers, stones) kept to a few px.
# The green families sit a step brighter than the biome.rs canon on purpose:
# tiles now carry 90% of the land colour (terrain.gdshader tile_mix) and the
# canon's dark forest read as dusk in every capture; the minimap still uses
# the canon colours, so the two agree in hue, not value.

const _TILE_PALS: Array = [
	{"w": "173070", "k": "12265c", "h": "2a4a94"},
	{"a": "4a8a3a", "d": "3d7530", "l": "5fa84a", "f": "e6cc60"},
	{"b": "236a2e", "d": "1a5223", "l": "2f8a3c", "m": "3fa04a"},
	{"e": "ad9454", "d": "947c42", "r": "c4ac68", "t": "7a6a4a"},
	{"r": "6b6673", "d": "575260", "l": "807a8a", "k": "494452"},
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
			"aaaaadaaaaalaaaa",
			"aalaaaaaadaaaaad",
			"aaaaaaalaaaalaaa",
			"adaaaalaaaaaaaaa",
			"aaaaaaaaaadaaala",
			"aaalaaaaaaaaaaaa",
			"aaaaaaadaaaalaaa",
			"alaaaafaaaaaaada",
			"aaaaadaaalaaaaaa",
			"aaaaaaaaaaaaalaa",
			"aadaalaaadaaaaaa",
			"aaaaaaaaaaaaaaad",
			"alaaaaaadaaalaaa",
			"aaaaadaaaaaafaaa",
			"aaalaaaaaalaadaa",
			"adaaaaalaaaaaaaa",
		],
		[
			"aaadaaaalaaaaada",
			"aaaaaalaaaaaaaaa",
			"alaaaaaaaadaalaa",
			"aaaaadaaaaaaaaaa",
			"aaaaaaaalaaadaaa",
			"adaalaaaaaaaaaal",
			"aaaaaaaaadaaaaaa",
			"aaalaaadaaaalaaa",
			"aaaaaaaaaaaaaada",
			"adaaaalaaalaaaaa",
			"aaaaaaaaaaaaaaaa",
			"aalaadaaaadaalaa",
			"aaaaaaaalaaaaaaa",
			"afaaaaaaaaaaadaa",
			"aaaadaalaaaaaaaa",
			"aaaaaaaaaadaalaa",
		],
	],
	[
		[
			"bbbdbbbbblbbbbbb",
			"bbbbbbdbbbbbmbbb",
			"blbbbbbbbdbbbbbb",
			"bbbbdbbbbbbblbbb",
			"bbbbbbmbbbbbbbbd",
			"bdbbbbbbblbbbbbb",
			"bbbblbbbbbbdbbbb",
			"bbbbbbbbmbbbbblb",
			"bmbbbdbbbbbbbbbb",
			"bbbbbbbblbbdbbbb",
			"bblbbbbbbbbbbmbb",
			"bbbbbdbbbbbbbbbb",
			"bdbbbbbblbbbdbbb",
			"bbbbmbbbbbbbbbbl",
			"bbbbbbbdbbmbbbbb",
			"blbbbbbbbbbbdbbb",
		],
		[
			"bbbbbbdbbbblbbbb",
			"bmbbbbbbbbbbbbdb",
			"bbbbdbbblbbbbbbb",
			"bbbbbbbbbbbdbmbb",
			"blbbbbdbbbbbbbbb",
			"bbbbbbbbbmbbblbb",
			"bbdbbbbbbbbbbbbd",
			"bbbbblbbdbbbbbbb",
			"bbbbbbbbbbblbbmb",
			"bdbbmbbbbbbbbbbb",
			"bbbbbbbdbblbbbbb",
			"bblbbbbbbbbbbbdb",
			"bbbbbbmbbdbbbbbb",
			"bbbdbbbbbbbbblbb",
			"blbbbbbbbmbbbbbb",
			"bbbbbdbbbbbbdbbb",
		],
	],
	[
		[
			"eeeeeeeeeeeeeeee",
			"eerreeeeeeeerree",
			"eeeeeeeeeeeeeeee",
			"erreeeeeeeerrree",
			"eeeeedeeeeeeeeee",
			"eeeeeeeeeeeeeeee",
			"eeeeeeerrrreeeee",
			"eedeeeeeeeeedeee",
			"eeeeeeeeeeeeeeee",
			"errreeeeteeeeeee",
			"eeeeeeeedteeeeee",
			"eeeeeeeeeeeerrre",
			"eeeedeeeeeeeeeee",
			"eeeeeeeeeeeeeeee",
			"erreeeeeerrreeee",
			"eeeeeeeeeeeeeeee",
		],
		[
			"eeeeeeerreeeeeee",
			"eeeeeeeeeeeeeeee",
			"eedeeeeeeeerrree",
			"eeeeeeeeeeeeeeee",
			"errrreeeeedeeeee",
			"eeeeeeeeeeeeeeee",
			"eeeeeeedeeeerree",
			"eeteeeeeeeeeeeee",
			"eedeeeeeerrrreee",
			"eeeeeeeeeeeeeeee",
			"eeeeerreeeeeedee",
			"eeeeeeeeeeeeeeee",
			"erreeeeeeteeeeee",
			"eeeeeeeeedeeeeee",
			"eeeeedeeeeeerrre",
			"eeeeeeeeeeeeeeee",
		],
	],
	[
		[
			"rrrrrrrrrrlrrrrr",
			"rrdrrrrrrrrrrrrr",
			"rrrkrrrrrrrdrrrr",
			"rrrrkrrlrrrrrrrr",
			"rrrrrkrrrrrrrlrr",
			"rlrrrrrrrrdrrrrr",
			"rrrrrrrrrrrrrrrr",
			"rrrrdrrrrkkrrrrr",
			"rrrrrrrrkrrrrdrr",
			"rrlrrrrkrrrrrrrr",
			"rrrrrrrrrrrlrrrr",
			"rdrrrrrrrrrrrrrr",
			"rrrrrlrrdrrrkrrr",
			"rrrrrrrrrrrrrkrr",
			"rrdrrrrrlrrrrrkr",
			"rrrrrrrrrrrrrrrr",
		],
		[
			"rrrrrdrrrrrrlrrr",
			"rrrrrrrrrkrrrrrr",
			"rlrrrrrrrrkrrdrr",
			"rrrrrrrrrrrkrrrr",
			"rrrdrrlrrrrrrrrr",
			"rrrrrrrrrrrrrrlr",
			"rrkrrrrrrdrrrrrr",
			"rrrkrrrrrrrrrrrr",
			"rrrrkrlrrrrdrrrr",
			"rrrrrrrrrrrrrrrr",
			"rdrrrrrrrlrrkrrr",
			"rrrrrrrrrrrkrrrr",
			"rrrrlrrdrrkrrrrr",
			"rrrrrrrrrrrrrrdr",
			"rrrdrrrrrrrrlrrr",
			"rlrrrrrrdrrrrrrr",
		],
	],
	[
		[
			"ssssdssssslsssss",
			"sslssssstsssssds",
			"ssssssstssslssss",
			"sdsssslsssssssss",
			"ssssssssssdsssls",
			"ssstssssssssssss",
			"sssssssdsssslsss",
			"slsssssssssssdss",
			"sssssdssslssssss",
			"ssssssssssssslst",
			"ssdsslssstssssss",
			"ssssssssssssssds",
			"slssssssdssslsss",
			"sssssdsssssstsss",
			"ssslsssssslssdss",
			"sdsssstlssssssss",
		],
		[
			"ssstssslssssdsss",
			"ssssssssssssssss",
			"sdssslssssdsssts",
			"ssssssssssssssss",
			"sslssstsssssldss",
			"ssssssssdsssssss",
			"stsssssssslsssss",
			"ssssdsssssssssds",
			"sssssslsstssssss",
			"sdsssssssssslsss",
			"ssssssdsssssssst",
			"sssslssssdssssss",
			"stssssssssssslss",
			"sssssssdssssssss",
			"sslssdssssltssss",
			"ssssssssssssssds",
		],
	],
	[
		[
			"jjdjjljjjjdjjjjl",
			"jjjjjjjdjjjjmjjj",
			"jljjdjjjjjljjjjd",
			"jjjjjjljjdjjjljj",
			"jdjjjjjjjjjjdjjj",
			"jjjmjjdjjljjjjjj",
			"jjjjjjjjjjjjmjjd",
			"jjdjjljjjdjjjjjj",
			"jjjjjjjjjjjjljjj",
			"jljjjdjmjjdjjjjj",
			"jjjjjjjjjjjjjjdj",
			"jjdjjjjljjjjljjj",
			"jjjjmjjjjdjjjjjj",
			"jdjjjjjjjjjjjmjj",
			"jjjjljjdjjljjjjj",
			"jjjjjjjjjjjjdjjj",
		],
		[
			"jjjjdjjjljjjjdjj",
			"jjmjjjjjjjjjjjjl",
			"jjjjjjdjjjmjjjjj",
			"jljjjjjjjjjjdjjj",
			"jjjjdjjljjjjjjmj",
			"jjjjjjjjjjdjjjjj",
			"jdjjjjmjjjjjljjj",
			"jjjjljjjjdjjjjjj",
			"jjjjjjjjjjjjjdjl",
			"jjdjmjjljjjjjjjj",
			"jjjjjjjjjdjjjjjj",
			"jljjjjdjjjjjmjjj",
			"jjjjjjjjljjjjjjd",
			"jjjdjjjjjjdjjjjj",
			"jmjjjjljjjjjjdjj",
			"jjjjjdjjjjjljjjj",
		],
	],
	[
		[
			"ggggdgggglgggggg",
			"ggngggggggggggdg",
			"ggggggglgggngggg",
			"gdgggggggggggllg",
			"gggggnggggdggggg",
			"gglgggggggggggng",
			"ggggggdggglggggg",
			"glggggggngggggdg",
			"gggggdggglgggggg",
			"ggggggggggggglgn",
			"ggdgglggggnggggg",
			"gggggggggggggggd",
			"glggggggdggglggg",
			"gggggdgggggggngg",
			"ggglggggggglggdg",
			"gdggggnlgggggggg",
		],
		[
			"gggnggglggggdggg",
			"gggggggggggggggg",
			"gdggglggggdgggng",
			"gggggggggggggggg",
			"gglgggngggggldgg",
			"ggggggggdggggggg",
			"gngggggggglggggg",
			"ggggdggggggggggd",
			"ggggggglggnggggg",
			"gdggggggggggglgg",
			"ggggggdggggggggn",
			"gglgggggggdggggg",
			"ggggnggglggggggg",
			"ggggggggggggdggg",
			"gglggggdgggggngg",
			"ggggggggglgggggg",
		],
	],
	[
		[
			"uuuudkuuuuluuuuu",
			"uuluuuuuuduuuuud",
			"uuuuuuukuuuluuuu",
			"uduuuukuuuuuuuuu",
			"uuuuuuuuuuduuulu",
			"uuuluuuuuuuuuuuu",
			"uuuuuuuduuuukuuu",
			"uluuuuuuuuuuuudu",
			"uuuuuduuukuuuuuu",
			"uuuuuuuuuuuuuluu",
			"uuduuluuuduuuuuu",
			"uuuuuuuuuuuuuuud",
			"uluuuuuuduuukuuu",
			"uuuuuduuuuuuuuuu",
			"uuuluuuuuuluuduu",
			"uduuuuukuuuuuuuu",
		],
		[
			"uukuuuluuuuduuuu",
			"uuuuuuuuuuuuuuuu",
			"uduuuluuuuduuuku",
			"uuuuuuuuuuuuuuuu",
			"uuluuukuuuuulduu",
			"uuuuuuuuduuuuuuu",
			"ukuuuuuuuuluuuuu",
			"uuuuduuuuuuuuudu",
			"uuuuuuluukuuuuuu",
			"uduuuuuuuuuuluuu",
			"uuuuuuduuuuuuuuk",
			"uuluuuuuuuduuuuu",
			"uuuukuuuluuuuuuu",
			"uuuuuuuuuuuuduuu",
			"uuluuuuduuuuukuu",
			"uuuuuuuuuluuuuuu",
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
]

# Suggested scatter prop per terrain; water stays bare.
const _TERRAIN_PROP: PackedInt32Array = [
	-1, BUSH, OAK, CACTUS, BOULDER, ACACIA, JUNGLE, PINE, SHRUB
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
	return _TERRAIN_PROP[terrain]


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
