extends SceneTree
# Headless unit test for territory_layer.gd's pure static helper:
# torus_copies(), the 3x3 world-shifted copies of a territory centre that
# let one ring draw across the torus seam at any camera position. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_territory_layer.gd
# Exits 0 on success, 1 on the first failed assertion.

const TerritoryLayer = preload("res://scripts/territory_layer.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	var c := Vector2(10.0, 1000.0)
	var copies: PackedVector2Array = TerritoryLayer.torus_copies(c, 1024.0)
	_check(copies.size() == 9, "nine copies, got %d" % copies.size())
	_check(copies.has(c), "the centre itself is a copy")
	_check(copies.has(c + Vector2(1024.0, -1024.0)), "diagonal shift present")
	_check(copies.has(c + Vector2(-1024.0, 0.0)), "west shift present")
	if _failed:
		quit(1)
		return
	print("test_territory_layer: all passed")
	quit(0)
