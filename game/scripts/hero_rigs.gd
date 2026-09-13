extends RefCounted
# Hand-authored 24 px hero silhouettes for the quadruped archetypes (the
# "hand-authored 24 px silhouettes" tier above the scaled rect rigs). One
# master per archetype, facing right, feet on row 21, rows 0..3 free for
# antlers and ears. Every pixel carries a PART tag as well as a zone
# colour, and the 16 pose cells the field shader expects (stand, three gait
# poses, graze A/B, fight A/B, alert A/B, flee A/B, sleep A/B, celebrate
# A/B) are derived by moving parts: legs swing for the gait, the head and a
# stretched neck dip to graze, the figure drops onto folded legs to sleep.
# Cells are shaded and outlined by ApeSprites, and packed through
# ApeSprites.pack_cells; apes keep their rect rigs (weapon poses).
#
# Key legend: c coat / u underside / d dark marking / w white marking (body);
# H head coat / D head dark / W head white / e eye / n nose (head); N neck;
# F front leg / f front hoof; B back leg / b back hoof; T tail / t tail tip;
# A horn or antler / I ivory (ride with the head).

const ApeSprites = preload("res://scripts/ape_sprites.gd")

const PX := 24
const CELL_COUNT := 16

enum Part { BODY, HEAD, NECK, FRONT, BACK, TAIL }

# key -> [part, colour] on the neutral quad ramp (coat_hue modulates it).
const KEYS := {
	"c": [Part.BODY, Color(0.60, 0.60, 0.60)],
	"u": [Part.BODY, Color(0.92, 0.92, 0.92)],
	"d": [Part.BODY, Color(0.42, 0.42, 0.42)],
	"w": [Part.BODY, Color(0.96, 0.96, 0.96)],
	"H": [Part.HEAD, Color(0.60, 0.60, 0.60)],
	"D": [Part.HEAD, Color(0.42, 0.42, 0.42)],
	"W": [Part.HEAD, Color(0.96, 0.96, 0.96)],
	"e": [Part.HEAD, Color(0.10, 0.10, 0.11)],
	"n": [Part.HEAD, Color(0.32, 0.30, 0.30)],
	"A": [Part.HEAD, Color(0.30, 0.28, 0.25)],
	"I": [Part.HEAD, Color(0.96, 0.94, 0.86)],
	"N": [Part.NECK, Color(0.60, 0.60, 0.60)],
	"F": [Part.FRONT, Color(0.60, 0.60, 0.60)],
	"f": [Part.FRONT, Color(0.35, 0.34, 0.34)],
	"B": [Part.BACK, Color(0.54, 0.54, 0.54)],
	"b": [Part.BACK, Color(0.32, 0.31, 0.31)],
	"T": [Part.TAIL, Color(0.60, 0.60, 0.60)],
	"t": [Part.TAIL, Color(0.42, 0.42, 0.42)],
}
# Draw order, back to front, so the head (and its antlers) overlays the neck.
const DRAW_ORDER: Array = [Part.TAIL, Part.BACK, Part.BODY, Part.NECK, Part.FRONT, Part.HEAD]

# Keyed by mammal_sprites archetype NAME (upper-case), see masters_for().
const MASTERS := {
	"HARE":
	[
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"..............H..H......",
		"..............HH.HH.....",
		"..............HHHHH.....",
		"...............HHHH.....",
		"..............HHHeHH....",
		".............HHHHHHn....",
		"........ccccccHHHH......",
		"......ccccccccccH.......",
		".....cccccccccccc.......",
		"....ccccccccccccc.......",
		"....ccccccccccccc.......",
		"...Tcccuuuuuuucc........",
		"..TTcccuuuuuuuc.........",
		"....BBB......FF.........",
		"....BBB.....FF..........",
		"....bbbb....ff..........",
		"........................",
		"........................",
	],
	"DEER":
	[
		"..............A...A.....",
		"..............AA.AA.....",
		"...............A.A......",
		"...............AAA......",
		"..............HHHH......",
		".............HHHeHHn....",
		".............HHHHHHn....",
		"............NNHHH.......",
		"............NNH.........",
		"...........NNN..........",
		"....ccccccccNN..........",
		"..TcccccccccccN.........",
		".TTccddddddddccc........",
		".Tcccccccccccccc........",
		"..ccccccccccccc.........",
		"...uuuuuuuuuuu..........",
		"...BB...BB.FF..FF.......",
		"...BB...BB.FF..FF.......",
		"...BB...BB.FF..FF.......",
		"...BB...BB.FF..FF.......",
		"...BB...BB.FF..FF.......",
		"...bb...bb.ff..ff.......",
		"........................",
		"........................",
	],
	"BOAR":
	[
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		".............HH.........",
		"....ddddddddHHHH........",
		"...ddddddddddHHHH.......",
		"..cccccccccccHHeHHH.....",
		".TccccccccccccHHHHn.....",
		"..ccccccccccccccHHIn....",
		"..cccccccccccccccH......",
		"...cccccccccccccc.......",
		"...uuuuuuuuuuuuu........",
		"...BB..BB..FF..FF.......",
		"...BB..BB..FF..FF.......",
		"...BB..BB..FF..FF.......",
		"...bb..bb..ff..ff.......",
		"........................",
		"........................",
		"........................",
	],
	"FOX":
	[
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"..............H...H.....",
		"..............HH.HH.....",
		"..............HHHHH.....",
		"...............HHHeHH...",
		"...............WHHHHHn..",
		"...............WWHH.....",
		"TT............NNHW......",
		"TTT.........ccNN........",
		".TTT.....cccccc.........",
		"..TTt..cccccccc.........",
		"...ttcccccccccc.........",
		"....cccuuuuuucc.........",
		".....BB......FF.........",
		".....BB......FF.........",
		".....bb......ff.........",
		"........................",
		"........................",
		"........................",
	],
	"WOLF":
	[
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"..............H..H......",
		"..............HH.HH.....",
		"..............HHHHH.....",
		".............HHHHeHH....",
		".............HHHHHHHn...",
		"...........NNHHHDDHn....",
		"..........NNNNHHHH......",
		"...ddddddddccNN.........",
		"..ccddddddddccc.........",
		".Tccccccccccccc.........",
		".Tcccccccccccc..........",
		".TTcccuuuuuuucc.........",
		"..TT.BB...BB.FF..FF.....",
		".....BB...BB.FF..FF.....",
		".....BB...BB.FF..FF.....",
		".....bb...bb.ff..ff.....",
		"........................",
		"........................",
		"........................",
	],
	"LIVESTOCK":
	[
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"...............A..A.....",
		"...............AHHA.....",
		"..............HHHHHH....",
		"..............HHeHHHH...",
		"..............HHHHHWW...",
		"...cccccccccccNHHHWW....",
		"..cwwwccccccccNH........",
		".Tcwwwcccccwwwcc........",
		".Tcccccccccwwwcc........",
		".Tcccccccccccccc........",
		"..ccuuuuuuuuuucc........",
		"...BB...BB..FF..FF......",
		"...BB...BB..FF..FF......",
		"...BB...BB..FF..FF......",
		"...bb...bb..ff..ff......",
		"........................",
		"........................",
		"........................",
	],
	"TORTOISE":
	[
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........dddddddd........",
		"......ddccccccccdd......",
		".....dcccddddcccccd.....",
		"....dccdddccccdddccd....",
		"....dccdccccccccdccd....",
		"....dcccccccccccccd.....",
		".....dddddddddddddHHH...",
		"....uuuuuuuuuuuuuHHeHn..",
		"...T.uuuuuuuuuuuuHHHH...",
		"....BBB.......FFF.......",
		"....BBB.......FFF.......",
		"....bbb.......fff.......",
		"........................",
		"........................",
		"........................",
	],
	"PORCUPINE":
	[
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		"........................",
		".....d..d..d..d.........",
		"....wd.wd.wd.wd.........",
		"...dwddwddwddwd.........",
		"..ddddddddddddddd.......",
		".dddddddddddddddd.HH....",
		".dddccccccccccccccHHH...",
		"..ddcccccccccccccHHeHn..",
		"...ccccccccccccccHHHH...",
		"...ccccccccccccccc......",
		"....uuuuuuuuuuuuu.......",
		".....BB.......FF........",
		".....BB.......FF........",
		".....bb.......ff........",
		"........................",
		"........................",
		"........................",
		"........................",
	],
	"MAMMOTH":
	[
		"........................",
		"........................",
		".......dddd.............",
		".....dddddddd...........",
		"....ddddddddddd.........",
		"...ccddddddddddd........",
		"..cccccddddddddHHHH.....",
		".cccccccccccccHHHHHH....",
		".cccccccccccccHHHeHHH...",
		"TccccccccccccccHHHHHHH..",
		"TccccccccccccccHHHHHHH..",
		"TccccccccccccccHHHHIHH..",
		".ccccccccccccccHHHIIHH..",
		".cccccccccccccccHHHHNH..",
		".ccccccccccccccc.HHHNN..",
		"..ccccccccccccc...INNN..",
		"...uuuuuuuuuuuu....IN...",
		"...BBB..BBB.FFF..FFF....",
		"...BBB..BBB.FFF..FFF....",
		"...BBB..BBB.FFF..FFF....",
		"...BBB..BBB.FFF..FFF....",
		"...bbb..bbb.fff..fff....",
		"........................",
		"........................",
	],
	"WADER":
	[
		"........................",
		"........................",
		"........................",
		"................HH......",
		"...............HHHH.....",
		"..............HHeHHnn...",
		"...............HHH......",
		"...............NN.......",
		"..............NN........",
		"..............NN........",
		".............NN.........",
		"......ccccccNN..........",
		"....ccccccccccc.........",
		"...Tcccddddccccc........",
		"..TTccccccccccc.........",
		"...Tccccccccccc.........",
		"....ccuuuuuucc..........",
		"......B....F............",
		"......B....F............",
		"......B....F............",
		"......B....F............",
		".....bbb..fff...........",
		"........................",
		"........................",
	],
}


static func has(archetype_name: String) -> bool:
	return MASTERS.has(archetype_name.to_upper())


# The master's pixels split by part: Part -> Array of [x, y, Color].
static func parts_of(rows: Array) -> Dictionary:
	var out: Dictionary = {}
	for p in Part.values():
		out[p] = []
	for y in rows.size():
		var row: String = rows[y]
		for x in row.length():
			var ch: String = row[x]
			if ch == "." or not KEYS.has(ch):
				continue
			var spec: Array = KEYS[ch]
			out[spec[0]].append([x, y, spec[1]])
	return out


# Pose recipe: per part a [dx, dy] shift; "extend" parts draw at both the
# original and the shifted spot (a neck that stretches down to graze);
# "fold" crops the legs to their top `fold` fraction and drops everything
# else onto them (sleep); "eyes_closed" paints the eye in coat colour.
static func pose_recipe(index: int) -> Dictionary:
	match index:
		1:
			return {Part.FRONT: [1, 0], Part.BACK: [-1, 0]}
		2:
			return {Part.BODY: [0, -1], Part.HEAD: [0, -1], Part.NECK: [0, -1], Part.TAIL: [0, -1]}
		3:
			return {Part.FRONT: [-1, 0], Part.BACK: [1, 0]}
		4:
			return {Part.HEAD: [1, 3], Part.NECK: [1, 3], "extend": [Part.NECK]}
		5:
			return {Part.HEAD: [1, 4], Part.NECK: [1, 4], "extend": [Part.NECK]}
		6:
			return {Part.HEAD: [1, 0], Part.NECK: [1, 0], "extend": [Part.NECK]}
		7:
			return {Part.HEAD: [2, 0], Part.NECK: [2, 0], Part.FRONT: [1, 0], "extend": [Part.NECK]}
		8:
			return {Part.HEAD: [0, -1], Part.NECK: [0, -1], "extend": [Part.NECK]}
		9:
			return {
				Part.HEAD: [0, -1], Part.NECK: [0, -1], Part.TAIL: [0, -1], "extend": [Part.NECK]
			}
		10:
			return {
				Part.FRONT: [2, 0],
				Part.BACK: [-2, 0],
				Part.BODY: [0, -1],
				Part.HEAD: [1, -1],
				Part.NECK: [1, -1],
				Part.TAIL: [0, -1],
			}
		11:
			return {Part.FRONT: [-1, 0], Part.BACK: [1, 0], Part.HEAD: [1, 0], Part.NECK: [1, 0]}
		12:
			return {"fold": 0.4, Part.HEAD: [0, 2], Part.NECK: [0, 2], "extend": [Part.NECK]}
		13:
			return {
				"fold": 0.4,
				Part.HEAD: [0, 3],
				Part.NECK: [0, 3],
				"extend": [Part.NECK],
				"eyes_closed": true,
			}
		14:
			return {
				Part.HEAD: [0, -1], Part.NECK: [0, -1], Part.TAIL: [0, -2], "extend": [Part.NECK]
			}
		15:
			return {
				Part.HEAD: [0, -1], Part.NECK: [0, -1], Part.TAIL: [1, -1], "extend": [Part.NECK]
			}
		_:
			return {}


# One pose cell (unflipped, shaded, outlined) from a master.
static func build_cell(rows: Array, index: int) -> Image:
	var parts: Dictionary = parts_of(rows)
	var recipe: Dictionary = pose_recipe(index)
	var img := Image.create(PX, PX, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	var drop := 0
	var fold: float = float(recipe.get("fold", 0.0))
	if fold > 0.0:
		# Legs keep their top `fold` share; the body drops by what was cut.
		var leg_top := PX
		var leg_bottom := 0
		for p in [Part.FRONT, Part.BACK]:
			for px in parts[p]:
				leg_top = mini(leg_top, int(px[1]))
				leg_bottom = maxi(leg_bottom, int(px[1]))
		var leg_h: int = leg_bottom - leg_top + 1
		var keep: int = maxi(1, int(round(float(leg_h) * fold)))
		drop = leg_h - keep
		for p in [Part.FRONT, Part.BACK]:
			var kept: Array = []
			for px in parts[p]:
				if int(px[1]) < leg_top + keep:
					kept.append(px)
			parts[p] = kept
	var extend: Array = recipe.get("extend", [])
	var eyes_closed: bool = bool(recipe.get("eyes_closed", false))
	var coat: Color = KEYS["c"][1]
	for p in DRAW_ORDER:
		var shift: Array = recipe.get(p, [0, 0])
		var dx: int = int(shift[0])
		# Everything, folded legs included, sits lower by the cut leg height.
		var dy: int = int(shift[1]) + drop
		for px in parts[p]:
			var col: Color = px[2]
			if eyes_closed and p == Part.HEAD and col.r < 0.2:
				col = coat
			if extend.has(p):
				_put(img, int(px[0]), int(px[1]) + drop, col)
			_put(img, int(px[0]) + dx, int(px[1]) + dy, col)
	ApeSprites.shade(img, PX)
	ApeSprites.outline(img, PX)
	return img


static func _put(img: Image, x: int, y: int, col: Color) -> void:
	if x >= 0 and y >= 0 and x < PX and y < PX:
		img.set_pixel(x, y, col)


# All 16 pose cells for an archetype name.
static func build_cells(archetype_name: String) -> Array:
	var rows: Array = MASTERS[archetype_name.to_upper()]
	var out: Array = []
	for i in CELL_COUNT:
		out.append(build_cell(rows, i))
	return out


static func opaque_pixels(img: Image) -> int:
	var count := 0
	for y in img.get_height():
		for x in img.get_width():
			if img.get_pixel(x, y).a > 0.0:
				count += 1
	return count
