extends HBoxContainer
# Bottom-centre hotbar (Phase 5 step 8 of docs/superpowers/specs/2026-09-12-
# pixel-world-at-scale-design.md): the time controls, then one icon button
# per keyboard tool. A tool button feeds its key through the input pipeline,
# so the panels keep their single `_unhandled_key_input` handler each and the
# button and the key can never drift apart.

const HudIcons = preload("res://scripts/hud_icons.gd")
const UiTheme = preload("res://scripts/ui_theme.gd")

# [key name, tooltip, icon kind], in hotbar order.
const TOOLS: Array = [
	["G", "[G] cycle ground overlay", HudIcons.LEAF],
	["C", "[C] cycle body overlay", HudIcons.PAW],
	["M", "[M] module pips", HudIcons.LEGS],
	["T", "[T] evolution panel", HudIcons.DNA],
	["Y", "[Y] co-evolution chart", HudIcons.BOOK],
	["R", "[R] replay last event", HudIcons.SUN],
	["V", "[V] event camera", HudIcons.EYE],
	["H", "[H] controls and legend", HudIcons.ERA],
]

@onready var main: Node2D = get_node("../..")

var _speed_btns: Dictionary = {}


# A pressed key event for a tool's key name ("G" -> KEY_G).
static func key_event(key_name: String) -> InputEventKey:
	var ev := InputEventKey.new()
	ev.keycode = OS.find_keycode_from_string(key_name)
	ev.physical_keycode = ev.keycode
	ev.pressed = true
	return ev


func _ready() -> void:
	$PauseButton.pressed.connect(_on_pause_pressed)
	$Speed1.pressed.connect(_on_speed.bind(1))
	$Speed4.pressed.connect(_on_speed.bind(4))
	$Speed16.pressed.connect(_on_speed.bind(16))
	$Speed64.pressed.connect(_on_speed.bind(64))
	$Restart.pressed.connect(_on_restart)
	$Menu.pressed.connect(_on_menu)
	_speed_btns = {1: $Speed1, 4: $Speed4, 16: $Speed16, 64: $Speed64}
	_highlight_speed(main.ticks_per_frame)
	var divider := Label.new()
	divider.text = "|"
	divider.add_theme_color_override("font_color", UiTheme.TEXT_DIM)
	add_child(divider)
	move_child(divider, $Restart.get_index())
	for tool in TOOLS:
		var b := Button.new()
		b.icon = HudIcons.kind_texture(int(tool[2]))
		b.tooltip_text = String(tool[1])
		b.focus_mode = Control.FOCUS_NONE
		b.pressed.connect(_on_tool.bind(String(tool[0])))
		add_child(b)
		move_child(b, $Restart.get_index())


func _on_tool(key_name: String) -> void:
	Input.parse_input_event(key_event(key_name))


func _on_pause_pressed() -> void:
	main.paused = not main.paused
	$PauseButton.text = "▶" if main.paused else "⏸"


func _on_speed(n: int) -> void:
	main.ticks_per_frame = n
	_highlight_speed(n)


# Mark the active speed with the accent so the current rate is obvious.
func _highlight_speed(n: int) -> void:
	for k in _speed_btns:
		var btn: Button = _speed_btns[k]
		btn.add_theme_color_override("font_color", UiTheme.ACCENT if k == n else UiTheme.TEXT)


func _on_restart() -> void:
	get_tree().reload_current_scene()


func _on_menu() -> void:
	get_tree().change_scene_to_file("res://scenes/menu.tscn")
