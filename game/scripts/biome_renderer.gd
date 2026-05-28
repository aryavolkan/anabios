extends Sprite2D

@onready var sim = get_node("/root/Main/Simulation")

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
	var colors: PackedColorArray = sim.biome_colors()
	if colors.size() != _res * _res:
		return
	for row in _res:
		for col in _res:
			_img.set_pixel(col, row, colors[row * _res + col])
	_tex.update(_img)
