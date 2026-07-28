shader_type canvas_item;

// Field-agent animation. One MultiMesh per species; each draws that species'
// own 4-pose atlas (ape_sprites.build_species_atlas, full-colour zone-painted
// figure) and passes per-instance state via INSTANCE_CUSTOM, written each tick
// in main.gd::_refresh_bodies:
//   r = phase (0..1, derived from position so alive-index reshuffles don't pop)
//   g = moving (1 walking, 0 idle)
//   b = face_left (1 mirrors the sprite horizontally)
// Moving agents cycle contact -> passing -> counter-contact -> passing (a
// step-lift-step gait with the bob baked into the passing pose); idle agents
// hold the neutral pose and get a gentle procedural bob, so even a settled
// cluster breathes.

uniform int frames = 4;
// Walk cadence (frames/sec) and idle bob (Hz, amplitude in sprite heights).
uniform float walk_fps = 5.0;
uniform float bob_hz = 1.4;
uniform float bob_amp = 0.04;

varying flat vec4 anim;

void vertex() {
	anim = INSTANCE_CUSTOM;
	float phase = anim.r * 6.2831853;
	// Walking: the bob is baked into the passing pose, so only a small bounce
	// here. Idle: slow breath-like bob with a hint of lateral sway. The
	// QuadMesh is a 3D mesh, so its local +Y points up-screen in the 2D
	// canvas: +VERTEX.y lifts the figure.
	float hz = mix(bob_hz, walk_fps * 0.5, anim.g);
	float amp = mix(bob_amp, bob_amp * 0.5, anim.g);
	// The quad mesh is 1x1 in local space; the per-instance transform scales it
	// to world size, so a local-unit offset scales with the body.
	VERTEX.y += sin(TIME * 6.2831853 * hz + phase) * amp;
	VERTEX.x += cos(TIME * 6.2831853 * hz * 0.5 + phase) * amp * 0.4 * (1.0 - anim.g);
}

void fragment() {
	float u = mix(UV.x, 1.0 - UV.x, anim.b);
	// Gait: s steps through 0..3 at walk_fps (phase-offset so a crowd doesn't
	// march in lockstep); map to poses 1,2,3,2 = contact, passing, contact,
	// passing. Idle agents hold pose 0 (neutral) — the bob still moves.
	float s = mod(floor(TIME * walk_fps + anim.r * 4.0), 4.0);
	float fr = mix(0.0, 1.0 + min(s, 4.0 - s), anim.g);
	// Frames stack vertically in the strip; cells are pre-flipped (the QuadMesh
	// V axis is flipped in the 2D canvas), so a plain sample draws the figure
	// upright.
	COLOR = texture(TEXTURE, vec2(u, (fr + UV.y) / float(frames))) * COLOR;
}
