extends SceneTree
# Headless unit test for the camera's stepped-zoom table (D4, pixel-perfect
# pipeline). Exercises zoom_steps.gd's two pure static helpers, `next_step`
# and `nearest_step_at_most`, directly — no scene tree or Camera2D instance
# required. camera_controller.gd re-exports these same functions as
# CameraController.next_step/nearest_step_at_most, but camera_controller.gd
# itself can't be preloaded from a `-s` script (it reads the GameConfig
# autoload; see test_event_fx.gd's StubCam note), which is why this test
# targets zoom_steps.gd, the dependency-free module holding the real logic.
# Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_camera_steps.gd
# Exits 0 on success, 1 on the first failed assertion.

const ZoomSteps = preload("res://scripts/zoom_steps.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	# --- next_step: up/down from an on-table value --------------------------
	_check(is_equal_approx(ZoomSteps.next_step(1.0, 1), 2.0), "1.0 up lands on the next step, 2.0")
	_check(
		is_equal_approx(ZoomSteps.next_step(1.0, -1), 0.5),
		"1.0 down lands on the previous step, 0.5"
	)

	# --- next_step: an off-table value snaps onto the table -----------------
	_check(
		is_equal_approx(ZoomSteps.next_step(1.5, 1), 2.0),
		"1.5 up snaps to the smallest step above it, 2.0"
	)
	_check(
		is_equal_approx(ZoomSteps.next_step(1.5, -1), 1.0),
		"1.5 down snaps to the largest step below it, 1.0"
	)

	# --- next_step: clamps at the table ends ---------------------------------
	_check(
		is_equal_approx(ZoomSteps.next_step(0.0625, -1), 0.0625),
		"the bottom step clamps going further down"
	)
	_check(is_equal_approx(ZoomSteps.next_step(8.0, 1), 8.0), "8.0 up stays at 8.0 (top clamps)")

	# --- nearest_step_at_most --------------------------------------------------
	_check(
		is_equal_approx(ZoomSteps.nearest_step_at_most(0.3), 0.25),
		"0.3 rounds down to the largest step at most 0.3, 0.25"
	)
	_check(is_equal_approx(ZoomSteps.nearest_step_at_most(7.9), 6.0), "7.9 rounds down to 6.0")
	_check(
		is_equal_approx(ZoomSteps.nearest_step_at_most(0.01), 0.0625),
		"a value under the whole table falls back to the smallest step, 0.0625"
	)
	_check(
		is_equal_approx(ZoomSteps.nearest_step_at_most(8.0), 8.0),
		"an exact top-of-table value returns itself"
	)

	if _failed:
		quit(1)
		return
	print("test_camera_steps: all passed")
	quit(0)
