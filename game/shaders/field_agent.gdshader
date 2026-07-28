shader_type canvas_item;

// Field-agent animation. The bodies MultiMesh draws one walk-cycle atlas
// (ape_sprites.build_walk_atlas) and passes per-instance animation state via
// INSTANCE_CUSTOM, written each tick in main.gd::_refresh_bodies:
//   r = phase (0..1, derived from position so alive-index reshuffles don't pop)
//   g = moving (1 walking, 0 idle)
//   b = face_left (1 mirrors the sprite horizontally)
// Moving agents cycle the atlas; idle agents hold the neutral frame and get a
// gentle procedural bob, so even a stationary cluster breathes.

uniform int frames = 4;
// Walk cadence (frames/sec) and idle bob (Hz, amplitude in sprite heights).
uniform float walk_fps = 6.0;
uniform float bob_hz = 1.4;
uniform float bob_amp = 0.05;

varying flat vec4 anim;

void vertex() {
	anim = INSTANCE_CUSTOM;
	float phase = anim.r * 6.2831853;
	// Walking: smaller, faster bounce synced to the stride (two steps per walk
	// cycle => bob at half the frame rate). Idle: slow breath-like bob with a
	// hint of lateral sway. The QuadMesh is a 3D mesh, so its local +Y points
	// up-screen in the 2D canvas: +VERTEX.y lifts the figure.
	float hz = mix(bob_hz, walk_fps * 0.5, anim.g);
	float amp = mix(bob_amp, bob_amp * 0.5, anim.g);
	// The quad mesh is 1x1 in local space; the per-instance transform scales it
	// to world size, so a local-unit offset scales with the body.
	VERTEX.y += sin(TIME * 6.2831853 * hz + phase) * amp;
	VERTEX.x += cos(TIME * 6.2831853 * hz * 0.5 + phase) * amp * 0.4 * (1.0 - anim.g);
}

void fragment() {
	float u = mix(UV.x, 1.0 - UV.x, anim.b);
	// Moving: cycle all frames at walk_fps, offset by the per-agent phase so a
	// crowd doesn't march in lockstep. Idle: hold frame 0 (the bob still moves).
	float fr = mod(floor(TIME * walk_fps + anim.r * float(frames)), float(frames)) * anim.g;
	// QuadMesh UVs are V-flipped in the 2D canvas (a 3D mesh: +Y is up there,
	// down here), so sample the atlas upside-down to draw the figure upright.
	COLOR = texture(TEXTURE, vec2((fr + u) / float(frames), 1.0 - UV.y)) * COLOR;
}
