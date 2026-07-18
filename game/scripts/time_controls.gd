extends HBoxContainer

const UiTheme = preload("res://scripts/ui_theme.gd")

@onready var main: Node2D = get_node("/root/Main")

var _speed_btns: Dictionary = {}

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
