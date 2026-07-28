shader_type canvas_item;

// Field-agent liveliness. The agents are one MultiMesh (a single shared texture +
// material for 1000-2500 instances), so per-node animation is impossible. TEXTURE
// is the single-frame hominin mask (ape_sprites.build_field_mask). Motion is a
// pure per-instance VERTEX bounce — no atlas frame-swapping, so nothing can smear.
//
// INSTANCE_CUSTOM is written each frame in main.gd::_refresh_bodies:
//   .r = phase   (0..1, per-agent — desyncs the crowd's bounce)
//   .g = moving  (1 = walking → quick springy hop, 0 = idle → slow breath)
// COLOR carries the per-instance genome/species tint, multiplied in as before.

uniform float idle_speed = 2.2;   // breaths/sec when standing
uniform float walk_speed = 8.5;   // hops/sec when moving
uniform float idle_amp = 0.03;    // hop height (fraction of the figure) when idle
uniform float walk_amp = 0.10;    // hop height when moving

varying float v_moving;

void vertex() {
	float phase = INSTANCE_CUSTOM.r;
	v_moving = INSTANCE_CUSTOM.g;
	float speed = mix(idle_speed, walk_speed, v_moving);
	float amp = mix(idle_amp, walk_amp, v_moving);
	// abs(sin) reads as a hop (always up from the ground, snappy at the bottom).
	// VERTEX is local quad space (±0.5), so amp is a fraction of the figure height.
	float hop = abs(sin(TIME * speed + phase * 6.2831853)) * amp;
	VERTEX.y -= hop;
	// A touch of squash-&-stretch: taller at the top of the hop, squatter at the
	// bottom, pivoting on the feet so the figure never sinks through the ground.
	float stretch = 1.0 + hop * mix(0.6, 1.4, v_moving);
	VERTEX.y = (VERTEX.y - 0.5) * stretch + 0.5; // 0.5 = feet (bottom of the quad)
}

void fragment() {
	COLOR = texture(TEXTURE, UV) * COLOR;
}
