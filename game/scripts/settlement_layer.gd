extends Node2D

# Settlement layer: persistent pixel hut clusters + tilled farm patches at the
# REAL settlement sites — the codex `settlement_active` latch per species, with
# the anchor centroid and member count from sim.settlement_sites(). Huts grow
# in number as membership grows; farms ring the larger settlements. Pure
# presentation over read-only sim state, refreshed a few times a second.

const ApeSprites = preload("res://scripts/ape_sprites.gd")

const REDRAW_EVERY := 20
# Huts are deliberately oversized next to agents (BODY_MIN ~6) so a village
# reads as architecture, not as a few more creatures.
const HUT_SCALE := 16.0
const FARM_SCALE := 11.0
const FARM_MIN_MEMBERS := 24
const MAX_HUTS := 6
const MAX_FARMS := 4

var _huts: MultiMeshInstance2D
var _farms: MultiMeshInstance2D
var _frame: int = REDRAW_EVERY - 1   # redraw on the very first frame
# Villages linger: the sim's settlement latch drops the moment anchor cohesion
# breaks, but a place people built shouldn't vanish overnight — sites stay
# drawn for LINGER seconds after the sim stops reporting them, fading out.
const LINGER := 45.0
const FADE := 10.0
var _villages: Dictionary = {}   # sid -> {pos, members, born, seen}
var _sites: Array = []           # last settlement_sites() result
var _now: float = 0.0

@onready var sim = get_node("/root/Main/Simulation")

func _ready() -> void:
	# Huts draw ABOVE agents (z=1): architecture looms over the crowd instead
	# of being perpetually covered by the villagers milling around it. Farms
	# stay at ground level. Both textures are pre-flipped for the MultiMesh
	# QuadMesh's flipped V axis (same convention as the ape atlas).
	_huts = _make_layer("Huts", _hut_texture(), 1)
	_farms = _make_layer("Farms", _farm_texture(), -6)
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
	for src in [_huts, _farms]:
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
			_villages[sid] = {"pos": site["pos"], "members": int(site["members"]), "born": _now, "seen": _now}
		else:
			v["pos"] = (v["pos"] as Vector2).lerp(site["pos"], 0.3)
			v["members"] = int(site["members"])
			v["seen"] = _now
	var hut_xf: Array = []
	var hut_col: Array = []
	var farm_xf: Array = []
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
		var sp: int = ApeSprites.ape_for_species(sid)
		var coat := Color(ApeSprites.PAL[ApeSprites.FIELD_ZONE_COLORS[sp]["c"]])
		var tint := Color(1, 1, 1).lerp(coat, 0.18)
		tint.a = fade
		var huts: int = clampi(members / 8, 1, MAX_HUTS)
		for i in huts:
			# Deterministic ring layout per species: golden-angle step keeps
			# huts scattered without churn as membership changes the count.
			var ang: float = sid * 2.39996 + i * 2.39996
			var r: float = 8.0 + float(i % 3) * 4.5
			var hp := pos + Vector2.from_angle(ang) * r
			var s := HUT_SCALE * ease
			hut_xf.append(Transform2D(0.0, Vector2(s, s), 0.0, hp))
			hut_col.append(tint)
		if members >= FARM_MIN_MEMBERS:
			var farms: int = mini(1 + members / 16, MAX_FARMS)
			for i in farms:
				var ang2: float = sid * 1.7 + i * (TAU / farms)
				var fp := pos + Vector2.from_angle(ang2) * (24.0 + float(i % 2) * 9.0)
				var fs := FARM_SCALE * ease
				farm_xf.append(Transform2D(ang2 * 0.5, Vector2(fs, fs), 0.0, fp))
	_write(_huts.multimesh, hut_xf, hut_col)
	_write_farms(farm_xf)

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
		[6, 2, 4, 1, "B"], [5, 3, 6, 1, "B"], [4, 4, 8, 1, "B"], [3, 5, 10, 1, "B"],
		[2, 6, 12, 1, "b"], [3, 7, 10, 7, "t"], [7, 9, 3, 5, "K"], [3, 7, 10, 1, "m"],
	]
	var img: Image = ApeSprites._build_cell(blocks)
	img.flip_y()
	return ImageTexture.create_from_image(img)

# 16x16 tilled patch: dark furrows on bare earth with a few crop sprouts.
func _farm_texture() -> ImageTexture:
	var blocks: Array = [
		[1, 1, 14, 14, "b"],
		[2, 3, 12, 1, "K"], [2, 6, 12, 1, "K"], [2, 9, 12, 1, "K"], [2, 12, 12, 1, "K"],
		[4, 2, 1, 1, "h"], [9, 5, 1, 1, "h"], [12, 8, 1, 1, "h"], [6, 11, 1, 1, "h"],
	]
	var img: Image = ApeSprites._build_cell(blocks)
	img.flip_y()
	return ImageTexture.create_from_image(img)
