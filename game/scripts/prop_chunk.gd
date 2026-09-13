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
const FloraSprites = preload("res://scripts/flora_sprites.gd")
const SpriteSplit = preload("res://scripts/sprite_split.gd")

const CHUNK_CELLS := 64
const _APRON := CHUNK_CELLS + 2

# Copied from terrain_scatter.gd's `_DENSITY` (fraction of cells of each
# terrain that grow a prop, indexed by TerrainType id). MUST stay in sync
# with that copy by hand; terrain_scatter.gd is owned by another module.
const _DENSITY: PackedFloat32Array = [0.0, 0.14, 0.18, 0.03, 0.05, 0.10, 0.16, 0.14, 0.08]
# 16px prop art drawn at ~10 world units — same constant as terrain_scatter.gd.
const PROP_SCALE := 0.625

var _mmis: Array = []
# Canopy trees: one MultiMesh per FloraSprites kind, 32 px art at 0.5 world
# units per texel (two biome cells), planned from a hash offset so trees and
# the 16 px props never coincide, sorted by y so nearer trees overlap farther.
const CANOPY_SCALE := 0.5
const CANOPY_HASH_OFFSET := 977
var _canopy: Array = []


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
			kind = TerrainSprites.prop_variant_for(id, h)
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


# Pure canopy plan for chunk (cx, cy): per FloraSprites kind, world positions
# (no wrap offset) of the trees its inner cells grow, y-sorted. Density and
# kind per terrain come from FloraSprites; the hash is taken at the global
# cell shifted by CANOPY_HASH_OFFSET so a cell can carry both a bush and a
# tree without them sharing one jitter.
static func plan_canopy(cx: int, cy: int, ids66: PackedByteArray, res: int, world: float) -> Array:
	var buckets: Array = []
	for k in FloraSprites.KIND_COUNT:
		buckets.append([])
	var cell_w := world / float(res)
	for ly in CHUNK_CELLS:
		for lx in CHUNK_CELLS:
			var id := ids66[(ly + 1) * _APRON + (lx + 1)]
			var kind := FloraSprites.kind_for_terrain(id)
			if kind < 0:
				continue
			var gx := (cx * CHUNK_CELLS + lx) % res
			var gy := (cy * CHUNK_CELLS + ly) % res
			var h := _hash2(gx + CANOPY_HASH_OFFSET, gy)
			if h >= FloraSprites.DENSITY[id]:
				continue
			var jx := fposmod(h * 17.17, 1.0)
			var jy := fposmod(h * 5.55, 1.0)
			var pos := Vector2(gx + 0.1 + 0.8 * jx, gy + 0.1 + 0.8 * jy) * cell_w
			buckets[FloraSprites.variant_for(kind, h)].append(pos)
	var out: Array = []
	for k in FloraSprites.KIND_COUNT:
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
	if _canopy.is_empty():
		_make_canopy_mmis()
	var trees := plan_canopy(cx, cy, ids66, res, world)
	for k in FloraSprites.KIND_COUNT:
		var positions: PackedVector2Array = trees[k]
		var mm: MultiMesh = _canopy[k].multimesh
		mm.instance_count = positions.size()
		var i := 0
		for pos in positions:
			# Anchor the sprite's trunk foot (near the bottom of the 32 px
			# cell) on the planned cell, so the canopy rises above it.
			mm.set_instance_transform_2d(
				i,
				Transform2D(0.0, Vector2(CANOPY_SCALE, CANOPY_SCALE), 0.0, pos - Vector2(0.0, 5.0))
			)
			i += 1


# Hide every prop instance without discarding the multimeshes (the chunk left
# the resident ring; `build()` will refill them if it comes back).
func clear() -> void:
	for mmi in _mmis:
		var mm: MultiMesh = mmi.multimesh
		mm.instance_count = 0
	for mmi in _canopy:
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
		mmi.texture = SpriteSplit.for_quad(TerrainSprites.prop_image(k))
		mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		mmi.name = "Prop%s" % TerrainSprites.PROP_NAMES[k]
		if k == TerrainSprites.DIRT:
			# Bare earth lies under the figures, not over their feet.
			mmi.z_index = -3
		add_child(mmi)
		_mmis.append(mmi)


# Canopy trees are cut into a trunk layer below the figures and a crown
# layer above them (sprite_split.gd, D9): both halves share one MultiMesh,
# so the crown sits exactly over its trunk and a figure walking north of the
# tree disappears under the crown. The crown's z lifts it past the ground
# (-10), this layer (+1 +4) and the figures (0): -10 + 1 + 4 + 6 = 1.
const CROWN_Z := 6


func _make_canopy_mmis() -> void:
	for k in FloraSprites.KIND_COUNT:
		var mm := MultiMesh.new()
		mm.transform_format = MultiMesh.TRANSFORM_2D
		var quad := QuadMesh.new()
		quad.size = Vector2(FloraSprites.CELL_PX, FloraSprites.CELL_PX)
		mm.mesh = quad
		var img: Image = FloraSprites.kind_image(k)
		# Trees cut at their first trunk row; rocks (no trunk) at the
		# generic 60% line, so the crag's top still covers a figure behind it.
		var row: int = FloraSprites.trunk_row(k)
		if row >= FloraSprites.CELL_PX:
			row = SpriteSplit.split_row(img)
		var mmi := MultiMeshInstance2D.new()
		mmi.multimesh = mm
		mmi.texture = SpriteSplit.for_quad(SpriteSplit.lower(img, row))
		mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		mmi.name = "Canopy%s" % FloraSprites.NAMES[k]
		# Above the 16 px props so a tree overlaps the bush at its foot.
		mmi.z_index = 1
		add_child(mmi)
		_canopy.append(mmi)
		var crown := MultiMeshInstance2D.new()
		crown.multimesh = mm
		crown.texture = SpriteSplit.for_quad(SpriteSplit.upper(img, row))
		crown.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		crown.name = "Crown%s" % FloraSprites.NAMES[k]
		crown.z_index = CROWN_Z
		add_child(crown)
