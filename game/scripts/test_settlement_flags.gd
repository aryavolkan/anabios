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
const VillageLayout = preload("res://scripts/village_layout.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
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
