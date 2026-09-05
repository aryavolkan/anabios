extends RefCounted

# Pure viewer-animation math, kept free of node/autoload references so the
# headless test can compile it standalone (main.gd and camera_controller.gd
# cannot load under -s). Static-only; no state.

# Traveling-pulse tuning for trade-route flow: bright spots FLOW_WAVELEN world
# units apart, moving from a segment's `from` toward its `to` end.
const FLOW_WAVELEN := 42.0
const FLOW_SPEED := 1.6
const FLOW_FLOOR := 0.65


# Construction-pop ease: overshoot a touch past full size, then settle
# (ease-out-back, the huts' cousin of the agents' birth pop). Clamped so
# callers can feed raw elapsed/duration ratios.
static func pop_scale(t: float) -> float:
	var x := clampf(t, 0.0, 1.0)
	var s := 1.70158
	var xm := x - 1.0
	return 1.0 + (s + 1.0) * xm * xm * xm + s * xm * xm


# Brightness factor [FLOW_FLOOR..1] for a point `proj` world units along a
# route at time `t`: a sharpened sine so distinct bright pulses travel in the
# +proj direction, reading as goods moving down the line.
static func flow_pulse(proj: float, t: float) -> float:
	var wave := 0.5 + 0.5 * sin(TAU * (proj / FLOW_WAVELEN - t * FLOW_SPEED))
	return FLOW_FLOOR + (1.0 - FLOW_FLOOR) * wave * wave * wave


# Radial falloff blob (bright core, soft quadratic edge): the particle/light
# texture shared by the fire lights and the settlement smoke plumes.
static func radial_texture(res: int) -> ImageTexture:
	var img := Image.create(res, res, false, Image.FORMAT_RGBA8)
	var c := (res - 1) * 0.5
	for y in res:
		for x in res:
			var d := Vector2(x - c, y - c).length() / c
			var a := clampf(1.0 - d * d, 0.0, 1.0)
			img.set_pixel(x, y, Color(a, a, a, 1.0))
	return ImageTexture.create_from_image(img)
