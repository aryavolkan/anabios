extends RefCounted

# Procedural 16x16 "ape" avatars for the DIT agents. Built the same way as the
# body disc in main.gd (an Image filled at load), so no PNG asset ships — the
# sprite data lives here as color-block lists. The agents ARE the apes: the
# inspector shows a pinned agent's lineage as one of these five hominins,
# selected from its species id.

const NAMES: PackedStringArray = [
	"Chimpanzee",
	"Gorilla",
	"Orangutan",
	"Australopith",
	"Sapiens",
]

# Shared palette (hex, no alpha => opaque). Keys match the block lists below.
const PAL := {
	"K": "14100f",
	"x": "241f26",
	"X": "3a3340",
	"b": "4a2f1e",
	"B": "7a4c2c",
	"r": "a06a3c",
	"t": "c99a63",
	"T": "e0c090",
	"w": "e7ded0",
	"W": "f6f2e6",
	"g": "5b5b66",
	"G": "7c7c88",
	"d": "3a3a44",
	"s": "b9b9c4",
	"p": "cf8f8f",
	"P": "e6b3b3",
	"n": "b5645f",
	"o": "c06a2e",
	"O": "d98f4a",
	"R": "c23a34",
	"y": "d9a83a",
	"h": "e8e2d0",
	"m": "b98a5a",
	"e": "f4f4f4",
	"k": "0e0d10",
}

# Each ape is a list of [x, y, w, h, color_key] blocks on a 16x16 grid, drawn
# back-to-front (later blocks overwrite earlier ones), facing right.
const APES: Array = [
	# 0 Chimpanzee — dark coat, round ears, pale muzzle, lit chest/forearms
	[
		[6, 12, 2, 3, "x"],
		[9, 12, 2, 3, "x"],
		[5, 7, 6, 6, "x"],
		[9, 5, 4, 4, "x"],
		[9, 6, 4, 1, "X"],
		[8, 6, 1, 2, "X"],
		[13, 6, 1, 2, "X"],
		[10, 8, 3, 2, "m"],
		[11, 8, 2, 1, "B"],
		[11, 7, 1, 1, "e"],
		[11, 7, 1, 1, "k"],
		[4, 7, 2, 6, "x"],
		[11, 7, 2, 6, "x"],
		[7, 9, 2, 3, "X"],
		[4, 10, 2, 3, "X"],
		[11, 10, 2, 3, "X"],
		[6, 12, 2, 3, "x"],
		[9, 12, 2, 3, "x"]
	],
	# 1 Gorilla — broad shoulders, silverback, knuckle stance, lit chest/arms
	[
		[6, 12, 3, 3, "x"],
		[10, 12, 3, 3, "x"],
		[4, 6, 8, 7, "x"],
		[5, 4, 5, 3, "x"],
		[4, 7, 5, 2, "X"],
		[9, 4, 4, 4, "x"],
		[9, 5, 4, 1, "X"],
		[10, 7, 3, 2, "B"],
		[11, 7, 1, 1, "e"],
		[11, 7, 1, 1, "k"],
		[2, 7, 3, 6, "x"],
		[2, 12, 3, 2, "x"],
		[12, 7, 2, 5, "x"],
		[6, 9, 3, 3, "X"],
		[2, 10, 2, 3, "X"],
		[12, 10, 2, 3, "X"],
		[6, 12, 3, 3, "x"],
		[10, 12, 3, 3, "x"]
	],
	# 2 Orangutan — rust coat, shaggy fringe, long arms
	[
		[7, 12, 2, 3, "o"],
		[9, 12, 2, 3, "o"],
		[5, 6, 6, 6, "o"],
		[4, 7, 1, 6, "O"],
		[11, 7, 1, 6, "O"],
		[9, 3, 4, 2, "B"],
		[9, 4, 4, 4, "o"],
		[10, 6, 3, 2, "T"],
		[11, 6, 1, 1, "k"],
		[3, 6, 2, 8, "o"],
		[12, 6, 2, 7, "o"],
		[7, 12, 2, 3, "o"],
		[9, 12, 2, 3, "o"]
	],
	# 3 Australopith — small, hairy, upright biped
	[
		[8, 2, 3, 1, "x"],
		[8, 3, 3, 3, "B"],
		[9, 4, 2, 1, "r"],
		[9, 4, 1, 1, "k"],
		[7, 6, 4, 5, "B"],
		[7, 6, 1, 5, "x"],
		[6, 6, 1, 5, "B"],
		[11, 6, 1, 4, "B"],
		[7, 11, 1, 4, "B"],
		[9, 11, 1, 4, "B"],
		[8, 14, 3, 1, "K"]
	],
	# 4 Sapiens — upright toolmaker, spear in hand (the culture marker)
	[
		[5, 1, 1, 11, "b"],
		[5, 1, 1, 1, "s"],
		[8, 2, 3, 1, "b"],
		[8, 3, 3, 2, "t"],
		[10, 3, 1, 1, "k"],
		[7, 5, 4, 5, "B"],
		[5, 5, 2, 5, "t"],
		[11, 5, 1, 4, "t"],
		[7, 10, 1, 4, "t"],
		[9, 10, 2, 4, "t"],
		[7, 14, 4, 1, "K"]
	],
]

# Field figures: one 16x16 pose per cell, zone-coloured per species so each
# agent renders in its own ape's fur/skin tones instead of a flat genome tint.
# Zones: "c" coat, "s" skin (face / chest / hands), "a" accent (chest patch).
# Blocks are [x, y, w, h, zone], drawn back-to-front.
const SPECIES_COUNT := 5
const WALK_FRAME_COUNT := 4
# The atlas stacks WALK_FRAME_COUNT gait poses, then one still per action
# (eat / fight / trade) that main.gd derives from combat, trade and energy
# signals; the shader switches to those rows when INSTANCE_CUSTOM.a != 0.
const POSE_COUNT := 7
const POSE_EAT := 4
const POSE_FIGHT := 5
const POSE_TRADE := 6
# Gait: 0 neutral (idle), 1 contact-left, 2 passing (whole figure lifted 1px —
# the walk bob), 3 contact-right. The shader cycles 1→2→3→2 when moving and
# holds 0 when idle, so the stride reads as step-lift-step-lift.
const FIELD_POSES: Array = [
	# 0 neutral stand
	[
		[6, 2, 4, 4, "c"],
		[7, 4, 2, 2, "s"],
		[7, 6, 2, 1, "c"],
		[4, 6, 8, 5, "c"],
		[7, 7, 2, 2, "a"],
		[3, 7, 2, 4, "c"],
		[3, 10, 2, 1, "s"],
		[11, 7, 2, 4, "c"],
		[11, 10, 2, 1, "s"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"]
	],
	# 1 contact left — left leg planted ahead, right trails; arms counter-swing
	[
		[6, 2, 4, 4, "c"],
		[7, 4, 2, 2, "s"],
		[7, 6, 2, 1, "c"],
		[4, 6, 8, 5, "c"],
		[7, 7, 2, 2, "a"],
		[3, 8, 2, 3, "c"],
		[3, 10, 2, 1, "s"],
		[11, 6, 2, 4, "c"],
		[11, 9, 2, 1, "s"],
		[4, 11, 2, 4, "c"],
		[9, 12, 2, 3, "c"]
	],
	# 2 passing — legs gathered under the body, figure raised 1px (the bob)
	[
		[6, 1, 4, 4, "c"],
		[7, 3, 2, 2, "s"],
		[7, 5, 2, 1, "c"],
		[4, 5, 8, 5, "c"],
		[7, 6, 2, 2, "a"],
		[3, 6, 2, 4, "c"],
		[3, 9, 2, 1, "s"],
		[11, 6, 2, 4, "c"],
		[11, 9, 2, 1, "s"],
		[6, 10, 2, 4, "c"],
		[9, 10, 2, 4, "c"]
	],
	# 3 contact right — mirror of 1
	[
		[6, 2, 4, 4, "c"],
		[7, 4, 2, 2, "s"],
		[7, 6, 2, 1, "c"],
		[4, 6, 8, 5, "c"],
		[7, 7, 2, 2, "a"],
		[3, 6, 2, 4, "c"],
		[3, 9, 2, 1, "s"],
		[11, 8, 2, 3, "c"],
		[11, 10, 2, 1, "s"],
		[6, 12, 2, 3, "c"],
		[10, 11, 2, 4, "c"]
	],
	# 4 eat — crouched low, head and reaching hand down at the food
	[
		[7, 6, 4, 4, "c"],
		[8, 8, 2, 2, "s"],
		[8, 10, 2, 1, "c"],
		[4, 8, 8, 5, "c"],
		[7, 9, 2, 2, "a"],
		[3, 9, 2, 3, "c"],
		[3, 11, 2, 1, "s"],
		[11, 10, 2, 3, "c"],
		[11, 12, 2, 1, "s"],
		[6, 13, 2, 2, "c"],
		[9, 13, 2, 2, "c"]
	],
	# 5 fight — lunging, arm raised high to strike, back leg braced
	[
		[6, 1, 4, 4, "c"],
		[7, 3, 2, 2, "s"],
		[7, 5, 2, 1, "c"],
		[4, 5, 8, 5, "c"],
		[7, 6, 2, 2, "a"],
		[10, 2, 2, 4, "c"],
		[10, 1, 2, 1, "s"],
		[3, 6, 2, 4, "c"],
		[3, 9, 2, 1, "s"],
		[3, 11, 3, 4, "c"],
		[8, 11, 2, 4, "c"],
		[11, 13, 3, 2, "c"]
	],
	# 6 trade — upright, one arm extended forward offering the good
	[
		[6, 2, 4, 4, "c"],
		[7, 4, 2, 2, "s"],
		[7, 6, 2, 1, "c"],
		[4, 6, 8, 5, "c"],
		[7, 7, 2, 2, "a"],
		[3, 7, 2, 4, "c"],
		[3, 10, 2, 1, "s"],
		[11, 6, 4, 2, "c"],
		[14, 6, 1, 1, "s"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"]
	],
]

# Zone colours per species, keyed into PAL — matched to the inspector avatars,
# lifted a step brighter than the true coats so figures stay readable over
# dark terrain.
const FIELD_ZONE_COLORS: Array = [
	{"c": "X", "s": "m", "a": "B"},  # Chimpanzee — charcoal coat, tan skin
	{"c": "d", "s": "B", "a": "G"},  # Gorilla — slate coat, silver chest
	{"c": "o", "s": "T", "a": "O"},  # Orangutan — rust coat, pale skin
	{"c": "B", "s": "t", "a": "r"},  # Australopith — brown coat, tan skin
	{"c": "m", "s": "t", "a": "h"},  # Sapiens — tawny clothes, tan skin
]


# Build one 16x16 cell from `blocks` ([x,y,w,h] white, or [x,y,w,h,key] via
# PAL), plus an auto 1px dark outline (every empty pixel touching the figure;
# collected first, then written, so outline pixels don't seed more outline).
static func _build_cell(blocks: Array) -> Image:
	var img := Image.create(16, 16, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	for b in blocks:
		var col := Color(1, 1, 1, 1) if b.size() < 5 else Color(PAL[b[4]])
		img.fill_rect(Rect2i(b[0], b[1], b[2], b[3]), col)
	var dirs := [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]
	var edges: Array = []
	for y in 16:
		for x in 16:
			if img.get_pixel(x, y).a > 0.0:
				continue
			for d in dirs:
				var nx: int = x + d.x
				var ny: int = y + d.y
				if nx >= 0 and nx < 16 and ny >= 0 and ny < 16 and img.get_pixel(nx, ny).a > 0.5:
					edges.append(Vector2i(x, y))
					break
	for e in edges:
		img.set_pixel(e.x, e.y, Color(0.34, 0.34, 0.34, 1.0))
	return img


# One species' walk cycle, zone colours applied.
static func _build_pose(pose: Array, zones: Dictionary) -> Image:
	var blocks: Array = []
	for b in pose:
		blocks.append([b[0], b[1], b[2], b[3], zones[b[4]]])
	return _build_cell(blocks)


# One species' poses packed VERTICALLY into one (16 x POSE_COUNT*16) strip:
# the 4 gait poses first, then the eat/fight/trade stills. One MultiMesh per
# species samples its own strip. The strip is 16px
# wide on purpose: wide-thin atlas textures sample garbage on the canvas
# MultiMesh path (Metal), while 16px rows match the field mask that has
# always rendered cleanly. Nearest-filtered; the transparent margins keep
# cells from bleeding. Cells are stored upside-down: the MultiMesh QuadMesh
# is a 3D mesh whose V axis renders flipped in the 2D canvas, so
# pre-flipping the art draws figures upright.
static func build_species_atlas(sp: int) -> ImageTexture:
	var atlas := Image.create(16, POSE_COUNT * 16, false, Image.FORMAT_RGBA8)
	atlas.fill(Color(0, 0, 0, 0))
	for fr in POSE_COUNT:
		var cell := _build_pose(FIELD_POSES[fr], FIELD_ZONE_COLORS[sp])
		cell.flip_y()
		atlas.blit_rect(cell, Rect2i(0, 0, 16, 16), Vector2i(0, fr * 16))
	return ImageTexture.create_from_image(atlas)


# Fallen figure for the death ghosts in main.gd: the neutral pose rotated 90°
# CW inside its 16x16 cell (rect [x,y,w,h] -> [15-(y+h), x, h, w]), zone-painted
# per species and pre-flipped like the walk atlas for the shared QuadMesh.
static func build_fallen_texture(sp: int) -> ImageTexture:
	var blocks: Array = []
	for b in FIELD_POSES[0]:
		blocks.append([15 - (b[1] + b[3]), b[0], b[3], b[2], b[4]])
	var cell := _build_pose(blocks, FIELD_ZONE_COLORS[sp])
	cell.flip_y()
	return ImageTexture.create_from_image(cell)


# Build the ImageTexture for ape `idx`, nearest-filtered when displayed so the
# 16x16 grid stays crisp when scaled up.
static func build(idx: int) -> ImageTexture:
	var img := Image.create(16, 16, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	for b in APES[idx]:
		img.fill_rect(Rect2i(b[0], b[1], b[2], b[3]), Color(PAL[b[4]]))
	return ImageTexture.create_from_image(img)


# Map an agent's species id to one of the five hominins (stable per species).
static func ape_for_species(species_id: int) -> int:
	return abs(species_id) % NAMES.size()
