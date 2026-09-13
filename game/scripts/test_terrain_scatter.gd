extends SceneTree
# Headless unit test for the decoration scatter planner (pure logic; the
# scene half of terrain_scatter.gd is exercised by the main.tscn boot smoke).
# Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_terrain_scatter.gd
# Exits 0 on success, 1 on the first failed assertion.

const S = preload("res://scripts/terrain_scatter.gd")
const T = preload("res://scripts/terrain_sprites.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _uniform_ids(res: int, id: int) -> PackedByteArray:
	var ids := PackedByteArray()
	ids.resize(res * res)
	ids.fill(id)
	return ids


func _total(plan: Array) -> int:
	var n := 0
	for arr in plan:
		n += arr.size()
	return n


func _init() -> void:
	var res := 32
	var world := 256.0

	# --- shape: one position array per prop kind ---
	var forest := S.plan(_uniform_ids(res, T.FOREST), res, world, 100000)
	_check(forest.size() == T.PROP_COUNT, "plan returns one array per prop kind")

	# --- determinism: same inputs, same plan ---
	var again := S.plan(_uniform_ids(res, T.FOREST), res, world, 100000)
	for k in T.PROP_COUNT:
		_check(forest[k] == again[k], "plan is deterministic (kind %d)" % k)

	# --- terrain -> prop mapping: all-forest places only forest props ---
	var forest_kinds: Array = T.props_for_terrain(T.FOREST)
	var placed_total := 0
	for k in T.PROP_COUNT:
		placed_total += forest[k].size()
		if forest_kinds.has(k):
			_check(forest[k].size() > 0, "forest world places %s" % T.PROP_NAMES[k])
		else:
			_check(forest[k].size() == 0, "forest world places no %s" % T.PROP_NAMES[k])
	_check(forest[T.OAK].size() > 0, "forest world places oaks")

	# --- density sanity: a plausible fraction of cells, not none, not all ---
	var frac: float = placed_total / float(res * res)
	_check(frac > 0.02 and frac < 0.40, "forest density plausible (frac=%.3f)" % frac)

	# --- bounds: every position inside [0, world) ---
	for pos in forest[T.OAK]:
		if pos.x < 0.0 or pos.x >= world or pos.y < 0.0 or pos.y >= world:
			_check(false, "position %s outside world" % pos)
			break

	# --- water stays bare ---
	var water := S.plan(_uniform_ids(res, T.WATER), res, world, 100000)
	_check(_total(water) == 0, "water world places nothing")

	# --- desert places cacti ---
	var desert := S.plan(_uniform_ids(res, T.DESERT), res, world, 100000)
	_check(
		desert[T.CACTUS].size() > 0 and _total(desert) == desert[T.CACTUS].size(),
		"desert world places only cacti"
	)

	# --- budget is a hard cap and stays deterministic ---
	var capped := S.plan(_uniform_ids(res, T.FOREST), res, world, 10)
	_check(_total(capped) <= 10, "budget caps total props (%d)" % _total(capped))
	var capped2 := S.plan(_uniform_ids(res, T.FOREST), res, world, 10)
	for k in T.PROP_COUNT:
		_check(capped[k] == capped2[k], "budgeted plan is deterministic (kind %d)" % k)

	if _failed:
		quit(1)
		return
	print("test_terrain_scatter: all passed")
	quit(0)
