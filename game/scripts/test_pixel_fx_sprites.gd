extends SceneTree
# Headless checks for the pixel-art event burst sprites. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_pixel_fx_sprites.gd
# Exits 0 on success, 1 on the first failed assertion.

const PixelFx = preload("res://scripts/pixel_fx_sprites.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		# quit() only REQUESTS an exit; the flag keeps a later quit(0) honest.
		_failed = true


func _init() -> void:
	_check(PixelFx.KIND_COUNT == 4, "ember, impact, discovery and smoke sprites")
	_check(PixelFx.EMBER == 0 and PixelFx.IMPACT == 1 and PixelFx.DISCOVERY == 2, "stable ids")
	for kind in PixelFx.KIND_COUNT:
		var image: Image = PixelFx.build_image(kind)
		_check(image.get_width() == 16 and image.get_height() == 16, "fx %d is 16x16" % kind)
		var opaque: int = PixelFx.opaque_pixels(image)
		_check(opaque >= 6, "fx %d silhouette is visible (%d px)" % [kind, opaque])
		# A burst particle is a small mark, not a filled tile: the cell must
		# stay transparent outside the outlined shape.
		_check(opaque <= 120, "fx %d leaves the cell mostly transparent (%d px)" % [kind, opaque])
		_check(image.get_pixel(0, 0).a == 0.0, "fx %d corner is transparent" % kind)
		_check(image.get_pixel(15, 15).a == 0.0, "fx %d far corner is transparent" % kind)
		var texture: ImageTexture = PixelFx.build(kind)
		_check(
			texture != null and texture.get_width() == 16 and texture.get_height() == 16,
			"fx %d builds a 16x16 texture" % kind
		)
		# Building twice returns the same cached texture (no per-spawn upload).
		_check(PixelFx.build(kind) == texture, "fx %d texture is cached" % kind)
	if _failed:
		quit(1)
		return
	print("test_pixel_fx_sprites: all passed")
	quit(0)
