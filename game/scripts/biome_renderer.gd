extends Sprite2D

@onready var sim = get_node("/root/Main/Simulation")
@onready var overlay = get_node("/root/Main/OverlayManager")

var _img: Image
var _tex: ImageTexture
var _res: int = 0

const REDRAW_EVERY := 6
var _frame: int = 0
var _last_mode: int = -999   # last (channel, or -1 biome, or -2 optimum) drawn

func _ready() -> void:
	centered = false
	position = Vector2.ZERO
	z_index = -10
	# Slightly dim + cool the ground so organisms and overlays read clearly on
	# top and the terrain harmonizes with the dark instrument HUD.
	modulate = Color(0.78, 0.82, 0.88)
	_setup(int(sim.biome_resolution()))

# (Re)build the texture at `res`. Needed because the scenario loads AFTER this
# child node's _ready (children ready before the Main parent), so at _ready the
# sim still reports the DEFAULT resolution — a larger scenario would otherwise
# leave a size mismatch and a blank ground. Also re-runs on Restart into a
# different-size scenario.
func _setup(res: int) -> void:
	_res = res
	if _res <= 0:
		return
	_img = Image.create(_res, _res, false, Image.FORMAT_RGBA8)
	_tex = ImageTexture.create_from_image(_img)
	texture = _tex
	var world: float = sim.world_size()
	scale = Vector2(world / _res, world / _res)
	_last_mode = -999  # force an immediate redraw

func _process(_delta: float) -> void:
	var res: int = int(sim.biome_resolution())
	if res != _res:
		_setup(res)
	if _res <= 0:
		return
	# Current ground selection encoded as one int: -2 optimum, -1 biome, else channel.
	var mode: int = -1
	if overlay.ground_is_optimum():
		mode = -2
	else:
		var ch0: int = overlay.ground_channel()
		if ch0 >= 0:
			mode = ch0
	# Throttle: rebuild every REDRAW_EVERY frames, but immediately when the ground
	# selection changed (so [G]/overlay toggles feel instant).
	_frame += 1
	if mode == _last_mode and _frame % REDRAW_EVERY != 0:
		return
	_last_mode = mode

	var colors: PackedColorArray
	if mode == -2:
		# Flat tint whose hue encodes the current global optimum in [0,1].
		var opt: float = sim.env_optimum()
		var c: Color = Color.from_hsv(clampf(opt, 0.0, 1.0) * 0.8, 0.7, 0.5) if opt >= 0.0 else Color(0.1, 0.1, 0.12)
		colors = PackedColorArray()
		colors.resize(_res * _res)
		colors.fill(c)
	elif mode >= 0:
		colors = sim.pheromone_colors(mode)
	else:
		colors = sim.biome_colors()
	if colors.size() != _res * _res:
		return

	# Build an RGBA8 byte buffer in one pass (faster than per-pixel set_pixel).
	var bytes := PackedByteArray()
	bytes.resize(_res * _res * 4)
	for i in colors.size():
		var col: Color = colors[i]
		var o: int = i * 4
		bytes[o] = int(clampf(col.r, 0.0, 1.0) * 255.0)
		bytes[o + 1] = int(clampf(col.g, 0.0, 1.0) * 255.0)
		bytes[o + 2] = int(clampf(col.b, 0.0, 1.0) * 255.0)
		bytes[o + 3] = int(clampf(col.a, 0.0, 1.0) * 255.0)
	_img.set_data(_res, _res, false, Image.FORMAT_RGBA8, bytes)
	_tex.update(_img)
