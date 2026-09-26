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
# Once speciation produces more territories than this, drawing every one of
# them piles the translucent discs into a single muddy, mostly-opaque blob
# with no legible boundaries (review finding on task-11). Capping to the
# largest MAX_RINGS keeps the overlay readable; radius grows with sqrt(member
# count), so "largest" means "most populous".
const MAX_RINGS := 6

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


# The largest max_rings sites by radius (ties broken by species_id ascending,
# so the selection is deterministic frame to frame), largest first. Pure;
# unit-tested. `sites` is species_territories()-shaped: dictionaries with at
# least "species_id" and "radius" keys.
static func select_sites(sites: Array, max_rings: int) -> Array:
	var sorted: Array = sites.duplicate()
	sorted.sort_custom(
		func(a, b):
			if a["radius"] != b["radius"]:
				return a["radius"] > b["radius"]
			return a["species_id"] < b["species_id"]
	)
	if sorted.size() > max_rings:
		sorted.resize(max_rings)
	return sorted


func _process(_delta: float) -> void:
	var active: bool = (
		_sim != null and bool(_sim.territory_active()) and _overlay.ground_is_territory()
	)
	visible = active
	if not active:
		return
	_frame += 1
	if _frame % REFRESH_FRAMES == 1 or _sites.is_empty():
		_sites = select_sites(_sim.species_territories(), MAX_RINGS)
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
