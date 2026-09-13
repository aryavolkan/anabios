extends PanelContainer
# Title block (Phase 5 of docs/superpowers/specs/2026-09-12-pixel-world-at-
# scale-design.md): the reference boards' top-left logo — the DNA mark at 2x
# beside the game name and its tagline. Static; no sim reads.

const HudIcons = preload("res://scripts/hud_icons.gd")
const UiTheme = preload("res://scripts/ui_theme.gd")

const TAGLINE := "Life Evolves. Worlds Respond."


func _ready() -> void:
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 8)
	add_child(row)
	var mark := TextureRect.new()
	mark.texture = HudIcons.kind_texture(HudIcons.DNA)
	mark.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	mark.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	mark.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	mark.custom_minimum_size = Vector2(32, 32)
	row.add_child(mark)
	var col := VBoxContainer.new()
	col.add_theme_constant_override("separation", 0)
	row.add_child(col)
	var name := Label.new()
	name.text = "ANABIOS"
	name.add_theme_font_size_override("font_size", 18)
	name.add_theme_color_override("font_color", UiTheme.TEXT)
	col.add_child(name)
	var tag := Label.new()
	tag.text = TAGLINE
	tag.add_theme_font_size_override("font_size", 11)
	tag.add_theme_color_override("font_color", UiTheme.ACCENT)
	col.add_child(tag)
