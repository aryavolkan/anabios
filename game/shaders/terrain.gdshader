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
		outc = base * (1.0 + clamp(shade, -0.5, 0.5));

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
