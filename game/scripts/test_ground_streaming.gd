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
	_test_wrap_copies_view_wider_than_world()
	_test_wrap_copies_narrow_view_needs_none()
	_test_wrap_copies_cap_nearest_first()
	_test_missing_count()

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


# The fit framing of a 1024 world at 1x on a 1280x800 window: the view is
# 256 units wider than the world, so the chunks on its left and right edges
# are on screen twice. Every point of the view must be covered by a placed
# chunk or one of its copies, and no copy may repeat a placed one.
func _test_wrap_copies_view_wider_than_world() -> void:
	var world := 1024.0
	var chunk_world := 512.0
	var cam_pos := Vector2(512.0, 512.0)
	var view_size := Vector2(1280.0, 800.0)
	var placed := S.visible_chunks(cam_pos, view_size, world, chunk_world, 2, 1)
	var copies := S.wrap_copies(cam_pos, view_size, world, chunk_world, placed, 96)
	_check(copies.size() > 0, "a view wider than the world needs wrap copies")
	var rects: Array = []
	for c in placed + copies:
		rects.append(
			Rect2(
				Vector2(c[0] * chunk_world + c[2], c[1] * chunk_world + c[3]), Vector2.ONE * 512.0
			)
		)
	for c in copies:
		for p in placed:
			if int(p[0]) == int(c[0]) and int(p[1]) == int(c[1]):
				_check(c[2] != p[2] or c[3] != p[3], "a copy never repeats the placed chunk")
	var covered := true
	var x := cam_pos.x - view_size.x * 0.5 + 1.0
	while x < cam_pos.x + view_size.x * 0.5:
		var y := cam_pos.y - view_size.y * 0.5 + 1.0
		while y < cam_pos.y + view_size.y * 0.5:
			var hit := false
			for r in rects:
				if r.has_point(Vector2(x, y)):
					hit = true
					break
			covered = covered and hit
			y += 37.0
		x += 37.0
	_check(covered, "placed chunks plus their copies cover the whole view (no bare seam)")
	for c in copies:
		_check(
			(
				is_equal_approx(fposmod(c[2], world), 0.0)
				and is_equal_approx(fposmod(c[3], world), 0.0)
			),
			"copy offsets are whole worlds (%s)" % [c]
		)


# A view narrower than a world minus a chunk can never meet two copies of
# one chunk: no mirrors, whatever the camera.
func _test_wrap_copies_narrow_view_needs_none() -> void:
	var world := 4096.0
	var chunk_world := 512.0
	var cam_pos := Vector2(4000.0, 30.0)  # near both seams
	var view_size := Vector2(1280.0, 800.0)
	var placed := S.visible_chunks(cam_pos, view_size, world, chunk_world, 8, 1)
	var copies := S.wrap_copies(cam_pos, view_size, world, chunk_world, placed, 96)
	_check(copies.is_empty(), "a narrow view needs no wrap copies (got %d)" % copies.size())


# A far zoom-out on a small world asks for many copies: the cap keeps the
# ones nearest the camera.
func _test_wrap_copies_cap_nearest_first() -> void:
	var world := 128.0
	var chunk_world := 64.0
	var cam_pos := Vector2(64.0, 64.0)
	var view_size := Vector2(2000.0, 2000.0)
	var placed := S.visible_chunks(cam_pos, view_size, world, chunk_world, 2, 1)
	var all := S.wrap_copies(cam_pos, view_size, world, chunk_world, placed, 100000)
	var capped := S.wrap_copies(cam_pos, view_size, world, chunk_world, placed, 10)
	_check(all.size() > 10, "a far zoom-out asks for many copies (got %d)" % all.size())
	_check(capped.size() == 10, "the cap limits the copies")
	var far := 0.0
	for c in capped:
		var center := (
			Vector2(c[0] * chunk_world + c[2], c[1] * chunk_world + c[3]) + Vector2.ONE * 32.0
		)
		far = maxf(far, cam_pos.distance_to(center))
	var nearest_left_out := INF
	for i in range(10, all.size()):
		var c: Array = all[i]
		var center := (
			Vector2(c[0] * chunk_world + c[2], c[1] * chunk_world + c[3]) + Vector2.ONE * 32.0
		)
		nearest_left_out = minf(nearest_left_out, cam_pos.distance_to(center))
	_check(far <= nearest_left_out, "the capped copies are the ones nearest the camera")


func _test_missing_count() -> void:
	var resident := {Vector2i(0, 0): {"version": 1, "age": 0}}
	var view := [[0, 0, 0.0, 0.0], [1, 0, 0.0, 0.0], [1, 1, 0.0, 0.0]]
	_check(S.missing_count(resident, view) == 2, "two of three view chunks are missing")
	_check(S.missing_count(resident, [[0, 0, 0.0, 0.0]]) == 0, "a complete view is missing none")
