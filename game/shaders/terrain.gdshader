shader_type canvas_item;

// Terrain presentation shader for the biome ground layer.
//
// The sim hands us a low-resolution grid of flat biome colors (nearest-neighbour
// upscaled), which reads as a hard color mosaic. This shader turns that grid into
// something that reads as *terrain*: gentle relief shading derived from color
// luminance, softened land transitions, and living water (depth gradient, coastal
// shallows, a slow animated shimmer) plus sandy shorelines and a soft vignette.
//
// It is a pure presentation pass over READ-ONLY sim colors — no data is altered.
// When the ground layer is showing a DATA overlay (pheromones, optimum, markets,
// succession) rather than the biome, `biome_mode` is set to 0 and the shader
// passes the colors straight through so heatmaps stay faithful.

uniform float biome_mode = 1.0;        // 1 = terrain treatment, 0 = passthrough
uniform float relief_strength = 3.5;   // hillshade intensity on land (elevation gradient)
uniform float soften = 0.10;           // 0 = crisp blocks, 1 = fully smoothed
uniform vec2 light_dir = vec2(-0.65, -0.75); // light from upper-left
uniform float world_size = 512.0;      // sim world extent, for seamless shimmer
uniform float sea_level = 0.35;        // viewer-derived water line, for depth shading
uniform float vignette = 0.06;         // corner darkening on terrain (0 = off)
// Land finish: a saturation lift and brightness gain applied after relief and
// shore passes so the muted sim palette reads as pixel art, not a heatmap.
uniform float vibrance = 0.12;
uniform float land_lift = 0.92;
// 2.5D relief (D9): land elevation is quantised into terraces; where a cell
// stands a terrace above its southern neighbour the bottom band of the cell
// wears a dark cliff face, a plateau's northern edge a lit rim, and the
// east/west edges a thin dark line, so highlands read as stacked ledges the
// way the reference boards' cliffs do. `terrace_levels` counts levels over
// the full 0..1 elevation range; `cliff_strength` scales the face darkening;
// `height_lift` brightens land with altitude.
uniform float terrace_levels = 14.0;
uniform float cliff_strength = 0.75;
uniform float cliff_band = 0.42;   // fraction of a cell the face occupies
uniform float height_lift = 0.22;

// Pixel-art ground tiles (terrain_sprites.gd atlas), blended into the land
// branch only — water keeps its living treatment, and the data-overlay
// passthrough below hides tiles automatically. `terrain_ids` is a res×res R8
// image of TerrainType discriminants (Water = 0 .. Tundra = 8) from
// `sim.biome_terrain_ids()`; exact ids beat re-classifying the lossy
// biomass/succession-lerped colors. The atlas MUST stay a square grid
// (extreme-aspect atlases corrupt on Metal — see terrain_sprites.gd).
uniform sampler2D tile_atlas : filter_nearest, repeat_disable;
uniform sampler2D terrain_ids : filter_nearest, repeat_disable;
uniform float tiles_enabled = 0.0; // biome_renderer flips this once textures are wired
uniform float tile_mix = 0.9;      // tile weight over the flat biome color at full detail
uniform float biome_res = 128.0;   // cells per axis, matches terrain_ids size
uniform float tile_variants = 3.0; // keep in sync with terrain_sprites.gd VARIANTS
uniform float atlas_cols = 8.0;    // keep in sync with terrain_sprites.gd ATLAS_COLS
uniform float atlas_cell_px = 16.0;
uniform float atlas_px = 128.0;

// Chunk addressing (D3, `docs/superpowers/specs/
// 2026-09-12-pixel-world-at-scale-design.md` §4/§6 Phase 2): a streamed
// GroundChunk shows a 66x66-cell apron texture through its interior 64x64
// region. Godot hands a region sprite its UV in WHOLE-texture space, so
// `UV * biome_res` (biome_res = 66 on a chunk) already lands in apron cell
// coordinates and the neighbour taps at TEXTURE_PIXEL_SIZE read the apron.
// `ids_offset` is the apron inset ((1,1) on a chunk, (0,0) on the
// whole-world sprite) and `cell_origin` the chunk's global cell coordinate;
// together they recover the WORLD cell for the per-cell hash, so a chunk's
// tile variants match the whole-world path and never drift at a border.
// `cell_world` is the width of one cell in world units for the LOD fade
// (0 = derive it from world_size / biome_res, which is only right on the
// whole-world sprite).
uniform vec2 ids_offset = vec2(0.0);
uniform vec2 cell_origin = vec2(0.0);
uniform float cell_world = 0.0;

// Dual-grid coast transitions (autotile.gdshaderinc, coast_tiles.gd): a
// 4x4 atlas of 16 px tiles indexed by the water mask of the four cells
// around each half-offset corner. Drawn over both the land and the water
// side at full tile detail, and faded out with the tile LOD.
#include "res://shaders/autotile.gdshaderinc"
uniform sampler2D coast_atlas : filter_nearest, repeat_disable;
uniform float autotile_enabled = 0.0;
uniform float coast_cols = 4.0;
uniform float coast_cell_px = 16.0;
uniform float coast_atlas_px = 64.0;

// World-space position of this fragment. The ground is drawn as 9 separate tiles
// (origin + 8 torus wrap copies), each with its own 0..1 UV; deriving the water
// shimmer from world position instead of per-tile UV keeps the wave continuous
// across tile seams (no phase jump where copies abut).
varying vec2 world_pos;
// The node's modulate, captured in the vertex stage. In the fragment stage
// Godot 4 hands COLOR already multiplied by the texture sample — and this
// texture's alpha is the packed ELEVATION, not opacity — so multiplying the
// result by the fragment COLOR used to make deep water nearly transparent
// and every land cell partially see-through over the black clear colour.
varying vec4 modulate_v;

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 0.0, 1.0)).xy;
	modulate_v = COLOR;
}

float luma(vec3 c) {
	return dot(c, vec3(0.299, 0.587, 0.114));
}

// Water is the only dark, blue-dominant biome. Require blue clearly above BOTH
// red and green and the cell to be overall dark: this tolerates pollution
// darkening water toward brown while staying clear of Rock (bright, blue≈red),
// Tundra (pale), and every green/warm land biome. Thresholds sanity-checked
// against the exact palette in anabios-godot::biome_colors().
bool is_water(vec3 c) {
	return c.b > c.r + 0.05 && c.b > c.g + 0.05 && c.b > 0.20 && max(c.r, c.g) < 0.45;
}

// Exact terrain id of a cell (Water = 0) from the id texture.
float tid_at(vec2 cell) {
	return floor(texture(terrain_ids, (cell + 0.5) / biome_res).r * 255.0 + 0.5);
}

// Water test for a cell: the exact id when the id texture is wired (rivers
// report Water there too), the colour heuristic on the legacy path. The sim's
// water colour is dark enough to fail the heuristic's blue floor, which used
// to send every lake through the land branch.
bool water_cell(vec2 cell, vec3 col) {
	return tiles_enabled > 0.5 ? tid_at(cell) < 0.5 : is_water(col);
}

// Elevation (packed in alpha) interpolated between the four nearest cell
// centres, so relief, altitude and water depth vary continuously across a
// cell instead of stepping at every cell edge (the per-cell steps were what
// made the ground read as a mosaic of tiles). Terraces still quantise the
// raw per-cell value on purpose.
float elev_bilin(sampler2D tex, vec2 cellf) {
	vec2 b = floor(cellf - 0.5);
	vec2 f = fract(cellf - 0.5);
	float e00 = texture(tex, (b + vec2(0.5, 0.5)) / biome_res).a;
	float e10 = texture(tex, (b + vec2(1.5, 0.5)) / biome_res).a;
	float e01 = texture(tex, (b + vec2(0.5, 1.5)) / biome_res).a;
	float e11 = texture(tex, (b + vec2(1.5, 1.5)) / biome_res).a;
	return mix(mix(e00, e10, f.x), mix(e01, e11, f.x), f.y);
}

float hash2(vec2 p) {
	return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

// Ground tile sample for a cell of terrain id `tid` at fractional cell
// coordinate `cellf`; `variant` is the tile variant (0..tile_variants-1).
vec3 tile_at(float tid, float variant, vec2 cellf) {
	float cidx = tid * tile_variants + variant;
	vec2 acell = vec2(mod(cidx, atlas_cols), floor(cidx / atlas_cols));
	vec2 tuv = (acell + fract(cellf)) * (atlas_cell_px / atlas_px);
	return texture(tile_atlas, tuv).rgb;
}

// The water tile family's flat base colour (terrain_sprites.gd "w"); ripple
// pixels are read as their difference from it.
const vec3 WATER_TILE_BASE = vec3(0.090, 0.188, 0.439);

void fragment() {
	// The node's modulate (dim/cool tint), from the vertex stage — see above.
	vec4 tint = modulate_v;
	vec2 px = TEXTURE_PIXEL_SIZE;
	vec3 c = texture(TEXTURE, UV).rgb;

	// Neighbour taps at biome-texel scale — used for both softening and relief.
	vec3 cL = texture(TEXTURE, UV - vec2(px.x, 0.0)).rgb;
	vec3 cR = texture(TEXTURE, UV + vec2(px.x, 0.0)).rgb;
	vec3 cU = texture(TEXTURE, UV - vec2(0.0, px.y)).rgb;
	vec3 cD = texture(TEXTURE, UV + vec2(0.0, px.y)).rgb;

	// Soften hard pixel stair-steps by blending toward the 4-neighbour mean,
	// but keep enough of the center that distinct biomes stay distinct.
	vec3 soft_c = (c * 4.0 + cL + cR + cU + cD) / 8.0;
	vec3 base = mix(c, soft_c, soften);

	vec2 cell0 = floor(UV * biome_res);
	bool water = water_cell(cell0, base);

	// Coast proximity: how many neighbours differ in water-ness from center.
	float coast = 0.0;
	coast += water_cell(cell0 - vec2(1.0, 0.0), cL) != water ? 1.0 : 0.0;
	coast += water_cell(cell0 + vec2(1.0, 0.0), cR) != water ? 1.0 : 0.0;
	coast += water_cell(cell0 - vec2(0.0, 1.0), cU) != water ? 1.0 : 0.0;
	coast += water_cell(cell0 + vec2(0.0, 1.0), cD) != water ? 1.0 : 0.0;
	coast *= 0.25; // 0..1

	// Tile-space cell coordinate and the LOD weight, shared by the land tiles
	// and the coast autotile: detail dissolves as a tile texel shrinks below a
	// screen pixel so far zoom looks exactly like the flat ground.
	vec2 cellf = UV * biome_res;
	float texel_w = (cell_world > 0.0 ? cell_world : world_size / biome_res) / atlas_cell_px;
	float texels_per_px = fwidth(world_pos.x) / max(texel_w, 1e-4);
	float detail = smoothstep(4.5, 1.2, texels_per_px);
	vec4 coast_tile = vec4(0.0);
	if (tiles_enabled > 0.5 && autotile_enabled > 0.5) {
		coast_tile = coast_autotile(terrain_ids, biome_res, cellf, coast_atlas, coast_cols, coast_cell_px, coast_atlas_px);
	}

	vec3 outc;
	if (water) {
		// Lake blue: one tone across open water (the reference boards' water
		// is flat and saturated), a faint depth cue from the interpolated
		// elevation, and a brighter shelf on the cells that touch land.
		vec3 deep = mix(base, vec3(0.16, 0.46, 0.84), 0.66);
		vec3 shallow = mix(base, vec3(0.42, 0.78, 0.92), 0.78);
		float es = elev_bilin(TEXTURE, cellf);
		float shallowness = clamp(es / max(sea_level, 1e-3), 0.0, 1.0);
		shallowness *= shallowness;
		outc = mix(deep, shallow, 0.22 * shallowness + 0.18 * step(0.01, coast));

		// Pixel ripples: the water tile family's light dashes and dark
		// specks, cycled through its variants on a per-cell phase so the
		// surface twinkles slowly; dissolves with the tile LOD like land.
		if (tiles_enabled > 0.5) {
			vec2 wcell = cell_origin + cell0 - ids_offset;
			float h = hash2(wcell);
			float variant = mod(floor(h * tile_variants + TIME * 1.1 + h * 7.0), tile_variants);
			vec3 tile = tile_at(0.0, variant, cellf);
			// Roughly a third of the cells stay calm so the ripples read as
			// scattered strokes rather than a hatch.
			float calm = step(0.35, hash2(wcell + vec2(13.0, 5.0)));
			outc += (tile - WATER_TILE_BASE) * 1.4 * detail * calm;
		}
		// A soft, slow swell in seamless world coordinates keeps far zoom
		// (where the ripples have dissolved) from going dead flat.
		vec2 wuv = world_pos / world_size;
		float w = sin(wuv.x * 90.0 + TIME * 0.6) * 0.5 + 0.5;
		w *= sin(wuv.y * 70.0 - TIME * 0.4) * 0.5 + 0.5;
		outc += vec3(0.02, 0.03, 0.04) * w * (1.0 - detail);

		// Coast tile on the water side (shallows + foam).
		outc = mix(outc, coast_tile.rgb, coast_tile.a * detail);
	} else {
		// Pixel-art tile detail: pick this cell's tile family from its exact
		// terrain id, a variant from a per-cell hash (same hash family as
		// terrain_scatter.gd so platforms can't drift), and blend it under the
		// relief/beach passes so lushness, succession and hillshade still show
		// through. Detail dissolves back to the flat biome color as a tile
		// texel shrinks below a screen pixel (whole-world framings), so far
		// zoom looks exactly like the pre-tile ground.
		vec3 land_base = base;
		if (tiles_enabled > 0.5) {
			vec2 cell = floor(cellf);
			float tid = tid_at(cell);
			if (tid > 0.5) {
				// Dithered biome edges: each ground texel looks up its tile
				// family from a cell jittered by up to half a cell in a
				// stable per-texel pattern, so grass and savanna (say)
				// interleave along their border instead of meeting at a
				// hard cell edge. A jitter that lands in water keeps the
				// cell's own family so coasts stay crisp.
				vec2 texel = floor(world_pos / max(texel_w, 1e-4));
				vec2 jit = vec2(hash2(texel + vec2(3.1, 7.7)), hash2(texel + vec2(9.3, 1.9))) - 0.5;
				vec2 jcell = floor(cellf + jit * 1.1);
				float jt = tid_at(jcell);
				vec2 pick = jt > 0.5 ? jcell : cell;
				float ptid = jt > 0.5 ? jt : tid;
				// World-space cell, for a hash that matches the whole-world
				// path exactly (cell_origin/ids_offset default to 0/(0,0)
				// there, so this is `cell` unchanged on the whole-world
				// sprite; a chunk undoes its (1,1) apron inset and adds its
				// own global cell coordinate).
				vec2 world_cell = cell_origin + pick - ids_offset;
				float h = hash2(world_cell);
				float variant = min(floor(h * tile_variants), tile_variants - 1.0);
				vec3 tile = tile_at(ptid, variant, cellf);
				land_base = mix(base, tile, tile_mix * detail);
			}
		}

		// Land relief: sample the packed real elevation (biome texture alpha) at
		// the four neighbours and light the height gradient directionally. This
		// replaces the old luminance-as-height fake so mountain ranges (not
		// bright biomes) cast the relief.
		float eL = texture(TEXTURE, UV - vec2(px.x, 0.0)).a;
		float eR = texture(TEXTURE, UV + vec2(px.x, 0.0)).a;
		float eU = texture(TEXTURE, UV - vec2(0.0, px.y)).a;
		float eD = texture(TEXTURE, UV + vec2(0.0, px.y)).a;
		// Continuous gradient from the interpolated elevation (the per-cell
		// taps above still drive the terraces).
		vec2 grad = vec2(
			elev_bilin(TEXTURE, cellf + vec2(1.0, 0.0)) - elev_bilin(TEXTURE, cellf - vec2(1.0, 0.0)),
			elev_bilin(TEXTURE, cellf + vec2(0.0, 1.0)) - elev_bilin(TEXTURE, cellf - vec2(0.0, 1.0)));
		float shade = dot(normalize(light_dir), grad) * relief_strength;
		// Brighten slopes facing the light, darken those facing away.
		// Kept gentle: at the old strength a slope facing away from the
		// light read as a large dark smear across open grass.
		outc = land_base * (1.0 + clamp(shade, -0.22, 0.22));

		// Terraces and cliff faces (D9). Levels are integers; a drop of one
		// or more levels toward a neighbour paints that edge of the cell.
		float e0 = texture(TEXTURE, UV).a;
		float lv = floor(e0 * terrace_levels);
		float lvD = floor(eD * terrace_levels);
		float lvU = floor(eU * terrace_levels);
		float lvL = floor(eL * terrace_levels);
		float lvR = floor(eR * terrace_levels);
		vec2 fc = fract(cellf);
		// Southern cliff face: the higher the drop, the darker and browner.
		float drop = clamp((lv - lvD) / 3.0, 0.0, 1.0);
		// A level crossing only paints where the elevation really steps (at
		// least 0.6 of a level between the two cells): gentle slopes cross
		// levels with tiny differences and used to sprout stray bars.
		float real = 0.6 / terrace_levels;
		float face = drop * step(1.0 - cliff_band, fc.y) * step(0.5, lv - lvD) * step(real, e0 - eD);
		vec3 cliff = mix(outc * 0.45, vec3(0.30, 0.23, 0.17), 0.5);
		// A lit lip along the top of the face.
		float lip = step(1.0 - cliff_band, fc.y) * (1.0 - step(1.0 - cliff_band + 0.08, fc.y));
		outc = mix(outc, cliff, face * cliff_strength);
		outc = mix(outc, outc * 1.35 + 0.08, face * lip * cliff_strength);
		// Northern rim of a plateau: a thin bright line.
		float rim = step(0.5, lv - lvU) * step(real, e0 - eU) * (1.0 - step(0.10, fc.y));
		outc = mix(outc, outc * 1.25 + 0.06, rim * cliff_strength * 0.8);
		// East/west ledges: a thin dark line where the cell stands higher.
		float ledge = step(0.5, lv - lvL) * step(real, e0 - eL) * (1.0 - step(0.08, fc.x))
			+ step(0.5, lv - lvR) * step(real, e0 - eR) * step(0.92, fc.x);
		outc = mix(outc, outc * 0.6, clamp(ledge, 0.0, 1.0) * cliff_strength * 0.3);
		// Altitude: highlands sit a touch brighter than the lowlands.
		float es = elev_bilin(TEXTURE, cellf);
		outc *= 1.0 + height_lift * clamp((es - sea_level) / max(1.0 - sea_level, 1e-3), 0.0, 1.0);

		// Sandy shoreline on the land side: a thin warm beach band right at the
		// waterline, fading inland, so coasts read as coast rather than a hard
		// color cut. Kept subtle and tinted to the cell so it reads under any
		// biome (grass/forest/desert).
		vec3 sand = vec3(0.80, 0.73, 0.52);
		outc = mix(outc, sand, coast * 0.18);

		// Slightly deepen the shoreline on the land side just past the beach so
		// the transition has a little depth.
		outc = mix(outc, outc * 0.86, max(coast - 0.25, 0.0) * 0.5);

		// Land finish: lift saturation and brightness so tiles read crisp and
		// warm, then stamp the shoreline band LAST so sand and foam stay bright
		// under the relief and shore-darkening passes above.
		float lum = luma(outc);
		outc = mix(vec3(lum), outc, 1.0 + vibrance) * land_lift;
		outc = mix(outc, coast_tile.rgb, coast_tile.a * detail);
	}

	// Soft vignette to frame the field — darkens screen corners only, and only
	// on terrain (folded into `outc`, so the passthrough branch below is unaffected
	// and data heatmaps keep full brightness edge-to-edge).
	float vig = 1.0 - vignette * pow(length(SCREEN_UV - 0.5) * 1.414, 2.5);
	outc *= vig;

	// Data-overlay mode (pheromone/optimum/market/succession): faithful
	// passthrough so heatmaps stay accurate; otherwise use the terrain result.
	outc = mix(c, outc, step(0.5, biome_mode));

	COLOR = vec4(clamp(outc, 0.0, 1.0), 1.0) * tint;
}
