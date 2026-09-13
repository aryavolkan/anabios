extends RefCounted
# Deterministic village layout generator (docs/superpowers/specs/
# 2026-09-12-pixel-world-at-scale-design.md §4 D1/D7, §6 Phase 4 step 1).
#
# Pure logic, no scene/node state: plan() maps (settlement id, anchor, member
# count, era, event flags, a water predicate) to a placement list on the
# local GRID (16 world units per cell — D1's structure-tile size). Nothing
# here reads or writes sim state; the same inputs always produce the same
# output, so replay stays bit-identical (D7, D8).
#
# The enum below is copied VERBATIM from structure_sprites.gd (the art
# module built in parallel against this exact contract) and the two files
# MUST be kept in sync — any change to one's kind order/names must be
# mirrored in the other.
enum {
	TENT,
	WINDBREAK,
	HEARTH,
	HUT,
	FENCE_H,
	FENCE_V,
	FIELD,
	GRANARY,
	HALL,
	PALISADE_H,
	PALISADE_V,
	PALISADE_CORNER,
	GATE,
	TOWER,
	MILL,
	FORGE,
	SCRIPTORIUM,
	BANNER,
	WELL,
	RUIN_BURNT,
	HUT_B,
	HUT_C,
}
# The three hut silhouettes an era-1 dwelling cell may take.
const HUT_KINDS: PackedInt32Array = [HUT, HUT_B, HUT_C]

# One structure occupies one grid cell of GRID world units (a 32 px sprite at
# 0.5 world units per texel, per D1).
const GRID := 16.0

# Recent-codex-event / held-invention flags, OR'd into `flags`.
const FLAG_TERRITORY := 1
const FLAG_WAR := 2
const FLAG_RAIDED := 4
const FLAG_FARMING := 8
const FLAG_MACHINERY := 16
const FLAG_METALWORKING := 32
const FLAG_WRITING := 64

# Sentinel cell for "no free cell found" searches.
const _NOT_FOUND := Vector2i(2147483647, 2147483647)

# Candidate-cell budget for the "first free spiral cell" searches (windbreak,
# well, forge, scriptorium): generous enough that a water-heavy site still
# finds land past a blocked half-plane in the unit tests.
const _SEARCH_CELLS := 400


# Deterministic per-(settlement, cell) hash in [0, 1) — same sin/const-scramble
# family as terrain_scatter._hash2 and the autotile shader hash, folding in
# `sid` so different settlements never share a layout by coincidence.
static func hash2(sid: int, gx: int, gy: int) -> float:
	return fposmod(
		sin(float(sid) * 12.9898 + float(gx) * 127.1 + float(gy) * 311.7) * 43758.5453, 1.0
	)


# First n cells of the square spiral around the origin (origin excluded),
# ring by ring, clockwise starting east: ring r (Chebyshev radius r) walks
# (r,0) -> (r,r) -> (-r,r) -> (-r,-r) -> (r,-r) -> (r,-1), 8*r cells per ring.
static func spiral(n: int) -> Array[Vector2i]:
	var out: Array[Vector2i] = []
	if n <= 0:
		return out
	var r := 1
	while out.size() < n:
		for i in range(0, r + 1):
			out.append(Vector2i(r, i))
			if out.size() >= n:
				return out
		for i in range(r - 1, -r - 1, -1):
			out.append(Vector2i(i, r))
			if out.size() >= n:
				return out
		for i in range(r - 1, -r - 1, -1):
			out.append(Vector2i(-r, i))
			if out.size() >= n:
				return out
		for i in range(-r + 1, r + 1):
			out.append(Vector2i(i, -r))
			if out.size() >= n:
				return out
		for i in range(-r + 1, 0):
			out.append(Vector2i(r, i))
			if out.size() >= n:
				return out
		r += 1
	return out


# Bounding rect (in grid cells) of an Array of Vector2i.
static func bounds(cells: Array) -> Rect2i:
	if cells.is_empty():
		return Rect2i()
	var first: Vector2i = cells[0]
	var min_x := first.x
	var min_y := first.y
	var max_x := first.x
	var max_y := first.y
	for c in cells:
		var v: Vector2i = c
		min_x = mini(min_x, v.x)
		min_y = mini(min_y, v.y)
		max_x = maxi(max_x, v.x)
		max_y = maxi(max_y, v.y)
	return Rect2i(min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)


static func _cell_pos(anchor: Vector2, cell: Vector2i) -> Vector2:
	return anchor + Vector2(cell.x, cell.y) * GRID


static func _is_water(is_water: Callable, anchor: Vector2, cell: Vector2i) -> bool:
	return bool(is_water.call(_cell_pos(anchor, cell)))


static func _flip_bit(sid: int, cell: Vector2i) -> bool:
	return hash2(sid, cell.x, cell.y) >= 0.5


# Places `kind` at `cell` iff the cell is free and not water; records the cell
# in `occupied` and appends {"kind","pos","flip","cell"} to `out`. Returns
# whether the placement happened, so callers can chain "first free" searches.
static func _place(
	cell: Vector2i,
	kind: int,
	flip: bool,
	occupied: Dictionary,
	anchor: Vector2,
	is_water: Callable,
	out: Array
) -> bool:
	if occupied.has(cell):
		return false
	if _is_water(is_water, anchor, cell):
		return false
	occupied[cell] = true
	out.append({"kind": kind, "pos": _cell_pos(anchor, cell), "flip": flip, "cell": cell})
	return true


# Places `kind` at the first free, non-water cell in `candidates` (in order).
# Returns whether a cell was found and placed.
static func _place_first_free(
	candidates: Array[Vector2i],
	kind: int,
	occupied: Dictionary,
	anchor: Vector2,
	is_water: Callable,
	out: Array,
	west_only: bool
) -> bool:
	for c in candidates:
		if west_only and c.x >= 0:
			continue
		if _place(c, kind, false, occupied, anchor, is_water, out):
			return true
	return false


static func _is_land_water_adjacent(cell: Vector2i, anchor: Vector2, is_water: Callable) -> bool:
	var neighbours: Array[Vector2i] = [
		cell + Vector2i(1, 0), cell + Vector2i(-1, 0), cell + Vector2i(0, 1), cell + Vector2i(0, -1)
	]
	for n in neighbours:
		if _is_water(is_water, anchor, n):
			return true
	return false


# Nearest land cell within `radius` grid cells of the anchor that has a water
# cell among its 4 neighbours (a river/lake bank a mill can sit on). Returns
# _NOT_FOUND when no such cell exists in range.
static func _nearest_water_adjacent_cell(
	anchor: Vector2, radius: int, occupied: Dictionary, is_water: Callable
) -> Vector2i:
	var best := _NOT_FOUND
	var best_dist := 2147483647
	for dy in range(-radius, radius + 1):
		for dx in range(-radius, radius + 1):
			var c := Vector2i(dx, dy)
			if occupied.has(c):
				continue
			if _is_water(is_water, anchor, c):
				continue
			if not _is_land_water_adjacent(c, anchor, is_water):
				continue
			var d := c.x * c.x + c.y * c.y
			if d < best_dist:
				best_dist = d
				best = c
	return best


# Pure village layout plan. Returns an Array of {"kind": int, "pos": Vector2,
# "flip": bool} in draw order (y ascending, ties by x). Deterministic from
# its arguments alone: no RNG object, no time, only `hash2(sid, ...)`.
static func plan(
	sid: int, anchor: Vector2, members: int, era: int, flags: int, is_water: Callable
) -> Array:
	var occupied: Dictionary = {}
	var out: Array = []
	var spiral_cells: Array[Vector2i] = spiral(_SEARCH_CELLS)
	# Dwellings sit on every other cell of the spiral in both axes, so each
	# hut or tent keeps a hut-width of open ground around it (paths, yards,
	# the space the reference villages breathe through) instead of the
	# spiral packing them into one solid heap; single structures (well,
	# forge, scriptorium) still take the first free cell and fill the gaps.
	var lattice_cells: Array[Vector2i] = []
	for c in spiral_cells:
		if c.x % 2 == 0 and c.y % 2 == 0:
			lattice_cells.append(c)

	# --- centrepiece: hearth (camp/thatch) or hall (timber/stone) ---
	var center_kind := HEARTH if era < 2 else HALL
	_place(Vector2i.ZERO, center_kind, false, occupied, anchor, is_water, out)

	if era <= 0:
		# --- Era 0 (camp): tents on the spiral, windbreak to windward ---
		var count := clampi(1 + members / 8, 1, 6)
		var i := 0
		var placed := 0
		while placed < count and i < lattice_cells.size():
			var c: Vector2i = lattice_cells[i]
			i += 1
			if _place(c, TENT, _flip_bit(sid, c), occupied, anchor, is_water, out):
				placed += 1
		if members >= 16:
			_place_first_free(spiral_cells, WINDBREAK, occupied, anchor, is_water, out, true)
	else:
		# --- Era >= 1 (thatch): huts on the spiral, well, fields, granary ---
		var count := clampi(2 + members / 6, 2, 14)
		var huts: Array[Vector2i] = []
		var i := 0
		var placed := 0
		while placed < count and i < lattice_cells.size():
			var c: Vector2i = lattice_cells[i]
			i += 1
			var hk: int = HUT_KINDS[
				int(hash2(sid * 3 + 1, c.x, c.y) * HUT_KINDS.size()) % HUT_KINDS.size()
			]
			if _place(c, hk, _flip_bit(sid, c), occupied, anchor, is_water, out):
				huts.append(c)
				placed += 1
		if members >= 24:
			_place_first_free(spiral_cells, WELL, occupied, anchor, is_water, out, false)

		var farming := (flags & FLAG_FARMING) != 0
		if farming or members >= 24:
			var fields := clampi(members / 12, 2, 6)
			var max_gx := 0
			for h in huts:
				max_gx = maxi(max_gx, h.x)
			var fx0 := max_gx + 2
			var fx1 := fx0 + 1
			var rows := int(ceil(float(fields) / 2.0))
			var fy0 := 0
			var fy1 := rows - 1
			var placed_fields := 0
			for row in rows:
				for col in range(2):
					if placed_fields >= fields:
						break
					_place(Vector2i(fx0 + col, row), FIELD, false, occupied, anchor, is_water, out)
					placed_fields += 1
				if placed_fields >= fields:
					break
			# Fence ring one cell out from the field block; top/bottom edges
			# run corner to corner (so the corners come out FENCE_H, as
			# specified), left/right edges fill only the interior rows.
			for gx in range(fx0 - 1, fx1 + 2):
				_place(Vector2i(gx, fy0 - 1), FENCE_H, false, occupied, anchor, is_water, out)
				_place(Vector2i(gx, fy1 + 1), FENCE_H, false, occupied, anchor, is_water, out)
			for gy in range(fy0, fy1 + 1):
				_place(Vector2i(fx0 - 1, gy), FENCE_V, false, occupied, anchor, is_water, out)
				_place(Vector2i(fx1 + 1, gy), FENCE_V, false, occupied, anchor, is_water, out)
			if farming:
				var granary_candidates: Array[Vector2i] = []
				for extra_x in range(1, 4):
					for gy2 in range(fy0, fy1 + 1):
						granary_candidates.append(Vector2i(fx1 + 1 + extra_x, gy2))
				_place_first_free(
					granary_candidates, GRANARY, occupied, anchor, is_water, out, false
				)

		if era >= 2:
			# --- Era >= 2 (timber/stone): forge, scriptorium, mill ---
			if (flags & FLAG_METALWORKING) != 0:
				_place_first_free(spiral_cells, FORGE, occupied, anchor, is_water, out, false)
			if (flags & FLAG_WRITING) != 0:
				_place_first_free(spiral_cells, SCRIPTORIUM, occupied, anchor, is_water, out, false)
			if (flags & FLAG_MACHINERY) != 0:
				var mill_cell := _nearest_water_adjacent_cell(anchor, 6, occupied, is_water)
				if mill_cell != _NOT_FOUND:
					_place(mill_cell, MILL, false, occupied, anchor, is_water, out)

	# --- palisade ring: any era >= 1 site under Territory or War ---
	if era >= 1 and (flags & (FLAG_TERRITORY | FLAG_WAR)) != 0:
		var max_extent := 0
		for p in out:
			var c: Vector2i = p["cell"]
			max_extent = maxi(max_extent, maxi(absi(c.x), absi(c.y)))
		var r := max_extent + 1
		for gx in range(-r + 1, r):
			if gx == 0:
				_place(Vector2i(0, r), GATE, false, occupied, anchor, is_water, out)
			else:
				_place(Vector2i(gx, r), PALISADE_H, false, occupied, anchor, is_water, out)
			_place(Vector2i(gx, -r), PALISADE_H, false, occupied, anchor, is_water, out)
		for gy in range(-r + 1, r):
			_place(Vector2i(-r, gy), PALISADE_V, false, occupied, anchor, is_water, out)
			_place(Vector2i(r, gy), PALISADE_V, false, occupied, anchor, is_water, out)
		var corners: Array[Vector2i] = [
			Vector2i(-r, -r), Vector2i(r, -r), Vector2i(-r, r), Vector2i(r, r)
		]
		for corner in corners:
			_place(corner, PALISADE_CORNER, false, occupied, anchor, is_water, out)
		_place(Vector2i(0, r - 1), TOWER, false, occupied, anchor, is_water, out)
		if (flags & FLAG_TERRITORY) != 0:
			_place(Vector2i(-1, r + 1), BANNER, false, occupied, anchor, is_water, out)
			_place(Vector2i(1, r + 1), BANNER, false, occupied, anchor, is_water, out)

	# --- raid aftermath: up to 2 huts/tents burn ---
	if (flags & FLAG_RAIDED) != 0:
		var burnable: Array = []
		for idx in out.size():
			var kind: int = out[idx]["kind"]
			if kind == TENT or HUT_KINDS.has(kind):
				var c: Vector2i = out[idx]["cell"]
				burnable.append([hash2(sid * 7 + 3, c.x, c.y), idx])
		burnable.sort_custom(func(a, b): return a[0] < b[0])
		for i in mini(2, burnable.size()):
			var idx: int = burnable[i][1]
			out[idx]["kind"] = RUIN_BURNT

	out.sort_custom(
		func(a, b):
			var pa: Vector2 = a["pos"]
			var pb: Vector2 = b["pos"]
			if pa.y != pb.y:
				return pa.y < pb.y
			return pa.x < pb.x
	)
	var result: Array = []
	for p in out:
		result.append({"kind": p["kind"], "pos": p["pos"], "flip": p["flip"]})
	return result
