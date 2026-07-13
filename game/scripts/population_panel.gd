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
	for child in list.get_children():
		child.queue_free()
	for s in stats:
		var lbl := Label.new()
		lbl.add_theme_font_size_override("font_size", 12)
		lbl.text = "sp %d   n=%d   E=%.0f" % [int(s["species_id"]), int(s["count"]), float(s["mean_energy"])]
		list.add_child(lbl)
