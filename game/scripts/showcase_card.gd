extends CanvasLayer

# Cinematic chrome for showcase recordings: chapter title cards, lower-third
# captions, letterbox bars, and a vignette. Everything is fire-and-forget
# (tweened fades), driven by showcase_director.gd. Styled after the shared
# field-instrument theme (ui_theme.gd): one cyan-green accent on near-black.

const UiTheme = preload("res://scripts/ui_theme.gd")

const TITLE_FADE: float = 0.7
const BAR_FRAC: float = 0.085  # letterbox bar height as a fraction of viewport

var _title_box: CenterContainer
var _title_label: Label
var _title_rule: ColorRect
var _title_sub: Label
var _lower: PanelContainer
var _lower_label: Label
var _bar_top: ColorRect
var _bar_bottom: ColorRect
var _vignette: TextureRect
var _title_tween: Tween = null
var _lower_tween: Tween = null
var _bar_tween: Tween = null

func _ready() -> void:
	layer = 10
	_build_title()
	_build_lower()
	_build_letterbox()

func _build_title() -> void:
	_title_box = CenterContainer.new()
	_title_box.set_anchors_preset(Control.PRESET_FULL_RECT)
	_title_box.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_title_box.modulate.a = 0.0
	var vbox := VBoxContainer.new()
	vbox.alignment = BoxContainer.ALIGNMENT_CENTER
	vbox.add_theme_constant_override("separation", 10)
	_title_label = Label.new()
	_title_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_title_label.add_theme_font_size_override("font_size", 44)
	_title_label.add_theme_color_override("font_color", UiTheme.ACCENT)
	_title_label.add_theme_color_override("font_outline_color", Color(0, 0, 0, 0.8))
	_title_label.add_theme_constant_override("outline_size", 8)
	vbox.add_child(_title_label)
	var rule_center := CenterContainer.new()
	_title_rule = ColorRect.new()
	_title_rule.custom_minimum_size = Vector2(140, 2)
	_title_rule.color = UiTheme.ACCENT
	rule_center.add_child(_title_rule)
	vbox.add_child(rule_center)
	_title_sub = Label.new()
	_title_sub.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_title_sub.add_theme_font_size_override("font_size", 19)
	_title_sub.add_theme_color_override("font_color", UiTheme.TEXT)
	_title_sub.add_theme_color_override("font_outline_color", Color(0, 0, 0, 0.8))
	_title_sub.add_theme_constant_override("outline_size", 6)
	vbox.add_child(_title_sub)
	_title_box.add_child(vbox)
	add_child(_title_box)

func _build_lower() -> void:
	_lower = PanelContainer.new()
	_lower.theme = UiTheme.build()
	_lower.set_anchors_preset(Control.PRESET_BOTTOM_LEFT)
	_lower.offset_left = 28
	_lower.offset_top = -96
	_lower.offset_bottom = -52
	_lower.modulate.a = 0.0
	_lower.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_lower_label = Label.new()
	_lower_label.add_theme_font_size_override("font_size", 16)
	_lower_label.add_theme_color_override("font_color", UiTheme.TEXT)
	# Wrap long captions instead of overflowing the strip.
	_lower_label.custom_minimum_size.x = 560
	_lower_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_lower.add_child(_lower_label)
	add_child(_lower)

func _build_letterbox() -> void:
	_bar_top = ColorRect.new()
	_bar_top.color = Color.BLACK
	_bar_top.set_anchors_preset(Control.PRESET_TOP_WIDE)
	_bar_top.offset_bottom = 0
	_bar_top.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_bar_top)
	_bar_bottom = ColorRect.new()
	_bar_bottom.color = Color.BLACK
	_bar_bottom.set_anchors_preset(Control.PRESET_BOTTOM_WIDE)
	_bar_bottom.offset_top = 0
	_bar_bottom.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_bar_bottom)
	_vignette = TextureRect.new()
	_vignette.texture = _vignette_texture(256)
	_vignette.set_anchors_preset(Control.PRESET_FULL_RECT)
	_vignette.stretch_mode = TextureRect.STRETCH_SCALE
	_vignette.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_vignette.modulate.a = 0.0
	add_child(_vignette)

# Big centered chapter card: fade in, hold, fade out.
func show_title(chapter: String, subtitle: String, dur: float = 3.2) -> void:
	_title_label.text = chapter
	_title_sub.text = subtitle
	_title_sub.visible = subtitle != ""
	if _title_tween != null:
		_title_tween.kill()
	_title_tween = create_tween()
	_title_tween.tween_property(_title_box, "modulate:a", 1.0, TITLE_FADE)
	_title_tween.tween_interval(dur)
	_title_tween.tween_property(_title_box, "modulate:a", 0.0, TITLE_FADE)

# Bottom-left caption strip (event name, tick, short narration).
func show_lower(text: String, dur: float = 4.5) -> void:
	_lower_label.text = text
	if _lower_tween != null:
		_lower_tween.kill()
	_lower_tween = create_tween()
	_lower_tween.tween_property(_lower, "modulate:a", 1.0, 0.35)
	_lower_tween.tween_interval(dur)
	_lower_tween.tween_property(_lower, "modulate:a", 0.0, 0.5)

# Cinematic bars + vignette, animated in/out.
func set_letterbox(on: bool) -> void:
	if _bar_tween != null:
		_bar_tween.kill()
	var h: float = get_viewport().get_visible_rect().size.y * BAR_FRAC
	_bar_tween = create_tween()
	_bar_tween.set_parallel(true)
	_bar_tween.set_trans(Tween.TRANS_SINE)
	_bar_tween.tween_property(_bar_top, "offset_bottom", h if on else 0.0, 0.9)
	_bar_tween.tween_property(_bar_bottom, "offset_top", -h if on else 0.0, 0.9)
	_bar_tween.tween_property(_vignette, "modulate:a", 1.0 if on else 0.0, 0.9)

# Soft dark-edge vignette, brighter center (procedural, no asset ships).
func _vignette_texture(res: int) -> ImageTexture:
	var img := Image.create(res, res, false, Image.FORMAT_RGBA8)
	var c := (res - 1) * 0.5
	for y in res:
		for x in res:
			var d := Vector2(x - c, y - c).length() / c
			var a := 0.42 * smoothstep(0.62, 1.35, d)
			img.set_pixel(x, y, Color(0, 0, 0, a))
	return ImageTexture.create_from_image(img)
