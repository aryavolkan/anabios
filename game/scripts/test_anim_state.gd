extends SceneTree
# Headless unit test for anim_state.gd (Phase 3 step 2, D6 — docs/superpowers/
# specs/2026-09-12-pixel-world-at-scale-design.md §6 Phase 3): the packed-
# array animation-state store meant to replace main.gd's `_facing`, `_gait`,
# `_locomotion`, `_actions` and `_birth_times` Dictionaries.
# Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_anim_state.gd
# Exits 0 on success, 1 on the first failed assertion.

const AnimState = preload("res://scripts/anim_state.gd")
const FxMath = preload("res://scripts/fx_math.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _ids(a: Array) -> PackedInt32Array:
	var p := PackedInt32Array()
	for v in a:
		p.append(v)
	return p


func _init() -> void:
	_test_initial_sync_allocates_with_defaults()
	_test_death_frees_and_lifo_reuses()
	_test_values_survive_across_syncs()
	_test_compact_shrinks_and_preserves_values()
	_test_large_population_and_half_death()
	_test_interleaved_two_pointer_merge()

	if _failed:
		quit(1)
		return
	print("test_anim_state: all passed")
	quit(0)


# sync([1,2,3]) on a fresh store allocates 3 dense slots (0,1,2), each seeded
# with the same defaults the Dictionaries use today: facing at (side 0, ease
# 0, heading 0), gait at FxMath.seed_gait(id), walk hold/weight at 0, action
# pose -1 / hold 0, birth_time == now, alive == 1.
func _test_initial_sync_allocates_with_defaults() -> void:
	var anim := AnimState.new()
	var now := 12.5
	var slots := anim.sync(_ids([1, 2, 3]), now)
	_check(slots.size() == 3, "slots parallel to ids has 3 entries")
	_check(anim.slot_count() == 3, "slot_count is 3 after first sync")
	_check(anim.capacity() == 3, "capacity is 3 after first sync (no frees yet)")
	var seen := {}
	for i in 3:
		var s: int = slots[i]
		_check(not seen.has(s), "slot %d not reused within one sync" % s)
		seen[s] = true
		_check(s >= 0 and s < 3, "slot %d in range" % s)
		_check(anim.alive[s] == 1, "fresh slot marked alive")
		_check(anim.facing_side[s] == 0, "fresh facing_side defaults to 0 (right)")
		_check(is_equal_approx(anim.facing_ease[s], 0.0), "fresh facing_ease defaults to 0.0")
		_check(is_equal_approx(anim.facing_heading[s], 0.0), "fresh facing_heading defaults to 0.0")
		var id: int = [1, 2, 3][i]
		_check(
			is_equal_approx(anim.gait[s], FxMath.seed_gait(id)),
			"fresh gait seeded via FxMath.seed_gait(id)"
		)
		_check(is_equal_approx(anim.walk_hold[s], 0.0), "fresh walk_hold defaults to 0.0")
		_check(is_equal_approx(anim.walk_weight[s], 0.0), "fresh walk_weight defaults to 0.0")
		_check(is_equal_approx(anim.action_pose[s], -1.0), "fresh action_pose defaults to -1.0")
		_check(is_equal_approx(anim.action_hold[s], 0.0), "fresh action_hold defaults to 0.0")
		_check(is_equal_approx(anim.birth_time[s], now), "fresh birth_time is now")
	_check(anim.slot_of.size() == 3, "slot_of has exactly 3 entries")
	for id in [1, 2, 3]:
		_check(anim.slot_of.has(id), "slot_of has id %d" % id)


# sync([1,3,4]) after [1,2,3]: id 2 died, freeing its slot; id 4 is new and
# reuses that freed slot (LIFO — the only slot on the free list). ids 1 and 3
# keep the exact slots they had before, and whatever was written into those
# slots survives.
func _test_death_frees_and_lifo_reuses() -> void:
	var anim := AnimState.new()
	var slots1 := anim.sync(_ids([1, 2, 3]), 0.0)
	var slot1: int = slots1[0]
	var slot2: int = slots1[1]
	var slot3: int = slots1[2]
	anim.gait[slot1] = 0.42
	anim.action_pose[slot3] = 7.0

	var slots2 := anim.sync(_ids([1, 3, 4]), 1.0)
	_check(slots2.size() == 3, "second sync returns 3 slots for 3 ids")
	var new_slot1: int = slots2[0]
	var new_slot3: int = slots2[1]
	var new_slot4: int = slots2[2]
	_check(new_slot1 == slot1, "id 1 keeps its slot across sync")
	_check(new_slot3 == slot3, "id 3 keeps its slot across sync")
	_check(new_slot4 == slot2, "id 4 reuses id 2's freed slot (LIFO, only slot free)")
	_check(is_equal_approx(anim.gait[new_slot1], 0.42), "id 1's written gait value survives")
	_check(is_equal_approx(anim.action_pose[new_slot3], 7.0), "id 3's written action_pose survives")
	_check(anim.alive[slot2] == 1, "reused slot marked alive again")
	_check(is_equal_approx(anim.action_pose[new_slot4], -1.0), "reused slot reset to defaults")
	_check(is_equal_approx(anim.gait[new_slot4], FxMath.seed_gait(4)), "reused slot gait reseeded")
	_check(not anim.slot_of.has(2), "dead id 2 removed from slot_of")
	_check(anim.slot_of.size() == 3, "slot_of has 3 live entries")
	_check(anim.capacity() == 3, "capacity unchanged: the freed slot was reused, not grown")


# Values written into a slot after sync() persist across further syncs that
# don't touch that id's liveness, exactly like a Dictionary entry would.
func _test_values_survive_across_syncs() -> void:
	var anim := AnimState.new()
	var slots := anim.sync(_ids([5]), 0.0)
	var s: int = slots[0]
	anim.facing_side[s] = 1
	anim.facing_ease[s] = 0.75
	anim.facing_heading[s] = -0.6
	anim.walk_hold[s] = 0.1
	anim.walk_weight[s] = 0.9
	anim.action_pose[s] = 3.0
	anim.action_hold[s] = 0.16

	for frame in 5:
		var slots_n := anim.sync(_ids([5]), float(frame))
		_check(slots_n[0] == s, "id 5 keeps the same slot every frame")

	_check(anim.facing_side[s] == 1, "facing_side value survives repeated syncs")
	_check(is_equal_approx(anim.facing_ease[s], 0.75), "facing_ease value survives")
	_check(is_equal_approx(anim.facing_heading[s], -0.6), "facing_heading value survives")
	_check(is_equal_approx(anim.walk_hold[s], 0.1), "walk_hold value survives")
	_check(is_equal_approx(anim.walk_weight[s], 0.9), "walk_weight value survives")
	_check(is_equal_approx(anim.action_pose[s], 3.0), "action_pose value survives")
	_check(is_equal_approx(anim.action_hold[s], 0.16), "action_hold value survives")


# compact() after many deaths renumbers live slots down to a dense run and
# shrinks capacity(), while every surviving id keeps its written values (its
# slot number may change, so re-look-up via slot_of).
func _test_compact_shrinks_and_preserves_values() -> void:
	var anim := AnimState.new()
	var all_ids := []
	for i in 20:
		all_ids.append(i)
	anim.sync(_ids(all_ids), 0.0)
	# Write a distinctive value per surviving id before killing most of them.
	var survivors := [0, 7, 19]
	for id in survivors:
		var s: int = int(anim.slot_of[id])
		anim.gait[s] = float(id) + 0.5

	# Kill everything except the survivors: free list should end up well over
	# half of capacity (20), which is compact()'s trigger condition.
	anim.sync(_ids(survivors), 1.0)
	var cap_before := anim.capacity()
	_check(cap_before == 20, "capacity unchanged by death alone (only slot_of shrinks)")
	_check(anim.slot_count() == 3, "slot_count reflects the 3 survivors")

	anim.compact()
	_check(anim.capacity() == 3, "compact() shrinks capacity down to the live count")
	_check(anim.slot_count() == 3, "slot_count still 3 after compact()")
	for id in survivors:
		_check(anim.slot_of.has(id), "compact() keeps survivor %d in slot_of" % id)
		var s: int = int(anim.slot_of[id])
		_check(s >= 0 and s < 3, "compact()'d slot for id %d is dense" % id)
		_check(
			is_equal_approx(anim.gait[s], float(id) + 0.5),
			"compact() preserves id %d's written gait value" % id
		)
		_check(anim.alive[s] == 1, "compact()'d slot marked alive")

	# compact() is a no-op once the free list is empty / small.
	anim.compact()
	_check(anim.capacity() == 3, "a second compact() with nothing to do is a no-op")

	# The store still works correctly after compact(): a further sync can add
	# new ids and drop the rest.
	var slots_after := anim.sync(_ids([0, 20]), 2.0)
	_check(slots_after.size() == 2, "sync after compact() still returns parallel slots")
	_check(anim.slot_of.has(0), "id 0 still tracked after compact() + further sync")
	_check(anim.slot_of.has(20), "new id 20 allocated after compact()")
	_check(not anim.slot_of.has(7), "id 7 (dropped this sync) is gone after compact()")


# A 10,000-id sync followed by a sync that kills every other id runs without
# error and returns a slots array parallel to ids in both cases.
func _test_large_population_and_half_death() -> void:
	var anim := AnimState.new()
	var all_ids := []
	for i in 10000:
		all_ids.append(i)
	var slots1 := anim.sync(_ids(all_ids), 0.0)
	_check(slots1.size() == 10000, "large sync returns 10000 slots")
	_check(anim.slot_count() == 10000, "slot_count is 10000 after large sync")

	var survivors := []
	for i in 10000:
		if i % 2 == 0:
			survivors.append(i)
	var slots2 := anim.sync(_ids(survivors), 1.0)
	_check(slots2.size() == survivors.size(), "half-death sync returns parallel slots")
	_check(anim.slot_count() == survivors.size(), "slot_count halves after killing odd ids")
	for i in survivors.size():
		var s: int = slots2[i]
		_check(anim.alive[s] == 1, "surviving slot still marked alive")
		_check(int(anim.slot_of[survivors[i]]) == s, "slot_of agrees with returned slot")


# Two-pointer correctness when prev and current id sets interleave rather
# than nest: prev [2,4,6] -> now [1,2,3,6,7] should free 4, keep 2 and 6 on
# their existing slots, and allocate fresh slots for 1, 3 and 7.
func _test_interleaved_two_pointer_merge() -> void:
	var anim := AnimState.new()
	var slots1 := anim.sync(_ids([2, 4, 6]), 0.0)
	var slot2: int = slots1[0]
	var slot4: int = slots1[1]
	var slot6: int = slots1[2]
	anim.gait[slot2] = 0.2
	anim.gait[slot6] = 0.6

	var slots2 := anim.sync(_ids([1, 2, 3, 6, 7]), 1.0)
	_check(slots2.size() == 5, "interleaved sync returns 5 slots")
	var s1: int = slots2[0]
	var s2: int = slots2[1]
	var s3: int = slots2[2]
	var s6: int = slots2[3]
	var s7: int = slots2[4]
	_check(s2 == slot2, "id 2 keeps its slot across an interleaved merge")
	_check(s6 == slot6, "id 6 keeps its slot across an interleaved merge")
	_check(is_equal_approx(anim.gait[s2], 0.2), "id 2's value survives the interleaved merge")
	_check(is_equal_approx(anim.gait[s6], 0.6), "id 6's value survives the interleaved merge")
	_check(not anim.slot_of.has(4), "id 4 (dropped) freed by the interleaved merge")
	# id 4's slot is the only one freed, so it is the one new ids reuse (LIFO);
	# with three new ids (1, 3, 7) and one free slot, exactly one of them
	# lands on slot4 and the other two grow capacity — which one depends only
	# on iteration order, so just check every slot is valid and distinct.
	var seen := {}
	for s in [s1, s2, s3, s6, s7]:
		_check(not seen.has(s), "slot %d used by only one id this frame" % s)
		seen[s] = true
	_check(anim.slot_of.has(1) and anim.slot_of.has(3) and anim.slot_of.has(7), "new ids tracked")
	_check(int(anim.slot_of[1]) == s1, "slot_of agrees with returned slot for id 1")
	_check(int(anim.slot_of[3]) == s3, "slot_of agrees with returned slot for id 3")
	_check(int(anim.slot_of[7]) == s7, "slot_of agrees with returned slot for id 7")
	_check(slot4 == s1 or slot4 == s3 or slot4 == s7, "id 4's freed slot was reused by a new id")
