extends RefCounted
# 2.5D layering helper (D9 of docs/superpowers/specs/2026-09-12-pixel-world-
# at-scale-design.md): a standing sprite (tree, hut) is cut at a row into an
# upper part drawn ABOVE the figures and a lower part drawn BELOW them. A
# figure north of the object (smaller y) is covered by the crown or roof; a
# figure south of it draws over the trunk or wall. Both halves ride the same
# MultiMesh transform, so the cut costs one extra draw and no per-frame work.
# The row is a fixed fraction of the sprite's opaque height — crowns and
# roofs take the upper ~60% of every tree and hut in the sets.

const SPLIT_FRAC := 0.6


# First opaque row, last opaque row (inclusive), or [-1, -1] when empty.
static func opaque_span(img: Image) -> PackedInt32Array:
	var first := -1
	var last := -1
	for y in img.get_height():
		for x in img.get_width():
			if img.get_pixel(x, y).a > 0.0:
				if first < 0:
					first = y
				last = y
				break
	return PackedInt32Array([first, last])


# The row where the upper part ends (exclusive): the top of the trunk/wall.
static func split_row(img: Image, frac: float = SPLIT_FRAC) -> int:
	var span := opaque_span(img)
	if span[0] < 0:
		return 0
	return span[0] + int(round(float(span[1] - span[0] + 1) * frac))


# Copy of `img` with every row at or below `row` cleared.
static func upper(img: Image, row: int) -> Image:
	var out := Image.create(img.get_width(), img.get_height(), false, Image.FORMAT_RGBA8)
	out.copy_from(img)
	for y in range(maxi(row, 0), img.get_height()):
		for x in img.get_width():
			out.set_pixel(x, y, Color(0, 0, 0, 0))
	return out


# Copy of `img` with every row above `row` cleared.
static func lower(img: Image, row: int) -> Image:
	var out := Image.create(img.get_width(), img.get_height(), false, Image.FORMAT_RGBA8)
	out.copy_from(img)
	for y in mini(row, img.get_height()):
		for x in img.get_width():
			out.set_pixel(x, y, Color(0, 0, 0, 0))
	return out
