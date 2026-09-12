extends RefCounted
# 16x16 pixel icons for the HUD (Phase 5 of
# docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md): the top
# bar's counters and one glyph per invention for the research panel. Row-
# string block art on a tiny shared palette, same technique as
# terrain_sprites.gd, drawn transparent outside the mark with a 1px dark
# outline so they read on any panel or terrain.

const PX := 16

const PAL := {
	"k": "14100f",  # outline
	"y": "f2c14e",  # sun / gold
	"o": "d98f4a",  # amber
	"r": "c23a34",  # red
	"g": "6fa040",  # leaf green
	"G": "3f7a2a",  # dark green
	"b": "5ca0d8",  # sky blue
	"B": "2f6f9c",  # deep blue
	"w": "e7ded0",  # bone / paper
	"s": "8a8a96",  # stone
	"S": "5b5b66",  # dark stone
	"t": "7a4c2c",  # timber
	"T": "4a2f1e",  # dark timber
	"c": "c9a24a",  # thatch / wheat
	"p": "cf8f8f",  # skin
	"m": "b98a5a",  # tan
	"v": "9b6fd0",  # violet (culture)
}

enum {
	SUN,
	PEOPLE,
	TRADE,
	ERA,
	HEART,
	LEAF,
	SWORD,
	SHIELD,
	SKULL,
	DNA,
	DROP,
	EYE,
	LEGS,
	MEAT,
	BOOK,
	PAW,
	HOME
}
const KIND_COUNT := 17

# Invention keys (anabios-core invention table) -> icon rows below.
const INVENTION_ICONS := {
	"stone_tools": "stone_tools",
	"fire": "fire",
	"farming": "farming",
	"metalworking": "metalworking",
	"writing": "writing",
	"medicine": "medicine",
	"husbandry": "husbandry",
	"machinery": "machinery",
	"electricity": "electricity",
	"nuclear_power": "nuclear_power",
}

const _ROWS := {
	"sun":
	[
		"................",
		".......y........",
		"...y...y...y....",
		"....y.....y.....",
		".....yyyyy......",
		"....yyyyyyy.....",
		".y..yyyyyyy..y..",
		"yyy.yyyyyyy.yyy.",
		".y..yyyyyyy..y..",
		"....yyyyyyy.....",
		".....yyyyy......",
		"....y.....y.....",
		"...y...y...y....",
		".......y........",
		"................",
		"................",
	],
	"people":
	[
		"................",
		"....pp....pp....",
		"....pp....pp....",
		"....pp....pp....",
		"...bbbb..bbbb...",
		"..bbbbbb.bbbbb..",
		"..bbbbbb.bbbbb..",
		"..b.bbb..b.bbb..",
		"....bb....bb....",
		"....bb....bb....",
		"...TTTT..TTTT...",
		"...T..T..T..T...",
		"...T..T..T..T...",
		"................",
		"................",
		"................",
	],
	"trade":
	[
		"................",
		"................",
		"...tttttttttt...",
		"..tTTTTTTTTTTt..",
		"..tTccccccccTt..",
		"..tTcTTTTTTcTt..",
		"..tTccccccccTt..",
		"..tTTTTTTTTTTt..",
		"..tTccccccccTt..",
		"..tTcTTTTTTcTt..",
		"..tTccccccccTt..",
		"..tTTTTTTTTTTt..",
		"...tttttttttt...",
		"................",
		"................",
		"................",
	],
	"era":
	[
		"................",
		".....s..s..s....",
		"....ssssssss....",
		"...ssSSSSSSss...",
		"..sSSs....sSSs..",
		"..sSs..ss..sSs..",
		"...Ss.sSSs.sS...",
		"...Ss.sSSs.sS...",
		"..sSs..ss..sSs..",
		"..sSSs....sSSs..",
		"...ssSSSSSSss...",
		"....ssssssss....",
		".....s..s..s....",
		"................",
		"................",
		"................",
	],
	"heart":
	[
		"................",
		"................",
		"...rrr...rrr....",
		"..rrrrr.rrrrr...",
		"..rrrrrrrrrrr...",
		"..rrrrrrrrrrr...",
		"..rrrrrrrrrrr...",
		"...rrrrrrrrr....",
		"....rrrrrrr.....",
		".....rrrrr......",
		"......rrr.......",
		".......r........",
		"................",
		"................",
		"................",
		"................",
	],
	"leaf":
	[
		"................",
		"..........gg....",
		"........gggg....",
		".......ggggg....",
		"......gggGgg....",
		".....gggGggg....",
		"....gggGgggg....",
		"....ggGggggg....",
		"...ggGgggggg....",
		"...gGggggggg....",
		"...Ggggggg......",
		"..G.............",
		".G..............",
		"................",
		"................",
		"................",
	],
	"sword":
	[
		"................",
		"...........ss...",
		"..........sss...",
		".........sss....",
		"........sss.....",
		".......sss......",
		"......sss.......",
		"..T..sss........",
		"..TTsss.........",
		"...TTT..........",
		"..TTTT..........",
		".TT.TT..........",
		"TT..............",
		"................",
		"................",
		"................",
	],
	"shield":
	[
		"................",
		"...ssssssssss...",
		"..sSSSSSSSSSSs..",
		"..sSbbbbbbbbSs..",
		"..sSbbbbbbbbSs..",
		"..sSbbbrrbbbSs..",
		"..sSbbrrrrbbSs..",
		"..sSbbbrrbbbSs..",
		"..sSbbbbbbbbSs..",
		"...sSbbbbbbSs...",
		"....sSbbbbSs....",
		".....sSbbSs.....",
		"......sSSs......",
		".......ss.......",
		"................",
		"................",
	],
	"stone_tools":
	[
		"................",
		"........ssss....",
		".......sSSSSs...",
		".......sSSSSs...",
		"........ssss....",
		".......tt.......",
		"......tt........",
		".....tt.........",
		"....tt..........",
		"...tt...........",
		"..tt............",
		".tt.............",
		"................",
		"................",
		"................",
		"................",
	],
	"fire":
	[
		"................",
		".......o........",
		"......oo........",
		"......ooo.......",
		".....oooo.......",
		".....ooyoo......",
		"....ooyyoo......",
		"....oyyyyoo.....",
		"...ooyyyyoo.....",
		"...ooyyyyyoo....",
		"...ooyyyyyoo....",
		"....ooyyyoo.....",
		".....ooooo......",
		"......ooo.......",
		"................",
		"................",
	],
	"farming":
	[
		"................",
		"......c.........",
		".....ccc..c.....",
		"......c..ccc....",
		"..c...c...c.....",
		".ccc..c...c.....",
		"..c...c...c.....",
		"..c...c...c.....",
		"..c...c...c.....",
		"..c...c...c.....",
		".GGGGGGGGGGGGG..",
		".GGGGGGGGGGGGG..",
		"..TTTTTTTTTTT...",
		"................",
		"................",
		"................",
	],
	"metalworking":
	[
		"................",
		"................",
		"....ssssssss....",
		"...sSSSSSSSSs...",
		"...sSSSSSSSSs...",
		"....sssSSsss....",
		".......SS.......",
		".......SS.......",
		".......SS.......",
		"......sSSs......",
		".....sSSSSs.....",
		"....ssssssss....",
		"................",
		"................",
		"................",
		"................",
	],
	"writing":
	[
		"................",
		"...wwwwwwwww....",
		"..wwwwwwwwwww...",
		"..wwTTTTTTwww...",
		"..wwwwwwwwwww...",
		"..wwTTTTTwwww...",
		"..wwwwwwwwwww...",
		"..wwTTTTTTwww...",
		"..wwwwwwwwwww...",
		"..wwTTTwwwwww...",
		"..wwwwwwwwwww...",
		"...wwwwwwwww....",
		"................",
		"................",
		"................",
		"................",
	],
	"medicine":
	[
		"................",
		"......rrrr......",
		"......rrrr......",
		"......rrrr......",
		"..rrrrrrrrrrrr..",
		"..rrrrwwwwrrrr..",
		"..rrrrwwwwrrrr..",
		"..rrrrrrrrrrrr..",
		"......rrrr......",
		"......rrrr......",
		"......rrrr......",
		"................",
		"................",
		"................",
		"................",
		"................",
	],
	"husbandry":
	[
		"................",
		"................",
		"....mm....mm....",
		"...mmmmmmmmmm...",
		"..mmmmmmmmmmmm..",
		"..mmmmmmmmmmmm..",
		"..mmwmmmmmmwmm..",
		"..mmmmmmmmmmmm..",
		"...mmmmmmmmmm...",
		"...mm..mm..mm...",
		"...mm..mm..mm...",
		"...TT..TT..TT...",
		"................",
		"................",
		"................",
		"................",
	],
	"machinery":
	[
		"................",
		"......t.........",
		".....ttt........",
		"....ttttt.......",
		"...tt.t.tt......",
		"..tt..t..tt.....",
		"..t...t...t.....",
		"..ttttttttt.....",
		"..t...t...t.....",
		"..tt..t..tt.....",
		"...tt.t.tt......",
		"....ttttt.......",
		".....ttt........",
		"......t.........",
		"................",
		"................",
	],
	"electricity":
	[
		"................",
		"........yyy.....",
		".......yyy......",
		"......yyy.......",
		".....yyy........",
		"....yyyyyyy.....",
		".....yyyyyy.....",
		"......yyyy......",
		".....yyy........",
		"....yyy.........",
		"...yyy..........",
		"..yy............",
		"................",
		"................",
		"................",
		"................",
	],
	"nuclear_power":
	[
		"................",
		".....vvvvvv.....",
		"....vv....vv....",
		"...vv..vv..vv...",
		"...v..vvvv..v...",
		"..vv.vvvvvv.vv..",
		"..v..vvvvvv..v..",
		"..vv..vvvv..vv..",
		"...v...vv...v...",
		"...vv......vv...",
		"....vv....vv....",
		".....vvvvvv.....",
		"................",
		"................",
		"................",
		"................",
	],
	"skull":
	[
		"................",
		".....wwwwww.....",
		"....wwwwwwww....",
		"...wwwwwwwwww...",
		"...wwwwwwwwww...",
		"...wwkkwwwkkw...",
		"...wkkkwwkkkw...",
		"...wwkkwwwkkw...",
		"...wwwwwwwwww...",
		"....wwwwwwww....",
		".....wwkwkw.....",
		".....wwwwww.....",
		"......w.w.w.....",
		"................",
		"................",
		"................",
	],
	"dna":
	[
		"................",
		"...bb.....bb....",
		"....bb...bb.....",
		".....bbbbb......",
		"......bbb.......",
		".....bb.bb......",
		"....bb...bb.....",
		"...bb.....bb....",
		"...bb.....bb....",
		"....bb...bb.....",
		".....bb.bb......",
		"......bbb.......",
		".....bbbbb......",
		"....bb...bb.....",
		"...bb.....bb....",
		"................",
	],
	"drop":
	[
		"................",
		".......b........",
		".......b........",
		"......bbb.......",
		"......bbb.......",
		".....bbbbb......",
		".....bbbbb......",
		"....bbbbbbb.....",
		"....bbwbbbb.....",
		"....bbwbbbb.....",
		"....bbbbbbb.....",
		".....bbbbb......",
		"......bbb.......",
		"................",
		"................",
		"................",
	],
	"eye":
	[
		"................",
		"................",
		"................",
		".....wwwwww.....",
		"...wwwwwwwwww...",
		"..wwwwbbbbwwww..",
		".wwwwbbBBbbwwww.",
		".wwwbbBkkBbbwww.",
		".wwwbbBkkBbbwww.",
		".wwwwbbBBbbwwww.",
		"..wwwwbbbbwwww..",
		"...wwwwwwwwww...",
		".....wwwwww.....",
		"................",
		"................",
		"................",
	],
	"legs":
	[
		"................",
		"................",
		"....mmmmmmmm....",
		"...mmmmmmmmmm...",
		"...mmmmmmmmmm...",
		"...mmm....mmm...",
		"...mmm....mmm...",
		"...mm......mm...",
		"...mm......mm...",
		"...mm......mm...",
		"..mmm.....mmm...",
		"..TTT.....TTT...",
		"..TTTT....TTTT..",
		"................",
		"................",
		"................",
	],
	"meat":
	[
		"................",
		"................",
		"...........ww...",
		"..........www...",
		"........rrrww...",
		".......rrrrr....",
		"......rrprrr....",
		".....rrpprr.....",
		"....rrrppr......",
		"...rrrrrr.......",
		"..wwrrrr........",
		"..www...........",
		"..ww............",
		"................",
		"................",
		"................",
	],
	"book":
	[
		"................",
		"................",
		"..tttttt.tttttt.",
		".tttwwwtttwwwtt.",
		".ttwwwwtttwwwwt.",
		".ttwwwwtttwwwwt.",
		".ttwTwwtttwTwwt.",
		".ttwwwwtttwwwwt.",
		".ttwTwwtttwTwwt.",
		".ttwwwwtttwwwwt.",
		".ttwwwwtttwwwwt.",
		".tttwwwtttwwwtt.",
		"..tttttttttttt..",
		"...tttttttttt...",
		"................",
		"................",
	],
	"paw":
	[
		"................",
		"................",
		"....mm....mm....",
		"...mmmm..mmmm...",
		"...mmmm..mmmm...",
		".mm..mm..mm..mm.",
		"mmmm........mmmm",
		"mmmm..mmmm..mmmm",
		".mm..mmmmmm..mm.",
		"....mmmmmmmm....",
		"....mmmmmmmm....",
		"....mmmmmmmm....",
		".....mmmmmm.....",
		"................",
		"................",
		"................",
	],
	"home":
	[
		"................",
		".......cc.......",
		"......cccc......",
		".....cccccc.....",
		"....cccccccc....",
		"...cccccccccc...",
		"..cccccccccccc..",
		".cccccccccccccc.",
		"...tttttttttt...",
		"...ttttttTTtt...",
		"...ttTTttTTtt...",
		"...ttTTttTTtt...",
		"...ttttttTTtt...",
		"...tttttttttt...",
		"................",
		"................",
	],
}

const _KIND_ROWS: PackedStringArray = [
	"sun",
	"people",
	"trade",
	"era",
	"heart",
	"leaf",
	"sword",
	"shield",
	"skull",
	"dna",
	"drop",
	"eye",
	"legs",
	"meat",
	"book",
	"paw",
	"home",
]

static var _cache: Dictionary = {}


# The icon for one of the KIND_COUNT HUD glyphs.
static func kind_texture(kind: int) -> ImageTexture:
	return named_texture(_KIND_ROWS[kind])


# The icon for an invention key; a generic gear when the key is unknown.
static func invention_texture(key: String) -> ImageTexture:
	return named_texture(String(INVENTION_ICONS.get(key, "era")))


static func named_texture(name: String) -> ImageTexture:
	if _cache.has(name):
		return _cache[name]
	var tex := ImageTexture.create_from_image(build_image(name))
	_cache[name] = tex
	return tex


# 16x16 RGBA8: the mark plus a 1px dark outline around it.
static func build_image(name: String) -> Image:
	var rows: Array = _ROWS[name]
	var img := Image.create(PX, PX, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	for y in PX:
		var row: String = rows[y]
		for x in PX:
			var c := row[x]
			if c != ".":
				img.set_pixel(x, y, Color(PAL[c]))
	var outlined := Image.create(PX, PX, false, Image.FORMAT_RGBA8)
	outlined.copy_from(img)
	for y in PX:
		for x in PX:
			if img.get_pixel(x, y).a > 0.0:
				continue
			for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
				var nx: int = x + d.x
				var ny: int = y + d.y
				if nx >= 0 and ny >= 0 and nx < PX and ny < PX and img.get_pixel(nx, ny).a > 0.0:
					outlined.set_pixel(x, y, Color(PAL["k"]))
					break
	return outlined


static func opaque_pixels(img: Image) -> int:
	var count := 0
	for y in img.get_height():
		for x in img.get_width():
			if img.get_pixel(x, y).a > 0.0:
				count += 1
	return count
