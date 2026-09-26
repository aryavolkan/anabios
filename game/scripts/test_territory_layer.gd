extends SceneTree
# Headless unit test for territory_layer.gd's pure static helpers:
# torus_copies(), the 3x3 world-shifted copies of a territory centre that
# let one ring draw across the torus seam at any camera position;
# select_sites(), the largest-N cap that keeps the overlay legible once
# speciation has produced more territories than can be drawn without
# turning into a muddy blob; and ring_color(), the coat colour a ring takes
# from the archetype its species' bodies are drawn as. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_territory_layer.gd
# Exits 0 on success, 1 on the first failed assertion.

const TerritoryLayer = preload("res://scripts/territory_layer.gd")
const MammalSprites = preload("res://scripts/mammal_sprites.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


# A minimal species_territories()-shaped site: only species_id/radius (and
# members, when present) matter to select_sites. `members` < 0 leaves the
# key out — the shape a caller predating the key passes.
func _site(species_id: int, radius: float, members: int = -1) -> Dictionary:
	var s := {
		"species_id": species_id,
		"pos": Vector2.ZERO,
		"radius": radius,
		"locomotion": 0,
		"color": Color.WHITE
	}
	if members >= 0:
		s["members"] = members
	return s


# A full bridge-shaped site with the body-sprite inputs ring_color reads,
# carrying the same neutral colour-gene colour whatever its locomotion.
func _body_site(species_id: int, diet: float, size: float, locomotion: int) -> Dictionary:
	return {
		"species_id": species_id,
		"pos": Vector2.ZERO,
		"radius": 100.0,
		"locomotion": locomotion,
		"diet": diet,
		"size": size,
		"members": 10,
		"color": Color(0.2, 0.6, 0.6)
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

	# Equal radii and no members key: the tie breaks by species_id ascending,
	# deterministically.
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

	# Equal radii (the TERRITORY_R_MAX clamp) rank by members descending
	# before species_id, so the ring cap keeps the most populous species.
	var clamped: Array = [_site(1, 512.0, 300), _site(2, 512.0, 900), _site(3, 512.0, 600)]
	var by_members: Array = TerritoryLayer.select_sites(clamped, 2)
	_check(
		(
			by_members.size() == 2
			and by_members[0]["species_id"] == 2
			and by_members[1]["species_id"] == 3
		),
		"equal-radius ties rank by members descending"
	)

	# A site without a members key counts as 0: it sorts after any populated
	# tie, and keyless ties still fall back to species_id.
	var mixed: Array = [_site(4, 10.0), _site(7, 10.0, 1), _site(3, 10.0)]
	var mixed_sorted: Array = TerritoryLayer.select_sites(mixed, 3)
	_check(
		(
			mixed_sorted[0]["species_id"] == 7
			and mixed_sorted[1]["species_id"] == 3
			and mixed_sorted[2]["species_id"] == 4
		),
		"a missing members key counts as 0 and keyless ties fall back to species_id"
	)

	# Fewer sites than the cap: every site is returned, none dropped.
	var few: Array = [_site(1, 5.0), _site(2, 9.0)]
	var few_sorted: Array = TerritoryLayer.select_sites(few, TerritoryLayer.MAX_RINGS)
	_check(few_sorted.size() == 2, "fewer sites than the cap returns all of them")


func _check_ring_color() -> void:
	# The showcase shape: three herbivore grazers differing only in
	# locomotion, all with the same neutral colour-gene colour. Each ring
	# takes its bodies' coat: Air draws as the Wader, Water as the Tortoise,
	# a large land herbivore as the Deer.
	var land := _body_site(0, 0.0, 1.75, MammalSprites.LOCO_LAND)
	var water := _body_site(1, 0.0, 1.5, MammalSprites.LOCO_WATER)
	var air := _body_site(2, 0.0, 1.25, MammalSprites.LOCO_AIR)
	var land_col: Color = TerritoryLayer.ring_color(land)
	var water_col: Color = TerritoryLayer.ring_color(water)
	var air_col: Color = TerritoryLayer.ring_color(air)
	_check(
		air_col.is_equal_approx(MammalSprites.coat_hue(MammalSprites.WADER, 2)),
		"an Air site takes the Wader coat band"
	)
	_check(
		water_col.is_equal_approx(MammalSprites.coat_hue(MammalSprites.TORTOISE, 1)),
		"a Water site takes the Tortoise coat band"
	)
	_check(
		land_col.is_equal_approx(MammalSprites.coat_hue(MammalSprites.DEER, 0)),
		"a large land herbivore takes the Deer coat band"
	)
	_check(
		(
			not land_col.is_equal_approx(water_col)
			and not water_col.is_equal_approx(air_col)
			and not land_col.is_equal_approx(air_col)
		),
		"the three showcase rings come out distinct"
	)
	_check(
		not land_col.is_equal_approx(land["color"]), "a coat band overrides the colour-gene colour"
	)

	# A land omnivore of primate size resolves to the self-coloured Primate,
	# whose coat_hue is white: the ring falls back to the site's colour gene.
	var ape := _body_site(3, 0.5, 2.0, MammalSprites.LOCO_LAND)
	_check(
		(
			MammalSprites.archetype_for(0.5, 2.0, false, 0, MammalSprites.LOCO_LAND)
			== MammalSprites.PRIMATE
		),
		"fixture: diet 0.5 / size 2.0 on land is the Primate"
	)
	_check(
		TerritoryLayer.ring_color(ape).is_equal_approx(ape["color"]),
		"a coat-less archetype falls back to the site colour"
	)


func _init() -> void:
	_check_torus_copies()
	_check_select_sites()
	_check_ring_color()
	if _failed:
		quit(1)
		return
	print("test_territory_layer: all passed")
	quit(0)
