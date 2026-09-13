extends Sprite2D
# One streamed ground tile (D3, `docs/superpowers/specs/
# 2026-09-12-pixel-world-at-scale-design.md` §4/§6 Phase 2): a 66×66 RGBA8
# colour texture and a 66×66 R8 id texture, both carrying a 1-cell torus
# apron (`biome_chunk_bytes`/`biome_chunk_ids`), displayed through the
# interior 64×64 region so the apron feeds the terrain shader's neighbour
# taps without itself drawing. Created and driven entirely by ground_layer.gd
# — this node owns no bridge calls or per-frame logic of its own.

const APRON := 66
const INTERIOR := 64

var _color_img: Image
var _color_tex: ImageTexture
var _ids_img: Image
var _ids_tex: ImageTexture
var _mat: ShaderMaterial


# One-time wiring for a freshly created chunk at grid index (cx, cy):
# duplicate the shared terrain material (so per-chunk uniform overrides never
# leak into the whole-world sprite or sibling chunks) and set the uniforms
# that never change again for this chunk's lifetime — `cell_origin` is the
# chunk's global cell coordinate, keeping the tile-variant hash identical to
# the whole-world path across chunk borders (D3).
func setup(shared_material: ShaderMaterial, cx: int, cy: int) -> void:
	centered = false
	region_enabled = true
	region_rect = Rect2(1, 1, INTERIOR, INTERIOR)
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_mat = shared_material.duplicate() as ShaderMaterial
	material = _mat
	# A region sprite's UV spans the WHOLE texture, so biome_res = 66 puts
	# `UV * biome_res` straight into apron cell coordinates; ids_offset is the
	# apron inset the hash subtracts, cell_origin the chunk's global cell.
	_mat.set_shader_parameter("biome_res", float(APRON))
	_mat.set_shader_parameter("ids_offset", Vector2(1.0, 1.0))
	_mat.set_shader_parameter("cell_origin", Vector2(cx * INTERIOR, cy * INTERIOR))


# Push fresh chunk bytes: `bytes` is APRON×APRON RGBA8 (`biome_chunk_bytes`),
# `ids` is APRON×APRON R8 (`biome_chunk_ids`). No-op (returns false) on a
# size mismatch, so a caller can tell an upload apart from a skip.
func upload(bytes: PackedByteArray, ids: PackedByteArray) -> bool:
	if bytes.size() != APRON * APRON * 4 or ids.size() != APRON * APRON:
		return false
	if _color_img == null:
		_color_img = Image.create_from_data(APRON, APRON, false, Image.FORMAT_RGBA8, bytes)
		_color_tex = ImageTexture.create_from_image(_color_img)
		texture = _color_tex
		_mat.set_shader_parameter("biome_tex", _color_tex)
	else:
		_color_img.set_data(APRON, APRON, false, Image.FORMAT_RGBA8, bytes)
		_color_tex.update(_color_img)
	if _ids_img == null:
		_ids_img = Image.create_from_data(APRON, APRON, false, Image.FORMAT_R8, ids)
		_ids_tex = ImageTexture.create_from_image(_ids_img)
	else:
		_ids_img.set_data(APRON, APRON, false, Image.FORMAT_R8, ids)
		_ids_tex.update(_ids_img)
	_mat.set_shader_parameter("terrain_ids", _ids_tex)
	return true


# Place the chunk's interior (the 64×64 region) at `world_pos` (its origin,
# not centered — matches `centered = false`), scaled so each texel is
# `cell_w` world units (one biome cell).
func place(world_pos: Vector2, cell_w: float) -> void:
	position = world_pos
	scale = Vector2(cell_w, cell_w)
	# The LOD fade needs the real cell width; world_size / biome_res would
	# read the 66-cell apron as a whole world.
	_mat.set_shader_parameter("cell_world", cell_w)
