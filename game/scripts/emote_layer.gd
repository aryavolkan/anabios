extends Node2D
# Emote pictogram layer: one MultiMesh floating tiny glyphs (emote_sprites.gd)
# above agents whose current action deserves a readable callout — Zzz over
# sleepers, a heart over courtship, a droplet over drinkers, an exclamation
# over fleers, a star over celebrants. The poses themselves carry the motion;
# the emote is the legibility layer that keeps those states readable when the
# camera is zoomed out and a figure is a handful of pixels.
#
# main.gd feeds one entry per emoting agent from its _refresh_bodies pass;
# this layer owns the per-id presence weight so emotes bloom in and fade out
# instead of strobing with the action debounce, and keeps fading entries for
# agents that stopped acting (or died) at their last seen spot.
#
# Presentation is zoom-aware: the whole layer fades out at census-wide zooms
# (the poses and body colours carry the story there) and a screen-pixel size
# floor keeps glyphs readable through the mid-zoom band.

const EmoteSprites = preload("res://scripts/emote_sprites.gd")
const EmoteShader = preload("res://shaders/emote.gdshader")

# Instance budget: emotes are a garnish, not a census. A 2000-agent night herd
# does not need 2000 Zzz — the first CAP emoting agents win.
const CAP := 512
# Presence weight per second. Deliberately asymmetric: the drives behind the
# emote-worthy actions flicker (a drinker's mood strobes with the walking
# debounce), and a fast-in / slow-out ramp keeps the glyph riding near full
# strength through the flicker instead of hovering half-faded.
const FADE_IN := 8.0
const FADE_OUT := 1.2
const RISE := 1.05  # emote height above the body centre, in body sizes
const EMOTE_SCALE := 0.62  # emote size relative to the body
# Size floor in world units (the emote analog of main.gd's BODY_MIN): a glyph
# over a hare-sized body would otherwise shrink into an unreadable speck.
const EMOTE_MIN := 2.0
# Zoom-aware presentation: below ZOOM_HIDE px-per-world-unit the layer fades
# out entirely (a census view doesn't want 300 glyphs of clutter), reaching
# full strength at ZOOM_FULL — the same close-up band as the dust puffs. A
# screen-pixel size floor keeps a glyph readable through the mid-zoom band.
const ZOOM_HIDE := 1.5
const ZOOM_FULL := 2.5
const EMOTE_MIN_PX := 7.0

# Action id -> glyph. Keys mirror main.gd's ACT_FLEE / ACT_SLEEP / ACT_DRINK /
# ACT_MATE / ACT_CELEBRATE (fight, trade, eat and scan stay emote-free: combat
# flashes, trade lanes and the poses already carry those).
const _KIND_FOR_ACT := {
	4: EmoteSprites.ALERT,
	5: EmoteSprites.SLEEP_Z,
	6: EmoteSprites.DROPLET,
	7: EmoteSprites.HEART,
	9: EmoteSprites.STAR,
}

var _mmi: MultiMeshInstance2D = null
# id -> [kind: int, weight: float, pos: Vector2, body_size: float]
var _entries: Dictionary = {}
# This frame's collect() buffer, consumed and cleared by refresh().
var _frame_ids: PackedInt32Array = PackedInt32Array()
var _frame_pts: PackedVector2Array = PackedVector2Array()
var _frame_kinds: PackedInt32Array = PackedInt32Array()
var _frame_sizes: PackedFloat32Array = PackedFloat32Array()


func setup() -> void:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_2D
	# No per-instance colours: the glyph atlas is self-coloured and the
	# emote shader cannot read COLOR on Metal (see emote.gdshader) — alpha
	# rides the custom-data channel instead.
	mm.use_custom_data = true
	var quad := QuadMesh.new()
	quad.size = Vector2(1, 1)
	mm.mesh = quad
	_mmi = MultiMeshInstance2D.new()
	_mmi.name = "Emotes"
	_mmi.multimesh = mm
	_mmi.texture = EmoteSprites.build_atlas()
	_mmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	var mat := ShaderMaterial.new()
	mat.shader = EmoteShader
	mat.set_shader_parameter("kind_scale", EmoteSprites.KIND_SCALE)
	_mmi.material = mat
	_mmi.z_index = 2
	add_child(_mmi)


func kind_for_act(act: float) -> int:
	return _KIND_FOR_ACT.get(int(act), EmoteSprites.NONE)


# Called from main.gd's body pass for every agent whose action resolved this
# frame; a no-op for emote-free actions so the call site stays one line.
func collect(id: int, pos: Vector2, body_size: float, act: float) -> void:
	var kind := kind_for_act(act)
	if kind == EmoteSprites.NONE:
		return
	_frame_ids.append(id)
	_frame_pts.append(pos)
	_frame_kinds.append(kind)
	_frame_sizes.append(body_size)


# One call per rendered frame, after the body pass has collect()ed this
# frame's emoting agents; `time` is main.gd's pause-aware animation clock.
func refresh(time: float, delta: float) -> void:
	if _mmi == null:
		return
	(_mmi.material as ShaderMaterial).set_shader_parameter("animation_time", time)
	# Capture-harness demo row: pin one of each glyph at full weight at a
	# fixed world spot, so an ANABIOS_SHOT run can review the emote art
	# in-scene without hunting for live sim states.
	if OS.has_environment("ANABIOS_EMOTE_DEMO"):
		var demo_at := Vector2(512, 512)
		if OS.has_environment("ANABIOS_EMOTE_DEMO_X"):
			demo_at.x = float(OS.get_environment("ANABIOS_EMOTE_DEMO_X"))
			demo_at.y = float(OS.get_environment("ANABIOS_EMOTE_DEMO_Y"))
		for k in range(1, EmoteSprites.KIND_COUNT):
			_frame_ids.append(-1000 - k)
			_frame_pts.append(demo_at + Vector2(4.0 * (k - 3), 0.0))
			_frame_kinds.append(k)
			_frame_sizes.append(2.0)
	var seen := {}
	for k in _frame_ids.size():
		var id := _frame_ids[k]
		seen[id] = true
		var e: Array = _entries.get(id, [_frame_kinds[k], 0.0, _frame_pts[k], _frame_sizes[k]])
		e[0] = _frame_kinds[k]
		e[1] = minf(e[1] + delta * FADE_IN, 1.0)
		e[2] = _frame_pts[k]
		e[3] = _frame_sizes[k]
		_entries[id] = e
	_frame_ids.clear()
	_frame_pts.clear()
	_frame_kinds.clear()
	_frame_sizes.clear()
	# Unseen ids (action ended, or the agent died) fade out where they were.
	for id in _entries.keys():
		if not seen.has(id):
			var e: Array = _entries[id]
			e[1] -= delta * FADE_OUT
			if e[1] <= 0.0:
				_entries.erase(id)
	# Capture-harness debug: histogram of live emotes + one anchor position,
	# so an agent-driven ANABIOS_SHOT run can verify and frame emotes.
	if OS.has_environment("ANABIOS_EMOTE_LOG") and not _entries.is_empty():
		var hist := {}
		var anchors := {}
		for id in _entries:
			hist[_entries[id][0]] = int(hist.get(_entries[id][0], 0)) + 1
			if not anchors.has(_entries[id][0]):
				anchors[_entries[id][0]] = _entries[id][2]
		print("[emotes] n=", _entries.size(), " kinds=", hist, " at=", anchors)
	var mm := _mmi.multimesh
	var n := mini(_entries.size(), CAP)
	if n > mm.instance_count:
		mm.instance_count = n
	# Zoom gate: px-per-world-unit from the canvas transform (the camera's
	# zoom). Entries above keep easing regardless, so zooming back in finds
	# the emotes mid-state instead of restarted. Outside the tree (headless
	# lifecycle test) there is no canvas: assume close zoom.
	var ppu := ZOOM_FULL
	if is_inside_tree():
		ppu = get_viewport().get_canvas_transform().x.x
	var zoom_alpha := smoothstep(ZOOM_HIDE, ZOOM_FULL, ppu)
	if zoom_alpha < 0.01:
		mm.visible_instance_count = 0
		return
	mm.visible_instance_count = n
	var j := 0
	for id in _entries:
		if j >= n:
			break
		var e: Array = _entries[id]
		var s: float = maxf(maxf(e[3] * EMOTE_SCALE, EMOTE_MIN), EMOTE_MIN_PX / ppu)
		var above: Vector2 = e[2] + Vector2(0.0, -maxf(e[3], s) * RISE)
		mm.set_instance_transform_2d(j, Transform2D(0.0, Vector2(s, s), 0.0, above))
		# Golden-ratio hash of the id: a stable per-agent phase so a cluster's
		# emotes drift out of step instead of pulsing in unison.
		var phase := fposmod(float(id) * 0.618034, 1.0)
		# Final alpha: presence weight plus each kind's animated fade (the
		# Zzz rise-and-vanish cycle, the star twinkle). Computed here and
		# shipped in the custom-data alpha channel because the shader cannot
		# carry a computed fade to its fragment on Metal (see emote.gdshader);
		# the cycle constants match the vertex stage's motion.
		var alpha: float = e[1] * zoom_alpha
		if e[0] == EmoteSprites.SLEEP_Z:
			alpha *= 1.0 - smoothstep(0.62, 1.0, fposmod(time * 0.30 + phase, 1.0))
		elif e[0] == EmoteSprites.STAR:
			alpha *= 0.75 + 0.25 * maxf(sin(time * 8.0 + phase * TAU), 0.0)
		mm.set_instance_custom_data(
			j, Color(float(e[0]) / EmoteSprites.KIND_SCALE, phase, e[1], alpha)
		)
		j += 1
