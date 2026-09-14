extends SceneTree
# Headless unit test for minimap_panel.gd's pure static helpers: the
# density-grid cell layout and the overview-mip size clamp. Both are plain
# math with no scene/bridge dependency, so they're tested directly rather
# than via a screenshot. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_minimap_math.gd
# Exits 0 on success, 1 on the first failed assertion.

const MinimapPanel = preload("res://scripts/minimap_panel.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	# --- density_cell_rects -------------------------------------------------
	var res := 8
	var panel := Vector2(80, 80)

	var empty := PackedByteArray()
	empty.resize(res * res)  # all zero
	_check(
		MinimapPanel.density_cell_rects(empty, res, panel).is_empty(),
		"all-zero counts produce no rects"
	)

	var counts := PackedByteArray()
	counts.resize(res * res)
	var col := 3
	var row := 5
	counts[row * res + col] = 8
	var rects := MinimapPanel.density_cell_rects(counts, res, panel)
	_check(rects.size() == 1, "one non-zero cell produces exactly one rect")
	if rects.size() == 1:
		var entry: Array = rects[0]
		var rect: Rect2 = entry[0]
		var alpha: float = entry[1]
		var cell := Vector2(panel.x / res, panel.y / res)  # (10, 10)
		var expected := Rect2(Vector2(col * cell.x, row * cell.y), cell)
		_check(
			rect.position.is_equal_approx(expected.position),
			(
				"rect position matches (col %d, row %d) -> %s, got %s"
				% [col, row, expected.position, rect.position]
			)
		)
		_check(
			rect.size.is_equal_approx(expected.size),
			"rect size matches the cell size %s, got %s" % [expected.size, rect.size]
		)
		_check(is_equal_approx(alpha, 1.0), "count 8 (>= 4) saturates alpha to 1.0, got %f" % alpha)

	var one := PackedByteArray()
	one.resize(res * res)
	one[0] = 1
	var one_rects := MinimapPanel.density_cell_rects(one, res, panel)
	_check(one_rects.size() == 1, "a single count of 1 still produces one rect")
	if one_rects.size() == 1:
		var alpha1: float = one_rects[0][1]
		_check(
			is_equal_approx(alpha1, 0.35),
			"count 1 clamps alpha up to the 0.35 floor, got %f" % alpha1
		)

	# res/panel mismatch with the counts array (wrong size): treated as empty
	# rather than indexing out of range.
	var short := PackedByteArray()
	short.resize(res * res - 1)
	_check(
		MinimapPanel.density_cell_rects(short, res, panel).is_empty(),
		"a too-short counts array yields no rects instead of erroring"
	)

	# --- overview_size_for ---------------------------------------------------
	_check(
		MinimapPanel.overview_size_for(512) == 256,
		"a well-resolved biome (res 512) gives the full 256px overview"
	)
	_check(MinimapPanel.overview_size_for(256) == 256, "res exactly at the overview size stays 256")
	_check(
		MinimapPanel.overview_size_for(64) == 64,
		(
			"overview size clamps down to res when res < 256, got %d"
			% MinimapPanel.overview_size_for(64)
		)
	)
	_check(MinimapPanel.overview_size_for(0) == 256, "res 0 (no world yet) falls back to 256")

	# --- overview alpha is elevation; the panel forces it opaque ---
	var packed := PackedByteArray([1, 2, 3, 40, 5, 6, 7, 0])
	var op: PackedByteArray = MinimapPanel.opaque_rgba(packed)
	_check(op[3] == 255 and op[7] == 255, "alpha forced opaque")
	_check(op[0] == 1 and op[6] == 7, "colour bytes untouched")
	_check(packed[3] == 40, "source bytes untouched")

	if _failed:
		quit(1)
		return
	print("test_minimap_math: all passed")
	quit(0)
