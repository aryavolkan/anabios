extends Node2D

# Settlement layer: persistent pixel hut clusters + tilled farm patches at the
# REAL settlement sites — the codex `settlement_active` latch per species, with
# the anchor centroid and member count from sim.settlement_sites(). Huts grow
# in number as membership grows; farms ring the larger settlements. Pure
# presentation over read-only sim state, refreshed a few times a second.

const ApeSprites = preload("res://scripts/ape_sprites.gd")
const MammalSprites = preload("res://scripts/mammal_sprites.gd")
const Buildings = preload("res://scripts/building_sprites.gd")
const FxMath = preload("res://scripts/fx_math.gd")

const REDRAW_EVERY := 20
# Huts are deliberately oversized next to agents (BODY_MIN ~6) so a village
# reads as architecture, not as a few more creatures.
const HUT_SCALE := 16.0
const FARM_SCALE := 11.0
const FARM_MIN_MEMBERS := 24
const MAX_HUTS := 6
const MAX_FARMS := 4
# Landmark/trade buildings sit a notch bigger than huts so a village's
# invention history and trade role read at a glance from the ring around it.
const BUILDING_SCALE := 16.0
const LANDMARK2_MIN_MEMBERS := 32
# Invention landmarks are anchored to the SPECIES that hold inventions, not to
# settlements: in organic runs the settling lineages are asocial foragers with
# no tech, while the inventive (cultural) lineages rarely settle. So a lineage
# needs this many live members before its tech earns a landmark at its centroid
# (keeps tiny splinter species from littering the map).
const INVENTION_MIN_MEMBERS := 25

# Chimney-smoke plume pool, assigned each redraw to the largest live villages.
const SMOKE_POOL := 8
# Construction pops are only sampled on the throttled redraw (~3/s), so the
# window must span several samples for the ease-out-back arc to actually show.
const POP_SECS := 1.5

var _huts: MultiMeshInstance2D
var _farms: MultiMeshInstance2D
var _smoke: Array[GPUParticles2D] = []
var _building_mmis: Array[MultiMeshInstance2D] = []
var _era_of: Dictionary = {}  # invention key -> era, cached once
var _frame: int = REDRAW_EVERY - 1  # redraw on the very first frame
# Villages linger: the sim's settlement latch drops the moment anchor cohesion
# breaks, but a place people built shouldn't vanish overnight — sites stay
# drawn for LINGER seconds after the sim stops reporting them, fading out.
const LINGER := 45.0
const FADE := 10.0
var _villages: Dictionary = {}  # sid -> {pos, members, born, seen}
# Per-lineage invention-landmark memory, same linger/fade contract as _villages
# so a landmark eases in when a lineage first earns its tech and lingers/fades
# when the lineage dies out, instead of popping. sid -> {pos, sig, born, seen}.
var _lineage_marks: Dictionary = {}
var _sites: Array = []  # last settlement_sites() result
var _now: float = 0.0

@onready var sim = get_node("/root/Main/Simulation")


func _ready() -> void:
	# Huts draw ABOVE agents (z=1): architecture looms over the crowd instead
	# of being perpetually covered by the villagers milling around it. Farms
	# stay at ground level. Both textures are pre-flipped for the MultiMesh
	# QuadMesh's flipped V axis (same convention as the ape atlas).
	_huts = _make_layer("Huts", _hut_texture(), 1)
	_farms = _make_layer("Farms", _farm_texture(), -6)
	# Landmark/trade buildings: one plain (no-shader) MultiMesh layer per
	# kind, drawn above agents like huts. Kept as separate layers (rather
	# than one shared atlas) so each building keeps its own untouched
	# texture on the Metal-safe plain-MultiMesh path.
	for k in Buildings.KIND_COUNT:
		var tex := Buildings.build(k)
		var mmi := _make_layer("Building_%s" % Buildings.NAMES[k], tex, 1)
		_building_mmis.append(mmi)
	for inv in sim.invention_catalog():
		_era_of[String(inv["key"])] = int(inv["era"])
	_make_smoke_pool()
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
	for src in [_huts, _farms] + _building_mmis:
		for gy in range(-1, 2):
			for gx in range(-1, 2):
				if gx == 0 and gy == 0:
					continue
				var clone := MultiMeshInstance2D.new()
				clone.multimesh = src.multimesh
				clone.texture = src.texture
				clone.texture_filter = src.texture_filter
				clone.z_index = src.z_index
				clone.position = Vector2(gx * world, gy * world)
				add_child(clone)


func _process(_delta: float) -> void:
	_now = Time.get_ticks_msec() / 1000.0
	_frame += 1
	if _frame % REDRAW_EVERY != 0:
		return
	_sites = sim.settlement_sites()
	_redraw()


func random_site_pos() -> Vector2:
	if _sites.is_empty():
		return Vector2.ZERO
	return _sites[randi() % _sites.size()]["pos"]


func has_sites() -> bool:
	return not _sites.is_empty()


func _redraw() -> void:
	# Fold the live sites into the village memory.
	for site in _sites:
		var sid: int = int(site["species_id"])
		var v: Dictionary = _villages.get(sid, {})
		if v.is_empty():
			_villages[sid] = {
				"pos": site["pos"], "members": int(site["members"]), "born": _now, "seen": _now
			}
		else:
			v["pos"] = (v["pos"] as Vector2).lerp(site["pos"], 0.3)
			v["members"] = int(site["members"])
			v["seen"] = _now
	var hut_xf: Array = []
	var hut_col: Array = []
	var farm_xf: Array = []
	# One species stats lookup per redraw (adopted inventions drive landmarks).
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
	# Per-kind transform/colour accumulators for the building MultiMeshes.
	var build_xf: Array = []
	var build_col: Array = []
	for k in Buildings.KIND_COUNT:
		build_xf.append([])
		build_col.append([])
	for sid in _villages.keys():
		var v: Dictionary = _villages[sid]
		var stale: float = _now - float(v["seen"])
		if stale > LINGER:
			_villages.erase(sid)
			continue
		var fade: float = clampf((LINGER - stale) / FADE, 0.0, 1.0)
		var grow: float = clampf((_now - float(v["born"])) / 0.6, 0.0, 1.0)
		var ease := 1.0 - pow(1.0 - grow, 3.0)
		var members: int = v["members"]
		var pos: Vector2 = v["pos"]
		var sp: int = MammalSprites.primate_skin_for(sid)
		var coat := Color(ApeSprites.PAL[ApeSprites.FIELD_ZONE_COLORS[sp]["c"]])
		var tint := Color(1, 1, 1).lerp(coat, 0.18)
		tint.a = fade
		var huts: int = clampi(members / 8, 1, MAX_HUTS)
		# Each hut remembers when it first appeared, so huts added as the
		# village grows get their own construction pop instead of snapping in.
		var hut_born: Array = v.get("hut_born", [])
		while hut_born.size() < huts:
			hut_born.append(_now)
		if hut_born.size() > huts:
			hut_born.resize(huts)
		v["hut_born"] = hut_born
		for i in huts:
			# Deterministic ring layout per species: golden-angle step keeps
			# huts scattered without churn as membership changes the count.
			var ang: float = sid * 2.39996 + i * 2.39996
			var r: float = 8.0 + float(i % 3) * 4.5
			var hp := pos + Vector2.from_angle(ang) * r
			var s := HUT_SCALE * FxMath.pop_scale((_now - float(hut_born[i])) / POP_SECS)
			hut_xf.append(Transform2D(0.0, Vector2(s, s), 0.0, hp))
			hut_col.append(tint)
		if members >= FARM_MIN_MEMBERS:
			var farms: int = mini(1 + members / 16, MAX_FARMS)
			for i in farms:
				var ang2: float = sid * 1.7 + i * (TAU / farms)
				var fp := pos + Vector2.from_angle(ang2) * (24.0 + float(i % 2) * 9.0)
				var fs := FARM_SCALE * ease
				# Quarter-turn steps, not the raw bearing: the plot sprite is
				# 16x16 pixel art, and at an arbitrary angle its furrows aliased
				# into a ragged brown lozenge. Snapping keeps the edges crisp
				# and still gives each plot one of two orientations.
				var frot: float = snappedf(ang2 * 0.5, PI * 0.5)
				farm_xf.append(Transform2D(frot, Vector2(fs, fs), 0.0, fp))
		# Trade building: market/warehouse where the live market-density field
		# says this village sits on a real market, on a reserved slot north of
		# the anchor. (Invention landmarks are handled separately below, keyed to
		# the inventive lineages rather than to settlements.)
		if not market_field.is_empty():
			var ci := Buildings.market_cell(pos, world_sz, market_res)
			if ci >= 0 and ci < market_field.size():
				var tkind := Buildings.trade_kind(market_field[ci].r, members)
				if tkind >= 0:
					var tp := pos + Vector2(0.0, -26.0)
					var pop_grow := clampf((_now - float(v["born"])) / POP_SECS, 0.0, 1.0)
					var ts := BUILDING_SCALE * FxMath.pop_scale(pop_grow)
					build_xf[tkind].append(Transform2D(0.0, Vector2(ts, ts), 0.0, tp))
					build_col[tkind].append(Color(1, 1, 1, fade))
	_place_invention_landmarks(stats_by_sid, build_xf, build_col)
	_assign_smoke()
	_write(_huts.multimesh, hut_xf, hut_col)
	_write_farms(farm_xf)
	for k in Buildings.KIND_COUNT:
		_write(_building_mmis[k].multimesh, build_xf[k], build_col[k])


# Invention landmarks mark the lineages that hold inventions. The sim now gates
# the tech tree to apes (the PRIMATE archetype: omnivore + large), so only ape
# lineages ever carry inventions — `adopted_inventions` is populated for them
# alone, and these landmarks therefore appear only over apes. Each landmark is
# PINNED at the spot the lineage first reached its tech (a monument), not trailed
# after a nomadic herd. One pass over the alive arrays builds each species'
# centroid + head-count; qualifying lineages fold into the linger/fade memory,
# then draw below into the shared per-kind build accumulators.
func _place_invention_landmarks(
	stats_by_sid: Dictionary, build_xf: Array, build_col: Array
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
		for slot in msig.size():
			var kind: int = msig[slot]
			var ang: float = float(s) * 2.39996 + float(slot) * 2.0
			var lp := pos + Vector2.from_angle(ang) * 22.0
			build_xf[kind].append(Transform2D(0.0, Vector2(lscale, lscale), 0.0, lp))
			build_col[kind].append(lcol)


# Looping gray plumes so villages read as inhabited, not just built. A fixed
# pool keeps the cost flat; each redraw points the emitters at the largest
# still-fresh villages. Not wrap-cloned (particle emitters can't share the
# MultiMesh trick; same tradeoff as the ember/dust effects).
func _make_smoke_pool() -> void:
	var tex := FxMath.radial_texture(16)
	for i in SMOKE_POOL:
		var p := GPUParticles2D.new()
		p.name = "Smoke%d" % i
		p.amount = 12
		p.lifetime = 3.0
		p.emitting = false
		p.z_index = 2
		p.visibility_rect = Rect2(-100, -160, 200, 220)
		p.texture = tex
		var m := ParticleProcessMaterial.new()
		m.direction = Vector3(0, -1, 0)
		m.spread = 10.0
		m.initial_velocity_min = 5.0
		m.initial_velocity_max = 9.0
		# A touch of sideways gravity gives every plume the same gentle wind.
		m.gravity = Vector3(1.5, -5.0, 0)
		m.scale_min = 1.1
		m.scale_max = 2.0
		var grad := Gradient.new()
		grad.set_color(0, Color(0.62, 0.58, 0.55, 0.28))
		grad.set_color(1, Color(0.55, 0.55, 0.58, 0.0))
		var gt := GradientTexture1D.new()
		gt.gradient = grad
		m.color_ramp = gt
		p.process_material = m
		add_child(p)
		_smoke.append(p)


func _assign_smoke() -> void:
	if _smoke.is_empty():
		return
	var live: Array = []
	for sid in _villages.keys():
		var v: Dictionary = _villages[sid]
		# Only villages not yet fading: a plume over a ghost town reads wrong.
		if _now - float(v["seen"]) <= LINGER - FADE:
			live.append([int(v["members"]), int(sid), v["pos"] as Vector2])
	# Deterministic tiebreak on species id: sort_custom is not stable, and
	# equal-membership ties would otherwise swap emitters every redraw.
	live.sort_custom(
		func(a: Array, b: Array) -> bool: return a[0] > b[0] if a[0] != b[0] else a[1] < b[1]
	)
	for i in _smoke.size():
		var p := _smoke[i]
		if i < live.size():
			# Offset to sit over a hut roof rather than the bare anchor.
			p.position = (live[i][2] as Vector2) + Vector2(3.0, -12.0)
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


func _write_farms(xfs: Array) -> void:
	var mm: MultiMesh = _farms.multimesh
	var m := xfs.size()
	if m > mm.instance_count:
		mm.instance_count = m
	mm.visible_instance_count = m
	for i in m:
		mm.set_instance_transform_2d(i, xfs[i])
		mm.set_instance_color(i, Color(1, 1, 1))


# 16x16 hut: pale wood walls, pitched thatch roof, dark door, auto-outline.
func _hut_texture() -> ImageTexture:
	var blocks: Array = [
		[6, 2, 4, 1, "B"],
		[5, 3, 6, 1, "B"],
		[4, 4, 8, 1, "B"],
		[3, 5, 10, 1, "B"],
		[2, 6, 12, 1, "b"],
		[3, 7, 10, 7, "t"],
		[7, 9, 3, 5, "K"],
		[3, 7, 10, 1, "m"],
	]
	var img: Image = ApeSprites._build_cell(blocks)
	img.flip_y()
	return ImageTexture.create_from_image(img)


# 16x16 tilled patch: dark furrows on bare earth with a few crop sprouts.
func _farm_texture() -> ImageTexture:
	var blocks: Array = [
		[1, 1, 14, 14, "b"],
		[2, 3, 12, 1, "K"],
		[2, 6, 12, 1, "K"],
		[2, 9, 12, 1, "K"],
		[2, 12, 12, 1, "K"],
		[4, 2, 1, 1, "h"],
		[9, 5, 1, 1, "h"],
		[12, 8, 1, 1, "h"],
		[6, 11, 1, 1, "h"],
	]
	var img: Image = ApeSprites._build_cell(blocks)
	img.flip_y()
	return ImageTexture.create_from_image(img)
