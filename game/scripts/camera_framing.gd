extends RefCounted
# Pure torus-framing math for camera_controller.gd, split out for the same
# reason as zoom_steps.gd: camera_controller.gd extends Camera2D and reads the
# GameConfig autoload, so it does not compile when preloaded from a `-s` test
# script, while this file has no such dependency and test_camera_steps.gd
# exercises it headless.


# Fold a camera position back into [0, world) on both axes. Every world layer
# draws its torus copies within +/-1 world of the origin (main.gd's wrap
# clones, the 3x3 ground tiles, settlement/hub clones) and the streamed ground
# chunks follow the camera, so folding is seamless; without it a long pan
# walked the view off the edge of those copies into empty clear colour.
static func fold_to_world(p: Vector2, world: float) -> Vector2:
	if world <= 0.0:
		return p
	return Vector2(fposmod(p.x, world), fposmod(p.y, world))
