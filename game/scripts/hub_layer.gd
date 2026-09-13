extends Node2D

# Trade-hub layer: draws a marketplace building at each predetermined hub
# (Warehouse where market heat is high, Market otherwise) plus a small ring of
# trade-good icons for the goods that meet there. Hubs are worldgen fixtures —
# positions never move — so geometry is built once, then only the building
# choice refreshes with the live market field. Presentation over read-only sim
# state; plain no-shader MultiMesh (Metal-safe), same as settlement_layer.

const Buildings = preload("res://scripts/building_sprites.gd")
const Clearings = preload("res://scripts/clearings.gd")
const AgentLayer = preload("res://scripts/agent_layer.gd")
const SettlementLayer = preload("res://scripts/settlement_layer.gd")
const SpriteSplit = preload("res://scripts/sprite_split.gd")
const StructureSprites = preload("res://scripts/structure_sprites.gd")

const HUB_SCALE := 20.0
const GOOD_SCALE := 9.0
const GOOD_RING_RADIUS := 24.0
const REDRAW_EVERY := 30
# Market square (spec §6 Phase 4 step 5): a packed-earth square under the
# hub with a ring of awning stalls around the market building, each stall
# carrying one of the goods that meet there; goods beyond the stalls sit on
# the old icon ring. Stalls are cut like every other structure so a trader
# walks behind the awning and in front of the counter.
const SQUARE_SCALE := 72.0
# Square image size: a texel per half world unit, like the ground tiles.
const SQUARE_PX := 144
const STALL_SCALE := 16.0
const STALL_RADIUS := 22.0
const STALL_COUNT := 3

var _market_mmi: MultiMeshInstance2D
var _warehouse_mmi: MultiMeshInstance2D
var _good_mmis: Array[MultiMeshInstance2D] = []
var _square_mmi: MultiMeshInstance2D
# Contact shadows under the building and the stalls (same ellipse and
# offsets as the settlement dwellings), over the square, under the walls.
var _shadow_mmi: MultiMeshInstance2D
var _stall_mmi: MultiMeshInstance2D
var _stall_top_mmi: MultiMeshInstance2D
var _hubs: Array = []
var _frame: int = REDRAW_EVERY - 1

@onready var sim = get_node("../Simulation")


func _ready() -> void:
	_square_mmi = _make_layer(
		"Hub_Square",
		SpriteSplit.for_quad(SettlementLayer.yard_image(SQUARE_PX)),
		SettlementLayer.YARD_Z
	)
	_shadow_mmi = _make_layer(
		"Hub_Shadow",
		ImageTexture.create_from_image(AgentLayer.shadow_image(16)),
		SettlementLayer.SHADOW_Z
	)
	_shadow_mmi.modulate = SettlementLayer.SHADOW_COLOR
	var stall: Image = StructureSprites.kind_image(StructureSprites.STALL)
	var cut: int = SpriteSplit.split_row(stall)
	_stall_mmi = _make_layer("Hub_Stall", SpriteSplit.for_quad(SpriteSplit.lower(stall, cut)), -1)
	_stall_top_mmi = MultiMeshInstance2D.new()
	_stall_top_mmi.name = "Hub_StallTop"
	_stall_top_mmi.multimesh = _stall_mmi.multimesh
	_stall_top_mmi.texture = SpriteSplit.for_quad(SpriteSplit.upper(stall, cut))
	_stall_top_mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_stall_top_mmi.z_index = 1
	add_child(_stall_top_mmi)
	_market_mmi = _make_layer("Hub_Market", Buildings.build(Buildings.MARKET), 1)
	_warehouse_mmi = _make_layer("Hub_Warehouse", Buildings.build(Buildings.WAREHOUSE), 1)
	for g in Buildings.GOOD_COUNT:
		_good_mmis.append(
			_make_layer("Hub_Good_%s" % Buildings.GOOD_NAMES[g], Buildings.build_good(g), 2)
		)
	_make_wrap_clones()


# World positions of the stalls around a hub at `pos`: `count` slots on a
# ring, starting south-east so the first stall never hides the building.
static func stall_slots(
	pos: Vector2, count: int, radius: float = STALL_RADIUS
) -> PackedVector2Array:
	var out := PackedVector2Array()
	for i in count:
		var ang: float = TAU * (0.125 + float(i) / float(max(count, 1)))
		out.append(pos + Vector2.from_angle(ang) * radius)
	return out


static func _shadow_xf(pos: Vector2, size: float) -> Transform2D:
	return Transform2D(
		0.0,
		Vector2(size * SettlementLayer.SHADOW_W, size * SettlementLayer.SHADOW_H),
		0.0,
		pos + SettlementLayer.SHADOW_OFFSET * size
	)


func _make_layer(pname: String, tex: ImageTexture, z: int) -> MultiMeshInstance2D:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_2D
	mm.mesh = QuadMesh.new()
	var mmi := MultiMeshInstance2D.new()
	mmi.name = pname
	mmi.multimesh = mm
	mmi.texture = tex
	mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	mmi.z_index = z
	add_child(mmi)
	return mmi


func _make_wrap_clones() -> void:
	var world: float = sim.world_size()
	for src in (
		[_square_mmi, _shadow_mmi, _stall_mmi, _stall_top_mmi, _market_mmi, _warehouse_mmi]
		+ _good_mmis
	):
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


func _process(_delta: float) -> void:
	_frame += 1
	if _frame % REDRAW_EVERY != 0:
		return
	if _hubs.is_empty():
		_hubs = sim.trade_hubs()
		if _hubs.is_empty():
			return
	_redraw()


func _redraw() -> void:
	var market_field: PackedColorArray = (
		sim.market_colors() if sim.resources_active() else PackedColorArray()
	)
	var res := int(sim.biome_resolution())
	var world_sz: float = sim.world_size()
	var market_xf: Array = []
	var warehouse_xf: Array = []
	var square_xf: Array = []
	var stall_xf: Array = []
	var shadow_xf: Array = []
	var good_xf: Array = []
	var clearings: Array[Rect2] = []
	for g in Buildings.GOOD_COUNT:
		good_xf.append([])
	for hub in _hubs:
		var pos: Vector2 = hub["pos"]
		square_xf.append(Transform2D(0.0, Vector2(SQUARE_SCALE, SQUARE_SCALE * 0.8), 0.0, pos))
		# The square and its stalls stand on trodden ground: no scatter.
		var half := Vector2(SQUARE_SCALE, SQUARE_SCALE * 0.8) * 0.5
		clearings.append(Clearings.snap(Rect2(pos - half, half * 2.0)))
		var slots: PackedVector2Array = stall_slots(pos, STALL_COUNT)
		for i in slots.size():
			# Stalls east of the building face west (flipped) so their
			# counters open onto the square.
			var sx: float = -STALL_SCALE if slots[i].x > pos.x else STALL_SCALE
			stall_xf.append(Transform2D(0.0, Vector2(sx, STALL_SCALE), 0.0, slots[i]))
			shadow_xf.append(_shadow_xf(slots[i], STALL_SCALE))
		# Busy hub (hot market cell) -> warehouse, else market.
		var busy := false
		if not market_field.is_empty():
			var ci := Buildings.market_cell(pos, world_sz, res)
			if ci >= 0 and ci < market_field.size():
				busy = market_field[ci].r >= Buildings.MARKET_MIN
		var xf := Transform2D(0.0, Vector2(HUB_SCALE, HUB_SCALE), 0.0, pos)
		shadow_xf.append(_shadow_xf(pos, HUB_SCALE))
		if busy:
			warehouse_xf.append(xf)
		else:
			market_xf.append(xf)
		# Goods: the first few sit on the stall counters, the rest on the
		# old icon ring.
		var goods: PackedInt32Array = hub["goods"]
		for slot in goods.size():
			var gi: int = goods[slot]
			var gp: Vector2
			if slot < slots.size():
				gp = slots[slot] + Vector2(0.0, -1.0)
			else:
				var ang: float = TAU * float(slot) / float(max(goods.size(), 1))
				gp = pos + Vector2.from_angle(ang) * GOOD_RING_RADIUS
			good_xf[gi].append(Transform2D(0.0, Vector2(GOOD_SCALE, GOOD_SCALE), 0.0, gp))
	_write(_square_mmi.multimesh, square_xf)
	Clearings.publish("hubs", clearings)
	_write(_stall_mmi.multimesh, stall_xf)
	_write(_shadow_mmi.multimesh, shadow_xf)
	_write(_market_mmi.multimesh, market_xf)
	_write(_warehouse_mmi.multimesh, warehouse_xf)
	for g in Buildings.GOOD_COUNT:
		_write(_good_mmis[g].multimesh, good_xf[g])


func _write(mm: MultiMesh, xfs: Array) -> void:
	var m := xfs.size()
	if m > mm.instance_count:
		mm.instance_count = m
	mm.visible_instance_count = m
	for i in m:
		mm.set_instance_transform_2d(i, xfs[i])
