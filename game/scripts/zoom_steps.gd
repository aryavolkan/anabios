extends RefCounted
# Pure zoom-step table math for the camera's stepped zoom (D4, pixel-perfect
# pipeline, docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md
# §4). Split out of camera_controller.gd so it can be unit-tested headless:
# camera_controller.gd extends Camera2D and reads the GameConfig autoload,
# which (like other autoload-touching scripts — see test_event_fx.gd's
# StubCam note) does not compile when preloaded from a `-s` test script. This
# script has neither problem, so test_camera_steps.gd preloads it directly.
# camera_controller.gd re-exports ZOOM_STEPS and thin static wrappers around
# next_step/nearest_step_at_most so callers keep using CameraController.* .

# Steps >= 1.0 are integer texel multiples (the D4 contract — 1 texel = 0.5
# world unit at 1x, so 2x/3x/4x/6x/8x are exact integer upscales); steps < 1.0
# are the overview range for worlds wider than the viewport can frame at 1x.
const ZOOM_STEPS: PackedFloat32Array = [0.0625, 0.125, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0]


# The step relative to z on the ZOOM_STEPS table: dir +1 is the smallest step
# strictly greater than z, dir -1 the largest strictly smaller, both clamped
# to the table ends. Applied to the CURRENT (possibly off-table) zoom on every
# wheel event, so a zoom left mid-ease by a fast scroll still lands on the
# table on the very next step.
static func next_step(z: float, dir: int) -> float:
	if dir > 0:
		for s in ZOOM_STEPS:
			if s > z:
				return s
		return ZOOM_STEPS[ZOOM_STEPS.size() - 1]
	for i in range(ZOOM_STEPS.size() - 1, -1, -1):
		if ZOOM_STEPS[i] < z:
			return ZOOM_STEPS[i]
	return ZOOM_STEPS[0]


# The largest step <= z, or the smallest step if z undercuts the whole table.
# Used to open a fresh fit (world or agent-cluster) on a crisp step rather
# than an arbitrary continuous zoom.
static func nearest_step_at_most(z: float) -> float:
	var result: float = ZOOM_STEPS[0]
	for s in ZOOM_STEPS:
		if s <= z:
			result = s
		else:
			break
	return result
