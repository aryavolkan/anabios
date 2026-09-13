extends PanelContainer
# Research panel (Phase 5 of docs/superpowers/specs/2026-09-12-pixel-world-at-
# scale-design.md): the reference boards' "Research" list — one row per
# invention in tree order with its icon, name, an adoption bar (share of the
# living population holding it) and a check once it is essentially universal.
# Visible only when the scenario's invention tree is on. Pure presentation
# over `alive_invention_masks()` and `invention_catalog()`.

const HudCommon = preload("res://scripts/hud_common.gd")
const PixelFont = preload("res://scripts/pixel_font.gd")
const HudIcons = preload("res://scripts/hud_icons.gd")
const UiTheme = preload("res://scripts/ui_theme.gd")

const REFRESH_EVERY := 30
const ROW_H := 19
const ICON := 16
const BAR_W := 96
const BAR_H := 8
const COMPLETE_AT := 0.9
# Rows shown: every invention some of the population holds, then the next
# unadopted ones in tree order, up to this many — the panel sits above the
# legend and must not run into it however long the catalog grows.
const MAX_ROWS := 12
const NEXT_UNADOPTED := 3
const FILL := Color(0.42, 0.80, 0.36)
const FILL_DONE := Color(0.30, 0.88, 0.70)
const BAR_BG := Color(0.06, 0.09, 0.11)

# As a page of the codex book (codex_panel.gd) the panel drops its own frame
# and title and leaves visibility to the book's tabs.
var embedded: bool = false

@onready var sim = HudCommon.find_sim(self)

var _catalog: Array = []  # [{key, name, era, bit}] in tree order
var _fractions: PackedFloat32Array = PackedFloat32Array()
var _shown: PackedInt32Array = PackedInt32Array()
var _frame: int = 0
var _rows: Control
var _font: Font


func _ready() -> void:
	_font = PixelFont.build()
	var box := VBoxContainer.new()
	add_child(box)
	if embedded:
		add_theme_stylebox_override("panel", StyleBoxEmpty.new())
	else:
		var title := Label.new()
		title.text = "RESEARCH"
		title.add_theme_font_size_override("font_size", 13)
		title.add_theme_color_override("font_color", UiTheme.ACCENT)
		box.add_child(title)
	_rows = Control.new()
	_rows.draw.connect(_draw_rows)
	box.add_child(_rows)
	_load_catalog()
	_rows.custom_minimum_size = Vector2(240, ROW_H * mini(_catalog.size(), MAX_ROWS))


func _load_catalog() -> void:
	_catalog.clear()
	var i := 0
	for inv in sim.invention_catalog():
		(
			_catalog
			. append(
				{
					"key": String(inv["key"]),
					"name": String(inv.get("name", inv["key"])),
					"era": int(inv["era"]),
					"bit": i,
				}
			)
		)
		i += 1
	_fractions.resize(_catalog.size())
	_fractions.fill(0.0)


# Share of alive agents holding each invention bit, from the packed masks.
static func adoption_fractions(masks: PackedInt32Array, count: int) -> PackedFloat32Array:
	var out := PackedFloat32Array()
	out.resize(count)
	out.fill(0.0)
	if masks.is_empty():
		return out
	var held := PackedInt32Array()
	held.resize(count)
	held.fill(0)
	for m in masks:
		for b in count:
			if m & (1 << b):
				held[b] += 1
	for b in count:
		out[b] = float(held[b]) / float(masks.size())
	return out


func _process(_delta: float) -> void:
	if not bool(sim.inventions_enabled()):
		visible = false
		return
	if not embedded:
		visible = true
	_frame += 1
	if _frame % REFRESH_EVERY != 7:
		return
	_fractions = adoption_fractions(sim.alive_invention_masks(), _catalog.size())
	_shown = visible_rows(_fractions, MAX_ROWS, NEXT_UNADOPTED)
	_rows.custom_minimum_size = Vector2(240, ROW_H * _shown.size())
	_rows.queue_redraw()


# Catalog indices to draw: adopted (fraction > 0) in tree order, then the
# first `next_unadopted` not-yet-held ones, capped at `max_rows`.
static func visible_rows(
	fractions: PackedFloat32Array, max_rows: int, next_unadopted: int
) -> PackedInt32Array:
	var out := PackedInt32Array()
	for i in fractions.size():
		if fractions[i] > 0.0 and out.size() < max_rows:
			out.append(i)
	var pending := 0
	for i in fractions.size():
		if fractions[i] <= 0.0 and pending < next_unadopted and out.size() < max_rows:
			out.append(i)
			pending += 1
	out.sort()
	return out


func _draw_rows() -> void:
	var y := 0
	for i in _shown:
		var inv: Dictionary = _catalog[i]
		var frac: float = _fractions[i] if i < _fractions.size() else 0.0
		var tex: Texture2D = HudIcons.invention_texture(inv["key"])
		_rows.draw_texture_rect(tex, Rect2(0, y + 3, ICON, ICON), false)
		var name: String = inv["name"].capitalize()
		var col: Color = UiTheme.TEXT if frac > 0.0 else UiTheme.TEXT_DIM
		_rows.draw_string(
			_font, Vector2(ICON + 6, y + 15), name, HORIZONTAL_ALIGNMENT_LEFT, 110, 12, col
		)
		var bx := ICON + 6 + 112
		_rows.draw_rect(Rect2(bx, y + 7, BAR_W, BAR_H), BAR_BG)
		if frac > 0.0:
			var fill: Color = FILL_DONE if frac >= COMPLETE_AT else FILL
			_rows.draw_rect(Rect2(bx, y + 7, BAR_W * clampf(frac, 0.0, 1.0), BAR_H), fill)
		_rows.draw_rect(Rect2(bx, y + 7, BAR_W, BAR_H), UiTheme.ACCENT_DIM, false, 1.0)
		var pct: String = "✓" if frac >= COMPLETE_AT else "%d%%" % int(round(frac * 100.0))
		_rows.draw_string(
			_font, Vector2(bx + BAR_W + 6, y + 15), pct, HORIZONTAL_ALIGNMENT_LEFT, -1, 11, col
		)
		y += ROW_H
