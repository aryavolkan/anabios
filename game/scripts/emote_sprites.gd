extends RefCounted
# Emote pictograms: tiny pixel glyphs floated above acting agents so the
# field reads at a glance even when the pose itself is small on screen —
# Zzz over sleepers, a heart over a courting pair, a droplet over drinkers,
# an exclamation over fleers, a star over celebrants.
#
# The glyphs are authored in the same block format as the figure atlases
# ([x, y, w, h, zone] rects in a 16x16 cell) and packed through
# ApeSprites._pack_grid, so they inherit the square 128x128 grid layout (the
# Metal MultiMesh2D-safe shape), the 1px auto-outline pass, and the
# pre-flip for the shared QuadMesh's V axis. Cell 0 is intentionally empty
# (kind 0 = no emote); kinds 1..5 map to the cells below.

const ApeSprites = preload("res://scripts/ape_sprites.gd")

enum { NONE, SLEEP_Z, HEART, DROPLET, ALERT, STAR }
const KIND_COUNT := 6
# Divisor packing the kind id into the 0..1 custom-data channel, mirrored by
# the emote shader's kind_scale uniform (same pattern as main.gd ACT_SCALE).
const KIND_SCALE := 8.0

# Glyph colours, resolved through the zones dictionary like the quadruped
# atlas resolves its neutral ramp.
const ZONES := {
	"z": Color(0.85, 0.88, 1.0),  # sleepy lavender-white
	"h": Color(0.95, 0.30, 0.42),  # rose red
	"hl": Color(1.0, 0.72, 0.78),  # heart highlight
	"d": Color(0.45, 0.75, 0.95),  # water cyan
	"dl": Color(0.85, 0.95, 1.0),  # droplet glint
	"a": Color(1.0, 0.82, 0.25),  # alert amber
	"s": Color(1.0, 0.90, 0.35),  # star gold
	"sl": Color(1.0, 1.0, 0.85),  # star core
}

# Blocks are [x, y, w, h, zone], drawn back-to-front in a 16x16 cell.
const GLYPHS: Array = [
	# 0 NONE — empty cell so kind 0 samples pure transparency.
	[],
	# 1 SLEEP_Z — one big bold Z (2px strokes survive the on-field downscale;
	# finer multi-Z art dissolved into the auto-outline).
	[
		[4, 4, 8, 2, "z"],
		[9, 6, 2, 1, "z"],
		[8, 7, 2, 1, "z"],
		[6, 8, 2, 1, "z"],
		[5, 9, 2, 1, "z"],
		[4, 10, 8, 2, "z"],
	],
	# 2 HEART — classic two-lobe heart with a light catch on one lobe.
	[
		[5, 4, 2, 1, "h"],
		[9, 4, 2, 1, "h"],
		[4, 5, 4, 1, "h"],
		[8, 5, 4, 1, "h"],
		[4, 6, 8, 2, "h"],
		[5, 8, 6, 1, "h"],
		[6, 9, 4, 1, "h"],
		[7, 10, 2, 1, "h"],
		[5, 5, 1, 1, "hl"],
	],
	# 3 DROPLET — teardrop narrowing to a point at the top, glint low-left.
	[
		[7, 3, 1, 1, "d"],
		[7, 4, 1, 1, "d"],
		[6, 5, 3, 1, "d"],
		[5, 6, 5, 3, "d"],
		[6, 9, 3, 1, "d"],
		[6, 7, 1, 1, "dl"],
	],
	# 4 ALERT — exclamation mark.
	[
		[7, 3, 2, 6, "a"],
		[7, 11, 2, 2, "a"],
	],
	# 5 STAR — four-point diamond sparkle with a bright core.
	[
		[7, 3, 2, 1, "s"],
		[6, 4, 4, 1, "s"],
		[5, 5, 6, 1, "s"],
		[4, 6, 8, 2, "s"],
		[5, 8, 6, 1, "s"],
		[6, 9, 4, 1, "s"],
		[7, 10, 2, 1, "s"],
		[7, 6, 2, 2, "sl"],
	],
]


static func build_atlas() -> ImageTexture:
	return ApeSprites._pack_grid(GLYPHS, ZONES)
