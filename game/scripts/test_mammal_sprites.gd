extends SceneTree
# Headless unit test for the pure archetype selector. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_mammal_sprites.gd
# Exits 0 on success, 1 on the first failed assertion.

const M = preload("res://scripts/mammal_sprites.gd")
const A = preload("res://scripts/ape_sprites.gd")
const H = preload("res://scripts/hero_rigs.gd")
const HM = preload("res://scripts/hominin_rigs.gd")

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
	# Hand-authored masters: every quad archetype has one, 24 rows of 24
	# valid keys, feet on the ground; the derived poses move the parts.
	for arch in M.QUAD_ORDER:
		var name: String = M.NAMES[arch]
		_check(H.has(name), "%s has a hand-authored master" % name)
		var rows: Array = H.MASTERS[name.to_upper()]
		_check(rows.size() == H.PX, "%s master has 24 rows" % name)
		for r in rows:
			_check((r as String).length() == H.PX, "%s master rows are 24 wide" % name)
			for ch in r:
				_check(ch == "." or H.KEYS.has(ch), "%s master uses known keys (%s)" % [name, ch])
		var cells: Array = H.build_cells(name)
		_check(cells.size() == H.CELL_COUNT, "%s derives 16 cells" % name)
		var stand: Image = cells[0]
		_check(H.opaque_pixels(stand) > 60, "%s stand has art" % name)
		_check(cells[4].get_data() != stand.get_data(), "%s graze differs from stand" % name)
		_check(cells[1].get_data() != cells[3].get_data(), "%s gait contacts differ" % name)
		_check(_top_row(cells[12]) > _top_row(stand), "%s sleeps lower than it stands" % name)
		_check(_bottom_row(stand) >= H.PX - 4, "%s stands on the ground rows" % name)


# Hominin masters: one per hominin, valid keys, feet on the ground, 22
# derived cells with the weapon overlays where the shader samples them and
# the arms above the head when celebrating.
func _check_hominin_masters() -> void:
	_check(HM.MASTERS.size() == A.SPECIES_COUNT, "one hominin master per species")
	for sp in A.SPECIES_COUNT:
		var name: String = A.NAMES[sp]
		var rows: Array = HM.MASTERS[sp]
		_check(rows.size() == HM.PX, "%s master has 24 rows" % name)
		for r in rows:
			_check((r as String).length() == HM.PX, "%s master rows are 24 wide" % name)
			for ch in r:
				_check(ch == "." or HM.KEYS.has(ch), "%s master uses known keys (%s)" % [name, ch])
		var cells: Array = HM.build_cells(sp)
		_check(cells.size() == A.POSE_COUNT, "%s derives every pose cell" % name)
		var stand: Image = cells[0]
		_check(_bottom_row(stand) >= HM.PX - 4, "%s stands on the ground rows" % name)
		_check(_top_row(cells[14]) < _top_row(stand), "%s raises its arms to celebrate" % name)
		for cell in [A.POSE_SPEAR, A.POSE_BOW, A.POSE_STEEL]:
			var img: Image = cells[cell]
			var blade := 0
			for y in HM.PX:
				for x in HM.PX:
					var c := img.get_pixel(x, y)
					# Flint/blade is the only light, blue-leaning grey in the
					# hominin palettes (shaded variants included).
					if c.a > 0.5 and c.r > 0.6 and c.b > c.r + 0.02:
						blade += 1
			_check(blade >= 2, "%s pose %d carries its weapon (%d px)" % [name, cell, blade])
	_check(
		HM.build_cell(0, 0).get_data() != HM.build_cell(1, 0).get_data(),
		"hominins differ in silhouette"
	)


# Topmost figure row, ignoring the auto outline (a neutral 0.34 grey).
func _top_row(img: Image) -> int:
	for y in img.get_height():
		for x in img.get_width():
			var c := img.get_pixel(x, y)
			if c.a > 0.5 and not (absf(c.r - 0.34) < 0.01 and absf(c.g - 0.34) < 0.01):
				return y
	return img.get_height()


func _bottom_row(img: Image) -> int:
	for y in range(img.get_height() - 1, -1, -1):
		for x in img.get_width():
			if img.get_pixel(x, y).a > 0.5:
				return y
	return -1


func _init() -> void:
	# --- combined atlas: every bucket's grid in one square, alpha names it ---
	var combined: ImageTexture = M.combined_atlas()
	_check(
		(
			combined.get_width() == M.COMBINED_ATLAS_PX
			and combined.get_height() == M.COMBINED_ATLAS_PX
		),
		"combined atlas is the square bucket grid"
	)
	_check(M.BUCKET_COUNT <= M.ATLAS_GRID * M.ATLAS_GRID, "every bucket fits the grid")
	var cimg: Image = combined.get_image()
	var b1: Image = M.bucket_atlas(1).get_image()
	_check(
		cimg.get_pixel(A.HERO_ATLAS_PX + 12, 12) == b1.get_pixel(12, 12),
		"bucket 1 lands in the second grid cell"
	)
	var seen_alpha: Dictionary = {}
	for b in M.BUCKET_COUNT:
		var a: float = M.bucket_alpha(b)
		_check(
			int(floor(a * M.ATLAS_GRID * M.ATLAS_GRID)) == b,
			"bucket %d round-trips through its alpha" % b
		)
		seen_alpha[a] = true
	_check(seen_alpha.size() == M.BUCKET_COUNT, "bucket alphas are distinct")
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
	_check_hominin_masters()
	if _failed:
		quit(1)
		return
	print("test_mammal_sprites: all passed")
	quit(0)
