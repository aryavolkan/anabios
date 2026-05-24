extends Camera2D

const ZOOM_STEP: float = 1.2
const ZOOM_MIN: float = 0.25
const ZOOM_MAX: float = 8.0
const PAN_SPEED_KEYS: float = 600.0

var _dragging: bool = false

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

func _process(delta: float) -> void:
	var v := Vector2.ZERO
	if Input.is_key_pressed(KEY_W) or Input.is_key_pressed(KEY_UP):    v.y -= 1
	if Input.is_key_pressed(KEY_S) or Input.is_key_pressed(KEY_DOWN):  v.y += 1
	if Input.is_key_pressed(KEY_A) or Input.is_key_pressed(KEY_LEFT):  v.x -= 1
	if Input.is_key_pressed(KEY_D) or Input.is_key_pressed(KEY_RIGHT): v.x += 1
	if v != Vector2.ZERO:
		position += v.normalized() * (PAN_SPEED_KEYS * delta) / zoom.x
