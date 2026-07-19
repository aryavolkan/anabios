extends Node2D

const UiTheme = preload("res://scripts/ui_theme.gd")
const Palette = preload("res://scripts/palette.gd")

# Number of sim ticks to run per rendered frame. Speeds: 1, 4, 16, 64.
@export var ticks_per_frame: int = 1
@export var paused: bool = false

const MODULE_COLORS: PackedColorArray = Palette.MODULE_COLORS
# Bodies are 0.5–3.0 world units across (genome size), which is only a few
# pixels at default zoom. Scale them up with a floor so even the smallest
# organism is an easy-to-see mark, not a stray pixel.
const BODY_SCALE: float = 3.2
const BODY_MIN: float = 2.6
const GLYPH_SIZE: float = 1.6

@onready var sim = $Simulation
@onready var bodies: MultiMeshInstance2D = $Bodies
@onready var hud: Label = $UI/HUD
@onready var inspector: PanelContainer = $UI/Inspector
@onready var module_layers: Node2D = $ModuleLayers
@onready var overlay = $OverlayManager
@onready var carcasses: MultiMeshInstance2D = $Carcasses
@onready var flashes: MultiMeshInstance2D = $Flashes
@onready var streaks: MultiMeshInstance2D = $Streaks

func _ready() -> void:
	var scenario_path: String = GameConfig.scenario_path
	var f = FileAccess.open(scenario_path, FileAccess.READ)
	if f == null:
		push_error("could not open " + scenario_path)
		return
	var text = f.get_as_text()
	f.close()
	if not sim.load_scenario_with_seed(text, GameConfig.seed):
		push_error("scenario load failed")
	# Apply UI scale from the menu.
	var s: float = GameConfig.ui_scale
	$UI.transform = Transform2D(0.0, Vector2(s, s), 0.0, Vector2.ZERO)
	_apply_ui_theme()
	var disc := _disc_texture()
	bodies.texture = disc
	carcasses.texture = disc
	flashes.texture = disc
	# streaks keep the raw quad: a solid line reads as a crisp shot streak.

# Give every HUD panel the shared instrument theme, and make the top-left
# readout legible over any terrain with a dark outline.
func _apply_ui_theme() -> void:
	var theme := UiTheme.build()
	for child in $UI.get_children():
		if child is Control:
			(child as Control).theme = theme
	hud.add_theme_color_override("font_color", UiTheme.ACCENT)
	hud.add_theme_color_override("font_outline_color", Color(0.0, 0.0, 0.0, 0.75))
	hud.add_theme_constant_override("outline_size", 5)
	hud.add_theme_font_size_override("font_size", 17)

func _notification(what: int) -> void:
	# Pause when the window loses focus; user resumes manually.
	if what == NOTIFICATION_APPLICATION_FOCUS_OUT:
		paused = true

func _process(_delta: float) -> void:
	if not paused:
		sim.step_n(ticks_per_frame)
	_refresh_bodies()
	_refresh_carcasses()
	_refresh_flashes()
	_refresh_streaks()
	var rate: String = "paused" if paused else ("%d×" % ticks_per_frame)
	hud.text = "tick %d · %d alive · %s" % [sim.tick(), sim.alive_count(), rate]

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
	var sizes: PackedFloat32Array = sim.alive_sizes()
	var rots: PackedFloat32Array = sim.alive_rotations()
	var body_colors: PackedColorArray = _body_colors(n)
	for i in n:
		var sz: float = maxf(sizes[i] * BODY_SCALE, BODY_MIN)
		var t: Transform2D = Transform2D(rots[i], Vector2(sz, sz), 0.0, positions[i])
		mm.set_instance_transform_2d(i, t)
		mm.set_instance_color(i, body_colors[i])

	_refresh_module_layers()

func _body_colors(n: int) -> PackedColorArray:
	match overlay.body_mode:
		overlay.BODY_DIALECT:
			var hues: PackedFloat32Array = sim.alive_dialect_hue()
			var out := PackedColorArray()
			out.resize(n)
			for i in n:
				out[i] = Color.from_hsv(hues[i], 0.7, 0.95)
			return out
		overlay.BODY_DIET:
			var diet: PackedFloat32Array = sim.alive_diet()
			var out2 := PackedColorArray()
			out2.resize(n)
			for i in n:
				out2[i] = Color(0.3, 0.9, 0.4).lerp(Color(1.0, 0.3, 0.3), clampf(diet[i], 0.0, 1.0))
			return out2
		overlay.BODY_ENERGY:
			var en: PackedFloat32Array = sim.alive_energy()
			var out3 := PackedColorArray()
			out3.resize(n)
			for i in n:
				var t := clampf(en[i] / 50.0, 0.0, 1.0)
				out3[i] = Color(0.2, 0.3, 0.8).lerp(Color(1.0, 0.9, 0.3), t)
			return out3
		_:
			return sim.alive_colors()

# A soft white disc (alpha falls off to the edge). Multiplied by each MultiMesh
# instance color, it turns the flat body quads into rounded, organic marks.
func _disc_texture(res: int = 32) -> ImageTexture:
	var img := Image.create(res, res, false, Image.FORMAT_RGBA8)
	var c := (res - 1) * 0.5
	for y in res:
		for x in res:
			var d := Vector2(x - c, y - c).length() / c          # 0 center .. 1 edge
			var a := clampf(1.0 - smoothstep(0.75, 1.0, d), 0.0, 1.0)
			img.set_pixel(x, y, Color(1.0, 1.0, 1.0, a))
	return ImageTexture.create_from_image(img)

func _refresh_carcasses() -> void:
	var data: Array = sim.carcass_data()
	var mm: MultiMesh = carcasses.multimesh
	var m: int = data.size()
	if m > mm.instance_count:
		mm.instance_count = m
	mm.visible_instance_count = m
	for i in m:
		var d: Dictionary = data[i]
		var pos: Vector2 = d["pos"]
		var f: float = clampf(float(d["flesh"]) / 20.0 * 4.0, 3.0, 7.0)
		mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(f, f), 0.0, pos))
		mm.set_instance_color(i, Color(0.77, 0.80, 0.86, 0.55))

func _refresh_flashes() -> void:
	var pts: PackedVector2Array = sim.combat_flashes()
	var mm: MultiMesh = flashes.multimesh
	var m: int = pts.size()
	if m > mm.instance_count:
		mm.instance_count = m
	mm.visible_instance_count = m
	for i in m:
		mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(6.0, 6.0), 0.0, pts[i]))
		mm.set_instance_color(i, Color(1.0, 0.92, 0.45, 0.95))

# Combat streaks: attacker→target segments, kept on screen for a few ticks
# as fading tracers so ranged (Spines) volleys read as volleys rather than
# single-frame slivers that are easy to miss between frames. Streaks tint to
# the attacker's genome hue, so each species' fire is distinguishable.
const STREAK_TTL: int = 8
var _streak_trail: Array = [] # entries: [from: Vector2, to: Vector2, ttl: int, color: Color]

func _refresh_streaks() -> void:
	var segs: PackedVector2Array = sim.combat_streaks()
	var cols: PackedColorArray = sim.combat_streak_colors()
	for i in segs.size() / 2:
		_streak_trail.append([segs[2 * i], segs[2 * i + 1], STREAK_TTL, cols[i]])
	var mm: MultiMesh = streaks.multimesh
	# Perf: cap the trail at the multimesh budget, dropping the oldest first.
	if _streak_trail.size() > mm.instance_count:
		_streak_trail = _streak_trail.slice(_streak_trail.size() - mm.instance_count)
	var kept: Array = []
	for s in _streak_trail:
		s[2] -= 1
		if s[2] > 0:
			kept.append(s)
	_streak_trail = kept
	var m: int = mini(_streak_trail.size(), mm.instance_count)
	mm.visible_instance_count = m
	for i in m:
		var from: Vector2 = _streak_trail[i][0]
		var to: Vector2 = _streak_trail[i][1]
		var delta: Vector2 = to - from
		var len: float = maxf(delta.length(), 0.001)
		var mid: Vector2 = (from + to) * 0.5
		mm.set_instance_transform_2d(i, Transform2D(delta.angle(), Vector2(len, 0.6), 0.0, mid))
		var c: Color = _streak_trail[i][3]
		c.a = 0.85 * float(_streak_trail[i][2]) / float(STREAK_TTL)
		mm.set_instance_color(i, c)

func _refresh_module_layers() -> void:
	var all: Array = sim.module_glyphs_all()
	var type_count: int = all.size()
	for t in type_count:
		var layer: MultiMeshInstance2D = module_layers.get_child(t)
		var glyphs: PackedVector2Array = all[t]
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
