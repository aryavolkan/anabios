extends PanelContainer

const REFRESH_EVERY := 6

@onready var sim = get_node("/root/Main/Simulation")
@onready var list: VBoxContainer = $VBox
var _frame: int = 0

func _process(_delta: float) -> void:
	_frame += 1
	if _frame % REFRESH_EVERY != 0:
		return
	var stats: Array = sim.species_stats()
	_sync_label_count(stats.size())
	var children: Array = list.get_children()
	for i in stats.size():
		var s: Dictionary = stats[i]
		(children[i] as Label).text = "sp %d   n=%d   E=%.0f" % [int(s["species_id"]), int(s["count"]), float(s["mean_energy"])]

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
