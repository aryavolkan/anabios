extends SceneTree
# Headless unit test for the streamed-ground planning logic (pure logic; the
# scene half of ground_layer.gd/ground_chunk.gd is exercised by the main.tscn
# boot smoke). Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_ground_streaming.gd
# Exits 0 on success, 1 on the first failed assertion.

const S = preload("res://scripts/ground_streaming.gd")

var _failed := false
var _version_map: Dictionary = {}


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _version_for(cx: int, cy: int) -> int:
	return int(_version_map.get(Vector2i(cx, cy), 0))


# [cx, cy] pairs, unordered, from a visible_chunks() result.
func _indices(chunks: Array) -> Array:
	var out: Array = []
	for c in chunks:
		out.append(Vector2i(int(c[0]), int(c[1])))
	return out


func _has_index(chunks: Array, cx: int, cy: int) -> bool:
	return _indices(chunks).has(Vector2i(cx, cy))


func _unique_count(chunks: Array) -> int:
	var seen := {}
	for c in chunks:
		seen[Vector2i(int(c[0]), int(c[1]))] = true
	return seen.size()


# True iff `got` (an Array of [cx, cy] Arrays) equals `expected` (an Array of
# Vector2i) in order — written out explicitly rather than relying on nested
# Array `==` semantics.
func _upload_order_matches(got: Array, expected: Array) -> bool:
	if got.size() != expected.size():
		return false
	for i in got.size():
		var e: Vector2i = expected[i]
		if int(got[i][0]) != e.x or int(got[i][1]) != e.y:
			return false
	return true


func _init() -> void:
	_test_visible_chunks_centered_high_zoom()
	_test_visible_chunks_wraps_negative_offsets()
	_test_visible_chunks_view_wider_than_world()
	_test_plan_uploads()

	if _failed:
		quit(1)
		return
	print("test_ground_streaming: all passed")
	quit(0)


# A camera parked exactly at the shared corner of a 2x2-chunk (4-chunk) world,
# zoomed in enough that the view rect is tiny but still straddles the corner
# on both axes: all 4 chunks intersect, none wrapped (every offset is zero).
func _test_visible_chunks_centered_high_zoom() -> void:
	var world := 128.0
	var chunk_world := 64.0
	var chunk_count := 2
	var cam_pos := Vector2(64.0, 64.0)  # exact corner where all 4 chunks meet
	var view_size := Vector2(20.0, 20.0)  # high zoom: [54,74] x [54,74]

	var got := S.visible_chunks(cam_pos, view_size, world, chunk_world, chunk_count, 0)
	_check(got.size() == 4, "centered corner view sees all 4 chunks (got %d)" % got.size())
	_check(_unique_count(got) == got.size(), "no chunk index repeated")
	for cy in range(2):
		for cx in range(2):
			_check(_has_index(got, cx, cy), "chunk (%d,%d) present" % [cx, cy])
	for c in got:
		_check(c[2] == 0.0 and c[3] == 0.0, "zero offset for chunk %s" % [c])


# A camera near the world origin; the far chunk's nearest wrapped copy sits
# on the negative side of the origin, so it must appear with a negative
# offset rather than being dropped or duplicated.
func _test_visible_chunks_wraps_negative_offsets() -> void:
	var world := 512.0
	var chunk_world := 128.0
	var chunk_count := 4
	var cam_pos := Vector2(5.0, 5.0)
	var view_size := Vector2(300.0, 300.0)  # [-145,155] x [-145,155]

	var got := S.visible_chunks(cam_pos, view_size, world, chunk_world, chunk_count, 0)
	_check(_unique_count(got) == got.size(), "no chunk index repeated near the seam")

	var found_wrapped := false
	for c in got:
		if int(c[0]) == 3 and int(c[1]) == 0:
			_check(c[2] == -512.0, "far chunk (3,0) wraps to offset -512 (got %s)" % [c[2]])
			found_wrapped = true
	_check(found_wrapped, "far chunk (3,0) is present via its wrapped copy")


# A view wider than the whole world must still yield every chunk exactly
# once (each index has exactly one wrapped copy nearest the camera, and that
# copy always falls inside a view already wider than one world).
func _test_visible_chunks_view_wider_than_world() -> void:
	var world := 128.0
	var chunk_world := 64.0
	var chunk_count := 2
	var cam_pos := Vector2(37.0, 91.0)  # off-center, to stress the wrap math
	var view_size := Vector2(500.0, 500.0)

	var got := S.visible_chunks(cam_pos, view_size, world, chunk_world, chunk_count, 0)
	_check(
		got.size() == chunk_count * chunk_count,
		"every chunk returned exactly once (got %d)" % got.size()
	)
	_check(_unique_count(got) == got.size(), "no chunk index repeated")
	for cy in range(chunk_count):
		for cx in range(chunk_count):
			_check(_has_index(got, cx, cy), "chunk (%d,%d) present" % [cx, cy])


func _test_plan_uploads() -> void:
	_version_map = {
		Vector2i(0, 0): 1,  # unchanged
		Vector2i(1, 0): 2,  # changed, age 3
		Vector2i(0, 1): 2,  # changed, age 9 (older than (1,0))
	}
	var resident := {
		Vector2i(0, 0): {"version": 1, "age": 5},
		Vector2i(1, 0): {"version": 1, "age": 3},
		Vector2i(0, 1): {"version": 1, "age": 9},
		Vector2i(2, 2): {"version": 1, "age": 1},  # not wanted this frame
	}
	var wanted := [[0, 0, 0.0, 0.0], [1, 0, 0.0, 0.0], [0, 1, 0.0, 0.0], [1, 1, 0.0, 0.0]]  # (1,1) missing
	var versions := Callable(self, "_version_for")

	# Budget 2: the missing chunk (highest priority) and the older of the two
	# version-changed chunks fit; the younger version-changed chunk is
	# deferred (stays resident, reported as kept) rather than dropped.
	var plan := S.plan_uploads(resident, wanted, versions, 2)
	var evict: Array = plan["evict"]
	_check(evict.size() == 1 and evict[0] == Vector2i(2, 2), "non-wanted resident chunk is evicted")
	_check(
		_upload_order_matches(plan["upload"], [Vector2i(1, 1), Vector2i(0, 1)]),
		(
			"missing chunk then the older version-changed chunk upload, oldest first (got %s)"
			% [plan["upload"]]
		)
	)
	var keep_set := {}
	for k in plan["keep"]:
		keep_set[k] = true
	_check(keep_set.has(Vector2i(0, 0)), "unchanged chunk is kept")
	_check(keep_set.has(Vector2i(1, 0)), "budget-deferred changed chunk stays resident (kept)")
	_check(not keep_set.has(Vector2i(0, 1)), "uploaded chunk is not also reported as kept")
	_check(plan["keep"].size() == 2, "keep has exactly the two non-uploaded resident+wanted chunks")

	# Budget 0: nothing uploads; both changed chunks are deferred (kept).
	var plan0 := S.plan_uploads(resident, wanted, versions, 0)
	var upload0: Array = plan0["upload"]
	_check(upload0.size() == 0, "budget 0 uploads nothing")
	var keep0 := {}
	for k in plan0["keep"]:
		keep0[k] = true
	_check(
		keep0.has(Vector2i(0, 0)) and keep0.has(Vector2i(1, 0)) and keep0.has(Vector2i(0, 1)),
		"budget 0 keeps every resident+wanted chunk"
	)

	# Budget large enough for everything: every candidate uploads, no keep
	# entries besides the already-unchanged chunk.
	var plan_all := S.plan_uploads(resident, wanted, versions, 10)
	_check(
		_upload_order_matches(plan_all["upload"], [Vector2i(1, 1), Vector2i(0, 1), Vector2i(1, 0)]),
		(
			"unlimited budget uploads every candidate, still oldest first (got %s)"
			% [plan_all["upload"]]
		)
	)
	var keep_all: Array = plan_all["keep"]
	_check(
		keep_all.size() == 1 and keep_all[0] == Vector2i(0, 0), "only the unchanged chunk is kept"
	)
