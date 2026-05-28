extends Node2D

# Number of sim ticks to run per rendered frame. Speeds: 1, 4, 16, 64.
@export var ticks_per_frame: int = 1
@export var paused: bool = false

const MODULE_COLORS: PackedColorArray = [
	Color(0.6, 0.8, 1.0),   # 0 Locomotor
	Color(0.4, 0.9, 0.5),   # 1 Sensor
	Color(1.0, 0.7, 0.3),   # 2 Mouth
	Color(1.0, 0.3, 0.3),   # 3 Weapon
	Color(0.7, 0.7, 0.7),   # 4 Armor
	Color(0.9, 0.9, 0.4),   # 5 Storage
	Color(0.8, 0.5, 1.0),   # 6 Communicator
	Color(0.5, 1.0, 0.9),   # 7 Pheromone
	Color(1.0, 0.5, 0.8),   # 8 Reproductive
]
const GLYPH_SIZE: float = 0.7

@onready var sim = $Simulation
@onready var bodies: MultiMeshInstance2D = $Bodies
@onready var hud: Label = $UI/HUD
@onready var inspector: PanelContainer = $UI/Inspector
@onready var module_layers: Node2D = $ModuleLayers

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
		_clear_module_layers()
		return

	var positions: PackedVector2Array = sim.alive_positions()
	var colors: PackedColorArray = sim.alive_colors()
	var sizes: PackedFloat32Array = sim.alive_sizes()
	var rots: PackedFloat32Array = sim.alive_rotations()
	for i in n:
		var t: Transform2D = Transform2D(rots[i], Vector2(sizes[i], sizes[i]), 0.0, positions[i])
		mm.set_instance_transform_2d(i, t)
		mm.set_instance_color(i, colors[i])

	_refresh_module_layers()

func _refresh_module_layers() -> void:
	var type_count: int = int(sim.module_type_count())
	for t in type_count:
		var layer: MultiMeshInstance2D = module_layers.get_child(t)
		var glyphs: PackedVector2Array = sim.module_glyphs(t)
		var m: int = glyphs.size()
		var mm: MultiMesh = layer.multimesh
		if m > mm.instance_count:
			mm.instance_count = m
		mm.visible_instance_count = m
		var col: Color = MODULE_COLORS[t]
		for i in m:
			mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(GLYPH_SIZE, GLYPH_SIZE), 0.0, glyphs[i]))
			mm.set_instance_color(i, col)

func _clear_module_layers() -> void:
	for child in module_layers.get_children():
		(child as MultiMeshInstance2D).multimesh.visible_instance_count = 0

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		if mb.button_index == MOUSE_BUTTON_LEFT and mb.pressed:
			var world_pos: Vector2 = ($Camera2D as Camera2D).get_global_mouse_position()
			var hit_id: int = int(sim.agent_near(world_pos, 4.0))
			inspector.pin(hit_id)
