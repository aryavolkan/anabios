shader_type canvas_item;

// Title-screen background: a calm "primordial field" that evokes the simulation
// without needing it. The menu scene has no Simulation/biome node, so everything
// here is procedural and animated via TIME — a deep near-black teal depth
// gradient with faint luminous microorganisms drifting slowly across it, a gentle
// flowing current, and a soft vignette to frame the form. Deliberately dark and
// low-contrast so the menu controls stay clearly legible on top.
//
// Palette is tied to the shared field-instrument language (UiTheme): the base is
// BG_PANEL-deep teal, the drifting cells glow in ACCENT (0.30, 0.88, 0.70).

uniform vec3 deep = vec3(0.018, 0.036, 0.046);   // outer / edge deep teal
uniform vec3 lift = vec3(0.050, 0.098, 0.104);   // centre lift behind the form
uniform vec3 cell_color = vec3(0.30, 0.88, 0.70); // ACCENT — the drifting cells
uniform float cell_gain = 0.055;                 // overall brightness of the cells
uniform float vignette = 0.52;                   // corner framing
uniform int cell_count = 16;                     // number of drifting organisms

vec2 hash2(float n) {
	return fract(sin(vec2(n, n + 1.73)) * vec2(43758.5453, 22578.1459));
}

void fragment() {
	vec2 uv = UV;

	// Aspect-correct space so blobs stay round and the drift reads evenly on a
	// wide screen (the exact ratio only affects roundness, not layout).
	float aspect = 1.6;
	vec2 auv = vec2(uv.x * aspect, uv.y);

	// Base field: a soft radial lift toward the centre (behind the form) over a
	// deep base, plus a very slow flowing brightness so the field breathes.
	float d = distance(uv, vec2(0.5, 0.46));
	float flow = 0.5 + 0.5 * sin(uv.x * 3.0 + TIME * 0.15) * sin(uv.y * 2.3 - TIME * 0.11);
	vec3 col = mix(lift, deep, smoothstep(0.0, 0.9, d));
	col += (lift - deep) * 0.18 * flow;

	// Drifting microorganisms: soft glowing points that wander slowly and pulse,
	// each seeded from its index so the pattern is deterministic and calm.
	float glow = 0.0;
	for (int i = 0; i < cell_count; i++) {
		float fi = float(i);
		vec2 h = hash2(fi * 7.13 + 1.0);
		// Slow wandering drift; fract keeps each organism looping across the field.
		vec2 p = vec2(
			fract(h.x + TIME * (0.010 + h.y * 0.018) + 0.12 * sin(TIME * 0.20 + fi)),
			fract(h.y + TIME * (0.006 + h.x * 0.014) + 0.09 * cos(TIME * 0.17 + fi))
		);
		p.x *= aspect;
		float r = distance(auv, p);
		float size = 0.010 + 0.018 * h.x;
		float pulse = 0.65 + 0.35 * sin(TIME * 0.6 + fi * 1.7);
		// Soft core with a quick falloff so cells stay small and faint.
		glow += pulse * size / (r + 0.006) * smoothstep(0.13, 0.0, r);
	}
	col += cell_color * glow * cell_gain;

	// Vignette to frame the field and sink the corners into black.
	col *= 1.0 - vignette * pow(clamp(d * 1.35, 0.0, 1.0), 2.2);

	COLOR = vec4(col, 1.0);
}
