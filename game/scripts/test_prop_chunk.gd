extends SceneTree
# Headless unit test for the per-chunk prop planner. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_prop_chunk.gd
# Exits 0 on success, 1 on the first failed assertion.

const S = preload("res://scripts/terrain_scatter.gd")
const PC = preload("res://scripts/prop_chunk.gd")
const T = preload("res://scripts/terrain_sprites.gd")

const RES := 128
const WORLD := 1024.0
const HUGE_BUDGET := 1000000

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


# Horizontal bands, one per terrain id, so every terrain (and therefore every
# prop kind) appears somewhere in the grid.
func _banded_ids(res: int) -> PackedByteArray:
	var ids := PackedByteArray()
	ids.resize(res * res)
	for y in res:
		var id: int = mini(int(y * T.TERRAIN_COUNT / float(res)), T.TERRAIN_COUNT - 1)
		for x in res:
			ids[y * res + x] = id
	return ids


# Torus-wrapped 66x66 apron for chunk (cx, cy) sliced out of a res x res grid.
func _apron_for(ids: PackedByteArray, res: int, cx: int, cy: int) -> PackedByteArray:
	var apron := PackedByteArray()
	var side := PC.CHUNK_CELLS + 2
	apron.resize(side * side)
	for ay in side:
		var gy := posmod(cy * PC.CHUNK_CELLS + ay - 1, res)
		for ax in side:
			var gx := posmod(cx * PC.CHUNK_CELLS + ax - 1, res)
			apron[ay * side + ax] = ids[gy * res + gx]
	return apron


func _total(plan: Array) -> int:
	var n := 0
	for arr in plan:
		n += arr.size()
	return n


# Sort a PackedVector2Array's positions into a comparable, order-independent
# form (lexicographic by x then y).
func _sorted(arr: PackedVector2Array) -> Array:
	var out: Array = []
	for p in arr:
		out.append(p)
	out.sort_custom(func(a, b): return a.x < b.x or (a.x == b.x and a.y < b.y))
	return out


func _arrays_equal(a: Array, b: Array, eps: float) -> bool:
	if a.size() != b.size():
		return false
	for i in a.size():
		var pa: Vector2 = a[i]
		var pb: Vector2 = b[i]
		if not pa.is_equal_approx(pb) and pa.distance_to(pb) > eps:
			return false
	return true


func _init() -> void:
	var chunks_per_axis := RES / PC.CHUNK_CELLS
	var whole_ids := _banded_ids(RES)
	var whole_plan := S.plan(whole_ids, RES, WORLD, HUGE_BUDGET)

	# --- union of every chunk's plan equals the whole-world plan, per kind ---
	var union_buckets: Array = []
	for k in T.PROP_COUNT:
		union_buckets.append([])
	for cy in chunks_per_axis:
		for cx in chunks_per_axis:
			var apron := _apron_for(whole_ids, RES, cx, cy)
			var chunk_plan := PC.plan(cx, cy, apron, RES, WORLD)
			_check(chunk_plan.size() == T.PROP_COUNT, "chunk plan returns one array per prop kind")
			for k in T.PROP_COUNT:
				for p in chunk_plan[k] as PackedVector2Array:
					union_buckets[k].append(p)
	for k in T.PROP_COUNT:
		var union_sorted: Array = union_buckets[k]
		union_sorted.sort_custom(func(a, b): return a.x < b.x or (a.x == b.x and a.y < b.y))
		var whole_sorted := _sorted(whole_plan[k])
		_check(
			_arrays_equal(union_sorted, whole_sorted, 0.001),
			(
				"chunked union matches whole-world plan for kind %s (chunked=%d whole=%d)"
				% [T.PROP_NAMES[k], union_sorted.size(), whole_sorted.size()]
			)
		)
	_check(_total(whole_plan) > 0, "banded world places some props")

	# --- a chunk within the y-sorted result is actually sorted by y ---
	var sample_apron := _apron_for(whole_ids, RES, 0, 0)
	var sample_plan := PC.plan(0, 0, sample_apron, RES, WORLD)
	for k in T.PROP_COUNT:
		var positions: PackedVector2Array = sample_plan[k]
		for i in range(1, positions.size()):
			_check(positions[i - 1].y <= positions[i].y, "kind %d positions sorted by y" % k)

	# --- water-only chunk places nothing ---
	var water_ids := PackedByteArray()
	water_ids.resize(RES * RES)
	water_ids.fill(T.WATER)
	var water_apron := _apron_for(water_ids, RES, 0, 0)
	var water_plan := PC.plan(0, 0, water_apron, RES, WORLD)
	_check(_total(water_plan) == 0, "water-only chunk places no props")

	# --- determinism ---
	var again := PC.plan(0, 0, sample_apron, RES, WORLD)
	for k in T.PROP_COUNT:
		_check(sample_plan[k] == again[k], "chunk plan is deterministic (kind %d)" % k)

	# --- build / clear / set_wrap_offset lifecycle ---
	var chunk := PC.new()
	chunk.build(0, 0, sample_apron, RES, WORLD, Vector2.ZERO)
	var any_instances := false
	for mmi in chunk._mmis:
		if (mmi as MultiMeshInstance2D).multimesh.instance_count > 0:
			any_instances = true
	_check(any_instances, "build fills at least one multimesh")
	chunk.clear()
	for mmi in chunk._mmis:
		_check(
			(mmi as MultiMeshInstance2D).multimesh.instance_count == 0,
			"clear zeroes every multimesh instance count"
		)
	_check(chunk.position == Vector2.ZERO, "chunk starts at the given offset")
	chunk.set_wrap_offset(Vector2(512.0, -256.0))
	_check(chunk.position == Vector2(512.0, -256.0), "set_wrap_offset moves the node")
	chunk.free()

	if _failed:
		quit(1)
		return
	# Per-tree tint: within the swing, varies from tree to tree, opaque.
	var seen := {}
	for i in 40:
		var c: Color = PC.tree_tint(Vector2(i * 7.3, i * 4.1))
		_check(c.a == 1.0, "tree tint is opaque")
		_check(c.r > 1.0 - PC.TINT_SWING - PC.TINT_HUE - 0.02, "tree tint stays near white (r)")
		_check(c.b < 1.0 + PC.TINT_SWING + PC.TINT_HUE + 0.02, "tree tint stays near white (b)")
		seen[str(c)] = true
	_check(seen.size() >= 20, "tree tints vary from tree to tree (%d distinct)" % seen.size())
	var tint_a: Color = PC.tree_tint(Vector2(10, 10))
	var tint_b: Color = PC.tree_tint(Vector2(10, 10))
	_check(tint_a == tint_b, "tree tint is stable")
	if _failed:
		quit(1)
		return
	print("test_prop_chunk: all passed")
	quit(0)
