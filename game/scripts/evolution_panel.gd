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
const PHYLO_MAX := 12

var _shown: bool = false

@onready var sim = get_node("/root/Main/Simulation")
var _chart: Control
var _phylo: Label
var _title: Label

func _ready() -> void:
	visible = false
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
	var legend := Label.new()
	var parts: PackedStringArray = []
	for s in TRAIT_SERIES:
		parts.append(s[1])
	legend.text = "  ".join(parts)
	legend.add_theme_font_size_override("font_size", 11)
	legend.add_theme_color_override("font_color", UiTheme.TEXT_DIM)
	vb.add_child(legend)
	_phylo = Label.new()
	_phylo.add_theme_font_size_override("font_size", 12)
	vb.add_child(_phylo)
	# Mid-left, clear of the HUD and the codex panel.
	set_anchors_preset(Control.PRESET_CENTER_LEFT)
	position.y = 40
	position.x = 12

func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_T:
		_shown = not _shown
		visible = _shown

func _process(_delta: float) -> void:
	if not _shown:
		return
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
			var y := CHART_H - clampf(data[i], 0.0, 1.0) * CHART_H
			var p := Vector2(x, y)
			if i > 0:
				_chart.draw_line(prev, p, col, 1.5)
			prev = p
