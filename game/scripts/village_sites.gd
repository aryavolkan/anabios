extends RefCounted
# Where settlement_layer.gd draws villages, as pure static rules over the
# sim's settlement sites (no scene/bridge state, unit-tested headless by
# test_village_pinning.gd): which species' sites become villages at all, and
# where a village stands as its site's centroid wanders.

const FxMath = preload("res://scripts/fx_math.gd")
const MammalSprites = preload("res://scripts/mammal_sprites.gd")
const VillageLayout = preload("res://scripts/village_layout.gd")

# Pinned villages. A site's position is the centroid of its members' home
# anchors, and that wanders every tick as anchors learn and members are born
# and die (tens of world units over a run, single-sample jumps of ~60 when a
# cohort dies); drawing the village at it — even eased — slid whole villages
# across the ground. A village is therefore pinned where it is founded and
# moves only when its people have plainly left: the smoothed centroid has to
# sit more than RELOCATE village radii away for DWELL seconds, and then the
# old village fades where it stood while a new one grows at the people's new
# home (the mean raw centroid over the dwell). A time jump (replay
# rewind/resume, a capture fast-forward) snaps instead.
#
# SMOOTH_TAU and DWELL run on the layer's sim clock: wall seconds that only
# advance while the sim does (capped per redraw), so they are frame-rate
# independent and a paused world never relocates a village on a transient
# centroid.
const RELOCATE := 2.5
const SMOOTH_TAU := 2.0
const DWELL := 4.0
# A layer redraw spans at most REDRAW_EVERY x (64 ticks/frame + the replay rewind's
# 64): 2560 ticks. A larger gap, or any step backwards, is a time jump.
const JUMP_TICKS := 4000

# Huts only for hominids. The sim's settlement latch fires for any species
# whose home anchors cluster — grazing herds and hunting packs included — so a
# site is drawn only while its species reads as a hominid: HOMINID_IN of its
# members in the omnivore diet band the agent layer draws as primates
# (MammalSprites.HERB_MAX..CARN_MIN) to join, dropping below HOMINID_OUT to
# leave (so a lineage drifting across the band edge does not flicker its
# village). Body size is deliberately not used: growing juveniles fall below
# MammalSprites.SIZE_SPLIT.
const HOMINID_IN := 0.5
const HOMINID_OUT := 0.3


# Pure: which species read as hominids, from sim.alive_species_ids() and
# alive_diet() (same order). A species joins once HOMINID_IN of its members
# sit in the omnivore band [HERB_MAX, CARN_MIN) (culture-bearing archetypes
# run at diet ~0.5, grazers near 0, hunters near 1); a member of `prev` stays
# until its share drops below HOMINID_OUT. Mismatched arrays keep `prev`.
static func classify_hominids(
	sp_ids: PackedInt32Array, diet: PackedFloat32Array, prev: Dictionary
) -> Dictionary:
	if sp_ids.size() != diet.size():
		return prev
	var tally: Dictionary = {}  # sid -> Vector2i(in band, members)
	for i in sp_ids.size():
		var d: float = diet[i]
		var in_band: int = 1 if d >= MammalSprites.HERB_MAX and d < MammalSprites.CARN_MIN else 0
		tally[sp_ids[i]] = (tally.get(sp_ids[i], Vector2i.ZERO) as Vector2i) + Vector2i(in_band, 1)
	var out: Dictionary = {}
	for sid in tally:
		var t: Vector2i = tally[sid]
		var share: float = float(t.x) / float(t.y)
		if share >= HOMINID_IN or (prev.has(sid) and share >= HOMINID_OUT):
			out[sid] = true
	return out


# Pure: the settlement sites whose species is a hominid.
static func hominid_sites(sites: Array, hominids: Dictionary) -> Array:
	return sites.filter(func(s) -> bool: return hominids.has(int(s["species_id"])))


# Pure: whether the sim tick moved by more than normal play between two
# redraws (a replay rewind/resume, a capture fast-forward, a fresh world).
static func is_time_jump(last_tick: int, tick: int) -> bool:
	return last_tick < 0 or tick < last_tick or tick - last_tick > JUMP_TICKS


# Pure: a village's radius (world units) — the farthest planned structure
# from its anchor, at least one grid cell.
static func footprint_radius(plan: Array, anchor: Vector2) -> float:
	var r := VillageLayout.GRID
	for p in plan:
		r = maxf(r, (p["pos"] as Vector2).distance_to(anchor))
	return r


# A new village record pinned at `pos`.
static func found_village(
	sid: int, pos: Vector2, members: int, now: float, clock: float
) -> Dictionary:
	return {
		"sid": sid,
		"pos": pos,
		"lpos": pos,
		"members": members,
		"born": now,
		"seen": now,
		"clock": clock,
		"far_since": -1.0,
		"far_sum": Vector2.ZERO,
		"far_n": 0,
		"retired": -1.0,
	}


# Pure (mutates `v` only): fold one sample of the live site centroid into a
# pinned village. The smoothed centroid eases toward `live` on the sim clock;
# the pinned "pos" stays put until the smoothed centroid has stood more than
# `reach` away for DWELL clock seconds, then moves to the mean raw centroid
# over that stretch and the village starts over (new plan, pop-in). On a
# `jump` a far village snaps straight to where its people are. Returns a copy
# of the village as it stood before a relocation — the ghost to fade in
# place — or an empty Dictionary.
static func track_site(
	v: Dictionary, live: Vector2, clock: float, jump: bool, reach: float
) -> Dictionary:
	var k: float = (
		1.0 if jump else FxMath.ease_factor(clock - float(v.get("clock", clock)), SMOOTH_TAU)
	)
	var lpos: Vector2 = (v.get("lpos", live) as Vector2).lerp(live, k)
	v["lpos"] = lpos
	v["clock"] = clock
	if lpos.distance_to(v["pos"]) <= reach:
		v["far_since"] = -1.0
		return {}
	if jump:
		v["pos"] = lpos
		v["far_since"] = -1.0
		return {}
	# While away, average the raw centroid: the smoothed one lags a real
	# move, and the new village belongs where the people were meanwhile.
	if float(v.get("far_since", -1.0)) < 0.0:
		v["far_since"] = clock
		v["far_sum"] = Vector2.ZERO
		v["far_n"] = 0
	v["far_sum"] = (v["far_sum"] as Vector2) + live
	v["far_n"] = int(v["far_n"]) + 1
	if clock - float(v["far_since"]) < DWELL:
		return {}
	var home: Vector2 = (v["far_sum"] as Vector2) / float(v["far_n"])
	v["far_since"] = -1.0
	if home.distance_to(v["pos"]) <= reach:
		return {}  # on average they never left: a spike, not a move
	var ghost: Dictionary = v.duplicate()
	v["pos"] = home
	v["lpos"] = home
	for key in ["plan", "plan_sig", "plan_anchor", "plan_born", "smoke_pos", "radius"]:
		v.erase(key)
	return ghost
