extends PanelContainer

const REFRESH_EVERY := 6

@onready var sim = get_node("/root/Main/Simulation")
@onready var list: VBoxContainer = $VBox
var _frame: int = 0

func _process(_delta: float) -> void:
	if not bool(sim.env_active()):
		visible = false
		return
	visible = true
	_frame += 1
	if _frame % REFRESH_EVERY != 0:
		return
	for child in list.get_children():
		child.queue_free()
	var header := Label.new()
	header.add_theme_font_size_override("font_size", 13)
	header.text = "DIT  env-optimum = %.2f" % float(sim.env_optimum())
	list.add_child(header)
	for s in sim.species_stats():
		var lbl := Label.new()
		lbl.add_theme_font_size_override("font_size", 12)
		lbl.text = "sp %d   match=%.2f" % [int(s["species_id"]), float(s["mean_technique_match"])]
		list.add_child(lbl)
