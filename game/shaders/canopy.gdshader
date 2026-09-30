shader_type canvas_item;

// Canopy trees and rock outcrops of one prop chunk (prop_chunk.gd), all
// FloraSprites kinds in ONE MultiMesh so the chunk's trees draw in a single
// y-sorted pass: with one MultiMesh per kind, every crown of a later kind
// covered every overlapping crown of an earlier one whatever their depth.
// The texture is a square grid of the kinds' cells (prop_chunk.gd
// canopy_atlas, pre-flipped for the QuadMesh V axis like SpriteSplit.for_quad);
// the trunk layer and the crown layer share the MultiMesh with the lower- and
// upper-half atlases.
// Per-instance colour, written by prop_chunk.gd:
//   rgb = the tree's tone (prop_chunk.tree_tint)
//   a   = the kind's atlas cell (prop_chunk.kind_alpha)
// Read in the vertex stage and carried in flat varyings: reading the built-in
// COLOR in fragment() is not safe on the Metal MultiMesh2D path (see
// field_agent.gdshader, which carries its tint and bucket the same way).

// prop_chunk.gd CANOPY_ATLAS_COLS; a uniform so the packer and the decode
// stay paired.
uniform float atlas_cols = 4.0;

varying flat vec4 tint;
varying flat float cell;

void vertex() {
	tint = vec4(COLOR.rgb, 1.0);
	cell = floor(COLOR.a * atlas_cols * atlas_cols);
}

void fragment() {
	float col = mod(cell, atlas_cols);
	float row = floor(cell / atlas_cols);
	vec4 t = texture(TEXTURE, (vec2(col, row) + UV) / atlas_cols);
	COLOR = t * tint;
}
