extends RefCounted
# 16x16 pixel-art marks for the viewer's event bursts, built with the shared
# ApeSprites cell painter (auto 1px outline, PAL palette) so a burst reads in
# the same hand as the creatures and buildings it lands among. Each mark is a
# small outlined shape on a transparent cell — the pooled GPUParticles2D in
# viewer_effects scatter a handful of them per event and tint them via
# modulate. Not flipped: particles draw the texture upright (the flip_y() in
# the building/prop registries is the MultiMesh QuadMesh's V convention).

const ApeSprites = preload("res://scripts/ape_sprites.gd")

enum { EMBER, IMPACT, DISCOVERY, SMOKE, FLAME, BONES }
const KIND_COUNT := 6

# [x, y, w, h, key] blocks on a 16x16 grid, drawn back-to-front.
const _BLOCKS: Array = [
	# EMBER — a squat diamond with a hot amber core: the spark of a fire event
	[
		[7, 5, 2, 6, "o"],
		[6, 6, 4, 4, "o"],
		[5, 7, 6, 2, "o"],
		[7, 6, 2, 4, "O"],
		[7, 7, 2, 2, "y"],
	],
	# IMPACT — four-direction star, white-hot along the arms, amber tips and
	# four diagonal pips: the strike of a raid or a war
	[
		[7, 2, 2, 12, "y"],
		[2, 7, 12, 2, "y"],
		[6, 6, 4, 4, "y"],
		[7, 4, 2, 8, "W"],
		[4, 7, 8, 2, "W"],
		[7, 7, 2, 2, "W"],
		[5, 5, 1, 1, "y"],
		[10, 5, 1, 1, "y"],
		[5, 10, 1, 1, "y"],
		[10, 10, 1, 1, "y"],
	],
	# DISCOVERY — an eight-point sparkle: bright cross arms, a hot core and
	# short diagonal glints — the flash of a breakthrough, thinner than the
	# impact star so the two never read alike at field scale
	[
		[7, 3, 2, 10, "W"],
		[3, 7, 10, 2, "W"],
		[6, 6, 4, 4, "W"],
		[7, 7, 2, 2, "e"],
		[4, 4, 1, 1, "y"],
		[3, 3, 1, 1, "s"],
		[11, 4, 1, 1, "y"],
		[12, 3, 1, 1, "s"],
		[4, 11, 1, 1, "y"],
		[3, 12, 1, 1, "s"],
		[11, 11, 1, 1, "y"],
		[12, 12, 1, 1, "s"],
	],
	# SMOKE — a lumpy three-lobed puff in two greys with a lit top edge: the
	# boards' chimney smoke is clumps of pixels, not a soft haze, so the
	# settlement plume scatters these hard-edged puffs instead of a radial
	# disc that blurred into a grey fog over the hearth
	[
		[3, 6, 10, 6, "g"],
		[5, 4, 6, 4, "g"],
		[2, 8, 3, 3, "g"],
		[11, 7, 3, 3, "g"],
		[6, 5, 4, 3, "G"],
		[4, 7, 7, 3, "G"],
		[6, 4, 3, 1, "s"],
		[3, 8, 2, 1, "s"],
	],
	# FLAME — a tongue of fire: orange body tapering to a tip, a yellow core
	# and a white-hot heart, for the pooled fires over a burning ruin
	[
		[7, 2, 2, 2, "O"],
		[6, 4, 4, 3, "O"],
		[5, 7, 6, 4, "O"],
		[4, 9, 8, 3, "o"],
		[7, 5, 2, 2, "y"],
		[6, 7, 4, 4, "y"],
		[7, 9, 2, 2, "W"],
	],
	# BONES — what a carcass leaves on the ground: a bleached skull at one
	# end of a spine with three rib pairs, lying flat
	[
		[3, 7, 10, 2, "w"],
		[6, 4, 1, 8, "w"],
		[8, 4, 1, 8, "w"],
		[10, 5, 1, 6, "w"],
		[11, 6, 4, 4, "w"],
		[12, 7, 1, 1, "K"],
		[14, 7, 1, 1, "K"],
		[13, 9, 1, 1, "K"],
		[3, 8, 10, 1, "s"],
	],
]

# One texture per kind, built on first use: a burst is retargeted many times
# a minute and must not upload a fresh texture per spawn.
static var _cache: Dictionary = {}


static func build_image(kind: int) -> Image:
	return ApeSprites._build_cell(_BLOCKS[kind])


static func build(kind: int) -> ImageTexture:
	if not _cache.has(kind):
		_cache[kind] = ImageTexture.create_from_image(build_image(kind))
	return _cache[kind]


static func opaque_pixels(image: Image) -> int:
	var n := 0
	for y in image.get_height():
		for x in image.get_width():
			if image.get_pixel(x, y).a > 0.5:
				n += 1
	return n
