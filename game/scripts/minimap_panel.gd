extends Control

# Whole-world overview in the HUD corner: draws the bridge's area-averaged
# overview mip (never the active [G] ground overlay — see biome_overview()'s
# docs), a density grid standing in for per-agent dots, the current camera
# viewport as a rectangle, a compass rose, and recenters the camera on
# click/drag. Pure viewer — no sim state touched.

@onready var sim = get_node("../../Simulation")
@onready var cam: Camera2D = get_node("../../Camera2D")

const BORDER := Color(0.8, 0.85, 0.9, 0.5)
const VIEWRECT := Color(1.0, 1.0, 1.0, 0.9)
const AGENT_DOT := Color(1.0, 0.75, 0.3)
# Infected agents' dots, the codex's EpidemicOutbreak chartreuse: an epidemic's
# location and spread read at the world scale (dots self-clear as agents
# recover, since SIS intensity decays to exactly 0).
const INFECTED_DOT := Color(0.75, 0.95, 0.4, 0.9)

# Overview mip: rebuilt from sim.biome_overview() on a slow cadence rather
# than every frame, since nothing needs whole-world detail at panel size and
# the bridge call walks the full biome grid.
const OVERVIEW_PX := 256
const OVERVIEW_REFRESH_SEC := 4.0

# Density grid standing in for one draw_rect per agent: a res*res saturating
# count grid from the bridge, one filled cell per non-empty bucket, alpha
# brightening with occupancy. Bounded work regardless of population size.
const DENSITY_RES := 128

const COMPASS_MARGIN := 6.0
const COMPASS_R := 14.0

var _overview_tex: ImageTexture
var _overview_size: int = 0
var _overview_timer: float = 0.0
var _last_res: int = -1
var _last_world: float = -1.0

var _font: Font


func _ready() -> void:
	# STOP so clicks on the minimap are consumed by _gui_input and never fall
	# through to the world agent-pick handler in main.gd:_unhandled_input.
	mouse_filter = Control.MOUSE_FILTER_STOP
	_font = get_theme_default_font()
	if _font == null:
		_font = ThemeDB.fallback_font


func _process(dt: float) -> void:
	_maybe_refresh_overview(dt)
	queue_redraw()  # viewport rect tracks the camera each frame; the panel is tiny


# Rebuilds the overview texture on the slow cadence, or immediately when the
# world's resolution/size changes (a new scenario/world) or on the very first
# frame (both trip the `_overview_tex == null` / mismatch checks below).
func _maybe_refresh_overview(dt: float) -> void:
	_overview_timer -= dt
	var res: int = sim.biome_resolution()
	var world: float = float(sim.world_size())
	if (
		_overview_tex != null
		and _overview_timer > 0.0
		and res == _last_res
		and world == _last_world
	):
		return
	_last_res = res
	_last_world = world
	_overview_timer = OVERVIEW_REFRESH_SEC
	var sz := overview_size_for(res)
	var bytes: PackedByteArray = sim.biome_overview(sz)
	if bytes.size() != sz * sz * 4:
		return  # no world loaded yet
	var img := Image.create_from_data(sz, sz, false, Image.FORMAT_RGBA8, bytes)
	if _overview_tex != null and _overview_size == sz:
		_overview_tex.update(img)
	else:
		_overview_tex = ImageTexture.create_from_image(img)
		_overview_size = sz


# The overview texture's side length: OVERVIEW_PX, clamped down to `res` when
# the source biome grid is coarser than that (asking for more pixels than the
# source has is meaningless — the bridge would clamp it anyway).
static func overview_size_for(res: int) -> int:
	if res <= 0:
		return OVERVIEW_PX
	return mini(OVERVIEW_PX, res)


# One [Rect2, alpha] entry per non-zero density cell, in panel-pixel space.
# `counts` is `res*res` row-major saturating counts (row = y, col = x, as
# `agent_density()` returns); `panel` is the panel's drawable size.
static func density_cell_rects(counts: PackedByteArray, res: int, panel: Vector2) -> Array:
	var out: Array = []
	if res <= 0 or counts.size() < res * res:
		return out
	var cell := Vector2(panel.x / res, panel.y / res)
	for row in res:
		for col in res:
			var count: int = counts[row * res + col]
			if count <= 0:
				continue
			var alpha: float = clampf(count / 4.0, 0.35, 1.0)
			out.append([Rect2(Vector2(col * cell.x, row * cell.y), cell), alpha])
	return out


func _draw() -> void:
	var world: float = float(sim.world_size())
	if world <= 0.0:
		return
	var ms: Vector2 = size
	if _overview_tex != null:
		draw_texture_rect(_overview_tex, Rect2(Vector2.ZERO, ms), false)
	_draw_density(ms)
	var diseased: bool = sim.disease_active()
	if diseased:
		_draw_infected(world, ms)
	var vp: Vector2 = get_viewport_rect().size
	var view_world := Vector2(vp.x / cam.zoom.x, vp.y / cam.zoom.y)
	var px_per_unit := ms / world
	var center := Vector2(fposmod(cam.position.x, world), fposmod(cam.position.y, world))
	_draw_view_rect(center * px_per_unit, (view_world * px_per_unit).min(ms), ms)
	_draw_compass(ms)
	# Panel border.
	draw_rect(Rect2(Vector2.ZERO, ms), BORDER, false, 1.0)


# Density dots: brighter cells read as denser clusters, without a per-agent
# draw_rect. ≤ DENSITY_RES² draws, independent of population size.
func _draw_density(ms: Vector2) -> void:
	var res: int = mini(DENSITY_RES, maxi(int(ms.x), 1))
	var counts: PackedByteArray = sim.agent_density(res)
	for entry in density_cell_rects(counts, res, ms):
		var rect: Rect2 = entry[0]
		var alpha: float = entry[1]
		draw_rect(rect, Color(AGENT_DOT.r, AGENT_DOT.g, AGENT_DOT.b, alpha))


# The epidemic cue: infected agents draw individually in the outbreak
# chartreuse over the density grid, since disease_active() gates this to a
# small subset of the (already small) live population — bounded per-agent work.
func _draw_infected(world: float, ms: Vector2) -> void:
	var positions: PackedVector2Array = sim.alive_positions()
	var infection: PackedFloat32Array = sim.alive_infection()
	var px_per_unit := ms / world
	for i in positions.size():
		if infection[i] <= 0.0:
			continue
		var p := positions[i]
		var mp := Vector2(fposmod(p.x, world), fposmod(p.y, world)) * px_per_unit
		draw_rect(Rect2(mp - Vector2(1, 1), Vector2(2, 2)), INFECTED_DOT)


# The camera box, clipped to the minimap. The world is a torus, so a view
# straddling a seam wraps to the far edge: draw the box and its three wrapped
# copies and intersect each with the panel. (It used to be drawn unclipped and
# spilled outside the minimap's frame onto the HUD whenever the camera sat near
# an edge or zoomed out past the world.)
func _draw_view_rect(center_px: Vector2, box: Vector2, ms: Vector2) -> void:
	var panel := Rect2(Vector2.ZERO, ms)
	var origin := Vector2(
		fposmod(center_px.x - box.x * 0.5, ms.x), fposmod(center_px.y - box.y * 0.5, ms.y)
	)
	# Zoomed out past a world edge the box already spans the whole map on that
	# axis; pin it so the wrap draws one rect around the map instead of two
	# abutting slivers with a seam line through the middle.
	if box.x >= ms.x:
		origin.x = 0.0
	if box.y >= ms.y:
		origin.y = 0.0
	for dx in [0.0, -ms.x]:
		for dy in [0.0, -ms.y]:
			var clipped := Rect2(origin + Vector2(dx, dy), box).intersection(panel)
			if clipped.size.x > 1.0 and clipped.size.y > 1.0:
				draw_rect(clipped, VIEWRECT, false, 2.0)


# N/E/S/W compass rose in the panel's top-right corner: a 4-point star of thin
# lines (cardinal spokes plus short diagonal ticks) with the letters at the
# spoke tips, so the minimap's fixed north-up orientation is explicit.
func _draw_compass(ms: Vector2) -> void:
	var center := Vector2(ms.x - COMPASS_MARGIN - COMPASS_R, COMPASS_MARGIN + COMPASS_R)
	for d in [Vector2.UP, Vector2.RIGHT, Vector2.DOWN, Vector2.LEFT]:
		draw_line(center, center + d * COMPASS_R, BORDER, 1.0)
	var diag := COMPASS_R * 0.4
	for d in [Vector2(1, 1), Vector2(1, -1), Vector2(-1, 1), Vector2(-1, -1)]:
		draw_line(center, center + d.normalized() * diag, BORDER, 1.0)
	if _font == null:
		return
	draw_string(
		_font, center + Vector2(-3, -COMPASS_R - 2), "N", HORIZONTAL_ALIGNMENT_LEFT, -1, 10, BORDER
	)
	draw_string(
		_font, center + Vector2(COMPASS_R + 2, 4), "E", HORIZONTAL_ALIGNMENT_LEFT, -1, 10, BORDER
	)
	draw_string(
		_font, center + Vector2(-3, COMPASS_R + 10), "S", HORIZONTAL_ALIGNMENT_LEFT, -1, 10, BORDER
	)
	draw_string(
		_font, center + Vector2(-COMPASS_R - 10, 4), "W", HORIZONTAL_ALIGNMENT_LEFT, -1, 10, BORDER
	)


func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_LEFT and event.pressed:
			_jump_to(event.position)
	elif event is InputEventMouseMotion:
		if event.button_mask & MOUSE_BUTTON_MASK_LEFT:
			_jump_to(event.position)


# Map a local minimap point to a world position and recenter the camera there.
func _jump_to(local: Vector2) -> void:
	var world: float = float(sim.world_size())
	if world <= 0.0 or size.x <= 0.0 or size.y <= 0.0:
		return
	cam.position = Vector2(local.x / size.x, local.y / size.y) * world
