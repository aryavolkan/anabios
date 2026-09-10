extends SceneTree
# Headless checks for the codex event -> visual effect mapping. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_event_fx.gd
# Exits 0 on success, 1 on the first failed assertion.

const EventFx = preload("res://scripts/event_fx.gd")
const FxMath = preload("res://scripts/fx_math.gd")
const FxRing = preload("res://scripts/fx_ring.gd")
const ViewerEffects = preload("res://scripts/viewer_effects.gd")
const CodexPanel = preload("res://scripts/codex_panel.gd")


# Stand-in for camera_controller.gd, which cannot compile under -s (it touches
# the GameConfig autoload). Records trauma so the test can assert on it.
class StubCam:
	extends Camera2D
	var trauma_total := 0.0

	func add_trauma(amount: float) -> void:
		trauma_total += amount


const KINDS := ["fire", "ring", "motes", "trauma"]
const FIRE_IDS := [4, 17, 35, 42, 43]
const TRAUMA_IDS := [7, 38]

# quit() only REQUESTS an exit — _init keeps running and a later quit(0)
# would override quit(1) — so failures set a flag and the verdict is issued
# exactly once, at the end.
var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	_check_spec_table()
	_check_ring_math()
	_check_apply_all()
	_check_pop_scale()
	_check_flow_pulse()
	_check_gait()
	_check_locomotion()
	_check_action_transition()
	_check_drink_action()
	_check_animation_clock()
	_check_facing()
	_check_shuttle_and_ease()
	_check_radial_texture()
	if _failed:
		quit(1)
		return
	print("test_event_fx: all passed")
	quit(0)


func _check_pop_scale() -> void:
	_check(absf(FxMath.pop_scale(0.0)) < 0.01, "pop starts at zero scale")
	_check(absf(FxMath.pop_scale(1.0) - 1.0) < 0.01, "pop settles at full scale")
	_check(FxMath.pop_scale(-0.5) == FxMath.pop_scale(0.0), "pop clamps below")
	_check(FxMath.pop_scale(2.0) == FxMath.pop_scale(1.0), "pop clamps above")
	var peak := 0.0
	for i in 101:
		peak = maxf(peak, FxMath.pop_scale(i / 100.0))
	_check(peak > 1.02 and peak < 1.25, "pop overshoots a little, not wildly (peak %f)" % peak)


func _check_flow_pulse() -> void:
	# Brightness stays within its band across the phase space.
	for i in 40:
		var v := FxMath.flow_pulse(i * 13.7, i * 0.31)
		_check(v >= 0.6 and v <= 1.001, "flow pulse in band (got %f)" % v)
	# The bright spot travels: at a fixed point, brightness changes over time...
	var a := FxMath.flow_pulse(10.0, 0.0)
	var b := FxMath.flow_pulse(10.0, 0.15)
	_check(absf(a - b) > 0.01, "flow pulse animates over time")
	# ...and at a fixed time the pulse is periodic along the route.
	var w := FxMath.FLOW_WAVELEN
	var c := FxMath.flow_pulse(3.0, 0.4)
	var d := FxMath.flow_pulse(3.0 + w, 0.4)
	_check(absf(c - d) < 0.001, "flow pulse periodic along the route")


# Distance-driven gait. The cycle must be a function of ground covered — not of
# frame count, frame rate, or heading — which is exactly what the old
# position-hashed phase got wrong.
func _check_gait() -> void:
	var stride := FxMath.stride_len(1.0, FxMath.GAIT_FPS_REF)
	_check(absf(stride - FxMath.STRIDE_WORLD) < 0.001, "reference size/cadence = base stride")
	_check(FxMath.stride_len(2.0, FxMath.GAIT_FPS_REF) > stride, "a bigger body strides further")
	_check(
		FxMath.stride_len(1.0, FxMath.GAIT_FPS_REF * 0.5) > stride,
		"a slower authored cadence covers more ground per cycle"
	)
	_check(FxMath.stride_len(0.0, 0.0) > 0.0, "stride guards a zero size/cadence")

	# Seeds spread a fresh crowd around the loop, and are stable per id.
	var octants := {}
	for id in 64:
		var s := FxMath.seed_gait(id)
		_check(s >= 0.0 and s < 1.0, "seed %d stays inside the cycle" % id)
		_check(s == FxMath.seed_gait(id), "seed %d is stable" % id)
		octants[int(s * 8.0)] = true
	_check(octants.size() >= 6, "seeds spread across the loop (%d/8 octants)" % octants.size())

	# Two strides of ground, chopped coarsely or finely, land on the same pose.
	# A clocked or frame-rate-dependent cycle fails this.
	var coarse := 0.25
	for _c in 8:
		coarse = FxMath.advance_gait(coarse, stride * 0.25, stride)
	var fine := 0.25
	for _f in 40:
		fine = FxMath.advance_gait(fine, stride * 0.05, stride)
	_check(
		absf(coarse - fine) < 0.001, "cycle tracks distance, not frames (%f/%f)" % [coarse, fine]
	)
	_check(absf(coarse - 0.25) < 0.001, "a whole number of strides returns to the same pose")

	# The per-frame ceiling holds, and the cycle never leaves [0, 1).
	var capped := FxMath.advance_gait(0.0, stride * 100.0, stride)
	_check(absf(capped - FxMath.GAIT_MAX_STEP) < 0.001, "a huge step is capped, not strobed")
	for i in 20:
		var p := FxMath.advance_gait(0.9, stride * i * 0.03, stride)
		_check(p >= 0.0 and p < 1.0, "cycle wraps into range (step %d)" % i)


# Locomotion debounce. The sim's heading flickers to exactly 0 every few
# frames; the hold credit has to bridge those gaps so the pose does not pop,
# while a real stop still settles to idle.
func _check_locomotion() -> void:
	var dt := 1.0 / 60.0
	var st := Vector2.ZERO

	# Walking steadily -> weight saturates and stays walking.
	for _i in 60:
		st = FxMath.step_locomotion(st, true, dt)
	_check(st.x > 0.0, "steady walking holds the walk state")
	_check(st.y > 0.95, "walk weight saturates near 1 (got %f)" % st.y)

	# The measured failure case: heading present ~half the frames, flipping
	# every ~4. This must NOT drop the pose state even once.
	var drops := 0
	for i in 240:
		var raw := int(i / 4.0) % 2 == 0
		st = FxMath.step_locomotion(st, raw, dt)
		if st.x <= 0.0:
			drops += 1
	_check(drops == 0, "4-frame heading flicker never drops the walk state (%d drops)" % drops)
	_check(st.y > 0.9, "weight stays high through the flicker (got %f)" % st.y)

	# A real stop still settles: hold expires, then the weight falls away.
	# Top the credit up first — the flicker loop above ends on non-moving
	# frames, so it leaves the hold already part-spent.
	st = FxMath.step_locomotion(st, true, dt)
	var held := 0
	for _i in 120:
		st = FxMath.step_locomotion(st, false, dt)
		if st.x > 0.0:
			held += 1
	_check(st.x == 0.0, "a real stop expires the hold credit")
	_check(st.y < 0.05, "a real stop settles the weight to idle (got %f)" % st.y)
	var held_secs := float(held) * dt
	_check(
		absf(held_secs - FxMath.WALK_HOLD) < 0.03,
		"hold lasts about WALK_HOLD seconds (got %f)" % held_secs
	)

	# Weight stays in range from any starting state.
	for i in 40:
		var s2 := FxMath.step_locomotion(Vector2(float(i) * 0.01, float(i) / 40.0), i % 2 == 0, dt)
		_check(s2.y >= 0.0 and s2.y <= 1.0, "weight stays in 0..1 (step %d)" % i)
		_check(s2.x >= 0.0, "hold credit never goes negative (step %d)" % i)


# Behavior poses need a short recovery beat when their source signal clears;
# otherwise a noisy threshold can cut the two-frame action off mid-motion.
func _check_action_transition() -> void:
	var dt := 1.0 / 60.0
	var state := Vector2.ZERO
	state = FxMath.step_action(state, 2.0, dt)
	_check(state.x == 2.0, "a new action starts immediately")
	_check(state.y > 0.0, "a new action receives recovery hold time")
	var held := FxMath.step_action(state, 0.0, dt)
	_check(held.x == 2.0, "clearing a signal does not cut the action immediately")
	for _i in 30:
		held = FxMath.step_action(held, 0.0, dt)
	_check(held.x == 0.0, "a cleared action eventually returns to neutral")


func _check_drink_action() -> void:
	_check(FxMath.action_pose_base(6.0) == 4, "drink reuses the graze/eat frame pair")
	_check(FxMath.action_pose_base(7.0) == 8, "courtship reuses the trade/alert frame pair")
	_check(FxMath.action_pose_base(8.0) == 8, "foraging scan reuses the alert frame pair")
	var state := FxMath.step_action(Vector2(-1.0, 0.0), 6.0, 0.05)
	_check(state.x == 6.0, "drink action starts immediately")
	_check(state.y > 0.0, "drink action receives recovery hold time")


func _check_animation_clock() -> void:
	_check(
		absf(FxMath.advance_animation_time(1.5, 0.25, false) - 1.75) < 0.001,
		"animation clock advances while running"
	)
	_check(
		absf(FxMath.advance_animation_time(1.5, 0.25, true) - 1.5) < 0.001,
		"animation clock freezes while paused"
	)


# Facing deadband. The measured failure was the sim zeroing an agent's heading
# to signal idle: cos(0) == 1, so a bare sign test snapped the sprite to face
# right on every flicker — 16 mirror flips per agent per second.
func _check_facing() -> void:
	var dt := 1.0 / 60.0

	# Settled facing left, then the heading flickers to 0 (idle) every few
	# frames. cos(0) = 1 would flip it right; the walking gate must prevent it.
	var st := Vector3(1.0, 1.0, -1.0)
	var flips := 0
	for i in 240:
		var idle := int(i / 3.0) % 2 == 0
		var hx := 1.0 if idle else -1.0
		st = FxMath.step_facing(st, hx, not idle, dt)
		if st.x != 1.0:
			flips += 1
	_check(flips == 0, "an idle-heading flicker never flips the mirror (%d flips)" % flips)
	_check(st.y > 0.9, "eased value stays on the left mirror (got %f)" % st.y)

	# The measured failure: a heading that genuinely swings full-scale several
	# times a second. A bare deadband on the instantaneous value cannot filter
	# this (it only got 14.5 flips/sec down to 10.8) — the low-pass must.
	var st2 := Vector3(0.0, 0.0, 0.0)
	var flips2 := 0
	for i in 600:
		var hx2 := 1.0 if int(i / 4.0) % 2 == 0 else -1.0
		var before := st2.x
		st2 = FxMath.step_facing(st2, hx2, true, dt)
		if st2.x != before:
			flips2 += 1
	var per_sec := 60.0 * float(flips2) / 600.0
	_check(per_sec < 1.0, "a 7Hz heading swing yields under 1 flip/sec (got %f)" % per_sec)

	# A sustained turn still commits, and the eased value follows it.
	var st3 := Vector3(0.0, 0.0, 1.0)
	for _i in 60:
		st3 = FxMath.step_facing(st3, -1.0, true, dt)
	_check(st3.x == 1.0, "a sustained left heading commits the turn")
	_check(st3.y > 0.95, "eased value catches up to the committed side (got %f)" % st3.y)

	# Mid-turn the eased value is strictly between the mirrors — that is the
	# flip-squash the shader renders — and it never leaves 0..1.
	var st4 := Vector3(0.0, 0.0, 1.0)
	var mid := 0
	for _i in 40:
		st4 = FxMath.step_facing(st4, -1.0, true, dt)
		if st4.y > 0.02 and st4.y < 0.98:
			mid += 1
		_check(st4.y >= 0.0 and st4.y <= 1.0, "eased facing stays in 0..1")
	_check(mid > 0, "a turn passes through the flip-squash rather than snapping")

	# A stopped body holds everything, including its tracked heading.
	var st5 := Vector3(1.0, 1.0, -0.8)
	for _i in 60:
		st5 = FxMath.step_facing(st5, 1.0, false, dt)
	_check(st5.x == 1.0 and is_equal_approx(st5.z, -0.8), "a stopped body holds its facing")


# Convoy turnaround easing, and the frame-rate-independent approach factor.
func _check_shuttle_and_ease() -> void:
	# Endpoints and midpoint are preserved, so the route span is unchanged.
	_check(absf(FxMath.shuttle_ease(0.0)) < 0.0001, "shuttle starts at 0")
	_check(absf(FxMath.shuttle_ease(1.0) - 1.0) < 0.0001, "shuttle ends at 1")
	_check(absf(FxMath.shuttle_ease(0.5) - 0.5) < 0.0001, "shuttle is centred at the midpoint")
	_check(FxMath.shuttle_ease(-1.0) == FxMath.shuttle_ease(0.0), "shuttle clamps below")
	_check(FxMath.shuttle_ease(2.0) == FxMath.shuttle_ease(1.0), "shuttle clamps above")

	# Monotonic, stays in range, and — the point of it — the speed falls to
	# near zero at each turn instead of reversing at full tilt.
	var prev := FxMath.shuttle_ease(0.0)
	var end_speed := 0.0
	var mid_speed := 0.0
	for i in range(1, 101):
		var v := FxMath.shuttle_ease(i / 100.0)
		_check(v >= prev - 0.0001, "shuttle is monotonic (step %d)" % i)
		_check(v >= 0.0 and v <= 1.0, "shuttle stays in 0..1 (step %d)" % i)
		var speed := v - prev
		if i <= 2 or i >= 100:
			end_speed = maxf(end_speed, speed)
		if i == 50:
			mid_speed = speed
		prev = v
	_check(
		end_speed < mid_speed * 0.2,
		"speed at the turn is a fraction of mid-route (%f vs %f)" % [end_speed, mid_speed]
	)

	# ease_factor: same elapsed time gives the same approach however it is
	# chopped up, which is exactly what a per-update lerp weight gets wrong.
	var tau := 0.93
	var one_step := FxMath.ease_factor(0.5, tau)
	var remaining := 1.0
	for _i in 30:
		remaining *= 1.0 - FxMath.ease_factor(0.5 / 30.0, tau)
	_check(
		absf((1.0 - remaining) - one_step) < 0.001,
		"ease_factor is frame-rate independent (%f vs %f)" % [1.0 - remaining, one_step]
	)
	_check(FxMath.ease_factor(0.0, tau) == 0.0, "no elapsed time means no movement")
	_check(FxMath.ease_factor(-1.0, tau) == 0.0, "negative elapsed time is clamped")
	_check(FxMath.ease_factor(100.0, tau) < 1.0001, "approach never overshoots the target")
	_check(FxMath.ease_factor(1.0, 0.0) <= 1.0, "a zero time constant stays bounded")

	# The eases inside step_locomotion / step_facing use the same exponential
	# form. A bare `delta * rate` weight degenerates to an instant snap once the
	# frame time exceeds 1/rate, so check a slow frame still eases rather than
	# jumping, and that equal elapsed time converges alike however it is chopped.
	var slow := FxMath.step_locomotion(Vector2.ZERO, true, 0.25)
	_check(slow.y > 0.0 and slow.y < 1.0, "a 4 fps frame still eases the walk weight (%f)" % slow.y)
	var coarse := FxMath.step_facing(Vector3(0.0, 0.0, 1.0), -1.0, true, 0.2)
	var fine := Vector3(0.0, 0.0, 1.0)
	for _i in 12:
		fine = FxMath.step_facing(fine, -1.0, true, 0.2 / 12.0)
	_check(
		absf(coarse.z - fine.z) < 0.02,
		"facing low-pass converges alike coarse or fine (%f vs %f)" % [coarse.z, fine.z]
	)


func _check_radial_texture() -> void:
	var tex := FxMath.radial_texture(16)
	_check(tex.get_width() == 16 and tex.get_height() == 16, "radial texture sized to request")
	var img := tex.get_image()
	var center := img.get_pixel(8, 8).r
	var edge := img.get_pixel(0, 8).r
	_check(center > 0.9, "radial texture bright at center (got %f)" % center)
	_check(edge < 0.15, "radial texture dark at edge (got %f)" % edge)


func _check_spec_table() -> void:
	var n_types := CodexPanel.CHAPTER_NAMES.size()
	var mapped := EventFx.FX.keys()
	_check(mapped.size() >= 24, "at least 24 event types mapped (got %d)" % mapped.size())
	for t in mapped:
		_check(t >= 0 and t < n_types, "event id %s in range 0..%d" % [t, n_types - 1])
		var specs: Array = EventFx.spec(int(t))
		_check(not specs.is_empty(), "spec(%d) non-empty" % t)
		for s in specs:
			_check(KINDS.has(s["kind"]), "kind '%s' valid for id %d" % [s["kind"], t])
			if s["kind"] == "ring" or s["kind"] == "motes":
				_check((s["color"] as Color).a > 0.0, "id %d %s color visible" % [t, s["kind"]])
			if s["kind"] == "trauma":
				_check(s["amount"] > 0.0 and s["amount"] <= 0.5, "id %d trauma sane" % t)
	# Unmapped ids yield an empty spec, not an error.
	_check(EventFx.spec(9999).is_empty(), "unknown id maps to no effects")
	# Legacy parity: the pre-existing fire and trauma events keep their effects.
	for t in FIRE_IDS:
		_check(_has_kind(EventFx.spec(t), "fire"), "id %d still fire-kind" % t)
	for t in TRAUMA_IDS:
		_check(_has_kind(EventFx.spec(t), "trauma"), "id %d still shakes the camera" % t)
		for s in EventFx.spec(t):
			if s["kind"] == "trauma":
				_check(s["amount"] == 0.25, "id %d keeps its original 0.25 trauma" % t)


func _has_kind(specs: Array, kind: String) -> bool:
	for s in specs:
		if s["kind"] == kind:
			return true
	return false


func _check_ring_math() -> void:
	var ring := FxRing.new()
	ring.start(Vector2(5, 5), Color(1, 0, 0, 0.8), 2.0, 100.0)
	_check(ring.active, "ring active after start")
	_check(ring.radius_now() < 10.0, "ring starts small (got %f)" % ring.radius_now())
	_check(ring.alpha_now() > 0.5, "ring starts visible")
	ring.step(1.0)
	var mid_r := ring.radius_now()
	_check(mid_r > 10.0 and mid_r < 100.0, "ring mid-flight radius grows (got %f)" % mid_r)
	ring.step(0.9)
	_check(ring.radius_now() > mid_r, "radius grows monotonically")
	_check(ring.alpha_now() < 0.3, "alpha fades toward end")
	ring.step(0.2)
	_check(not ring.active, "ring deactivates after its duration")
	ring.free()


func _check_apply_all() -> void:
	var fx: Node2D = ViewerEffects.new()
	var cam := StubCam.new()
	var disc := ImageTexture.create_from_image(Image.create(8, 8, false, Image.FORMAT_RGBA8))
	fx.setup(null, cam, null, disc)
	# Every mapped event applies without error, at a location and at ZERO
	# (ZERO means "no location": positional effects skip, trauma still fires).
	for t in EventFx.FX.keys():
		fx.apply_event_fx(int(t), Vector2(12, 34))
		fx.apply_event_fx(int(t), Vector2.ZERO)
	var rings_live := 0
	for r in fx.rings():
		if r.active:
			rings_live += 1
	_check(rings_live > 0, "applying ring events activates pooled rings")
	_check(cam.trauma_total > 0.0, "trauma events reach the camera even without a location")
	fx.update_rings(0.1)
	fx.free()
	cam.free()
