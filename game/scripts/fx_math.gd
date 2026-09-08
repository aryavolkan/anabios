extends RefCounted

# Pure viewer-animation math, kept free of node/autoload references so the
# headless test can compile it standalone (main.gd and camera_controller.gd
# cannot load under -s). Static-only; no state.

# Traveling-pulse tuning for trade-route flow: bright spots FLOW_WAVELEN world
# units apart, moving from a segment's `from` toward its `to` end.
const FLOW_WAVELEN := 42.0
const FLOW_SPEED := 1.6
const FLOW_FLOOR := 0.65

# Distance-driven gait. The field_agent shader reads a per-instance cycle
# position (0..1 = one contact -> passing -> contact -> passing loop) and picks
# the walk pose from it. Pacing that cycle by the distance a body actually
# covers on screen — rather than ticking it on a clock — is what keeps the feet
# with the ground. The earlier scheme hashed the agent's *position* into a
# phase offset and clocked the cycle from TIME, so the phase drifted as an
# agent walked (about half a pose per body-length, signed by travel direction):
# westbound animals ran their gait backwards and every turn stuttered it.
#
# STRIDE_WORLD is how far a size-1.0 body travels per full cycle.
const STRIDE_WORLD := 1.6
# The authored per-species cadences (MammalSprites.bucket_gait_fps) used to set
# the cycle rate directly. They now set stride *length* instead — a species
# authored slow covers more ground per cycle — so the hand-tuned character
# survives the move to distance-driven timing. Reference = the hominin default.
const GAIT_FPS_REF := 5.0
# Ceiling on one frame's advance, so a long frame or a time-lapse burst rolls
# the poses forward smoothly instead of strobing through them.
const GAIT_MAX_STEP := 0.34

# Locomotion state. The sim reports a heading of exactly 0.0 whenever an
# agent's velocity rounds away, and that flickers: measured at 3.5 flips per
# agent per second, holding each state a mean of 4.2 frames. Fed straight to
# the shader it pops the sprite between the neutral pose and the walk cycle
# several times a second. WALK_HOLD is how long a body keeps "recently moving"
# credit after the heading drops, which bridges those gaps so only a real stop
# reads as standing.
const WALK_HOLD := 0.25
# How fast the blended walk weight chases the held state — quick enough to feel
# responsive, slow enough that a stride eases in rather than snapping on.
const WALK_BLEND := 9.0

# Facing. cos(heading) is near zero whenever a body travels near-vertically,
# and it is exactly 1.0 when the sim zeroes the heading to mean "idle" — so a
# bare sign test snaps the sprite to face right every time the heading
# flickers. Measured at 16 mirror flips per agent per second, leaving 37% of
# the crowd caught mid-flip and horizontally squashed at any moment. Commit to
# a turn only once the heading is decisively sideways, and let a body that has
# stopped keep the way it was last facing.
const FACE_DEADBAND := 0.25
const FACE_EASE := 12.0
# A deadband on the instantaneous heading is not enough on its own: these
# agents genuinely swing their heading ~14 times a second, so the raw value
# clears any usable threshold constantly (measured: a bare deadband only got
# 14.5 flips/sec down to 10.8). Low-pass the heading first and test the
# deadband against THAT, so only a sustained turn commits. The time constant
# has to be well longer than the ~70ms flicker period.
const FACE_TRACK := 4.0


# Construction-pop ease: overshoot a touch past full size, then settle
# (ease-out-back, the huts' cousin of the agents' birth pop). Clamped so
# callers can feed raw elapsed/duration ratios.
static func pop_scale(t: float) -> float:
	var x := clampf(t, 0.0, 1.0)
	var s := 1.70158
	var xm := x - 1.0
	return 1.0 + (s + 1.0) * xm * xm * xm + s * xm * xm


# Ease-out-back: overshoot past the target, then settle. Shared by the birth
# pop and the death ghost's topple.
static func ease_out_back(t: float) -> float:
	const C1 := 1.70158
	const C3 := C1 + 1.0
	return 1.0 + C3 * pow(t - 1.0, 3) + C1 * pow(t - 1.0, 2)


# Birth scale: a quick anticipation squash (grow 0.3 -> 0.8), then an
# ease-out-back spring to 1.0 with overshoot.
static func birth_scale(t: float) -> float:
	const ANTICIPATE := 0.3
	if t < ANTICIPATE:
		return lerpf(0.3, 0.8, t / ANTICIPATE)
	return 0.8 + 0.2 * ease_out_back((t - ANTICIPATE) / (1.0 - ANTICIPATE))


# Brightness factor [FLOW_FLOOR..1] for a point `proj` world units along a
# route at time `t`: a sharpened sine so distinct bright pulses travel in the
# +proj direction, reading as goods moving down the line.
static func flow_pulse(proj: float, t: float) -> float:
	var wave := 0.5 + 0.5 * sin(TAU * (proj / FLOW_WAVELEN - t * FLOW_SPEED))
	return FLOW_FLOOR + (1.0 - FLOW_FLOOR) * wave * wave * wave


# Distance a body of `size` covers per full gait cycle, in world units, for a
# render bucket whose authored cadence is `gait_fps`. Bigger animals stride
# further, so a deer and a hare crossing the same ground don't share a cadence.
static func stride_len(size: float, gait_fps: float) -> float:
	return STRIDE_WORLD * maxf(size, 0.3) * (GAIT_FPS_REF / maxf(gait_fps, 0.5))


# Where a newly seen agent starts its cycle. Hashing the id by the golden ratio
# spreads a freshly seeded crowd around the loop so they don't march in step.
static func seed_gait(id: int) -> float:
	return fposmod(float(id) * 0.6180339887, 1.0)


# Advance a cycle position by the `step` distance a body covered on screen.
static func advance_gait(phase: float, step: float, stride: float) -> float:
	return fposmod(phase + minf(step / stride, GAIT_MAX_STEP), 1.0)


# Advance one agent's locomotion state. `st.x` is the remaining "recently
# moving" credit in seconds, `st.y` the blended 0..1 walk weight the shader
# reads. Returns the updated pair; `st.x > 0` is the debounced "is walking".
static func step_locomotion(st: Vector2, raw_moving: bool, delta: float) -> Vector2:
	var hold: float = WALK_HOLD if raw_moving else maxf(st.x - delta, 0.0)
	var target: float = 1.0 if hold > 0.0 else 0.0
	return Vector2(hold, lerpf(st.y, target, minf(1.0, delta * WALK_BLEND)))


# Advance one agent's facing. `st` is (committed side 0 right / 1 left, eased
# value the shader mirrors with, low-passed heading x). Mid-way the eased value
# collapses the shader's UV mix to the centre column, so a turn reads as a
# flip-squash. `heading_x` is cos(heading); a stopped body holds everything.
static func step_facing(st: Vector3, heading_x: float, walking: bool, delta: float) -> Vector3:
	var hx: float = lerpf(st.z, heading_x, minf(1.0, delta * FACE_TRACK)) if walking else st.z
	var side: float = st.x
	if hx > FACE_DEADBAND:
		side = 0.0
	elif hx < -FACE_DEADBAND:
		side = 1.0
	return Vector3(side, lerpf(st.y, side, minf(1.0, delta * FACE_EASE)), hx)


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
