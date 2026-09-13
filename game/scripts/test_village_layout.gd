extends SceneTree
# Headless unit test for the village layout planner (pure logic; village_layer
# consumes plan() output through a scene, exercised elsewhere).
# Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_village_layout.gd
# Exits 0 on success, 1 on the first failed assertion.

const L = preload("res://scripts/village_layout.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _no_water(_pos: Vector2) -> bool:
	return false


func _count_kind(result: Array, kind: int) -> int:
	var n := 0
	for p in result:
		if int(p["kind"]) == kind:
			n += 1
	return n


func _cell_of(anchor: Vector2, pos: Vector2) -> Vector2i:
	return Vector2i(
		int(round((pos.x - anchor.x) / L.GRID)), int(round((pos.y - anchor.y) / L.GRID))
	)


func _init() -> void:
	var no_water := Callable(self, "_no_water")

	# --- spiral(): first ring, clockwise starting east ---
	var expected_ring1: Array[Vector2i] = [
		Vector2i(1, 0),
		Vector2i(1, 1),
		Vector2i(0, 1),
		Vector2i(-1, 1),
		Vector2i(-1, 0),
		Vector2i(-1, -1),
		Vector2i(0, -1),
		Vector2i(1, -1),
	]
	_check(L.spiral(8) == expected_ring1, "spiral(8) matches the expected ring-1 sequence")
	_check(L.spiral(0).is_empty(), "spiral(0) is empty")

	# --- bounds() ---
	var some_cells: Array[Vector2i] = [Vector2i(1, 2), Vector2i(-3, 5), Vector2i(0, 0)]
	_check(L.bounds(some_cells) == Rect2i(-3, 0, 5, 6), "bounds() covers min/max inclusive")
	_check(L.bounds([]) == Rect2i(), "bounds() of no cells is empty")

	# --- determinism: same inputs -> identical output ---
	var anchor_a := Vector2(1000.0, 1000.0)
	var flags_all := (
		L.FLAG_TERRITORY
		| L.FLAG_WAR
		| L.FLAG_FARMING
		| L.FLAG_MACHINERY
		| L.FLAG_METALWORKING
		| L.FLAG_WRITING
		| L.FLAG_RAIDED
	)
	var plan_a := L.plan(42, anchor_a, 50, 2, flags_all, no_water)
	var plan_a2 := L.plan(42, anchor_a, 50, 2, flags_all, no_water)
	_check(plan_a == plan_a2, "plan() is deterministic for identical inputs")

	# --- determinism: different sid -> different output ---
	var plan_sid1 := L.plan(1, Vector2.ZERO, 10, 1, 0, no_water)
	var plan_sid2 := L.plan(2, Vector2.ZERO, 10, 1, 0, no_water)
	_check(plan_sid1 != plan_sid2, "different sid changes flips/positions")

	# --- no two placements share a cell (rich scenario, some cells water) ---
	var stress_anchor := Vector2(2000.0, 2000.0)
	var west_water := func(pos: Vector2) -> bool: return pos.x < stress_anchor.x - 40.0
	var stress := L.plan(7, stress_anchor, 50, 2, flags_all, west_water)
	_check(not stress.is_empty(), "stress scenario places something")
	var seen: Dictionary = {}
	for p in stress:
		var cell := _cell_of(stress_anchor, p["pos"])
		_check(not seen.has(cell), "cell %s is used by only one placement" % cell)
		seen[cell] = true

	# --- output sorted by y ascending, ties by x ---
	for i in range(1, stress.size()):
		var pa: Vector2 = stress[i - 1]["pos"]
		var pb: Vector2 = stress[i]["pos"]
		var ok := pa.y < pb.y or (pa.y == pb.y and pa.x <= pb.x)
		_check(ok, "placement %d..%d stays sorted by (y, x)" % [i - 1, i])

	# --- no land structure on water ---
	var half_anchor := Vector2(500.0, 500.0)
	var half_water := func(pos: Vector2) -> bool: return pos.x < half_anchor.x - 40.0
	var half := L.plan(3, half_anchor, 40, 1, L.FLAG_FARMING, half_water)
	for p in half:
		_check(not bool(half_water.call(p["pos"])), "placement at %s avoids water" % p["pos"])

	# --- everything is water: no errors, nothing placed ---
	var all_water := func(_pos: Vector2) -> bool: return true
	var drowned := L.plan(4, Vector2.ZERO, 50, 2, flags_all, all_water)
	_check(drowned.is_empty(), "an all-water site places nothing")

	# --- era 0 (camp): tents + hearth, never a hut ---
	var camp := L.plan(5, Vector2.ZERO, 10, 0, 0, no_water)
	_check(_count_kind(camp, L.HEARTH) == 1, "era 0 has exactly one hearth")
	_check(_count_kind(camp, L.HUT) == 0, "era 0 places no huts")
	_check(
		_count_kind(camp, L.TENT) + _count_kind(camp, L.TENT_B) == clampi(1 + 10 / 8, 1, 6),
		"era 0 tent count matches formula"
	)
	var big_camp := L.plan(12, Vector2.ZERO, 60, 0, 0, no_water)
	_check(_count_kind(big_camp, L.FIELD) == 2, "a big camp tends a two-cell plot")
	_check(
		_count_kind(big_camp, L.FENCE_H) + _count_kind(big_camp, L.FENCE_V) >= 6,
		"the plot is fenced"
	)
	_check(_count_kind(big_camp, L.GRANARY) == 0, "no granary before farming")
	_check(_count_kind(camp, L.FIELD) == 0, "a small camp has no plot")
	_check(
		_count_kind(big_camp, L.TENT) > 0 and _count_kind(big_camp, L.TENT_B) > 0,
		"a large camp mixes tents and yurts"
	)

	# --- era 1, farming, members 30: >= 2 fields, fully fenced ---
	var farm_anchor := Vector2(300.0, 300.0)
	var farm := L.plan(6, farm_anchor, 30, 1, L.FLAG_FARMING, no_water)
	var cellkind: Dictionary = {}
	for p in farm:
		cellkind[_cell_of(farm_anchor, p["pos"])] = int(p["kind"])
	var field_cells: Array[Vector2i] = []
	for cell in cellkind.keys():
		if cellkind[cell] == L.FIELD:
			field_cells.append(cell)
	_check(field_cells.size() >= 2, "farming era-1 village has >= 2 fields")
	var block := L.bounds(field_cells)
	var offsets: Array[Vector2i] = [
		Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)
	]
	for fc in field_cells:
		for off in offsets:
			var nb := fc + off
			if block.has_point(nb):
				continue
			var k: int = int(cellkind.get(nb, -1))
			_check(
				k == L.FENCE_H or k == L.FENCE_V or k == L.FIELD,
				"field %s neighbour %s outside the block is a fence or field (got %d)" % [fc, nb, k]
			)

	# --- era 1 huts mix the three silhouettes ---
	var big := L.plan(11, Vector2.ZERO, 80, 1, 0, no_water)
	var hut_kinds: Dictionary = {}
	for p in big:
		if L.HUT_KINDS.has(int(p["kind"])):
			hut_kinds[int(p["kind"])] = true
	_check(hut_kinds.size() >= 2, "a large era-1 village mixes hut silhouettes")

	# --- a fortified farming village walls its huts and keeps the fields outside ---
	var fort := L.plan(13, Vector2.ZERO, 60, 1, L.FLAG_TERRITORY | L.FLAG_FARMING, no_water)
	var wall_x := 0
	for p in fort:
		if int(p["kind"]) == L.PALISADE_V:
			wall_x = maxi(wall_x, _cell_of(Vector2.ZERO, p["pos"]).x)
	_check(wall_x >= 2, "the wall stands east of the huts")
	for p in fort:
		var c := _cell_of(Vector2.ZERO, p["pos"])
		var k: int = int(p["kind"])
		if L.HUT_KINDS.has(k):
			_check(absi(c.x) < wall_x and absi(c.y) < wall_x, "hut %s is inside the wall" % c)
		elif k == L.FIELD:
			_check(c.x > wall_x, "field %s lies outside the wall" % c)

	# --- FLAG_WAR: exactly one gate, at least one tower, 4 corners ---
	var war := L.plan(8, Vector2.ZERO, 5, 1, L.FLAG_WAR, no_water)
	_check(_count_kind(war, L.GATE) == 1, "war village has exactly one gate")
	_check(_count_kind(war, L.TOWER) >= 1, "war village has at least one tower")
	_check(_count_kind(war, L.PALISADE_CORNER) == 4, "war village has all four palisade corners")

	# --- MILL: appears only when a water-adjacent land cell exists in range ---
	var river := func(pos: Vector2) -> bool: return pos.x >= 48.0 and pos.x < 64.0
	var milled := L.plan(9, Vector2.ZERO, 5, 2, L.FLAG_MACHINERY, river)
	_check(_count_kind(milled, L.MILL) == 1, "machinery + nearby water places one mill")
	var mill_pos := Vector2.ZERO
	for p in milled:
		if int(p["kind"]) == L.MILL:
			mill_pos = p["pos"]
	# The bank column is x = 2 (the river fills column 3); the hut lattice
	# takes (2, 0), so the mill lands on the nearest free bank cell beside it.
	_check(
		mill_pos.x == 32.0 and absf(mill_pos.y) <= 16.0,
		"mill sits on the nearest free water-adjacent land cell (got %s)" % mill_pos
	)
	var dry := L.plan(9, Vector2.ZERO, 5, 2, L.FLAG_MACHINERY, no_water)
	_check(_count_kind(dry, L.MILL) == 0, "machinery with no water in range places no mill")

	# --- FLAG_RAIDED: up to 2 huts/tents become ruins ---
	var raided := L.plan(10, Vector2.ZERO, 30, 1, L.FLAG_RAIDED, no_water)
	var ruins := _count_kind(raided, L.RUIN_BURNT)
	_check(ruins >= 1 and ruins <= 2, "raid burns between 1 and 2 structures (got %d)" % ruins)
	var calm := L.plan(10, Vector2.ZERO, 30, 1, 0, no_water)
	_check(_count_kind(calm, L.RUIN_BURNT) == 0, "an unraided village has no ruins")

	if _failed:
		quit(1)
		return
	print("test_village_layout: all passed")
	quit(0)
