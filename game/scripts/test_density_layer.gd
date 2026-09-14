extends SceneTree
# Headless unit test for density_layer.gd's pure static helper:
# density_image_bytes(), the count-grid -> RGBA8 byte packer used by the
# far-zoom density layer. No scene/bridge dependency, so it's tested
# directly rather than via a screenshot. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_density_layer.gd
# Exits 0 on success, 1 on the first failed assertion.

const DensityLayer = preload("res://scripts/density_layer.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	var res := 8

	# --- all-zero counts -> fully transparent -------------------------------
	var zero := PackedByteArray()
	zero.resize(res * res)
	var zero_bytes := DensityLayer.density_image_bytes(zero, res)
	_check(zero_bytes.size() == res * res * 4, "output length is res*res*4 for all-zero input")
	var all_transparent := true
	for i in zero_bytes.size():
		if zero_bytes[i] != 0:
			all_transparent = false
			break
	_check(all_transparent, "an all-zero count grid packs to all-zero (transparent) bytes")

	# --- a single cell with count 1: amber, alpha ~= 0.52 -------------------
	var one := PackedByteArray()
	one.resize(res * res)
	var col := 2
	var row := 5
	one[row * res + col] = 1
	var one_bytes := DensityLayer.density_image_bytes(one, res)
	var o1 := (row * res + col) * 4
	# alpha = clampf(0.35 + 1/6, 0, 1) = 0.51666...; *255 = 131.75 -> rounds to 132.
	_check(one_bytes[o1] == 255, "count 1 red channel is full amber red, got %d" % one_bytes[o1])
	_check(
		one_bytes[o1 + 1] == 191, "count 1 green channel is amber green, got %d" % one_bytes[o1 + 1]
	)
	_check(
		one_bytes[o1 + 2] == 77, "count 1 blue channel is amber blue, got %d" % one_bytes[o1 + 2]
	)
	_check(
		one_bytes[o1 + 3] == 132,
		"count 1 alpha byte is exactly 132 (~=0.52), got %d" % one_bytes[o1 + 3]
	)
	# Every other cell in this grid stayed at count 0: still transparent.
	_check(one_bytes[0] == 0 and one_bytes[3] == 0, "untouched cells stay transparent")

	# --- count >= 12 reads brighter/whiter than a merely-occupied cell -----
	var hot := PackedByteArray()
	hot.resize(res * res)
	hot[0] = 12
	var warm := PackedByteArray()
	warm.resize(res * res)
	warm[0] = 3
	var hot_bytes := DensityLayer.density_image_bytes(hot, res)
	var warm_bytes := DensityLayer.density_image_bytes(warm, res)
	_check(
		hot_bytes[3] > warm_bytes[3], "count 12's alpha is higher (brighter) than count 3's alpha"
	)
	_check(
		hot_bytes[1] > warm_bytes[1] and hot_bytes[2] > warm_bytes[2],
		"count 12's green/blue channels shift toward white past count 3's plain amber"
	)

	# --- output length always matches res*res*4 -----------------------------
	var res2 := 16
	var mid := PackedByteArray()
	mid.resize(res2 * res2)
	mid[10] = 5
	_check(
		DensityLayer.density_image_bytes(mid, res2).size() == res2 * res2 * 4,
		"output length is res*res*4 for a populated grid"
	)

	# --- a wrong-length input yields an empty array, not an out-of-range read
	var short := PackedByteArray()
	short.resize(res * res - 1)
	_check(
		DensityLayer.density_image_bytes(short, res).is_empty(),
		"a too-short counts array yields an empty result instead of erroring"
	)
	var long := PackedByteArray()
	long.resize(res * res + 1)
	_check(
		DensityLayer.density_image_bytes(long, res).is_empty(),
		"a too-long counts array yields an empty result instead of erroring"
	)

	if _failed:
		quit(1)
		return
	print("test_density_layer: all passed")
	quit(0)
