extends SceneTree
# Headless unit test for territory_layer.gd's pure static helpers:
# torus_copies(), the 3x3 world-shifted copies of a territory centre that
# let one ring draw across the torus seam at any camera position; and
# select_sites(), the largest-N cap that keeps the overlay legible once
# speciation has produced more territories than can be drawn without
# turning into a muddy blob. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_territory_layer.gd
# Exits 0 on success, 1 on the first failed assertion.

const TerritoryLayer = preload("res://scripts/territory_layer.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


# A minimal species_territories()-shaped site: only species_id/radius matter
# to select_sites, but the real bridge dict carries pos/locomotion/color too.
func _site(species_id: int, radius: float) -> Dictionary:
	return {
		"species_id": species_id,
		"pos": Vector2.ZERO,
		"radius": radius,
		"locomotion": 0,
		"color": Color.WHITE
	}


func _check_torus_copies() -> void:
	var c := Vector2(10.0, 1000.0)
	var copies: PackedVector2Array = TerritoryLayer.torus_copies(c, 1024.0)
	_check(copies.size() == 9, "nine copies, got %d" % copies.size())
	_check(copies.has(c), "the centre itself is a copy")
	_check(copies.has(c + Vector2(1024.0, -1024.0)), "diagonal shift present")
	_check(copies.has(c + Vector2(-1024.0, 0.0)), "west shift present")


func _check_select_sites() -> void:
	# More sites than the cap: exactly MAX_RINGS returned, largest radius first.
	var many: Array = []
	for i in 8:
		many.append(_site(i, float(i + 1)))  # radius 1..8, species_id 0..7
	var capped: Array = TerritoryLayer.select_sites(many, TerritoryLayer.MAX_RINGS)
	_check(capped.size() == TerritoryLayer.MAX_RINGS, "capped to MAX_RINGS, got %d" % capped.size())
	_check(capped[0]["species_id"] == 7, "largest radius (species 7) sorts first")
	_check(capped[1]["species_id"] == 6, "second-largest radius sorts second")
	for i in capped.size() - 1:
		_check(capped[i]["radius"] >= capped[i + 1]["radius"], "selection stays sorted descending")

	# Equal radii break the tie by species_id ascending, deterministically.
	var tied: Array = [_site(5, 10.0), _site(2, 10.0), _site(9, 10.0)]
	var tied_sorted: Array = TerritoryLayer.select_sites(tied, 3)
	_check(
		(
			tied_sorted[0]["species_id"] == 2
			and tied_sorted[1]["species_id"] == 5
			and tied_sorted[2]["species_id"] == 9
		),
		"equal-radius ties break by species_id ascending"
	)

	# Fewer sites than the cap: every site is returned, none dropped.
	var few: Array = [_site(1, 5.0), _site(2, 9.0)]
	var few_sorted: Array = TerritoryLayer.select_sites(few, TerritoryLayer.MAX_RINGS)
	_check(few_sorted.size() == 2, "fewer sites than the cap returns all of them")


func _init() -> void:
	_check_torus_copies()
	_check_select_sites()
	if _failed:
		quit(1)
		return
	print("test_territory_layer: all passed")
	quit(0)
