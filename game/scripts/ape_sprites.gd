extends RefCounted

# Procedural 16x16 "ape" avatars for the DIT agents. Built the same way as the
# body disc in main.gd (an Image filled at load), so no PNG asset ships — the
# sprite data lives here as color-block lists. The agents ARE the apes: the
# inspector shows a pinned agent's lineage as one of these five hominins,
# selected from its species id.

const NAMES: PackedStringArray = [
	"Chimpanzee", "Gorilla", "Orangutan", "Australopith", "Sapiens",
]

# Shared palette (hex, no alpha => opaque). Keys match the block lists below.
const PAL := {
	"K": "14100f", "x": "241f26", "X": "3a3340",
	"b": "4a2f1e", "B": "7a4c2c", "r": "a06a3c", "t": "c99a63", "T": "e0c090",
	"w": "e7ded0", "W": "f6f2e6",
	"g": "5b5b66", "G": "7c7c88", "d": "3a3a44", "s": "b9b9c4",
	"p": "cf8f8f", "P": "e6b3b3", "n": "b5645f",
	"o": "c06a2e", "O": "d98f4a", "R": "c23a34", "y": "d9a83a",
	"h": "e8e2d0", "m": "b98a5a", "e": "f4f4f4", "k": "0e0d10",
}

# Each ape is a list of [x, y, w, h, color_key] blocks on a 16x16 grid, drawn
# back-to-front (later blocks overwrite earlier ones), facing right.
const APES: Array = [
	# 0 Chimpanzee — dark coat, round ears, pale muzzle, lit chest/forearms
	[[6, 12, 2, 3, "x"], [9, 12, 2, 3, "x"], [5, 7, 6, 6, "x"], [9, 5, 4, 4, "x"],
	[9, 6, 4, 1, "X"], [8, 6, 1, 2, "X"], [13, 6, 1, 2, "X"], [10, 8, 3, 2, "m"],
	[11, 8, 2, 1, "B"], [11, 7, 1, 1, "e"], [11, 7, 1, 1, "k"], [4, 7, 2, 6, "x"],
	[11, 7, 2, 6, "x"], [7, 9, 2, 3, "X"], [4, 10, 2, 3, "X"], [11, 10, 2, 3, "X"],
	[6, 12, 2, 3, "x"], [9, 12, 2, 3, "x"]],
	# 1 Gorilla — broad shoulders, silverback, knuckle stance, lit chest/arms
	[[6, 12, 3, 3, "x"], [10, 12, 3, 3, "x"], [4, 6, 8, 7, "x"], [5, 4, 5, 3, "x"],
	[4, 7, 5, 2, "X"], [9, 4, 4, 4, "x"], [9, 5, 4, 1, "X"], [10, 7, 3, 2, "B"],
	[11, 7, 1, 1, "e"], [11, 7, 1, 1, "k"], [2, 7, 3, 6, "x"], [2, 12, 3, 2, "x"],
	[12, 7, 2, 5, "x"], [6, 9, 3, 3, "X"], [2, 10, 2, 3, "X"], [12, 10, 2, 3, "X"],
	[6, 12, 3, 3, "x"], [10, 12, 3, 3, "x"]],
	# 2 Orangutan — rust coat, shaggy fringe, long arms
	[[7, 12, 2, 3, "o"], [9, 12, 2, 3, "o"], [5, 6, 6, 6, "o"], [4, 7, 1, 6, "O"],
	[11, 7, 1, 6, "O"], [9, 3, 4, 2, "B"], [9, 4, 4, 4, "o"], [10, 6, 3, 2, "T"],
	[11, 6, 1, 1, "k"], [3, 6, 2, 8, "o"], [12, 6, 2, 7, "o"], [7, 12, 2, 3, "o"],
	[9, 12, 2, 3, "o"]],
	# 3 Australopith — small, hairy, upright biped
	[[8, 2, 3, 1, "x"], [8, 3, 3, 3, "B"], [9, 4, 2, 1, "r"], [9, 4, 1, 1, "k"],
	[7, 6, 4, 5, "B"], [7, 6, 1, 5, "x"], [6, 6, 1, 5, "B"], [11, 6, 1, 4, "B"],
	[7, 11, 1, 4, "B"], [9, 11, 1, 4, "B"], [8, 14, 3, 1, "K"]],
	# 4 Sapiens — upright toolmaker, spear in hand (the culture marker)
	[[5, 1, 1, 11, "b"], [5, 1, 1, 1, "s"], [8, 2, 3, 1, "b"], [8, 3, 3, 2, "t"],
	[10, 3, 1, 1, "k"], [7, 5, 4, 5, "B"], [5, 5, 2, 5, "t"], [11, 5, 1, 4, "t"],
	[7, 10, 1, 4, "t"], [9, 10, 2, 4, "t"], [7, 14, 4, 1, "K"]],
]

# A bold upright-hominin silhouette for the FIELD: the per-agent body mark that
# replaces the plain disc. Pure white with an auto 1px dark outline so, once
# multiplied by each agent's genome colour (exactly like the old disc was), it
# reads as a little tinted ape with a crisp edge at any zoom.
const FIELD_SHAPE: Array = [
	[6, 2, 4, 4],   # head
	[7, 6, 2, 1],   # neck
	[4, 6, 8, 5],   # shoulders / torso
	[3, 7, 2, 4],   # left arm
	[11, 7, 2, 4],  # right arm
	[6, 11, 2, 4],  # left leg
	[9, 11, 2, 4],  # right leg
]

# Four-frame walk cycle: neutral, left-lead, neutral, right-lead. Head / neck /
# torso are fixed; the legs stride and the arms counter-swing so a moving agent
# reads as walking. Frame 0 is the neutral/idle pose (== FIELD_SHAPE).
const WALK_FRAME_COUNT := 4
const WALK_FRAMES: Array = [
	# 0 neutral
	[[6, 2, 4, 4], [7, 6, 2, 1], [4, 6, 8, 5], [3, 7, 2, 4], [11, 7, 2, 4], [6, 11, 2, 4], [9, 11, 2, 4]],
	# 1 left lead — left leg out & forward, right trails; right arm up, left arm back
	[[6, 2, 4, 4], [7, 6, 2, 1], [4, 6, 8, 5], [3, 8, 2, 3], [11, 6, 2, 4], [4, 11, 2, 4], [9, 12, 2, 3]],
	# 2 neutral
	[[6, 2, 4, 4], [7, 6, 2, 1], [4, 6, 8, 5], [3, 7, 2, 4], [11, 7, 2, 4], [6, 11, 2, 4], [9, 11, 2, 4]],
	# 3 right lead — mirror of 1
	[[6, 2, 4, 4], [7, 6, 2, 1], [4, 6, 8, 5], [3, 6, 2, 4], [11, 8, 2, 3], [6, 12, 2, 3], [10, 11, 2, 4]],
]

# Build one 16x16 cell: a white silhouette from `blocks` + an auto 1px dark
# outline (every empty pixel touching the silhouette; collected first, then
# written, so outline pixels don't seed more outline).
static func _build_cell(blocks: Array) -> Image:
	var img := Image.create(16, 16, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	for b in blocks:
		img.fill_rect(Rect2i(b[0], b[1], b[2], b[3]), Color(1, 1, 1, 1))
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

# A single static hominin silhouette (the neutral frame), 16x16.
static func build_field_mask() -> ImageTexture:
	return ImageTexture.create_from_image(_build_cell(FIELD_SHAPE))

# The walk cycle packed horizontally into one (WALK_FRAME_COUNT*16 x 16) atlas
# for the field-agent shader. Nearest-filtered; each cell keeps its own outline,
# and the transparent left/right margins keep neighbouring cells from bleeding.
static func build_walk_atlas() -> ImageTexture:
	var atlas := Image.create(WALK_FRAME_COUNT * 16, 16, false, Image.FORMAT_RGBA8)
	atlas.fill(Color(0, 0, 0, 0))
	for fr in WALK_FRAME_COUNT:
		atlas.blit_rect(_build_cell(WALK_FRAMES[fr]), Rect2i(0, 0, 16, 16), Vector2i(fr * 16, 0))
	return ImageTexture.create_from_image(atlas)

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
