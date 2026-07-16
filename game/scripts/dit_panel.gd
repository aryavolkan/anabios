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
	if _frame % REFRESH_EVERY != 3:   # phase-offset from population_panel (== 0)
		return
	var stats: Array = sim.species_stats()
	_sync_label_count(stats.size() + 1)   # +1 header row
	var children: Array = list.get_children()
	# Header renders one point larger than the rows (parity with the pre-pool version).
	(children[0] as Label).add_theme_font_size_override("font_size", 13)
	(children[0] as Label).text = "DIT  env-optimum = %.2f" % float(sim.env_optimum())
	for i in stats.size():
		var s: Dictionary = stats[i]
		(children[i + 1] as Label).text = "sp %d   match=%.2f" % [int(s["species_id"]), float(s["mean_technique_match"])]

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
