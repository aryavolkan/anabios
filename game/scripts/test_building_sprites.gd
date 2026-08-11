extends SceneTree
# Headless checks for the building sprite module. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_building_sprites.gd
# Exits 0 on success, 1 on the first failed assertion.

const B = preload("res://scripts/building_sprites.gd")
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
	"nuclear_power"
]


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		quit(1)


func _init() -> void:
	# Enum/name/count coherence.
	_check(B.KIND_COUNT == 12, "12 building kinds")
	_check(B.NAMES.size() == B.KIND_COUNT, "NAMES parallels the enum")
	# Every kind builds a 16x16 image with at least one opaque (figure) pixel.
	for k in B.KIND_COUNT:
		var img: Image = B.build_image(k)
		_check(img.get_width() == 16 and img.get_height() == 16, "%s is 16x16" % B.NAMES[k])
		var opaque := 0
		for y in 16:
			for x in 16:
				if img.get_pixel(x, y).a > 0.5:
					opaque += 1
		_check(opaque >= 8, "%s has a visible figure" % B.NAMES[k])
	# The invention map covers all ten keys and points at real kinds.
	for key in INV_KEYS:
		var kind: int = B.building_for_invention(key)
		_check(kind >= 0 and kind < B.KIND_COUNT, "invention '%s' maps to a building" % key)
	# Unknown keys return -1 (no building).
	_check(B.building_for_invention("not_a_thing") == -1, "unknown key -> -1")
	print("test_building_sprites: all passed")
	quit(0)
