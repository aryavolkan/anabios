extends Node2D

# Trade-hub layer: draws a marketplace building at each predetermined hub
# (Warehouse where market heat is high, Market otherwise) plus a small ring of
# trade-good icons for the goods that meet there. Hubs are worldgen fixtures —
# positions never move — so geometry is built once, then only the building
# choice refreshes with the live market field. Presentation over read-only sim
# state; plain no-shader MultiMesh (Metal-safe), same as settlement_layer.

const Buildings = preload("res://scripts/building_sprites.gd")

const HUB_SCALE := 20.0
const GOOD_SCALE := 9.0
const GOOD_RING_RADIUS := 24.0
const REDRAW_EVERY := 30

var _market_mmi: MultiMeshInstance2D
var _warehouse_mmi: MultiMeshInstance2D
var _good_mmis: Array[MultiMeshInstance2D] = []
var _hubs: Array = []
var _frame: int = REDRAW_EVERY - 1

@onready var sim = get_node("/root/Main/Simulation")


func _ready() -> void:
	_market_mmi = _make_layer("Hub_Market", Buildings.build(Buildings.MARKET))
	_warehouse_mmi = _make_layer("Hub_Warehouse", Buildings.build(Buildings.WAREHOUSE))
	for g in Buildings.GOOD_COUNT:
		_good_mmis.append(
			_make_layer("Hub_Good_%s" % Buildings.GOOD_NAMES[g], Buildings.build_good(g))
		)
	_make_wrap_clones()


func _make_layer(pname: String, tex: ImageTexture) -> MultiMeshInstance2D:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_2D
	mm.mesh = QuadMesh.new()
	var mmi := MultiMeshInstance2D.new()
	mmi.name = pname
	mmi.multimesh = mm
	mmi.texture = tex
	mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	mmi.z_index = 1
	add_child(mmi)
	return mmi


func _make_wrap_clones() -> void:
	var world: float = sim.world_size()
	for src in [_market_mmi, _warehouse_mmi] + _good_mmis:
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
	var good_xf: Array = []
	for g in Buildings.GOOD_COUNT:
		good_xf.append([])
	for hub in _hubs:
		var pos: Vector2 = hub["pos"]
		# Busy hub (hot market cell) -> warehouse, else market.
		var busy := false
		if not market_field.is_empty():
			var ci := Buildings.market_cell(pos, world_sz, res)
			if ci >= 0 and ci < market_field.size():
				busy = market_field[ci].r >= Buildings.MARKET_MIN
		var xf := Transform2D(0.0, Vector2(HUB_SCALE, HUB_SCALE), 0.0, pos)
		if busy:
			warehouse_xf.append(xf)
		else:
			market_xf.append(xf)
		# Goods ring: one icon per good that meets at this hub.
		var goods: PackedInt32Array = hub["goods"]
		for slot in goods.size():
			var gi: int = goods[slot]
			var ang: float = TAU * float(slot) / float(max(goods.size(), 1))
			var gp := pos + Vector2.from_angle(ang) * GOOD_RING_RADIUS
			good_xf[gi].append(Transform2D(0.0, Vector2(GOOD_SCALE, GOOD_SCALE), 0.0, gp))
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
