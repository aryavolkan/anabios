extends Node2D
# Territory ground overlay (territory/habitat/collision layer): a translucent
# disc + ring per live species at its territory centre and radius, in the
# species colour, shown only while the [G] ground mode is "territory". Plain
# _draw() primitives — no shader, so it is Metal-safe. Draws the 3x3 torus
# copies itself (torus_copies) instead of relying on main.gd's wrap clones.

const RING_WIDTH := 2.0
const FILL_ALPHA := 0.08
# Territories only change every species step (200 ticks); a slow refresh is
# visually identical and keeps the bridge walk off the hot path.
const REFRESH_FRAMES := 15

var _sim
var _overlay
var _sites: Array = []
var _world: float = 0.0
var _frame: int = 0


func setup(sim, overlay) -> void:
	_sim = sim
	_overlay = overlay
	z_index = 1


# The centre plus its eight one-world shifts, so a ring near the seam (or a
# camera looking across it) still shows. Pure; unit-tested.
static func torus_copies(c: Vector2, world: float) -> PackedVector2Array:
	var out := PackedVector2Array()
	for gy in range(-1, 2):
		for gx in range(-1, 2):
			out.append(c + Vector2(gx * world, gy * world))
	return out


func _process(_delta: float) -> void:
	var active: bool = (
		_sim != null and bool(_sim.territory_active()) and _overlay.ground_is_territory()
	)
	visible = active
	if not active:
		return
	_frame += 1
	if _frame % REFRESH_FRAMES == 1 or _sites.is_empty():
		_sites = _sim.species_territories()
		_world = float(_sim.world_size())
		queue_redraw()


func _draw() -> void:
	for s in _sites:
		var col: Color = s["color"]
		var r: float = s["radius"]
		var fill := Color(col.r, col.g, col.b, FILL_ALPHA)
		for c in torus_copies(s["pos"], _world):
			draw_circle(c, r, fill)
			draw_arc(c, r, 0.0, TAU, 96, col, RING_WIDTH, true)
