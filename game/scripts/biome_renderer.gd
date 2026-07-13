extends Sprite2D

@onready var sim = get_node("/root/Main/Simulation")
@onready var overlay = get_node("/root/Main/OverlayManager")

var _img: Image
var _tex: ImageTexture
var _res: int = 0

func _ready() -> void:
	_res = int(sim.biome_resolution())
	if _res <= 0:
		return
	_img = Image.create(_res, _res, false, Image.FORMAT_RGBA8)
	_tex = ImageTexture.create_from_image(_img)
	texture = _tex
	centered = false
	var world: float = sim.world_size()
	scale = Vector2(world / _res, world / _res)
	position = Vector2.ZERO
	z_index = -10

func _process(_delta: float) -> void:
	if _res <= 0:
		return
	var colors: PackedColorArray
	var ch: int = overlay.ground_channel()
	if overlay.ground_is_optimum():
		# Flat tint whose hue encodes the current global optimum in [0,1].
		var opt: float = sim.env_optimum()
		var c: Color = Color.from_hsv(clampf(opt, 0.0, 1.0) * 0.8, 0.7, 0.5) if opt >= 0.0 else Color(0.1, 0.1, 0.12)
		colors = PackedColorArray()
		colors.resize(_res * _res)
		colors.fill(c)
	elif ch >= 0:
		colors = sim.pheromone_colors(ch)
	else:
		colors = sim.biome_colors()
	if colors.size() != _res * _res:
		return
	for row in _res:
		for col in _res:
			_img.set_pixel(col, row, colors[row * _res + col])
	_tex.update(_img)
