extends SceneTree
# Headless unit test for the emote pictogram atlas + action mapping. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_emote_sprites.gd
# Exits 0 on success, 1 on the first failed assertion.

const ApeSprites = preload("res://scripts/ape_sprites.gd")
const EmoteSprites = preload("res://scripts/emote_sprites.gd")
const EmoteLayer = preload("res://scripts/emote_layer.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		# quit() only REQUESTS an exit; the flag keeps a later quit(0) honest.
		_failed = true


func _opaque_pixels(img: Image, cell: int) -> int:
	var cx := (cell % ApeSprites.ATLAS_COLS) * ApeSprites.CELL_PX
	var cy := int(cell / float(ApeSprites.ATLAS_COLS)) * ApeSprites.CELL_PX
	var count := 0
	for y in ApeSprites.CELL_PX:
		for x in ApeSprites.CELL_PX:
			if img.get_pixel(cx + x, cy + y).a > 0.5:
				count += 1
	return count


# Behavioral pass over collect()/refresh(): emotes appear for emoting agents,
# ramp their presence weight in, and fade out over the frames after the agent
# stops acting — instead of strobing with the action threshold.
func _check_lifecycle(layer) -> void:
	layer.setup()
	var mm: MultiMesh = layer._mmi.multimesh
	# Two emoting agents this frame: a sleeper and a drinker.
	layer.collect(7, Vector2(10, 10), 1.5, 5.0)
	layer.collect(9, Vector2(20, 20), 1.0, 6.0)
	layer.collect(11, Vector2(30, 30), 1.0, 2.0)  # fight: no emote
	layer.refresh(0.0, 1.0 / 60.0)
	_check(mm.visible_instance_count == 2, "two emotes drawn for two emoting agents")
	_check(layer._entries.size() == 2, "fight stays out of the emote entries")
	var w0: float = layer._entries[7][1]
	_check(w0 > 0.0 and w0 < 1.0, "presence weight ramps in, not snaps")
	# Same agents keep emoting: weight saturates at 1.
	for _i in 60:
		layer.collect(7, Vector2(10, 10), 1.5, 5.0)
		layer.collect(9, Vector2(20, 20), 1.0, 6.0)
		layer.refresh(0.0, 1.0 / 60.0)
	_check(layer._entries[7][1] == 1.0, "presence weight saturates while acting")
	# Both stop acting: entries fade for a few frames, then drop.
	layer.refresh(0.0, 1.0 / 60.0)
	_check(mm.visible_instance_count == 2, "ended emotes linger while fading")
	_check(layer._entries[7][1] < 1.0, "ended emotes lose weight")
	for _i in 60:
		layer.refresh(0.0, 1.0 / 60.0)
	_check(layer._entries.is_empty(), "faded emotes are dropped")
	_check(mm.visible_instance_count == 0, "no instances after all emotes fade")


func _init() -> void:
	var atlas := EmoteSprites.build_atlas()
	_check(atlas != null, "atlas builds")
	var img := atlas.get_image()
	_check(
		img.get_width() == ApeSprites.ATLAS_PX and img.get_height() == ApeSprites.ATLAS_PX,
		"atlas is the square 64x64 grid"
	)
	# Cell 0 (kind NONE) must stay fully transparent; every glyph cell must
	# actually contain art.
	_check(_opaque_pixels(img, EmoteSprites.NONE) == 0, "NONE cell is empty")
	for kind in range(1, EmoteSprites.KIND_COUNT):
		_check(_opaque_pixels(img, kind) > 8, "glyph cell %d has art" % kind)
	_check(EmoteSprites.KIND_COUNT <= int(EmoteSprites.KIND_SCALE), "kinds fit the channel scale")
	# Action -> glyph mapping (keys mirror main.gd's ACT_* ids).
	var layer := EmoteLayer.new()
	_check(layer.kind_for_act(5.0) == EmoteSprites.SLEEP_Z, "sleep maps to Zzz")
	_check(layer.kind_for_act(4.0) == EmoteSprites.ALERT, "flee maps to alert")
	_check(layer.kind_for_act(6.0) == EmoteSprites.DROPLET, "drink maps to droplet")
	_check(layer.kind_for_act(7.0) == EmoteSprites.HEART, "courtship maps to heart")
	_check(layer.kind_for_act(9.0) == EmoteSprites.STAR, "celebrate maps to star")
	for silent in [0.0, 1.0, 2.0, 3.0, 8.0]:
		_check(layer.kind_for_act(silent) == EmoteSprites.NONE, "act %d stays emote-free" % silent)
	_check_lifecycle(layer)
	layer.free()
	if _failed:
		quit(1)
		return
	print("test_emote_sprites: all passed")
	quit(0)
