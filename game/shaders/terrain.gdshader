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
uniform float relief_strength = 12.0;  // hillshade intensity on land (elevation gradient)
uniform float soften = 0.45;           // 0 = crisp blocks, 1 = fully smoothed
uniform vec2 light_dir = vec2(-0.65, -0.75); // light from upper-left
uniform float world_size = 512.0;      // sim world extent, for seamless shimmer
uniform float sea_level = 0.35;        // viewer-derived water line, for depth shading
uniform float vignette = 0.12;         // corner darkening on terrain (0 = off)

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
uniform float tile_mix = 0.7;      // tile weight over the flat biome color at full detail
uniform float biome_res = 128.0;   // cells per axis, matches terrain_ids size
uniform float tile_variants = 3.0; // keep in sync with terrain_sprites.gd VARIANTS
uniform float atlas_cols = 8.0;    // keep in sync with terrain_sprites.gd ATLAS_COLS
uniform float atlas_cell_px = 16.0;
uniform float atlas_px = 128.0;

// Chunk addressing (D3, `docs/superpowers/specs/
// 2026-09-12-pixel-world-at-scale-design.md` §4/§6 Phase 2): a streamed
// GroundChunk shows a 66x66-cell apron texture through its interior 64x64
// region, so its UV does not span `biome_res` cells the way the whole-world
// sprite's does. `ids_cells > 0.5` switches the cell lookup to
// `UV * ids_cells + ids_offset` (chunk: ids_cells = 64, ids_offset = (1,1),
// biome_res overridden to 66 so `terrain_ids` sampling lands in the apron
// texture); at the defaults below (`ids_cells = 0`) the lookup is exactly
// the original `UV * biome_res` whole-world path. `cell_origin` is this
// texture's local (0,0) cell in WORLD cell coordinates, added back in for
// the per-cell hash only, so a chunk's tile variants match the whole-world
// hash and never drift at a chunk border (defaults to 0 = identity for the
// whole-world sprite, whose cell coordinates already are world coordinates).
uniform vec2 ids_offset = vec2(0.0);
uniform float ids_cells = 0.0;
uniform vec2 cell_origin = vec2(0.0);

// World-space position of this fragment. The ground is drawn as 9 separate tiles
// (origin + 8 torus wrap copies), each with its own 0..1 UV; deriving the water
// shimmer from world position instead of per-tile UV keeps the wave continuous
// across tile seams (no phase jump where copies abut).
varying vec2 world_pos;

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 0.0, 1.0)).xy;
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

void fragment() {
	// Incoming COLOR carries the node's modulate (dim/cool tint) — preserve it.
	vec4 tint = COLOR;
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

	bool water = is_water(base);

	// Coast proximity: how many neighbours differ in water-ness from center.
	float coast = 0.0;
	coast += is_water(cL) != water ? 1.0 : 0.0;
	coast += is_water(cR) != water ? 1.0 : 0.0;
	coast += is_water(cU) != water ? 1.0 : 0.0;
	coast += is_water(cD) != water ? 1.0 : 0.0;
	coast *= 0.25; // 0..1

	vec3 outc;
	if (water) {
		// Depth gradient: deeper (darker, more saturated) water in open sea,
		// brighter teal shallows toward the coast.
		vec3 deep = base * vec3(0.72, 0.82, 1.05);
		vec3 shallow = mix(base, vec3(0.30, 0.62, 0.72), 0.55);
		// Real depth: elevation (packed in alpha, C2) relative to the water line.
		// ~1 at the shoreline (elevation just below sea level), →0 in the deepest
		// basin, so open-ocean trenches read dark and shelves read bright teal.
		float elev = texture(TEXTURE, UV).a;
		// Deepest basins sit well below sea level, so raw elev/sea_level never
		// reaches 0; square it to deepen open ocean while keeping shelves bright.
		float shallowness = clamp(elev / max(sea_level, 1e-3), 0.0, 1.0);
		shallowness *= shallowness;
		outc = mix(deep, shallow, shallowness);

		// Slow directional shimmer in seamless world coordinates (continuous
		// across the 9 wrap tiles). One world-width spans the same phase count
		// the per-tile UV version used, so the look is unchanged but seam-free.
		vec2 wuv = world_pos / world_size;
		float w = sin(wuv.x * 90.0 + TIME * 0.6) * 0.5 + 0.5;
		w *= sin(wuv.y * 70.0 - TIME * 0.4) * 0.5 + 0.5;
		outc += vec3(0.05, 0.08, 0.10) * w * (0.5 + 0.5 * (1.0 - coast));

		// A crisp bright rim right at the waterline (surf).
		outc += vec3(0.18, 0.26, 0.28) * smoothstep(0.24, 0.26, coast) * (1.0 - smoothstep(0.5, 0.9, coast));
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
			vec2 cellf = ids_cells > 0.5 ? (UV * ids_cells + ids_offset) : (UV * biome_res);
			vec2 cell = floor(cellf);
			float tid = floor(texture(terrain_ids, (cell + 0.5) / biome_res).r * 255.0 + 0.5);
			if (tid > 0.5) {
				// World-space cell, for a hash that matches the whole-world
				// path exactly (cell_origin/ids_offset default to 0/(0,0)
				// there, so this is `cell` unchanged on the whole-world
				// sprite; a chunk undoes its (1,1) apron offset and adds its
				// own global cell coordinate).
				vec2 world_cell = cell_origin + cell - ids_offset;
				float h = fract(sin(dot(world_cell, vec2(127.1, 311.7))) * 43758.5453);
				float variant = min(floor(h * tile_variants), tile_variants - 1.0);
				float cidx = tid * tile_variants + variant;
				vec2 acell = vec2(mod(cidx, atlas_cols), floor(cidx / atlas_cols));
				vec2 tuv = (acell + fract(cellf)) * (atlas_cell_px / atlas_px);
				vec3 tile = texture(tile_atlas, tuv).rgb;
				float texel_w = (world_size / biome_res) / atlas_cell_px;
				float texels_per_px = fwidth(world_pos.x) / max(texel_w, 1e-4);
				float detail = smoothstep(4.5, 1.2, texels_per_px);
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
		vec2 grad = vec2(eR - eL, eD - eU);
		float shade = dot(normalize(light_dir), grad) * relief_strength;
		// Brighten slopes facing the light, darken those facing away.
		outc = land_base * (1.0 + clamp(shade, -0.5, 0.5));

		// Sandy shoreline on the land side: a thin warm beach band right at the
		// waterline, fading inland, so coasts read as coast rather than a hard
		// color cut. Kept subtle and tinted to the cell so it reads under any
		// biome (grass/forest/desert).
		vec3 sand = vec3(0.80, 0.73, 0.52);
		outc = mix(outc, sand, coast * 0.40);

		// Slightly deepen the shoreline on the land side just past the beach so
		// the transition has a little depth.
		outc = mix(outc, outc * 0.86, max(coast - 0.25, 0.0) * 0.5);
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
