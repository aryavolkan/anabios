extends Control

const SCENARIOS: Array[Dictionary] = [
	{ "label": "Minimal (200 herbivores)", "path": "res://../scenarios/minimal.toml" },
	{ "label": "Divergent (two founders)", "path": "res://../scenarios/divergent.toml" },
]

@onready var scenario_pick: OptionButton = $VBox/ScenarioPick
@onready var seed_spin: SpinBox = $VBox/SeedRow/SeedSpin
@onready var scale_spin: SpinBox = $VBox/ScaleRow/ScaleSpin
@onready var start_btn: Button = $VBox/StartButton

func _ready() -> void:
	for s in SCENARIOS:
		scenario_pick.add_item(s["label"])
	seed_spin.value = GameConfig.seed
	scale_spin.value = GameConfig.ui_scale
	start_btn.pressed.connect(_on_start)

func _on_start() -> void:
	var idx: int = scenario_pick.selected
	if idx < 0:
		idx = 0
	GameConfig.scenario_path = SCENARIOS[idx]["path"]
	GameConfig.seed = int(seed_spin.value)
	GameConfig.ui_scale = scale_spin.value
	get_tree().change_scene_to_file("res://scenes/main.tscn")
