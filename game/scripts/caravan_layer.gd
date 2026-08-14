extends Node2D

# Trade caravans: a fixed route network links each hub to its nearest neighbours,
# and short cart trains shuttle along every route. Each cart carries a trade-good
# icon; a route's cart cargo is apportioned (largest-remainder) to the summed
# per-good trade tally of its two endpoint hubs, so busy Salt routes haul mostly
# Salt. Pure presentation over read-only sim state; the sim is unchanged.

const Buildings = preload("res://scripts/building_sprites.gd")

const CARAVAN_NEIGHBORS := 2  # edges added per hub (undirected, deduped)
const CARTS_PER_ROUTE := 3
const TRAVERSE_PERIOD := 10.0  # seconds for one out-and-back along a route
const CART_GAP_FRAC := 0.08  # even fractional spacing between carts in a convoy
const CART_SCALE := 11.0
const GOOD_SCALE := 7.0
const GOOD_DY := -9.0  # goods icon rides above the cart
const REDRAW_MIX_EVERY := 40
const LINE_COLOR := Color(0.85, 0.80, 0.55, 0.18)
const LINE_DASH := 8.0

var _cart_mmi: MultiMeshInstance2D
var _good_mmis: Array[MultiMeshInstance2D] = []
var _hubs: Array = []
var _routes: Array = []  # each: {a, b, pa: Vector2, pb: Vector2, cargo: PackedInt32Array}
var _t: float = 0.0
var _frame: int = 0

@onready var sim = get_node("/root/Main/Simulation")


func _ready() -> void:
	_cart_mmi = _make_layer("Caravan_Cart", Buildings.build_cart(), 2)
	for g in Buildings.GOOD_COUNT:
		_good_mmis.append(_make_layer("Caravan_Good_%d" % g, Buildings.build_good(g), 3))
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


func _make_wrap_clones() -> void:
	var world: float = sim.world_size()
	for src in [_cart_mmi] + _good_mmis:
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


func _min_image(from: Vector2, to: Vector2, world: float) -> Vector2:
	var d := to - from
	d.x = fposmod(d.x + world * 0.5, world) - world * 0.5
	d.y = fposmod(d.y + world * 0.5, world) - world * 0.5
	return from + d


# Nearest-neighbour route network over the hub positions (torus-aware), each hub
# linked to its CARAVAN_NEIGHBORS closest others; undirected edges deduped.
func _build_routes() -> void:
	_routes.clear()
	var n := _hubs.size()
	if n < 2:
		return
	var world: float = sim.world_size()
	var seen := {}
	for i in n:
		var pi: Vector2 = _hubs[i]["pos"]
		var dists: Array = []
		for j in n:
			if j == i:
				continue
			var pj := _min_image(pi, _hubs[j]["pos"], world)
			dists.append({"j": j, "d": pi.distance_squared_to(pj), "pj": pj})
		dists.sort_custom(func(x, y): return x["d"] < y["d"])
		for k in mini(CARAVAN_NEIGHBORS, dists.size()):
			var j: int = dists[k]["j"]
			var key := "%d-%d" % [mini(i, j), maxi(i, j)]
			if seen.has(key):
				continue
			seen[key] = true
			_routes.append(
				{"a": i, "b": j, "pa": pi, "pb": dists[k]["pj"], "cargo": PackedInt32Array()}
			)


# Apportion CARTS_PER_ROUTE carts to goods by largest-remainder over the summed
# per-good tally of the route's two endpoint hubs. Empty (all -1) until trades.
func _recompute_cargo(tallies: Array) -> void:
	for r in _routes:
		var sums := PackedInt32Array()
		sums.resize(Buildings.GOOD_COUNT)
		var total := 0
		for hub_idx in [r["a"], r["b"]]:
			if hub_idx < tallies.size():
				var t: PackedInt32Array = tallies[hub_idx]
				for g in mini(t.size(), Buildings.GOOD_COUNT):
					sums[g] += t[g]
					total += t[g]
		var cargo := PackedInt32Array()
		cargo.resize(CARTS_PER_ROUTE)
		if total <= 0:
			for c in CARTS_PER_ROUTE:
				cargo[c] = -1
			r["cargo"] = cargo
			continue
		# Largest-remainder apportionment.
		var alloc := PackedInt32Array()
		alloc.resize(Buildings.GOOD_COUNT)
		var rema: Array = []
		var used := 0
		for g in Buildings.GOOD_COUNT:
			var exact := float(sums[g]) * float(CARTS_PER_ROUTE) / float(total)
			var base := int(floor(exact))
			alloc[g] = base
			used += base
			rema.append({"g": g, "r": exact - float(base)})
		rema.sort_custom(func(x, y): return x["r"] > y["r"])
		var leftover := CARTS_PER_ROUTE - used
		for m in leftover:
			alloc[rema[m % rema.size()]["g"]] += 1
		var idx := 0
		for g in Buildings.GOOD_COUNT:
			for _c in alloc[g]:
				if idx < CARTS_PER_ROUTE:
					cargo[idx] = g
					idx += 1
		r["cargo"] = cargo


func _process(delta: float) -> void:
	_t += delta
	_frame += 1
	if _routes.is_empty():
		_hubs = sim.trade_hubs()
		if _hubs.is_empty():
			return
		_build_routes()
		queue_redraw()  # paint the (static) route lines once
	if _frame % REDRAW_MIX_EVERY == 0:
		_recompute_cargo(sim.hub_trade_tally())
	_animate()


# Place each route's cart convoy along its segment. Carts keep a FIXED even
# spacing (CART_GAP_FRAC) and the convoy centre ping-pongs within a bounded band,
# so carts never bunch up against the route ends. Writes per-instance transforms.
func _animate() -> void:
	var cart_xf: Array = []
	var good_xf: Array = []
	for g in Buildings.GOOD_COUNT:
		good_xf.append([])
	# Convoy centre travels within [half, 1-half] so all carts stay on the route.
	var half := CART_GAP_FRAC * float(CARTS_PER_ROUTE - 1) * 0.5
	var center := lerpf(half, 1.0 - half, pingpong(_t / TRAVERSE_PERIOD, 1.0))
	var mid := float(CARTS_PER_ROUTE - 1) * 0.5
	for r in _routes:
		var pa: Vector2 = r["pa"]
		var pb: Vector2 = r["pb"]
		var cargo: PackedInt32Array = r["cargo"]
		for c in CARTS_PER_ROUTE:
			var f: float = center + (float(c) - mid) * CART_GAP_FRAC
			var p := pa.lerp(pb, f)
			cart_xf.append(Transform2D(0.0, Vector2(CART_SCALE, CART_SCALE), 0.0, p))
			if c < cargo.size():
				var gi: int = cargo[c]
				if gi >= 0 and gi < Buildings.GOOD_COUNT:
					var gp := p + Vector2(0.0, GOOD_DY)
					good_xf[gi].append(Transform2D(0.0, Vector2(GOOD_SCALE, GOOD_SCALE), 0.0, gp))
	_write(_cart_mmi.multimesh, cart_xf)
	for g in Buildings.GOOD_COUNT:
		_write(_good_mmis[g].multimesh, good_xf[g])


func _write(mm: MultiMesh, xfs: Array) -> void:
	var m := xfs.size()
	if m > mm.instance_count:
		mm.instance_count = m
	mm.visible_instance_count = m
	for i in m:
		mm.set_instance_transform_2d(i, xfs[i])
		mm.set_instance_color(i, Color(1, 1, 1))


# Faint dashed route lines, drawn at all 9 torus offsets so seam-crossing routes
# read correctly. Static: repainted only when the route network is (re)built.
func _draw() -> void:
	if _routes.is_empty():
		return
	var world: float = sim.world_size()
	for gy in range(-1, 2):
		for gx in range(-1, 2):
			var off := Vector2(gx * world, gy * world)
			for r in _routes:
				draw_dashed_line(
					(r["pa"] as Vector2) + off,
					(r["pb"] as Vector2) + off,
					LINE_COLOR,
					1.0,
					LINE_DASH
				)
