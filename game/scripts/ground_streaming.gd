extends RefCounted
# Pure logic behind the streamed ground (D3/D6, `docs/superpowers/specs/
# 2026-09-12-pixel-world-at-scale-design.md` §4/§6 Phase 2): which chunks the
# camera can see, and which resident chunk textures to evict/upload/keep this
# frame. No nodes, no bridge calls — ground_layer.gd drives both functions
# with plain values so this stays unit-testable headless (test_ground_
# streaming.gd) and free of scene-tree timing.
#
# Chunk edge length in cells — matches CHUNK_CELLS in
# crates/anabios-godot/src/lib.rs (the two must agree for the chunk grid to
# line up; a change to one belongs with a change to the other).
const CHUNK_CELLS := 64

# Sentinel age for a chunk that isn't resident yet (missing): it always sorts
# ahead of any resident chunk whose version merely changed, since an absent
# texture is more urgent than a stale one.
const _MISSING_AGE := 0x7fffffff


# Every chunk index (torus-wrapped into `0 ..< chunk_count` on each axis)
# whose world rect — placed at the wrapped copy nearest `cam_pos` (D6) —
# intersects the view rect (`cam_pos` centred, size `view_size`) grown by
# `ring` chunks on every side. Each chunk index appears at most once: the
# copy nearest the camera is chosen independently per axis before the
# intersection test, so a view wider than the world still yields each index
# exactly once (every chunk's nearest copy lies within half a world of the
# camera, and the grown view then covers the whole world on that axis).
# Returns `[[cx, cy, offset_x, offset_y], ...]`; `offset_*` is the wrap
# offset (a multiple of `world`, may be negative) to add to the chunk's
# unwrapped position `(cx * chunk_world, cy * chunk_world)`.
static func visible_chunks(
	cam_pos: Vector2,
	view_size: Vector2,
	world: float,
	chunk_world: float,
	chunk_count: int,
	ring: int
) -> Array:
	var out: Array = []
	if world <= 0.0 or chunk_world <= 0.0 or chunk_count <= 0:
		return out
	var margin := float(ring) * chunk_world
	var view_rect := Rect2(
		cam_pos - view_size * 0.5 - Vector2(margin, margin),
		view_size + Vector2(margin, margin) * 2.0
	)
	for cy in range(chunk_count):
		for cx in range(chunk_count):
			var base := Vector2(cx * chunk_world, cy * chunk_world)
			var center := base + Vector2(chunk_world, chunk_world) * 0.5
			var off_x := roundf((cam_pos.x - center.x) / world) * world
			var off_y := roundf((cam_pos.y - center.y) / world) * world
			var rect := Rect2(base + Vector2(off_x, off_y), Vector2(chunk_world, chunk_world))
			if rect.intersects(view_rect):
				out.append([cx, cy, off_x, off_y])
	return out


# Diff `wanted` (an array of `[cx, cy, ...]` entries, as `visible_chunks`
# returns — extra trailing elements are ignored) against `resident`, a
# `Dictionary` of `Vector2i(cx, cy) -> {"version": int, "age": int}` for the
# chunks currently uploaded. Returns
# `{"evict": [Vector2i, ...], "upload": [[cx, cy], ...], "keep": [Vector2i, ...]}`:
#   - `evict`: resident chunks no longer wanted.
#   - `upload`: wanted chunks that are missing from `resident` or whose
#     `versions.call(cx, cy)` differs from the resident version, oldest
#     (largest `age`) first, capped at `budget` entries. A missing chunk is
#     always older than any resident one.
#   - `keep`: wanted, resident chunks whose version matches, plus any
#     version-changed chunk that didn't fit under `budget` (it stays resident
#     with its current texture until a later frame's budget picks it up).
static func plan_uploads(
	resident: Dictionary, wanted: Array, versions: Callable, budget: int
) -> Dictionary:
	var wanted_keys := {}
	for entry in wanted:
		wanted_keys[Vector2i(int(entry[0]), int(entry[1]))] = true

	var evict: Array = []
	for key in resident.keys():
		if not wanted_keys.has(key):
			evict.append(key)

	var candidates: Array = []  # [Vector2i key, int age]
	var keep: Array = []
	for key in wanted_keys.keys():
		if resident.has(key):
			var info: Dictionary = resident[key]
			var ver: int = int(versions.call(key.x, key.y))
			if ver != int(info.get("version", -1)):
				candidates.append([key, int(info.get("age", 0))])
			else:
				keep.append(key)
		else:
			candidates.append([key, _MISSING_AGE])

	candidates.sort_custom(func(a, b): return a[1] > b[1])  # oldest (largest age) first

	var upload: Array = []
	var budget_n: int = maxi(0, budget)
	for i in range(mini(budget_n, candidates.size())):
		var key: Vector2i = candidates[i][0]
		upload.append([key.x, key.y])
	for i in range(budget_n, candidates.size()):
		var key: Vector2i = candidates[i][0]
		if resident.has(key):
			keep.append(key)

	return {"evict": evict, "upload": upload, "keep": keep}
