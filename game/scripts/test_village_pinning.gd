extends SceneTree
# Headless unit test for village_sites.gd, settlement_layer.gd's village
# placement rules (pure static helpers, no scene/bridge dependency):
#   - a village is pinned where it is founded and does not slide with the
#     wandering anchor centroid (track_site); it moves only after the smoothed
#     centroid has stayed RELOCATE radii away for DWELL, leaving a ghost of the
#     old village to fade in place; a time jump snaps it;
#   - only hominids build villages (classify_hominids / hominid_sites), with a
#     join/leave hysteresis on the omnivore diet-band share.
# Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_village_pinning.gd
# Exits 0 on success, 1 on any failed assertion.

const SettlementLayer = preload("res://scripts/settlement_layer.gd")
const VillageSites = preload("res://scripts/village_sites.gd")
const MammalSprites = preload("res://scripts/mammal_sprites.gd")
const VillageLayout = preload("res://scripts/village_layout.gd")

# Redraws land every REDRAW_EVERY frames: a third of a second at 60 fps.
const STEP := 1.0 / 3.0

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


# Species ids and diets for `pairs` of [sid, diet].
func _agents(pairs: Array) -> Array:
	var ids := PackedInt32Array()
	var diet := PackedFloat32Array()
	for p in pairs:
		ids.append(int(p[0]))
		diet.append(float(p[1]))
	return [ids, diet]


# Ten members of `sid`, `in_band` of them omnivores and the rest grazers.
func _share(sid: int, in_band: int) -> Array:
	var pairs: Array = []
	for k in 10:
		pairs.append([sid, 0.5 if k < in_band else 0.05])
	return _agents(pairs)


func _test_pinning() -> void:
	var reach := VillageSites.RELOCATE * 40.0
	var home := Vector2(500, 500)
	var clock := 0.0
	var v: Dictionary = VillageSites.found_village(3, home, 40, 0.0, clock)
	_check(v["pos"] == home and float(v["retired"]) < 0.0, "a village is founded where it is seen")
	# The centroid jitters and wanders inside the reach, and spikes out past
	# it for less than DWELL (a cohort dying): the village never moves.
	var spike_from := 60.0
	for i in 600:
		clock += STEP
		var spike: bool = clock > spike_from and clock < spike_from + VillageSites.DWELL * 0.5
		var live := home + Vector2(sin(i * 0.37), cos(i * 0.21)) * reach * 0.8
		if spike:
			live += Vector2(reach * 3.0, 0.0)
		var ghost: Dictionary = VillageSites.track_site(v, live, clock, false, reach)
		_check(ghost.is_empty(), "jitter and a short spike never relocate (clock %.1f)" % clock)
	_check(v["pos"] == home, "the village stays pinned where it was founded")
	# The people move 3 reaches east and stay: the village follows once,
	# after the dwell, and hands back a ghost where it stood.
	v["plan"] = [{"kind": VillageLayout.HUT, "pos": home}]
	v["plan_sig"] = "sig"
	v["plan_anchor"] = home
	var far := home + Vector2(reach * 3.0, 0.0)
	var moved_at := -1.0
	var ghosts: Array = []
	var moved_from := clock
	for i in 120:
		clock += STEP
		var g: Dictionary = VillageSites.track_site(v, far, clock, false, reach)
		if not g.is_empty():
			ghosts.append(g)
			if moved_at < 0.0:
				moved_at = clock
	_check(ghosts.size() == 1, "a sustained move relocates exactly once (%d)" % ghosts.size())
	_check(
		moved_at - moved_from >= VillageSites.DWELL,
		"the move waits out the dwell (%.2f s)" % (moved_at - moved_from)
	)
	_check(moved_at - moved_from < VillageSites.DWELL + 4.0, "and does not wait much longer")
	_check(
		(v["pos"] as Vector2).distance_to(far) < reach,
		"relocated near the new home (%s, want ~%s)" % [v["pos"], far]
	)
	_check(not v.has("plan") and not v.has("plan_sig"), "the moved village is planned afresh")
	if ghosts.size() == 1:
		var g: Dictionary = ghosts[0]
		_check(g["pos"] == home, "the ghost stays where the old village stood")
		_check(g.has("plan") and g["plan_anchor"] == home, "the ghost keeps its old plan")
	# The smoothed centroid can sit far off for the whole dwell (a big spike
	# decaying) while the people themselves stayed home: no relocation.
	var stay: Dictionary = VillageSites.found_village(4, home, 40, 0.0, 0.0)
	stay["lpos"] = home + Vector2(reach * 10.0, 0.0)
	var t := 0.0
	var stay_moved := false
	while t < VillageSites.DWELL + 1.0:
		t += STEP
		var sg: Dictionary = VillageSites.track_site(
			stay, home + Vector2(reach * 0.5, 0.0), t, false, reach
		)
		stay_moved = stay_moved or not sg.is_empty()
	_check(not stay_moved and stay["pos"] == home, "a raw home inside the reach keeps the pin")
	# A time jump (replay seek / fast-forward) snaps the village instead.
	var j: Dictionary = VillageSites.found_village(2, Vector2(100, 100), 16, 0.0, 0.0)
	var snap: Dictionary = VillageSites.track_site(j, Vector2(400, 100), 0.0, true, 50.0)
	_check(snap.is_empty() and j["pos"] == Vector2(400, 100), "a time jump snaps the village")
	var near: Dictionary = VillageSites.found_village(2, Vector2(100, 100), 16, 0.0, 0.0)
	VillageSites.track_site(near, Vector2(120, 100), 0.0, true, 50.0)
	_check(near["pos"] == Vector2(100, 100), "a jump inside the reach keeps the pin")
	# Frame-rate independence: the same wall time in coarser samples moves
	# the smoothed centroid the same way.
	var fine: Dictionary = VillageSites.found_village(1, Vector2.ZERO, 16, 0.0, 0.0)
	var coarse: Dictionary = VillageSites.found_village(1, Vector2.ZERO, 16, 0.0, 0.0)
	for i in 6:
		VillageSites.track_site(fine, Vector2(10, 0), (i + 1) * STEP, false, 1000.0)
	for i in 3:
		VillageSites.track_site(coarse, Vector2(10, 0), (i + 1) * STEP * 2.0, false, 1000.0)
	_check(
		is_equal_approx((fine["lpos"] as Vector2).x, (coarse["lpos"] as Vector2).x),
		"centroid smoothing is frame-rate independent"
	)


func _test_time_jumps() -> void:
	_check(VillageSites.is_time_jump(-1, 0), "the first redraw is a jump")
	_check(not VillageSites.is_time_jump(1000, 1000), "a paused redraw is not a jump")
	_check(not VillageSites.is_time_jump(1000, 1000 + 20 * 64), "64x play is not a jump")
	_check(VillageSites.is_time_jump(5000, 4000), "a rewind is a jump")
	_check(VillageSites.is_time_jump(1000, 60000), "a fast-forward is a jump")


func _test_footprint_radius() -> void:
	_check(
		VillageSites.footprint_radius([], Vector2(5, 5)) == VillageLayout.GRID,
		"an unplanned village is one cell wide"
	)
	var plan: Array = [
		{"kind": VillageLayout.HEARTH, "pos": Vector2(5, 5)},
		{"kind": VillageLayout.HUT, "pos": Vector2(65, 5)},
		{"kind": VillageLayout.HUT, "pos": Vector2(5, -25)},
	]
	_check(
		is_equal_approx(VillageSites.footprint_radius(plan, Vector2(5, 5)), 60.0),
		"the radius reaches the farthest structure"
	)


func _test_hominids() -> void:
	var lo: float = MammalSprites.HERB_MAX
	var hi: float = MammalSprites.CARN_MIN
	# An omnivore lineage settles into a village; a grazing herd (diet ~0)
	# and a hunting pack (~1) do not. The band is [HERB_MAX, CARN_MIN).
	var a: Array = _agents(
		[[1, 0.5], [1, 0.48], [1, lo], [2, 0.01], [2, 0.0], [3, 1.0], [3, hi], [4, hi - 0.01]]
	)
	var h: Dictionary = VillageSites.classify_hominids(a[0], a[1], {})
	_check(h.has(1), "an omnivore lineage is a hominid")
	_check(not h.has(2), "a grazing herd is not")
	_check(not h.has(3), "a hunting pack is not (CARN_MIN is outside the band)")
	_check(h.has(4), "just under CARN_MIN is inside the band")
	# Join at HOMINID_IN, stay down to HOMINID_OUT, rejoin only at HOMINID_IN.
	var s5: Array = _share(6, 5)
	var joined: Dictionary = VillageSites.classify_hominids(s5[0], s5[1], {})
	_check(joined.has(6), "50% in band joins")
	var s4: Array = _share(6, 4)
	_check(not VillageSites.classify_hominids(s4[0], s4[1], {}).has(6), "40% cannot join")
	var kept: Dictionary = VillageSites.classify_hominids(s4[0], s4[1], joined)
	_check(kept.has(6), "40% keeps an existing hominid")
	var s2: Array = _share(6, 2)
	var dropped: Dictionary = VillageSites.classify_hominids(s2[0], s2[1], kept)
	_check(not dropped.has(6), "20% drops it")
	_check(not VillageSites.classify_hominids(s4[0], s4[1], dropped).has(6), "no easy rejoin")
	_check(
		(
			VillageSites.classify_hominids(PackedInt32Array([1]), PackedFloat32Array(), joined)
			== joined
		),
		"mismatched arrays keep the previous set"
	)
	# Only hominid sites become villages, and only they merge: a big herd
	# camped beside a hominid village neither absorbs it nor is drawn.
	var sites: Array = [
		{"species_id": 1, "pos": Vector2(100, 100), "members": 40},
		{"species_id": 2, "pos": Vector2(110, 104), "members": 300},
		{"species_id": 3, "pos": Vector2(800, 800), "members": 40},
	]
	var kept_sites: Array = VillageSites.hominid_sites(sites, h)
	_check(kept_sites.size() == 1, "only the hominid site is kept (%d)" % kept_sites.size())
	var merged: Array = SettlementLayer.merge_sites(kept_sites, SettlementLayer.MERGE_RADIUS)
	_check(
		merged.size() == 1 and int(merged[0]["species_id"]) == 1,
		"the hominid keeps its village next to a herd"
	)
	_check(int(merged[0]["members"]) == 40, "the herd's members are not folded in")
	_check(VillageSites.hominid_sites(sites, {}).is_empty(), "no hominids, no villages")


func _init() -> void:
	_test_pinning()
	_test_time_jumps()
	_test_footprint_radius()
	_test_hominids()
	if _failed:
		quit(1)
		return
	print("test_village_pinning: all passed")
	quit(0)
