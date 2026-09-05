extends Node2D

# Presentation-only effects subsystem for the viewer, split out of main.gd to
# keep that scene controller under the gdlint file-length budget. Owns the
# pooled particle/light emitters (embers, dust, flickering fire lights), the
# screen bloom, the ambient weather (motes / tundra snow), the global climate
# grade, and the codex-event watcher that fires them. main.gd creates one of
# these as a child, calls setup() once, then drives it per frame via update();
# it stays a pure sink of sim/camera state, holding no simulation logic.

const FIRE_LIGHT_TTL: float = 4.0

const EventFx = preload("res://scripts/event_fx.gd")
const FxRing = preload("res://scripts/fx_ring.gd")

var _sim = null
var _cam: Camera2D = null
var _climate: CanvasModulate = null
var _disc: Texture2D = null

var _embers: Array[GPUParticles2D] = []
var _ember_idx: int = 0
var _fire_lights: Array = []  # entries: [light: PointLight2D, t: float]
var _fire_light_idx: int = 0
var _motes: GPUParticles2D = null
var _snow: GPUParticles2D = null
var _weather_t: float = 999.0
var _dust: Array[GPUParticles2D] = []
var _dust_idx: int = 0
var _rings: Array = []
var _ring_idx: int = 0
var _sparks: Array[GPUParticles2D] = []
var _spark_idx: int = 0
var _event_cursor: int = 0


# Wire up the sim/camera/climate references and the shared particle disc, then
# build every pool. Called once from main._ready after the climate node exists.
func setup(sim, cam: Camera2D, climate: CanvasModulate, disc: Texture2D) -> void:
	_sim = sim
	_cam = cam
	_climate = climate
	_disc = disc
	_make_bloom()
	_make_ember_pool()
	_make_dust_pool()
	_make_fire_light_pool()
	_make_ring_pool()
	_make_spark_pool()
	_make_weather()


# Per-frame tick, driven from main._process in the original call order. The
# walker sample and pause flag live in main (written during the body pass), so
# they are passed in rather than read back.
func update(delta: float, moving_sample: PackedVector2Array, is_paused: bool) -> void:
	watch_events()
	update_fire_lights(delta)
	update_rings(delta)
	update_weather(delta)
	update_dust(moving_sample, is_paused)


# Screen glow so the additive combat layers (flashes, streaks) bloom where
# they overlap; the threshold sits above the terrain/UI so only overbright
# additive stacks and event lights halo.
func _make_bloom() -> void:
	var env := Environment.new()
	env.glow_enabled = true
	env.glow_blend_mode = Environment.GLOW_BLEND_MODE_SCREEN
	env.glow_intensity = 0.4
	env.glow_strength = 1.0
	env.glow_bloom = 0.05
	env.glow_hdr_threshold = 0.9
	var we := WorldEnvironment.new()
	we.name = "BloomEnv"
	we.environment = env
	add_child(we)


# Pooled one-shot ember bursts for "fire-kind" codex events (discoveries,
# tools, settlements, markets): a spray of rising orange motes that fades as
# it cools. Round-robin so a burst never cuts an earlier one short.
func _make_ember_pool() -> void:
	for i in 6:
		var p := GPUParticles2D.new()
		p.name = "Embers%d" % i
		p.amount = 22
		p.lifetime = 1.3
		p.one_shot = true
		p.explosiveness = 0.85
		p.z_index = 6
		p.visibility_rect = Rect2(-200, -200, 400, 400)
		p.texture = _disc
		var m := ParticleProcessMaterial.new()
		m.direction = Vector3(0, -1, 0)
		m.spread = 38.0
		m.initial_velocity_min = 16.0
		m.initial_velocity_max = 40.0
		m.gravity = Vector3(0, -16, 0)
		m.damping_min = 5.0
		m.damping_max = 12.0
		m.scale_min = 0.7
		m.scale_max = 1.5
		var grad := Gradient.new()
		grad.set_color(0, Color(1.0, 0.72, 0.30))
		grad.set_color(1, Color(1.0, 0.30, 0.08, 0.0))
		var gt := GradientTexture1D.new()
		gt.gradient = grad
		m.color_ramp = gt
		p.process_material = m
		var add := CanvasItemMaterial.new()
		add.blend_mode = CanvasItemMaterial.BLEND_MODE_ADD
		p.material = add
		add_child(p)
		_embers.append(p)


func spawn_embers(pos: Vector2) -> void:
	if _embers.is_empty():
		return
	var p := _embers[_ember_idx]
	_ember_idx = (_ember_idx + 1) % _embers.size()
	p.position = pos
	p.restart()


# Pooled dust puffs: at close zoom, walkers kick up a faint tan cloud so
# movement has a sense of ground contact. Normal-blended (dust occludes,
# it doesn't glow) and drab next to the embers on purpose.
func _make_dust_pool() -> void:
	for i in 3:
		var p := GPUParticles2D.new()
		p.name = "Dust%d" % i
		p.amount = 8
		p.lifetime = 0.7
		p.one_shot = true
		p.explosiveness = 0.7
		p.z_index = 6
		p.visibility_rect = Rect2(-100, -100, 200, 200)
		p.texture = _disc
		var m := ParticleProcessMaterial.new()
		m.direction = Vector3(0, -1, 0)
		m.spread = 160.0
		m.initial_velocity_min = 5.0
		m.initial_velocity_max = 13.0
		m.gravity = Vector3(0, 9, 0)
		m.damping_min = 8.0
		m.damping_max = 16.0
		m.scale_min = 1.2
		m.scale_max = 2.2
		var grad := Gradient.new()
		grad.set_color(0, Color(0.72, 0.66, 0.52, 0.30))
		grad.set_color(1, Color(0.72, 0.66, 0.52, 0.0))
		var gt := GradientTexture1D.new()
		gt.gradient = grad
		m.color_ramp = gt
		p.process_material = m
		add_child(p)
		_dust.append(p)


func update_dust(moving_sample: PackedVector2Array, is_paused: bool) -> void:
	if _dust.is_empty() or moving_sample.is_empty() or is_paused:
		return
	if _cam.zoom.x < 2.5:
		return
	var p := _dust[_dust_idx]
	_dust_idx = (_dust_idx + 1) % _dust.size()
	p.position = moving_sample[randi() % moving_sample.size()] + Vector2(0, 2.0)
	p.restart()


# One dust puff at `pos` (births), sharing the walkers' pool and zoom gate.
func spawn_dust(pos: Vector2) -> void:
	if _dust.is_empty() or _cam.zoom.x < 2.5:
		return
	var p := _dust[_dust_idx]
	_dust_idx = (_dust_idx + 1) % _dust.size()
	p.position = pos + Vector2(0, 2.0)
	p.restart()


# Pooled expanding ring pulses marking codex events in the world; hue comes
# from the event's spec so the world echoes the codex timeline's colors.
func _make_ring_pool() -> void:
	for i in 10:
		var r: Node2D = FxRing.new()
		r.name = "FxRing%d" % i
		r.z_index = 6
		r.visible = false
		add_child(r)
		_rings.append(r)


func spawn_ring(pos: Vector2, color: Color, dur: float, radius: float) -> void:
	if _rings.is_empty():
		return
	var r: Node2D = _rings[_ring_idx]
	_ring_idx = (_ring_idx + 1) % _rings.size()
	r.start(pos, color, dur, radius)


func update_rings(delta: float) -> void:
	for r in _rings:
		if r.active:
			r.step(delta)


func rings() -> Array:
	return _rings


# Pooled tinted spark bursts: the embers' shape with a neutral ramp, tinted
# per spawn via modulate so one pool serves every event hue.
func _make_spark_pool() -> void:
	for i in 6:
		var p := GPUParticles2D.new()
		p.name = "Sparks%d" % i
		p.amount = 16
		p.lifetime = 1.1
		p.one_shot = true
		p.explosiveness = 0.85
		p.z_index = 6
		p.visibility_rect = Rect2(-200, -200, 400, 400)
		p.texture = _disc
		var m := ParticleProcessMaterial.new()
		m.direction = Vector3(0, -1, 0)
		m.spread = 48.0
		m.initial_velocity_min = 12.0
		m.initial_velocity_max = 32.0
		m.gravity = Vector3(0, -14, 0)
		m.damping_min = 5.0
		m.damping_max = 12.0
		m.scale_min = 0.6
		m.scale_max = 1.3
		var grad := Gradient.new()
		grad.set_color(0, Color(1.0, 1.0, 1.0))
		grad.set_color(1, Color(1.0, 1.0, 1.0, 0.0))
		var gt := GradientTexture1D.new()
		gt.gradient = grad
		m.color_ramp = gt
		p.process_material = m
		var add := CanvasItemMaterial.new()
		add.blend_mode = CanvasItemMaterial.BLEND_MODE_ADD
		p.material = add
		add_child(p)
		_sparks.append(p)


func spawn_motes(pos: Vector2, color: Color) -> void:
	if _sparks.is_empty():
		return
	var p := _sparks[_spark_idx]
	_spark_idx = (_spark_idx + 1) % _sparks.size()
	p.position = pos
	p.modulate = color
	p.restart()


# Pooled flickering fire lights: a warm PointLight2D (additive, so it can only
# brighten) that breathes for a few seconds where a fire-kind event fired.
func _make_fire_light_pool() -> void:
	var tex := _radial_texture(64)
	for i in 6:
		var l := PointLight2D.new()
		l.name = "FireLight%d" % i
		l.texture = tex
		l.texture_scale = 4.0
		l.color = Color(1.0, 0.58, 0.22)
		l.blend_mode = Light2D.BLEND_MODE_ADD
		l.energy = 0.0
		l.enabled = false
		add_child(l)
		_fire_lights.append([l, FIRE_LIGHT_TTL])


func _spawn_fire_light(pos: Vector2) -> void:
	if _fire_lights.is_empty():
		return
	var e: Array = _fire_lights[_fire_light_idx]
	_fire_light_idx = (_fire_light_idx + 1) % _fire_lights.size()
	(e[0] as PointLight2D).position = pos
	(e[0] as PointLight2D).enabled = true
	e[1] = 0.0


func update_fire_lights(delta: float) -> void:
	for e in _fire_lights:
		var l := e[0] as PointLight2D
		if not l.enabled:
			continue
		e[1] += delta
		var t: float = e[1]
		if t >= FIRE_LIGHT_TTL:
			l.enabled = false
			l.energy = 0.0
			continue
		var fade := 1.0 - t / FIRE_LIGHT_TTL
		l.energy = (0.85 + 0.3 * sin(t * 23.0) * sin(t * 7.3)) * fade


# Radial falloff texture for the fire lights (bright core, soft edge).
func _radial_texture(res: int) -> ImageTexture:
	var img := Image.create(res, res, false, Image.FORMAT_RGBA8)
	var c := (res - 1) * 0.5
	for y in res:
		for x in res:
			var d := Vector2(x - c, y - c).length() / c
			var a := clampf(1.0 - d * d, 0.0, 1.0)
			img.set_pixel(x, y, Color(a, a, a, 1.0))
	return ImageTexture.create_from_image(img)


# Ambient weather: faint drifting motes everywhere (depth cue), snow when the
# camera sits over tundra. Both are children of the camera so the emitter
# follows the view; the emission box is resized to the visible world rect.
func _make_weather() -> void:
	_motes = _make_ambient_particles(
		"Motes", 70, Color(1.0, 0.95, 0.8, 0.10), Vector3(6, 1, 0), 7.0
	)
	_cam.add_child(_motes)
	_snow = _make_ambient_particles(
		"Snow", 160, Color(0.95, 0.97, 1.0, 0.55), Vector3(2, 14, 0), 5.0
	)
	_snow.emitting = false
	_cam.add_child(_snow)


func _make_ambient_particles(
	pname: String, amount: int, col: Color, vel: Vector3, lifetime: float
) -> GPUParticles2D:
	var p := GPUParticles2D.new()
	p.name = pname
	p.amount = amount
	p.lifetime = lifetime
	p.z_index = 7
	p.visibility_rect = Rect2(-4000, -4000, 8000, 8000)
	p.texture = _disc
	var m := ParticleProcessMaterial.new()
	m.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_BOX
	m.emission_box_extents = Vector3(600, 400, 1)
	m.direction = Vector3(vel.x, vel.y, 0).normalized()
	m.spread = 180.0
	m.initial_velocity_min = vel.length() * 0.5
	m.initial_velocity_max = vel.length() * 1.5
	m.gravity = Vector3.ZERO
	m.scale_min = 0.5
	m.scale_max = 1.1
	m.color = col
	p.process_material = m
	return p


# Keep the camera-child emitters covering the visible world rect, and poll the
# biome under the camera to toggle snow over tundra (base colour 0.62/0.66/0.62
# — the only pale, low-chroma land terrain; desert/savanna fail the b test).
func update_weather(delta: float) -> void:
	var vp: Vector2 = get_viewport_rect().size / _cam.zoom.x * 0.55
	for p in [_motes, _snow]:
		if p != null:
			(p.process_material as ParticleProcessMaterial).emission_box_extents = Vector3(
				vp.x, vp.y, 1
			)
	_weather_t += delta
	if _weather_t < 0.5 or _snow == null:
		return
	_weather_t = 0.0
	# Climate grade: the sim's global optimum washes the world subtly warm
	# (hothouse) or cool (ice age); -1 means the environment system is off.
	var opt: float = _sim.env_optimum()
	if _climate != null:
		var target := Color(1, 1, 1)
		if opt >= 0.0:
			target = Color(0.94, 0.97, 1.05).lerp(Color(1.05, 1.0, 0.92), clampf(opt, 0.0, 1.0))
		_climate.color = _climate.color.lerp(target, 0.25)
	var res := int(_sim.biome_resolution())
	var colors: PackedColorArray = _sim.biome_colors()
	var world: float = _sim.world_size()
	if res <= 0 or colors.size() != res * res or world <= 0.0:
		return
	var cx := clampi(int(fposmod(_cam.position.x, world) / world * res), 0, res - 1)
	var cy := clampi(int(fposmod(_cam.position.y, world) / world * res), 0, res - 1)
	var c: Color = colors[cy * res + cx]
	var tundra := c.r > 0.5 and c.g > 0.55 and c.b > 0.5 and absf(c.r - c.b) < 0.08
	# Tundra always snows; a deep global cold snap snows everywhere.
	_snow.emitting = tundra or (opt >= 0.0 and opt < 0.2)


# Codex-driven effects, dispatched through event_fx's spec table. The cursor
# mirrors codex_panel's so a reload resets.
func watch_events() -> void:
	var count: int = int(_sim.codex_event_count())
	if count < _event_cursor:
		_event_cursor = 0
	if count == _event_cursor:
		return
	for ev in _sim.codex_events_since(_event_cursor).slice(-10):
		# (slice caps effects at the batch tail: a flood from scenario load or
		# a tick jump is history, not news — don't light up ancient events.)
		apply_event_fx(int(ev["type"]), ev.get("loc", Vector2.ZERO))
	_event_cursor = count


# Apply one event's effects. Positional kinds need a real location (ZERO is
# the sim's "no location" sentinel); trauma is global and always lands.
func apply_event_fx(event_type: int, loc: Vector2) -> void:
	for s in EventFx.spec(event_type):
		match s["kind"]:
			"fire":
				if loc != Vector2.ZERO:
					spawn_embers(loc)
					_spawn_fire_light(loc)
			"ring":
				if loc != Vector2.ZERO:
					spawn_ring(loc, s["color"], s["dur"], s["radius"])
			"motes":
				if loc != Vector2.ZERO:
					spawn_motes(loc, s["color"])
			"trauma":
				_cam.add_trauma(s["amount"])
