extends PanelContainer

const UiTheme = preload("res://scripts/ui_theme.gd")
const CHAPTER_NAMES: PackedStringArray = [
	"Extinction", "PopCrash", "Speciation", "Migration", "NovelModule", "NovelBehavior",
	"Predation", "CombatRaid", "ArmsRace", "Territory", "NichePartition",
	"Dialect", "MemeSweep", "AlarmCall", "Cooperation", "PackHunting", "HerdCohesion",
	"Discovery", "Adoption", "BadIdea", "BadAdopt", "Trade", "DowryBirth"
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
	Color(1.0, 0.5, 0.5),     # 6 Predation   — salmon
	Color(1.0, 0.55, 0.3),    # 7 CombatRaid  — deep orange
	Color(1.0, 0.5, 0.85),    # 8 ArmsRace    — magenta
	Color(0.45, 0.9, 0.85),   # 9 Territory   — teal
	Color(0.7, 0.95, 0.5),    # 10 NichePartition — lime
	Color(1.0, 0.7, 0.35),    # 11 Dialect    — light orange
	Color(1.0, 0.9, 0.4),     # 12 MemeSweep  — yellow
	Color(1.0, 0.6, 0.75),    # 13 AlarmCall  — pink
	Color(0.6, 1.0, 0.7),     # 14 Cooperation — mint
	Color(0.85, 0.7, 0.5),    # 15 PackHunting — tan
	Color(0.6, 0.8, 1.0),     # 16 HerdCohesion — sky
	Color(1.0, 0.9, 0.35),    # 17 Discovery  — gold
	Color(0.55, 0.95, 1.0),   # 18 Adoption   — light sky
	Color(0.95, 0.45, 0.55),  # 19 PracticeDiscovered — rose
	Color(0.85, 0.3, 0.3),    # 20 PracticeAdopted    — dark red
	Color(0.95, 0.8, 0.45),   # 21 ResourceTraded     — wheat
	Color(0.8, 0.95, 0.9),    # 22 DowryBirth         — pale mint
]
const MAX_RECENT: int = 30

var _counts: Array[int] = []
var _recent: Array[Dictionary] = []
var _cursor: int = 0

@onready var sim = get_node("/root/Main/Simulation")
@onready var camera: Camera2D = get_node("/root/Main/Camera2D")
@onready var counts_label: Label = $VBox/Counts
@onready var recent_list: VBoxContainer = $VBox/Scroll/RecentList

func _ready() -> void:
	# These arrays are indexed by the core EventType discriminant, so they must
	# stay one-per-variant and in sync with each other. Assert it at boot: a new
	# EventType added core-side without a name/color here would otherwise render
	# as "kind N" (or index out of range) with no other warning.
	assert(CHAPTER_NAMES.size() == CHAPTER_COLORS.size(),
		"codex name/color arrays out of sync")
	assert(CHAPTER_NAMES.size() == int(sim.event_type_count()),
		"codex arrays lag core EventType — add the new variant's name and color")
	# The running tally reads as the panel's title — mark it with the accent.
	counts_label.add_theme_color_override("font_color", UiTheme.ACCENT)
	# With 23 event types the single-line tally overflows the panel; wrap it.
	counts_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_counts.resize(CHAPTER_NAMES.size())
	_counts.fill(0)

func _process(_delta: float) -> void:
	# Event log is cleared on scenario (re)load; a shrink below our cursor means
	# a reload — reset so counts/recent reflect the new run.
	if sim.codex_event_count() < _cursor:
		_cursor = 0
		_counts.fill(0)
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
	# Show only event types that have actually occurred — most of the 23 are
	# zero, and listing them all overflows the panel with noise.
	var parts: PackedStringArray = []
	for i in CHAPTER_NAMES.size():
		if _counts[i] > 0:
			parts.append("%s: %d" % [CHAPTER_NAMES[i], _counts[i]])
	counts_label.text = "  ".join(parts) if not parts.is_empty() else "codex"

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
