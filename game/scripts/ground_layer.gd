extends Node2D
# Streamed ground (D3/D6, `docs/superpowers/specs/
# 2026-09-12-pixel-world-at-scale-design.md` §4/§6 Phase 2): keeps the
# GroundChunk sprites intersecting the camera view (+1 ring) resident,
# uploading at most UPLOAD_BUDGET new/changed chunks a frame, oldest first.
# All the "which chunks, which uploads" logic lives in the pure, unit-tested
# ground_streaming.gd; this node is the thin per-frame driver: read the
# camera, call into that logic, apply the result to actual GroundChunk nodes
# and bridge calls.
#
# Created by biome_renderer.gd as a child of the Biome sprite (like
# TerrainScatter/BiomeProps), counter-scaled back to world units the same
# way. Visible only over the biome view, with both [B] (tiles) and [N]
# (streaming) on; the whole-world Biome sprite stays underneath as the
# far-zoom fallback and the [B]/[N] A/B reference.

const GroundStreaming = preload("res://scripts/ground_streaming.gd")
const GroundChunk = preload("res://scripts/ground_chunk.gd")

const UPLOAD_BUDGET := 8
const RING := 1
const CHUNK_CELLS := GroundStreaming.CHUNK_CELLS

var _biome  # biome_renderer.gd — untyped to avoid a preload cycle (biome_renderer preloads this script)
var _sim
var _cam: Camera2D

# Vector2i(cx, cy) -> GroundChunk node.
var _chunks: Dictionary = {}
# Vector2i(cx, cy) -> {"version": int, "age": int}; mirrors _chunks' keys.
var _resident: Dictionary = {}


func _ready() -> void:
	# Just above the parent Biome sprite's whole-world ground (z -10); the
	# scatter/prop children sit further above (TerrainScatter is 4, i.e.
	# actual -6) and keep drawing over the chunks like they do the
	# whole-world sprite.
	z_index = 1


# One-time wiring from biome_renderer._ready, before the scenario's
# resolution is known (that arrives in _process below, same as every other
# biome_renderer child).
func setup(biome_renderer, sim) -> void:
	_biome = biome_renderer
	_sim = sim
	_cam = get_node_or_null("../../Camera2D")


func _process(_delta: float) -> void:
	if _biome == null or _sim == null or _cam == null:
		return
	var show := _biome.is_biome_view() and _biome.tiles_enabled() and _biome.streaming_enabled()
	visible = show
	if not show:
		return
	var world: float = _sim.world_size()
	var res: int = int(_sim.biome_resolution())
	var chunk_count: int = int(_sim.biome_chunk_count())
	if world <= 0.0 or res <= 0 or chunk_count <= 0:
		return
	var cell_w: float = world / float(res)
	var chunk_world: float = cell_w * CHUNK_CELLS

	var vp: Vector2 = get_viewport().get_visible_rect().size
	var view_size := Vector2(vp.x / _cam.zoom.x, vp.y / _cam.zoom.y)
	var wanted := GroundStreaming.visible_chunks(
		_cam.position, view_size, world, chunk_world, chunk_count, RING
	)

	for key in _resident.keys():
		_resident[key]["age"] += 1
	var plan := GroundStreaming.plan_uploads(
		_resident, wanted, Callable(_sim, "biome_chunk_version"), UPLOAD_BUDGET
	)

	for key in plan["evict"]:
		if _chunks.has(key):
			_chunks[key].queue_free()
			_chunks.erase(key)
		_resident.erase(key)

	var perf := get_node_or_null("../../UI/PerfReadout")
	for pair in plan["upload"]:
		var cx: int = pair[0]
		var cy: int = pair[1]
		var key := Vector2i(cx, cy)
		var chunk = _chunks.get(key)
		if chunk == null:
			chunk = GroundChunk.new()
			chunk.setup(_biome.terrain_material(), cx, cy)
			add_child(chunk)
			_chunks[key] = chunk
		var bytes: PackedByteArray = _sim.biome_chunk_bytes(cx, cy)
		var ids: PackedByteArray = _sim.biome_chunk_ids(cx, cy)
		if chunk.upload(bytes, ids):
			_resident[key] = {"version": int(_sim.biome_chunk_version(cx, cy)), "age": 0}
			if perf != null:
				perf.note_chunk_upload()

	# Position every wanted, resident chunk at this frame's wrapped offset —
	# the camera can move (and a torus-wrapped copy switch) without the
	# chunk's content changing.
	for entry in wanted:
		var key := Vector2i(int(entry[0]), int(entry[1]))
		var chunk = _chunks.get(key)
		if chunk != null:
			var pos := Vector2(entry[0] * chunk_world + entry[2], entry[1] * chunk_world + entry[3])
			chunk.place(pos, cell_w)

	if perf != null:
		perf.set_resident_chunks(_resident.size())
