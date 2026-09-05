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


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		quit(1)


func _init() -> void:
	_check_spec_table()
	_check_ring_math()
	_check_apply_all()
	_check_pop_scale()
	_check_flow_pulse()
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
	_check_radial_texture()


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
