extends Node2D

# Settlement layer: persistent village footprints + trade/invention landmarks
# at the REAL settlement sites — the codex `settlement_active` latch per
# species, with the anchor centroid and member count from
# sim.settlement_sites(). Footprints come from VillageLayout.plan(): a
# deterministic function of (species id, anchor, member count, era, recent
# events) drawn through the `structures` atlas kinds (structure_sprites.gd).
# Pure presentation over read-only sim state, refreshed a few times a second.

const ApeSprites = preload("res://scripts/ape_sprites.gd")
const MammalSprites = preload("res://scripts/mammal_sprites.gd")
const Buildings = preload("res://scripts/building_sprites.gd")
const FxMath = preload("res://scripts/fx_math.gd")
const AgentLayer = preload("res://scripts/agent_layer.gd")
const PixelFxSprites = preload("res://scripts/pixel_fx_sprites.gd")
# Landed separately by parallel agents (D5/D7): the village footprint planner
# and its per-kind sprite atlas. Preloaded by path so this file keeps
# compiling against the agreed contract even before those files land.
const StructureSprites = preload("res://scripts/structure_sprites.gd")
const StructureTall = preload("res://scripts/structure_tall.gd")
const SpriteSplit = preload("res://scripts/sprite_split.gd")
const VillageLayout = preload("res://scripts/village_layout.gd")
const Clearings = preload("res://scripts/clearings.gd")

const REDRAW_EVERY := 20
# Structure sprites are 32px, drawn at 0.625 scale (huts/structures span 20
# world units) — deliberately oversized next to agents (BODY_MIN ~6) so a
# village reads as architecture, not as a few more creatures.
const STRUCTURE_SCALE := 0.625
# Height over width of a tall (walled) structure's quad: TALL_PX / CELL_PX.
const TALL_RATIO := float(StructureTall.TALL_PX) / float(StructureSprites.CELL_PX)
# Sites closer than this (world units) share one village footprint.
const MERGE_RADIUS := 48.0
# Clearing margin around the outermost structure centre (world units): half
# a village cell plus a yard's worth.
const CLEARING_MARGIN := 14.0
const MEMBERS_BUCKET := 6
# Landmark/trade buildings sit a notch bigger than huts so a village's
# invention history and trade role read at a glance from the ring around it.
const BUILDING_SCALE := 20.0
const LANDMARK2_MIN_MEMBERS := 32
# Radius of the ring an invention-holding lineage's workshops stand on.
const LANDMARK_RING := 22.0
# Invention landmarks are anchored to the SPECIES that hold inventions, not to
# settlements: in organic runs the settling lineages are asocial foragers with
# no tech, while the inventive (cultural) lineages rarely settle. So a lineage
# needs this many live members before its tech earns a landmark at its centroid
# (keeps tiny splinter species from littering the map). Settled species draw
# their era architecture from VillageLayout instead (MILL/FORGE/SCRIPTORIUM/
# GRANARY placements replace these markers) — this path now only fires for
# tech-holding lineages with NO settlement.
const INVENTION_MIN_MEMBERS := 25

# Codex event ids (codex_panel.gd CHAPTER_NAMES) that drive the village-plan
# flags: Territory formation and War mark an ongoing state (a longer window),
# a raid is a short flare (huts still smoking).
const EVT_COMBAT_RAID := 7
const EVT_TERRITORY := 9
const EVT_WAR := 38
const RECENT_TICKS := 1500
const RAID_TICKS := 400

# Chimney-smoke plume pool, assigned each redraw to the largest live villages.
const SMOKE_POOL := 8
# Fires over burning ruins: a pooled flame emitter plus a dark smoke plume
# per ruin, assigned each redraw to the first FIRE_POOL burnt structures.
const FIRE_POOL := 6
# Construction pops are only sampled on the throttled redraw (~3/s), so the
# window must span several samples for the ease-out-back arc to actually show.
const POP_SECS := 1.5
# Flame flicker for animated buildings/structures: two authored frames swapped
# at a fixed low cadence (whole-texture swaps on the plain MultiMesh layers,
# so no per-building nodes and nothing on the Metal atlas path).
const FLICKER_PERIOD := 1.0 / 3.0

var _building_mmis: Array[MultiMeshInstance2D] = []
var _structure_mmis: Array[MultiMeshInstance2D] = []
# Trampled dirt yards under every dwelling and around the hearth, drawn under
# the fields (the reference villages stand on packed earth, not on grass).
var _yard_mmi: MultiMeshInstance2D = null
# Contact shadows under the dwellings: a soft ellipse cast to the lower
# right, drawn over the yard and under the walls, so a hut sits on its
# ground the way the boards' do instead of floating on the dirt patch.
var _shadow_mmi: MultiMeshInstance2D = null
const SHADOW_Z := -6
const SHADOW_COLOR := Color(0.0, 0.0, 0.0, 0.30)
const SHADOW_W := 0.80  # of the structure's drawn size
const SHADOW_H := 0.34
const SHADOW_OFFSET := Vector2(0.12, 0.30)  # of the drawn size, centre from the sprite centre
const YARD_PX := 32
const YARD_Z := -7
const YARD_SCALE := 1.7  # in structure widths; the hearth's yard is wider
const HEARTH_YARD_SCALE := 2.6
# Trodden paths: small yard patches stepped from every dwelling toward the
# village centre, so the square's dirt runs out to each door.
const PATH_STEP := 7.0
const PATH_SCALE := 0.55
var _smoke: Array[GPUParticles2D] = []
var _fires: Array[GPUParticles2D] = []
var _fire_smoke: Array[GPUParticles2D] = []
# Animated kinds only: key -> [phase-0 texture, phase-1 texture] and key ->
# every MultiMeshInstance2D showing it (the source layer plus its eight torus
# clones), so a frame swap can never leave a clone a frame behind. Shared by
# both building families; old Buildings kinds key as "b<kind>", new
# StructureSprites kinds as "s<kind>" so the two enums can't collide.
var _building_frames: Dictionary = {}
var _building_nodes: Dictionary = {}
# Roof layers per structure kind (null for the flat FIELD), see _ready.
var _structure_top_mmis: Array = []
var _flicker_elapsed := 0.0
var _flicker_phase := 0
var _era_of: Dictionary = {}  # invention key -> era, cached once
var _frame: int = REDRAW_EVERY - 1  # redraw on the very first frame
# Villages linger: the sim's settlement latch drops the moment anchor cohesion
# breaks, but a place people built shouldn't vanish overnight — sites stay
# drawn for LINGER seconds after the sim stops reporting them, fading out.
const LINGER := 45.0
const FADE := 10.0
var _villages: Dictionary = {}  # sid -> {pos, members, born, seen, plan...}
# Per-lineage invention-landmark memory, same linger/fade contract as _villages
# so a landmark eases in when a lineage first earns its tech and lingers/fades
# when the lineage dies out, instead of popping. sid -> {pos, sig, born, seen}.
var _lineage_marks: Dictionary = {}
var _sites: Array = []  # last settlement_sites() result
var _now: float = 0.0
# Wall-clock time of the last redraw, so the village anchor ease can be made
# independent of frame rate. ANCHOR_TAU is the exponential time constant: at
# 60 fps a redraw lands every REDRAW_EVERY/60 s, which reproduces the original
# 0.3-per-redraw drift.
var _last_ease: float = 0.0
const ANCHOR_TAU := 0.93
# Codex event cursor for the flags poll (Territory/War/Raid), independent of
# codex_panel's own cursor. sid -> {"territory": tick, "war": tick, "raid": tick}.
var _event_cursor: int = 0
var _recent_events: Dictionary = {}
# Capture aid: ANABIOS_VILLAGE_ERA / ANABIOS_VILLAGE_FLAGS force every
# village's era and OR in layout flags (VillageLayout.FLAG_*), so the era-2
# architecture can be framed without waiting for a lineage to earn it.
# Presentation only; unset in normal play.
var _demo_era: int = -1
var _demo_flags: int = 0

@onready var sim = get_node("../Simulation")
@onready var biome = get_node("../Biome")
# Optional: the particle/lighting effects subsystem, created as a sibling
# before this layer in main.gd (same order as `sim`/`biome`). Guarded with
# get_node_or_null so a burnt-ruin ember burst is skipped harmlessly if the
# hook is ever missing, rather than adding a new effects pool here.
@onready var _effects = get_node_or_null("../ViewerEffects")


func _ready() -> void:
	if OS.has_environment("ANABIOS_VILLAGE_ERA"):
		_demo_era = int(OS.get_environment("ANABIOS_VILLAGE_ERA"))
		_demo_flags = int(OS.get_environment("ANABIOS_VILLAGE_FLAGS"))
	# Landmark/trade buildings: one plain (no-shader) MultiMesh layer per
	# kind, drawn above agents. Kept as separate layers (rather than one
	# shared atlas) so each building keeps its own untouched texture on the
	# Metal-safe plain-MultiMesh path.
	for k in Buildings.KIND_COUNT:
		var tex := Buildings.build(k)
		var mmi := _make_layer("Building_%s" % Buildings.NAMES[k], tex, 1)
		_building_mmis.append(mmi)
		if Buildings.is_animated(k):
			var bkey := "b%d" % k
			_building_frames[bkey] = [tex, Buildings.build_variant(k, 1)]
			_building_nodes[bkey] = [mmi]
	_yard_mmi = _make_layer("Yard", SpriteSplit.for_quad(yard_image()), YARD_Z)
	_shadow_mmi = _make_layer(
		"StructureShadow", ImageTexture.create_from_image(AgentLayer.shadow_image(16)), SHADOW_Z
	)
	_shadow_mmi.modulate = SHADOW_COLOR
	# Village-footprint structures: one plain MultiMesh layer per kind, same
	# Metal-safe contract. Fields draw below agents like the old farm patches;
	# every other structure kind is cut in two (sprite_split.gd, D9): walls
	# below the figures (z -1, still above the ground props) and the roof
	# above them (z 1) on a second layer sharing the same MultiMesh, so a
	# figure south of a hut stands in front of it and one north of it is
	# hidden behind the roof.
	for k in StructureSprites.KIND_COUNT:
		# Walled kinds draw their 32x44 tall variant (StructureTall.TALL_ROWS).
		var img0: Image = StructureTall.tall_image(k, 0)
		if k == StructureSprites.FIELD:
			_structure_mmis.append(_make_layer("Structure_%d" % k, SpriteSplit.for_quad(img0), -6))
			_structure_top_mmis.append(null)
			continue
		var row: int = StructureTall.tall_split_row(k)
		var base_tex := SpriteSplit.for_quad(SpriteSplit.lower(img0, row))
		var top_tex := SpriteSplit.for_quad(SpriteSplit.upper(img0, row))
		var smmi := _make_layer("Structure_%d" % k, base_tex, -1)
		_structure_mmis.append(smmi)
		var top := MultiMeshInstance2D.new()
		top.name = "StructureTop_%d" % k
		top.multimesh = smmi.multimesh
		top.texture = top_tex
		top.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		top.z_index = 1
		add_child(top)
		_structure_top_mmis.append(top)
		if StructureSprites.is_animated(k):
			var img1: Image = StructureTall.tall_image(k, 1)
			_building_frames["s%d" % k] = [
				base_tex, SpriteSplit.for_quad(SpriteSplit.lower(img1, row))
			]
			_building_nodes["s%d" % k] = [smmi]
			_building_frames["t%d" % k] = [
				top_tex, SpriteSplit.for_quad(SpriteSplit.upper(img1, row))
			]
			_building_nodes["t%d" % k] = [top]
	for inv in sim.invention_catalog():
		_era_of[String(inv["key"])] = int(inv["era"])
	_make_smoke_pool()
	_make_fire_pool()
	_make_wrap_clones()


func _make_layer(pname: String, tex: ImageTexture, z: int) -> MultiMeshInstance2D:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_2D
	mm.use_colors = true
	mm.mesh = QuadMesh.new()
	var mmi := MultiMeshInstance2D.new()
	mmi.name = pname
	mmi.multimesh = mm
	mmi.texture = tex
	mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	mmi.z_index = z
	add_child(mmi)
	return mmi


# Same 9-way torus tiling as the agent layers, sharing each MultiMesh.
func _make_wrap_clones() -> void:
	var world: float = sim.world_size()
	for k in Buildings.KIND_COUNT:
		_clone_layer(_building_mmis[k], "b%d" % k, world)
	_clone_layer(_yard_mmi, "y", world)
	_clone_layer(_shadow_mmi, "sh", world)
	for k in StructureSprites.KIND_COUNT:
		_clone_layer(_structure_mmis[k], "s%d" % k, world)
		if _structure_top_mmis[k] != null:
			_clone_layer(_structure_top_mmis[k], "t%d" % k, world)


func _clone_layer(src: MultiMeshInstance2D, key: String, world: float) -> void:
	for gy in range(-1, 2):
		for gx in range(-1, 2):
			if gx == 0 and gy == 0:
				continue
			var clone := MultiMeshInstance2D.new()
			clone.multimesh = src.multimesh
			clone.texture = src.texture
			clone.texture_filter = src.texture_filter
			clone.z_index = src.z_index
			clone.modulate = src.modulate
			clone.position = Vector2(gx * world, gy * world)
			add_child(clone)
			if _building_nodes.has(key):
				_building_nodes[key].append(clone)


func _process(delta: float) -> void:
	_now = Time.get_ticks_msec() / 1000.0
	_flicker_elapsed += delta
	if _flicker_elapsed >= FLICKER_PERIOD:
		# Carry the overshoot so the cadence holds at low frame rates.
		_flicker_elapsed -= FLICKER_PERIOD
		_flicker_phase = 1 - _flicker_phase
		_tick_landmark_animation()
	_frame += 1
	if _frame % REDRAW_EVERY != 0:
		return
	_sites = sim.settlement_sites()
	_redraw()


# Swap the flame frame on every instance of each animated kind — the origin
# layer and its torus clones in one pass — leaving every other kind's texture
# untouched.
func _tick_landmark_animation() -> void:
	for key in _building_frames:
		var tex: ImageTexture = _building_frames[key][_flicker_phase]
		for node in _building_nodes[key]:
			node.texture = tex


func random_site_pos() -> Vector2:
	if _sites.is_empty():
		return Vector2.ZERO
	return _sites[randi() % _sites.size()]["pos"]


func has_sites() -> bool:
	return not _sites.is_empty()


# Pure: invention keys a species holds (adopted_inventions from
# species_stats()) plus a memory of recently-fired codex events for that
# species -> the VillageLayout flag bitmask. `recent` carries the last tick
# seen for "territory"/"war"/"raid" (absent = never seen).
static func flags_for(invention_keys: PackedStringArray, recent: Dictionary, tick: int) -> int:
	var flags := 0
	for key in invention_keys:
		match key:
			"farming":
				flags |= VillageLayout.FLAG_FARMING
			"machinery":
				flags |= VillageLayout.FLAG_MACHINERY
			"metalworking":
				flags |= VillageLayout.FLAG_METALWORKING
			"writing":
				flags |= VillageLayout.FLAG_WRITING
	if recent.has("territory") and tick - int(recent["territory"]) <= RECENT_TICKS:
		flags |= VillageLayout.FLAG_TERRITORY
	if recent.has("war") and tick - int(recent["war"]) <= RECENT_TICKS:
		flags |= VillageLayout.FLAG_WAR
	if recent.has("raid") and tick - int(recent["raid"]) <= RAID_TICKS:
		flags |= VillageLayout.FLAG_RAIDED
	return flags


# Pure: highest era among a species' held inventions, via the invention
# catalogue's key -> era map (the same lookup the old landmark signature used).
static func era_for(invention_keys: PackedStringArray, era_of: Dictionary) -> int:
	var era := 0
	for key in invention_keys:
		if era_of.has(key):
			era = maxi(era, int(era_of[key]))
	return era


# Pure: cache key for a village's plan. Bucketed member count so the layout
# does not replan (and re-pop) on every single birth/death — only when the
# village crosses a bucket boundary, era advances, or flags change.
static func plan_signature(sid: int, members: int, era: int, flags: int) -> String:
	var bucket := int(members / MEMBERS_BUCKET)
	return "%d|%d|%d|%d" % [sid, bucket, era, flags]


# Fold newly-fired Territory/War/CombatRaid events into the per-species
# recency memory used by flags_for. Own cursor, independent of codex_panel's.
func _poll_events() -> void:
	var count: int = int(sim.codex_event_count())
	if count < _event_cursor:
		_event_cursor = 0
		_recent_events.clear()
	var events: Array = sim.codex_events_since(_event_cursor)
	for ev in events:
		_event_cursor = int(ev["index"]) + 1
		var etype: int = int(ev["type"])
		if etype != EVT_TERRITORY and etype != EVT_WAR and etype != EVT_COMBAT_RAID:
			continue
		var esid: int = int(ev["species_id"])
		var etick: int = int(ev["tick"])
		var er: Dictionary = _recent_events.get(esid, {})
		if etype == EVT_TERRITORY:
			er["territory"] = etick
		elif etype == EVT_WAR:
			er["war"] = etick
		else:
			er["raid"] = etick
		_recent_events[esid] = er


# Structure kind priority for the smoke anchor: hearths first, then a hall's
# communal fire, then a forge. -1 = not a smoke source.
static func _smoke_rank(kind: int) -> int:
	if kind == StructureSprites.HEARTH:
		return 0
	if kind == StructureSprites.HALL:
		return 1
	if kind == StructureSprites.FORGE:
		return 2
	return -1


# Structure kinds that stand on a dirt yard (dwellings, work buildings and
# the hearth); fences, fields and walls keep the ground they are on.
const _YARD_KINDS: PackedInt32Array = [
	StructureSprites.TENT,
	StructureSprites.TENT_B,
	StructureSprites.HUT,
	StructureSprites.HUT_B,
	StructureSprites.HUT_C,
	StructureSprites.HOUSE,
	StructureSprites.WELL,
	StructureSprites.FORGE,
	StructureSprites.SCRIPTORIUM,
	StructureSprites.GRANARY,
	StructureSprites.MILL,
	StructureSprites.RUIN_BURNT,
]

# Brightness multiplier in [1 - TONE_SWING, 1 + TONE_SWING] for a
# structure at `cell` of village `sid`, stable across redraws.
const TONE_SWING := 0.06


static func tone_swing(sid: int, cell: Vector2i) -> float:
	return 1.0 + (VillageLayout.hash2(sid * 7 + 3, cell.x, cell.y) - 0.5) * 2.0 * TONE_SWING


static func yard_scale(kind: int) -> float:
	if kind == StructureSprites.HEARTH or kind == StructureSprites.HALL:
		return HEARTH_YARD_SCALE
	if _YARD_KINDS.has(kind):
		return YARD_SCALE
	return 0.0


# Points every `step` world units along the open stretch from `from` toward
# `to`, leaving the first and last step clear of the two yards.
# Steps wander up to PATH_WANDER sideways on a stable hash of the step and
# the path's start, so a trodden path winds a little the way the boards'
# do instead of running as a ruled line of dots.
const PATH_WANDER := 1.5


static func path_steps(from: Vector2, to: Vector2, step: float) -> PackedVector2Array:
	var out := PackedVector2Array()
	var d := from.distance_to(to)
	var n := int(d / step)
	if n <= 0:
		return out
	var dir := (to - from) / maxf(d, 0.001)
	var side := Vector2(-dir.y, dir.x)
	for i in range(2, n - 1):
		var h := fposmod(sin(float(i) * 7.31 + from.x * 0.53 + from.y * 0.29) * 43758.5453, 1.0)
		out.append(from.lerp(to, float(i) / float(n)) + side * ((h - 0.5) * 2.0 * PATH_WANDER))
	return out


# A 32 px patch of packed earth: an ellipse with a dithered rim and a few
# darker specks, top-down like every other structure sprite.
# `px` sizes the image: the village yards draw the default at ~16 world
# units (a texel per half unit, the ground tiles' grain); the 72-unit market
# square asks for a larger one so its texels stay that size instead of
# turning into 2-unit checker blocks at 8x.
static func yard_image(px: int = YARD_PX) -> Image:
	var img := Image.create(px, px, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	var earth := Color("8a6a42")
	var dark := Color("735634")
	var c := (px - 1) * 0.5
	var rx := float(px) * 15.5 / 32.0
	var ry := float(px) * 11.5 / 32.0
	for y in px:
		for x in px:
			var dx := (x - c) / rx
			var dy := (y - c) / ry
			var d := dx * dx + dy * dy
			if d > 1.0:
				continue
			# Dithered rim: alternate pixels drop out over the outer band.
			if d > 0.72 and (x + y) % 2 == 1:
				continue
			var speck := (x * 7 + y * 13) % 17 == 0
			img.set_pixel(x, y, dark if speck else earth)
	return img


# Cluster settlement sites whose anchors lie within `radius` of an already
# accepted (larger) site: the accepted site keeps its species id and position
# and absorbs the smaller site's members. Pure; sorted by members descending
# so the merge is deterministic for a given site list.
static func merge_sites(sites: Array, radius: float) -> Array:
	var ordered: Array = sites.duplicate()
	ordered.sort_custom(func(a, b): return int(a["members"]) > int(b["members"]))
	var merged: Array = []
	for site in ordered:
		var pos: Vector2 = site["pos"]
		var absorbed := false
		for head in merged:
			if (head["pos"] as Vector2).distance_to(pos) <= radius:
				head["members"] = int(head["members"]) + int(site["members"])
				absorbed = true
				break
		if not absorbed:
			merged.append(
				{"species_id": int(site["species_id"]), "pos": pos, "members": int(site["members"])}
			)
	return merged


func _redraw() -> void:
	var tick: int = int(sim.tick())
	_poll_events()
	# Fold the live sites into the village memory. Co-located anchors (several
	# lineages settling the same market square) merge into ONE village under
	# the largest lineage so footprints never stack on top of each other.
	for site in merge_sites(_sites, MERGE_RADIUS):
		var sid: int = int(site["species_id"])
		var v: Dictionary = _villages.get(sid, {})
		if v.is_empty():
			_villages[sid] = {
				"pos": site["pos"], "members": int(site["members"]), "born": _now, "seen": _now
			}
		else:
			# Ease the village anchor toward the live site. _redraw runs on a
			# frame count, not a wall-clock interval, so a bare 0.3 weight
			# would drift at half speed on a 30 fps machine; go through the
			# elapsed time instead. ANCHOR_TAU reproduces the old 60 fps feel.
			var k: float = FxMath.ease_factor(_now - _last_ease, ANCHOR_TAU)
			v["pos"] = (v["pos"] as Vector2).lerp(site["pos"], k)
			v["members"] = int(site["members"])
			v["seen"] = _now
	_last_ease = _now
	# One species stats lookup per redraw (adopted inventions drive era/flags).
	var stats_by_sid: Dictionary = {}
	for st in sim.species_stats():
		stats_by_sid[int(st["species_id"])] = st
	# Market field only matters when resources are active; sampling an empty
	# field cleanly disables all trade buildings below.
	var market_field: PackedColorArray = (
		sim.market_colors() if sim.resources_active() else PackedColorArray()
	)
	var market_res := int(sim.biome_resolution())
	var world_sz: float = sim.world_size()
	# Per-kind transform/colour accumulators for the two building families.
	var build_xf: Array = []
	var build_col: Array = []
	for k in Buildings.KIND_COUNT:
		build_xf.append([])
		build_col.append([])
	var struct_xf: Array = []
	var struct_col: Array = []
	for k in StructureSprites.KIND_COUNT:
		struct_xf.append([])
		struct_col.append([])
	var yard_xf: Array = []
	var yard_col: Array = []
	var shadow_xf: Array = []
	var shadow_col: Array = []
	var clearings: Array[Rect2] = []
	var fire_pos: PackedVector2Array = PackedVector2Array()
	for sid in _villages.keys():
		var v: Dictionary = _villages[sid]
		var stale: float = _now - float(v["seen"])
		if stale > LINGER:
			_villages.erase(sid)
			continue
		var fade: float = clampf((LINGER - stale) / FADE, 0.0, 1.0)
		var members: int = v["members"]
		var pos: Vector2 = v["pos"]
		var sp: int = MammalSprites.primate_skin_for(sid)
		var coat := Color(ApeSprites.PAL[ApeSprites.FIELD_ZONE_COLORS[sp]["c"]])
		var tint := Color(1, 1, 1).lerp(coat, 0.18)
		tint.a = fade
		var stats: Dictionary = stats_by_sid.get(sid, {})
		var adopted: PackedStringArray = stats.get("adopted_inventions", PackedStringArray())
		var era: int = era_for(adopted, _era_of)
		var recent: Dictionary = _recent_events.get(sid, {})
		var flags: int = flags_for(adopted, recent, tick)
		if _demo_era >= 0:
			era = _demo_era
			flags |= _demo_flags
		var sig: String = plan_signature(sid, members, era, flags)
		if String(v.get("plan_sig", "")) != sig:
			v["plan"] = VillageLayout.plan(
				sid, pos, members, era, flags, Callable(biome, "is_water_at")
			)
			v["plan_anchor"] = pos
			v["plan_sig"] = sig
		var plan: Array = v.get("plan", [])
		var plan_anchor: Vector2 = v.get("plan_anchor", pos)
		var delta: Vector2 = pos - plan_anchor
		var old_born: Dictionary = v.get("plan_born", {})
		var new_born: Dictionary = {}
		var smoke_rank := 99
		var smoke_pos: Vector2 = pos + Vector2(3.0, -12.0)
		var footprint := PackedVector2Array()
		for p in plan:
			var kind: int = int(p["kind"])
			var base_pos: Vector2 = p["pos"]
			var ppos: Vector2 = base_pos + delta
			footprint.append(ppos)
			# Keyed by kind and grid cell, not by the absolute position: the
			# anchor eases every redraw and a crowded square re-plans often,
			# and a key built from the exact position restarted every
			# structure's pop-in each time, leaving the whole village at
			# scale zero.
			var cell: Vector2i = Vector2i(((base_pos - plan_anchor) / VillageLayout.GRID).round())
			var pk: String = "%d:%d,%d" % [kind, cell.x, cell.y]
			var first_seen: bool = not old_born.has(pk)
			var born: float = float(old_born.get(pk, _now))
			new_born[pk] = born
			var flip: bool = bool(p.get("flip", false))
			var base_scale: float = float(StructureSprites.CELL_PX) * STRUCTURE_SCALE
			var pop: float = FxMath.pop_scale((_now - born) / POP_SECS)
			var s: float = base_scale * pop
			var sx: float = -s if flip else s
			# A tall kind's quad is half again as high, its base kept on the
			# footprint, so the extra wall rises up-screen.
			var tall: bool = StructureTall.is_tall(kind)
			var sy: float = s * TALL_RATIO if tall else s
			var spos: Vector2 = ppos - Vector2(0.0, (sy - s) * 0.5)
			struct_xf[kind].append(Transform2D(0.0, Vector2(sx, sy), 0.0, spos))
			# Each structure takes its own small tone swing from its cell, so
			# a row of huts or a block of fields is not one sprite stamped
			# over and over (the boards' roofs and crops vary hut to hut).
			var swing: float = tone_swing(sid, cell)
			struct_col[kind].append(Color(tint.r * swing, tint.g * swing, tint.b * swing, tint.a))
			var ys: float = yard_scale(kind)
			if ys > 0.0:
				shadow_xf.append(
					Transform2D(
						0.0, Vector2(s * SHADOW_W, s * SHADOW_H), 0.0, ppos + SHADOW_OFFSET * s
					)
				)
				shadow_col.append(Color(1, 1, 1, fade))
				var yw: float = base_scale * ys
				# Sits a little below the sprite's centre, under its footprint.
				yard_xf.append(Transform2D(0.0, Vector2(yw, yw), 0.0, ppos + Vector2(0.0, 3.0)))
				yard_col.append(Color(1, 1, 1, 0.9 * fade))
				if ys < HEARTH_YARD_SCALE:
					var pw: float = base_scale * PATH_SCALE
					for step_pos in path_steps(ppos + Vector2(0.0, 3.0), pos, PATH_STEP):
						yard_xf.append(Transform2D(0.0, Vector2(pw, pw), 0.0, step_pos))
						yard_col.append(Color(1, 1, 1, 0.75 * fade))
			var rank: int = _smoke_rank(kind)
			if rank >= 0 and rank < smoke_rank:
				smoke_rank = rank
				smoke_pos = ppos + Vector2(0.0, -16.0 if StructureTall.is_tall(kind) else -8.0)
			if kind == StructureSprites.RUIN_BURNT:
				if first_seen and _effects != null:
					_effects.spawn_embers(ppos)
				if fade > 0.5:
					fire_pos.append(ppos + Vector2(0.0, -3.0))
		v["plan_born"] = new_born
		v["smoke_pos"] = smoke_pos
		# The village stands in a clearing: its structures' bounds plus most
		# of a cell, so the scatter keeps off the outermost yards and fields.
		if not footprint.is_empty():
			clearings.append(Clearings.bounds_of(footprint, CLEARING_MARGIN))
		# Trade building: market/warehouse where the live market-density field
		# says this village sits on a real market, on a reserved slot north of
		# the anchor. (Invention landmarks are handled separately below, keyed
		# to the inventive lineages rather than to settlements.)
		if not market_field.is_empty():
			var ci := Buildings.market_cell(pos, world_sz, market_res)
			if ci >= 0 and ci < market_field.size():
				var tkind := Buildings.trade_kind(market_field[ci].r, members)
				if tkind >= 0:
					var pop_grow := clampf((_now - float(v["born"])) / POP_SECS, 0.0, 1.0)
					var ts := BUILDING_SCALE * FxMath.pop_scale(pop_grow)
					# The hi-res trade buildings draw their tall art with the
					# base kept where the square icon's was.
					var th := ts * Buildings.height_ratio(tkind)
					var tp := pos + Vector2(0.0, -26.0 - (th - ts) * 0.5)
					build_xf[tkind].append(Transform2D(0.0, Vector2(ts, th), 0.0, tp))
					build_col[tkind].append(Color(1, 1, 1, fade))
	_place_invention_landmarks(stats_by_sid, build_xf, build_col, yard_xf, yard_col, clearings)
	_assign_smoke()
	_assign_fires(fire_pos)
	for k in Buildings.KIND_COUNT:
		_write(_building_mmis[k].multimesh, build_xf[k], build_col[k])
	for k in StructureSprites.KIND_COUNT:
		_write(_structure_mmis[k].multimesh, struct_xf[k], struct_col[k])
	_write(_yard_mmi.multimesh, yard_xf, yard_col)
	_write(_shadow_mmi.multimesh, shadow_xf, shadow_col)
	Clearings.publish("villages", clearings)


# Invention landmarks mark tech-holding lineages that have NO settlement of
# their own — a settled species now draws its era architecture (mill, forge,
# scriptorium, granary…) from VillageLayout instead, so this path skips any
# sid present in `_villages`. The sim now gates the tech tree to apes (the
# PRIMATE archetype: omnivore + large), so only ape lineages ever carry
# inventions — `adopted_inventions` is populated for them alone, and these
# landmarks therefore appear only over apes. Each landmark is PINNED at the
# spot the lineage first reached its tech (a monument), not trailed after a
# nomadic herd. One pass over the alive arrays builds each species' centroid +
# head-count; qualifying lineages fold into the linger/fade memory, then draw
# below into the shared per-kind build accumulators.
func _place_invention_landmarks(
	stats_by_sid: Dictionary,
	build_xf: Array,
	build_col: Array,
	yard_xf: Array,
	yard_col: Array,
	clearings: Array[Rect2]
) -> void:
	var sp_ids: PackedInt32Array = sim.alive_species_ids()
	var sp_pos: PackedVector2Array = sim.alive_positions()
	var n := sp_ids.size()
	if n == 0 or sp_pos.size() != n:
		return
	var sum_pos: Dictionary = {}  # sid -> Vector2 sum of member positions
	var counts: Dictionary = {}  # sid -> member count
	for i in n:
		var s: int = sp_ids[i]
		sum_pos[s] = (sum_pos.get(s, Vector2.ZERO) as Vector2) + sp_pos[i]
		counts[s] = int(counts.get(s, 0)) + 1
	# Fold qualifying lineages into the landmark memory (pinned pos, latest
	# signature); draw from memory below so marks linger/fade like villages.
	for s in counts.keys():
		if _villages.has(s):
			continue
		var cnt: int = counts[s]
		if cnt < INVENTION_MIN_MEMBERS:
			continue
		var stats: Dictionary = stats_by_sid.get(s, {})
		var adopted: PackedStringArray = stats.get("adopted_inventions", PackedStringArray())
		var want := 2 if cnt >= LANDMARK2_MIN_MEMBERS else 1
		var sig: PackedInt32Array = Buildings.signature_kinds(adopted, _era_of, want)
		if sig.is_empty():
			continue
		var mark: Dictionary = _lineage_marks.get(s, {})
		if mark.is_empty():
			var centroid: Vector2 = (sum_pos[s] as Vector2) / float(cnt)
			_lineage_marks[s] = {"pos": centroid, "sig": sig, "born": _now, "seen": _now}
		else:
			# Position stays PINNED at the first-sighting centroid; only the
			# signature (tech can advance) and the seen-time refresh.
			mark["sig"] = sig
			mark["seen"] = _now
	for s in _lineage_marks.keys():
		var m: Dictionary = _lineage_marks[s]
		var stale: float = _now - float(m["seen"])
		if stale > LINGER:
			_lineage_marks.erase(s)
			continue
		var fade: float = clampf((LINGER - stale) / FADE, 0.0, 1.0)
		var grow: float = clampf((_now - float(m["born"])) / POP_SECS, 0.0, 1.0)
		var lscale := BUILDING_SCALE * FxMath.pop_scale(grow)
		var lcol := Color(1, 1, 1, fade)
		var pos: Vector2 = m["pos"]
		var msig: PackedInt32Array = m["sig"]
		# The ring of workshops stands in a clearing, like a village.
		var half := Vector2(LANDMARK_RING + lscale, LANDMARK_RING + lscale)
		clearings.append(Clearings.snap(Rect2(pos - half, half * 2.0)))
		for slot in msig.size():
			var kind: int = msig[slot]
			var ang: float = float(s) * 2.39996 + float(slot) * 2.0
			var lp := pos + Vector2.from_angle(ang) * LANDMARK_RING
			# Each workshop stands on its own dirt yard ...
			var yw: float = lscale * YARD_SCALE
			yard_xf.append(Transform2D(0.0, Vector2(yw, yw), 0.0, lp + Vector2(0.0, 3.0)))
			yard_col.append(Color(1, 1, 1, 0.9 * fade))
			# ... as a 32x44 walled front, base on the ring.
			var lh: float = lscale * Buildings.height_ratio(kind)
			lp.y -= (lh - lscale) * 0.5
			build_xf[kind].append(Transform2D(0.0, Vector2(lscale, lh), 0.0, lp))
			build_col[kind].append(lcol)


# Looping gray plumes so villages read as inhabited, not just built. A fixed
# pool keeps the cost flat; each redraw points the emitters at the largest
# still-fresh villages, anchored to a hearth/hall/forge placement when the
# village's plan has one. Not wrap-cloned (particle emitters can't share the
# MultiMesh trick; same tradeoff as the ember/dust effects).
func _make_smoke_pool() -> void:
	# Hard-edged pixel puffs (drawn unfiltered): the radial disc this used
	# blurred into a grey fog over the hearth at 4x.
	var tex := PixelFxSprites.build(PixelFxSprites.SMOKE)
	for i in SMOKE_POOL:
		var p := GPUParticles2D.new()
		p.name = "Smoke%d" % i
		p.amount = 10
		p.lifetime = 3.0
		p.emitting = false
		p.z_index = 2
		p.visibility_rect = Rect2(-100, -160, 200, 220)
		p.texture = tex
		p.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		var m := ParticleProcessMaterial.new()
		m.direction = Vector3(0, -1, 0)
		m.spread = 10.0
		m.initial_velocity_min = 5.0
		m.initial_velocity_max = 9.0
		# A touch of sideways gravity gives every plume the same gentle wind.
		m.gravity = Vector3(1.5, -5.0, 0)
		# Compact puffs (the disc is sized in world units; at 2x a two-unit
		# scale read as a grey haze over half the village).
		m.scale_min = 0.35
		m.scale_max = 0.6
		var grad := Gradient.new()
		grad.set_color(0, Color(1.0, 1.0, 1.0, 0.85))
		grad.set_color(1, Color(1.0, 1.0, 1.0, 0.0))
		var gt := GradientTexture1D.new()
		gt.gradient = grad
		m.color_ramp = gt
		p.process_material = m
		add_child(p)
		_smoke.append(p)


# One flame emitter and one dark plume per burning ruin: flames as pixel
# tongues licking up from the ruin's footprint (the boards' raided huts
# burn with a visible fire, not just an ember spray on the first frame),
# black smoke rising above them.
func _make_fire_pool() -> void:
	var flame := PixelFxSprites.build(PixelFxSprites.FLAME)
	var puff := PixelFxSprites.build(PixelFxSprites.SMOKE)
	for i in FIRE_POOL:
		var p := GPUParticles2D.new()
		p.name = "Fire%d" % i
		p.amount = 14
		p.lifetime = 0.8
		p.emitting = false
		p.z_index = 3
		p.visibility_rect = Rect2(-40, -60, 80, 80)
		p.texture = flame
		p.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		var m := ParticleProcessMaterial.new()
		m.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_BOX
		m.emission_box_extents = Vector3(5.0, 1.5, 1.0)
		m.direction = Vector3(0, -1, 0)
		m.spread = 12.0
		m.initial_velocity_min = 6.0
		m.initial_velocity_max = 12.0
		m.gravity = Vector3(0, -6.0, 0)
		m.scale_min = 0.3
		m.scale_max = 0.55
		var grad := Gradient.new()
		grad.set_color(0, Color(1.0, 1.0, 1.0, 1.0))
		grad.add_point(0.6, Color(1.0, 0.75, 0.55, 0.9))
		grad.set_color(1, Color(0.4, 0.15, 0.1, 0.0))
		var gt := GradientTexture1D.new()
		gt.gradient = grad
		m.color_ramp = gt
		p.process_material = m
		add_child(p)
		_fires.append(p)
		var s := GPUParticles2D.new()
		s.name = "FireSmoke%d" % i
		s.amount = 8
		s.lifetime = 2.4
		s.emitting = false
		s.z_index = 3
		s.visibility_rect = Rect2(-80, -140, 160, 180)
		s.texture = puff
		s.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		var sm := ParticleProcessMaterial.new()
		sm.direction = Vector3(0, -1, 0)
		sm.spread = 14.0
		sm.initial_velocity_min = 8.0
		sm.initial_velocity_max = 14.0
		sm.gravity = Vector3(2.0, -6.0, 0)
		sm.scale_min = 0.4
		sm.scale_max = 0.7
		var sg := Gradient.new()
		sg.set_color(0, Color(0.25, 0.22, 0.22, 0.9))
		sg.set_color(1, Color(0.3, 0.3, 0.32, 0.0))
		var sgt := GradientTexture1D.new()
		sgt.gradient = sg
		sm.color_ramp = sgt
		s.process_material = sm
		add_child(s)
		_fire_smoke.append(s)


func _assign_fires(positions: PackedVector2Array) -> void:
	for i in _fires.size():
		var on: bool = i < positions.size()
		if on:
			_fires[i].position = positions[i]
			_fire_smoke[i].position = positions[i] + Vector2(0.0, -6.0)
		_fires[i].emitting = on
		_fire_smoke[i].emitting = on


func _assign_smoke() -> void:
	if _smoke.is_empty():
		return
	var live: Array = []
	for sid in _villages.keys():
		var v: Dictionary = _villages[sid]
		# Only villages not yet fading: a plume over a ghost town reads wrong.
		if _now - float(v["seen"]) <= LINGER - FADE:
			var spos: Vector2 = v.get("smoke_pos", v["pos"])
			live.append([int(v["members"]), int(sid), spos])
	# Deterministic tiebreak on species id: sort_custom is not stable, and
	# equal-membership ties would otherwise swap emitters every redraw.
	live.sort_custom(
		func(a: Array, b: Array) -> bool: return a[0] > b[0] if a[0] != b[0] else a[1] < b[1]
	)
	for i in _smoke.size():
		var p := _smoke[i]
		if i < live.size():
			p.position = live[i][2] as Vector2
			p.emitting = true
		else:
			p.emitting = false


func _write(mm: MultiMesh, xfs: Array, cols: Array) -> void:
	var m := xfs.size()
	if m > mm.instance_count:
		mm.instance_count = m
	mm.visible_instance_count = m
	for i in m:
		mm.set_instance_transform_2d(i, xfs[i])
		mm.set_instance_color(i, cols[i])
