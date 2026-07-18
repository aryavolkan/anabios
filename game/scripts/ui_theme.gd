extends RefCounted

# Shared HUD look: a "living field-instrument" language. Deep near-black teal
# panels (translucent, so the world still breathes underneath), each marked by
# a single cyan-green accent hairline down its left edge. One accent, applied
# consistently — the panels read as one instrument, not stock Godot controls.

const BG_PANEL := Color(0.035, 0.055, 0.065, 0.88)
const BG_ELEV := Color(0.08, 0.115, 0.13, 0.94)
const BG_HOVER := Color(0.12, 0.17, 0.19, 0.96)
const BG_PRESSED := Color(0.10, 0.24, 0.22, 0.96)
const ACCENT := Color(0.30, 0.88, 0.70)
const ACCENT_DIM := Color(0.30, 0.88, 0.70, 0.28)
const TEXT := Color(0.86, 0.92, 0.93)
const TEXT_DIM := Color(0.56, 0.67, 0.69)

static func build() -> Theme:
	var theme := Theme.new()

	var panel := StyleBoxFlat.new()
	panel.bg_color = BG_PANEL
	panel.set_corner_radius_all(4)
	panel.border_width_left = 2
	panel.border_color = ACCENT
	panel.content_margin_left = 11
	panel.content_margin_right = 9
	panel.content_margin_top = 7
	panel.content_margin_bottom = 7
	theme.set_stylebox("panel", "PanelContainer", panel)

	theme.set_color("font_color", "Label", TEXT)
	theme.set_font_size("font_size", "Label", 12)

	theme.set_stylebox("normal", "Button", _button_box(BG_ELEV, ACCENT_DIM))
	theme.set_stylebox("hover", "Button", _button_box(BG_HOVER, ACCENT))
	theme.set_stylebox("pressed", "Button", _button_box(BG_PRESSED, ACCENT))
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
