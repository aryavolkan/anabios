extends Node

# Showcase Director — plays a scripted chapter timeline over the live sim for
# recorded demos. Enabled by ANABIOS_SHOWCASE=<timeline.json>; inert otherwise
# (Main only adds this node when the env var is set).
#
# Timeline format (JSON):
#   {"beats": [
#     {"at_tick": 120, "do": [...]},          # fire when the sim reaches tick
#     {"at_event": "Domesticated", "do": [...]}, # fire on the NEXT codex event
#                                              #   of that type (armed now)
#     {"on_event": "Domesticated", "do": [...]}, # fire on the LATEST known
#                                              #   event of that type (scans
#                                              #   the whole log; waits if none)
#     {"after": 4.0, "do": [...]},            # fire N seconds after the prev beat
#   ]}
# Any trigger may add "timeout": <seconds> — if the trigger has not fired by
# then, the beat is SKIPPED (logged) instead of stalling the recording.
# A beat with no trigger fires immediately after the previous one. Actions:
#   {"camera": {"x": 240, "y": 430, "zoom": 2.5, "dur": 2.0}}
#       or {"camera": {"at": "event", "zoom": 2.5}}  # the triggering event's loc
#   {"speed": 64}  {"pause": true}
#   {"title": {"chapter": "THE HERD", "subtitle": "domestication", "dur": 3.0}}
#   {"lower_third": {"text": "t=1234 AnimalDomesticated", "dur": 4.0}}
#   {"ground": "markets"}  # biome|pheromone0..3|env|succession|markets (or int)
#   {"body": "dialect"}    # species|dialect|diet|energy (or int)
#   {"panel": {"name": "tech", "visible": true}}
#   {"highlight": {"x": 250, "y": 445}}  or  {"highlight": "event"}
#   {"clear_highlight": true}
#   {"letterbox": true}  {"hud": false}
#   {"follow": {"near": [250, 445]}}  or  {"follow": "event"}  {"unfollow": true}
#   {"end": true}  # quits (finishes a --write-movie recording)

const ShowcaseCard = preload("res://scripts/showcase_card.gd")
const ReplayHighlight = preload("res://scripts/replay_highlight.gd")
const CHAPTER_NAMES: PackedStringArray = preload("res://scripts/codex_panel.gd").CHAPTER_NAMES

const GROUND_MODES := {
	"biome": 0, "pheromone0": 1, "pheromone1": 2, "pheromone2": 3, "pheromone3": 4,
	"env": 5, "succession": 6, "markets": 7,
}
const BODY_MODES := {"species": 0, "dialect": 1, "diet": 2, "energy": 3}
const PANEL_NODES := {
	"evo": "EvolutionPanel", "coevo": "CoevolutionPanel", "tech": "TechPanel",
	"inspector": "Inspector", "legend": "LegendPanel", "codex": "CodexPanel",
	"population": "PopulationPanel", "dit": "DitPanel",
}

var _beats: Array = []
var _idx: int = 0
# Current trigger: waiting on a tick, an event name, a delay, or nothing.
var _wait_tick: int = -1
var _wait_event: String = ""
var _wait_recent: String = ""
var _wait_delay: float = -1.0
var _timeout: float = -1.0  # seconds left before the trigger is abandoned
var _event_cursor: int = 0
var _scan_cursor: int = 0
var _recent_loc: Vector2 = Vector2.ZERO
var _recent_found: bool = false
var _event_loc: Vector2 = Vector2.ZERO  # loc of the event that fired this beat
var _follow_id: int = -1
var _cam_tween: Tween = null
var _highlight: Node2D = null
var _card: CanvasLayer = null

@onready var main: Node2D = get_parent()
@onready var sim: Node = main.get_node("Simulation")
@onready var camera: Camera2D = main.get_node("Camera2D")
@onready var overlay: Node = main.get_node("OverlayManager")

func _ready() -> void:
	var path: String = OS.get_environment("ANABIOS_SHOWCASE")
	var text: String = FileAccess.get_file_as_string(path)
	if text.is_empty():
		push_error("[showcase] could not read timeline " + path)
		return
	var parsed: Variant = JSON.parse_string(text)
	if not (parsed is Dictionary) or not (parsed as Dictionary).get("beats") is Array:
		push_error("[showcase] bad timeline (need {\"beats\": [...]}) in " + path)
		return
	_beats = (parsed as Dictionary)["beats"]
	if _beats.is_empty():
		push_error("[showcase] timeline has no beats: " + path)
		return
	GameConfig.showcase_active = true
	# Clean footage: the interactive controls are noise on a recording.
	main.get_node("UI/TimeControls").visible = false
	_card = ShowcaseCard.new()
	_card.name = "ShowcaseCard"
	main.get_node("UI").add_child(_card)
	_arm(_beats[0])
	print("[showcase] playing %s (%d beats)" % [path, _beats.size()])

func _arm(beat: Dictionary) -> void:
	_event_cursor = int(sim.codex_event_count())
	_wait_tick = int(beat.get("at_tick", -1))
	_wait_event = str(beat.get("at_event", ""))
	_wait_recent = str(beat.get("on_event", ""))
	_wait_delay = float(beat.get("after", -1.0))
	_timeout = float(beat.get("timeout", -1.0))
	if not _wait_recent.is_empty():
		_scan_cursor = 0
		_recent_found = false
	if _wait_tick < 0 and _wait_event.is_empty() and _wait_recent.is_empty() and _wait_delay < 0.0:
		_fire(beat)  # no trigger: run immediately

func _process(delta: float) -> void:
	_update_follow()
	if _idx >= _beats.size():
		return
	if _timeout >= 0.0:
		_timeout -= delta
		if _timeout <= 0.0:
			print("[showcase] beat %d/%d TIMED OUT — skipped" % [_idx + 1, _beats.size()])
			_idx += 1
			if _idx < _beats.size():
				_arm(_beats[_idx])
			return
	var beat: Dictionary = _beats[_idx]
	if _wait_tick >= 0 and int(sim.tick()) >= _wait_tick:
		_fire(beat)
	elif not _wait_event.is_empty():
		var count: int = int(sim.codex_event_count())
		if count > _event_cursor:
			var found := false
			for ev in sim.codex_events_since(_event_cursor):
				if _event_name(ev) == _wait_event:
					_event_loc = ev.get("loc", Vector2.ZERO)
					found = true
					break
			_event_cursor = count  # scan each new event exactly once
			if found:
				_fire(beat)
	elif not _wait_recent.is_empty():
		var count: int = int(sim.codex_event_count())
		if count > _scan_cursor:
			for ev in sim.codex_events_since(_scan_cursor):
				if _event_name(ev) == _wait_recent:
					_recent_found = true
					_recent_loc = ev.get("loc", Vector2.ZERO)
			_scan_cursor = count
		if _recent_found:
			_event_loc = _recent_loc
			_fire(beat)
	elif _wait_delay >= 0.0:
		_wait_delay -= delta
		if _wait_delay <= 0.0:
			_fire(beat)

func _event_name(ev: Dictionary) -> String:
	var t: int = int(ev.get("type", -1))
	return CHAPTER_NAMES[t] if t >= 0 and t < CHAPTER_NAMES.size() else ""

func _fire(beat: Dictionary) -> void:
	print("[showcase] beat %d/%d @ tick %d" % [_idx + 1, _beats.size(), int(sim.tick())])
	for action in beat.get("do", []):
		_do(action)
	_idx += 1
	if _idx < _beats.size():
		_arm(_beats[_idx])

# Event-triggered beats point the camera / highlight / follow at the event's
# location — but some event types carry no meaningful location (loc == ZERO,
# the world's corner). Like the replay manager, leave the camera be then.
func _event_loc_valid() -> bool:
	return _event_loc != Vector2.ZERO

func _do(action: Dictionary) -> void:
	for key in action:
		var v: Variant = action[key]
		match key:
			"camera":
				if _cam_tween != null:
					_cam_tween.kill()
				var dur: float = float(v.get("dur", 2.0))
				var target := Vector2(float(v.get("x", camera.position.x)), float(v.get("y", camera.position.y)))
				if str(v.get("at", "")) == "event":
					if not _event_loc_valid():
						break  # no usable loc: hold the current shot
					target = _event_loc
				_cam_tween = create_tween()
				_cam_tween.set_parallel(true)
				_cam_tween.set_trans(Tween.TRANS_SINE)
				_cam_tween.set_ease(Tween.EASE_IN_OUT)
				_cam_tween.tween_property(camera, "position", target, dur)
				if v.has("zoom"):
					var z: float = float(v["zoom"])
					_cam_tween.tween_property(camera, "zoom", Vector2(z, z), dur)
			"speed":
				main.ticks_per_frame = int(v)
			"pause":
				main.paused = bool(v)
			"title":
				_card.show_title(str(v.get("chapter", "")), str(v.get("subtitle", "")),
					float(v.get("dur", 3.2)))
			"lower_third":
				_card.show_lower(str(v.get("text", "")), float(v.get("dur", 4.5)))
			"ground":
				# JSON numbers parse as floats; accept both numbers and names.
				overlay.ground_mode = int(v) if v is float or v is int \
					else int(GROUND_MODES.get(str(v), 0))
			"body":
				overlay.body_mode = int(v) if v is float or v is int \
					else int(BODY_MODES.get(str(v), 0))
			"panel":
				var pname: String = str(v.get("name", ""))
				if not PANEL_NODES.has(pname):
					push_warning("[showcase] unknown panel " + pname)
					break
				var node: Node = main.get_node_or_null("UI/" + str(PANEL_NODES[pname]))
				if node != null:
					var vis: bool = bool(v.get("visible", true))
					node.visible = vis
					# The chart panels gate their redraw on _shown.
					if pname in ["evo", "coevo"]:
						node.set("_shown", vis)
			"highlight":
				_clear_highlight()
				var at_event: bool = v is String
				if at_event and not _event_loc_valid():
					break  # no usable loc: no ring
				_highlight = ReplayHighlight.new()
				_highlight.position = _event_loc if at_event \
					else Vector2(float(v.get("x", 0.0)), float(v.get("y", 0.0)))
				_highlight.z_index = 10
				main.add_child(_highlight)
			"clear_highlight":
				_clear_highlight()
			"letterbox":
				_card.set_letterbox(bool(v))
			"hud":
				main.get_node("UI/HUD").visible = bool(v)
			"follow":
				if v is String:
					if not _event_loc_valid():
						break
					_follow_id = int(sim.agent_near(_event_loc, 100000.0))
				else:
					var near: Array = v.get("near", [0.0, 0.0])
					_follow_id = int(sim.agent_near(Vector2(float(near[0]), float(near[1])), 100000.0))
			"unfollow":
				_follow_id = -1
			"end":
				get_tree().quit()
			_:
				push_warning("[showcase] unknown action " + key)

# Ease the camera onto the followed agent each frame; drop the follow if the
# agent died (the timeline should cut away with an explicit camera beat).
func _update_follow() -> void:
	if _follow_id < 0:
		return
	var info: Dictionary = sim.get_agent_info(_follow_id)
	if not info.get("alive", false):
		_follow_id = -1
		return
	camera.position = camera.position.lerp(info["position"], 0.12)

func _clear_highlight() -> void:
	if _highlight != null and is_instance_valid(_highlight):
		_highlight.queue_free()
	_highlight = null
