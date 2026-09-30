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


# The extra torus copies a view needs beyond the one `visible_chunks` placed
# per chunk index (D6). Once the view is wider or taller than a world minus
# a chunk, two copies of one chunk can both be on screen — the fit framing
# of a 1024 world at 1x, 0.25x on a 4096 world, even a camera near a chunk
# edge at 2x on a two-chunk world — and a chunk drawn once left the other
# copy to the whole-world sprite: no props or canopy, linear filtering, a
# hard seam. For every entry of `placed` (as `visible_chunks` returns), each
# further copy whose rect meets the view itself (no ring: a copy shares its
# chunk's upload, so nothing needs preloading) is returned as
# `[cx, cy, offset_x, offset_y]`, nearest the camera first and at most
# `max_copies` of them (a far zoom-out on a small world would otherwise ask
# for hundreds; past the cap the whole-world sprite keeps the far copies).
static func wrap_copies(
	cam_pos: Vector2,
	view_size: Vector2,
	world: float,
	chunk_world: float,
	placed: Array,
	max_copies: int
) -> Array:
	var out: Array = []
	if world <= 0.0 or chunk_world <= 0.0 or max_copies <= 0:
		return out
	if view_size.x <= world - chunk_world and view_size.y <= world - chunk_world:
		return out
	var v0 := cam_pos - view_size * 0.5
	var v1 := cam_pos + view_size * 0.5
	var found: Array = []  # [dist², cx, cy, off_x, off_y]
	for entry in placed:
		var ox: float = entry[2]
		var oy: float = entry[3]
		var base := Vector2(int(entry[0]) * chunk_world + ox, int(entry[1]) * chunk_world + oy)
		# Copies k worlds over whose rect overlaps the view on each axis.
		var kx0 := int(floorf((v0.x - chunk_world - base.x) / world)) + 1
		var kx1 := int(ceilf((v1.x - base.x) / world)) - 1
		var ky0 := int(floorf((v0.y - chunk_world - base.y) / world)) + 1
		var ky1 := int(ceilf((v1.y - base.y) / world)) - 1
		for ky in range(ky0, ky1 + 1):
			for kx in range(kx0, kx1 + 1):
				if kx == 0 and ky == 0:
					continue
				var off := Vector2(ox + kx * world, oy + ky * world)
				var center := (
					base + Vector2(kx * world, ky * world) + Vector2.ONE * chunk_world * 0.5
				)
				found.append(
					[cam_pos.distance_squared_to(center), entry[0], entry[1], off.x, off.y]
				)
	found.sort_custom(func(a, b): return a[0] < b[0])
	for i in mini(max_copies, found.size()):
		out.append([found[i][1], found[i][2], found[i][3], found[i][4]])
	return out


# How many of `view_chunks` (the ring-0 `visible_chunks` of the view itself)
# are not resident yet. The ground layer holds freshly built props back
# while this is above zero, so a fill after a jump ([F], a minimap click, a
# zoom-out) shows its props all at once instead of chunk by chunk.
static func missing_count(resident: Dictionary, view_chunks: Array) -> int:
	var n := 0
	for entry in view_chunks:
		if not resident.has(Vector2i(int(entry[0]), int(entry[1]))):
			n += 1
	return n


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
