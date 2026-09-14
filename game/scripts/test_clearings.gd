extends SceneTree
# Headless unit test for the village-clearing registry. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_clearings.gd
# Exits 0 on success, 1 on the first failed assertion.

const C = preload("res://scripts/clearings.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	C.reset()
	# Nothing published: filter returns the input untouched.
	var pts := PackedVector2Array([Vector2(10, 10), Vector2(100, 100)])
	_check(C.filter(pts) == pts, "empty registry filters nothing")
	_check(C.version == 0, "fresh registry at version 0")

	# Bounds grow by the margin and snap outwards to the grid.
	var b := C.bounds_of(PackedVector2Array([Vector2(20, 30), Vector2(52, 62)]), 14.0)
	_check(b.has_point(Vector2(20 - 14, 30 - 14)), "bounds include the margin")
	_check(b.has_point(Vector2(52 + 13, 62 + 13)), "bounds include the far margin")
	_check(is_equal_approx(fmod(b.position.x, C.SNAP), 0.0), "bounds snap to the grid")
	_check(C.bounds_of(PackedVector2Array(), 14.0).size == Vector2.ZERO, "empty bounds")

	# Publishing filters; republishing the same set (any order) keeps version.
	var a := Rect2(0, 0, 64, 64)
	var d := Rect2(200, 200, 32, 32)
	C.publish("villages", [a, d] as Array[Rect2])
	_check(C.version == 1, "first publish bumps the version")
	var kept := C.filter(
		PackedVector2Array([Vector2(10, 10), Vector2(100, 100), Vector2(210, 210)])
	)
	_check(kept == PackedVector2Array([Vector2(100, 100)]), "points inside clearings are dropped")
	C.publish("villages", [d, a] as Array[Rect2])
	_check(C.version == 1, "same set in another order is not a change")
	# A second source merges rather than replacing.
	C.publish("hubs", [Rect2(90, 90, 20, 20)] as Array[Rect2])
	_check(C.version == 2, "a new source bumps the version")
	_check(C.contains(Vector2(100, 100)), "hub clearing merged in")
	_check(C.contains(Vector2(10, 10)), "village clearing still there")
	C.publish("villages", [] as Array[Rect2])
	_check(C.version == 3 and not C.contains(Vector2(10, 10)), "a source can clear its set")
	_check(C.contains(Vector2(100, 100)), "other sources survive a clear")
	# A drift smaller than a snap step publishes the same rectangle.
	var r1 := C.bounds_of(PackedVector2Array([Vector2(20, 30)]), 14.0)
	var r2 := C.bounds_of(PackedVector2Array([Vector2(20.4, 30.3)]), 14.0)
	_check(r1 == r2, "sub-snap drift is absorbed")

	# Road strips clear a band either side of the segment, and merge with
	# the rectangles under their own source.
	C.reset()
	C.publish("villages", [Rect2(0, 0, 10, 10)] as Array[Rect2])
	var v1 := C.version
	C.publish_segments("roads", [PackedVector2Array([Vector2(100, 100), Vector2(200, 100)])])
	_check(C.version == v1 + 1, "publishing road strips bumps the version")
	_check(C.contains(Vector2(150, 100 + C.ROAD_HALF - 0.5)), "inside the road band")
	_check(not C.contains(Vector2(150, 100 + C.ROAD_HALF + 0.5)), "outside the road band")
	_check(not C.contains(Vector2(210, 100)), "past the road's end")
	_check(C.contains(Vector2(5, 5)), "village rectangle survives the road publish")
	C.publish_segments("roads", [PackedVector2Array([Vector2(100, 100), Vector2(200, 100)])])
	_check(C.version == v1 + 1, "the same strips are not a change")
	C.publish_segments("roads", [] as Array[PackedVector2Array])
	_check(not C.contains(Vector2(150, 100)), "roads can be cleared")

	C.reset()
	if _failed:
		quit(1)
	else:
		print("test_clearings: all passed")
		quit(0)
