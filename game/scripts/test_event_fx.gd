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
		var raw := (i / 4) % 2 == 0
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
