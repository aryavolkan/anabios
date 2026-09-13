extends PanelContainer
# Ecosystem meters (Phase 5 step 9 of docs/superpowers/specs/2026-09-12-
# pixel-world-at-scale-design.md): the reference boards' bottom-right
# "Ecosystem Health" and "Biodiversity" bars. Health is the live population
# against the run's peak so far (a crash reads as a drop, recovery as a
# climb); biodiversity is Simpson's index over the live species counts (the
# chance two random agents belong to different species). The panel shares
# its corner with the unit card and yields to it while an agent is pinned.

const HudIcons = preload("res://scripts/hud_icons.gd")
const PixelFont = preload("res://scripts/pixel_font.gd")
const UiTheme = preload("res://scripts/ui_theme.gd")

const REFRESH_EVERY := 30
const BAR_W := 150
const BAR_H := 8
const ROW_H := 30
const HEALTH_FILL := Color(0.42, 0.80, 0.36)
const DIVERSITY_FILL := Color(0.35, 0.62, 0.95)
const BAR_BG := Color(0.06, 0.09, 0.11)

@onready var sim = get_node("../../Simulation")
@onready var card: Control = get_node("../Inspector")

var _frame: int = 0
var _health: float = 0.0
var _diversity: float = 0.0
var _peak_live: int = 0
var _body: Control
var _font: Font


func _ready() -> void:
	_font = PixelFont.build()
	_body = Control.new()
	_body.custom_minimum_size = Vector2(200, ROW_H * 2)
	_body.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_body.draw.connect(_draw_meters)
	add_child(_body)


# Live population as a share of the highest count seen this run.
static func health_of(live: int, peak: int) -> float:
	if peak <= 0:
		return 0.0
	return clampf(float(live) / float(peak), 0.0, 1.0)


func _process(_delta: float) -> void:
	visible = not card.visible
	if not visible:
		return
	_frame += 1
	if _frame % REFRESH_EVERY != 0:
		return
	var live: int = int(sim.alive_count())
	if live > _peak_live or int(sim.tick()) == 0:
		_peak_live = live
	_health = health_of(live, _peak_live)
	var counts := PackedInt32Array()
	for st in sim.species_stats():
		counts.append(int(st["count"]))
	_diversity = simpson(counts)
	_body.queue_redraw()


# Simpson's diversity 1 - sum(p_i^2) over species shares: 0 for a single
# species, approaching 1 as many species share the population evenly.
static func simpson(counts: PackedInt32Array) -> float:
	var total := 0
	for c in counts:
		total += c
	if total <= 0:
		return 0.0
	var sum_sq := 0.0
	for c in counts:
		var p := float(c) / float(total)
		sum_sq += p * p
	return clampf(1.0 - sum_sq, 0.0, 1.0)


func _row(y: int, icon: int, label: String, frac: float, fill: Color) -> void:
	_body.draw_texture_rect(HudIcons.kind_texture(icon), Rect2(0, y + 2, 16, 16), false)
	_body.draw_string(
		_font, Vector2(24, y + 12), label, HORIZONTAL_ALIGNMENT_LEFT, -1, 12, UiTheme.TEXT
	)
	_body.draw_rect(Rect2(24, y + 16, BAR_W, BAR_H), BAR_BG)
	_body.draw_rect(Rect2(24, y + 16, BAR_W * frac, BAR_H), fill)
	_body.draw_rect(Rect2(24, y + 16, BAR_W, BAR_H), UiTheme.ACCENT_DIM, false, 1.0)


func _draw_meters() -> void:
	_row(0, HudIcons.LEAF, "Ecosystem Health", _health, HEALTH_FILL)
	_row(ROW_H, HudIcons.PAW, "Biodiversity", _diversity, DIVERSITY_FILL)
