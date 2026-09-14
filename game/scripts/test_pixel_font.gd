extends SceneTree
# Headless check for the 5x7 pixel HUD font. Run with:
#   godot --headless --rendering-driver dummy --path game -s res://scripts/test_pixel_font.gd

const PixelFont = preload("res://scripts/pixel_font.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	_check(PixelFont.GLYPHS.size() == 95, "every printable ASCII code point has a glyph")
	for code in range(32, 127):
		var ch := char(code)
		_check(PixelFont.GLYPHS.has(ch), "glyph for code %d" % code)
		if not PixelFont.GLYPHS.has(ch):
			continue
		var rows: Array = PixelFont.GLYPHS[ch]
		_check(rows.size() == PixelFont.CELL_H, "'%s' has %d rows" % [ch, PixelFont.CELL_H])
		for r in rows:
			_check(
				(r as String).length() == PixelFont.GLYPH_W,
				"'%s' rows are %d wide" % [ch, PixelFont.GLYPH_W]
			)
		var ink := 0
		for r in rows:
			ink += (r as String).count("#")
		if code != 32:
			_check(ink >= 2, "'%s' has ink" % ch)
	var atlas: Image = PixelFont.atlas_image()
	_check(atlas.get_width() == PixelFont.ATLAS_COLS * PixelFont.CELL_W, "atlas width")
	_check(atlas.get_pixel(0, 0).a == 0.0, "the space glyph is blank")
	var font: FontFile = PixelFont.build()
	_check(font != null, "font builds")
	_check(font.fixed_size == PixelFont.CELL_H, "font is fixed at the cell height")
	_check(PixelFont.build() == font, "font is cached")
	_check(font.has_char("A".unicode_at(0)), "font carries A")
	_check(font.has_char("~".unicode_at(0)), "font carries ~")
	var w: float = (
		font.get_string_size("Anabios", HORIZONTAL_ALIGNMENT_LEFT, -1, PixelFont.CELL_H).x
	)
	_check(
		is_equal_approx(w, 7.0 * PixelFont.CELL_W),
		"seven glyphs advance %d px each (got %.1f)" % [PixelFont.CELL_W, w]
	)
	_check(
		font.get_height(PixelFont.CELL_H) == float(PixelFont.ASCENT + PixelFont.DESCENT),
		"line height is ascent + descent"
	)
	if _failed:
		quit(1)
		return
	print("test_pixel_font: all passed")
	quit(0)
