extends Node2D

const UiTheme = preload("res://scripts/ui_theme.gd")
const Palette = preload("res://scripts/palette.gd")
const ApeSprites = preload("res://scripts/ape_sprites.gd")
const MammalSprites = preload("res://scripts/mammal_sprites.gd")
const FxMath = preload("res://scripts/fx_math.gd")
const FieldAgentShader = preload("res://shaders/field_agent.gdshader")
const EmoteLayer = preload("res://scripts/emote_layer.gd")
const AgentLayer = preload("res://scripts/agent_layer.gd")

# Number of sim ticks to run per rendered frame. Speeds: 1, 4, 16, 64.
@export var ticks_per_frame: int = 1
@export var paused: bool = false

const MODULE_COLORS: PackedColorArray = Palette.MODULE_COLORS
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
# garbage there), and per-species meshes dodge it entirely. Shared with
# AgentLayer (see setup() in _ready): main.gd owns the nodes (scene
# structure, torus wrap clone sources) and their per-frame shader parameter;
# AgentLayer owns the per-frame instance transforms/colours/custom data.
var _body_mmis: Array[MultiMeshInstance2D] = []
var _glyph_clones: Array[MultiMeshInstance2D] = []

# Field-agent body pass (smoothing, gait/facing/action, colour, death
# ghosts): split into its own layer — see agent_layer.gd's header — and,
# since Phase 3 step 1
# (docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md, §6
# Phase 3), culled to the agents inside the camera view.
var _agent_layer: Node2D = null

# Tier 2 effects (embers, firelight, ambient weather, bloom, codex-event
# watcher) live in viewer_effects.gd, created as a child in _ready.
var _effects: Node2D = null
# Footstep tracks + combat/trade segment trails live in trail_layer.gd,
# created as a child in _ready; it owns the Tracks MMI (see tracks_mmi()).
var _trail_layer: Node2D = null
var _climate: CanvasModulate = null
var _settlement_layer: Node2D = null
var _emote_layer: Node2D = null
var _ember_ambient_t: float = 0.0
var _animation_time: float = 0.0


func _ready() -> void:
	var scenario_path: String = GameConfig.scenario_path
	var f = FileAccess.open(scenario_path, FileAccess.READ)
	if f == null:
		push_error("could not open " + scenario_path)
		_fail_to_load("scenario not found:\n" + scenario_path)
		return
	var text = f.get_as_text()
	f.close()
	if not sim.load_scenario_with_seed(text, GameConfig.rng_seed):
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
	# species' own colours instead of a plain disc, one MultiMesh + pose atlas
	# per bucket, with the [C] overlays multiplying on top as a tint. The
	# shader reads per-instance animation state written each tick by
	# AgentLayer.refresh(). Texture + material are set BEFORE _make_wrap_clones() so
	# the 8 torus wrap clones inherit them; use_custom_data exposes
	# INSTANCE_CUSTOM (shared by the clones via the same MultiMesh).
	# Per-bucket gait cadence and rig kind come from the archetype registry
	# (bucket_gait_fps reads ApeSprites.WALK_FPS for the hominin buckets).
	_body_mmis.append(bodies)
	for b in range(1, MammalSprites.BUCKET_COUNT):
		var mmi := MultiMeshInstance2D.new()
		mmi.name = "Bodies%d" % b
		var mm := MultiMesh.new()
		mm.transform_format = MultiMesh.TRANSFORM_2D
		mm.use_colors = true
		mm.use_custom_data = true
		mm.mesh = bodies.multimesh.mesh
		mmi.multimesh = mm
		add_child(mmi)
		move_child(mmi, bodies.get_index() + b)
		_body_mmis.append(mmi)
	for b in MammalSprites.BUCKET_COUNT:
		var mmi := _body_mmis[b]
		mmi.texture = MammalSprites.bucket_atlas(b)
		mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		var sp_mat := ShaderMaterial.new()
		sp_mat.shader = FieldAgentShader
		sp_mat.set_shader_parameter("frames", MammalSprites.POSE_COUNT)
		sp_mat.set_shader_parameter("act_scale", AgentLayer.ACT_SCALE)
		sp_mat.set_shader_parameter("walk_fps", MammalSprites.bucket_gait_fps(b))
		sp_mat.set_shader_parameter("rig_kind", MammalSprites.bucket_rig_kind(b))
		sp_mat.set_shader_parameter("animation_time", 0.0)
		mmi.material = sp_mat
	# use_custom_data can only be toggled at instance_count 0; the scene's
	# Bodies ships with a pre-grown buffer, so clear first, enable, then
	# AgentLayer.refresh() re-grows it on the first tick. (The code-created
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
	# Far-zoom (< 1x) density-dot stand-in for individual agent bodies; hidden
	# whenever the camera is above 1x (AgentLayer draws real bodies there).
	var density := preload("res://scripts/density_layer.gd").new()
	density.name = "DensityLayer"
	add_child(density)
	move_child(density, bodies.get_index() + MammalSprites.BUCKET_COUNT)
	density.setup(sim, $Camera2D as Camera2D)
	# Trail pools (footstep tracks + segment trails), split from this file.
	# Sits where the Tracks MMI used to be added so tree order is unchanged;
	# the scene's Streaks/TradeRoutes multimeshes are passed in so their
	# authored z order and additive material stay in charge of the draw.
	_trail_layer = preload("res://scripts/trail_layer.gd").new()
	_trail_layer.name = "TrailLayer"
	add_child(_trail_layer)
	move_child(_trail_layer, carcasses.get_index())
	_trail_layer.setup(streaks.multimesh, trade_routes.multimesh, bodies.multimesh.mesh, disc)
	# Global climate grade: a subtle warm/cool wash tracking the sim's
	# environmental optimum (affects the world canvas, not the UI layer).
	_climate = CanvasModulate.new()
	_climate.name = "Climate"
	add_child(_climate)
	move_child(_climate, 0)
	# Particle / lighting / weather effects subsystem (split from this file).
	# Created after _climate so it can drive the global climate grade.
	_effects = preload("res://scripts/viewer_effects.gd").new()
	_effects.name = "ViewerEffects"
	add_child(_effects)
	_effects.setup(sim, $Camera2D as Camera2D, _climate, _disc_texture(8), $Biome)
	# Emote pictograms (Zzz / heart / droplet / ! / star) above acting agents.
	_emote_layer = EmoteLayer.new()
	_emote_layer.name = "EmoteLayer"
	add_child(_emote_layer)
	_emote_layer.setup()
	# Field-agent body pass (agent_layer.gd): smoothing, gait/facing/action,
	# colour and the death ghosts, culled to the camera's visible set (§6
	# Phase 3). Must be set up before _make_wrap_clones() below, which sources
	# its death-ghost meshes from it the way it already does _trail_layer's
	# tracks_mmi().
	_agent_layer = AgentLayer.new()
	_agent_layer.name = "AgentLayer"
	add_child(_agent_layer)
	move_child(_agent_layer, carcasses.get_index())
	_agent_layer.setup(sim, _body_mmis, overlay, $Camera2D as Camera2D, _effects, _emote_layer)
	# Settlement layer: hut clusters + farms at the codex settlement sites.
	_settlement_layer = preload("res://scripts/settlement_layer.gd").new()
	_settlement_layer.name = "SettlementLayer"
	add_child(_settlement_layer)
	move_child(_settlement_layer, module_layers.get_index())
	var hub_layer = preload("res://scripts/hub_layer.gd").new()
	hub_layer.name = "HubLayer"
	add_child(hub_layer)
	move_child(hub_layer, module_layers.get_index())
	var caravan_layer = preload("res://scripts/caravan_layer.gd").new()
	caravan_layer.name = "CaravanLayer"
	add_child(caravan_layer)
	move_child(caravan_layer, module_layers.get_index())
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
	# Frame-time / scale readout (Phase 0 instrumentation), toggled with [F3].
	$UI.add_child(preload("res://scripts/perf_readout.gd").new())
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
	_layout_hud()


# Panels pinned to the right edge / the bottom edge of the design viewport.
const DESIGN_VP := Vector2(1280.0, 800.0)
const HUD_RIGHT: PackedStringArray = ["Minimap", "PopulationPanel", "DitPanel", "TechPanel"]
const HUD_BOTTOM: PackedStringArray = ["TimeControls", "LegendPanel", "EventLog", "CodexPanel"]
# Glued to both the right and the bottom edge: the unit card and the meters
# that share its corner.
const HUD_CORNER: PackedStringArray = ["Inspector", "EcoMeters"]
# The right rail, top to bottom. Every panel here is content-sized, so the stack
# is laid out live rather than at fixed offsets (see _layout_rail).
const RAIL_TOP := 10.0
const RAIL_GAP := 10.0
# Authored y of each rail panel, captured before the first layout pass. Each
# acts as a floor, so a panel only moves when the one above actually needs the
# room — the familiar layout is preserved and nothing jumps on a pin/unpin.
var _rail_home: Dictionary = {}


# Stack the right rail under whichever panels above it are actually visible.
# All four used to sit at fixed y, sized for their worst expected content — so
# a tall inspector (a tech-holding ape lists its inventions, and the detail
# label wraps) drew straight down through the species table underneath it, with
# the species panel painting over the inspector's last line. Laying the stack
# out from the live panel heights means any of them can grow without colliding,
# while the per-panel floor keeps the resting layout exactly where it was.
func _layout_rail() -> void:
	var y: float = RAIL_TOP
	var limit: float = DESIGN_VP.y / maxf(0.01, GameConfig.ui_scale)
	# The unit card (or the meters) sits in the bottom-right corner; the rail
	# stops above the slot whether or not a unit is pinned.
	limit = minf(limit, inspector.position.y)
	for n in HUD_RIGHT:
		var c := $UI.get_node_or_null(n) as Control
		if c == null or not c.visible:
			continue
		var home: float = _rail_home.get(n, RAIL_TOP)
		# Never push a panel off the bottom; the last one clamps instead.
		c.position.y = minf(maxf(y, home), maxf(RAIL_TOP, limit - c.size.y - RAIL_GAP))
		y = c.position.y + c.size.y + RAIL_GAP


# The HUD is laid out in absolute pixels against a 1280x800 viewport, and the
# menu's UI-scale option scales the whole CanvasLayer about its origin — so a
# panel at x=1050 lands at 1050*s, not at (screen edge - width). Below 1.0 the
# right and bottom panels floated inward and left a dead band along two edges;
# the layout only ever looked right at exactly 1.0. Re-place each edge group
# inside the logical viewport the scale leaves behind (DESIGN_VP / s) so it
# stays glued to its own edge at any scale. (Scales above 1.0 shrink the logical
# viewport below the design size, which this fixed-size layout cannot fit — the
# menu caps the option at 1.0 for that reason.)
func _layout_hud() -> void:
	for n in HUD_RIGHT:
		var home := $UI.get_node_or_null(n) as Control
		if home != null:
			_rail_home[n] = home.position.y
	var s: float = maxf(0.01, GameConfig.ui_scale)
	if is_equal_approx(s, 1.0):
		return
	var shift: Vector2 = DESIGN_VP / s - DESIGN_VP
	for n in HUD_RIGHT:
		var c := $UI.get_node_or_null(n) as Control
		if c != null:
			c.position.x += shift.x
	for n in HUD_BOTTOM:
		var c2 := $UI.get_node_or_null(n) as Control
		if c2 != null:
			c2.position.y += shift.y
	for n in HUD_CORNER:
		var c3 := $UI.get_node_or_null(n) as Control
		if c3 != null:
			c3.position += shift


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
	var wrap_box := Node2D.new()
	wrap_box.name = "WrapClones"
	add_child(wrap_box)
	move_child(wrap_box, module_layers.get_index() + 1)
	var sources: Array[MultiMeshInstance2D] = _body_mmis.duplicate()
	sources.append_array(_agent_layer.death_mmis())
	sources.append(_agent_layer.shadow_mmi())
	sources.append_array([carcasses, flashes, streaks, trade_routes, _trail_layer.tracks_mmi()])
	for src in sources:
		for gy in range(-1, 2):
			for gx in range(-1, 2):
				if gx == 0 and gy == 0:
					continue
				var clone := MultiMeshInstance2D.new()
				clone.multimesh = src.multimesh
				clone.texture = src.texture
				clone.material = src.material  # keep additive glow at the seams
				clone.modulate = src.modulate  # keep the shadow tint at the seams
				clone.texture_filter = src.texture_filter  # keep the crisp 8-bit body
				clone.z_index = src.z_index
				clone.position = Vector2(gx * world, gy * world)
				wrap_box.add_child(clone)
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
				wrap_box.add_child(clone)
				_glyph_clones.append(clone)


# Give every HUD panel the shared instrument theme, and make the top-left
# readout legible over any terrain with a dark outline.
# Bail out of a scenario that could not be opened without leaving a half-built
# scene behind. _ready() used to just `return` here, which skipped the theme and
# every layer setup: the viewer came up as unstyled stock-Godot controls over an
# empty world with no hint of what went wrong. Show the reason on the HUD, and
# keep the theme so the Menu/Restart buttons still look like the rest of the app.
func _fail_to_load(reason: String) -> void:
	$UI.transform = Transform2D(0.0, Vector2.ONE * GameConfig.ui_scale, 0.0, Vector2.ZERO)
	_apply_ui_theme()
	hud.text = "⚠ " + reason
	hud.add_theme_color_override("font_color", Color(1.0, 0.5, 0.45))
	for n in [
		$UI/Minimap,
		$UI/CodexPanel,
		$UI/EventLog,
		$UI/EcoMeters,
		$UI/LegendPanel,
		$UI/PopulationPanel
	]:
		(n as CanvasItem).visible = false
	set_process(false)


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
	_layout_rail()
	_animation_time = FxMath.advance_animation_time(_animation_time, delta, paused)
	for mmi in _body_mmis:
		var body_mat := mmi.material as ShaderMaterial
		if body_mat != null:
			body_mat.set_shader_parameter("animation_time", _animation_time)
	if not paused:
		sim.step_n(ticks_per_frame)
	# Fetch this tick's segments once: the trail pass draws them and the body
	# pass uses their attacker/trader endpoints as fight/trade hotspots.
	var streak_segs: PackedVector2Array = sim.combat_streaks()
	var streak_cols: PackedColorArray = sim.combat_streak_colors()
	var trade_segs: PackedVector2Array = sim.trade_routes()
	var trade_cols: PackedColorArray = sim.trade_route_colors()
	var fight_pts: PackedVector2Array = _hotspots(streak_segs, 64)
	var trade_pts: PackedVector2Array = _hotspots(trade_segs, 64)
	_agent_layer.refresh(delta, _animation_time, ticks_per_frame, fight_pts, trade_pts)
	# Module-glyph pips are a separate layer main.gd still owns directly (not
	# moved to AgentLayer): mirror the gate AgentLayer.refresh() used to apply
	# internally when the population hits zero.
	if module_layers.visible:
		if sim.alive_count() == 0:
			_clear_module_layers()
		else:
			_refresh_module_layers()
	_refresh_carcasses()
	var flash_count := _refresh_flashes()
	if flash_count > 0:
		($Camera2D as Camera2D).add_trauma(minf(0.03, 0.0025 * flash_count))
	var world: float = sim.world_size()
	var moving_sample: PackedVector2Array = _agent_layer.moving_sample()
	_trail_layer.update(
		delta, moving_sample, paused, streak_segs, streak_cols, trade_segs, trade_cols, world
	)
	_effects.update(delta, moving_sample, paused)
	# Hearth smoke: settled sites breathe an occasional ember wisp.
	_ember_ambient_t += delta
	if _ember_ambient_t > 1.6:
		_ember_ambient_t = 0.0
		if _settlement_layer != null and _settlement_layer.has_sites():
			_effects.spawn_embers(_settlement_layer.random_site_pos())
	var rate: String = "paused" if paused else ("%d×" % ticks_per_frame)
	var total: int = int(sim.total_trades())
	var trades: String = "" if total == 0 else " · %d trades" % total
	hud.text = "tick %d · %d alive · %s%s" % [sim.tick(), sim.alive_count(), rate, trades]


# Attacker/trader endpoints of this tick's segments (every second vector), so
# the body pass can flag nearby agents with the fight/trade action poses.
func _hotspots(segs: PackedVector2Array, cap: int) -> PackedVector2Array:
	var out := PackedVector2Array()
	var m := mini(int(segs.size() / 2.0), cap)
	for i in m:
		out.append(segs[2 * i])
	return out


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
		# Bone, not the old cold near-white: at 0.55 alpha a pale blue-grey disc
		# was the brightest thing on a green field, so every carcass pulled the
		# eye like a UI marker. Warm and dim reads as remains on the ground.
		mm.set_instance_color(i, Color(0.78, 0.74, 0.63, 0.42))


func _refresh_flashes() -> int:
	var pts: PackedVector2Array = sim.combat_flashes()
	var mm: MultiMesh = flashes.multimesh
	var m: int = pts.size()
	if m > mm.instance_count:
		mm.instance_count = m
	mm.visible_instance_count = m
	for i in m:
		mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(6.0, 6.0), 0.0, pts[i]))
		mm.set_instance_color(i, Color(1.0, 0.92, 0.45, 0.95))
	return m


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
		if k.pressed and not k.echo and k.keycode == KEY_F11:
			# Keep windowed mode as the default, but make fullscreen a reversible
			# presentation choice for demos and screenshots.
			var mode := DisplayServer.window_get_mode()
			DisplayServer.window_set_mode(
				(
					DisplayServer.WINDOW_MODE_WINDOWED
					if mode == DisplayServer.WINDOW_MODE_FULLSCREEN
					else DisplayServer.WINDOW_MODE_FULLSCREEN
				)
			)
		elif k.pressed and not k.echo and k.keycode == KEY_M:
			module_layers.visible = not module_layers.visible
			for clone in _glyph_clones:
				clone.visible = module_layers.visible
