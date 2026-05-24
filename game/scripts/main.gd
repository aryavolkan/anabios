extends Node2D

# Number of sim ticks to run per rendered frame. Speeds: 1, 4, 16, 64.
@export var ticks_per_frame: int = 1
@export var paused: bool = false

@onready var sim = $Simulation
@onready var bodies: MultiMeshInstance2D = $Bodies
@onready var hud: Label = $UI/HUD
@onready var inspector: PanelContainer = $UI/Inspector

func _ready() -> void:
	var scenario_path = "res://../scenarios/minimal.toml"
	var f = FileAccess.open(scenario_path, FileAccess.READ)
	if f == null:
		push_error("could not open " + scenario_path)
		return
	var text = f.get_as_text()
	f.close()
	if not sim.load_scenario(text):
		push_error("scenario load failed")

func _process(_delta: float) -> void:
	if not paused:
		sim.step_n(ticks_per_frame)
	_refresh_bodies()
	hud.text = "tick=%d alive=%d" % [sim.tick(), sim.alive_count()]

func _refresh_bodies() -> void:
	var n: int = int(sim.alive_count())
	var mm: MultiMesh = bodies.multimesh
	if n > mm.instance_count:
		mm.instance_count = n
	mm.visible_instance_count = n

	if n == 0:
		return

	var positions: PackedVector2Array = sim.alive_positions()
	var colors: PackedColorArray = sim.alive_colors()
	var sizes: PackedFloat32Array = sim.alive_sizes()
	for i in n:
		var t: Transform2D = Transform2D(0.0, Vector2(sizes[i], sizes[i]), 0.0, positions[i])
		mm.set_instance_transform_2d(i, t)
		mm.set_instance_color(i, colors[i])

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		if mb.button_index == MOUSE_BUTTON_LEFT and mb.pressed:
			var world_pos: Vector2 = ($Camera2D as Camera2D).get_global_mouse_position()
			var hit_id: int = int(sim.agent_near(world_pos, 4.0))
			inspector.pin(hit_id)
