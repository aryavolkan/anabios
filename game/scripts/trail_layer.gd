extends Node2D

# Trail pools split out of main.gd to keep that scene controller under the
# gdlint file-length budget (the viewer_effects.gd pattern): footstep tracks
# and the combat-streak / trade-route segment trails. main.gd creates one of
# these as a child in _ready (at the tree position the Tracks MMI used to
# occupy), calls setup() once with the scene's authored Streaks/TradeRoutes
# multimeshes — keeping their z order and additive material — then drives it
# per frame via update(). The layer owns the Tracks MultiMeshInstance2D it
# builds; tracks_mmi() exposes it so the torus wrap clones can share it.

const FxMath = preload("res://scripts/fx_math.gd")

# Footsteps: each walker sampled this frame drops a small fading track mark,
# so migration paths and foraging loops read as trampled trails at close zoom.
const TRACK_TTL: float = 1.4
const TRACK_CAP: int = 256

# Segment trails: world-space links kept on screen for a few ticks as fading
# tracers. Combat streaks (attacker→target) are wide, bright, and brief so
# ranged (Spines) volleys read as volleys; trade routes (trader→partner) are
# thin, dim, and long-lived so recurring swaps along species borders
# accumulate into visible lanes. Both tint to the initiator's genome hue.
# Streaks/flashes draw above everything (they are events in the air); the trade
# lanes draw at ground level, under bodies and huts (z=-2 in the scene) — over
# a busy market they used to pile up into bright coloured scribbles across the
# village roofs instead of reading as paths worn between settlements.
# Seconds, not frames: as integer per-frame counts a streak lived twice as long
# in wall-clock at 30 fps. Values match the old 8 and 24 frames at 60 fps.
const STREAK_TTL: float = 0.133
const TRADE_TTL: float = 0.4
# At the pixel-art zooms (CLOSE_ZOOM and in) the trade lanes stop being
# genome-hued tracers and become the earth they are worn into: the same
# dark trodden brown as the footstep tracks, thin and faint, no pulse. At
# 4x a busy hub's hundreds of pale hue-lines used to fuse into a lavender
# haze with cracks across the whole market square.
const CLOSE_ZOOM: float = 2.0
const TRADE_EARTH := Color(0.22, 0.18, 0.13, 1.0)
const TRADE_EARTH_ALPHA: float = 0.28
const STREAK_CLOSE_WIDTH: float = 0.5
const STREAK_CLOSE_WHITE: float = 0.7  # how far the species hue bleaches toward white

var _tracks_mmi: MultiMeshInstance2D = null
var _tracks: Array = []  # entries: [pos: Vector2, ttl: float]
var _streaks_mm: MultiMesh = null
var _trade_mm: MultiMesh = null
var _streak_trail: Array = []  # entries: [from: Vector2, to: Vector2, expiry: float, color]
var _trade_trail: Array = []  # entries: [from: Vector2, to: Vector2, expiry: float, color]


# Wire the authored segment-trail multimeshes and build the tracks MMI.
# Called once from main._ready where the Tracks MMI used to be built inline.
func setup(
	streaks_mm: MultiMesh, trade_mm: MultiMesh, track_mesh: Mesh, track_texture: Texture2D
) -> void:
	_streaks_mm = streaks_mm
	_trade_mm = trade_mm
	# Footstep tracks: walkers leave a short-lived dotted trail behind them.
	var tmm := MultiMesh.new()
	tmm.transform_format = MultiMesh.TRANSFORM_2D
	tmm.use_colors = true
	tmm.mesh = track_mesh
	_tracks_mmi = MultiMeshInstance2D.new()
	_tracks_mmi.name = "Tracks"
	_tracks_mmi.multimesh = tmm
	_tracks_mmi.texture = track_texture
	_tracks_mmi.z_index = -1
	add_child(_tracks_mmi)


# The tracks layer, for main._make_wrap_clones to duplicate at the torus seams.
func tracks_mmi() -> MultiMeshInstance2D:
	return _tracks_mmi


# Streak colours for this zoom: the species hues at far zoom, bleached most
# of the way to white from CLOSE_ZOOM in.
static func streak_palette(cols: PackedColorArray, zoom: float) -> PackedColorArray:
	if zoom < CLOSE_ZOOM:
		return cols
	var out := PackedColorArray()
	out.resize(cols.size())
	for i in cols.size():
		var c: Color = cols[i]
		out[i] = Color(c.lerp(Color.WHITE, STREAK_CLOSE_WHITE), c.a)
	return out


# Lane colours for this zoom: the genome hues as given at far zoom, one
# trodden-earth brown per lane from CLOSE_ZOOM in.
static func trade_palette(trade_cols: PackedColorArray, zoom: float) -> PackedColorArray:
	if zoom < CLOSE_ZOOM:
		return trade_cols
	var earth := PackedColorArray()
	earth.resize(trade_cols.size())
	earth.fill(TRADE_EARTH)
	return earth


# Per-frame tick, driven from main._process. The walker sample and pause flag
# live in main (written during the body pass), so they are passed in rather
# than read back; the segment endpoints/colors are this tick's sim fetch, which
# main also feeds to the fight/trade hotspot pass.
func update(
	delta: float,
	moving_sample: PackedVector2Array,
	is_paused: bool,
	streak_segs: PackedVector2Array,
	streak_cols: PackedColorArray,
	trade_segs: PackedVector2Array,
	trade_cols: PackedColorArray,
	world: float,
	zoom: float = 1.0
) -> void:
	var close := zoom >= CLOSE_ZOOM
	# Up close a combat streak is a thin, near-white shaft (the boards'
	# arrows in flight), not a species-hued bar four pixels wide.
	_update_segment_trail(
		_streak_trail,
		_streaks_mm,
		streak_segs,
		streak_palette(streak_cols, zoom),
		STREAK_TTL,
		STREAK_CLOSE_WIDTH if close else 1.0,
		0.85,
		world
	)
	_update_segment_trail(
		_trade_trail,
		_trade_mm,
		trade_segs,
		trade_palette(trade_cols, zoom),
		TRADE_TTL,
		0.5,
		TRADE_EARTH_ALPHA if close else 0.6,
		world,
		not close
	)
	_update_tracks(delta, moving_sample, is_paused)


func _update_tracks(delta: float, moving_sample: PackedVector2Array, is_paused: bool) -> void:
	if _tracks_mmi == null:
		return
	if not is_paused:
		for pos in moving_sample:
			if _tracks.size() >= TRACK_CAP:
				_tracks.pop_front()
			_tracks.append([pos, TRACK_TTL])
	var write := 0
	for t in _tracks:
		t[1] -= delta
		if t[1] > 0.0:
			_tracks[write] = t
			write += 1
	_tracks.resize(write)
	var mm: MultiMesh = _tracks_mmi.multimesh
	var m := _tracks.size()
	if m > mm.instance_count:
		mm.instance_count = m
	mm.visible_instance_count = m
	for i in m:
		var a: float = 0.20 * float(_tracks[i][1]) / TRACK_TTL
		mm.set_instance_transform_2d(i, Transform2D(0.0, Vector2(1.6, 1.6), 0.0, _tracks[i][0]))
		mm.set_instance_color(i, Color(0.22, 0.18, 0.13, a))


# Append this tick's segments, age the trail, then draw each survivor as a
# tinted quad. Segments use the shortest-path torus delta: a hop across the
# seam is really a short step the other way, and the wrap clones continue it.
func _update_segment_trail(
	trail: Array,
	mm: MultiMesh,
	segs: PackedVector2Array,
	cols: PackedColorArray,
	ttl: float,
	width: float,
	max_alpha: float,
	world: float,
	flow: bool = false
) -> void:
	# Absolute expiry, so ageing is wall-clock with no delta threaded through.
	var now: float = Time.get_ticks_msec() / 1000.0
	for i in int(segs.size() / 2.0):
		trail.append([segs[2 * i], segs[2 * i + 1], now + ttl, cols[i]])
	# Perf: cap the trail at the multimesh budget, dropping the oldest first.
	while trail.size() > mm.instance_count:
		trail.pop_front()
	# Compact out the expired.
	var write := 0
	for read_i in trail.size():
		var s: Array = trail[read_i]
		if s[2] > now:
			trail[write] = s
			write += 1
	trail.resize(write)
	var m: int = mini(trail.size(), mm.instance_count)
	mm.visible_instance_count = m
	for i in m:
		var from: Vector2 = trail[i][0]
		var d: Vector2 = trail[i][1] - from
		if d.x > world * 0.5:
			d.x -= world
		elif d.x < -world * 0.5:
			d.x += world
		if d.y > world * 0.5:
			d.y -= world
		elif d.y < -world * 0.5:
			d.y += world
		var seg_len: float = maxf(d.length(), 0.001)
		var mid: Vector2 = from + d * 0.5
		mm.set_instance_transform_2d(i, Transform2D(d.angle(), Vector2(seg_len, width), 0.0, mid))
		var c: Color = trail[i][3]
		c.a = max_alpha * clampf((float(trail[i][2]) - now) / ttl, 0.0, 1.0)
		if flow:
			# Directional pulses: project the midpoint onto the segment's own
			# axis so the bright spots march from `from` toward `to`. Collinear
			# neighbours of one route stay phase-continuous; bends and torus
			# seams introduce a small phase jump (invisible in practice).
			c.a *= FxMath.flow_pulse(mid.dot(d / seg_len), now)
		mm.set_instance_color(i, c)
