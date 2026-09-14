extends PanelContainer
# Top bar (Phase 5 of docs/superpowers/specs/2026-09-12-pixel-world-at-scale-
# design.md): the reference boards' top-right strip — day counter, the
# pause / play / fast transport, then icon-tagged counters (population with
# its change over the last day, species, trades when resources are on, era).
# Pure readout over read-only sim queries plus the same speed writes the
# hotbar makes; the old HUD label stays in the scene (hidden) for the
# scenario-failed-to-load message.

const HudIcons = preload("res://scripts/hud_icons.gd")
const UiTheme = preload("res://scripts/ui_theme.gd")

# One "day" of the readout. Purely presentational: the sim counts ticks.
const TICKS_PER_DAY := 100
const REFRESH_EVERY := 10
# The slower species scan (highest tech era) runs on its own cadence.
const ERA_REFRESH_EVERY := 60

@onready var sim = get_node("../../Simulation")
@onready var main: Node2D = get_node("../..")

var _frame: int = 0
var _era: int = 0
var _row: HBoxContainer
var _cells: Dictionary = {}
var _transport: Dictionary = {}
# Population at the start of the current day, for the "+N" delta.
var _day_index: int = -1
var _day_start_alive: int = 0


func _ready() -> void:
	var hud: Label = get_node_or_null("../HUD")
	if hud != null:
		hud.visible = false
	_row = HBoxContainer.new()
	_row.add_theme_constant_override("separation", 14)
	add_child(_row)
	_add_cell("day", HudIcons.SUN, "Day 0")
	_add_divider()
	_add_transport("pause", "❚❚", _on_pause)
	_add_transport("play", "▶", _on_play)
	_add_transport("fast", "▶▶", _on_fast)
	_add_divider()
	_add_cell("alive", HudIcons.PEOPLE, "0")
	_add_cell("species", HudIcons.DNA, "0")
	_add_cell("trade", HudIcons.TRADE, "0")
	_add_cell("era", HudIcons.ERA, "era 0")
	_refresh()


func _add_transport(key: String, glyph: String, handler: Callable) -> void:
	var b := Button.new()
	b.text = glyph
	b.focus_mode = Control.FOCUS_NONE
	b.add_theme_font_size_override("font_size", 11)
	b.pressed.connect(handler)
	_row.add_child(b)
	_transport[key] = b


func _on_pause() -> void:
	main.paused = not main.paused


func _on_play() -> void:
	main.paused = false
	main.ticks_per_frame = 1


# Cycle the fast speeds the hotbar offers.
func _on_fast() -> void:
	main.paused = false
	match int(main.ticks_per_frame):
		4:
			main.ticks_per_frame = 16
		16:
			main.ticks_per_frame = 64
		_:
			main.ticks_per_frame = 4


# The delta shown beside the population: change since the day began.
static func delta_text(now: int, day_start: int) -> String:
	var d := now - day_start
	if d == 0:
		return ""
	return " (%+d)" % d


func _add_divider() -> void:
	var d := Label.new()
	d.text = "|"
	d.add_theme_color_override("font_color", UiTheme.TEXT_DIM)
	_row.add_child(d)


func _add_cell(key: String, icon: int, text: String) -> void:
	var cell := HBoxContainer.new()
	cell.add_theme_constant_override("separation", 5)
	if icon >= 0:
		var rect := TextureRect.new()
		rect.texture = HudIcons.kind_texture(icon)
		rect.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		rect.stretch_mode = TextureRect.STRETCH_KEEP_CENTERED
		rect.custom_minimum_size = Vector2(16, 16)
		cell.add_child(rect)
	var lbl := Label.new()
	lbl.text = text
	lbl.add_theme_font_size_override("font_size", 14)
	lbl.add_theme_color_override("font_color", UiTheme.TEXT)
	cell.add_child(lbl)
	_row.add_child(cell)
	_cells[key] = lbl


func _process(_delta: float) -> void:
	_frame += 1
	if _frame % REFRESH_EVERY == 0:
		_refresh()
	if _frame % ERA_REFRESH_EVERY == 0:
		_refresh_era()


static func day_of(tick: int) -> int:
	return 1 + int(tick / TICKS_PER_DAY)


func _refresh() -> void:
	if sim == null:
		return
	var tick: int = int(sim.tick())
	var day: int = day_of(tick)
	(_cells["day"] as Label).text = "Day %d" % day
	var alive: int = int(sim.alive_count())
	if day != _day_index:
		_day_index = day
		_day_start_alive = alive
	(_cells["alive"] as Label).text = str(alive) + delta_text(alive, _day_start_alive)
	(_cells["species"] as Label).text = str(sim.species_stats().size())
	var paused: bool = bool(main.paused)
	var rate: int = int(main.ticks_per_frame)
	(_transport["pause"] as Button).add_theme_color_override(
		"font_color", UiTheme.ACCENT if paused else UiTheme.TEXT_DIM
	)
	(_transport["play"] as Button).add_theme_color_override(
		"font_color", UiTheme.ACCENT if not paused and rate == 1 else UiTheme.TEXT_DIM
	)
	(_transport["fast"] as Button).add_theme_color_override(
		"font_color", UiTheme.ACCENT if not paused and rate > 1 else UiTheme.TEXT_DIM
	)
	var trade_cell: Control = (_cells["trade"] as Label).get_parent()
	trade_cell.visible = bool(sim.resources_active())
	(_cells["trade"] as Label).text = str(int(sim.total_trades()))
	(_cells["era"] as Label).text = "era %d" % _era


func _refresh_era() -> void:
	if sim == null or not bool(sim.inventions_enabled()):
		_era = 0
		return
	var best := 0
	for st in sim.species_stats():
		best = maxi(best, int(st["tech_era"]))
	_era = best
