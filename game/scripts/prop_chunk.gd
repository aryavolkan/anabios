extends Node2D
# Per-chunk decoration prop scatter (Phase 2 step 4, D3/D6 —
# docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md §6 Phase 2).
# One plain MultiMeshInstance2D per prop kind, filled from a single chunk's
# 66x66 terrain-id apron (`sim.biome_chunk_ids(cx, cy)`). The density table
# and deterministic cell hash are copied verbatim from `terrain_scatter.gd`
# (which this module may not edit) so a chunked world plants every prop at
# exactly the same world position the whole-world scatter would: the two
# paths are swappable and seamless across chunk borders.
#
# Owned and driven by the streaming ground layer: it calls `build()` when a
# chunk enters the resident ring (or its version bumps), `clear()` when the
# chunk leaves it, and `set_wrap_offset()` every frame the chunk's
# view-relative wrapped position changes (D6) without a rebuild.

const TerrainSprites = preload("res://scripts/terrain_sprites.gd")

const CHUNK_CELLS := 64
const _APRON := CHUNK_CELLS + 2

# Copied from terrain_scatter.gd's `_DENSITY` (fraction of cells of each
# terrain that grow a prop, indexed by TerrainType id). MUST stay in sync
# with that copy by hand; terrain_scatter.gd is owned by another module.
const _DENSITY: PackedFloat32Array = [0.0, 0.05, 0.14, 0.03, 0.05, 0.06, 0.16, 0.12, 0.04]
# 16px prop art drawn at ~10 world units — same constant as terrain_scatter.gd.
const PROP_SCALE := 0.625

var _mmis: Array = []


# Copied from terrain_scatter.gd's `_hash2`: same family as the tile-variant
# hash in terrain.gdshader, so scatter and tiles cannot drift per-platform.
# MUST stay identical to that copy by hand.
static func _hash2(x: int, y: int) -> float:
	return fposmod(sin(x * 127.1 + y * 311.7) * 43758.5453, 1.0)


# Pure per-chunk scatter plan: for each prop kind, world-space positions (no
# wrap offset) for chunk (cx, cy)'s inner 64x64 cells, sorted by y ascending
# (draw order for y-sorting against agents/structures later). `ids66` is the
# apron image from `sim.biome_chunk_ids(cx, cy)` (row-major, row 0 / col 0
# are the 1-cell torus apron, so the inner cell (lx, ly) sits at apron index
# (lx + 1, ly + 1)). The hash is taken at the GLOBAL wrapped cell coordinate
# `(cx*CHUNK_CELLS + lx) mod res, (cy*CHUNK_CELLS + ly) mod res` with the same
# formula and jitter as `terrain_scatter.plan()`, so a chunked world places
# every prop exactly where the whole-world scatter did.
static func plan(cx: int, cy: int, ids66: PackedByteArray, res: int, world: float) -> Array:
	var buckets: Array = []
	for k in TerrainSprites.PROP_COUNT:
		buckets.append([])
	var cell_w := world / float(res)
	for ly in CHUNK_CELLS:
		for lx in CHUNK_CELLS:
			var id := ids66[(ly + 1) * _APRON + (lx + 1)]
			var kind := TerrainSprites.prop_for_terrain(id)
			if kind < 0:
				continue
			var gx := (cx * CHUNK_CELLS + lx) % res
			var gy := (cy * CHUNK_CELLS + ly) % res
			var h := _hash2(gx, gy)
			if h >= _DENSITY[id]:
				continue
			var jx := fposmod(h * 13.37, 1.0)
			var jy := fposmod(h * 7.77, 1.0)
			var pos := Vector2(gx + 0.2 + 0.6 * jx, gy + 0.2 + 0.6 * jy) * cell_w
			buckets[kind].append(pos)
	var out: Array = []
	for k in TerrainSprites.PROP_COUNT:
		var positions: Array = buckets[k]
		positions.sort_custom(func(a, b): return a.y < b.y)
		out.append(PackedVector2Array(positions))
	return out


# (Re)fill every prop kind's multimesh for chunk (cx, cy) and place this node
# at `offset` (D6) so every instance transform, kept in unshifted world
# space, reads at its wrapped position. `ids66`, `res` and `world` are as for
# `plan()`.
func build(
	cx: int, cy: int, ids66: PackedByteArray, res: int, world: float, offset: Vector2
) -> void:
	if _mmis.is_empty():
		_make_mmis()
	position = offset
	var planned := plan(cx, cy, ids66, res, world)
	for k in TerrainSprites.PROP_COUNT:
		var positions: PackedVector2Array = planned[k]
		var mm: MultiMesh = _mmis[k].multimesh
		mm.instance_count = positions.size()
		var i := 0
		for pos in positions:
			mm.set_instance_transform_2d(
				i, Transform2D(0.0, Vector2(PROP_SCALE, PROP_SCALE), 0.0, pos)
			)
			i += 1


# Hide every prop instance without discarding the multimeshes (the chunk left
# the resident ring; `build()` will refill them if it comes back).
func clear() -> void:
	for mmi in _mmis:
		var mm: MultiMesh = mmi.multimesh
		mm.instance_count = 0


# Move the chunk to its new view-relative wrapped position (D6) without
# rebuilding the multimeshes.
func set_wrap_offset(offset: Vector2) -> void:
	position = offset


func _make_mmis() -> void:
	for k in TerrainSprites.PROP_COUNT:
		var mm := MultiMesh.new()
		mm.transform_format = MultiMesh.TRANSFORM_2D
		var quad := QuadMesh.new()
		quad.size = Vector2(TerrainSprites.CELL_PX, TerrainSprites.CELL_PX)
		mm.mesh = quad
		var mmi := MultiMeshInstance2D.new()
		mmi.multimesh = mm
		mmi.texture = ImageTexture.create_from_image(TerrainSprites.prop_image(k))
		mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		mmi.name = "Prop%s" % TerrainSprites.PROP_NAMES[k]
		add_child(mmi)
		_mmis.append(mmi)
