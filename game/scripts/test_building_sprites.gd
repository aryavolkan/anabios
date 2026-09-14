extends SceneTree
# Headless checks for the building sprite module. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_building_sprites.gd
# Exits 0 on success, 1 on the first failed assertion.

const B = preload("res://scripts/building_sprites.gd")
const T = preload("res://scripts/structure_tall.gd")
const StructureSprites = preload("res://scripts/structure_sprites.gd")
const INV_KEYS := [
	"stone_tools",
	"fire",
	"farming",
	"metalworking",
	"writing",
	"medicine",
	"husbandry",
	"machinery",
	"electricity",
	"nuclear_power",
	"hafted_spears",
	"archery",
	"fortifications",
	"steel_arms",
	"pottery",
	"irrigation",
	"currency",
	"printing",
	"sanitation",
	"gunpowder",
	"wells",
	"vaccination"
]

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		# quit() only REQUESTS an exit; the flag keeps a later quit(0) honest.
		_failed = true


func _init() -> void:
	# Enum/name/count coherence.
	_check(B.KIND_COUNT == 24, "24 building kinds")
	_check(B.NAMES.size() == B.KIND_COUNT, "NAMES parallels the enum")
	# Every kind builds a 16x16 image with at least one opaque (figure) pixel.
	for k in B.KIND_COUNT:
		var img: Image = B.build_image(k)
		_check(img.get_width() == 32 and img.get_height() == 44, "%s is 32x44" % B.NAMES[k])
		var opaque := 0
		for y in img.get_height():
			for x in img.get_width():
				if img.get_pixel(x, y).a > 0.5:
					opaque += 1
		_check(opaque >= 8, "%s has a visible figure" % B.NAMES[k])
	# The invention map covers all ten keys and points at real kinds.
	for key in INV_KEYS:
		var kind: int = B.building_for_invention(key)
		_check(kind >= 0 and kind < B.KIND_COUNT, "invention '%s' maps to a building" % key)
	# Unknown keys return -1 (no building).
	_check(B.building_for_invention("not_a_thing") == -1, "unknown key -> -1")

	# --- signature_kinds: highest era first, skips unbuildable, honours want ---
	var era_of := {"fire": 1, "writing": 3, "farming": 2}
	var adopted := PackedStringArray(["fire", "writing", "farming"])
	var sig: PackedInt32Array = B.signature_kinds(adopted, era_of, 2)
	_check(sig.size() == 2, "want=2 returns two kinds")
	_check(sig[0] == B.WRITING, "highest era (writing) first")
	_check(sig[1] == B.FARMING, "second highest (farming) next")
	# want beyond held count returns only what's held.
	var one: PackedInt32Array = B.signature_kinds(PackedStringArray(["fire"]), era_of, 2)
	_check(one.size() == 1, "want clamps to held count")
	_check(one[0] == B.FIRE, "single held invention -> its building")
	# empty adoption -> no landmarks.
	_check(
		B.signature_kinds(PackedStringArray(), era_of, 2).size() == 0,
		"no inventions -> no landmark"
	)

	# --- trade_kind: density + members thresholds ---
	_check(B.trade_kind(0.1, 10) == -1, "low density -> no trade building")
	_check(B.trade_kind(0.5, 10) == B.MARKET, "high density, small -> market")
	_check(B.trade_kind(0.5, 60) == B.WAREHOUSE, "high density, large -> warehouse")

	# --- market_cell: row-major index, clamped ---
	_check(B.market_cell(Vector2(0, 0), 100.0, 10) == 0, "origin -> cell 0")
	_check(B.market_cell(Vector2(55, 25), 100.0, 10) == 2 * 10 + 5, "mid maps row-major")
	_check(
		B.market_cell(Vector2(999, 999), 100.0, 10) == 9 * 10 + 9,
		"out-of-range clamps to last cell"
	)
	_check(B.market_cell(Vector2(1, 1), 0.0, 10) == -1, "bad world_size -> -1")

	# signature_kinds skips keys with no building.
	var mixed := PackedStringArray(["fire", "not_a_building"])
	var only_fire: PackedInt32Array = B.signature_kinds(mixed, {"fire": 1}, 2)
	_check(only_fire.size() == 1, "unbuildable key is skipped")
	_check(only_fire[0] == B.FIRE, "only the buildable invention remains")

	_check(B.market_cell(Vector2(1, 1), 100.0, 0) == -1, "res<=0 -> -1")
	_check(B.trade_kind(B.MARKET_MIN - 0.01, 10) == -1, "just below MARKET_MIN -> none")
	_check(B.trade_kind(B.MARKET_MIN, 10) == B.MARKET, "exactly MARKET_MIN -> market")
	_check(
		B.trade_kind(B.MARKET_MIN, B.WAREHOUSE_MIN_MEMBERS) == B.WAREHOUSE,
		"exact thresholds -> warehouse"
	)

	# --- goods icons: index-aligned to sim Good indices, every kind builds ---
	_check(B.GOOD_COUNT == 4, "4 trade goods")
	_check(B.GOOD_NAMES.size() == B.GOOD_COUNT, "GOOD_NAMES parallels GOOD_COUNT")
	for k in B.KIND_COUNT:
		var tex: ImageTexture = B.build(k)
		_check(
			tex != null and tex.get_size() == Vector2(32, 44), "%s texture is 32x44" % B.NAMES[k]
		)
		_check(
			is_equal_approx(B.height_ratio(k), 44.0 / 32.0), "%s height ratio is 44/32" % B.NAMES[k]
		)
	for g in B.GOOD_COUNT:
		var gtex: ImageTexture = B.build_good(g)
		_check(
			gtex != null and gtex.get_width() == 16 and gtex.get_height() == 16,
			"%s good texture is 16x16" % B.GOOD_NAMES[g]
		)
		var gimg: Image = B.build_good_image(g)
		var opaque := 0
		for y in 16:
			for x in 16:
				if gimg.get_pixel(x, y).a > 0.5:
					opaque += 1
		_check(opaque >= 8, "%s good has a visible icon" % B.GOOD_NAMES[g])

	# --- animated landmarks: Fire and Metalworking flicker between two frames ---
	_check(B.is_animated(B.FIRE), "fire landmark animates")
	_check(B.is_animated(B.METALWORKING), "forge landmark animates")
	_check(not B.is_animated(B.MARKET), "market stays static")
	_check(not B.is_animated(B.WRITING), "library stays static")
	_check(B.ANIMATED_KINDS.size() == 2, "exactly two animated landmarks")
	for k in B.ANIMATED_KINDS:
		var low: Image = B.build_variant(k, 0).get_image()
		var high: Image = B.build_variant(k, 1).get_image()
		_check(low.get_size() == Vector2i(32, 44), "%s flicker frame stays 32x44" % B.NAMES[k])
		_check(high.get_size() == Vector2i(32, 44), "%s lifted frame stays 32x44" % B.NAMES[k])
		_check(low.get_data() == B.build_image(k).get_data(), "%s phase 0 is the base" % B.NAMES[k])
		_check(low.get_data() != high.get_data(), "%s flicker frames differ" % B.NAMES[k])
		_check(
			B.build_variant(k, 3).get_image().get_data() == high.get_data(),
			"%s odd phases share the lifted frame" % B.NAMES[k]
		)
		# The lift only touches a few flame pixels: the silhouette survives.
		var changed := 0
		var lit := 0
		for y in 44:
			for x in 32:
				if low.get_pixel(x, y) != high.get_pixel(x, y):
					changed += 1
				if high.get_pixel(x, y).a > 0.5:
					lit += 1
		_check(
			changed >= 1 and changed <= 16,
			"%s lift moves a few pixels (%d)" % [B.NAMES[k], changed]
		)
		_check(lit >= 8, "%s lifted frame keeps a visible figure" % B.NAMES[k])
	# The hub centrepieces are the 32 px structure art, bottom-up like the
	# icons: the market hall's awning (row 4 of the top-down art) lands in
	# the upper half of the flipped image.
	var market: Image = B.build_image(B.MARKET)
	var mid_top := 0
	for y in range(22, 44):
		for x in 32:
			if market.get_pixel(x, y).a > 0.5:
				mid_top += 1
	_check(mid_top > 0, "market hall art fills the upper half of its flipped cell")
	_check(
		B.is_hi_res(B.MARKET) and B.is_hi_res(B.WAREHOUSE) and not B.is_hi_res(B.FIRE),
		"only the hub centrepieces are hi-res"
	)
	# An invention landmark is the workshop front with its icon as the sign:
	# the icon's pixels land on the wall at SIGN_POS (pushed down by the
	# extruded band), plaster shows around the sign, and a plain wall row
	# is the one extruded (no icon pixel repeats down the band).
	var fire_icon: Image = B.icon_image(B._BLOCKS[B.FIRE])
	var fire: Image = B.topdown_image(B.FIRE)
	var lift: int = T.EXTRA_ROWS - B.WORKSHOP_TALL_ROWS + B.WORKSHOP_TALL_ROWS
	var sign_ok := true
	for y in 16:
		for x in 16:
			var ip := fire_icon.get_pixel(x, y)
			if ip.a > 0.5:
				var wp := fire.get_pixel(B.SIGN_POS.x + x, B.SIGN_POS.y + y + lift)
				sign_ok = sign_ok and wp.is_equal_approx(ip)
	_check(sign_ok, "the fire icon hangs on the workshop wall")
	var plaster := StructureSprites.PAL["h"]
	_check(fire.get_pixel(6, 20 + lift).to_html(false) == plaster, "plaster shows beside the sign")
	var band_ok := true
	for i in B.WORKSHOP_TALL_ROWS:
		for x in 32:
			var y0: int = B.WORKSHOP_EXTRUDE_ROW + (T.EXTRA_ROWS - B.WORKSHOP_TALL_ROWS)
			band_ok = band_ok and fire.get_pixel(x, y0 + i) == fire.get_pixel(x, y0 - 1)
	_check(band_ok, "the workshop band repeats the plain wall row")
	# A static kind returns its base art for every phase.
	_check(
		B.build_variant(B.MARKET, 1).get_image().get_data() == B.build_image(B.MARKET).get_data(),
		"static kinds ignore the phase"
	)

	# --- caravan cart: 16x16 texture with a visible figure ---
	var cart := B.build_cart()
	_check(
		cart != null and cart.get_width() == 16 and cart.get_height() == 16, "cart texture is 16x16"
	)
	var cart_img: Image = B.build_cart_image()
	var cart_opaque := 0
	for y in 16:
		for x in 16:
			if cart_img.get_pixel(x, y).a > 0.5:
				cart_opaque += 1
	_check(cart_opaque >= 8, "cart has a visible figure")

	if _failed:
		quit(1)
		return
	print("test_building_sprites: all passed")
	quit(0)
