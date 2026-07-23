extends Node

# Replay & event camera (E2). Owns a ring of periodic world snapshots and
# three viewer modes:
#   [R] replay the most recent codex event (rewind -> fast-forward -> pause)
#   [U] run at max speed until the next codex event fires, then pause there
#   [V] event camera: auto-cut tour of recent event locations
# Restore semantics: `Simulation.restore_snapshot` clears the gdext-side event
# log / coevo history (view-only buffers), so the codex panel re-accumulates
# from the resume point — the world itself resumes bit-identically.

const UiTheme = preload("res://scripts/ui_theme.gd")

const RING_EVERY: int = 250
const RING_CAP: int = 16
const REPLAY_STEP: int = 64
const UNTIL_SPEED: int = 64
const EVENT_CAM_DWELL: float = 15.0
const EVENT_CAM_ZOOM: float = 2.0
const EVENT_CAM_RECENT: int = 20

var _ring: Array = []          # [{tick: int, bytes: PackedByteArray}]
var _last_ring_tick: int = -RING_EVERY

var _replaying: bool = false
var _replay_arrived: bool = false
var _replay_target: int = -1   # event tick; pause at world tick == target + 1
var _replay_loc: Vector2 = Vector2.ZERO
var _live_backup: Dictionary = {}

var _until_active: bool = false
var _until_count: int = 0
var _until_speed: int = 1

var _cam_active: bool = false
var _cam_timer: float = 0.0
var _cam_events: Array = []
var _cam_index: int = 0
var _cam_saved_pos: Vector2 = Vector2.ZERO
var _cam_saved_zoom: Vector2 = Vector2.ONE
var _cam_tween: Tween = null

var _highlight: Node2D = null
var _banner: Label = null

@onready var main: Node2D = get_parent()
@onready var sim: Node = main.get_node("Simulation")
@onready var camera: Camera2D = main.get_node("Camera2D")

func _process(delta: float) -> void:
	_capture_ring()
	if _replaying:
		if _replay_arrived:
			return
		# The event stamped tick=T is emitted by the step ending at world
		# tick T+1; fast-forward while we are at or before T so the pause
		# lands with the event freshly fired.
		if int(sim.tick()) <= _replay_target:
			# Clamp the step so the pause lands exactly at target+1 (the tick
			# whose step emits the event), not a full batch past it.
			var remaining: int = _replay_target + 1 - int(sim.tick())
			sim.step_n(mini(REPLAY_STEP, remaining))
		else:
			_replay_arrived = true
			_arrive_replay()
	elif _until_active:
		if int(sim.codex_event_count()) > _until_count:
			_stop_until(true)
	elif _cam_active:
		if Input.is_key_pressed(KEY_W) or Input.is_key_pressed(KEY_A) \
				or Input.is_key_pressed(KEY_S) or Input.is_key_pressed(KEY_D):
			stop_event_cam()
		else:
			_cam_timer -= delta
			if _cam_timer <= 0.0:
				_cam_index = (_cam_index + 1) % _cam_events.size()
				_go_to_cam_event()

func _unhandled_key_input(event: InputEvent) -> void:
	if not (event is InputEventKey and event.pressed and not event.echo):
		return
	match event.keycode:
		KEY_R:
			if _replaying:
				stop_replay()
			else:
				start_replay()
		KEY_U:
			if _until_active:
				_stop_until(false)
			else:
				start_until()
		KEY_V:
			if _cam_active:
				stop_event_cam()
			else:
				start_event_cam()
		KEY_ESCAPE:
			if _replaying:
				stop_replay()
			elif _until_active:
				_stop_until(false)
			elif _cam_active:
				stop_event_cam()

# --- snapshot ring ---------------------------------------------------------

func _capture_ring() -> void:
	if _replaying:
		return
	var t: int = int(sim.tick())
	if t - _last_ring_tick < RING_EVERY:
		return
	var bytes: PackedByteArray = sim.snapshot_bytes()
	if bytes.is_empty():
		return
	_ring.append({"tick": t, "bytes": bytes})
	while _ring.size() > RING_CAP:
		_ring.pop_front()
	_last_ring_tick = t

func _nearest_snapshot(tick: int) -> Dictionary:
	var best: Dictionary = {}
	for entry in _ring:
		if int(entry["tick"]) <= tick and (best.is_empty() or int(entry["tick"]) > int(best["tick"])):
			best = entry
	return best

func _latest_event() -> Dictionary:
	var count: int = int(sim.codex_event_count())
	if count == 0:
		return {}
	var events: Array = sim.codex_events_since(count - 1)
	if events.is_empty():
		return {}
	return events[events.size() - 1]

# --- [R] replay ------------------------------------------------------------

func start_replay() -> void:
	if _until_active:
		_stop_until(false)
	if _cam_active:
		stop_event_cam()
	var ev: Dictionary = _latest_event()
	if ev.is_empty():
		_flash_banner("no codex events yet", 2.0)
		return
	var snap: Dictionary = _nearest_snapshot(int(ev["tick"]))
	if snap.is_empty():
		_flash_banner("event older than the snapshot ring (%d ticks)" % (RING_EVERY * RING_CAP), 2.5)
		return
	_live_backup = {
		"bytes": sim.snapshot_bytes(),
		"paused": main.paused,
		"speed": main.ticks_per_frame,
		# Replay yanks the camera to the event; stash the live view so Esc
		# restores it rather than leaving you zoomed in on the event.
		"cam_pos": camera.position,
		"cam_zoom": camera.zoom,
	}
	if not sim.restore_snapshot(snap["bytes"]):
		_flash_banner("snapshot restore failed", 2.5)
		return
	main.paused = true
	_replaying = true
	_replay_arrived = false
	_replay_target = int(ev["tick"])
	_replay_loc = ev["loc"]
	_set_banner("REPLAY t=%d %s — rewinding…" % [_replay_target, _event_name(ev)], true)

func _arrive_replay() -> void:
	main.paused = true
	# Events without a meaningful location (loc == ZERO) leave the camera be.
	if _replay_loc != Vector2.ZERO:
		camera.position = _replay_loc
		camera.zoom = Vector2(1.5, 1.5)
		_spawn_highlight(_replay_loc)
	_set_banner("REPLAY t=%d · [R]/Esc resume live" % _replay_target, true)

func stop_replay() -> void:
	_replaying = false
	_replay_arrived = false
	_replay_target = -1
	_clear_highlight()
	_clear_banner()
	if not _live_backup.is_empty():
		sim.restore_snapshot(_live_backup["bytes"])
		main.paused = _live_backup["paused"]
		main.ticks_per_frame = _live_backup["speed"]
		camera.position = _live_backup["cam_pos"]
		camera.zoom = _live_backup["cam_zoom"]
		_live_backup = {}

# --- [U] run until next event -----------------------------------------------

func start_until() -> void:
	if _replaying or _cam_active:
		return
	_until_count = int(sim.codex_event_count())
	_until_speed = main.ticks_per_frame
	main.ticks_per_frame = UNTIL_SPEED
	main.paused = false
	_until_active = true
	_set_banner("RUNNING until next event · [U]/Esc cancel", true)

func _stop_until(arrived: bool) -> void:
	_until_active = false
	main.ticks_per_frame = _until_speed
	if arrived:
		main.paused = true
		var ev: Dictionary = _latest_event()
		if not ev.is_empty() and ev["loc"] != Vector2.ZERO:
			camera.position = ev["loc"]
		_set_banner("event t=%d %s · resume when ready" % [int(ev.get("tick", 0)), _event_name(ev)], true)
	else:
		_clear_banner()

# --- [V] event camera --------------------------------------------------------

func start_event_cam() -> void:
	if _replaying or _until_active:
		return
	var count: int = int(sim.codex_event_count())
	if count == 0:
		_flash_banner("no codex events yet", 2.0)
		return
	var from: int = maxi(0, count - EVENT_CAM_RECENT)
	_cam_events = sim.codex_events_since(from)
	# Newest first.
	_cam_events.reverse()
	_cam_index = 0
	_cam_saved_pos = camera.position
	_cam_saved_zoom = camera.zoom
	_cam_active = true
	_set_banner("EVENT CAM · [V]/Esc exit", true)
	_go_to_cam_event()

func _go_to_cam_event() -> void:
	var ev: Dictionary = _cam_events[_cam_index]
	_cam_timer = EVENT_CAM_DWELL
	if _cam_tween != null:
		_cam_tween.kill()
	_cam_tween = create_tween()
	_cam_tween.set_parallel(true)
	_cam_tween.tween_property(camera, "position", ev["loc"], 1.2).set_trans(Tween.TRANS_SINE)
	_cam_tween.tween_property(camera, "zoom", Vector2(EVENT_CAM_ZOOM, EVENT_CAM_ZOOM), 1.2).set_trans(Tween.TRANS_SINE)
	_set_banner("EVENT CAM t=%d %s · [V]/Esc exit" % [int(ev["tick"]), _event_name(ev)], true)

func stop_event_cam() -> void:
	_cam_active = false
	_cam_events = []
	if _cam_tween != null:
		_cam_tween.kill()
		_cam_tween = null
	camera.position = _cam_saved_pos
	camera.zoom = _cam_saved_zoom
	_clear_banner()

# --- chrome ------------------------------------------------------------------

func _event_name(ev: Dictionary) -> String:
	var names: PackedStringArray = preload("res://scripts/codex_panel.gd").CHAPTER_NAMES
	var t: int = int(ev.get("type", -1))
	return names[t] if t >= 0 and t < names.size() else "event"

func _set_banner(text: String, _pin: bool) -> void:
	_clear_banner()
	_banner = Label.new()
	_banner.text = text
	_banner.add_theme_font_size_override("font_size", 15)
	_banner.add_theme_color_override("font_color", UiTheme.ACCENT)
	_banner.add_theme_color_override("font_outline_color", Color(0.0, 0.0, 0.0, 0.75))
	_banner.add_theme_constant_override("outline_size", 5)
	_banner.set_anchors_preset(Control.PRESET_CENTER_TOP)
	_banner.position.y = 12
	main.get_node("UI").add_child(_banner)

func _flash_banner(text: String, seconds: float) -> void:
	_set_banner(text, false)
	var b: Label = _banner
	get_tree().create_timer(seconds).timeout.connect(func() -> void:
		if is_instance_valid(b):
			b.queue_free()
		if _banner == b:
			_banner = null)

func _clear_banner() -> void:
	if _banner != null and is_instance_valid(_banner):
		_banner.queue_free()
	_banner = null

func _spawn_highlight(loc: Vector2) -> void:
	_clear_highlight()
	_highlight = preload("res://scripts/replay_highlight.gd").new()
	_highlight.position = loc
	_highlight.z_index = 10
	main.add_child(_highlight)

func _clear_highlight() -> void:
	if _highlight != null and is_instance_valid(_highlight):
		_highlight.queue_free()
	_highlight = null
