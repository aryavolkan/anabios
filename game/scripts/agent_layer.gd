extends Node2D
# Field-agent body pass, split out of main.gd to keep that scene controller
# under the gdlint file-length budget (the viewer_effects.gd / trail_layer.gd
# pattern) and — the Phase 3 step 1 change
# (docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md, §6
# Phase 3) — to cull the per-frame GDScript work to the agents actually on
# screen. main.gd creates the per-bucket MultiMeshInstance2D nodes (scene
# structure shared with the torus wrap clones) and the Camera2D, calls
# setup() once with them, then drives this layer per frame via refresh().
# This layer owns the death-ghost MultiMeshes it builds; death_mmis() exposes
# them so the wrap clones can share them, the way trail_layer.tracks_mmi()
# already does.
#
# Visible-set culling (§6 Phase 3, item 1): refresh() computes the camera's
# world rect grown by CULL_MARGIN and asks the bridge for the alive-array
# indices inside it (sim.alive_in_rect, torus-aware). Smoothing, gait,
# facing, action, colour and the MultiMesh writes only run for those
# indices — an off-screen agent is neither smoothed, animated nor uploaded.
# Below 1x zoom (cam.is_overview()) no bodies are drawn at all. The id-merge
# birth/death bookkeeping still runs over the FULL alive array every frame
# (cheap index arithmetic, no per-id Dictionary lookups) so population
# tracking, death ghosts elsewhere on the map, and an agent's smoothed
# position all stay correct world-wide; only an on-screen death spawns a
# ghost (an off-screen death is presentation nobody would see).

const MammalSprites = preload("res://scripts/mammal_sprites.gd")
const Palette = preload("res://scripts/palette.gd")
const FxMath = preload("res://scripts/fx_math.gd")

# Bodies are 0.5–3.0 world units across (genome size). Scale them up generously
# with a floor so the hominin silhouette (head, limbs) reads as a figure at the
# default cluster-framed zoom — not just when zoomed all the way in.
const BODY_SCALE: float = 7.0
const BODY_MIN: float = 6.0
# World units of margin added on every side of the camera's world rect before
# querying alive_in_rect(): keeps an agent walking toward the edge of the
# screen from popping in/out of the visible set the frame it crosses the
# viewport border, and gives death ghosts near the edge somewhere to fall.
const CULL_MARGIN: float = 64.0
# Action poses in the field atlases (two frames each after the four gait
# poses), picked per agent from the sim signals in _refresh_bodies. Encoded
# into instance custom data as act / ACT_SCALE — custom data clamps at 1.0,
# so the scale leaves headroom for poses past 4.
const ACT_EAT := 1.0
const ACT_FIGHT := 2.0
const ACT_TRADE := 3.0
const ACT_FLEE := 4.0
const ACT_SLEEP := 5.0
const ACT_DRINK := 6.0
const ACT_MATE := 7.0
const ACT_SCAN := 8.0
const ACT_CELEBRATE := 9.0
const ACT_SPEAR := 10.0
const ACT_BOW := 11.0
const ACT_SCALE := 13.0
# Mood discriminants from the sim's mood.rs (alive_moods) that drive poses.
# All-CONTENT when the scenario's affect layer is off.
const MOOD_CONTENT := 0
const MOOD_SEEK_FOOD := 1
const MOOD_SEEK_WATER := 2
const MOOD_SLEEP := 3
const MOOD_FLEE := 4
const MOOD_FIGHT := 5
const MOOD_SEEK_MATE := 6
const MOOD_MATE := 7
# Same band as the sim's interact::FIRE_THRESHOLD: fire_intent crosses 0.5 on
# the strike, so the build-up above it is the visible hunt.
const FIRE_POSE_THRESHOLD := 0.5
const DEATH_TTL: float = 1.4
# Seconds for a death ghost to topple from tilted to flat (ease-out-back, so
# it rolls a hair past flat and settles — a body hitting the ground).
const DEATH_FALL: float = 0.35
const DEATH_CAP: int = 512
const BIRTH_POP: float = 0.3

# Smooth-motion state. Agents teleport once per tick; rendering eases each
# body toward its latest tick position so movement glides. Identity is by
# agent id (alive indices reshuffle as agents die): both id arrays ascend, so
# a two-pointer merge finds each agent's last smoothed position in O(n).
# Only a seam crossing snaps: on a wrapped world a real displacement is at
# most half the map per axis. SMOOTH is the approach at 60 fps, scaled by the
# frame delta AND ticks_per_frame — one frame of 64x covers 64 ticks, so it
# needs almost no easing and time-lapse stays crisp instead of mushy.
const SMOOTH: float = 0.35

# Dependencies, wired once by setup().
var sim = null
var _body_mmis: Array[MultiMeshInstance2D] = []
var _overlay = null
var _cam: Camera2D = null
var _effects: Node2D = null
var _emote_layer: Node2D = null

# perf_readout.gd is created after this layer during Main._ready (it needs no
# earlier hook), so it is looked up lazily — see perf_readout.gd's header —
# rather than injected: by the time refresh() first runs (the first
# _process, strictly after _ready finishes) it always exists.
var _perf_readout: Node = null
var _perf_lookup_done: bool = false

# One MultiMesh per species drawing the fallen pose, fed by ids that vanish
# from the alive list. They sit above the carcass discs (z -4, set by the
# caller) but below living bodies.
var _death_mmis: Array[MultiMeshInstance2D] = []
var _death_effects: Array = []

var _prev_ids: PackedInt32Array = PackedInt32Array()
var _prev_smooth: PackedVector2Array = PackedVector2Array()
var _prev_sizes: PackedFloat32Array = PackedFloat32Array()
var _prev_bucket: PackedInt32Array = PackedInt32Array()
# Last frame's per-agent body colour (coat hue for quads, white for the
# self-coloured hominin atlases), kept in sync with the other _prev_* arrays so
# a death ghost can inherit its agent's colour instead of a flat grey.
var _prev_color: PackedColorArray = PackedColorArray()
var _birth_times: Dictionary = {}
# Per-id facing: (committed side 0 right / 1 left, eased value, low-passed
# heading x). Turns ease through a horizontal squash, not a snap. See FxMath.
var _facing: Dictionary = {}
# Per-id gait cycle position (0..1 = one contact -> passing -> contact ->
# passing loop), advanced by the distance a body actually covers on screen so
# the cadence tracks real speed instead of a fixed frame rate. Frozen while an
# agent stands still, where it doubles as that agent's stable idle-bob offset.
var _gait: Dictionary = {}
# Per-id locomotion state (hold credit, blended walk weight) — see FxMath.
var _locomotion: Dictionary = {}
# Per-id action state (current pose, remaining hold time), debounced so a
# threshold crossing cannot interrupt a two-frame action at an arbitrary beat.
var _actions: Dictionary = {}
# Distance each body moved on screen this frame, parallel to the alive arrays;
# feeds the gait accumulator above. Filled in the smoothing pass.
var _step_dist: PackedFloat32Array = PackedFloat32Array()
# Alive-array index (this frame) -> previous frame's alive-array index, or -1
# for a newly seen id. Parallel to the current alive arrays.
var _match_prev: PackedInt32Array = PackedInt32Array()

# First few visible walkers this frame, sampled for the close-zoom dust puffs
# and footstep tracks; main.gd reads this via moving_sample() for
# trail_layer.update()/viewer_effects.update().
var _moving_sample: PackedVector2Array = PackedVector2Array()
# Last frame's per-agent energy, parallel to _prev_ids at the time it was
# captured — read by the idle-with-rising-energy eat-pose check.
var _prev_energy: PackedFloat32Array = PackedFloat32Array()


# body_mmis: one MultiMeshInstance2D per render bucket (MammalSprites.BUCKET_COUNT),
# created and textured by main.gd — scene structure shared with the torus wrap
# clones. overlay: $OverlayManager (reads .body_mode / BODY_* constants).
func setup(
	sim_ref,
	body_mmis: Array[MultiMeshInstance2D],
	overlay,
	cam: Camera2D,
	effects: Node2D,
	emote_layer: Node2D
) -> void:
	sim = sim_ref
	_body_mmis = body_mmis
	_overlay = overlay
	_cam = cam
	_effects = effects
	_emote_layer = emote_layer
	for b in MammalSprites.BUCKET_COUNT:
		var dmm := MultiMesh.new()
		dmm.transform_format = MultiMesh.TRANSFORM_2D
		dmm.use_colors = true
		dmm.mesh = _body_mmis[0].multimesh.mesh
		var dmi := MultiMeshInstance2D.new()
		dmi.name = "Deaths%d" % b
		dmi.multimesh = dmm
		dmi.texture = MammalSprites.bucket_fallen(b)
		dmi.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		dmi.z_index = -4
		add_child(dmi)
		_death_mmis.append(dmi)


# Torus wrap clone sources (see main.gd::_make_wrap_clones): the death-ghost
# meshes, in bucket order.
func death_mmis() -> Array[MultiMeshInstance2D]:
	return _death_mmis


# First few visible walkers sampled this frame, for trail_layer/viewer_effects.
func moving_sample() -> PackedVector2Array:
	return _moving_sample


# Pure camera-rect math (no node/autoload references, so the headless test
# can preload this script and call it directly): the camera's world rect —
# viewport size / zoom, centred on cam_pos — grown by margin on every side.
static func view_rect(cam_pos: Vector2, zoom: float, viewport: Vector2, margin: float) -> Rect2:
	var z: float = maxf(zoom, 0.0001)
	var half: Vector2 = viewport / (2.0 * z) + Vector2(margin, margin)
	return Rect2(cam_pos - half, half * 2.0)


# Torus-aware "is pos inside [x0,x1) x [y0,y1)": pos is shifted by whole
# worlds so it lands in [x0, x0+world) x [y0, y0+world) first, mirroring D6
# (each agent is placed at its wrapped position nearest the camera) — so a
# death near a seam is judged against the copy actually inside the rect.
static func pos_in_rect(
	pos: Vector2, x0: float, y0: float, x1: float, y1: float, world: float
) -> bool:
	var w: float = maxf(world, 1.0)
	var px: float = x0 + fposmod(pos.x - x0, w)
	var py: float = y0 + fposmod(pos.y - y0, w)
	return px < x1 and py < y1


# Two-pointer merge of the ascending `ids` array against last frame's
# ascending `prev_ids`: for each current id, the previous value at the
# matching prev_ids entry, or `fallback[i]` when the id is new (or the merge
# is being exercised standalone, as in test_agent_layer.gd). Pure and
# side-effect-free — refresh() layers the death callback, the wrap-aware
# distance check and the visibility-gated ease on top of this.
static func merge_prev(
	prev_ids: PackedInt32Array,
	prev_vals: PackedVector2Array,
	ids: PackedInt32Array,
	fallback: PackedVector2Array
) -> PackedVector2Array:
	var n: int = ids.size()
	var out := PackedVector2Array()
	out.resize(n)
	var p := 0
	var pn: int = prev_ids.size()
	for i in n:
		var id: int = ids[i]
		while p < pn and prev_ids[p] < id:
			p += 1
		if p < pn and prev_ids[p] == id:
			out[i] = prev_vals[p]
			p += 1
		else:
			out[i] = fallback[i]
	return out


# Called from Main._process where _refresh_bodies used to run. animation_time
# and ticks_per_frame come from main.gd (the shared animation clock and the
# user-selected sim speed the smoothing rate scales with) — main.gd already
# computes both for its own shader-parameter loop, so refresh() reuses them
# instead of keeping a second copy in sync.
func refresh(
	delta: float,
	animation_time: float,
	ticks_per_frame: int,
	fight_pts: PackedVector2Array,
	trade_pts: PackedVector2Array
) -> void:
	var n: int = int(sim.alive_count())
	if n == 0:
		for mmi in _body_mmis:
			mmi.multimesh.visible_instance_count = 0
		_prev_ids = PackedInt32Array()
		_prev_smooth = PackedVector2Array()
		_prev_sizes = PackedFloat32Array()
		_prev_bucket = PackedInt32Array()
		_prev_energy = PackedFloat32Array()
		_prev_color = PackedColorArray()
		_actions.clear()
		_report_visible(0)
		_emote_layer.refresh(animation_time, delta)
		_refresh_death_effects(delta)
		return

	var is_overview: bool = _cam != null and _cam.is_overview()
	var world: float = sim.world_size()
	var vis: PackedInt32Array = PackedInt32Array()
	var x0 := 0.0
	var y0 := 0.0
	var x1 := 0.0
	var y1 := 0.0
	if not is_overview:
		if _cam != null:
			var rect: Rect2 = view_rect(
				_cam.position, _cam.zoom.x, get_viewport_rect().size, CULL_MARGIN
			)
			x0 = rect.position.x
			y0 = rect.position.y
			x1 = x0 + rect.size.x
			y1 = y0 + rect.size.y
			vis = sim.alive_in_rect(x0, y0, x1, y1)
		else:
			# No camera wired (e.g. a harness without one): fall back to
			# drawing everything rather than silently culling to nothing.
			vis.resize(n)
			for i in n:
				vis[i] = i
			x0 = -INF
			y0 = -INF
			x1 = INF
			y1 = INF
	_report_visible(vis.size())

	var positions: PackedVector2Array = sim.alive_positions()
	var ids: PackedInt32Array = sim.alive_ids()
	var sizes: PackedFloat32Array = sim.alive_sizes()
	var rots: PackedFloat32Array = sim.alive_rotations()
	var sp_ids: PackedInt32Array = sim.alive_species_ids()
	var energies: PackedFloat32Array = sim.alive_energy()
	# Pose-driving intent channels: fire_intent is written for every agent
	# every tick in any world (the hunt signal); the mood column is the
	# affect layer's behavior label (fight/flee/sleep) and stays all-CONTENT
	# in flag-off worlds.
	var fire_intents: PackedFloat32Array = sim.alive_fire_intent()
	var moods: PackedInt32Array = sim.alive_moods() if sim.affect_active() else PackedInt32Array()
	# Held-invention bits (all-zero in flag-off worlds) arm the fight pose.
	var inv_masks: PackedInt32Array = sim.alive_invention_masks()
	var body_colors: PackedColorArray = _body_colors(n)
	var have_rots: bool = rots.size() == n
	var have_sp: bool = sp_ids.size() == n
	var have_ids: bool = ids.size() == n
	var have_en: bool = energies.size() == n
	var have_fire: bool = fire_intents.size() == n
	var have_moods: bool = moods.size() == n

	# Visibility mask, built once by walking `vis` (ascending) in lockstep
	# with the alive-array index — reused by both passes below instead of a
	# per-index search.
	var visible_mask := PackedByteArray()
	visible_mask.resize(n)
	var vp := 0
	var vn: int = vis.size()
	for i in n:
		if vp < vn and vis[vp] == i:
			visible_mask[i] = 1
			vp += 1

	var now: float = Time.get_ticks_msec() / 1000.0
	var half_world: float = maxf(world * 0.5, 1.0)
	var tick_rate: float = maxf(float(ticks_per_frame), 1.0)
	var k: float = 1.0 - pow(1.0 - SMOOTH, delta * 60.0 * tick_rate)
	var smooth: PackedVector2Array = positions
	_match_prev = PackedInt32Array()
	_step_dist.resize(n)
	_step_dist.fill(0.0)
	_moving_sample = PackedVector2Array()

	if have_ids:
		# Pass 1: match current ids against last frame's (birth/death
		# bookkeeping), world-wide — cheap index arithmetic, no per-id
		# Dictionary lookups, so this always runs over the full alive array
		# regardless of visibility.
		_match_prev.resize(n)
		_match_prev.fill(-1)
		var p := 0
		var pn: int = _prev_ids.size()
		for i in n:
			var id: int = ids[i]
			while p < pn and _prev_ids[p] < id:
				_kill(_prev_ids[p], p, not is_overview, x0, y0, x1, y1, world)
				p += 1
			if p < pn and _prev_ids[p] == id:
				_match_prev[i] = p
				p += 1
		while p < pn:
			_kill(_prev_ids[p], p, not is_overview, x0, y0, x1, y1, world)
			p += 1

		# Pass 2: smoothed render positions. `from_arr` is last frame's
		# smoothed position per current id (or this frame's own position for
		# a new id) via the tested two-pointer merge; only a visible, matched
		# agent actually eases toward its target — an invisible agent (on- or
		# off-screen last frame) just tracks its exact sim position, so it
		# never carries stale lerp lag when it scrolls into view.
		var from_arr: PackedVector2Array = merge_prev(_prev_ids, _prev_smooth, ids, positions)
		smooth = PackedVector2Array()
		smooth.resize(n)
		for i in n:
			var target: Vector2 = positions[i]
			if visible_mask[i] and _match_prev[i] >= 0:
				var from: Vector2 = from_arr[i]
				var d: Vector2 = target - from
				if absf(d.x) < half_world and absf(d.y) < half_world:
					smooth[i] = from.lerp(target, k)
					# Measure the *rendered* step, not the sim step: the
					# feet have to keep pace with the glide the viewer
					# actually draws, which lags the tick position.
					_step_dist[i] = from.distance_to(smooth[i])
				else:
					smooth[i] = target
			else:
				smooth[i] = target
				if _match_prev[i] < 0 and not _birth_times.has(ids[i]):
					_birth_times[ids[i]] = now
					if visible_mask[i] and _effects != null:
						_effects.spawn_dust(target)
		_prev_ids = ids
		_prev_smooth = smooth
		_prev_sizes = sizes
		_prev_energy = energies
		_prev_color = body_colors

	# Zoom-compensated floor: as the camera pulls out, the minimum body size
	# grows so hominin figures stay legible at the world overview instead of
	# dissolving into dots.
	var zoom_boost := 1.0
	if _cam != null:
		zoom_boost = clampf(1.2 / _cam.zoom.x, 1.0, 3.0)
	var min_body := BODY_MIN * zoom_boost

	# Bucket alive indices by render bucket — one MultiMesh per bucket. Only
	# visible indices are grouped into `buckets` (the MultiMesh writes below
	# are the O(visible) pass); `bucket_ix` still gets a value for every
	# index (visible: computed fresh; invisible: copied through from last
	# frame's bucket at the matched previous index, or computed fresh for a
	# new invisible agent) because it is what death ghosts look up later —
	# see the FULL-array comment on _prev_bucket above.
	var diet: PackedFloat32Array = sim.alive_diet()
	var live: PackedInt32Array = _livestock_flags(n)
	# Module-keyed body tags (armour/spines/storage/locomotor bits — see
	# MammalSprites.TAG_*); alive_body_tags() is all-zero in worlds without
	# those modules, so this falls through to the plain diet/size table there.
	var body_tags: PackedInt32Array = sim.alive_body_tags()
	var have_tags: bool = body_tags.size() == n
	var buckets: Array = []
	for b in MammalSprites.BUCKET_COUNT:
		buckets.append(PackedInt32Array())
	var bucket_ix := PackedInt32Array()
	bucket_ix.resize(n)
	for i in n:
		if visible_mask[i]:
			var tags: int = body_tags[i] if have_tags else 0
			var arch := (
				MammalSprites.archetype_for(diet[i], sizes[i], live[i] != 0, tags)
				if have_sp
				else MammalSprites.PRIMATE
			)
			var b := MammalSprites.bucket_of(arch, sp_ids[i]) if have_sp else 0
			bucket_ix[i] = b
			buckets[b].append(i)
		else:
			var pm: int = _match_prev[i] if i < _match_prev.size() else -1
			if pm >= 0 and pm < _prev_bucket.size():
				bucket_ix[i] = _prev_bucket[pm]
			else:
				var tags2: int = body_tags[i] if have_tags else 0
				var arch2 := (
					MammalSprites.archetype_for(diet[i], sizes[i], live[i] != 0, tags2)
					if have_sp
					else MammalSprites.PRIMATE
				)
				bucket_ix[i] = MammalSprites.bucket_of(arch2, sp_ids[i]) if have_sp else 0
	if have_ids:
		_prev_bucket = bucket_ix

	for b in MammalSprites.BUCKET_COUNT:
		var mm: MultiMesh = _body_mmis[b].multimesh
		var idx: PackedInt32Array = buckets[b]
		var m: int = idx.size()
		var gait_fps: float = MammalSprites.bucket_gait_fps(b)
		if m > mm.instance_count:
			mm.instance_count = m
		mm.visible_instance_count = m
		for j in m:
			var i: int = idx[j]
			var sz: float = maxf(sizes[i] * BODY_SCALE, min_body)
			# New agents squash in, then spring to full size with an overshoot
			# (anticipation-then-pop) instead of blinking into existence;
			# after BIRTH_POP seconds the scale is exactly 1.
			if have_ids:
				var age: float = now - float(_birth_times.get(ids[i], now - 1.0))
				if age < BIRTH_POP:
					sz *= FxMath.birth_scale(age / BIRTH_POP)
			# Upright: the hominin stands, not spins — heading drives the
			# walk shader (walk weight + facing), not the transform rotation.
			var t: Transform2D = Transform2D(0.0, Vector2(sz, sz), 0.0, smooth[i])
			mm.set_instance_transform_2d(j, t)
			mm.set_instance_color(j, body_colors[i])
			# Per-instance animation state for the field_agent shader. The sim
			# reports heading exactly 0.0 when velocity ≈ 0, which doubles as
			# the idle flag; facing is the heading's x-sign.
			var rot: float = rots[i] if have_rots else 0.0
			# `moving` is the blended 0..1 walk weight the shader mixes its
			# secondary motion with; `walking` is the debounced state picking
			# the pose and driving the gait. Splitting them stops the sprite
			# popping on the sim's flickering heading (~3.5 times a second).
			var walking: bool = rot != 0.0
			var moving: float = 1.0 if walking else 0.0
			if have_ids:
				var loco: Vector2 = FxMath.step_locomotion(
					_locomotion.get(ids[i], Vector2.ZERO), walking, delta
				)
				_locomotion[ids[i]] = loco
				walking = loco.x > 0.0
				moving = loco.y
			if walking and _moving_sample.size() < 8:
				_moving_sample.append(smooth[i])
			# Ease the facing mirror per id: the shader's fractional mix turns
			# the transition into a quick flip-squash rather than a snap. The
			# side is deadbanded and held while stopped (see FxMath), so a
			# flickering heading cannot strobe the sprite.
			var cx: float = cos(rot)
			var face_left := 1.0 if cx < 0.0 else 0.0
			if have_ids:
				var face: Vector3 = FxMath.step_facing(
					_facing.get(ids[i], Vector3(face_left, face_left, cx)), cx, walking, delta
				)
				_facing[ids[i]] = face
				face_left = face.y
			# Gait cycle position, paced by the distance this body covered on
			# screen so the feet keep up with the ground (see FxMath).
			var phase: float
			var footfall := false
			if have_ids:
				var gid: int = ids[i]
				var previous_phase: float = float(_gait.get(gid, FxMath.seed_gait(gid)))
				phase = previous_phase
				if walking:
					var stride: float = FxMath.stride_len(sizes[i], gait_fps)
					phase = FxMath.advance_gait(phase, _step_dist[i], stride)
					# The four-pose cycle has two contact beats. Fire dust when
					# crossing either half-cycle boundary, so the puff lands under
					# a planted foot instead of appearing at a random frame.
					footfall = int(floor(previous_phase * 2.0)) != int(floor(phase * 2.0))
				_gait[gid] = phase
			else:
				phase = fposmod(positions[i].x * 0.11 + positions[i].y * 0.07, 1.0)
			if footfall and _effects != null:
				_effects.spawn_dust(smooth[i])
			# Action pose from sim signals. Priority: sleep (SLEEP mood while
			# standing) > flee (FLEE mood — the mood is the behavior arbiter,
			# so fear outranks even a high fire_intent) > hunt/fight (real
			# fire_intent in any world, the FIGHT mood, or standing at a
			# strike hotspot) > courtship bow > water-seeking sip
			# > foraging scan > trade hotspot > idle-with-rising-energy eat.
			# A pursuing predator fires while moving, so the hunt reads as a
			# moving lunge instead of the old hotspot-only strike instant.
			var act := 0.0
			var mood: int = moods[i] if have_moods else MOOD_CONTENT
			if mood == MOOD_SLEEP and not walking:
				act = ACT_SLEEP
			elif mood == MOOD_FLEE:
				act = ACT_FLEE
			elif (have_fire and fire_intents[i] > FIRE_POSE_THRESHOLD) or mood == MOOD_FIGHT:
				act = ACT_FIGHT
			elif (mood == MOOD_MATE or mood == MOOD_SEEK_MATE) and not walking:
				act = ACT_CELEBRATE if mood == MOOD_MATE else ACT_MATE
			elif mood == MOOD_SEEK_WATER and not walking:
				act = ACT_DRINK
			elif mood == MOOD_SEEK_FOOD and not walking:
				act = ACT_SCAN
			else:
				for fp in fight_pts:
					if smooth[i].distance_squared_to(fp) < 36.0:
						act = ACT_FLEE if walking else ACT_FIGHT
						break
			if act == 0.0:
				for tp in trade_pts:
					if smooth[i].distance_squared_to(tp) < 36.0:
						act = ACT_TRADE
						break
			if act == 0.0 and not walking and have_en:
				var pi: int = _match_prev[i] if i < _match_prev.size() else -1
				if pi >= 0 and pi < _prev_energy.size() and energies[i] > _prev_energy[pi] + 0.02:
					act = ACT_EAT
			if i < inv_masks.size():
				act = FxMath.weapon_action(act, inv_masks[i])
			if have_ids:
				var action_state := FxMath.step_action(
					_actions.get(ids[i], Vector2(-1.0, 0.0)), act, delta
				)
				_actions[ids[i]] = action_state
				act = action_state.x
				# Emote-worthy actions get a pictogram above the agent's head.
				_emote_layer.collect(ids[i], smooth[i], sz, act)
				if act == ACT_DRINK and _effects != null:
					_effects.tick_sip(ids[i], smooth[i], sz)
			mm.set_instance_custom_data(j, Color(phase, moving, face_left, act / ACT_SCALE))

	_emote_layer.refresh(animation_time, delta)
	_refresh_death_effects(delta)


# An id present last frame but gone now died (or left the alive list). Record a
# fallen-figure ghost at its last smoothed position, in its species and size,
# toppling away from its last facing, and forget its animation state so a
# recycled id would pop in fresh. `maybe_ghost` gates the actual ghost spawn on
# whether the agent's last known position is inside the current camera rect
# (and overview isn't active) — an off-screen death is presentation nobody
# would see.
func _kill(
	id: int,
	prev_idx: int,
	maybe_ghost: bool,
	x0: float,
	y0: float,
	x1: float,
	y1: float,
	world: float
) -> void:
	_birth_times.erase(id)
	_gait.erase(id)
	_locomotion.erase(id)
	_actions.erase(id)
	var fv: Vector3 = _facing.get(id, Vector3.ZERO)
	var side: float = -1.0 if fv.y >= 0.5 else 1.0
	_facing.erase(id)
	if prev_idx >= _prev_smooth.size():
		return
	var last_pos: Vector2 = _prev_smooth[prev_idx]
	if not (maybe_ghost and pos_in_rect(last_pos, x0, y0, x1, y1, world)):
		return
	if _death_effects.size() >= DEATH_CAP:
		_death_effects.pop_front()
	var sp := 0
	var sz := BODY_MIN
	if prev_idx < _prev_bucket.size():
		sp = _prev_bucket[prev_idx]
	if prev_idx < _prev_sizes.size():
		sz = maxf(_prev_sizes[prev_idx] * BODY_SCALE, BODY_MIN)
	# Inherit the agent's body colour so a quadruped ghost keeps its coat hue
	# instead of the neutral-grey value-ramp; hominin atlases are self-coloured
	# (white here) so their ghosts are unchanged.
	var col := Color(1, 1, 1)
	if prev_idx < _prev_color.size():
		col = _prev_color[prev_idx]
	_death_effects.append([last_pos, 0.0, sp, sz, side, col])


# Age and draw the ghosts: fallen figures that fade out quadratically over
# DEATH_TTL seconds while the sim's own carcass disc persists beneath them.
func _refresh_death_effects(delta: float) -> void:
	if _death_mmis.is_empty():
		return
	var write := 0
	for e in _death_effects:
		e[1] += delta
		if e[1] < DEATH_TTL:
			_death_effects[write] = e
			write += 1
	_death_effects.resize(write)
	var buckets: Array = []
	for b in MammalSprites.BUCKET_COUNT:
		buckets.append(PackedInt32Array())
	for i in _death_effects.size():
		buckets[_death_effects[i][2]].append(i)
	for b in MammalSprites.BUCKET_COUNT:
		var mm: MultiMesh = _death_mmis[b].multimesh
		var idx: PackedInt32Array = buckets[b]
		var m := idx.size()
		if m > mm.instance_count:
			mm.instance_count = m
		mm.visible_instance_count = m
		for j in m:
			var e: Array = _death_effects[idx[j]]
			var life: float = 1.0 - float(e[1]) / DEATH_TTL
			# Topple: the ghost starts tilted and eases flat with a slight
			# bounce, dipping vertically mid-fall (the impact squash).
			var ft: float = clampf(float(e[1]) / DEATH_FALL, 0.0, 1.0)
			var ang: float = float(e[4]) * 0.55 * (1.0 - FxMath.ease_out_back(ft))
			var sy: float = float(e[3]) * (1.0 - 0.18 * sin(ft * PI))
			mm.set_instance_transform_2d(j, Transform2D(ang, Vector2(e[3], sy), 0.0, e[0]))
			var c: Color = e[5]
			c.a = 0.85 * life * life
			mm.set_instance_color(j, c)


func _body_colors(n: int) -> PackedColorArray:
	var out := PackedColorArray()
	out.resize(n)
	match _overlay.body_mode:
		_overlay.BODY_DIALECT:
			var hues: PackedFloat32Array = sim.alive_dialect_hue()
			for i in n:
				out[i] = Color.from_hsv(hues[i], 0.7, 0.95)
		_overlay.BODY_DIET:
			out = _ramp_body_colors(n, Palette.RAMP_DIET, sim.alive_diet(), 1.0)
		_overlay.BODY_ENERGY:
			out = _ramp_body_colors(n, Palette.RAMP_ENERGY, sim.alive_energy(), 50.0)
		_overlay.BODY_AFFECT:
			out = _ramp_body_colors(n, Palette.RAMP_AROUSAL, sim.alive_arousal(), 1.0)
		_overlay.BODY_INFECTION:
			out = _ramp_body_colors(n, Palette.RAMP_INFECTION, sim.alive_infection(), 1.0)
		_overlay.BODY_MOOD:
			var moods: PackedInt32Array = sim.alive_moods()
			for i in n:
				out[i] = Palette.MOOD_COLORS[clampi(moods[i], 0, Palette.MOOD_COLORS.size() - 1)]
		_:
			# Species mode: Primate atlases carry their own coat/skin colours, so
			# white; quadruped atlases are neutral grayscale, so each agent gets
			# its per-species coat hue here. Diet/size come from the same batches
			# refresh() already fetched.
			var diet: PackedFloat32Array = sim.alive_diet()
			var sizes: PackedFloat32Array = sim.alive_sizes()
			var sp_ids: PackedInt32Array = sim.alive_species_ids()
			var live: PackedInt32Array = _livestock_flags(n)
			var body_tags: PackedInt32Array = sim.alive_body_tags()
			var have_tags: bool = body_tags.size() == n
			for i in n:
				var tags: int = body_tags[i] if have_tags else 0
				var arch := MammalSprites.archetype_for(diet[i], sizes[i], live[i] != 0, tags)
				out[i] = MammalSprites.coat_hue(arch, sp_ids[i])
	return out


# One body colour per agent from a Palette ramp over a per-agent scalar,
# normalized by `value_scale` (energy runs 0..~50; the rest are already 0..1).
func _ramp_body_colors(
	n: int, ramp: Array, values: PackedFloat32Array, value_scale: float
) -> PackedColorArray:
	var out := PackedColorArray()
	out.resize(n)
	for i in n:
		out[i] = Palette.ramp(ramp, values[i] / value_scale)
	return out


func _livestock_flags(n: int) -> PackedInt32Array:
	if sim.domestication_enabled():
		return sim.alive_livestock_flags()
	var z := PackedInt32Array()
	z.resize(n)
	return z


func _report_visible(n: int) -> void:
	if not _perf_lookup_done:
		_perf_lookup_done = true
		_perf_readout = get_node_or_null("../UI/PerfReadout")
	if _perf_readout != null:
		_perf_readout.set_visible_agents(n)
