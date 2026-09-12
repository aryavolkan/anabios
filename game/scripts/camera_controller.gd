extends Camera2D

# Pixel-perfect pipeline (D4, docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md
# §4). Three project.godot [rendering] settings this camera's stepped zoom
# depends on (project.godot cannot hold comments, so they're documented here):
#   2d/snap/snap_2d_transforms_to_pixel=true   - node positions snap to whole
#       pixels, so an integer-zoomed world never shimmers.
#   2d/snap/snap_2d_vertices_to_pixel=false    - vertices stay unsnapped; the
#       MultiMesh prop/agent quads would distort if their corners snapped
#       independently of their transform.
#   textures/canvas_textures/default_texture_filter=0 (nearest) - crisp texels
#       everywhere that doesn't opt into linear filtering explicitly
#       (biome_renderer.gd's ground sprite and 3×3 tiles still request LINEAR).
#
# Wheel zoom now snaps to a fixed step table instead of an endless
# multiplicative ramp: steps >= 1.0 are integer texel multiples (the D4
# contract — 1 texel = 0.5 world unit at 1x, so 2x/3x/4x/6x/8x are exact
# integer upscales); steps < 1.0 are the overview range for worlds wider than
# the viewport can frame at 1x. The table and the two pure step functions
# live in zoom_steps.gd (not here) so test_camera_steps.gd can preload and
# exercise them headless: this script extends Camera2D and reads the
# GameConfig autoload below, which does not compile when preloaded from a
# `-s` test script (see test_event_fx.gd's StubCam note) — ZoomSteps has
# neither problem. ZOOM_STEPS and next_step/nearest_step_at_most are
# re-exported here so callers keep using CameraController.* as before.
const ZoomSteps = preload("res://scripts/zoom_steps.gd")
const ZOOM_STEPS: PackedFloat32Array = ZoomSteps.ZOOM_STEPS
const ZOOM_MIN: float = 0.0625  # ZOOM_STEPS[0]
const ZOOM_MAX: float = 8.0  # ZOOM_STEPS[ZOOM_STEPS.size() - 1]
const PAN_SPEED_KEYS: float = 600.0
const ZOOM_DAMP: float = 12.0
const PAN_INERTIA_DAMP: float = 5.0
# How fast a held key ramps the pan up to PAN_SPEED_KEYS. Brisk enough not to
# feel laggy, slow enough that the start of a pan is not a jolt.
const PAN_ACCEL: float = 12.0

var _dragging: bool = false
var _target_zoom: float = 1.0
var _zoom_easing: bool = false
var _pan_vel: Vector2 = Vector2.ZERO


func _ready() -> void:
	_fit_to_world()
	_target_zoom = zoom.x


# The step relative to z on the ZOOM_STEPS table: dir +1 is the smallest step
# strictly greater than z, dir -1 the largest strictly smaller, both clamped to
# the table ends. Applied to the CURRENT (possibly off-table) zoom on every
# wheel event, so a zoom left mid-ease by a fast scroll still lands on the
# table on the very next step. (See zoom_steps.gd for the implementation.)
static func next_step(z: float, dir: int) -> float:
	return ZoomSteps.next_step(z, dir)


# The largest step <= z, or the smallest step if z undercuts the whole table.
# Used to open a fresh fit (world or agent-cluster) on a crisp step rather
# than an arbitrary continuous zoom. (See zoom_steps.gd for the implementation.)
static func nearest_step_at_most(z: float) -> float:
	return ZoomSteps.nearest_step_at_most(z)


# Index of the step the current zoom sits on exactly, or -1 when an external
# writer (showcase_director.gd, debug_capture.gd, main.gd's ANABIOS_ZOOM)
# has set an off-table zoom — those writes are honoured as-is and never
# snapped, so the ground layer can tell "on a step" from "in between".
func zoom_step_index() -> int:
	for i in range(ZOOM_STEPS.size()):
		if is_equal_approx(zoom.x, ZOOM_STEPS[i]):
			return i
	return -1


# Below 1x the world no longer fits at an integer texel multiple; the ground
# layer switches to the whole-world overview mip in that range (D4).
func is_overview() -> bool:
	return zoom.x < 1.0


# Frame the whole world: fill the viewport (larger ratio wins, so there are no
# empty gutters) and center on the world's midpoint.
func _fit_to_world() -> void:
	var sim = get_node_or_null("../Simulation")
	if sim == null:
		return
	var world: float = float(sim.world_size())
	if world <= 0.0:
		return
	var vp: Vector2 = get_viewport_rect().size
	var z: float = maxf(vp.x / world, vp.y / world)
	z = nearest_step_at_most(clampf(z, ZOOM_MIN, ZOOM_MAX))
	zoom = Vector2(z, z)
	_target_zoom = z
	position = Vector2(world * 0.5, world * 0.5)


# Frame the living cluster (the bounding box of all agents) instead of the empty
# whole world, so a run opens on the action — with the agents now little
# hominins, that's where the 8-bit bodies actually read. [F] still resets to the
# full world for the overview. Called from Main._ready after the scenario loads
# (the sim has no agents yet at this node's own _ready).
func fit_to_agents() -> void:
	var sim = get_node_or_null("../Simulation")
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
	var z: float = nearest_step_at_most(clampf(vp.y / span, ZOOM_MIN, ZOOM_MAX))
	zoom = Vector2(z, z)
	_target_zoom = z
	position = c


# Combat and other high-energy events feed trauma here. Camera shake is
# deliberately DISABLED: chronic events (MassFright fires every tick in
# panic-heavy scenarios) kept the screen juddering near-constantly at speed,
# so the trauma sink is inert. The event-side doses (event_fx.gd, main.gd's
# flash rumble) still flow in as a record of event intensity, and this stays
# the single place to reinstate shake if it's ever wanted again.
func add_trauma(_amount: float) -> void:
	pass


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
			_target_zoom = next_step(_target_zoom, 1)
		elif mb.button_index == MOUSE_BUTTON_WHEEL_DOWN and mb.pressed:
			if not _zoom_easing:
				_target_zoom = zoom.x
				_zoom_easing = true
			_target_zoom = next_step(_target_zoom, -1)
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
		if Input.is_key_pressed(KEY_W) or Input.is_key_pressed(KEY_UP):
			v.y -= 1
		if Input.is_key_pressed(KEY_S) or Input.is_key_pressed(KEY_DOWN):
			v.y += 1
		if Input.is_key_pressed(KEY_A) or Input.is_key_pressed(KEY_LEFT):
			v.x -= 1
		if Input.is_key_pressed(KEY_D) or Input.is_key_pressed(KEY_RIGHT):
			v.x += 1
		if v != Vector2.ZERO:
			# Keys drive the same velocity a drag does, ramping up to speed
			# rather than starting at full tilt — and because the velocity
			# survives the key release it falls into the glide below instead
			# of dead-stopping. Both pan inputs now come to rest alike.
			var target := v.normalized() * (PAN_SPEED_KEYS / zoom.x)
			_pan_vel = _pan_vel.lerp(target, 1.0 - exp(-PAN_ACCEL * delta))
			position += _pan_vel * delta
		# Key release and middle-drag fling both glide to a stop from here.
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
