extends Node2D

# One pooled expanding ring pulse: an eased-out circle that grows to
# max_radius while fading, used by viewer_effects to mark codex events in the
# world. Pool-owned: start() re-arms it, step() advances it, and it hides
# itself when its duration elapses (draw idiom follows replay_highlight.gd and
# the custom-drawing demo's antialiased draw_arc).

var active := false

var _color := Color.WHITE
var _duration := 1.0
var _max_radius := 80.0
var _t := 0.0


func start(pos: Vector2, color: Color, duration: float, max_radius: float) -> void:
	position = pos
	_color = color
	_duration = maxf(duration, 0.05)
	_max_radius = max_radius
	_t = 0.0
	active = true
	visible = true
	queue_redraw()


func step(delta: float) -> void:
	if not active:
		return
	_t += delta
	if _t >= _duration:
		active = false
		visible = false
	queue_redraw()


# Ease-out growth: fast opening, gentle landing at max_radius.
func radius_now() -> float:
	var x := _progress()
	return _max_radius * (1.0 - (1.0 - x) * (1.0 - x))


# Linear fade from the spec color's own alpha down to zero.
func alpha_now() -> float:
	return _color.a * (1.0 - _progress())


func _progress() -> float:
	return clampf(_t / _duration, 0.0, 1.0)


func _draw() -> void:
	if not active:
		return
	var r := radius_now()
	if r < 0.5:
		return
	var a := alpha_now()
	draw_arc(Vector2.ZERO, r, 0.0, TAU, 48, Color(_color.r, _color.g, _color.b, a), 2.0, true)
	draw_arc(
		Vector2.ZERO,
		r * 0.78,
		0.0,
		TAU,
		40,
		Color(_color.r, _color.g, _color.b, a * 0.45),
		1.2,
		true
	)
