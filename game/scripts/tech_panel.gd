extends PanelContainer

# Per-species tech table: tech era + adopted inventions, most-advanced first.
# Visible only when the loaded scenario has the invention tree enabled.

const UiTheme = preload("res://scripts/ui_theme.gd")
const REFRESH_EVERY := 6
# Row cap (mirrors dit_panel/population_panel): keep the panel inside its
# fixed slot so a long tail of singleton species can't grow it.
const MAX_ROWS := 6

@onready var sim = get_node("/root/Main/Simulation")
@onready var list: VBoxContainer = $VBox
var _frame: int = 0

func _process(_delta: float) -> void:
	if not bool(sim.inventions_enabled()):
		visible = false
		return
	visible = true
	_frame += 1
	if _frame % REFRESH_EVERY != 4:   # phase-offset from dit_panel (== 3)
		return
	var stats: Array = sim.species_stats()
	# Most-advanced species first — that's the interesting end of the tech race.
	stats.sort_custom(func(a, b):
		if int(a["tech_era"]) != int(b["tech_era"]):
			return int(a["tech_era"]) > int(b["tech_era"])
		return int(a["count"]) > int(b["count"]))
	var shown: int = min(stats.size(), MAX_ROWS)
	var overflow: int = stats.size() - shown
	_sync_label_count(shown + 1 + (1 if overflow > 0 else 0))   # +1 header row
	var children: Array = list.get_children()
	(children[0] as Label).add_theme_font_size_override("font_size", 13)
	(children[0] as Label).add_theme_color_override("font_color", UiTheme.ACCENT)
	(children[0] as Label).text = "TECH"
	for i in shown:
		var s: Dictionary = stats[i]
		var adopted: Array = s["adopted_inventions"]
		var techs: String = ", ".join(adopted) if adopted.size() <= 4 \
			else ", ".join(adopted.slice(0, 4)) + ", +%d" % (adopted.size() - 4)
		(children[i + 1] as Label).text = "sp %d  era %d  n=%d  [%s]" % [
			int(s["species_id"]), int(s["tech_era"]), int(s["count"]), techs]
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
