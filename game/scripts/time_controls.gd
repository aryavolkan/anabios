extends HBoxContainer

@onready var main: Node2D = get_node("/root/Main")

func _ready() -> void:
	$PauseButton.pressed.connect(_on_pause_pressed)
	$Speed1.pressed.connect(_on_speed.bind(1))
	$Speed4.pressed.connect(_on_speed.bind(4))
	$Speed16.pressed.connect(_on_speed.bind(16))
	$Speed64.pressed.connect(_on_speed.bind(64))
	$Restart.pressed.connect(_on_restart)
	$Menu.pressed.connect(_on_menu)

func _on_pause_pressed() -> void:
	main.paused = not main.paused
	$PauseButton.text = "▶" if main.paused else "⏸"

func _on_speed(n: int) -> void:
	main.ticks_per_frame = n

func _on_restart() -> void:
	get_tree().reload_current_scene()

func _on_menu() -> void:
	get_tree().change_scene_to_file("res://scenes/menu.tscn")
