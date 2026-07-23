extends Control

const UiTheme = preload("res://scripts/ui_theme.gd")

const SCENARIOS: Array[Dictionary] = [
	# Foundations
	{ "label": "Foundations — Minimal (200 herbivores)", "path": "res://../scenarios/minimal.toml", "ground": 0, "body": 0 },
	{ "label": "Foundations — Divergent (two founders)", "path": "res://../scenarios/divergent.toml", "ground": 0, "body": 0 },
	# Milestones
	{ "label": "M12 — Predator / prey", "path": "res://../scenarios/predator-prey.toml", "ground": 0, "body": 2 },
	{ "label": "E3 — Trophic cascade", "path": "res://../scenarios/trophic-cascade.toml", "ground": 0, "body": 2 },
	{ "label": "E4 — Disturbance (fire & succession)", "path": "res://../scenarios/disturbance.toml", "ground": 6, "body": 0 },
	{ "label": "E5 — Convergent evolution", "path": "res://../scenarios/convergent.toml", "ground": 0, "body": 0 },
	{ "label": "E7 — War & alliance", "path": "res://../scenarios/war.toml", "ground": 0, "body": 2 },
	{ "label": "M13 — Territories (pheromones)", "path": "res://../scenarios/territories.toml", "ground": 1, "body": 0 },
	{ "label": "M14 — Dialects (memes)", "path": "res://../scenarios/dialects.toml", "ground": 1, "body": 1 },
	{ "label": "M15 — Cooperation & kin", "path": "res://../scenarios/cooperation.toml", "ground": 0, "body": 0 },
	{ "label": "Gene–culture (baseline)", "path": "res://../scenarios/gene-culture.toml", "ground": 0, "body": 1 },
	{ "label": "Gene–culture — Skill", "path": "res://../scenarios/gene-culture-skill.toml", "ground": 0, "body": 1 },
	{ "label": "Gene–culture — Hunt", "path": "res://../scenarios/gene-culture-hunt.toml", "ground": 0, "body": 2 },
	{ "label": "Gene–culture — Alarm", "path": "res://../scenarios/gene-culture-alarm.toml", "ground": 1, "body": 1 },
	# DIT boundary
	{ "label": "DIT — Env slow (culture tracks)", "path": "res://../scenarios/dit-env-slow.toml", "ground": 5, "body": 1 },
	{ "label": "DIT — Env fast (culture stale)", "path": "res://../scenarios/dit-env-fast.toml", "ground": 5, "body": 1 },
	{ "label": "DIT — Env static (culture redundant)", "path": "res://../scenarios/dit-env-static.toml", "ground": 5, "body": 1 },
	{ "label": "DIT — Rogers (imitators invade)", "path": "res://../scenarios/dit-rogers.toml", "ground": 5, "body": 1 },
	# Invention tree
	{ "label": "Inventions — innovators vs traditionalists", "path": "res://../scenarios/inventions.toml", "ground": 0, "body": 1 },
	{ "label": "Cognitive — IQ, tech & bad ideas", "path": "res://../scenarios/cognitive-coevolution.toml", "ground": 0, "body": 1 },
]

@onready var scenario_pick: OptionButton = $VBox/ScenarioPick
@onready var seed_spin: SpinBox = $VBox/SeedRow/SeedSpin
@onready var scale_spin: SpinBox = $VBox/ScaleRow/ScaleSpin
@onready var start_btn: Button = $VBox/StartButton

func _ready() -> void:
	theme = UiTheme.build()
	$Background.color = Color(0.04, 0.055, 0.07, 1.0)
	$VBox/Title.add_theme_color_override("font_color", UiTheme.ACCENT)
	$VBox/Subtitle.add_theme_color_override("font_color", UiTheme.TEXT_DIM)
	for s in SCENARIOS:
		scenario_pick.add_item(s["label"])
	seed_spin.value = GameConfig.seed
	scale_spin.value = GameConfig.ui_scale
	start_btn.pressed.connect(_on_start)

func _on_start() -> void:
	var idx: int = scenario_pick.selected
	if idx < 0:
		idx = 0
	var s: Dictionary = SCENARIOS[idx]
	GameConfig.scenario_path = s["path"]
	GameConfig.seed = int(seed_spin.value)
	GameConfig.ui_scale = scale_spin.value
	GameConfig.default_ground = int(s["ground"])
	GameConfig.default_body = int(s["body"])
	get_tree().change_scene_to_file("res://scenes/main.tscn")
