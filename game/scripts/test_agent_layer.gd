extends SceneTree
# Headless unit test for agent_layer.gd's pure static helpers: the
# view-rect arithmetic behind the Phase 3 visible-set culling
# (docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md, §6
# Phase 3), the torus-aware death-ghost rect check, and the two-pointer id
# merge that the smoothing pass is built on — the piece most likely to
# regress. AgentLayer itself references no autoload (see camera_controller.gd
# and main.gd, which cannot compile under -s for that reason — noted in
# test_event_fx.gd), so the whole script preloads cleanly here. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_agent_layer.gd
# Exits 0 on success, 1 on the first failed assertion.

const AgentLayer = preload("res://scripts/agent_layer.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _check_view_rect() -> void:
	# zoom 2x halves the world-space viewport extent; margin grows it back out
	# on every side.
	var r: Rect2 = AgentLayer.view_rect(Vector2(100, 50), 2.0, Vector2(800, 600), 64.0)
	_check(
		r.position.is_equal_approx(Vector2(-164, -164)),
		"view_rect position: expected (-164,-164), got %s" % r.position
	)
	_check(
		r.size.is_equal_approx(Vector2(528, 428)),
		"view_rect size: expected (528,428), got %s" % r.size
	)

	# zoom 1x, no margin: the rect is exactly the viewport in world units,
	# centred on the camera.
	var r2: Rect2 = AgentLayer.view_rect(Vector2.ZERO, 1.0, Vector2(200, 100), 0.0)
	_check(
		r2.position.is_equal_approx(Vector2(-100, -50)),
		"view_rect (no margin) position: expected (-100,-50), got %s" % r2.position
	)
	_check(
		r2.size.is_equal_approx(Vector2(200, 100)),
		"view_rect (no margin) size: expected (200,100), got %s" % r2.size
	)

	# A zero zoom must not divide by zero (defensive floor); the rect should
	# still come out finite and centred on cam_pos.
	var r3: Rect2 = AgentLayer.view_rect(Vector2(5, 5), 0.0, Vector2(100, 100), 10.0)
	_check(is_finite(r3.position.x) and is_finite(r3.size.x), "view_rect stays finite at zoom 0")
	_check(
		r3.get_center().is_equal_approx(Vector2(5, 5)),
		"view_rect stays centred on cam_pos at zoom 0, got center %s" % r3.get_center()
	)


func _check_pos_in_rect() -> void:
	# A plain non-wrapping rect.
	_check(
		AgentLayer.pos_in_rect(Vector2(5, 5), 0.0, 0.0, 10.0, 10.0, 100.0),
		"a point inside a normal rect is inside"
	)
	_check(
		not AgentLayer.pos_in_rect(Vector2(50, 5), 0.0, 0.0, 10.0, 10.0, 100.0),
		"a point outside a normal rect is outside"
	)

	# A rect that straddles the torus seam: [90, 110) on a world of size 100
	# wraps to cover [90,100) and [0,10).
	_check(
		AgentLayer.pos_in_rect(Vector2(95, 5), 90.0, 0.0, 110.0, 10.0, 100.0),
		"a point on the near side of a seam-straddling rect is inside"
	)
	_check(
		AgentLayer.pos_in_rect(Vector2(5, 5), 90.0, 0.0, 110.0, 10.0, 100.0),
		"a point wrapped onto the far side of a seam-straddling rect is inside"
	)
	_check(
		not AgentLayer.pos_in_rect(Vector2(50, 5), 90.0, 0.0, 110.0, 10.0, 100.0),
		"a point on neither side of a seam-straddling rect is outside"
	)


func _check_merge_prev() -> void:
	var prev_ids := PackedInt32Array([1, 3, 5])
	var prev_vals := PackedVector2Array([Vector2(1, 1), Vector2(3, 3), Vector2(5, 5)])
	var ids := PackedInt32Array([3, 4, 5, 6])
	var fallback := PackedVector2Array(
		[Vector2(30, 30), Vector2(40, 40), Vector2(50, 50), Vector2(60, 60)]
	)
	var out: PackedVector2Array = AgentLayer.merge_prev(prev_ids, prev_vals, ids, fallback)
	_check(out.size() == 4, "merge_prev returns one entry per current id")
	_check(out[0].is_equal_approx(Vector2(3, 3)), "id 3 matches prev id 3's value, got %s" % out[0])
	_check(out[1].is_equal_approx(Vector2(40, 40)), "id 4 (new) falls back, got %s" % out[1])
	_check(out[2].is_equal_approx(Vector2(5, 5)), "id 5 matches prev id 5's value, got %s" % out[2])
	_check(out[3].is_equal_approx(Vector2(60, 60)), "id 6 (new) falls back, got %s" % out[3])

	# No previous frame at all: every id falls back.
	var out_empty: PackedVector2Array = AgentLayer.merge_prev(
		PackedInt32Array(), PackedVector2Array(), ids, fallback
	)
	for i in out_empty.size():
		_check(
			out_empty[i].is_equal_approx(fallback[i]),
			"empty prev_ids falls back for every id (index %d)" % i
		)

	# Identical id sets: every entry matches, fallback unused entirely.
	var same_ids := PackedInt32Array([1, 3, 5])
	var out_same: PackedVector2Array = AgentLayer.merge_prev(
		prev_ids,
		prev_vals,
		same_ids,
		PackedVector2Array([Vector2(-1, -1), Vector2(-1, -1), Vector2(-1, -1)])
	)
	for i in out_same.size():
		_check(
			out_same[i].is_equal_approx(prev_vals[i]),
			"identical id sets match every entry, got %s at %d" % [out_same[i], i]
		)

	# A death (prev id 3 has no counterpart in ids) does not corrupt the
	# matches around it.
	var ids_no4 := PackedInt32Array([1, 5])
	var out_gap: PackedVector2Array = AgentLayer.merge_prev(
		prev_ids, prev_vals, ids_no4, PackedVector2Array([Vector2(-1, -1), Vector2(-1, -1)])
	)
	_check(out_gap[0].is_equal_approx(Vector2(1, 1)), "id 1 still matches across a gap")
	_check(out_gap[1].is_equal_approx(Vector2(5, 5)), "id 5 still matches across a gap")


func _init() -> void:
	_check_view_rect()
	_check_pos_in_rect()
	_check_merge_prev()

	if _failed:
		quit(1)
		return
	print("test_agent_layer: all passed")
	quit(0)
