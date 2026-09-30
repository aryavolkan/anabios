extends RefCounted
# Pure torus-framing math for camera_controller.gd, split out for the same
# reason as zoom_steps.gd: camera_controller.gd extends Camera2D and reads the
# GameConfig autoload, so it does not compile when preloaded from a `-s` test
# script, while this file has no such dependency and test_camera_steps.gd
# exercises it headless.

const ZoomSteps = preload("res://scripts/zoom_steps.gd")

# The opening shot frames this fraction of the world's height, so a small
# world opens on its living cluster rather than the whole map.
const BOOT_SPAN_FRAC: float = 0.35
# ...but never below 1x: under 1.0 the camera is in overview (is_overview()),
# where AgentLayer draws no bodies at all, only density dots. On a 4096 or
# 8192 world the 0.35 span alone lands on 0.5x / 0.25x and a run used to
# open on an empty-looking map.
const BOOT_ZOOM_MIN: float = 1.0


# Fold a camera position back into [0, world) on both axes. Every world layer
# draws its torus copies within +/-1 world of the origin (main.gd's wrap
# clones, the 3x3 ground tiles, settlement/hub clones) and the streamed ground
# chunks follow the camera, so folding is seamless; without it a long pan
# walked the view off the edge of those copies into empty clear colour.
static func fold_to_world(p: Vector2, world: float) -> Vector2:
	if world <= 0.0:
		return p
	return Vector2(fposmod(p.x, world), fposmod(p.y, world))


# The boot zoom for a viewport height vp_h on a world of side `world`: the
# largest table step framing BOOT_SPAN_FRAC of the world, raised to at least
# BOOT_ZOOM_MIN (itself a table step) so the bodies are drawn.
static func boot_zoom(vp_h: float, world: float) -> float:
	var span: float = maxf(world * BOOT_SPAN_FRAC, 1.0)
	var z: float = ZoomSteps.nearest_step_at_most(vp_h / span)
	return maxf(z, BOOT_ZOOM_MIN)


# Where to centre a view `window` world units across so it holds the most
# agents. The arithmetic mean of the positions is not torus-aware: two
# clusters (or one straddling the seam) average into the empty ground between
# them. Instead, bin the agents on a grid of cells a third of the window wide,
# take the 3x3 block of cells (wrapping) holding the most agents — the first
# such block in row-major order on a tie, so the pick is deterministic — and
# return the torus mean of the agents in that block, folded into [0, world).
static func densest_centre(ps: PackedVector2Array, world: float, window: float) -> Vector2:
	if world <= 0.0:
		return Vector2.ZERO
	if ps.is_empty():
		return Vector2(world * 0.5, world * 0.5)
	var cells: int = clampi(int(round(world * 3.0 / maxf(window, 1.0))), 1, 64)
	var cw: float = world / float(cells)
	var counts := PackedInt32Array()
	counts.resize(cells * cells)
	for p in ps:
		var ix: int = clampi(int(fposmod(p.x, world) / cw), 0, cells - 1)
		var iy: int = clampi(int(fposmod(p.y, world) / cw), 0, cells - 1)
		counts[iy * cells + ix] += 1
	var best: int = -1
	var bx: int = 0
	var by: int = 0
	for cy in cells:
		for cx in cells:
			var s: int = 0
			for dy in range(-1, 2):
				for dx in range(-1, 2):
					s += counts[posmod(cy + dy, cells) * cells + posmod(cx + dx, cells)]
			if s > best:
				best = s
				bx = cx
				by = cy
	var c0 := Vector2((float(bx) + 0.5) * cw, (float(by) + 0.5) * cw)
	# Mean of the agents in the winning block, as offsets from its centre cell
	# taken the short way round the torus.
	var reach: float = cw * 1.5
	var acc := Vector2.ZERO
	var n: int = 0
	for p in ps:
		var d := Vector2(
			fposmod(p.x - c0.x + world * 0.5, world) - world * 0.5,
			fposmod(p.y - c0.y + world * 0.5, world) - world * 0.5
		)
		if absf(d.x) <= reach and absf(d.y) <= reach:
			acc += d
			n += 1
	if n > 0:
		c0 += acc / float(n)
	return fold_to_world(c0, world)
