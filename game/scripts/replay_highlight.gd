extends Node2D

# Pulsing gold ring marking a replayed event's location.

var _phase: float = 0.0

func _process(delta: float) -> void:
	_phase += delta
	queue_redraw()

func _draw() -> void:
	var pulse: float = 0.5 + 0.5 * sin(_phase * 4.0)
	var alpha: float = 0.45 + 0.45 * pulse
	var radius: float = 18.0 + 6.0 * pulse
	draw_arc(Vector2.ZERO, radius, 0.0, TAU, 48, Color(1.0, 0.9, 0.35, alpha), 2.5)
	draw_arc(Vector2.ZERO, radius * 0.55, 0.0, TAU, 36, Color(1.0, 0.9, 0.35, alpha * 0.6), 1.5)
