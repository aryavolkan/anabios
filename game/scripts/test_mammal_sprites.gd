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


# Opaque-pixel count inside one hero (24x24) grid cell of a packed atlas.
func _cell_opaque(img: Image, cell: int) -> int:
	var cx := (cell % A.ATLAS_COLS) * A.HERO_PX
	var cy := int(cell / float(A.ATLAS_COLS)) * A.HERO_PX
	var n := 0
	for y in A.HERO_PX:
		for x in A.HERO_PX:
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
			img.get_width() == A.HERO_ATLAS_PX and img.get_height() == A.HERO_ATLAS_PX,
			"species %d atlas is the shared square hero grid" % sp
		)
		for base in pairs:
			for cell in [base, base + 1]:
				var n := _cell_opaque(img, cell)
				_check(n >= 24, "species %d weapon cell %d has art (%d px)" % [sp, cell, n])
	var quad: Image = M.bucket_atlas(M.SKIN_COUNT).get_image()
	for cell in range(16, 22):
		_check(_cell_opaque(quad, cell) == 0, "quad weapon cell %d stays empty" % cell)


# Every quadruped archetype's atlas is the shared 192x192 hero grid, each of its
# 14 authored pose cells carries opaque art, and its fallen-ghost texture
# differs from the idle (frame 0) pose — regression guard for a rig whose
# pose list is accidentally empty or whose fallen build forgets to rotate.
# Covers the original six quads and the four module-keyed archetypes alike.
func _check_quad_archetypes() -> void:
	for arch in M.QUAD_ORDER:
		var b: int = M.bucket_of(arch, 0)
		var img: Image = M.bucket_atlas(b).get_image()
		_check(
			img.get_width() == A.HERO_ATLAS_PX and img.get_height() == A.HERO_ATLAS_PX,
			"%s atlas is the shared square hero grid" % M.NAMES[arch]
		)
		for cell in range(14):
			var n := _cell_opaque(img, cell)
			_check(n > 0, "%s pose %d has opaque pixels" % [M.NAMES[arch], cell])
		var idle := img.get_region(Rect2i(0, 0, A.HERO_PX, A.HERO_PX))
		var fallen: Image = M.bucket_fallen(b).get_image()
		_check(
			fallen.get_width() == A.HERO_PX and fallen.get_height() == A.HERO_PX,
			"%s fallen texture is one cell" % M.NAMES[arch]
		)
		_check(
			fallen.get_data() != idle.get_data(), "%s fallen pose differs from idle" % M.NAMES[arch]
		)


# Hero atlas: block scaling keeps the cell filled and adjacency intact, the
# head anchor is the topmost-rightmost block, the shading pass lightens the
# crown, and the deer wears antlers above its head in the field atlas.
func _check_hero_art() -> void:
	var full: Array = A.scale_blocks([[0, 0, 16, 16, "c"]], 16, 20)
	_check(full == [[0, 0, 20, 20, "c"]], "a full cell scales to the hero figure box")
	var pair: Array = A.scale_blocks([[0, 0, 8, 16, "c"], [8, 0, 8, 16, "u"]], 16, 20)
	_check(pair[0][2] + pair[1][2] == 20 and pair[1][0] == 10, "adjacent blocks stay adjacent")
	var deer_top: Image = M.portrait(M.DEER).get_image()
	var headroom_used := false
	for x in A.HERO_PX:
		for y in A.HERO_HEADROOM:
			if deer_top.get_pixel(x, y).a > 0.5:
				headroom_used = true
	_check(headroom_used, "deer antlers rise into the headroom rows")
	_check(A.scale_blocks([[3, 3, 1, 1]], 16, 24)[0][2] >= 1, "no block vanishes")
	var head: Rect2i = A.head_block([[4, 6, 8, 5], [11, 1, 3, 2], [3, 1, 2, 2]])
	_check(head == Rect2i(11, 1, 3, 2), "head is the topmost, rightmost block")
	var deer: Image = M.portrait(M.DEER).get_image()
	_check(deer.get_width() == A.HERO_PX, "portraits are hero cells")
	var horn := 0
	var lit := 0
	for y in A.HERO_PX:
		for x in A.HERO_PX:
			var c := deer.get_pixel(x, y)
			# Horn pixels (lit or shaded) are the only warm dark tone: the
			# outline is neutral grey and the coat ramp is neutral too.
			if c.a > 0.5 and c.r < 0.42 and c.r - c.b > 0.03:
				horn += 1
			if c.a > 0.5 and c.r > 0.70 and c.r < 0.76:
				lit += 1
	_check(horn >= 8, "deer portrait carries antler pixels (%d)" % horn)
	_check(lit >= 4, "shading lightens the crown (%d lit px)" % lit)
	var hare: Image = M.portrait(M.HARE).get_image()
	_check(hare.get_data() != deer.get_data(), "accents differ per archetype")


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

	# Module-keyed tags (default 0) layer module-family archetypes on top of
	# the diet/size table without disturbing the untagged picks above.
	_check(M.archetype_for(0.1, 0.8, false, 0) == M.HARE, "tags 0 keeps herb small = hare")
	_check(
		M.archetype_for(0.1, 0.8, false, M.TAG_ARMOR) == M.TORTOISE, "armor + herbivore = tortoise"
	)
	_check(
		M.archetype_for(0.9, 0.8, false, M.TAG_SPINES) == M.PORCUPINE,
		"spines + carnivore = porcupine"
	)
	_check(
		M.archetype_for(0.1, 2.0, false, M.TAG_STORAGE) == M.MAMMOTH,
		"large herb + storage = mammoth"
	)
	_check(
		M.archetype_for(0.1, 0.8, false, M.TAG_LOCOMOTOR2) == M.WADER,
		"small herb + 2 locomotors = wader"
	)
	_check(M.archetype_for(0.9, 2.0, true, M.TAG_ARMOR) == M.LIVESTOCK, "livestock beats any tag")

	_check_weapon_cells()
	_check_quad_archetypes()
	_check_hero_art()
	if _failed:
		quit(1)
		return
	print("test_mammal_sprites: all passed")
	quit(0)
