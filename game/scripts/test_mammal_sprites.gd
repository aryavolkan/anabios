extends SceneTree
# Headless unit test for the pure archetype selector. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_mammal_sprites.gd
# Exits 0 on success, 1 on the first failed assertion.

const M = preload("res://scripts/mammal_sprites.gd")
const A = preload("res://scripts/ape_sprites.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		# quit() only REQUESTS an exit; the flag keeps a later quit(0) honest.
		_failed = true


# Opaque-pixel count inside one 16x16 grid cell of a packed atlas.
func _cell_opaque(img: Image, cell: int) -> int:
	var cx := (cell % A.ATLAS_COLS) * A.CELL_PX
	var cy := int(cell / float(A.ATLAS_COLS)) * A.CELL_PX
	var n := 0
	for y in A.CELL_PX:
		for x in A.CELL_PX:
			if img.get_pixel(cx + x, cy + y).a > 0.5:
				n += 1
	return n


# The spear/bow pairs (cells 16..19) are ape-only art: every hominin atlas
# must carry them, every quadruped atlas must leave them empty (inventions
# never reach quads, so the shader never samples there).
func _check_weapon_cells() -> void:
	_check(A.POSE_COUNT == 22, "field grid holds the spear/bow/steel pairs")
	var pairs := [A.POSE_SPEAR, A.POSE_BOW, A.POSE_STEEL]
	for sp in A.SPECIES_COUNT:
		var img: Image = A.build_species_atlas(sp).get_image()
		_check(
			img.get_width() == A.ATLAS_PX and img.get_height() == A.ATLAS_PX,
			"species %d atlas is the shared square grid" % sp
		)
		for base in pairs:
			for cell in [base, base + 1]:
				var n := _cell_opaque(img, cell)
				_check(n >= 24, "species %d weapon cell %d has art (%d px)" % [sp, cell, n])
	var quad: Image = M.bucket_atlas(M.SKIN_COUNT).get_image()
	for cell in range(16, 22):
		_check(_cell_opaque(quad, cell) == 0, "quad weapon cell %d stays empty" % cell)


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
	_check_weapon_cells()
	if _failed:
		quit(1)
		return
	print("test_mammal_sprites: all passed")
	quit(0)
