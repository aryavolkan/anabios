shader_type canvas_item;

// Field-agent animation. One MultiMesh per species; each draws that species'
// own 7-pose atlas (ape_sprites.build_species_atlas, full-colour zone-painted
// figure: 4 gait rows, then eat/fight/trade stills) and passes per-instance
// state via INSTANCE_CUSTOM, written each tick in main.gd::_refresh_bodies:
//   r = phase (0..1, derived from position so alive-index reshuffles don't pop)
//   g = moving (1 walking, 0 idle)
//   b = face_left (1 mirrors the sprite horizontally)
//   a = action / 4 (0 none, 1 eat, 2 fight, 3 trade, 4 flee — sim signals)
// Moving agents cycle contact -> passing -> counter-contact -> passing (a
// step-lift-step gait with the bob baked into the passing pose); idle agents
// hold the neutral pose and get a gentle procedural bob, so even a settled
// cluster breathes. An acting agent holds its action still instead.

uniform int frames = 8;
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
	// Squash & stretch synced to the stride: compress at the contact poses,
	// stretch through the passing pose, so the gait has weight.
	float sq = sin(TIME * 6.2831853 * walk_fps * 0.5 + phase) * anim.g;
	VERTEX.y *= 1.0 - 0.05 * sq;
	VERTEX.x *= 1.0 + 0.04 * sq;
}

void fragment() {
	float u = mix(UV.x, 1.0 - UV.x, anim.b);
	float act = anim.a * 4.0;
	float fr;
	if (act > 0.5) {
		// Action stills live below the gait rows: 4 eat, 5 fight, 6 trade, 7 flee.
		fr = 3.0 + act;
	} else {
		// Gait: s steps through 0..3 at walk_fps (phase-offset so a crowd
		// doesn't march in lockstep); map to poses 1,2,3,2 = contact,
		// passing, contact, passing. Idle agents hold pose 0 (neutral).
		float s = mod(floor(TIME * walk_fps + anim.r * 4.0), 4.0);
		fr = mix(0.0, 1.0 + min(s, 4.0 - s), anim.g);
	}
	// Frames stack vertically in the strip; cells are pre-flipped (the QuadMesh
	// V axis is flipped in the 2D canvas), so a plain sample draws the figure
	// upright.
	COLOR = texture(TEXTURE, vec2(u, (fr + UV.y) / float(frames))) * COLOR;
}
