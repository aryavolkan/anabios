extends Control

const UiTheme = preload("res://scripts/ui_theme.gd")
const MenuBgShader := preload("res://shaders/menu_bg.gdshader")

const SCENARIOS: Array[Dictionary] = [
	# Foundations
	{
		"label": "Minimal — 200 herbivores",
		"path": "res://../scenarios/minimal.toml",
		"ground": 0,
		"body": 0
	},
	{
		"label": "Predator / prey — cycles & cascades",
		"path": "res://../scenarios/predator-prey.toml",
		"ground": 0,
		"body": 2
	},
	{
		"label": "Speciation — morphs, dialects, kin",
		"path": "res://../scenarios/speciation.toml",
		"ground": 1,
		"body": 1
	},
	# Worlds
	{
		"label": "Tribes — tools, war, traditions",
		"path": "res://../scenarios/tribes.toml",
		"ground": 0,
		"body": 1
	},
	{
		"label": "Cognition threshold — IQ ladder vs fitness",
		"path": "res://../scenarios/cognition-threshold.toml",
		"ground": 0,
		"body": 1
	},
	{
		"label": "Markets — settlements & trade",
		"path": "res://../scenarios/markets.toml",
		"ground": 7,
		"body": 0
	},
	{
		"label": "Habitat — land / sea / air territories",
		"path": "res://../scenarios/habitat-territories.toml",
		"ground": 8,
		"body": 0
	},
	{
		"label": "Grand theater — staged emergence",
		"path": "res://../scenarios/grand-theater.toml",
		"ground": 0,
		"body": 0
	},
	{
		"label": "Out of Africa — the saga",
		"path": "res://../scenarios/out-of-africa-saga.toml",
		"ground": 0,
		"body": 0
	},
	{
		"label": "Out of Africa — Earth (4096)",
		"path": "res://../scenarios/out-of-africa-earth.toml",
		"ground": 0,
		"body": 0
	},
	# Scale
	{
		"label": "Sandbox — 2048², 8k cap",
		"path": "res://../scenarios/sandbox.toml",
		"ground": 0,
		"body": 0
	},
	{
		"label": "Riverlands — 4096 continents",
		"path": "res://../scenarios/riverlands.toml",
		"ground": 0,
		"body": 0
	},
	{
		"label": "Huge steppe — 8192 scale test",
		"path": "res://../scenarios/huge-steppe.toml",
		"ground": 0,
		"body": 0
	},
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
	seed_spin.value = GameConfig.rng_seed
	scale_spin.value = GameConfig.ui_scale
	start_btn.pressed.connect(_on_start)


func _on_start() -> void:
	var idx: int = scenario_pick.selected
	if idx < 0:
		idx = 0
	var s: Dictionary = SCENARIOS[idx]
	GameConfig.scenario_path = s["path"]
	GameConfig.rng_seed = int(seed_spin.value)
	GameConfig.ui_scale = scale_spin.value
	GameConfig.default_ground = int(s["ground"])
	GameConfig.default_body = int(s["body"])
	get_tree().change_scene_to_file("res://scenes/main.tscn")
