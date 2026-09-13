extends RefCounted

# Shared HUD look: a "living field-instrument" language. Deep near-black teal
# panels (translucent, so the world still breathes underneath), each marked by
# a single cyan-green accent hairline down its left edge. One accent, applied
# consistently — the panels read as one instrument, not stock Godot controls.

# The world backdrop (rendering/environment/defaults/default_clear_color in
# project.godot, visible past the 3x3 terrain wrap tiles and on a scenario that
# fails to load) is deliberately a near-match for BG_PANEL: Godot's stock
# mid-grey clashed hard with the near-black instrument HUD. Keep them in step.
# This note lives here because project.godot cannot: Godot strips comments and
# prunes default-valued entries whenever it re-saves the file.
const BG_PANEL := Color(0.035, 0.055, 0.065, 0.88)
const BG_ELEV := Color(0.08, 0.115, 0.13, 0.94)
const BG_HOVER := Color(0.12, 0.17, 0.19, 0.96)
const BG_PRESSED := Color(0.10, 0.24, 0.22, 0.96)
const ACCENT := Color(0.30, 0.88, 0.70)
const ACCENT_DIM := Color(0.30, 0.88, 0.70, 0.28)
const PixelFont = preload("res://scripts/pixel_font.gd")

const TEXT := Color(0.86, 0.92, 0.93)
const TEXT_DIM := Color(0.56, 0.67, 0.69)


static func build() -> Theme:
	var theme := Theme.new()
	# Pixel HUD text: the fixed-size bitmap font scales by whole steps, so
	# the 11-13 px sizes below all render at 9 px per line and the brand
	# name at 18.
	theme.default_font = PixelFont.build()
	theme.default_font_size = PixelFont.CELL_H

	theme.set_stylebox("panel", "PanelContainer", pixel_frame(BG_PANEL, ACCENT_DIM, 9, 7))

	theme.set_color("font_color", "Label", TEXT)
	theme.set_font_size("font_size", "Label", 12)

	theme.set_stylebox("normal", "Button", pixel_frame(BG_ELEV, ACCENT_DIM, 8, 4))
	theme.set_stylebox("hover", "Button", pixel_frame(BG_HOVER, ACCENT, 8, 4))
	theme.set_stylebox("pressed", "Button", pixel_frame(BG_PRESSED, ACCENT, 8, 4))
	var focus := StyleBoxFlat.new()
	focus.bg_color = Color(0, 0, 0, 0)
	focus.set_corner_radius_all(3)
	focus.set_border_width_all(1)
	focus.border_color = ACCENT
	theme.set_stylebox("focus", "Button", focus)
	theme.set_color("font_color", "Button", TEXT)
	theme.set_color("font_hover_color", "Button", ACCENT)
	theme.set_color("font_pressed_color", "Button", ACCENT)
	theme.set_color("font_focus_color", "Button", TEXT)
	theme.set_font_size("font_size", "Button", 12)

	# --- Form controls (menu screen) ---
	theme.set_stylebox("normal", "OptionButton", _button_box(BG_ELEV, ACCENT_DIM))
	theme.set_stylebox("hover", "OptionButton", _button_box(BG_HOVER, ACCENT))
	theme.set_stylebox("pressed", "OptionButton", _button_box(BG_PRESSED, ACCENT))
	theme.set_stylebox("focus", "OptionButton", focus)
	theme.set_color("font_color", "OptionButton", TEXT)
	theme.set_color("font_hover_color", "OptionButton", ACCENT)
	theme.set_font_size("font_size", "OptionButton", 13)

	var field := _button_box(BG_ELEV, ACCENT_DIM)
	theme.set_stylebox("normal", "LineEdit", field)
	theme.set_stylebox("focus", "LineEdit", focus)
	theme.set_color("font_color", "LineEdit", TEXT)
	theme.set_color("caret_color", "LineEdit", ACCENT)
	theme.set_font_size("font_size", "LineEdit", 13)

	var popup := StyleBoxFlat.new()
	popup.bg_color = Color(0.05, 0.07, 0.085, 0.98)
	popup.set_corner_radius_all(4)
	popup.set_border_width_all(1)
	popup.border_color = ACCENT_DIM
	popup.set_content_margin_all(4)
	theme.set_stylebox("panel", "PopupMenu", popup)
	theme.set_color("font_color", "PopupMenu", TEXT)
	theme.set_color("font_hover_color", "PopupMenu", ACCENT)
	theme.set_color("font_accelerator_color", "PopupMenu", TEXT_DIM)
	theme.set_font_size("font_size", "PopupMenu", 13)

	return theme


# A pixel-art 9-slice frame (Phase 5, D1): a 2px near-black outline, a 1px
# bevel in the accent colour along the top/left and a darker one along the
# bottom/right, over the translucent panel fill. Built once per (fill, accent)
# pair as a 12x12 Image, so panels and buttons carry the reference boards'
# framed look without any imported texture.
static var _frame_cache: Dictionary = {}


static func pixel_frame(fill: Color, accent: Color, pad_x: int, pad_y: int) -> StyleBoxTexture:
	var key := "%s|%s" % [fill.to_html(), accent.to_html()]
	var tex: ImageTexture = _frame_cache.get(key)
	if tex == null:
		var n := 12
		var img := Image.create(n, n, false, Image.FORMAT_RGBA8)
		img.fill(fill)
		var outline := Color(0.03, 0.04, 0.05, 1.0)
		var lit := Color(accent.r, accent.g, accent.b, 0.9)
		var dark := Color(accent.r * 0.35, accent.g * 0.35, accent.b * 0.35, 0.9)
		for i in n:
			for e in [0, 1]:
				img.set_pixel(i, e, outline)
				img.set_pixel(i, n - 1 - e, outline)
				img.set_pixel(e, i, outline)
				img.set_pixel(n - 1 - e, i, outline)
		for i in range(2, n - 2):
			img.set_pixel(i, 2, lit)
			img.set_pixel(2, i, lit)
			img.set_pixel(i, n - 3, dark)
			img.set_pixel(n - 3, i, dark)
		# Notched corners: the light bevel wins the top-left corner, the dark
		# one the bottom-right, the two others meet as outline.
		img.set_pixel(2, 2, lit)
		img.set_pixel(n - 3, n - 3, dark)
		tex = ImageTexture.create_from_image(img)
		_frame_cache[key] = tex
	var sb := StyleBoxTexture.new()
	sb.texture = tex
	sb.texture_margin_left = 4
	sb.texture_margin_top = 4
	sb.texture_margin_right = 4
	sb.texture_margin_bottom = 4
	sb.content_margin_left = pad_x
	sb.content_margin_right = pad_x
	sb.content_margin_top = pad_y
	sb.content_margin_bottom = pad_y
	sb.draw_center = true
	return sb


static func _button_box(bg: Color, border: Color) -> StyleBoxFlat:
	var sb := StyleBoxFlat.new()
	sb.bg_color = bg
	sb.set_corner_radius_all(3)
	sb.set_border_width_all(1)
	sb.border_color = border
	sb.content_margin_left = 8
	sb.content_margin_right = 8
	sb.content_margin_top = 4
	sb.content_margin_bottom = 4
	return sb
