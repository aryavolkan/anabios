extends SceneTree
# Headless unit test for the pure archetype selector. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_mammal_sprites.gd
# Exits 0 on success, 1 on the first failed assertion.

const M = preload("res://scripts/mammal_sprites.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		# quit() only REQUESTS an exit; the flag keeps a later quit(0) honest.
		_failed = true


func _init() -> void:
	# Livestock override beats everything.
	_check(M.archetype_for(0.9, 2.0, true) == M.LIVESTOCK, "livestock override")
	# Herbivore band (diet < 0.34): small -> Hare, large -> Deer.
	_check(M.archetype_for(0.1, 0.8, false) == M.HARE, "herb small = hare")
	_check(M.archetype_for(0.1, 2.0, false) == M.DEER, "herb large = deer")
	# Omnivore band (0.34..0.66): small -> Boar, large -> Primate.
	_check(M.archetype_for(0.5, 0.8, false) == M.BOAR, "omni small = boar")
	_check(M.archetype_for(0.5, 2.0, false) == M.PRIMATE, "omni large = primate")
	# Carnivore band (>= 0.66): small -> Fox, large -> Wolf.
	_check(M.archetype_for(0.9, 0.8, false) == M.FOX, "carn small = fox")
	_check(M.archetype_for(0.9, 2.0, false) == M.WOLF, "carn large = wolf")
	# Boundary: exactly SIZE_SPLIT counts as large; exactly a band edge is the
	# higher band's floor (diet == 0.34 is omnivore, diet == 0.66 is carnivore).
	_check(M.archetype_for(0.1, M.SIZE_SPLIT, false) == M.DEER, "size split = large")
	_check(M.archetype_for(M.HERB_MAX, 0.8, false) == M.BOAR, "diet 0.34 = omnivore")
	_check(M.archetype_for(M.CARN_MIN, 0.8, false) == M.FOX, "diet 0.66 = carnivore")
	if _failed:
		quit(1)
		return
	print("test_mammal_sprites: all passed")
	quit(0)
