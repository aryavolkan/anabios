extends PanelContainer

# Per-species tech table: tech era + adopted inventions, most-advanced first.
# Visible only when the loaded scenario has the invention tree enabled.

const UiTheme = preload("res://scripts/ui_theme.gd")
const REFRESH_EVERY := 6
# Row cap (mirrors dit_panel/population_panel): keep the panel inside its
# fixed slot so a long tail of singleton species can't grow it.
const MAX_ROWS := 6
# Tighter cap while the DIT table is also up — the two share one rail slot.
const SHARED_MAX_ROWS := 4

@onready var sim = get_node("/root/Main/Simulation")
@onready var list: VBoxContainer = $VBox
@onready var _dit: Control = get_parent().get_node_or_null("DitPanel")
var _frame: int = 0
# Scene-authored slot; the panel drops below the DIT table when that one is up.
var _base_y: float = 0.0


func _ready() -> void:
	_base_y = position.y


# The DIT table shares this slot in the right rail, and a scenario that runs the
# environment model *and* the invention tree shows both — they used to overprint
# ("TECH" stamped over "DIT env-optimum = …"). Stack under it instead, clamped so
# the pair never runs off the bottom of the screen.
func _restack() -> void:
	var y: float = _base_y
	if _dit != null and _dit.visible:
		y = _dit.position.y + _dit.size.y + 10.0
	var vp_h: float = get_viewport_rect().size.y
	position.y = minf(y, maxf(_base_y, vp_h - size.y - 10.0))


func _process(_delta: float) -> void:
	if not bool(sim.inventions_enabled()):
		visible = false
		return
	visible = true
	_restack()
	_frame += 1
	if _frame % REFRESH_EVERY != 4:  # phase-offset from dit_panel (== 3)
		return
	var stats: Array = sim.species_stats()
	# Most-advanced species first — that's the interesting end of the tech race.
	stats.sort_custom(
		func(a, b):
			if int(a["tech_era"]) != int(b["tech_era"]):
				return int(a["tech_era"]) > int(b["tech_era"])
			return int(a["count"]) > int(b["count"])
	)
	var cap: int = SHARED_MAX_ROWS if _dit != null and _dit.visible else MAX_ROWS
	var shown: int = min(stats.size(), cap)
	var overflow: int = stats.size() - shown
	_sync_label_count(shown + 1 + (1 if overflow > 0 else 0))  # +1 header row
	var children: Array = list.get_children()
	(children[0] as Label).add_theme_font_size_override("font_size", 13)
	(children[0] as Label).add_theme_color_override("font_color", UiTheme.ACCENT)
	(children[0] as Label).text = "TECH"
	for i in shown:
		var s: Dictionary = stats[i]
		var adopted: Array = s["adopted_inventions"]
		var techs: String = (
			", ".join(adopted)
			if adopted.size() <= 4
			else ", ".join(adopted.slice(0, 4)) + ", +%d" % (adopted.size() - 4)
		)
		(children[i + 1] as Label).text = (
			"sp %d  era %d  n=%d  [%s]"
			% [int(s["species_id"]), int(s["tech_era"]), int(s["count"]), techs]
		)
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
