extends RichTextLabel

const MAX_LINES: int = 80
const TYPE_NAMES: PackedStringArray = ["Extinction", "PopCrash", "Speciation"]

var _lines: Array[String] = []

@onready var sim = get_node("/root/Main/Simulation")

func _process(_delta: float) -> void:
	var events: Array = sim.take_codex_events()
	if events.is_empty():
		return
	for ev in events:
		var name: String = TYPE_NAMES[int(ev["type"])] if int(ev["type"]) < TYPE_NAMES.size() else "Event"
		_lines.append("t=%d %s species=%d value=%.2f" % [
			ev["tick"], name, ev["species_id"], ev["value"]
		])
		while _lines.size() > MAX_LINES:
			_lines.pop_front()
	text = "\n".join(_lines)
	scroll_to_line(get_line_count() - 1)
