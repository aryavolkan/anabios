extends PanelContainer

const UiTheme = preload("res://scripts/ui_theme.gd")
const CHAPTER_NAMES: PackedStringArray = [
	"Extinction", "PopCrash", "Speciation", "Migration", "NovelModule", "NovelBehavior"
]
# One color per event type so the timeline is scannable at a glance (matches
# the co-evolution chart's marker hues where they overlap).
const CHAPTER_COLORS: PackedColorArray = [
	Color(1.0, 0.42, 0.42),   # 0 Extinction  — red
	Color(1.0, 0.62, 0.35),   # 1 PopCrash    — orange
	Color(0.55, 0.85, 1.0),   # 2 Speciation  — cyan
	Color(0.65, 0.75, 1.0),   # 3 Migration   — blue
	Color(1.0, 0.85, 0.4),    # 4 NovelModule — amber
	Color(0.55, 0.95, 0.6),   # 5 NovelBehavior — green
]
const MAX_RECENT: int = 30

var _counts: Array[int] = [0, 0, 0, 0, 0, 0]
var _recent: Array[Dictionary] = []
var _cursor: int = 0

@onready var sim = get_node("/root/Main/Simulation")
@onready var camera: Camera2D = get_node("/root/Main/Camera2D")
@onready var counts_label: Label = $VBox/Counts
@onready var recent_list: VBoxContainer = $VBox/Scroll/RecentList

func _ready() -> void:
	# The running tally reads as the panel's title — mark it with the accent.
	counts_label.add_theme_color_override("font_color", UiTheme.ACCENT)

func _process(_delta: float) -> void:
	# Event log is cleared on scenario (re)load; a shrink below our cursor means
	# a reload — reset so counts/recent reflect the new run.
	if sim.codex_event_count() < _cursor:
		_cursor = 0
		_counts = [0, 0, 0, 0, 0, 0]
		_recent.clear()
	var events: Array = sim.codex_events_since(_cursor)
	if events.is_empty():
		return
	for ev in events:
		_cursor = int(ev["index"]) + 1
		var t: int = int(ev["type"])
		if t >= 0 and t < _counts.size():
			_counts[t] += 1
		_recent.append(ev)
		while _recent.size() > MAX_RECENT:
			_recent.pop_front()
	_render()

func _render() -> void:
	var parts: PackedStringArray = []
	for i in CHAPTER_NAMES.size():
		parts.append("%s: %d" % [CHAPTER_NAMES[i], _counts[i]])
	counts_label.text = "  ".join(parts)

	for child in recent_list.get_children():
		child.queue_free()
	# Newest first.
	for i in range(_recent.size() - 1, -1, -1):
		var ev: Dictionary = _recent[i]
		var t: int = int(ev["type"])
		var name: String = CHAPTER_NAMES[t] if t < CHAPTER_NAMES.size() else "Event"
		var btn := Button.new()
		btn.text = "t=%d %s sp=%d" % [int(ev["tick"]), name, int(ev["species_id"])]
		btn.alignment = HORIZONTAL_ALIGNMENT_LEFT
		btn.add_theme_font_size_override("font_size", 11)
		if t >= 0 and t < CHAPTER_COLORS.size():
			btn.add_theme_color_override("font_color", CHAPTER_COLORS[t])
		var loc: Vector2 = ev["loc"]
		btn.pressed.connect(_jump_to.bind(loc))
		recent_list.add_child(btn)

func _jump_to(loc: Vector2) -> void:
	if loc != Vector2.ZERO:
		camera.position = loc
