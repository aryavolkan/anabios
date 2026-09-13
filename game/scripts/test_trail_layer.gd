extends SceneTree
# Headless unit test for the trail layer (footstep tracks + combat/trade
# segment trails), split out of main.gd. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_trail_layer.gd
# Exits 0 on success, 1 on the first failed assertion.
#
# The dummy rendering driver stores visible_instance_count on the resource but
# NOT per-instance transforms/colors (they read back identity/black), so the
# assertions target visible counts and the layer's own pools.

const TrailLayer = preload("res://scripts/trail_layer.gd")

const WORLD := 100.0

var _no_pts := PackedVector2Array()
var _no_cols := PackedColorArray()
var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


# A stand-in for the scene's Streaks/TradeRoutes multimeshes (main.tscn ships
# them pre-grown; instance_count is the segment-trail budget).
func _make_mm(budget: int = 4096) -> MultiMesh:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_2D
	mm.use_colors = true
	mm.mesh = QuadMesh.new()
	mm.instance_count = budget
	return mm


func _make_layer(streaks_mm: MultiMesh, trade_mm: MultiMesh) -> Node2D:
	var layer := TrailLayer.new()
	root.add_child(layer)
	var img := Image.create(4, 4, false, Image.FORMAT_RGBA8)
	layer.setup(streaks_mm, trade_mm, QuadMesh.new(), ImageTexture.create_from_image(img))
	return layer


# Feed only the footstep-track inputs, leaving the segment trails idle.
func _tick_tracks(layer: Node2D, delta: float, sample: PackedVector2Array, paused: bool) -> void:
	layer.update(delta, sample, paused, _no_pts, _no_cols, _no_pts, _no_cols, WORLD)


func _check_tracks() -> void:
	var layer := _make_layer(_make_mm(), _make_mm())
	var mmi: MultiMeshInstance2D = layer.tracks_mmi()
	_check(mmi != null, "layer exposes the tracks MMI for the wrap clones")
	_check(mmi.get_parent() == layer, "tracks MMI is built as the layer's child")
	_check(mmi.z_index == -1, "tracks keep their authored z order (below bodies)")
	_check(mmi.multimesh != null and mmi.texture != null, "tracks MMI is fully wired")

	# One track per sampled walker; a track fades TRACK_TTL seconds after birth
	# (the spawning update itself already ages it by delta).
	var sample := PackedVector2Array([Vector2(1, 2), Vector2(3, 4), Vector2(5, 6)])
	_tick_tracks(layer, 0.1, sample, false)
	var tmm: MultiMesh = mmi.multimesh
	_check(tmm.visible_instance_count == 3, "one track mark per sampled walker")
	for i in 12:
		_tick_tracks(layer, 0.1, _no_pts, false)
	_check(tmm.visible_instance_count == 3, "tracks persist through TRACK_TTL")
	_tick_tracks(layer, 0.2, _no_pts, false)
	_check(tmm.visible_instance_count == 0, "tracks age out after TRACK_TTL")

	# The pool is capped: the oldest marks drop first, never past TRACK_CAP.
	var flood := PackedVector2Array()
	for i in TrailLayer.TRACK_CAP + 44:
		flood.append(Vector2(i, i))
	_tick_tracks(layer, 0.0, flood, false)
	_check(
		tmm.visible_instance_count == TrailLayer.TRACK_CAP,
		"track pool respects TRACK_CAP (oldest dropped first)"
	)

	# Pause gates SPAWNING only; existing marks keep fading in wall time (the
	# world freezes but the rain-washed-footprints effect keeps playing).
	_tick_tracks(layer, 2.0, _no_pts, false)
	_check(tmm.visible_instance_count == 0, "flood ages out (test isolation)")
	_tick_tracks(layer, 0.1, sample, true)
	_check(tmm.visible_instance_count == 0, "paused: no new tracks spawn")
	_tick_tracks(layer, 0.1, sample, false)
	_check(tmm.visible_instance_count == 3, "unpaused: spawning resumes")
	for i in 15:
		_tick_tracks(layer, 0.1, _no_pts, true)
	_check(tmm.visible_instance_count == 0, "paused: existing tracks still age out")


func _check_segment_trails() -> void:
	var streaks_mm := _make_mm()
	var trade_mm := _make_mm()
	var layer := _make_layer(streaks_mm, trade_mm)
	var segs := PackedVector2Array([Vector2(10, 10), Vector2(20, 10)])
	var cols := PackedColorArray([Color(1.0, 0.3, 0.2, 1.0)])

	# One quad per segment pair, on both trails.
	layer.update(0.016, _no_pts, false, segs, cols, segs, cols, WORLD)
	_check(streaks_mm.visible_instance_count == 1, "combat streak segment drawn")
	_check(trade_mm.visible_instance_count == 1, "trade route segment drawn")

	# Ageing is wall-clock (absolute expiry stamps): past STREAK_TTL but short
	# of TRADE_TTL only the brief combat streak has died.
	OS.delay_msec(int(TrailLayer.STREAK_TTL * 1000.0) + 80)
	layer.update(0.016, _no_pts, false, _no_pts, _no_cols, _no_pts, _no_cols, WORLD)
	_check(streaks_mm.visible_instance_count == 0, "streaks expire after STREAK_TTL")
	_check(trade_mm.visible_instance_count == 1, "trade lanes outlive streaks (longer TTL)")
	OS.delay_msec(int(TrailLayer.TRADE_TTL * 1000.0) + 80)
	layer.update(0.016, _no_pts, false, _no_pts, _no_cols, _no_pts, _no_cols, WORLD)
	_check(trade_mm.visible_instance_count == 0, "trade lanes expire after TRADE_TTL")

	# At the pixel-art zooms the trade lane is worn earth, not a hue tracer:
	# every lane takes the trodden brown, ignoring the genome colour.
	layer.update(0.016, _no_pts, false, _no_pts, _no_cols, segs, cols, WORLD, 4.0)
	_check(trade_mm.visible_instance_count == 1, "close-zoom trade lane drawn")
	var close_pal := TrailLayer.trade_palette(cols, 4.0)
	_check(
		close_pal.size() == 1 and close_pal[0] == TrailLayer.TRADE_EARTH, "close lanes are earth"
	)
	_check(TrailLayer.trade_palette(cols, 1.0) == cols, "far lanes keep the genome hue")
	_check(TrailLayer.TRADE_EARTH_ALPHA < 0.6, "close lanes are fainter than the far tracers")

	# Perf cap: the trail never outgrows the multimesh budget; oldest first.
	var tiny_mm := _make_mm(4)
	var tiny := _make_layer(tiny_mm, _make_mm(4))
	var three := PackedVector2Array(
		[Vector2(0, 0), Vector2(1, 0), Vector2(2, 0), Vector2(3, 0), Vector2(4, 0), Vector2(5, 0)]
	)
	var three_cols := PackedColorArray([Color.RED, Color.GREEN, Color.BLUE])
	tiny.update(0.016, _no_pts, false, three, three_cols, _no_pts, _no_cols, WORLD)
	tiny.update(0.016, _no_pts, false, three, three_cols, _no_pts, _no_cols, WORLD)
	_check(
		tiny_mm.visible_instance_count == 4, "segment trail capped at the multimesh instance budget"
	)


func _init() -> void:
	_check_tracks()
	_check_segment_trails()
	if _failed:
		quit(1)
		return
	print("test_trail_layer: all passed")
	quit(0)
