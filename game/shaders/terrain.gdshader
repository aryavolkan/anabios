shader_type canvas_item;

// Terrain presentation shader for the biome ground layer.
//
// The sim hands us a low-resolution grid of flat biome colors (nearest-neighbour
// upscaled), which reads as a hard color mosaic. This shader turns that grid into
// something that reads as *terrain*: gentle relief shading derived from color
// luminance, softened land transitions, and living water (depth gradient, coastal
// shallows, a slow animated shimmer).
//
// It is a pure presentation pass over READ-ONLY sim colors — no data is altered.
// When the ground layer is showing a DATA overlay (pheromones, optimum, markets,
// succession) rather than the biome, `biome_mode` is set to 0 and the shader
// passes the colors straight through so heatmaps stay faithful.

uniform float biome_mode = 1.0;        // 1 = terrain treatment, 0 = passthrough
uniform float relief_strength = 0.9;   // hillshade intensity on land
uniform float soften = 0.45;           // 0 = crisp blocks, 1 = fully smoothed
uniform vec2 light_dir = vec2(-0.65, -0.75); // light from upper-left

float luma(vec3 c) {
	return dot(c, vec3(0.299, 0.587, 0.114));
}

// Water biome cells are blue-dominant; land (grass/forest/desert/rock) is not.
bool is_water(vec3 c) {
	return (c.b > c.r + 0.05) && (c.b > c.g + 0.01) && (c.b > 0.18);
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
		outc = mix(deep, shallow, coast);

		// Slow directional shimmer — two low-frequency wave sets in world UV.
		float w = sin(UV.x * 90.0 + TIME * 0.6) * 0.5 + 0.5;
		w *= sin(UV.y * 70.0 - TIME * 0.4) * 0.5 + 0.5;
		outc += vec3(0.05, 0.08, 0.10) * w * (0.5 + 0.5 * (1.0 - coast));

		// A crisp bright rim right at the waterline (surf).
		outc += vec3(0.18, 0.26, 0.28) * smoothstep(0.24, 0.26, coast) * (1.0 - smoothstep(0.5, 0.9, coast));
	} else {
		// Land relief: treat luminance as pseudo-height and light it directionally.
		float hL = luma(cL);
		float hR = luma(cR);
		float hU = luma(cU);
		float hD = luma(cD);
		vec2 grad = vec2(hR - hL, hD - hU);
		float shade = dot(normalize(light_dir), grad) * relief_strength;
		// Brighten slopes facing the light, darken those facing away.
		outc = base * (1.0 + clamp(shade, -0.5, 0.5));

		// Slightly deepen the shoreline on the land side so coasts read cleanly.
		outc = mix(outc, outc * 0.82, coast * 0.5);
	}

	// Data-overlay mode (pheromone/optimum/market/succession): faithful
	// passthrough so heatmaps stay accurate; otherwise use the terrain result.
	outc = mix(c, outc, step(0.5, biome_mode));

	COLOR = vec4(clamp(outc, 0.0, 1.0), 1.0) * tint;
}
