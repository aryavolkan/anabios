extends SceneTree
# Headless check for the HUD icon set: every HUD glyph and every invention
# icon is a 16x16 image with a readable mark, and unknown invention keys fall
# back to the generic era gear instead of erroring. Run with:
#   godot --headless --rendering-driver dummy --path game -s res://scripts/test_hud_icons.gd

const Icons = preload("res://scripts/hud_icons.gd")
const ResearchPanel = preload("res://scripts/research_panel.gd")
const TopBar = preload("res://scripts/top_bar.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	for kind in Icons.KIND_COUNT:
		var img: Image = Icons.kind_texture(kind).get_image()
		_check(img.get_width() == 16 and img.get_height() == 16, "kind %d icon is 16x16" % kind)
		_check(Icons.opaque_pixels(img) >= 20, "kind %d icon has a readable mark" % kind)
	for key in Icons.INVENTION_ICONS.keys():
		var img: Image = Icons.invention_texture(key).get_image()
		_check(Icons.opaque_pixels(img) >= 20, "invention icon %s has a readable mark" % key)
	_check(
		Icons.invention_texture("no_such_invention") == Icons.named_texture("era"),
		"unknown invention key falls back to the era gear"
	)
	# Every row string is exactly 16 wide and 16 tall.
	for name in Icons._ROWS.keys():
		var rows: Array = Icons._ROWS[name]
		_check(rows.size() == 16, "%s has 16 rows" % name)
		for r in rows:
			_check((r as String).length() == 16, "%s rows are 16 wide" % name)

	# --- research adoption fractions ---
	var masks := PackedInt32Array([0b0011, 0b0001, 0b0000, 0b0011])
	var f: PackedFloat32Array = ResearchPanel.adoption_fractions(masks, 3)
	_check(f.size() == 3, "one fraction per invention")
	_check(is_equal_approx(f[0], 0.75), "bit 0 held by 3 of 4")
	_check(is_equal_approx(f[1], 0.5), "bit 1 held by 2 of 4")
	_check(is_equal_approx(f[2], 0.0), "bit 2 held by none")
	_check(
		ResearchPanel.adoption_fractions(PackedInt32Array(), 2).size() == 2, "empty masks -> zeros"
	)

	# --- research visible rows: adopted first, then a few next, capped ---
	var fr := PackedFloat32Array([0.0, 1.0, 0.0, 0.4, 0.0, 0.0, 0.0])
	var rows: PackedInt32Array = ResearchPanel.visible_rows(fr, 12, 2)
	_check(rows == PackedInt32Array([0, 1, 2, 3]), "adopted plus the next two unadopted, in order")
	_check(ResearchPanel.visible_rows(fr, 3, 2).size() == 3, "row cap holds")

	# --- day counter ---
	_check(TopBar.day_of(0) == 1, "tick 0 is day 1")
	_check(TopBar.day_of(TopBar.TICKS_PER_DAY * 3 + 5) == 4, "days advance every TICKS_PER_DAY")

	if _failed:
		quit(1)
		return
	print("test_hud_icons: all passed")
	quit(0)
