extends Node2D
# Decoration prop scatter over the pixel-art ground: one plain (shader-free)
# MultiMeshInstance2D per prop kind — the Metal-safe pattern the settlement
# and hub layers use — filled from a deterministic cell-hash plan so the same
# world always grows the same forest.
#
# Created by biome_renderer as a child of the Biome sprite. The parent sprite
# is scaled world/res (biome-pixel units), so _ready counter-scales this node
# back to world units for the instance transforms. Rebuilds only when the
# terrain-id grid actually changes; biome_renderer drives the cadence.

const TerrainSprites = preload("res://scripts/terrain_sprites.gd")
const SpriteSplit = preload("res://scripts/sprite_split.gd")

# Fraction of cells of each terrain that grow a prop (indexed by TerrainType
# id). Water is bare; forests read denser than steppe and tundra.
const _DENSITY: PackedFloat32Array = [0.0, 0.05, 0.14, 0.03, 0.05, 0.06, 0.16, 0.12, 0.04]
# Hard cap on planned props (pre torus wrap) so continental res-512 worlds
# stay bounded; lowest-hash cells win deterministically.
const PROP_BUDGET := 12000
# 16px prop art drawn at ~10 world units (a body is ~7, a biome cell 8).
const PROP_SCALE := 0.625

var _mmis: Array = []
var _last_ids := PackedByteArray()


func _ready() -> void:
	# Sit between the ground (parent, z -10) and footstep tracks (-1).
	z_index = 4
	var parent := get_parent() as Node2D
	var ps: Vector2 = parent.scale if parent != null else Vector2.ONE
	if ps.x != 0.0 and ps.y != 0.0:
		scale = Vector2(1.0 / ps.x, 1.0 / ps.y)


# Deterministic per-cell hash in [0, 1) — same family as the tile-variant
# hash in terrain.gdshader, so scatter and tiles cannot drift per-platform.
static func _hash2(x: int, y: int) -> float:
	return fposmod(sin(x * 127.1 + y * 311.7) * 43758.5453, 1.0)


# Pure scatter plan: for each prop kind, world-space positions. Cells whose
# hash clears the terrain's density threshold grow that terrain's suggested
# prop, jittered inside the cell; over budget, lowest hashes win.
static func plan(ids: PackedByteArray, res: int, world: float, budget: int) -> Array:
	var cand: Array = []
	var cell_w := world / float(res)
	for y in res:
		for x in res:
			var id := ids[y * res + x]
			var kind := TerrainSprites.prop_for_terrain(id)
			if kind < 0:
				continue
			var h := _hash2(x, y)
			if h >= _DENSITY[id]:
				continue
			var jx := fposmod(h * 13.37, 1.0)
			var jy := fposmod(h * 7.77, 1.0)
			var pos := Vector2(x + 0.2 + 0.6 * jx, y + 0.2 + 0.6 * jy) * cell_w
			cand.append([h, kind, pos])
	if cand.size() > budget:
		cand.sort_custom(func(a, b): return a[0] < b[0])
		cand.resize(budget)
	var buckets: Array = []
	for k in TerrainSprites.PROP_COUNT:
		buckets.append([])
	for c in cand:
		buckets[c[1]].append(c[2])
	var out: Array = []
	for k in TerrainSprites.PROP_COUNT:
		out.append(PackedVector2Array(buckets[k]))
	return out


# Refill the per-kind multimeshes when the terrain-id grid changes. Each
# position is drawn 9 times (torus 3x3) like every other world layer.
func rebuild(ids: PackedByteArray, res: int, world: float) -> void:
	if ids == _last_ids or ids.is_empty():
		return
	_last_ids = ids.duplicate()
	if _mmis.is_empty():
		_make_mmis()
	var planned := plan(ids, res, world, PROP_BUDGET)
	for k in TerrainSprites.PROP_COUNT:
		var positions: PackedVector2Array = planned[k]
		var mm: MultiMesh = _mmis[k].multimesh
		mm.instance_count = positions.size() * 9
		var i := 0
		for gy in [-1, 0, 1]:
			for gx in [-1, 0, 1]:
				var off := Vector2(gx * world, gy * world)
				for pos in positions:
					mm.set_instance_transform_2d(
						i, Transform2D(0.0, Vector2(PROP_SCALE, PROP_SCALE), 0.0, pos + off)
					)
					i += 1


func _make_mmis() -> void:
	for k in TerrainSprites.PROP_COUNT:
		var mm := MultiMesh.new()
		mm.transform_format = MultiMesh.TRANSFORM_2D
		var quad := QuadMesh.new()
		quad.size = Vector2(TerrainSprites.CELL_PX, TerrainSprites.CELL_PX)
		mm.mesh = quad
		var mmi := MultiMeshInstance2D.new()
		mmi.multimesh = mm
		mmi.texture = SpriteSplit.for_quad(TerrainSprites.prop_image(k))
		mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		mmi.name = "Prop%s" % TerrainSprites.PROP_NAMES[k]
		add_child(mmi)
		_mmis.append(mmi)
