extends RefCounted
# Phase 3 step 2 (D6 — docs/superpowers/specs/2026-09-12-pixel-world-at-scale-
# design.md §6 Phase 3): per-agent animation state, out of Dictionaries and
# into slot-indexed Packed*Arrays. Replaces main.gd's `_facing`, `_gait`,
# `_locomotion` and `_actions` Dictionaries (plus the `_birth_times` Dictionary
# a slot's `birth_time` folds in) with one dense store a stable integer slot
# indexes into — no per-frame Dictionary hit for 3k+ live agents, only the
# handful of birth/death slot changes each frame touch `slot_of` at all.
#
# How `_refresh_bodies` is meant to use this (three lines, once the caller
# already has this frame's ascending alive `ids: PackedInt32Array` and `now`):
#
#   var slots := _anim.sync(ids, now)
#   # for each visible index i (the same bucket loop as today):
#   var s := slots[i]
#   # ...read _anim.facing_side[s] / .facing_ease[s] / .facing_heading[s] /
#   # .gait[s] / .walk_hold[s] / .walk_weight[s] / .action_pose[s] /
#   # .action_hold[s] / .birth_time[s] as "previous" state, feed the FxMath
#   # step_* functions the same way main.gd's `.get(id, default)` calls do
#   # today, and write the result straight back into the same slot (e.g.
#   # `_anim.gait[s] = FxMath.advance_gait(...)`) — no dictionary writes.
#
# `slot_of` (agent id -> slot) is the only Dictionary here, and `sync()`
# touches it only for the ids that were born or died this frame; every other
# read/write goes straight through the Packed*Arrays above, indexed by the
# `slots` array `sync()` hands back (parallel to the `ids` passed in).
#
# One deliberate difference from the Dictionaries: `_facing`'s Dictionary
# default in main.gd is dynamic — `Vector3(face_left, face_left, cx)`, built
# from the agent's *current* heading at the point of the `.get()` call, which
# only `_refresh_bodies` has (this module's `sync()` runs before that, and
# only ever sees `ids` and `now`). A freshly allocated slot here seeds facing
# to (side 0 / right, ease 0.0, heading 0.0) instead; the difference is
# invisible after the first `step_facing` call on that slot folds the real
# heading in, so it costs a newborn at most one frame of extra ease-in on its
# very first facing update. Every other default below is exactly what the
# Dictionaries use today (see the call sites `sync()`'s doc points at).

# Per-id facing: committed side (0 right / 1 left), eased mix value, and the
# low-passed heading x FxMath.step_facing tracks. Mirrors main.gd's `_facing`
# Dictionary; see FxMath.step_facing for the per-frame update.
var facing_side: PackedInt32Array = PackedInt32Array()
var facing_ease: PackedFloat32Array = PackedFloat32Array()
var facing_heading: PackedFloat32Array = PackedFloat32Array()
# Per-id gait cycle position (0..1), seeded by FxMath.seed_gait(id) so a
# freshly seen crowd spreads around the loop instead of marching in step —
# exactly the default main.gd's `_gait.get(gid, FxMath.seed_gait(gid))` falls
# back to. Advanced by FxMath.advance_gait.
var gait: PackedFloat32Array = PackedFloat32Array()
# Per-id locomotion state: FxMath.step_locomotion's (hold credit, blended walk
# weight) pair, split into two arrays. Mirrors `_locomotion`'s Vector2.ZERO
# default.
var walk_hold: PackedFloat32Array = PackedFloat32Array()
var walk_weight: PackedFloat32Array = PackedFloat32Array()
# Per-id action state: FxMath.step_action's (current pose, remaining hold)
# pair, split into two arrays. Mirrors `_actions`' Vector2(-1.0, 0.0) default
# (pose -1 means "no action committed yet").
var action_pose: PackedFloat32Array = PackedFloat32Array()
var action_hold: PackedFloat32Array = PackedFloat32Array()
# Per-id birth time (Time.get_ticks_msec()/1000.0 at first sighting), used by
# the birth-pop scale ramp. Mirrors `_birth_times`.
var birth_time: PackedFloat32Array = PackedFloat32Array()
# 1 while the slot is in use, 0 once freed (and until reused). Lets a caller
# sanity-check a slot without a Dictionary lookup; sync()/compact() are the
# only writers.
var alive: PackedByteArray = PackedByteArray()

# agent id -> slot. The only Dictionary; touched only on birth/death inside
# sync() (and rebuilt wholesale by compact()) — never per visible agent.
var slot_of: Dictionary = {}

# Free slot indices, LIFO (a just-freed slot is the next one reused, matching
# the spec's "free list LIFO" requirement — and it's simply what appending to
# / popping from the back of a PackedInt32Array gives for free).
var _free: PackedInt32Array = PackedInt32Array()

# Last frame's ascending alive-id array, kept internally so sync() can two-
# pointer merge against it the same way main.gd's `_refresh_bodies` merges
# `ids` against `_prev_ids` today.
var _prev_ids: PackedInt32Array = PackedInt32Array()

const _DEFAULT_ACTION_POSE := -1.0

const FxMath = preload("res://scripts/fx_math.gd")


# Number of slots currently in use (== slot_of.size()).
func slot_count() -> int:
	return slot_of.size()


# Total slot storage, in use or free (== every Packed*Array's length).
func capacity() -> int:
	return alive.size()


# Reconcile this frame's ascending alive `ids` against last frame's, freeing
# slots for ids that vanished and allocating fresh ones (seeded with the
# defaults documented above) for ids seen for the first time. Returns a
# `slots` array parallel to `ids`: `slots[i]` is `ids[i]`'s slot this frame.
# O(n) via the same two-pointer merge main.gd's `_refresh_bodies` already
# uses to match this frame's ids against the previous frame's.
func sync(ids: PackedInt32Array, now: float) -> PackedInt32Array:
	var n := ids.size()
	var slots := PackedInt32Array()
	slots.resize(n)
	var p := 0
	var pn := _prev_ids.size()
	for i in n:
		var id := ids[i]
		while p < pn and _prev_ids[p] < id:
			_release(_prev_ids[p])
			p += 1
		if p < pn and _prev_ids[p] == id:
			slots[i] = int(slot_of[id])
			p += 1
		else:
			slots[i] = _acquire(id, now)
	while p < pn:
		_release(_prev_ids[p])
		p += 1
	_prev_ids = ids
	return slots


# Renumber every live slot down to a dense [0, slot_count()) run and rebuild
# `slot_of` to match, discarding the free list. A no-op unless the free list
# holds more than half of capacity() — cheap to call every frame from the
# owner; it only actually does work once deaths have hollowed the arrays out.
func compact() -> void:
	var cap := alive.size()
	if cap == 0 or _free.size() * 2 <= cap:
		return
	var new_facing_side := PackedInt32Array()
	var new_facing_ease := PackedFloat32Array()
	var new_facing_heading := PackedFloat32Array()
	var new_gait := PackedFloat32Array()
	var new_walk_hold := PackedFloat32Array()
	var new_walk_weight := PackedFloat32Array()
	var new_action_pose := PackedFloat32Array()
	var new_action_hold := PackedFloat32Array()
	var new_birth_time := PackedFloat32Array()
	var new_alive := PackedByteArray()
	var new_slot_of := {}
	# _prev_ids is exactly the set of currently-live ids, ascending; walking it
	# in order keeps the new slot numbering stable (and so still cache-
	# friendly) instead of depending on Dictionary iteration order.
	for id in _prev_ids:
		var old_s: int = int(slot_of[id])
		var new_s := new_alive.size()
		new_facing_side.append(facing_side[old_s])
		new_facing_ease.append(facing_ease[old_s])
		new_facing_heading.append(facing_heading[old_s])
		new_gait.append(gait[old_s])
		new_walk_hold.append(walk_hold[old_s])
		new_walk_weight.append(walk_weight[old_s])
		new_action_pose.append(action_pose[old_s])
		new_action_hold.append(action_hold[old_s])
		new_birth_time.append(birth_time[old_s])
		new_alive.append(1)
		new_slot_of[id] = new_s
	facing_side = new_facing_side
	facing_ease = new_facing_ease
	facing_heading = new_facing_heading
	gait = new_gait
	walk_hold = new_walk_hold
	walk_weight = new_walk_weight
	action_pose = new_action_pose
	action_hold = new_action_hold
	birth_time = new_birth_time
	alive = new_alive
	slot_of = new_slot_of
	_free = PackedInt32Array()


func _acquire(id: int, now: float) -> int:
	var s: int
	if _free.is_empty():
		s = alive.size()
		facing_side.append(0)
		facing_ease.append(0.0)
		facing_heading.append(0.0)
		gait.append(FxMath.seed_gait(id))
		walk_hold.append(0.0)
		walk_weight.append(0.0)
		action_pose.append(_DEFAULT_ACTION_POSE)
		action_hold.append(0.0)
		birth_time.append(now)
		alive.append(1)
	else:
		s = _free[_free.size() - 1]
		_free.resize(_free.size() - 1)
		facing_side[s] = 0
		facing_ease[s] = 0.0
		facing_heading[s] = 0.0
		gait[s] = FxMath.seed_gait(id)
		walk_hold[s] = 0.0
		walk_weight[s] = 0.0
		action_pose[s] = _DEFAULT_ACTION_POSE
		action_hold[s] = 0.0
		birth_time[s] = now
		alive[s] = 1
	slot_of[id] = s
	return s


func _release(id: int) -> void:
	var s: int = int(slot_of.get(id, -1))
	if s < 0:
		return
	slot_of.erase(id)
	alive[s] = 0
	_free.append(s)
