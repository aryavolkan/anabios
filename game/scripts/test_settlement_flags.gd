extends SceneTree
# Headless unit test for settlement_layer.gd's pure static helpers:
# flags_for() (invention keys + recent-event memory -> VillageLayout flag
# bitmask), era_for() (invention keys + era-of map -> highest held era) and
# plan_signature() (the village-plan cache key). No scene/bridge dependency.
# Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_settlement_flags.gd
# Exits 0 on success, 1 on the first failed assertion.
#
# settlement_layer.gd preloads structure_sprites.gd and village_layout.gd —
# the village-footprint modules landed by parallel work (D5/D7) — so this
# test only compiles once those files exist alongside it.

const SettlementLayer = preload("res://scripts/settlement_layer.gd")
const HubLayer = preload("res://scripts/hub_layer.gd")
const CaravanLayer = preload("res://scripts/caravan_layer.gd")
const VillageLayout = preload("res://scripts/village_layout.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	# --- market square: stalls ring the hub at a fixed radius, none on it ---
	var slots: PackedVector2Array = HubLayer.stall_slots(Vector2(100, 100), 3)
	_check(slots.size() == 3, "three stall slots")
	for sl in slots:
		_check(
			is_equal_approx(sl.distance_to(Vector2(100, 100)), HubLayer.STALL_RADIUS),
			"stall %s sits on the ring" % sl
		)
	_check(slots[0] != slots[1] and slots[1] != slots[2], "stall slots are distinct")
	_check(HubLayer.stall_slots(Vector2.ZERO, 0).is_empty(), "no stalls, no slots")
	# --- caravan routes stop at the square's gate ---
	var ends: PackedVector2Array = CaravanLayer.gate_ends(Vector2(0, 0), Vector2(200, 0), 34.0)
	_check(
		ends[0] == Vector2(34, 0) and ends[1] == Vector2(166, 0),
		"route ends pull in by the gate margin"
	)
	var short: PackedVector2Array = CaravanLayer.gate_ends(Vector2(0, 0), Vector2(40, 0), 34.0)
	_check(
		short[0] == Vector2(0, 0) and short[1] == Vector2(40, 0), "a short route keeps its centres"
	)
	# --- dirt roads: a patch every step along the route, none on water ---
	var road: PackedVector2Array = CaravanLayer.road_steps(
		Vector2(0, 0), Vector2(60, 0), 6.0, Callable()
	)
	_check(road.size() == 9, "a 60-unit road gets nine 6-unit patches clear of both ends")
	for p in road:
		_check(
			absf(p.y) <= CaravanLayer.ROAD_WANDER + 0.001, "road patch %s stays near the line" % p
		)
	var wet := func(p: Vector2) -> bool: return p.x > 20.0 and p.x < 40.0
	var dry: PackedVector2Array = CaravanLayer.road_steps(Vector2(0, 0), Vector2(60, 0), 6.0, wet)
	_check(dry.size() == 6, "road patches skip the water cells (%d)" % dry.size())
	_check(
		CaravanLayer.road_steps(Vector2.ZERO, Vector2(3, 0), 6.0, Callable()).is_empty(), "no road"
	)
	# A narrow crossing (one or two water steps with land either side) gets
	# a bridge on the line; the wide lake above stays bare.
	var river := func(p: Vector2) -> bool: return p.x > 27.0 and p.x < 33.0
	var crossing: Dictionary = CaravanLayer.road_plan(Vector2(0, 0), Vector2(60, 0), 6.0, river)
	_check((crossing["bridge"] as PackedVector2Array).size() == 1, "one bridge step over the river")
	_check(
		(crossing["bridge"] as PackedVector2Array)[0] == Vector2(30, 0), "bridge sits on the line"
	)
	_check((crossing["road"] as PackedVector2Array).size() == 8, "road patches flank the bridge")
	var lake: Dictionary = CaravanLayer.road_plan(Vector2(0, 0), Vector2(60, 0), 6.0, wet)
	_check((lake["bridge"] as PackedVector2Array).is_empty(), "a wide water is not bridged")
	var bay := func(p: Vector2) -> bool: return p.x > 45.0 and p.x < 51.0
	var corner: Dictionary = CaravanLayer.road_plan(Vector2(0, 0), Vector2(60, 0), 6.0, bay)
	_check(
		(corner["bridge"] as PackedVector2Array).is_empty(),
		"a crossing with no road beyond it is not bridged"
	)
	var shore := func(p: Vector2) -> bool: return p.x < 9.0
	var edge: Dictionary = CaravanLayer.road_plan(Vector2(0, 0), Vector2(60, 0), 6.0, shore)
	_check(
		(edge["bridge"] as PackedVector2Array).is_empty(), "water at the road's end is not bridged"
	)
	# --- dirt yards: a 32 px earth patch, solid centre, clear corners ---
	var yard: Image = SettlementLayer.yard_image()
	_check(yard.get_width() == 32 and yard.get_height() == 32, "yard patch is 32x32")
	_check(yard.get_pixel(16, 16).a > 0.9, "yard centre is packed earth")
	_check(yard.get_pixel(0, 0).a == 0.0, "yard corners stay clear")
	# The market square asks for a larger image with the same ellipse.
	var square: Image = SettlementLayer.yard_image(144)
	_check(square.get_width() == 144 and square.get_height() == 144, "square patch is 144x144")
	_check(square.get_pixel(72, 72).a > 0.9, "square centre is packed earth")
	_check(square.get_pixel(0, 0).a == 0.0 and square.get_pixel(143, 72).a == 0.0, "square ellipse")
	_check(
		square.get_pixel(72, 8).a == 0.0 and square.get_pixel(72, 24).a > 0.9, "square is flatter"
	)
	_check(SettlementLayer.yard_scale(VillageLayout.HUT) > 0.0, "huts stand on a yard")
	# Per-structure tone: bounded, stable, and different across cells.
	var tones := {}
	for i in 20:
		var sw: float = SettlementLayer.tone_swing(3, Vector2i(i, -i))
		_check(
			(
				sw >= 1.0 - SettlementLayer.TONE_SWING - 0.001
				and sw <= 1.0 + SettlementLayer.TONE_SWING + 0.001
			),
			"tone swing stays within its band"
		)
		tones[snappedf(sw, 0.001)] = true
	_check(tones.size() >= 10, "tone swing varies across cells")
	var sw_a: float = SettlementLayer.tone_swing(3, Vector2i(2, 2))
	var sw_b: float = SettlementLayer.tone_swing(3, Vector2i(2, 2))
	_check(sw_a == sw_b, "tone swing is stable")
	var steps: PackedVector2Array = SettlementLayer.path_steps(Vector2.ZERO, Vector2(70, 0), 7.0)
	_check(steps.size() == 7, "a 70-unit path gets seven 7-unit steps clear of both yards")
	_check(
		steps[0] == Vector2(14, 0) and steps[-1] == Vector2(56, 0),
		"path steps stay between the yards"
	)
	_check(
		SettlementLayer.path_steps(Vector2.ZERO, Vector2(10, 0), 7.0).is_empty(),
		"no path for next-door yards"
	)
	_check(SettlementLayer.yard_scale(VillageLayout.FENCE_H) == 0.0, "fences keep the ground")
	_check(
		(
			SettlementLayer.yard_scale(VillageLayout.HEARTH)
			> SettlementLayer.yard_scale(VillageLayout.HUT)
		),
		"the hearth's yard is the widest"
	)
	# --- era_for --------------------------------------------------------
	_check(
		SettlementLayer.era_for(PackedStringArray(), {}) == 0,
		"era_for with no held inventions is era 0"
	)
	_check(
		SettlementLayer.era_for(PackedStringArray(["mystery"]), {"fire": 1}) == 0,
		"era_for ignores a held key absent from the era map"
	)
	_check(
		(
			SettlementLayer.era_for(
				PackedStringArray(["fire", "farming"]), {"fire": 1, "farming": 2}
			)
			== 2
		),
		"era_for takes the highest era among held inventions"
	)
	_check(
		(
			SettlementLayer.era_for(
				PackedStringArray(["fire", "writing"]), {"fire": 3, "writing": 0}
			)
			== 3
		),
		"era_for is not confused by a held era-0 invention alongside a higher one"
	)

	# --- flags_for: invention keys ---------------------------------------
	_check(
		(
			SettlementLayer.flags_for(PackedStringArray(["farming"]), {}, 0)
			== VillageLayout.FLAG_FARMING
		),
		"flags_for sets FLAG_FARMING for a held 'farming' key"
	)
	_check(
		SettlementLayer.flags_for(PackedStringArray(["stone_tools"]), {}, 0) == 0,
		"flags_for ignores an invention key with no flag mapping"
	)
	var all_tech := PackedStringArray(["farming", "machinery", "metalworking", "writing"])
	var expect_all := (
		VillageLayout.FLAG_FARMING
		| VillageLayout.FLAG_MACHINERY
		| VillageLayout.FLAG_METALWORKING
		| VillageLayout.FLAG_WRITING
	)
	_check(
		SettlementLayer.flags_for(all_tech, {}, 0) == expect_all,
		"flags_for sets one bit per held tech key, all four held"
	)

	# --- flags_for: recent-event windows ----------------------------------
	var recent := PackedStringArray()
	_check(
		(
			(
				SettlementLayer.flags_for(
					recent, {"territory": 50}, 50 + SettlementLayer.RECENT_TICKS
				)
				& VillageLayout.FLAG_TERRITORY
			)
			!= 0
		),
		"FLAG_TERRITORY is set exactly at the RECENT_TICKS boundary (inclusive)"
	)
	_check(
		(
			(
				SettlementLayer.flags_for(
					recent, {"territory": 50}, 51 + SettlementLayer.RECENT_TICKS
				)
				& VillageLayout.FLAG_TERRITORY
			)
			== 0
		),
		"FLAG_TERRITORY clears one tick past the RECENT_TICKS window"
	)
	_check(
		(
			(
				SettlementLayer.flags_for(recent, {"war": 10}, 10 + SettlementLayer.RECENT_TICKS)
				& VillageLayout.FLAG_WAR
			)
			!= 0
		),
		"FLAG_WAR is set inside the RECENT_TICKS window"
	)
	_check(
		(
			(
				SettlementLayer.flags_for(recent, {"raid": 5}, 5 + SettlementLayer.RAID_TICKS)
				& VillageLayout.FLAG_RAIDED
			)
			!= 0
		),
		"FLAG_RAIDED is set exactly at the (shorter) RAID_TICKS boundary"
	)
	_check(
		(
			(
				SettlementLayer.flags_for(recent, {"raid": 5}, 6 + SettlementLayer.RAID_TICKS)
				& VillageLayout.FLAG_RAIDED
			)
			== 0
		),
		"FLAG_RAIDED clears one tick past the shorter RAID_TICKS window"
	)
	_check(
		SettlementLayer.flags_for(recent, {}, 999999) == 0,
		"flags_for with no recent events and no held tech is 0"
	)
	var combo_recent := {"territory": 100, "war": 100, "raid": 100}
	var combo_flags := SettlementLayer.flags_for(PackedStringArray(["farming"]), combo_recent, 100)
	var combo_expect := (
		VillageLayout.FLAG_FARMING
		| VillageLayout.FLAG_TERRITORY
		| VillageLayout.FLAG_WAR
		| VillageLayout.FLAG_RAIDED
	)
	_check(
		combo_flags == combo_expect, "flags_for combines held-tech and recent-event bits together"
	)

	# --- plan_signature ----------------------------------------------------
	_check(
		SettlementLayer.plan_signature(1, 6, 2, 3) == SettlementLayer.plan_signature(1, 11, 2, 3),
		"plan_signature is stable across a members change inside the same bucket"
	)
	_check(
		SettlementLayer.plan_signature(1, 5, 2, 3) != SettlementLayer.plan_signature(1, 6, 2, 3),
		"plan_signature changes when members crosses a bucket boundary"
	)
	_check(
		SettlementLayer.plan_signature(1, 6, 2, 3) != SettlementLayer.plan_signature(2, 6, 2, 3),
		"plan_signature changes with species id"
	)
	_check(
		SettlementLayer.plan_signature(1, 6, 2, 3) != SettlementLayer.plan_signature(1, 6, 3, 3),
		"plan_signature changes with era"
	)
	_check(
		SettlementLayer.plan_signature(1, 6, 2, 3) != SettlementLayer.plan_signature(1, 6, 2, 5),
		"plan_signature changes with flags"
	)
	var sig_a := SettlementLayer.plan_signature(3, 17, 1, 9)
	var sig_b := SettlementLayer.plan_signature(3, 17, 1, 9)
	_check(sig_a == sig_b, "plan_signature is deterministic for identical inputs")

	# --- merge_sites: co-located anchors fold into the largest lineage ---
	var sites: Array = [
		{"species_id": 1, "pos": Vector2(100, 100), "members": 40},
		{"species_id": 2, "pos": Vector2(110, 104), "members": 300},
		{"species_id": 3, "pos": Vector2(900, 900), "members": 20},
		{"species_id": 4, "pos": Vector2(130, 96), "members": 5},
	]
	var merged: Array = SettlementLayer.merge_sites(sites, 48.0)
	_check(merged.size() == 2, "three co-located sites merge into one village (+1 far site)")
	_check(int(merged[0]["species_id"]) == 2, "the largest lineage keeps the village")
	_check(int(merged[0]["members"]) == 345, "merged members are summed")
	_check(int(merged[1]["species_id"]) == 3, "a distant site stays its own village")
	_check(SettlementLayer.merge_sites([], 48.0).is_empty(), "no sites merge to nothing")

	if _failed:
		quit(1)
		return
	print("test_settlement_flags: all passed")
	quit(0)
