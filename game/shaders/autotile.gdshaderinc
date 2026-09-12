// Dual-grid water/land coast autotiling (Phase 2 step 3 of
// docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md, D1/D2).
//
// The transition grid is offset half a cell from the terrain id grid. For a
// fragment at fractional cell coordinate `cellf` (cell space of the id
// texture — i.e. UV * biome_res), the corner cell is
// `c = floor(cellf - 0.5)`; its 4 surrounding terrain cells are the corners
// of the coast tile:
//
//   TL = c              TR = c + (1, 0)
//   BL = c + (0, 1)     BR = c + (1, 1)
//
// A 4-bit mask picks one of the 16 coast_tiles.gd tiles:
//
//   bit 0 (1) = TL is water   bit 1 (2) = TR is water
//   bit 2 (4) = BL is water   bit 3 (8) = BR is water
//
// mask 0 (all land) and mask 15 (all water) carry no tile (alpha 0 — nothing
// to draw, the flat shader branches already look right); the coast atlas
// cell index equals the mask, row-major, matching coast_tiles.gd's
// `build_atlas()`.
//
// Written for a Godot 4 canvas_item shader include: no TIME, no `texture()`
// outside a function body, only functions/uniforms usable from an included
// file.

// True when the terrain id at `cell` (cell space of `ids_tex`, `ids_res`
// cells per axis) is Water (id 0) — same sampling convention terrain.gdshader
// already uses for terrain_ids: `(cell + 0.5) / ids_res`, id decoded as
// `floor(r * 255.0 + 0.5)`.
bool at_is_water(sampler2D ids_tex, float ids_res, vec2 cell) {
	float tid = floor(texture(ids_tex, (cell + 0.5) / ids_res).r * 255.0 + 0.5);
	return tid < 0.5;
}

// The dual-grid coast transition colour for `cellf` (fractional cell
// coordinate in the id texture's cell space). Alpha 0 where there is no
// transition tile (mask 0 or 15) — callers mix it in with `t.a` as the
// blend weight, e.g.:
//
//   vec4 t = coast_autotile(terrain_ids, biome_res, cellf, coast_atlas, 4.0, 16.0, 64.0);
//   land_base = mix(land_base, t.rgb, t.a * detail);
vec4 coast_autotile(sampler2D ids_tex, float ids_res, vec2 cellf,
		sampler2D coast_atlas, float atlas_cols, float cell_px, float atlas_px) {
	vec2 c = floor(cellf - 0.5);
	bool tl = at_is_water(ids_tex, ids_res, c);
	bool tr = at_is_water(ids_tex, ids_res, c + vec2(1.0, 0.0));
	bool bl = at_is_water(ids_tex, ids_res, c + vec2(0.0, 1.0));
	bool br = at_is_water(ids_tex, ids_res, c + vec2(1.0, 1.0));
	int mask = int(tl) + int(tr) * 2 + int(bl) * 4 + int(br) * 8;
	if (mask == 0 || mask == 15) {
		return vec4(0.0);
	}
	vec2 acell = vec2(mod(float(mask), atlas_cols), floor(float(mask) / atlas_cols));
	vec2 local = fract(cellf - 0.5);
	vec2 tuv = (acell + local) * (cell_px / atlas_px);
	return texture(coast_atlas, tuv);
}
