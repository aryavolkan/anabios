extends Camera2D

const ZOOM_STEP: float = 1.2
const ZOOM_MIN: float = 0.25
const ZOOM_MAX: float = 8.0
const PAN_SPEED_KEYS: float = 600.0
const ZOOM_DAMP: float = 12.0
const PAN_INERTIA_DAMP: float = 5.0
const TRAUMA_DECAY: float = 1.8
const TRAUMA_MAX: float = 0.6
const SHAKE_MAX: float = 10.0

var _dragging: bool = false
var _target_zoom: float = 1.0
var _zoom_easing: bool = false
var _pan_vel: Vector2 = Vector2.ZERO
var _trauma: float = 0.0
var _shake_t: float = 0.0

func _ready() -> void:
	_fit_to_world()
	_target_zoom = zoom.x

# Frame the whole world: fill the viewport (larger ratio wins, so there are no
# empty gutters) and center on the world's midpoint.
func _fit_to_world() -> void:
	var sim = get_node_or_null("/root/Main/Simulation")
	if sim == null:
		return
	var world: float = float(sim.world_size())
	if world <= 0.0:
		return
	var vp: Vector2 = get_viewport_rect().size
	var z: float = maxf(vp.x / world, vp.y / world)
	z = clampf(z, ZOOM_MIN, ZOOM_MAX)
	zoom = Vector2(z, z)
	_target_zoom = z
	position = Vector2(world * 0.5, world * 0.5)

# Frame the living cluster (the bounding box of all agents) instead of the empty
# whole world, so a run opens on the action — with the agents now little
# hominins, that's where the 8-bit bodies actually read. [F] still resets to the
# full world for the overview. Called from Main._ready after the scenario loads
# (the sim has no agents yet at this node's own _ready).
func fit_to_agents() -> void:
	var sim = get_node_or_null("/root/Main/Simulation")
	if sim == null:
		return
	var ps: PackedVector2Array = sim.alive_positions()
	if ps.size() == 0:
		_fit_to_world()
		return
	# Centre on the agent centroid — being count-weighted it lands in the densest
	# region (e.g. the founding continent), not in empty sea between clusters.
	var c: Vector2 = Vector2.ZERO
	for p in ps:
		c += p
	c /= float(ps.size())
	# Frame a slice of the world (not the full extent) so individual hominins are
	# legible on boot; [F] resets to the whole-world overview.
	var world: float = float(sim.world_size())
	var span: float = maxf(world * 0.35, 1.0)
	var vp: Vector2 = get_viewport_rect().size
	var z: float = clampf(vp.y / span, ZOOM_MIN, ZOOM_MAX)
	zoom = Vector2(z, z)
	_target_zoom = z
	position = c

# Combat and other high-energy events feed trauma here; _process decays it and
# turns the remainder into a screen-space shake via `offset`, so the camera's
# position/zoom (and anything scripting them, like the showcase director) are
# never disturbed.
func add_trauma(amount: float) -> void:
	_trauma = clampf(_trauma + amount, 0.0, TRAUMA_MAX)

func _input(event: InputEvent) -> void:
	if GameConfig.showcase_active:
		return
	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		# Wheel zoom eases toward the target and stays anchored on the cursor:
		# the world point under the mouse doesn't move while the zoom settles.
		# Easing is armed only by wheel input — external zoom writes (screenshot
		# harness, showcase director) take effect immediately, never fought.
		if mb.button_index == MOUSE_BUTTON_WHEEL_UP and mb.pressed:
			if not _zoom_easing:
				_target_zoom = zoom.x
				_zoom_easing = true
			_target_zoom = clampf(_target_zoom * ZOOM_STEP, ZOOM_MIN, ZOOM_MAX)
		elif mb.button_index == MOUSE_BUTTON_WHEEL_DOWN and mb.pressed:
			if not _zoom_easing:
				_target_zoom = zoom.x
				_zoom_easing = true
			_target_zoom = clampf(_target_zoom / ZOOM_STEP, ZOOM_MIN, ZOOM_MAX)
		elif mb.button_index == MOUSE_BUTTON_MIDDLE:
			_dragging = mb.pressed
			if mb.pressed:
				_pan_vel = Vector2.ZERO
	elif event is InputEventMouseMotion and _dragging:
		var mm := event as InputEventMouseMotion
		position -= mm.relative / zoom.x
		_pan_vel = -mm.relative / zoom.x * 60.0

# Discrete key toggles go through _unhandled_key_input (matches overlay_manager
# [G]/[C] and legend [H]), so a focused text field could consume them first.
func _unhandled_key_input(event: InputEvent) -> void:
	if GameConfig.showcase_active:
		return
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_F:
		_fit_to_world()

func _process(delta: float) -> void:
	if not GameConfig.showcase_active:
		var v := Vector2.ZERO
		if Input.is_key_pressed(KEY_W) or Input.is_key_pressed(KEY_UP):    v.y -= 1
		if Input.is_key_pressed(KEY_S) or Input.is_key_pressed(KEY_DOWN):  v.y += 1
		if Input.is_key_pressed(KEY_A) or Input.is_key_pressed(KEY_LEFT):  v.x -= 1
		if Input.is_key_pressed(KEY_D) or Input.is_key_pressed(KEY_RIGHT): v.x += 1
		if v != Vector2.ZERO:
			position += v.normalized() * (PAN_SPEED_KEYS * delta) / zoom.x
			_pan_vel = Vector2.ZERO
		# Middle-drag fling: after release the pan glides to a stop.
		elif not _dragging and _pan_vel != Vector2.ZERO:
			position += _pan_vel * delta
			_pan_vel = _pan_vel.lerp(Vector2.ZERO, 1.0 - exp(-PAN_INERTIA_DAMP * delta))
			if _pan_vel.length_squared() < 1.0:
				_pan_vel = Vector2.ZERO
		# Wheel zoom eases toward the target and stays anchored on the cursor:
		# the world point under the mouse doesn't move while the zoom settles.
		if _zoom_easing and not is_equal_approx(zoom.x, _target_zoom):
			var anchor := get_global_mouse_position()
			var nz := lerpf(zoom.x, _target_zoom, 1.0 - exp(-ZOOM_DAMP * delta))
			if absf(nz - _target_zoom) < 0.001:
				nz = _target_zoom
				_zoom_easing = false
			zoom = Vector2(nz, nz)
			position += anchor - get_global_mouse_position()
		elif _zoom_easing:
			_zoom_easing = false
	# Trauma shake runs even under the showcase director: offset is pure
	# presentation and leaves the scripted camera path untouched.
	if _trauma > 0.0:
		_trauma = maxf(0.0, _trauma - TRAUMA_DECAY * delta)
		_shake_t += delta * 28.0
		var mag := SHAKE_MAX * _trauma * _trauma
		offset = Vector2(
			sin(_shake_t * 1.3) * 0.6 + sin(_shake_t * 3.7) * 0.4,
			cos(_shake_t * 1.7) * 0.6 + cos(_shake_t * 4.3) * 0.4) * mag
	elif offset != Vector2.ZERO:
		offset = Vector2.ZERO
