shader_type canvas_item;

// Field-agent animation. One MultiMesh per species; each draws that species'
// own 12-pose atlas (ape_sprites.build_species_atlas, full-colour zone-painted
// figure: 4 gait rows, then TWO frames per action — eat/fight/trade/flee) and
// passes per-instance state via INSTANCE_CUSTOM, written each tick in
// main.gd::_refresh_bodies:
//   r = phase (0..1, derived from position so alive-index reshuffles don't pop)
//   g = moving (1 walking, 0 idle)
//   b = face_left (smoothed 0..1; mid-flip the mirror mix squashes the sprite
//       horizontally, so turns read as a weight shift instead of a snap)
//   a = action / 4 (0 none, 1 eat, 2 fight, 3 trade, 4 flee — sim signals)
// Moving agents cycle contact -> passing -> counter-contact -> passing (a
// step-lift-step gait with the bob baked into the passing pose); idle agents
// hold the neutral pose and get a gentle procedural bob, so even a settled
// cluster breathes. An acting agent loops its two action frames instead, and
// the vertex stage adds a fight lunge / flee tremble on top.

uniform int frames = 12;
// Walk cadence (frames/sec) and idle bob (Hz, amplitude in sprite heights).
uniform float walk_fps = 5.0;
uniform float bob_hz = 1.4;
uniform float bob_amp = 0.04;
// Action loop cadence: each action's A/B frames swap at this rate.
uniform float action_fps = 4.0;
// Signature-move family (mammal_sprites.RigKind): 0 prey, 1 predator,
// 2 primate (default = today's apes, all new branches off), 3 livestock.
uniform int rig_kind = 2;

varying flat vec4 anim;

void vertex() {
	anim = INSTANCE_CUSTOM;
	float phase = anim.r * 6.2831853;
	float act = anim.a * 4.0;
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
	// Fight/attack (act 2): primate lunges; predators add a forward pounce arc.
	if (act > 1.5 && act < 2.5) {
		float c = fract(TIME * action_fps * 0.5 + anim.r * 2.0);
		float lunge = pow(max(sin(c * 3.14159265), 0.0), 1.5);
		float dir = mix(1.0, -1.0, step(0.5, anim.b));
		VERTEX.x += dir * lunge * 0.10;
		if (rig_kind == 1) {          // PREDATOR pounce: lift through the leap
			VERTEX.y += lunge * 0.12;
			VERTEX.x += dir * lunge * 0.06;
		}
	} else if (act > 3.5) {           // Flee/gallop (act 4)
		if (rig_kind == 0 || rig_kind == 1) {
			// Quadruped gallop: stretch the body along travel + a bounding hop
			// (prey bound higher than predators chase).
			float g = TIME * walk_fps * 0.9 + anim.r * 6.2831853;
			VERTEX.x *= 1.0 + 0.10 * abs(sin(g));
			float hop = (rig_kind == 0) ? 0.10 : 0.05;
			VERTEX.y += max(sin(g), 0.0) * hop;
		} else {
			// Biped flee tremble (unchanged primate behaviour).
			VERTEX.x += sin(TIME * 43.0 + phase) * 0.03;
			VERTEX.y += cos(TIME * 37.0 + phase) * 0.02;
		}
	}
	// Livestock idle: a slow chewing head-bob (only while standing).
	if (rig_kind == 3 && anim.g < 0.5 && act < 0.5) {
		VERTEX.y += sin(TIME * 3.0 + phase) * 0.02;
	}
	// Quadruped idle tail/rear sway: the trailing (left) columns drift so a
	// settled animal isn't frozen. UV.x < 0.5 is the rear before the facing
	// mirror; scale by (1 - moving) so it only shows at rest.
	if ((rig_kind == 0 || rig_kind == 1) && act < 0.5) {
		float rear = clamp(0.5 - UV.x, 0.0, 0.5) * 2.0;
		VERTEX.x += sin(TIME * 2.2 + phase) * 0.03 * rear * (1.0 - anim.g);
	}
}

void fragment() {
	// Fractional facing eases the mirror; at b=0.5 (mid-flip) the UV mix
	// collapses to the centre column — a free horizontal flip-squash.
	float u = mix(UV.x, 1.0 - UV.x, anim.b);
	float act = anim.a * 4.0;
	float fr;
	if (act > 0.5) {
		// Action pairs live below the gait rows: eat 4/5, fight 6/7, trade
		// 8/9, flee 10/11; swap A/B at action_fps, phase-offset so a trading
		// cluster doesn't chomp in lockstep.
		float a2 = floor(mod(TIME * action_fps + anim.r * 2.0, 2.0));
		fr = 2.0 + act * 2.0 + a2;
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
