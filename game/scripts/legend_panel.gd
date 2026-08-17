extends PanelContainer

const UiTheme = preload("res://scripts/ui_theme.gd")
const Palette = preload("res://scripts/palette.gd")

const GROUND_NAMES := [
	"biome", "phero-0", "phero-1", "phero-2", "phero-3", "env-optimum", "succession", "markets"
]
const BODY_NAMES := ["species", "dialect", "diet", "energy", "arousal"]

# Key bindings, pre-broken to lines that fit the panel's 370px slot at font 13.
# (One long "panels" line used to overflow the panel and bleed through the codex
# panel beside it; keep every line under ~50 characters.)
const CONTROLS_FMT := """[G] ground: %s
[C] body: %s
[M] module pips · [Y] co-evolution · [T] evolution
[X] DIT helix · [F] reset view · [H] hide
[R] replay event · [U] run to event · [V] event cam
WASD/drag pan · wheel zoom · click inspect"""

@onready var overlay = get_node("/root/Main/OverlayManager")
@onready var module_layers = get_node("/root/Main/ModuleLayers")

var _controls: Label
var _key_box: VBoxContainer
var _expanded: bool = true
var _last_body: int = -1
var _last_modules: bool = true


func _ready() -> void:
	# Replace the scene's placeholder Label with our own layout.
	for c in get_children():
		c.queue_free()
	var vb := VBoxContainer.new()
	vb.add_theme_constant_override("separation", 6)
	add_child(vb)
	_controls = Label.new()
	_controls.add_theme_font_size_override("font_size", 13)
	# The panel is a fixed 370px slot. Without wrapping, the key-binding lines ran
	# past its right edge and bled through the (translucent) codex panel next to
	# it; the lines below are pre-broken to fit, and this is the safety net for a
	# wider font or a longer overlay name.
	_controls.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	vb.add_child(_controls)
	_key_box = VBoxContainer.new()
	_key_box.add_theme_constant_override("separation", 3)
	vb.add_child(_key_box)


func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_H:
		_expanded = not _expanded
		_key_box.visible = _expanded


func _process(_delta: float) -> void:
	if not _expanded:
		_controls.text = "[H] show controls"
		return
	var g: int = clampi(overlay.ground_mode, 0, GROUND_NAMES.size() - 1)
	var b: int = clampi(overlay.body_mode, 0, BODY_NAMES.size() - 1)
	_controls.text = CONTROLS_FMT % [GROUND_NAMES[g], BODY_NAMES[b]]
	var m: bool = module_layers.visible
	if b != _last_body or m != _last_modules:
		_last_body = b
		_last_modules = m
		_rebuild_key(b)


func _rebuild_key(body_mode: int) -> void:
	for c in _key_box.get_children():
		c.queue_free()
	if module_layers.visible:
		_key_box.add_child(_header("modules"))
		_key_box.add_child(_swatch_wrap(Palette.MODULE_COLORS, Palette.MODULE_NAMES))
	_key_box.add_child(_header("links: bright brief = combat · thin lingering = trade"))
	match body_mode:
		1:
			_key_box.add_child(_header("body: hue = dialect"))
		2:
			_key_box.add_child(_header("body: diet"))
			_key_box.add_child(
				_ramp_row(Color(0.3, 0.9, 0.4), Color(1.0, 0.3, 0.3), "herbivore", "carnivore")
			)
		3:
			_key_box.add_child(_header("body: energy"))
			_key_box.add_child(_ramp_row(Color(0.2, 0.3, 0.8), Color(1.0, 0.9, 0.3), "low", "high"))
		4:
			_key_box.add_child(_header("body: arousal (fear/rage/panic)"))
			_key_box.add_child(
				_ramp_row(Color(0.55, 0.6, 0.7), Color(1.0, 0.35, 0.25), "calm", "aroused")
			)
		_:
			_key_box.add_child(_header("body: species — each animal in its own coat colours"))


func _header(text: String) -> Label:
	var l := Label.new()
	l.text = text
	l.add_theme_font_size_override("font_size", 11)
	l.add_theme_color_override("font_color", UiTheme.TEXT_DIM)
	return l


func _chip(col: Color) -> ColorRect:
	var r := ColorRect.new()
	r.color = col
	r.custom_minimum_size = Vector2(12, 12)
	return r


func _swatch_wrap(colors: PackedColorArray, names: PackedStringArray) -> HFlowContainer:
	var flow := HFlowContainer.new()
	flow.add_theme_constant_override("h_separation", 4)
	flow.add_theme_constant_override("v_separation", 2)
	for i in colors.size():
		var chip := _chip(colors[i])
		if i < names.size():
			chip.tooltip_text = names[i]
		flow.add_child(chip)
	return flow


func _ramp_row(a: Color, b: Color, left: String, right: String) -> HBoxContainer:
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 4)
	var la := Label.new()
	la.text = left
	la.add_theme_font_size_override("font_size", 11)
	row.add_child(la)
	# Five swatches interpolating a -> b as a compact ramp.
	for i in 5:
		row.add_child(_chip(a.lerp(b, float(i) / 4.0)))
	var lb := Label.new()
	lb.text = right
	lb.add_theme_font_size_override("font_size", 11)
	row.add_child(lb)
	return row
