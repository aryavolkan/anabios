extends Sprite2D

@onready var sim = get_node("/root/Main/Simulation")
@onready var overlay = get_node("/root/Main/OverlayManager")

var _img: Image
var _tex: ImageTexture
var _res: int = 0

# A biome-only copy of the world, for the minimap. `_tex` carries whatever the
# [G] ground selection is showing, and the data overlays (pheromone, markets,
# env-optimum) are near-black over most of the map — which turned the minimap
# into a black square. This one always holds the terrain, refreshed on a slow
# cadence because nothing reads it at full-screen size.
var _mini_img: Image
var _mini_tex: ImageTexture
const MINI_REDRAW_EVERY := 10  # in units of _redraw_interval, not frames
var _mini_frame: int = 0

const REDRAW_EVERY := 6
var _frame: int = 0
var _last_mode: int = -999  # last (channel, or -1 biome, or -2 optimum) drawn
# Rebuild interval, scaled up for big biomes: the per-pixel GDScript rebuild is
# O(res²) (262k px at res=512), and the biome changes slowly, so large worlds
# redraw less often. Default res (128) keeps the original 6-frame cadence.
var _redraw_interval := REDRAW_EVERY

# The world is a torus, so the ground is tiled 3x3 around the origin copy:
# a camera near a seam (or zoomed out past a world edge) sees the terrain wrap
# instead of the empty backdrop. All nine sprites share this node's ImageTexture.
var _tiles: Array[Sprite2D] = []

# Shared terrain shader: relief shading + living water over the raw biome grid.
# One ShaderMaterial drives this node and all 8 wrap tiles, so a single uniform
# write (biome_mode) switches the whole ground between terrain and passthrough.
const TerrainShader := preload("res://shaders/terrain.gdshader")
var _terrain_mat: ShaderMaterial


func _ready() -> void:
	centered = false
	position = Vector2.ZERO
	z_index = -10
	# Slightly dim + cool the ground so organisms and overlays read clearly on
	# top and the terrain harmonizes with the dark instrument HUD.
	modulate = Color(0.85, 0.88, 0.92)
	# Linear filtering removes the harshest nearest-neighbour stair-steps before
	# the shader's relief/softening pass; the shader keeps biomes distinct.
	texture_filter = CanvasItem.TEXTURE_FILTER_LINEAR
	_terrain_mat = ShaderMaterial.new()
	_terrain_mat.shader = TerrainShader
	material = _terrain_mat
	for gy in range(-1, 2):
		for gx in range(-1, 2):
			if gx == 0 and gy == 0:
				continue
			var tile := Sprite2D.new()
			tile.centered = false
			tile.z_index = -10
			tile.texture_filter = CanvasItem.TEXTURE_FILTER_LINEAR
			tile.material = _terrain_mat
			add_child(tile)
			_tiles.append(tile)
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
	_mini_img = Image.create(_res, _res, false, Image.FORMAT_RGBA8)
	_mini_tex = ImageTexture.create_from_image(_mini_img)
	_mini_frame = 0
	var world: float = sim.world_size()
	scale = Vector2(world / _res, world / _res)
	# Feed the world extent to the terrain shader so its water shimmer runs in
	# seamless world coordinates across the 9 wrap tiles.
	if _terrain_mat != null:
		_terrain_mat.set_shader_parameter("world_size", world)
		_terrain_mat.set_shader_parameter("sea_level", sim.sea_level())
	# Neighbor tiles are children (they inherit the wrap scale), each offset by
	# whole worlds in biome-pixel units.
	var i := 0
	for gy in range(-1, 2):
		for gx in range(-1, 2):
			if gx == 0 and gy == 0:
				continue
			var tile := _tiles[i]
			i += 1
			tile.texture = _tex
			tile.position = Vector2(gx * _res, gy * _res)
	_redraw_interval = REDRAW_EVERY * maxi(1, _res / 128)
	_last_mode = -999  # force an immediate redraw


# The whole-world ground ImageTexture (res×res) as currently displayed — the
# biome, or whichever data overlay [G] selected. May be null before the first
# _setup(); callers must guard.
func world_texture() -> ImageTexture:
	return _tex


# The whole-world *biome* ImageTexture, whatever the ground overlay is showing.
# Identical to world_texture() in the default biome view (no second rebuild);
# a separately-maintained copy while a data overlay is up.
func minimap_texture() -> ImageTexture:
	return _tex if _last_mode == -1 else _mini_tex


func _process(_delta: float) -> void:
	var res: int = int(sim.biome_resolution())
	if res != _res:
		_setup(res)
	if _res <= 0:
		return
	# Current ground selection encoded as one int: -4 markets, -3 succession, -2 optimum, -1 biome, else channel.
	var mode: int = -1
	if overlay.ground_is_markets():
		mode = -4
	elif overlay.ground_is_succession():
		mode = -3
	elif overlay.ground_is_optimum():
		mode = -2
	else:
		var ch0: int = overlay.ground_channel()
		if ch0 >= 0:
			mode = ch0
	# Terrain treatment (relief + water) applies only to the biome view; data
	# overlays (pheromone/optimum/market/succession) pass through faithfully.
	if _terrain_mat != null:
		_terrain_mat.set_shader_parameter("biome_mode", 1.0 if mode == -1 else 0.0)
	# While a data overlay owns the ground texture, keep the minimap's biome copy
	# current on its own (much slower) cadence — the minimap is 200px wide and
	# the terrain creeps. `== 1` refreshes on the first frame after the switch so
	# the copy is never stale-empty. In the biome view the minimap shares `_tex`
	# and this does nothing.
	if mode == -1:
		_mini_frame = 0
	else:
		_mini_frame += 1
		if _mini_frame % (_redraw_interval * MINI_REDRAW_EVERY) == 1:
			_blit(sim.biome_colors(), _mini_img, _mini_tex)
	# Throttle: rebuild every REDRAW_EVERY frames, but immediately when the ground
	# selection changed (so [G]/overlay toggles feel instant).
	_frame += 1
	if mode == _last_mode and _frame % _redraw_interval != 0:
		return
	_last_mode = mode

	var colors: PackedColorArray
	if mode == -4:
		colors = sim.market_colors()
	elif mode == -3:
		colors = sim.succession_colors()
	elif mode == -2:
		# Flat tint whose hue encodes the current global optimum in [0,1].
		var opt: float = sim.env_optimum()
		var c: Color = (
			Color.from_hsv(clampf(opt, 0.0, 1.0) * 0.8, 0.7, 0.5)
			if opt >= 0.0
			else Color(0.1, 0.1, 0.12)
		)
		colors = PackedColorArray()
		colors.resize(_res * _res)
		colors.fill(c)
	elif mode >= 0:
		colors = sim.pheromone_colors(mode)
	else:
		colors = sim.biome_colors()
	_blit(colors, _img, _tex)


# Pack a res² colour grid into an RGBA8 byte buffer and push it to `tex` (one
# pass; faster than per-pixel set_pixel). No-op on a size mismatch.
func _blit(colors: PackedColorArray, img: Image, tex: ImageTexture) -> void:
	if img == null or tex == null or colors.size() != _res * _res:
		return
	var bytes := PackedByteArray()
	bytes.resize(_res * _res * 4)
	for i in colors.size():
		var col: Color = colors[i]
		var o: int = i * 4
		bytes[o] = int(clampf(col.r, 0.0, 1.0) * 255.0)
		bytes[o + 1] = int(clampf(col.g, 0.0, 1.0) * 255.0)
		bytes[o + 2] = int(clampf(col.b, 0.0, 1.0) * 255.0)
		bytes[o + 3] = int(clampf(col.a, 0.0, 1.0) * 255.0)
	img.set_data(_res, _res, false, Image.FORMAT_RGBA8, bytes)
	tex.update(img)
