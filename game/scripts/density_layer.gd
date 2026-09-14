# main.gd: var d := preload("res://scripts/density_layer.gd").new(); add_child(d); d.setup(sim, $Camera2D)
# (add this after the body meshes are created, so the layer sits at the
# bodies' z level — they are hidden in overview anyway.)
extends Sprite2D

# Far-zoom (< 1x) stand-in for individual agent bodies: a res*res saturating
# count grid from the bridge (agent_density(), the same call the minimap
# panel uses), painted into one world-sized texture that fades from amber to
# white as a cell's occupancy climbs. At zoom.x < 1.0 the viewport can no
# longer frame bodies at an honest texel scale (docs/superpowers/specs/
# 2026-09-12-pixel-world-at-scale-design.md §4/D4), so this layer takes over
# from the (still-present, but hidden by main.gd in overview) body meshes.
#
# Tiled 3x3 like biome_renderer.gd's ground: this Sprite2D is the origin
# copy, plus 8 child Sprite2Ds sharing its ImageTexture and offset by whole
# worlds, so the torus wraps at the overview the same way the terrain does.

const AMBER_DOT := Color(1.0, 0.75, 0.3)

# Cells at or above this count start blending toward white so hotspots read
# as distinctly denser than a merely-occupied cell; HOT_RANGE is how many
# further counts it takes to reach full white.
const HOT_COUNT := 12
const HOT_RANGE := 12.0

# Density grid resolution. Higher than the minimap's 128 since this layer
# fills the whole viewport rather than a small HUD panel; still well under
# the bridge's 512 clamp.
const DENSITY_RES := 256

# Rebuild cadence: the bridge call walks every alive agent, and overview mode
# is already a coarse whole-world view, so a few stale frames are invisible.
const REFRESH_FRAMES := 4

var _sim
var _cam: Camera2D
var _img: Image
var _tex: ImageTexture
var _tiles: Array[Sprite2D] = []
var _frame: int = 0
var _last_world: float = -1.0
var _last_res: int = -1
var _was_overview: bool = false


func setup(sim, cam: Camera2D) -> void:
	_sim = sim
	_cam = cam
	centered = false
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	z_index = 0
	modulate = Color(1.0, 1.0, 1.0, 0.9)
	_img = Image.create(DENSITY_RES, DENSITY_RES, false, Image.FORMAT_RGBA8)
	_tex = ImageTexture.create_from_image(_img)
	texture = _tex
	for gy in range(-1, 2):
		for gx in range(-1, 2):
			if gx == 0 and gy == 0:
				continue
			var tile := Sprite2D.new()
			tile.centered = false
			tile.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
			tile.z_index = 0
			tile.texture = _tex
			tile.position = Vector2(gx * DENSITY_RES, gy * DENSITY_RES)
			add_child(tile)
			_tiles.append(tile)


func _process(_delta: float) -> void:
	if _sim == null or _cam == null:
		return
	if not _cam.is_overview():
		visible = false
		_was_overview = false
		return
	var just_shown := not _was_overview
	visible = true
	_was_overview = true
	var world: float = float(_sim.world_size())
	var res: int = int(_sim.biome_resolution())
	if world != _last_world or res != _last_res:
		_last_world = world
		_last_res = res
		_refresh_scale(world)
	_frame += 1
	if not just_shown and _frame % REFRESH_FRAMES != 0:
		return
	var counts: PackedByteArray = _sim.agent_density(DENSITY_RES)
	var bytes: PackedByteArray = density_image_bytes(counts, DENSITY_RES)
	if bytes.size() != DENSITY_RES * DENSITY_RES * 4:
		return
	_img.set_data(DENSITY_RES, DENSITY_RES, false, Image.FORMAT_RGBA8, bytes)
	_tex.update(_img)


# Scale the DENSITY_RES-pixel texture so it spans exactly one world (tiles are
# positioned in texture-pixel space in setup(), so they need no rescaling of
# their own — they inherit this node's scale like biome_renderer's tiles do).
func _refresh_scale(world: float) -> void:
	if world <= 0.0:
		return
	scale = Vector2(world / DENSITY_RES, world / DENSITY_RES)


# Pack a res*res saturating count grid (agent_density()'s row-major layout)
# into an RGBA8 byte buffer: count 0 -> fully transparent; count >= 1 -> the
# amber dot colour, alpha climbing with occupancy; count >= HOT_COUNT ->
# blended toward white so hotspots read as distinctly denser. Returns an
# empty array on a size mismatch (no world loaded yet, or a stale res),
# rather than indexing out of range.
static func density_image_bytes(counts: PackedByteArray, res: int) -> PackedByteArray:
	var out := PackedByteArray()
	if res <= 0 or counts.size() != res * res:
		return out
	out.resize(res * res * 4)
	for i in counts.size():
		var count: int = counts[i]
		var o := i * 4
		if count <= 0:
			continue  # buffer is zero-initialized: fully transparent
		var alpha: float = clampf(0.35 + count / 6.0, 0.0, 1.0)
		var col: Color = AMBER_DOT
		if count >= HOT_COUNT:
			# +1 so the blend is already visibly underway right at HOT_COUNT,
			# not merely armed for the count after it.
			var t: float = clampf(float(count - HOT_COUNT + 1) / HOT_RANGE, 0.0, 1.0)
			col = AMBER_DOT.lerp(Color(1.0, 1.0, 1.0), t)
		out[o] = int(round(col.r * 255.0))
		out[o + 1] = int(round(col.g * 255.0))
		out[o + 2] = int(round(col.b * 255.0))
		out[o + 3] = int(round(alpha * 255.0))
	return out
