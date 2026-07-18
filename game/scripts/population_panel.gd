extends PanelContainer

const REFRESH_EVERY := 6
# Cap rows so the panel stays inside its fixed slot (see main.tscn). A long
# tail of singleton species would otherwise grow the container and spill its
# text onto the world and the codex panel below.
const MAX_ROWS := 10

@onready var sim = get_node("/root/Main/Simulation")
@onready var list: VBoxContainer = $VBox
var _frame: int = 0

func _process(_delta: float) -> void:
	_frame += 1
	if _frame % REFRESH_EVERY != 0:
		return
	var stats: Array = sim.species_stats()
	# Most-populous first, so the meaningful species stay visible and the
	# singleton tail is what gets summarized away.
	stats.sort_custom(func(a, b): return int(a["count"]) > int(b["count"]))
	var shown: int = min(stats.size(), MAX_ROWS)
	var overflow: int = stats.size() - shown
	var rows: int = shown + (1 if overflow > 0 else 0)
	_sync_label_count(rows)
	var children: Array = list.get_children()
	for i in shown:
		var s: Dictionary = stats[i]
		(children[i] as Label).text = "sp %d   n=%d   E=%.0f" % [int(s["species_id"]), int(s["count"]), float(s["mean_energy"])]
	if overflow > 0:
		var more := children[shown] as Label
		more.text = "+%d more species" % overflow
		more.add_theme_color_override("font_color", Color(0.7, 0.75, 0.82))

func _sync_label_count(want: int) -> void:
	var have: int = list.get_child_count()
	while have < want:
		var lbl := Label.new()
		lbl.add_theme_font_size_override("font_size", 12)
		list.add_child(lbl)
		have += 1
	while have > want:
		list.get_child(have - 1).queue_free()
		have -= 1
