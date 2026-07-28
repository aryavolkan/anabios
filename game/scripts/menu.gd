extends Control

const UiTheme = preload("res://scripts/ui_theme.gd")
const MenuBgShader := preload("res://shaders/menu_bg.gdshader")

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
	{ "label": "E8 — Settlements & markets", "path": "res://../scenarios/settlement.toml", "ground": 7, "body": 0 },
	{ "label": "E9 — Traditions & institutions", "path": "res://../scenarios/traditions.toml", "ground": 7, "body": 1 },
	{ "label": "E10 — Drifting climate (moving optimum)", "path": "res://../scenarios/drifting-climate.toml", "ground": 5, "body": 1 },
	{ "label": "E11 — Climate maladaptation (lag)", "path": "res://../scenarios/maladaptation.toml", "ground": 5, "body": 1 },
	{ "label": "E12 — Sexual dimorphism", "path": "res://../scenarios/dimorphism.toml", "ground": 0, "body": 2 },
	{ "label": "E13 — Domestication (husbandry pens)", "path": "res://../scenarios/domestication.toml", "ground": 0, "body": 1 },
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
	# Living title-screen background: a procedural "primordial field" (deep teal
	# depth gradient + drifting luminous cells) that evokes the sim without needing
	# a Simulation node on the menu. See shaders/menu_bg.gdshader.
	var bg_mat := ShaderMaterial.new()
	bg_mat.shader = MenuBgShader
	$Background.material = bg_mat
	# Seat the form in a translucent instrument panel (same language as the HUD:
	# near-black teal, a single accent hairline down the left edge) so the controls
	# read as one console instead of floating in the void.
	var form_panel := StyleBoxFlat.new()
	form_panel.bg_color = Color(0.035, 0.055, 0.065, 0.62)
	form_panel.set_corner_radius_all(6)
	form_panel.border_width_left = 2
	form_panel.border_color = UiTheme.ACCENT
	form_panel.content_margin_left = 28
	form_panel.content_margin_right = 28
	form_panel.content_margin_top = 22
	form_panel.content_margin_bottom = 22
	$FormPanel.add_theme_stylebox_override("panel", form_panel)
	# Title: accent color with a soft accent glow (a zero-offset shadow outline),
	# so "anabios" reads as lit rather than flat.
	var title: Label = $VBox/Title
	title.add_theme_color_override("font_color", UiTheme.ACCENT)
	title.add_theme_color_override("font_shadow_color", Color(0.30, 0.88, 0.70, 0.35))
	title.add_theme_constant_override("shadow_offset_x", 0)
	title.add_theme_constant_override("shadow_offset_y", 0)
	title.add_theme_constant_override("shadow_outline_size", 11)
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
