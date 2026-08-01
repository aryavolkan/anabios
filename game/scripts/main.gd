extends Node2D

const UiTheme = preload("res://scripts/ui_theme.gd")
const Palette = preload("res://scripts/palette.gd")
const ApeSprites = preload("res://scripts/ape_sprites.gd")
const FieldAgentShader = preload("res://shaders/field_agent.gdshader")

# Number of sim ticks to run per rendered frame. Speeds: 1, 4, 16, 64.
@export var ticks_per_frame: int = 1
@export var paused: bool = false

const MODULE_COLORS: PackedColorArray = Palette.MODULE_COLORS
# Bodies are 0.5–3.0 world units across (genome size). Scale them up generously
# with a floor so the hominin silhouette (head, limbs) reads as a figure at the
# default cluster-framed zoom — not just when zoomed all the way in.
const BODY_SCALE: float = 7.0
const BODY_MIN: float = 6.0
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
@onready var trade_routes: MultiMeshInstance2D = $TradeRoutes

# One MultiMesh per ape species ($Bodies is species 0; the rest are created in
# _ready). Each draws its own small 4-pose atlas — a single wide all-species
# atlas corrupts on the canvas MultiMesh path (wide-thin textures sample
# garbage there), and per-species meshes dodge it entirely.
var _body_mmis: Array[MultiMeshInstance2D] = []
var _glyph_clones: Array[MultiMeshInstance2D] = []

# Smooth-motion state. Agents teleport once per tick; rendering eases each
# body toward its latest tick position so movement glides instead of
# stepping. Identity is tracked by agent id (alive indices reshuffle as
# agents die): both the previous and current id arrays are ascending, so a
# two-pointer merge finds each agent's last smoothed position in O(n).
# A jump larger than SNAP_DIST (torus seam crossing, or many ticks per
# frame at high speed) snaps straight to the target — time-lapse stays crisp.
# SMOOTH is the per-frame approach at 60 fps; _refresh_bodies scales it by
# the real frame delta so the glide looks identical at any frame rate.
const SMOOTH: float = 0.35
const SNAP_DIST: float = 4.0
var _prev_ids: PackedInt32Array = PackedInt32Array()
var _prev_smooth: PackedVector2Array = PackedVector2Array()


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
	# Open framed on the living cluster so the agents (now little hominins) read
	# immediately, instead of as dots in the whole-world view. [F] resets to the
	# world overview. Screenshot runs that set their own zoom opt out.
	if not OS.has_environment("ANABIOS_ZOOM"):
		($Camera2D as Camera2D).fit_to_agents()
	# Apply UI scale from the menu.
	var s: float = GameConfig.ui_scale
	$UI.transform = Transform2D(0.0, Vector2(s, s), 0.0, Vector2.ZERO)
	_apply_ui_theme()
	var disc := _disc_texture()
	carcasses.texture = disc
	flashes.texture = disc
	# The agents are the apes of DIT: render each as an 8-bit hominin in its
	# species' own colours (zone-painted coat / skin / accent) instead of a
	# plain disc. Each species gets its own MultiMesh + 4-pose walk atlas; the
	# figure is drawn full-colour, and the [C] overlays (dialect / diet /
	# energy) multiply on top as a tint when cycling away from the default
	# species view. Agents animate: the shader reads per-instance data
	# (phase / moving / facing) written each tick in _refresh_bodies.
	# Texture + material are set BEFORE _make_wrap_clones() so the 8 torus
	# wrap clones inherit them; use_custom_data exposes INSTANCE_CUSTOM to
	# the shader (and is shared by the clones via the same MultiMesh).
	var body_mat := ShaderMaterial.new()
	body_mat.shader = FieldAgentShader
	body_mat.set_shader_parameter("frames", ApeSprites.WALK_FRAME_COUNT)
	_body_mmis.append(bodies)
	for sp in range(1, ApeSprites.SPECIES_COUNT):
		var mmi := MultiMeshInstance2D.new()
		mmi.name = "Bodies%d" % sp
		var mm := MultiMesh.new()
		mm.transform_format = MultiMesh.TRANSFORM_2D
		mm.use_colors = true
		mm.use_custom_data = true
		mm.mesh = bodies.multimesh.mesh
		mmi.multimesh = mm
		add_child(mmi)
		move_child(mmi, bodies.get_index() + sp)
		_body_mmis.append(mmi)
	for sp in ApeSprites.SPECIES_COUNT:
		var mmi := _body_mmis[sp]
		mmi.texture = ApeSprites.build_species_atlas(sp)
		mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		mmi.material = body_mat
	# use_custom_data can only be toggled at instance_count 0; the scene's
	# Bodies ships with a pre-grown buffer, so clear first, enable, then
	# _refresh_bodies re-grows it on the first tick. (The code-created
	# species meshes start empty and already have it enabled.)
	bodies.multimesh.instance_count = 0
	bodies.multimesh.use_custom_data = true
	# Per-module glyph pips are hidden by default: the agent reads as one clean
	# hominin figure, not a cluster of coloured blocks. [M] toggles them back on
	# for debugging. (Module make-up is always in the inspector.)
	module_layers.visible = false
	# streaks keep the raw quad: a solid line reads as a crisp shot streak.
	# Combat reads as energy: additive blending makes flashes and shot streaks
	# glow and bloom where they overlap, so a volley or brawl visibly sparks
	# instead of sitting flat on the terrain. Trade lanes stay normal-blended so
	# they remain the calm, lingering counterpoint to combat.
	var add_mat := CanvasItemMaterial.new()
	add_mat.blend_mode = CanvasItemMaterial.BLEND_MODE_ADD
	flashes.material = add_mat
	streaks.material = add_mat
	_make_wrap_clones()
	# Replay & event camera (E2): snapshot ring + R/U/V modes.
	var replay_manager := preload("res://scripts/replay_manager.gd").new()
	replay_manager.name = "ReplayManager"
	add_child(replay_manager)
	# Showcase director: scripted cinematic timeline for recorded demos.
	# Locks manual camera input (GameConfig.showcase_active) and drives the
	# sim speed, camera, overlays, and title cards from a JSON beat list.
	if OS.has_environment("ANABIOS_SHOWCASE"):
		var director := preload("res://scripts/showcase_director.gd").new()
		director.name = "ShowcaseDirector"
		add_child(director)
	# Evolution panel (E5): trait drift + phylogeny, toggled with [T].
	var evolution_panel := preload("res://scripts/evolution_panel.gd").new()
	evolution_panel.name = "EvolutionPanel"
	evolution_panel.theme = UiTheme.build()
	$UI.add_child(evolution_panel)
	# Dual-inheritance helix: genome × meme strands + coupling rungs, [X].
	var helix_panel := preload("res://scripts/helix_panel.gd").new()
	helix_panel.name = "HelixPanel"
	$UI.add_child(helix_panel)
	# Capture hooks (inert in normal play): ANABIOS_PIN opens the inspector on a
	# representative agent (a click otherwise); ANABIOS_ZOOM frames the camera on
	# that agent so screenshot runs can show the field body art up close.
	if OS.has_environment("ANABIOS_PIN") or OS.has_environment("ANABIOS_ZOOM"):
		var ps: PackedVector2Array = sim.alive_positions()
		if ps.size() > 0:
			var focus: Vector2 = ps[0]
			if OS.has_environment("ANABIOS_ZOOM"):
				var cam := $Camera2D as Camera2D
				var z: float = float(OS.get_environment("ANABIOS_ZOOM"))
				cam.zoom = Vector2(z, z)
				cam.position = focus
			if OS.has_environment("ANABIOS_PIN"):
				var pid: int = int(sim.agent_near(focus, 5.0))
				if pid >= 0:
					inspector.pin(pid)


# The world is a torus but rendering is not: a camera near a seam sees agents
# vanish at the edge. Duplicate every agent layer into the 8 neighboring world
# offsets; each clone shares its source's MultiMesh and texture, so per-frame
# instance updates propagate with zero extra CPU work. Clones live in one
# WrapClones container right after ModuleLayers: it keeps Main's child list
# clean, keeps clones out of ModuleLayers (whose children are indexed by
# module type), and puts glyph clones after body clones in tree order so the
# z_index=0 layers stack at the seams the same way the origin copies do.
func _make_wrap_clones() -> void:
	var world: float = sim.world_size()
	var wrap := Node2D.new()
	wrap.name = "WrapClones"
	add_child(wrap)
	move_child(wrap, module_layers.get_index() + 1)
	var sources: Array[MultiMeshInstance2D] = _body_mmis.duplicate()
	sources.append_array([carcasses, flashes, streaks, trade_routes])
	for src in sources:
		for gy in range(-1, 2):
			for gx in range(-1, 2):
				if gx == 0 and gy == 0:
					continue
				var clone := MultiMeshInstance2D.new()
				clone.multimesh = src.multimesh
				clone.texture = src.texture
				clone.material = src.material  # keep additive glow at the seams
				clone.texture_filter = src.texture_filter  # keep the crisp 8-bit body
				clone.z_index = src.z_index
				clone.position = Vector2(gx * world, gy * world)
				wrap.add_child(clone)
	# Glyph clones follow the [M] toggle so pips appear at the seams too.
	for child in module_layers.get_children():
		for gy in range(-1, 2):
			for gx in range(-1, 2):
				if gx == 0 and gy == 0:
					continue
				var clone := MultiMeshInstance2D.new()
				clone.multimesh = (child as MultiMeshInstance2D).multimesh
				clone.texture = (child as MultiMeshInstance2D).texture
				clone.position = Vector2(gx * world, gy * world)
				clone.visible = module_layers.visible
				wrap.add_child(clone)
				_glyph_clones.append(clone)


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
	# Pause when the window loses focus; user resumes manually. Screenshot
	# runs (ANABIOS_SHOT) and showcase recordings (which must keep stepping
	# hands-free while --write-movie captures) opt out.
	if (
		what == NOTIFICATION_APPLICATION_FOCUS_OUT
		and not OS.has_environment("ANABIOS_SHOT")
		and not GameConfig.showcase_active
	):
		paused = true


func _process(delta: float) -> void:
	if not paused:
		sim.step_n(ticks_per_frame)
	_refresh_bodies(delta)
	_refresh_carcasses()
	_refresh_flashes()
	var world: float = sim.world_size()
	_update_segment_trail(
		_streak_trail,
		streaks.multimesh,
		sim.combat_streaks(),
		sim.combat_streak_colors(),
		STREAK_TTL,
		1.0,
		0.85,
		world
	)
	_update_segment_trail(
		_trade_trail,
		trade_routes.multimesh,
		sim.trade_routes(),
		sim.trade_route_colors(),
		TRADE_TTL,
		0.5,
		0.6,
		world
	)
	var rate: String = "paused" if paused else ("%d×" % ticks_per_frame)
	var total: int = int(sim.total_trades())
	var trades: String = "" if total == 0 else " · %d trades" % total
	hud.text = "tick %d · %d alive · %s%s" % [sim.tick(), sim.alive_count(), rate, trades]


func _refresh_bodies(delta: float = 1.0 / 60.0) -> void:
	var n: int = int(sim.alive_count())
	if n == 0:
		for mmi in _body_mmis:
			mmi.multimesh.visible_instance_count = 0
		if module_layers.visible:
			_clear_module_layers()
		_prev_ids = PackedInt32Array()
		_prev_smooth = PackedVector2Array()
		return

	var positions: PackedVector2Array = sim.alive_positions()
	var ids: PackedInt32Array = sim.alive_ids()
	var sizes: PackedFloat32Array = sim.alive_sizes()
	var rots: PackedFloat32Array = sim.alive_rotations()
	var sp_ids: PackedInt32Array = sim.alive_species_ids()
	var body_colors: PackedColorArray = _body_colors(n)
	var have_rots: bool = rots.size() == n
	var have_sp: bool = sp_ids.size() == n
	var have_ids: bool = ids.size() == n

	# Smoothed render positions: merge-join the current ascending id array
	# against last frame's to find each agent's previous smoothed position,
	# then ease toward the new tick position. Becomes next frame's prev.
	# The approach rate is scaled to the frame delta: at 60 fps this is
	# exactly SMOOTH per frame, at 30 fps twice that — the same glide.
	var k: float = 1.0 - pow(1.0 - SMOOTH, delta * 60.0)
	var smooth: PackedVector2Array = positions
	if have_ids:
		smooth = PackedVector2Array()
		smooth.resize(n)
		var p := 0
		var pn: int = _prev_ids.size()
		for i in n:
			var id: int = ids[i]
			while p < pn and _prev_ids[p] < id:
				p += 1
			var target: Vector2 = positions[i]
			if p < pn and _prev_ids[p] == id:
				var from: Vector2 = _prev_smooth[p]
				if from.distance_squared_to(target) <= SNAP_DIST * SNAP_DIST:
					smooth[i] = from.lerp(target, k)
				else:
					smooth[i] = target
			else:
				smooth[i] = target
		_prev_ids = ids
		_prev_smooth = smooth

	# Bucket alive indices by ape species — one MultiMesh per species.
	var buckets: Array = []
	for sp in ApeSprites.SPECIES_COUNT:
		buckets.append(PackedInt32Array())
	for i in n:
		var sp: int = ApeSprites.ape_for_species(sp_ids[i]) if have_sp else 0
		buckets[sp].append(i)

	for sp in ApeSprites.SPECIES_COUNT:
		var mm: MultiMesh = _body_mmis[sp].multimesh
		var idx: PackedInt32Array = buckets[sp]
		var m: int = idx.size()
		if m > mm.instance_count:
			mm.instance_count = m
		mm.visible_instance_count = m
		for j in m:
			var i: int = idx[j]
			var sz: float = maxf(sizes[i] * BODY_SCALE, BODY_MIN)
			# Upright: the hominin stands, not spins — heading drives the
			# walk shader (moving flag + facing), not the transform rotation.
			var t: Transform2D = Transform2D(0.0, Vector2(sz, sz), 0.0, smooth[i])
			mm.set_instance_transform_2d(j, t)
			mm.set_instance_color(j, body_colors[i])
			# Per-instance animation state for the field_agent shader. Phase
			# is hashed from position (stable enough across alive-index
			# reshuffles); the sim reports heading exactly 0.0 when
			# velocity ≈ 0, which doubles as the idle flag; facing is the
			# heading's x-sign.
			var rot: float = rots[i] if have_rots else 0.0
			var moving: float = 1.0 if rot != 0.0 else 0.0
			var face_left: float = 1.0 if cos(rot) < 0.0 else 0.0
			var phase: float = fposmod(positions[i].x * 0.11 + positions[i].y * 0.07, 1.0)
			mm.set_instance_custom_data(j, Color(phase, moving, face_left, 0.0))

	# Skip the per-tick glyph pass while the pips are hidden ([M] toggles).
	if module_layers.visible:
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
			# Species mode: white — the atlas already carries each ape's own
			# coat/skin colours; the other [C] modes tint over it.
			var out4 := PackedColorArray()
			out4.resize(n)
			out4.fill(Color(1, 1, 1))
			return out4


# A shaded disc, multiplied by each MultiMesh instance color to turn the flat
# body quads into rounded, organic marks. A bright core fading to a darker rim
# gives each organism a subtle spherical shading (full genome color at the
# center, deepened toward the edge) so bodies read as little creatures rather
# than flat dots — and separate cleanly from the terrain at any zoom.
func _disc_texture(res: int = 32) -> ImageTexture:
	var img := Image.create(res, res, false, Image.FORMAT_RGBA8)
	var c := (res - 1) * 0.5
	for y in res:
		for x in res:
			var d := Vector2(x - c, y - c).length() / c  # 0 center .. 1 edge
			var a := clampf(1.0 - smoothstep(0.78, 1.0, d), 0.0, 1.0)
			# Spherical shading: bright at the core, deepening toward the rim.
			var shade := 1.0 - 0.42 * smoothstep(0.0, 0.95, d)
			img.set_pixel(x, y, Color(shade, shade, shade, a))
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


# Segment trails: world-space links kept on screen for a few ticks as fading
# tracers. Combat streaks (attacker→target) are wide, bright, and brief so
# ranged (Spines) volleys read as volleys; trade routes (trader→partner) are
# thin, dim, and long-lived so recurring swaps along species borders
# accumulate into visible lanes. Both tint to the initiator's genome hue.
const STREAK_TTL: int = 8
const TRADE_TTL: int = 24
var _streak_trail: Array = []  # entries: [from: Vector2, to: Vector2, ttl: int, color: Color]
var _trade_trail: Array = []  # entries: [from: Vector2, to: Vector2, ttl: int, color: Color]


# Append this tick's segments to the trail, age it, then draw each survivor
# as a tinted quad stretched from→to. Segments are unwrapped with the
# shortest-path torus delta: a hop across the seam (|delta| near world size)
# is really a short step the other way, and drawing it with the wrapped delta
# lets the wrap clones render its continuation past the world edge.
func _update_segment_trail(
	trail: Array,
	mm: MultiMesh,
	segs: PackedVector2Array,
	cols: PackedColorArray,
	ttl: int,
	width: float,
	max_alpha: float,
	world: float
) -> void:
	for i in segs.size() / 2:
		trail.append([segs[2 * i], segs[2 * i + 1], ttl, cols[i]])
	# Perf: cap the trail at the multimesh budget, dropping the oldest first.
	while trail.size() > mm.instance_count:
		trail.pop_front()
	# Age in place and compact out the expired.
	var write := 0
	for read_i in trail.size():
		var s: Array = trail[read_i]
		s[2] -= 1
		if s[2] > 0:
			trail[write] = s
			write += 1
	trail.resize(write)
	var m: int = mini(trail.size(), mm.instance_count)
	mm.visible_instance_count = m
	for i in m:
		var from: Vector2 = trail[i][0]
		var d: Vector2 = trail[i][1] - from
		if d.x > world * 0.5:
			d.x -= world
		elif d.x < -world * 0.5:
			d.x += world
		if d.y > world * 0.5:
			d.y -= world
		elif d.y < -world * 0.5:
			d.y += world
		var len: float = maxf(d.length(), 0.001)
		var mid: Vector2 = from + d * 0.5
		mm.set_instance_transform_2d(i, Transform2D(d.angle(), Vector2(len, width), 0.0, mid))
		var c: Color = trail[i][3]
		c.a = max_alpha * float(trail[i][2]) / float(ttl)
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
			mm.set_instance_transform_2d(
				i, Transform2D(0.0, Vector2(GLYPH_SIZE, GLYPH_SIZE), 0.0, glyphs[i])
			)
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
	elif event is InputEventKey:
		var k := event as InputEventKey
		if k.pressed and not k.echo and k.keycode == KEY_M:
			module_layers.visible = not module_layers.visible
			for clone in _glyph_clones:
				clone.visible = module_layers.visible
