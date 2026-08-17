extends PanelContainer

# Evolution panel (E5) — trait-drift lines for the dominant species and the
# living phylogeny. Toggle with [T]. Read-only.

const UiTheme = preload("res://scripts/ui_theme.gd")

# (slot id, label, color) for the trait-drift chart.
const TRAIT_SERIES := [
	[0, "size", Color(0.9, 0.7, 0.3)],
	[5, "metabolism", Color(0.55, 0.85, 0.5)],
	[26, "perception", Color(0.5, 0.75, 1.0)],
	[12, "openness", Color(0.85, 0.6, 0.9)],
]
const CHART_W := 300
const CHART_H := 80
# 6, not 12: the panel sits between the minimap and the legend panel, and a
# taller phylogeny list grew it down into the legend.
const PHYLO_MAX := 6
# Resting top edge — clear of the minimap, which ends at y=270.
const TOP_Y := 288.0
# Floor for the slide-up when the legend panel below is unusually tall (clear of
# the HUD line).
const MIN_Y := 40.0
# Narrowest y-axis window the auto-scale will open to. Genome traits drift
# inside a few hundredths of each other, so a fixed [0,1] axis drew every series
# as one flat line through the middle of the chart; this is the floor that keeps
# a genuinely constant trait from being amplified into noise.
const MIN_SPAN := 0.02

var _shown: bool = false

@onready var sim = get_node("/root/Main/Simulation")
var _chart: Control
var _phylo: Label
var _title: Label
var _font: Font


func _ready() -> void:
	visible = false
	_font = ThemeDB.fallback_font
	var vb := VBoxContainer.new()
	vb.add_theme_constant_override("separation", 6)
	add_child(vb)
	_title = Label.new()
	_title.add_theme_color_override("font_color", UiTheme.ACCENT)
	vb.add_child(_title)
	_chart = Control.new()
	_chart.custom_minimum_size = Vector2(CHART_W, CHART_H)
	_chart.draw.connect(_draw_chart)
	vb.add_child(_chart)
	# One label per series, painted in that series' own colour — the chart is
	# four overlapping lines, and a single dim caption gave no way to tell them
	# apart.
	var legend := HBoxContainer.new()
	legend.add_theme_constant_override("separation", 10)
	for s in TRAIT_SERIES:
		var l := Label.new()
		l.text = s[1]
		l.add_theme_font_size_override("font_size", 11)
		l.add_theme_color_override("font_color", s[2])
		legend.add_child(l)
	vb.add_child(legend)
	_phylo = Label.new()
	_phylo.add_theme_font_size_override("font_size", 12)
	vb.add_child(_phylo)
	# Left rail, below the minimap (which ends at y=270) and above the legend
	# panel: the old y=40 put the panel straight on top of the minimap, so the
	# world overview showed through it.
	set_anchors_preset(Control.PRESET_TOP_LEFT)
	position.y = TOP_Y
	position.x = 12


func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_T:
		_shown = not _shown
		visible = _shown


# Keep the panel inside the left rail's free band. The legend panel below grows
# upward as key rows are added ([M] module swatches, the [C] body ramps), so its
# top edge is the live floor; only an unusually tall legend pushes this panel up
# off its resting spot.
func _reposition() -> void:
	var legend: Control = get_parent().get_node_or_null("LegendPanel")
	var floor_y: float = (legend.position.y - 8.0) if legend != null else 612.0
	position.y = clampf(floor_y - size.y, MIN_Y, TOP_Y)


func _process(_delta: float) -> void:
	if not _shown:
		return
	_reposition()
	var rows: Array = sim.phylogeny()
	if rows.is_empty():
		return
	# Dominant species = largest count.
	var top: Dictionary = rows[0]
	for r in rows:
		if int(r["count"]) > int(top["count"]):
			top = r
	_title.text = "EVOLUTION · dominant sp%d (n=%d)" % [int(top["id"]), int(top["count"])]
	_chart.set_meta("sid", int(top["id"]))
	_chart.queue_redraw()
	# Phylogeny list: top by count, indented by depth.
	var sorted := rows.duplicate()
	sorted.sort_custom(func(a, b): return int(a["count"]) > int(b["count"]))
	var lines: PackedStringArray = []
	for r in sorted.slice(0, PHYLO_MAX):
		var indent := ""
		for _i in mini(int(r["depth"]), 8):
			indent += "  "
		lines.append("%ssp%d n=%d" % [indent, int(r["id"]), int(r["count"])])
	if sorted.size() > PHYLO_MAX:
		lines.append("+%d more species" % (sorted.size() - PHYLO_MAX))
	_phylo.text = "\n".join(lines)


func _draw_chart() -> void:
	var sid: int = int(_chart.get_meta("sid", -1))
	if sid < 0:
		return
	var series_data := []
	var max_len := 0
	for s in TRAIT_SERIES:
		var data: PackedFloat32Array = sim.species_trait_series(sid, s[0])
		series_data.append(data)
		max_len = maxi(max_len, data.size())
	if max_len < 2:
		return
	# Auto-scale to the data. Traits sit within a few hundredths of 0.5, so the
	# old fixed [0,1] axis flattened every series onto one horizontal line; fit
	# the window to the actual min/max (widened to MIN_SPAN and padded) so drift
	# is visible, and print the bounds since the axis is no longer implicit.
	var lo := INF
	var hi := -INF
	for data in series_data:
		for v in data as PackedFloat32Array:
			lo = minf(lo, v)
			hi = maxf(hi, v)
	if lo > hi:
		return
	var mid: float = (lo + hi) * 0.5
	var span: float = maxf(hi - lo, MIN_SPAN)
	lo = mid - span * 0.6  # 0.6 = half-span plus 10% headroom top and bottom
	hi = mid + span * 0.6
	span = hi - lo
	# Frame.
	_chart.draw_rect(Rect2(0, 0, CHART_W, CHART_H), Color(1, 1, 1, 0.08), false, 1.0)
	for k in series_data.size():
		var data: PackedFloat32Array = series_data[k]
		if data.size() < 2:
			continue
		var col: Color = TRAIT_SERIES[k][2]
		var prev := Vector2.ZERO
		for i in data.size():
			var x := float(i) / float(data.size() - 1) * CHART_W
			var y := CHART_H - clampf((data[i] - lo) / span, 0.0, 1.0) * CHART_H
			var p := Vector2(x, y)
			if i > 0:
				_chart.draw_line(prev, p, col, 1.5)
			prev = p
	var axis := Color(0.56, 0.67, 0.69)
	_chart.draw_string(_font, Vector2(3, 10), "%.3f" % hi, HORIZONTAL_ALIGNMENT_LEFT, -1, 9, axis)
	_chart.draw_string(
		_font, Vector2(3, CHART_H - 3), "%.3f" % lo, HORIZONTAL_ALIGNMENT_LEFT, -1, 9, axis
	)
