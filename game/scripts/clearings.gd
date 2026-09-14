extends RefCounted
# Village clearings: the ground a settlement has trodden bare of trees and
# scrub. The reference boards' villages sit in clearings — huts, fields and
# the square on packed earth with the forest standing back from the wall —
# while the prop scatter, planned from the terrain hash alone, grew oaks in
# the middle of a field and boulders on the crop rows.
#
# The settlement and hub layers publish the rectangles their footprints
# cover (world space, one per village or market square) through this static
# registry after each redraw; the prop planners filter their scatter through
# `filter()` at build time and the ground layers rebuild their props when
# `version` moves. Rectangles are snapped to half a village cell so the
# anchor easing that nudges a village a fraction of a unit each redraw does
# not force a rebuild of every resident chunk every twenty frames.

const SNAP := 8.0

static var rects: Array[Rect2] = []
static var version: int = 0
# Road strips: [from, to] segment pairs (world space) with ROAD_HALF of
# clear ground either side, published by the caravan layer; a strip is a
# poorer fit for a rectangle than a village is.
static var segments: Array[PackedVector2Array] = []
const ROAD_HALF := 5.0
static var _seg_by_source: Dictionary = {}
# Source name -> Array[Rect2]; each layer owns one entry and `rects` is
# their union, so the settlement and hub layers can publish independently.
static var _by_source: Dictionary = {}


# Snap a rectangle outwards to the SNAP grid so a footprint that drifts by
# less than a snap step publishes the same rectangle.
static func snap(r: Rect2) -> Rect2:
	var x0 := floorf(r.position.x / SNAP) * SNAP
	var y0 := floorf(r.position.y / SNAP) * SNAP
	var x1 := ceilf(r.end.x / SNAP) * SNAP
	var y1 := ceilf(r.end.y / SNAP) * SNAP
	return Rect2(x0, y0, x1 - x0, y1 - y0)


# Bounding rectangle of a set of structure positions grown by `margin` on
# every side (half a structure cell plus a little, so the last hut's yard is
# inside the clearing too). Empty input gives an empty rectangle.
static func bounds_of(positions: PackedVector2Array, margin: float) -> Rect2:
	if positions.is_empty():
		return Rect2()
	var r := Rect2(positions[0], Vector2.ZERO)
	for p in positions:
		r = r.expand(p)
	return snap(r.grow(margin))


# Replace one source's published set. Bumps `version` only when the
# snapped set actually changed (order-insensitive), so callers polling it
# stay idle while the villages merely breathe.
static func publish(source: String, new_rects: Array[Rect2]) -> void:
	var old: Array[Rect2] = _by_source.get(source, [] as Array[Rect2])
	if _same(new_rects, old):
		return
	_by_source[source] = new_rects.duplicate()
	var merged: Array[Rect2] = []
	for key in _by_source:
		merged.append_array(_by_source[key])
	rects = merged
	version += 1


static func _same(a: Array[Rect2], b: Array[Rect2]) -> bool:
	if a.size() != b.size():
		return false
	for r in a:
		if not b.has(r):
			return false
	return true


# Replace one source's road strips (see `publish`).
static func publish_segments(source: String, segs: Array[PackedVector2Array]) -> void:
	var old: Array[PackedVector2Array] = _seg_by_source.get(source, [] as Array[PackedVector2Array])
	if segs == old:
		return
	_seg_by_source[source] = segs.duplicate()
	var merged: Array[PackedVector2Array] = []
	for key in _seg_by_source:
		merged.append_array(_seg_by_source[key])
	segments = merged
	version += 1


static func contains(p: Vector2) -> bool:
	for r in rects:
		if r.has_point(p):
			return true
	for s in segments:
		if _segment_dist_sq(p, s[0], s[1]) < ROAD_HALF * ROAD_HALF:
			return true
	return false


static func _segment_dist_sq(p: Vector2, a: Vector2, b: Vector2) -> float:
	var ab := b - a
	var l2 := ab.length_squared()
	if l2 <= 0.0:
		return p.distance_squared_to(a)
	var f := clampf((p - a).dot(ab) / l2, 0.0, 1.0)
	return p.distance_squared_to(a + ab * f)


# Drop every position that falls inside a clearing. Returns the input
# unchanged (same object) when nothing is published, so the streaming
# planners pay nothing on a world without villages.
static func filter(positions: PackedVector2Array) -> PackedVector2Array:
	if rects.is_empty() and segments.is_empty():
		return positions
	var out := PackedVector2Array()
	for p in positions:
		if not contains(p):
			out.append(p)
	return out


static func reset() -> void:
	rects = []
	segments = []
	_by_source = {}
	_seg_by_source = {}
	version = 0
