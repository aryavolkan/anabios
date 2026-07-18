extends PanelContainer

const UiTheme = preload("res://scripts/ui_theme.gd")
const REFRESH_EVERY := 6
# Row cap (mirrors population_panel): keep the panel inside its fixed slot so a
# long tail of singleton species can't grow it and spill onto neighbours.
const MAX_ROWS := 6

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
	# Best-adapted species first — that's the interesting end of the cline.
	stats.sort_custom(func(a, b): return float(a["mean_technique_match"]) > float(b["mean_technique_match"]))
	var shown: int = min(stats.size(), MAX_ROWS)
	var overflow: int = stats.size() - shown
	_sync_label_count(shown + 1 + (1 if overflow > 0 else 0))   # +1 header row
	var children: Array = list.get_children()
	# Header renders one point larger than the rows (parity with the pre-pool version).
	(children[0] as Label).add_theme_font_size_override("font_size", 13)
	(children[0] as Label).add_theme_color_override("font_color", UiTheme.ACCENT)
	(children[0] as Label).text = "DIT  env-optimum = %.2f" % float(sim.env_optimum())
	for i in shown:
		var s: Dictionary = stats[i]
		(children[i + 1] as Label).text = "sp %d   match=%.2f" % [int(s["species_id"]), float(s["mean_technique_match"])]
	if overflow > 0:
		(children[shown + 1] as Label).text = "+%d more species" % overflow

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
