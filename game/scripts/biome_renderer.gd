extends Sprite2D

@onready var sim = get_node("../Simulation")
@onready var overlay = get_node("../OverlayManager")

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

# Pixel-art ground: the terrain_sprites.gd tile atlas sampled in the shader,
# keyed by an R8 texture of exact TerrainType ids from the bridge, plus the
# decoration prop scatter child. [B] toggles the whole treatment.
const TerrainSprites := preload("res://scripts/terrain_sprites.gd")
const TerrainScatter := preload("res://scripts/terrain_scatter.gd")
var _tiles_on := true
var _ids := PackedByteArray()
var _ids_tex: ImageTexture = null
var _scatter: Node2D = null
# Decorative props (reeds, shrubs, conifers, rocks, logs) scattered from the
# terrain colours. A child of this sprite so it shares the ground's wrap
# tiling and dim/cool modulate; shown only in the biome view.
const BiomeProps = preload("res://scripts/biome_props.gd")
var _props: Node2D

# Phase 2 chunk streaming (D3): GroundLayer draws resident 64-cell chunk
# sprites over this whole-world sprite while active; the whole-world sprite
# always stays underneath as the far-zoom fallback and the [B]-toggle A/B
# reference. [N] toggles streaming on/off, independent of [B], for A/B
# captures between the two ground paths.
const GroundLayer = preload("res://scripts/ground_layer.gd")
var _ground_layer: Node2D
var _chunks_on := true


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
	# Static tile-atlas wiring; per-world uniforms (biome_res, terrain_ids)
	# follow in _setup once the scenario's resolution is known.
	_terrain_mat.set_shader_parameter("tile_atlas", TerrainSprites.build_atlas())
	_terrain_mat.set_shader_parameter("tile_variants", float(TerrainSprites.VARIANTS))
	_terrain_mat.set_shader_parameter("atlas_cols", float(TerrainSprites.ATLAS_COLS))
	_terrain_mat.set_shader_parameter("atlas_cell_px", float(TerrainSprites.CELL_PX))
	_terrain_mat.set_shader_parameter(
		"atlas_px", float(TerrainSprites.ATLAS_COLS * TerrainSprites.CELL_PX)
	)
	_scatter = TerrainScatter.new()
	_scatter.name = "TerrainScatter"
	add_child(_scatter)
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
	_props = BiomeProps.new()
	_props.name = "BiomeProps"
	add_child(_props)
	_props.setup(sim)
	_ground_layer = GroundLayer.new()
	_ground_layer.name = "GroundLayer"
	add_child(_ground_layer)
	_ground_layer.setup(self, sim)
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
	_redraw_interval = REDRAW_EVERY * maxi(1, int(_res / 128.0))
	_last_mode = -999  # force an immediate redraw
	# Reset the tile-id cache so the id texture and scatter rebuild for the new
	# world; counter-scale the scatter child back to world units (children
	# inherit this node's world/res scale).
	_ids = PackedByteArray()
	_terrain_mat.set_shader_parameter("biome_res", float(_res))
	if _scatter != null:
		_scatter.scale = Vector2(_res / world, _res / world)
	if _ground_layer != null:
		_ground_layer.scale = Vector2(_res / world, _res / world)


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


# The shared terrain ShaderMaterial (relief/water/tile-atlas uniforms already
# wired). GroundLayer duplicates this per chunk so a chunk's `terrain_ids`/
# `biome_res`/`ids_*` overrides never leak into the whole-world sprite or
# sibling chunks, while everything else (tile_atlas, world_size, sea_level,
# tiles_enabled, tile_mix, ...) tracks this node's uniform writes untouched.
func terrain_material() -> ShaderMaterial:
	return _terrain_mat


# True while the ground selection is the biome view (as opposed to a data
# overlay) — GroundLayer only draws chunks over the biome view.
func is_biome_view() -> bool:
	return _last_mode == -1


# True while the pixel-art ground treatment ([B]) is on.
func tiles_enabled() -> bool:
	return _tiles_on


# True while chunk streaming ([N]) is on. Chunk streaming is independent of
# [B]: GroundLayer draws only when both are on, so [N] alone A/B-tests the
# whole-world sprite against the streamed chunks with tiles already showing.
func streaming_enabled() -> bool:
	return _chunks_on


# True when the biome pixel under `world_pos` is water, using the exact
# thresholds of terrain.gdshader's is_water() so presentation effects (drink
# ripples) agree with where the ground shader draws water. Reads whichever
# image currently carries raw biome colours: `_img` in the biome view,
# the minimap's slow biome copy while a data overlay owns the ground.
func is_water_at(world_pos: Vector2) -> bool:
	var img: Image = _img if _last_mode == -1 else _mini_img
	if img == null or _res <= 0:
		return false
	var world: float = _res * scale.x
	var px := clampi(int(fposmod(world_pos.x, world) / world * _res), 0, _res - 1)
	var py := clampi(int(fposmod(world_pos.y, world) / world * _res), 0, _res - 1)
	var c := img.get_pixel(px, py)
	return c.b > c.r + 0.05 and c.b > c.g + 0.05 and c.b > 0.20 and maxf(c.r, c.g) < 0.45


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
	# Pixel-art ground upkeep: refresh the terrain-id texture and the prop
	# scatter when the id grid changes (rarely — worldgen or disturbance).
	# Props hide under data overlays so heatmaps stay uncluttered; the tile
	# blend needs no gating because the passthrough branch already bypasses it.
	if _scatter != null:
		_scatter.visible = _tiles_on and mode == -1
	if _tiles_on and _frame % _redraw_interval == 0:
		_refresh_terrain_ids()
	# Scenery follows the same rule: props only over the real terrain.
	_props.set_terrain_visible(mode == -1)
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
	# Re-scatter the props from the fresh terrain (a no-op while the terrain
	# checksum holds); only the biome view carries raw terrain colours.
	if mode == -1:
		_props.refresh(colors, _res, sim.world_size())


# Upload the exact TerrainType id grid as an R8 texture for the tile lookup
# and re-plan the prop scatter — both only when the ids actually changed.
func _refresh_terrain_ids() -> void:
	var ids: PackedByteArray = sim.biome_terrain_ids()
	if ids.size() != _res * _res or ids == _ids:
		return
	_ids = ids
	var img := Image.create_from_data(_res, _res, false, Image.FORMAT_R8, ids)
	if _ids_tex == null or _ids_tex.get_width() != _res:
		_ids_tex = ImageTexture.create_from_image(img)
	else:
		_ids_tex.update(img)
	_terrain_mat.set_shader_parameter("terrain_ids", _ids_tex)
	_terrain_mat.set_shader_parameter("tiles_enabled", 1.0 if _tiles_on else 0.0)
	if _scatter != null:
		_scatter.rebuild(ids, _res, _res * scale.x)


# [B] toggles the pixel-art ground (tiles + props) without touching the
# relief/water treatment or any data overlay. [N] toggles chunk streaming
# (Phase 2, D3) independently, for A/B captures of the streamed ground
# against the whole-world sprite it draws over.
func _unhandled_input(event: InputEvent) -> void:
	if not (event is InputEventKey and event.pressed and not event.echo):
		return
	if event.keycode == KEY_B:
		_tiles_on = not _tiles_on
		_terrain_mat.set_shader_parameter("tiles_enabled", 1.0 if _tiles_on else 0.0)
		if _scatter != null:
			_scatter.visible = _tiles_on and _last_mode == -1
	elif event.keycode == KEY_N:
		_chunks_on = not _chunks_on


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
