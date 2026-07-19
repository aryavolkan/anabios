extends Control

# Gene↔culture co-evolution time-series. Reads the Rust per-tick history and
# draws a vertical stack of small-multiple charts sharing one time axis.
# Toggle with [Y]. Click a legend label to hide/show a series; click a chart to
# drop a scrub cursor with a value readout. (Read-only; no World mutation.)

@onready var sim = get_node("/root/Main/Simulation")

# Series grouped into stacked sub-charts. Each entry: {key, label, color}.
# unit "01" charts share a fixed [0,1] axis; "auto" charts self-scale.
const CHARTS := [
	{
		"title": "gene vs culture",
		"unit": "01",
		"series": [
			{"key": "communicator_frac", "label": "Communicator gene", "color": Color(0.35, 0.75, 1.0)},
			{"key": "mean_skill", "label": "skill meme", "color": Color(1.0, 0.75, 0.25)},
			{"key": "mean_social_learning", "label": "SocialLearning", "color": Color(0.55, 0.9, 0.55, 0.8)},
			{"key": "mean_individual_learning", "label": "IndividualLearning", "color": Color(0.9, 0.55, 0.85, 0.8)},
		],
	},
	{
		"title": "cultural divergence",
		"unit": "01",
		"series": [
			{"key": "meme_divergence", "label": "dialect L2", "color": Color(1.0, 0.4, 0.4)},
			{"key": "mean_tech_match", "label": "tech match", "color": Color(0.5, 1.0, 0.8)},
		],
	},
	{
		"title": "invention adoption",
		"unit": "01",
		"series": [
			{"key": "inv_stone_tools_frac", "label": "stone tools", "color": Color(0.7, 0.7, 0.75)},
			{"key": "inv_farming_frac", "label": "farming", "color": Color(0.5, 0.9, 0.4)},
			{"key": "inv_writing_frac", "label": "writing", "color": Color(1.0, 0.85, 0.4)},
			{"key": "inv_machinery_frac", "label": "machinery", "color": Color(0.9, 0.55, 0.3)},
			{"key": "inv_nuclear_power_frac", "label": "nuclear", "color": Color(0.65, 0.5, 1.0)},
		],
	},
	{
		"title": "cognition & maladaptive culture",
		"unit": "01",
		"series": [
			{"key": "mean_iq", "label": "mean IQ", "color": Color(0.5, 0.85, 1.0)},
			{"key": "practice_inbreeding_frac", "label": "inbreeding", "color": Color(0.95, 0.45, 0.55)},
			{"key": "practice_child_sacrifice_frac", "label": "child sacrifice", "color": Color(0.85, 0.3, 0.3)},
		],
	},
	{
		"title": "population",
		"unit": "auto",
		"series": [
			{"key": "live_count", "label": "alive", "color": Color(0.8, 0.8, 0.85)},
			{"key": "species_count", "label": "species", "color": Color(0.6, 0.7, 1.0)},
		],
	},
	{
		"title": "genetic diversity",
		"unit": "auto",
		"series": [
			{"key": "genetic_diversity", "label": "mean slot var", "color": Color(0.7, 0.9, 0.6)},
		],
	},
]

const PAD_LEFT := 116.0           # left gutter for legend labels
const PAD_RIGHT := 10.0
const LEGEND_W := 104.0           # label wrap/clip width inside the gutter

# Codex EventType ids we mark, mapped to a line color.
# (0=Extinction, 2=Speciation, 11=DialectFormed, 12=MemeSweep.)
const MARKER_COLORS := {
	0: Color(1.0, 0.3, 0.3, 0.5),    # Extinction
	2: Color(0.6, 0.9, 1.0, 0.5),    # Speciation
	11: Color(1.0, 0.6, 0.2, 0.6),   # DialectFormed
	12: Color(1.0, 0.9, 0.3, 0.6),   # MemeSweep
	17: Color(1.0, 0.9, 0.35, 0.5),  # InventionDiscovered
	18: Color(0.55, 0.95, 1.0, 0.4), # InventionAdopted
	19: Color(0.95, 0.45, 0.55, 0.5), # PracticeDiscovered
	20: Color(0.85, 0.3, 0.3, 0.45),  # PracticeAdopted
}

var _shown: bool = false
var _hidden_keys: Dictionary = {}          # key -> true when toggled off
var _scrub_index: int = -1                  # -1 = none
var _font: Font
var _legend_hitboxes: Array[Dictionary] = []   # [{rect, key}] rebuilt each _draw
var _marks: Array[Dictionary] = []             # [{tick:int, type:int}]
var _mark_cursor: int = 0                       # own cursor over the shared event log

func _ready() -> void:
	visible = false
	_font = ThemeDB.fallback_font

func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_Y:
		_shown = not _shown
		visible = _shown

func _process(_delta: float) -> void:
	_poll_marks()
	if _shown:
		queue_redraw()

# Accumulate markable codex events regardless of visibility, so the marker log
# stays complete even while the panel is hidden. Own cursor over the shared,
# non-draining event log (codex_panel.gd keeps its own cursor independently).
func _poll_marks() -> void:
	# The Rust event log is cleared on scenario (re)load; if it shrank below our
	# cursor, a reload happened — reset so we don't skip the new run's events.
	if sim.codex_event_count() < _mark_cursor:
		_mark_cursor = 0
		_marks.clear()
		_scrub_index = -1
	var evs: Array = sim.codex_events_since(_mark_cursor)
	for ev in evs:
		_mark_cursor = int(ev["index"]) + 1
		var t: int = int(ev["type"])
		if MARKER_COLORS.has(t):
			_marks.append({"tick": int(ev["tick"]), "type": t})

func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		_handle_click(event.position)

func _handle_click(pos: Vector2) -> void:
	# Legend label click toggles that series.
	for hb in _legend_hitboxes:
		if (hb["rect"] as Rect2).has_point(pos):
			var key: String = hb["key"]
			if _hidden_keys.has(key):
				_hidden_keys.erase(key)
			else:
				_hidden_keys[key] = true
			queue_redraw()
			return
	# Otherwise, a click in the plot area moves the scrub cursor.
	var n: int = sim.coevo_history_len()
	if n <= 0:
		return
	var plot_w: float = maxf(1.0, size.x - PAD_LEFT - PAD_RIGHT)
	if pos.x >= PAD_LEFT:
		var frac: float = clampf((pos.x - PAD_LEFT) / plot_w, 0.0, 1.0)
		_scrub_index = int(round(frac * float(n - 1)))
		queue_redraw()

func _draw() -> void:
	_legend_hitboxes.clear()
	# Near-opaque so the charts stay legible over any terrain (the world was
	# bleeding through the lower plots at 0.9).
	draw_rect(Rect2(Vector2.ZERO, size), Color(0.035, 0.05, 0.065, 0.985))
	var n: int = sim.coevo_history_len()
	if n <= 1:
		draw_string(_font, Vector2(12, 26), "co-evolution — waiting for data…",
			HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color.WHITE)
		return

	var ticks: PackedFloat32Array = sim.coevo_series("tick")
	# Fetch each series ONCE this draw (charts otherwise re-fetch per series and
	# again for auto-scale — an O(history) copy each time).
	var cache: Dictionary = {}
	for c in CHARTS:
		for s in c["series"]:
			var k: String = s["key"]
			if not cache.has(k):
				cache[k] = sim.coevo_series(k)
	var plot_w: float = maxf(1.0, size.x - PAD_LEFT - PAD_RIGHT)
	var chart_h: float = (size.y - 24.0) / float(CHARTS.size())
	var y0 := 20.0
	for c in CHARTS:
		_draw_chart(c, cache, n, PAD_LEFT, plot_w, y0, chart_h - 8.0)
		y0 += chart_h

	_draw_marks(ticks, PAD_LEFT, plot_w)

	# Scrub cursor + readout across the full height.
	if _scrub_index >= 0 and _scrub_index < n:
		var sx: float = PAD_LEFT + plot_w * (float(_scrub_index) / float(n - 1))
		draw_line(Vector2(sx, 16), Vector2(sx, size.y - 4), Color(1, 1, 1, 0.5), 1.0)
		_draw_readout(_scrub_index)

func _draw_chart(c: Dictionary, cache: Dictionary, n: int, pad: float, plot_w: float, top: float, h: float) -> void:
	# y-scale.
	var vmax := 1.0
	var vmin := 0.0
	if c["unit"] == "auto":
		vmax = 0.0001
		for s in c["series"]:
			if _hidden_keys.has(s["key"]):
				continue
			for v in (cache[s["key"]] as PackedFloat32Array):
				vmax = maxf(vmax, v)
	# Frame + title.
	draw_rect(Rect2(Vector2(pad, top), Vector2(plot_w, h)), Color(1, 1, 1, 0.06))
	draw_string(_font, Vector2(pad + 4, top + 12), str(c["title"]),
		HORIZONTAL_ALIGNMENT_LEFT, -1, 11, Color(0.8, 0.85, 0.95))
	# Legend rows (left gutter) + polylines.
	var cols: int = int(minf(plot_w, float(n)))
	var legend_y: float = top + 12.0
	for s in c["series"]:
		var key: String = s["key"]
		var col: Color = s["color"]
		var off: bool = _hidden_keys.has(key)
		var draw_col := Color(col.r, col.g, col.b, 0.3) if off else col
		draw_string(_font, Vector2(6, legend_y), s["label"], HORIZONTAL_ALIGNMENT_LEFT, LEGEND_W, 10, draw_col)
		_legend_hitboxes.append({"rect": Rect2(4, legend_y - 10, LEGEND_W + 4, 13), "key": key})
		legend_y += 13.0
		if off:
			continue
		var arr: PackedFloat32Array = cache[key]
		if arr.size() < 2:
			continue
		# Min/max decimation: each pixel column spans a source-index range and
		# emits both its min and max sample, so single-tick spikes survive even
		# when many ticks collapse into one column (spec: "spikes survive").
		var span: float = maxf(0.0001, vmax - vmin)
		var pts := PackedVector2Array()
		for cx in range(cols):
			var lo: int = int(float(cx) / float(cols) * float(n))
			var hi: int = int(float(cx + 1) / float(cols) * float(n))
			if hi <= lo:
				hi = lo + 1
			hi = mini(hi, n)
			var vlo: float = arr[lo]
			var vhi: float = arr[lo]
			for k in range(lo, hi):
				vlo = minf(vlo, arr[k])
				vhi = maxf(vhi, arr[k])
			var px: float = pad + (float(cx) + 0.5) / float(cols) * plot_w
			var py_hi: float = top + h - clampf((vhi - vmin) / span, 0.0, 1.0) * h
			var py_lo: float = top + h - clampf((vlo - vmin) / span, 0.0, 1.0) * h
			pts.push_back(Vector2(px, py_hi))
			pts.push_back(Vector2(px, py_lo))
		if pts.size() >= 2:
			draw_polyline(pts, col, 1.5, true)

# Vertical color-coded lines at each markable event's tick, across all charts.
func _draw_marks(ticks: PackedFloat32Array, pad: float, plot_w: float) -> void:
	var n: int = ticks.size()
	if n < 2 or _marks.is_empty():
		return
	var t_first: float = ticks[0]
	var t_last: float = ticks[n - 1]
	var span: float = maxf(1.0, t_last - t_first)
	for m in _marks:
		var mt: float = float(m["tick"])
		if mt < t_first or mt > t_last:
			continue
		var mx: float = pad + ((mt - t_first) / span) * plot_w
		draw_line(Vector2(mx, 16), Vector2(mx, size.y - 4), MARKER_COLORS[m["type"]], 1.0)

func _draw_readout(index: int) -> void:
	var s: Dictionary = sim.coevo_sample_at(index)
	if s.is_empty():
		return
	var lines := PackedStringArray()
	lines.append("t=%d" % int(s.get("tick", 0)))
	lines.append("comm=%.2f skill=%.2f" % [float(s.get("communicator_frac", 0)), float(s.get("mean_skill", 0))])
	lines.append("div=%.2f match=%.2f" % [float(s.get("meme_divergence", 0)), float(s.get("mean_tech_match", 0))])
	lines.append("alive=%d sp=%d" % [int(s.get("live_count", 0)), int(s.get("species_count", 0))])
	var box := Vector2(150, 8 + lines.size() * 13)
	var origin := Vector2(size.x - box.x - 8, 20)
	draw_rect(Rect2(origin, box), Color(0, 0, 0, 0.75))
	var y := origin.y + 14
	for ln in lines:
		draw_string(_font, Vector2(origin.x + 6, y), ln, HORIZONTAL_ALIGNMENT_LEFT, -1, 10, Color.WHITE)
		y += 13
