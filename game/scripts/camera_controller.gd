extends Camera2D

const ZOOM_STEP: float = 1.2
const ZOOM_MIN: float = 0.25
const ZOOM_MAX: float = 8.0
const PAN_SPEED_KEYS: float = 600.0

var _dragging: bool = false

func _ready() -> void:
	_fit_to_world()

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
	position = c

func _input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		if mb.button_index == MOUSE_BUTTON_WHEEL_UP and mb.pressed:
			zoom = (zoom * ZOOM_STEP).clamp(Vector2(ZOOM_MIN, ZOOM_MIN), Vector2(ZOOM_MAX, ZOOM_MAX))
		elif mb.button_index == MOUSE_BUTTON_WHEEL_DOWN and mb.pressed:
			zoom = (zoom / ZOOM_STEP).clamp(Vector2(ZOOM_MIN, ZOOM_MIN), Vector2(ZOOM_MAX, ZOOM_MAX))
		elif mb.button_index == MOUSE_BUTTON_MIDDLE:
			_dragging = mb.pressed
	elif event is InputEventMouseMotion and _dragging:
		var mm := event as InputEventMouseMotion
		position -= mm.relative / zoom.x

# Discrete key toggles go through _unhandled_key_input (matches overlay_manager
# [G]/[C] and legend [H]), so a focused text field could consume them first.
func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_F:
		_fit_to_world()

func _process(delta: float) -> void:
	var v := Vector2.ZERO
	if Input.is_key_pressed(KEY_W) or Input.is_key_pressed(KEY_UP):    v.y -= 1
	if Input.is_key_pressed(KEY_S) or Input.is_key_pressed(KEY_DOWN):  v.y += 1
	if Input.is_key_pressed(KEY_A) or Input.is_key_pressed(KEY_LEFT):  v.x -= 1
	if Input.is_key_pressed(KEY_D) or Input.is_key_pressed(KEY_RIGHT): v.x += 1
	if v != Vector2.ZERO:
		position += v.normalized() * (PAN_SPEED_KEYS * delta) / zoom.x
