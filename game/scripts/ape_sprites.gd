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
# Per-hominin gait cadence (frames/sec), read by mammal_sprites.bucket_gait_fps.
const WALK_FPS: PackedFloat32Array = [5.0, 4.2, 4.6, 4.8, 5.6]
# The atlas stacks WALK_FRAME_COUNT gait poses, then TWO frames per action
# (eat / fight / trade / flee / sleep / celebrate / spear / bow / steel) that
# main.gd derives from combat, trade, fire-intent, mood, energy and invention
# signals; the shader cycles each pair when INSTANCE_CUSTOM.a != 0.
const POSE_COUNT := 22
const POSE_EAT := 4
const POSE_FIGHT := 6
const POSE_TRADE := 8
const POSE_FLEE := 10
const POSE_SLEEP := 12
const POSE_SPEAR := 16
const POSE_BOW := 18
const POSE_STEEL := 20
# Atlas layout: the 22 poses fill a SQUARE 128x128 grid (8 cols x 8 rows),
# NOT a 16x192 vertical strip. On Metal (Apple GPUs) an
# extreme-aspect texture sampled through a canvas_item ShaderMaterial on the
# MultiMesh2D path corrupts into torn horizontal streaks — a square
# power-of-two texture renders cleanly. The field_agent shader maps a pose
# index `fr` to cell (fr % ATLAS_COLS, fr / ATLAS_COLS). Keep in sync with the
# shaders' ATLAS_COLS / ATLAS_PX / CELL_PX constants (field_agent AND emote —
# the emote glyphs pack through the same grid) and mammal_sprites.
const ATLAS_COLS := 8
const ATLAS_PX := 128
const CELL_PX := 16
# Hero atlas (D1's 24 px figures, adopted): the authored 16 px rigs are
# scaled 1.5x onto 24 px cells, shaded (lit crown, dark belly) and given
# per-archetype accents (mammal_sprites.ACCENTS, anchored to each pose's
# head block), then packed ATLAS_COLS per row into a square 192x192 grid.
# field_agent.gdshader reads the cell/atlas sizes as uniforms.
const HERO_PX := 24
const HERO_ATLAS_PX := 192
# The authored figure occupies the bottom HERO_FIGURE_PX rows of the hero
# cell; the rows above are headroom for accents (antlers, ears).
const HERO_FIGURE_PX := 20
const HERO_HEADROOM := HERO_PX - HERO_FIGURE_PX
const SHADE_LIT := 1.22
const SHADE_DARK := 0.72
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
	# 5 eat B — the chomp: head dips a pixel, hand comes up to the mouth
	[
		[7, 7, 4, 4, "c"],
		[8, 9, 2, 2, "s"],
		[8, 11, 2, 1, "c"],
		[4, 8, 8, 5, "c"],
		[7, 9, 2, 2, "a"],
		[3, 9, 2, 2, "c"],
		[4, 8, 2, 1, "s"],
		[11, 10, 2, 3, "c"],
		[11, 12, 2, 1, "s"],
		[6, 13, 2, 2, "c"],
		[9, 13, 2, 2, "c"]
	],
	# 6 fight — lunging, arm raised high to strike, back leg braced
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
	# 7 fight B — the strike lands: raised arm swings forward at shoulder height
	[
		[6, 1, 4, 4, "c"],
		[7, 3, 2, 2, "s"],
		[7, 5, 2, 1, "c"],
		[4, 5, 8, 5, "c"],
		[7, 6, 2, 2, "a"],
		[11, 5, 4, 2, "c"],
		[14, 4, 1, 1, "s"],
		[3, 6, 2, 4, "c"],
		[3, 9, 2, 1, "s"],
		[3, 11, 3, 4, "c"],
		[8, 11, 2, 4, "c"],
		[11, 13, 3, 2, "c"]
	],
	# 8 trade — upright, one arm extended forward offering the good
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
	# 9 trade B — the offering arm dips a pixel (a small beckoning bob)
	[
		[6, 2, 4, 4, "c"],
		[7, 4, 2, 2, "s"],
		[7, 6, 2, 1, "c"],
		[4, 6, 8, 5, "c"],
		[7, 7, 2, 2, "a"],
		[3, 7, 2, 4, "c"],
		[3, 10, 2, 1, "s"],
		[11, 7, 4, 2, "c"],
		[14, 7, 1, 1, "s"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"]
	],
	# 10 flee — leaning into a panicked run, both arms flung up
	[
		[8, 2, 4, 4, "c"],
		[9, 4, 2, 2, "s"],
		[8, 6, 2, 1, "c"],
		[5, 6, 8, 5, "c"],
		[8, 7, 2, 2, "a"],
		[3, 4, 2, 4, "c"],
		[3, 3, 2, 1, "s"],
		[12, 5, 2, 4, "c"],
		[12, 4, 2, 1, "s"],
		[4, 11, 3, 4, "c"],
		[9, 12, 3, 3, "c"]
	],
	# 11 flee B — the counter-stride: arms swap, legs scissor the other way
	[
		[8, 2, 4, 4, "c"],
		[9, 4, 2, 2, "s"],
		[8, 6, 2, 1, "c"],
		[5, 6, 8, 5, "c"],
		[8, 7, 2, 2, "a"],
		[3, 6, 2, 3, "c"],
		[3, 5, 2, 1, "s"],
		[12, 4, 2, 4, "c"],
		[12, 3, 2, 1, "s"],
		[5, 12, 3, 3, "c"],
		[9, 11, 3, 4, "c"]
	],
	# 12 sleep — curled up on the ground: torso horizontal along the ground
	# line, head resting at the facing (right) end, arm folded under the
	# cheek, legs drawn up. The shader swaps the idle bob for a slow breath.
	[
		[11, 9, 4, 4, "c"],
		[12, 11, 2, 2, "s"],
		[3, 11, 9, 4, "c"],
		[8, 12, 2, 1, "a"],
		[9, 13, 3, 1, "c"],
		[11, 14, 2, 1, "s"],
		[3, 13, 4, 2, "c"]
	],
	# 13 sleep B — the inhale: the back swells one pixel
	[
		[11, 9, 4, 4, "c"],
		[12, 11, 2, 2, "s"],
		[3, 10, 9, 5, "c"],
		[8, 11, 2, 1, "a"],
		[9, 13, 3, 1, "c"],
		[11, 14, 2, 1, "s"],
		[3, 13, 4, 2, "c"]
	],
	# 14 celebrate — arms raised after a successful mating bond
	[
		[6, 2, 4, 4, "c"],
		[7, 4, 2, 2, "s"],
		[7, 6, 2, 1, "c"],
		[4, 6, 8, 5, "c"],
		[7, 7, 2, 2, "a"],
		[2, 3, 2, 5, "c"],
		[2, 2, 2, 1, "s"],
		[12, 3, 2, 5, "c"],
		[12, 2, 2, 1, "s"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"]
	],
	# 15 celebrate B — a small upward bounce with the hands still aloft
	[
		[6, 1, 4, 4, "c"],
		[7, 3, 2, 2, "s"],
		[7, 5, 2, 1, "c"],
		[4, 5, 8, 5, "c"],
		[7, 6, 2, 2, "a"],
		[2, 2, 2, 5, "c"],
		[2, 1, 2, 1, "s"],
		[12, 2, 2, 5, "c"],
		[12, 1, 2, 1, "s"],
		[6, 10, 2, 4, "c"],
		[9, 10, 2, 4, "c"]
	],
	# 16 spear thrust — poised: shaft level at the shoulder, weight coiled on
	# the braced back leg (the fight lunge stance, armed). Zones "w"/"f" are
	# the hafted weapon's wood and knapped stone.
	[
		[6, 1, 4, 4, "c"],
		[7, 3, 2, 2, "s"],
		[7, 5, 2, 1, "c"],
		[4, 5, 8, 5, "c"],
		[7, 6, 2, 2, "a"],
		[3, 6, 2, 4, "c"],
		[3, 9, 2, 1, "s"],
		[3, 11, 3, 4, "c"],
		[8, 11, 2, 4, "c"],
		[11, 13, 3, 2, "c"],
		[1, 4, 11, 1, "w"],
		[12, 4, 2, 1, "f"],
		[9, 3, 2, 2, "s"]
	],
	# 17 spear thrust B — the thrust lands: body leans in, shaft driven a
	# stride forward with the point at the leading edge
	[
		[7, 1, 4, 4, "c"],
		[8, 3, 2, 2, "s"],
		[8, 5, 2, 1, "c"],
		[5, 5, 8, 5, "c"],
		[8, 6, 2, 2, "a"],
		[3, 7, 2, 3, "c"],
		[3, 10, 2, 1, "s"],
		[3, 11, 3, 4, "c"],
		[9, 11, 2, 4, "c"],
		[12, 13, 3, 2, "c"],
		[3, 4, 11, 1, "w"],
		[14, 4, 2, 1, "f"],
		[10, 3, 2, 2, "s"]
	],
	# 18 bow draw — upright archer: stave held out front, string and nocked
	# arrow pulled back to the cheek
	[
		[5, 2, 4, 4, "c"],
		[6, 4, 2, 2, "s"],
		[6, 6, 2, 1, "c"],
		[3, 6, 7, 5, "c"],
		[6, 7, 2, 2, "a"],
		[4, 11, 2, 4, "c"],
		[8, 11, 2, 4, "c"],
		[10, 6, 3, 1, "c"],
		[12, 2, 1, 4, "w"],
		[13, 5, 1, 3, "w"],
		[12, 8, 1, 4, "w"],
		[10, 3, 1, 3, "f"],
		[10, 7, 1, 5, "f"],
		[10, 6, 4, 1, "w"],
		[14, 6, 1, 1, "f"],
		[9, 6, 1, 1, "s"]
	],
	# 19 bow loose — the string snaps home against the stave, the arrow is
	# away and the draw hand flings open behind
	[
		[5, 2, 4, 4, "c"],
		[6, 4, 2, 2, "s"],
		[6, 6, 2, 1, "c"],
		[3, 6, 7, 5, "c"],
		[6, 7, 2, 2, "a"],
		[4, 11, 2, 4, "c"],
		[8, 11, 2, 4, "c"],
		[10, 6, 3, 1, "c"],
		[12, 2, 1, 4, "w"],
		[13, 5, 1, 3, "w"],
		[12, 8, 1, 4, "w"],
		[11, 3, 1, 8, "f"],
		[8, 5, 2, 1, "s"]
	],
	# 20 steel swing — era-3 blade raised high over the braced lunge stance
	# ("f" doubles as polished steel, "w" the wrapped hilt)
	[
		[6, 2, 4, 4, "c"],
		[7, 4, 2, 2, "s"],
		[7, 6, 2, 1, "c"],
		[4, 6, 8, 5, "c"],
		[7, 7, 2, 2, "a"],
		[3, 7, 2, 4, "c"],
		[3, 10, 2, 1, "s"],
		[3, 11, 3, 4, "c"],
		[8, 11, 2, 4, "c"],
		[11, 13, 3, 2, "c"],
		[10, 3, 2, 3, "c"],
		[10, 2, 2, 1, "s"],
		[11, 1, 1, 1, "w"],
		[11, 0, 4, 1, "f"],
		[14, 1, 1, 1, "f"]
	],
	# 21 steel swing B — the blade sweeps level through the strike
	[
		[6, 1, 4, 4, "c"],
		[7, 3, 2, 2, "s"],
		[7, 5, 2, 1, "c"],
		[4, 5, 8, 5, "c"],
		[7, 6, 2, 2, "a"],
		[3, 6, 2, 4, "c"],
		[3, 9, 2, 1, "s"],
		[3, 11, 3, 4, "c"],
		[8, 11, 2, 4, "c"],
		[11, 13, 3, 2, "c"],
		[11, 5, 2, 2, "c"],
		[12, 4, 1, 1, "s"],
		[13, 4, 1, 1, "w"],
		[13, 3, 3, 1, "f"],
		[15, 4, 1, 1, "f"]
	],
]

# Zone colours per species, keyed into PAL — matched to the inspector avatars,
# lifted a step brighter than the true coats so figures stay readable over
# dark terrain. "w"/"f" are the weapon cells' wood shaft and knapped stone,
# the same pair the Sapiens avatar's spear uses, shared by every hominin.
const FIELD_ZONE_COLORS: Array = [
	{"c": "X", "s": "m", "a": "B", "w": "b", "f": "s"},  # Chimpanzee — charcoal coat
	{"c": "d", "s": "B", "a": "G", "w": "b", "f": "s"},  # Gorilla — slate coat
	{"c": "o", "s": "T", "a": "O", "w": "b", "f": "s"},  # Orangutan — rust coat
	{"c": "B", "s": "t", "a": "r", "w": "b", "f": "s"},  # Australopith — brown coat
	{"c": "m", "s": "t", "a": "h", "w": "b", "f": "s"},  # Sapiens — tawny clothes
]


# Rescale rect blocks ([x, y, w, h, ...]) from one cell size to another,
# rounding each edge so adjacent blocks stay adjacent and no block vanishes.
static func scale_blocks(blocks: Array, from_px: int, to_px: int) -> Array:
	var f := float(to_px) / float(from_px)
	var out: Array = []
	for b in blocks:
		var x0 := int(round(float(b[0]) * f))
		var y0 := int(round(float(b[1]) * f))
		var x1 := int(round(float(b[0] + b[2]) * f))
		var y1 := int(round(float(b[1] + b[3]) * f))
		var nb: Array = [x0, y0, maxi(1, x1 - x0), maxi(1, y1 - y0)]
		for extra in b.slice(4):
			nb.append(extra)
		out.append(nb)
	return out


# The head of a right-facing pose: its topmost block, the rightmost on ties.
static func head_block(blocks: Array) -> Rect2i:
	var best := Rect2i(0, 0, 0, 0)
	var found := false
	for b in blocks:
		var r := Rect2i(b[0], b[1], b[2], b[3])
		var higher: bool = r.position.y < best.position.y
		var righter: bool = r.position.y == best.position.y and r.end.x > best.end.x
		if not found or higher or righter:
			best = r
			found = true
	return best


# Lit crown / dark belly: an opaque pixel with nothing above it brightens,
# one with nothing below it darkens, so a flat block figure reads as a
# rounded body. Applied before the outline pass.
static func shade(img: Image, px: int) -> void:
	var lit: Array = []
	var dark: Array = []
	for y in px:
		for x in px:
			if img.get_pixel(x, y).a <= 0.5:
				continue
			if y == 0 or img.get_pixel(x, y - 1).a <= 0.5:
				lit.append(Vector2i(x, y))
			elif y == px - 1 or img.get_pixel(x, y + 1).a <= 0.5:
				dark.append(Vector2i(x, y))
	for p in lit:
		var c := img.get_pixel(p.x, p.y)
		img.set_pixel(
			p.x,
			p.y,
			Color(
				minf(c.r * SHADE_LIT, 1.0),
				minf(c.g * SHADE_LIT, 1.0),
				minf(c.b * SHADE_LIT, 1.0),
				c.a
			)
		)
	for p in dark:
		var c := img.get_pixel(p.x, p.y)
		img.set_pixel(p.x, p.y, Color(c.r * SHADE_DARK, c.g * SHADE_DARK, c.b * SHADE_DARK, c.a))


# Build one px×px cell from `blocks` ([x,y,w,h] white, or [x,y,w,h,key] via
# PAL, or [x,y,w,h,Color]), optionally shaded, plus an auto 1px dark outline
# (every empty pixel touching the figure; collected first, then written, so
# outline pixels don't seed more outline).
static func _build_cell(blocks: Array, px: int = CELL_PX, hero: bool = false) -> Image:
	var img := Image.create(px, px, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	for b in blocks:
		var col: Color = (
			b[4]
			if (b.size() >= 5 and b[4] is Color)
			else (Color(1, 1, 1, 1) if b.size() < 5 else Color(PAL[b[4]]))
		)
		img.fill_rect(Rect2i(b[0], b[1], b[2], b[3]), col)
	if hero:
		shade(img, px)
	outline(img, px)
	return img


# The 1px dark outline: every empty pixel touching the figure (collected
# first, then written, so outline pixels don't seed more outline).
static func outline(img: Image, px: int) -> void:
	var dirs := [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]
	var edges: Array = []
	for y in px:
		for x in px:
			if img.get_pixel(x, y).a > 0.0:
				continue
			for d in dirs:
				var nx: int = x + d.x
				var ny: int = y + d.y
				if nx >= 0 and nx < px and ny >= 0 and ny < px and img.get_pixel(nx, ny).a > 0.5:
					edges.append(Vector2i(x, y))
					break
	for e in edges:
		img.set_pixel(e.x, e.y, Color(0.34, 0.34, 0.34, 1.0))


# Pre-built hero cells (hero_rigs.gd) packed into the square hero grid, each
# flipped for the QuadMesh's V axis like _pack_grid does.
static func pack_cells(cells: Array) -> ImageTexture:
	var atlas := Image.create(HERO_ATLAS_PX, HERO_ATLAS_PX, false, Image.FORMAT_RGBA8)
	atlas.fill(Color(0, 0, 0, 0))
	for fr in cells.size():
		var cell := Image.create(HERO_PX, HERO_PX, false, Image.FORMAT_RGBA8)
		cell.copy_from(cells[fr])
		cell.flip_y()
		var cx := (fr % ATLAS_COLS) * HERO_PX
		var cy := int(fr / float(ATLAS_COLS)) * HERO_PX
		atlas.blit_rect(cell, Rect2i(0, 0, HERO_PX, HERO_PX), Vector2i(cx, cy))
	return ImageTexture.create_from_image(atlas)


# One pose, zone colours applied. `hero` scales the 16 px blocks onto the
# 24 px cell, shades them and adds `accents` ([dx, dy, w, h, zone], placed
# from the head block's top-right corner) — antlers, tusks, ears.
static func _build_pose(
	pose: Array, zones: Dictionary, hero: bool = false, accents: Array = []
) -> Image:
	var src: Array = pose
	if hero:
		src = []
		for b in scale_blocks(pose, CELL_PX, HERO_FIGURE_PX):
			var nb: Array = b.duplicate()
			nb[1] += HERO_HEADROOM
			src.append(nb)
	var blocks: Array = []
	for b in src:
		blocks.append([b[0], b[1], b[2], b[3], zones[b[4]]])
	if hero and not accents.is_empty():
		var head := head_block(src)
		for a in accents:
			blocks.append([head.end.x + a[0], head.position.y + a[1], a[2], a[3], zones[a[4]]])
	return _build_cell(blocks, HERO_PX if hero else CELL_PX, hero)


# One species' poses packed into a SQUARE 64x64 grid (ATLAS_COLS per row):
# the 4 gait poses, then the eat/fight/trade/flee stills. One MultiMesh per
# species samples its own grid. A near-square power-of-two atlas is deliberate:
# an extreme-aspect (16x192) texture sampled through the field_agent
# ShaderMaterial on the MultiMesh2D canvas path corrupts into torn streaks on
# Metal; the 64x64 grid renders cleanly. Nearest-filtered; the transparent
# margins keep cells from bleeding. Cells are stored upside-down: the MultiMesh
# QuadMesh is a 3D mesh whose V axis renders flipped in the 2D canvas, so
# pre-flipping the art draws figures upright. Cell(fr) = (fr % ATLAS_COLS,
# fr / ATLAS_COLS) — kept in sync with field_agent.gdshader.
static func build_species_atlas(sp: int) -> ImageTexture:
	return _pack_grid(FIELD_POSES, FIELD_ZONE_COLORS[sp], true)


# Shared grid packer: paint each pose to a 16x16 cell (flipped for the QuadMesh
# V axis) and blit it into the 64x64 grid at its (col, row). Used by the ape
# atlas here and the quadruped atlas in mammal_sprites (which passes explicit
# Colours already resolved, so `zones` maps zone-key -> Color).
static func _pack_grid(
	poses: Array, zones: Dictionary, hero: bool = false, accents: Array = []
) -> ImageTexture:
	var px: int = HERO_PX if hero else CELL_PX
	var apx: int = HERO_ATLAS_PX if hero else ATLAS_PX
	var atlas := Image.create(apx, apx, false, Image.FORMAT_RGBA8)
	atlas.fill(Color(0, 0, 0, 0))
	for fr in poses.size():
		var cell := _build_pose(poses[fr], zones, hero, accents)
		cell.flip_y()
		var cx := (fr % ATLAS_COLS) * px
		var cy := int(fr / float(ATLAS_COLS)) * px
		atlas.blit_rect(cell, Rect2i(0, 0, px, px), Vector2i(cx, cy))
	return ImageTexture.create_from_image(atlas)


# Fallen figure for the death ghosts in main.gd: the neutral pose rotated 90°
# CW inside its 16x16 cell (rect [x,y,w,h] -> [15-(y+h), x, h, w]), zone-painted
# per species and pre-flipped like the walk atlas for the shared QuadMesh.
static func build_fallen_texture(sp: int) -> ImageTexture:
	var blocks: Array = []
	for b in FIELD_POSES[0]:
		blocks.append([15 - (b[1] + b[3]), b[0], b[3], b[2], b[4]])
	var cell := _build_pose(blocks, FIELD_ZONE_COLORS[sp], true)
	cell.flip_y()
	return ImageTexture.create_from_image(cell)


# Build the ImageTexture for ape `idx`, nearest-filtered when displayed so the
# 16x16 grid stays crisp when scaled up.
static func build(idx: int) -> ImageTexture:
	var blocks: Array = []
	for b in scale_blocks(APES[idx], CELL_PX, HERO_FIGURE_PX):
		blocks.append([b[0], b[1] + HERO_HEADROOM, b[2], b[3], Color(PAL[b[4]])])
	return ImageTexture.create_from_image(_build_cell(blocks, HERO_PX, true))


# Map an agent's species id to one of the five hominins (stable per species).
static func ape_for_species(species_id: int) -> int:
	return abs(species_id) % NAMES.size()
