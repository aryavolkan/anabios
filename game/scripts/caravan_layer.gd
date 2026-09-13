extends Node2D

# Trade caravans: a fixed route network links each hub to its nearest neighbours,
# and short cart trains shuttle along every route. Each cart carries a trade-good
# icon; a route's cart cargo is apportioned (largest-remainder) to the summed
# per-good trade tally of its two endpoint hubs, so busy Salt routes haul mostly
# Salt. Pure presentation over read-only sim state; the sim is unchanged.

const Buildings = preload("res://scripts/building_sprites.gd")
const FxMath = preload("res://scripts/fx_math.gd")
const SettlementLayer = preload("res://scripts/settlement_layer.gd")
const SpriteSplit = preload("res://scripts/sprite_split.gd")

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
# Convoy geometry, derived once from the cart constants (loop-invariant).
const CONVOY_HALF := CART_GAP_FRAC * (CARTS_PER_ROUTE - 1) * 0.5
# Carts halt at the edge of the market square (hub_layer draws the square
# SQUARE_SCALE wide around the hub), the square's "gate", instead of
# driving over the stalls: each route is trimmed by this much at both ends.
const GATE_MARGIN := 34.0
const CART_MID := (CARTS_PER_ROUTE - 1) * 0.5

var _cart_mmi: MultiMeshInstance2D
# Dirt roads: the yard-earth patch laid every ROAD_STEP units along each
# route (skipping water cells, with a little side-to-side wander), so the
# caravan routes read as the boards' worn tracks between settlements
# instead of a dashed line. Built once with the route network.
var _road_mmi: MultiMeshInstance2D
const ROAD_STEP := 6.0
const ROAD_SCALE := 9.0
const ROAD_WANDER := 1.5
const ROAD_ALPHA := 0.8
var _good_mmis: Array[MultiMeshInstance2D] = []
var _hubs: Array = []
var _routes: Array = []  # each: {a, b, pa: Vector2, pb: Vector2, cargo: PackedInt32Array}
var _t: float = 0.0
var _frame: int = 0
var _built: bool = false  # route network built once (hubs are immutable at runtime)

@onready var sim = get_node("../Simulation")
@onready var _biome = get_node_or_null("../Biome")


func _ready() -> void:
	_road_mmi = _make_layer(
		"Caravan_Road", SpriteSplit.for_quad(SettlementLayer.yard_image()), SettlementLayer.YARD_Z
	)
	_road_mmi.modulate = Color(1, 1, 1, ROAD_ALPHA)
	_cart_mmi = _make_layer("Caravan_Cart", Buildings.build_cart(), 2)
	for g in Buildings.GOOD_COUNT:
		_good_mmis.append(_make_layer("Caravan_Good_%d" % g, Buildings.build_good(g), 3))
	_make_wrap_clones()


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
	for src in [_road_mmi, _cart_mmi] + _good_mmis:
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
			var ends: PackedVector2Array = gate_ends(pi, dists[k]["pj"], GATE_MARGIN)
			_routes.append(
				{"a": i, "b": j, "pa": ends[0], "pb": ends[1], "cargo": PackedInt32Array()}
			)
	_lay_roads()


# Road patches along every route, the water cells left bare.
func _lay_roads() -> void:
	var is_water := Callable(_biome, "is_water_at") if _biome != null else Callable()
	var xfs: Array = []
	for r in _routes:
		for p in road_steps(r["pa"], r["pb"], ROAD_STEP, is_water):
			xfs.append(Transform2D(0.0, Vector2(ROAD_SCALE, ROAD_SCALE * 0.8), 0.0, p))
	_write(_road_mmi.multimesh, xfs)


# Patch centres for a road from `pa` to `pb`: one every `step` units,
# wandering up to ROAD_WANDER sideways on a stable hash, none on a water
# cell (`is_water` may be an empty Callable: every step is land).
static func road_steps(
	pa: Vector2, pb: Vector2, step: float, is_water: Callable
) -> PackedVector2Array:
	var out := PackedVector2Array()
	var d := pb - pa
	var len := d.length()
	if len < step or step <= 0.0:
		return out
	var dir := d / len
	var side := Vector2(-dir.y, dir.x)
	var n := int(len / step)
	for i in range(1, n):
		var h := fposmod(sin(float(i) * 12.9898 + pa.x * 0.37 + pa.y * 0.73) * 43758.5453, 1.0)
		var p := pa + dir * (step * i) + side * ((h - 0.5) * 2.0 * ROAD_WANDER)
		if is_water.is_valid() and bool(is_water.call(p)):
			continue
		out.append(p)
	return out


# The route's endpoints pulled in by `margin` from each hub centre (the
# square's gate); a route too short for two margins keeps its centres.
static func gate_ends(pa: Vector2, pb: Vector2, margin: float) -> PackedVector2Array:
	var d := pb - pa
	var len := d.length()
	if len <= margin * 2.0 + 1.0:
		return PackedVector2Array([pa, pb])
	var dir := d / len
	return PackedVector2Array([pa + dir * margin, pb - dir * margin])


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
	if not _built:
		_hubs = sim.trade_hubs()
		if _hubs.is_empty():
			return  # hubs not placed yet (or resources off); retry next frame
		_built = true
		_build_routes()
		queue_redraw()  # paint the (static) route lines once
	if _routes.is_empty():
		return  # <2 hubs: no routes to draw
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
	# Convoy centre travels within [CONVOY_HALF, 1-CONVOY_HALF] so all carts stay on-route.
	# Ease the turnaround: a bare pingpong is a triangle wave, so the convoy
	# reverses at full speed at each end of the route. shuttle_ease keeps the
	# period but brings the carts to a stop before they turn.
	var tri := pingpong(_t / TRAVERSE_PERIOD, 1.0)
	var center := lerpf(CONVOY_HALF, 1.0 - CONVOY_HALF, FxMath.shuttle_ease(tri))
	for r in _routes:
		var pa: Vector2 = r["pa"]
		var pb: Vector2 = r["pb"]
		var cargo: PackedInt32Array = r["cargo"]
		for c in CARTS_PER_ROUTE:
			var f: float = center + (float(c) - CART_MID) * CART_GAP_FRAC
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
