extends PanelContainer

const UiTheme = preload("res://scripts/ui_theme.gd")
const REFRESH_EVERY := 6
# Cap rows so the panel (accent header + species rows) stays inside its fixed
# slot (see main.tscn) and clears the DIT panel stacked directly below it. A
# long tail of singleton species would otherwise spill onto its neighbours.
const MAX_ROWS := 7

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
	# Row 0 is the accent header; species rows follow.
	var rows: int = 1 + shown + (1 if overflow > 0 else 0)
	_sync_label_count(rows)
	var children: Array = list.get_children()
	var header := children[0] as Label
	header.text = "species (%d)" % stats.size()
	header.add_theme_color_override("font_color", UiTheme.ACCENT)
	for i in shown:
		var s: Dictionary = stats[i]
		var row := children[i + 1] as Label
		# Reset color in case this label was previously the dimmed overflow row.
		row.add_theme_color_override("font_color", UiTheme.TEXT)
		row.text = "sp %d   n=%d   E=%.0f" % [int(s["species_id"]), int(s["count"]), float(s["mean_energy"])]
	if overflow > 0:
		var more := children[shown + 1] as Label
		more.text = "+%d more species" % overflow
		more.add_theme_color_override("font_color", UiTheme.TEXT_DIM)

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
