extends PanelContainer

const CHAPTER_NAMES: PackedStringArray = [
	"Extinction", "PopCrash", "Speciation", "Migration", "NovelModule", "NovelBehavior"
]
const MAX_RECENT: int = 30

var _counts: Array[int] = [0, 0, 0, 0, 0, 0]
var _recent: Array[Dictionary] = []
var _cursor: int = 0

@onready var sim = get_node("/root/Main/Simulation")
@onready var camera: Camera2D = get_node("/root/Main/Camera2D")
@onready var counts_label: Label = $VBox/Counts
@onready var recent_list: VBoxContainer = $VBox/Scroll/RecentList

func _process(_delta: float) -> void:
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
		var loc: Vector2 = ev["loc"]
		btn.pressed.connect(_jump_to.bind(loc))
		recent_list.add_child(btn)

func _jump_to(loc: Vector2) -> void:
	if loc != Vector2.ZERO:
		camera.position = loc
