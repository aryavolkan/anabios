shader_type canvas_item;

// Field-agent animation. One MultiMesh per species; each draws that species'
// own pose atlas (ape_sprites.build_species_atlas, full-colour zone-painted
// figure: 4 gait poses, then TWO frames per action — eat/fight/trade/flee,
// sleep, celebrate and the ape-only weapon pairs) and passes per-instance
// state via INSTANCE_CUSTOM,
// written each tick in main.gd::_refresh_bodies:
//   r = gait cycle position (0..1 = one contact -> passing -> contact ->
//       passing loop). Advanced CPU-side by the distance the body actually
//       covers on screen, so the stride paces with the ground instead of
//       ticking on a clock; frozen while an agent stands still, where it
//       doubles as that agent's stable idle-bob offset.
//   g = walk weight (1 walking, 0 idle, blended across the transition so the
//       secondary motion eases in; the pose index thresholds it at 0.5)
//   b = face_left (smoothed 0..1; mid-flip the mirror mix squashes the sprite
//       horizontally, so turns read as a weight shift instead of a snap)
//   a = action / act_scale (0 none, 1 eat, 2 fight, 3 trade, 4 flee,
//       5 sleep, 6 drink, 7 mate, 8 scan, 9 celebrate, 10 spear, 11 bow,
//       12 steel — sim signals: strike hotspots, fire_intent, the mood
//       column, and the held-invention mask that upgrades a fight to the
//       wielder's most advanced weapon)
// Moving agents cycle contact -> passing -> counter-contact -> passing (a
// step-lift-step gait with the bob baked into the passing pose), paced by the
// distance travelled so feet don't slide; idle agents hold the neutral pose
// and get a gentle procedural bob, so even a settled cluster breathes. An
// acting agent loops its two action frames instead, and the vertex stage adds
// a fight lunge / flee tremble on top.

uniform int frames = 16;
// Atlas layout: poses live in a SQUARE grid, atlas_cols per row (see
// ape_sprites.gd — an extreme-aspect strip corrupts on Metal's MultiMesh2D
// canvas path). A pose index `fr` maps to cell (fr % atlas_cols, fr / atlas_cols).
// Hero atlas by default (ape_sprites.gd HERO_PX / HERO_ATLAS_PX); main.gd
// sets these per material so the shader and the packer cannot drift.
uniform float atlas_cols = 8.0;
uniform float atlas_px = 192.0;
uniform float cell_px = 24.0;
// Combined atlas (mammal_sprites.combined_atlas): every bucket's 192 px
// atlas packed into a bucket_cols x bucket_cols grid, so all figures share
// ONE MultiMesh and y-sort across species. The bucket comes from the
// instance colour's alpha (mammal_sprites.bucket_alpha); its rig kind from
// rig_kinds[bucket]. With bucket_cols = 1 the atlas is a single bucket.
uniform float bucket_cols = 1.0;
uniform int rig_kinds[16];
// Authored walk cadence (frames/sec). The walk cycle itself is now paced by
// distance travelled (see anim.r above), so this only sets the flee gallop's
// bounding rate; main.gd reuses the same per-bucket value to scale stride
// length, which is where the authored per-species character now lives.
uniform float walk_fps = 5.0;
// Idle bob (Hz, amplitude in sprite heights).
uniform float bob_hz = 1.4;
uniform float bob_amp = 0.04;
// Action loop cadence: each action's A/B frames swap at this rate.
uniform float action_fps = 4.0;
// Small physical accents layered over the pixel poses. Keeping these in the
// vertex stage preserves the crisp atlas while making a crowd feel alive.
uniform float action_breath_amp = 0.035;
uniform float action_squash = 0.06;
// Driven by main.gd so character animation freezes with the simulation.
uniform float animation_time = 0.0;
// Signature-move family (mammal_sprites.RigKind): 0 prey, 1 predator,
// 2 primate (default = today's apes, all new branches off), 3 livestock.
uniform int rig_kind_default = 2;
// Divisor main.gd uses to fit the action id into the 0..1 custom-data
// channel (main.gd ACT_SCALE). Driven as a uniform so the encode and decode
// cannot drift apart when a new action pushes the scale up.
uniform float act_scale = 8.0;

varying flat vec4 anim;
// Per-instance colour (species coat tint, [C] overlay tint, fade alpha),
// carried from the vertex stage. Reading the built-in COLOR in fragment()
// is NOT safe on this MultiMesh2D path: on Metal (and the MoltenVK/ANGLE
// paths on macOS) it collapses UV to the whole texture, so every body drew
// the full pose atlas instead of one cell. Same corruption family as the
// emote shader's INSTANCE_CUSTOM.a workaround; a flat varying survives.
varying flat vec4 tint;
varying flat int bucket;
// 1 when the figure stands on a water cell (mammal_sprites.bucket_alpha):
// the fragment stage cuts it off at the waterline.
varying flat int wading;
// Fraction of the cell (from the feet) under water for a wading figure.
uniform float waterline = 0.3;

void vertex() {
	anim = INSTANCE_CUSTOM;
	tint = vec4(COLOR.rgb, 1.0);
	int slots = int(bucket_cols * bucket_cols);
	// Alpha: lower half names the bucket, upper half the same bucket wading.
	float a2 = COLOR.a * 2.0;
	wading = a2 >= 1.0 ? 1 : 0;
	bucket = clamp(int(floor(fract(a2) * float(slots))), 0, slots - 1);
	int rig_kind = bucket_cols > 1.5 ? rig_kinds[bucket] : rig_kind_default;
	float phase = anim.r * 6.2831853;
	float act = anim.a * act_scale;
	// The QuadMesh is a 3D mesh, so its local +Y points up-screen in the 2D
	// canvas: +VERTEX.y lifts the figure. The mesh is 1x1 in local space and
	// the per-instance transform scales it to world size, so a local-unit
	// offset scales with the body.
	// Idle: a slow breath-like bob on the clock, offset by the (frozen) cycle
	// position so a settled cluster doesn't breathe in unison, plus a hint of
	// lateral sway. Walking: the bob is mostly baked into the passing pose, so
	// only a small bounce here — and it rides the stride itself (two contacts
	// per loop, body lowest as a foot plants) rather than the clock, so it
	// stays locked to the feet at whatever speed the agent is travelling.
	float idle_y = sin(animation_time * 6.2831853 * bob_hz + phase) * bob_amp;
	float walk_y = -sin(phase * 2.0) * bob_amp * 0.5;
	VERTEX.y += mix(idle_y, walk_y, anim.g);
	VERTEX.x += cos(animation_time * 6.2831853 * bob_hz * 0.5 + phase) * bob_amp * 0.4 * (1.0 - anim.g);
	// Action poses get a slow, offset weight shift instead of freezing between
	// their two atlas frames. The phase keeps nearby agents from breathing in
	// perfect lockstep, while the action id changes the character of the motion.
	if (act > 0.5) {
		float action_wave = sin(animation_time * 6.2831853 * (action_fps * 0.5) + phase);
		VERTEX.y += action_wave * action_breath_amp;
		VERTEX.x += cos(animation_time * 6.2831853 * (action_fps * 0.25) + phase) * action_breath_amp * 0.35;
	}
	// Squash & stretch synced to the stride: compress at the contact poses,
	// stretch through the passing pose, so the gait has weight. Driven by the
	// same cycle position as the frame index, so the weight lands with the
	// feet however fast the body is moving.
	float sq = sin(phase * 2.0) * anim.g;
	VERTEX.y *= 1.0 - 0.05 * sq;
	VERTEX.x *= 1.0 + 0.04 * sq;
	// Eat (act 1): the atlas alternates reach-down and chomp, which on its own
	// reads as a flat two-frame flicker. Press the body down into each chomp
	// and let it spring back, so the bite has somewhere to land.
	if (act > 0.5 && act < 1.5) {
		float c = fract(animation_time * action_fps * 0.5 + anim.r * 2.0);
		float chew = pow(max(sin(c * 6.2831853), 0.0), 0.7);
		VERTEX.y -= chew * 0.05;
		VERTEX.y *= 1.0 - chew * action_squash * 0.8;
		VERTEX.x *= 1.0 + chew * action_squash * 0.5;
	// Fight/attack (act 2): primate lunges; predators add a forward pounce arc.
	} else if (act > 1.5 && act < 2.5) {
		float c = fract(animation_time * action_fps * 0.5 + anim.r * 2.0);
		float lunge = pow(max(sin(c * 3.14159265), 0.0), 1.5);
		float dir = mix(1.0, -1.0, step(0.5, anim.b));
		// A brief compression before the strike gives the lunge a readable
		// anticipation beat without needing another atlas pose.
		float prep = smoothstep(0.0, 0.22, c) * (1.0 - smoothstep(0.22, 0.42, c));
		VERTEX.x *= 1.0 - prep * action_squash;
		VERTEX.y *= 1.0 + prep * action_squash;
		VERTEX.x += dir * lunge * 0.10;
		if (rig_kind == 1) {          // PREDATOR pounce: lift through the leap
			VERTEX.y += lunge * 0.12;
			VERTEX.x += dir * lunge * 0.06;
		}
	// Trade (act 3): one reach per loop, leaning out toward the partner the
	// figure is facing and settling back, so the offer carries across rather
	// than the arm just twitching in place.
	} else if (act > 2.5 && act < 3.5) {
		float c = fract(animation_time * action_fps * 0.5 + anim.r * 2.0);
		float reach = sin(c * 3.14159265);
		float dir = mix(1.0, -1.0, step(0.5, anim.b));
		VERTEX.x += dir * reach * 0.06;
		VERTEX.y += reach * 0.02;
	} else if (act > 3.5 && act < 4.5) {  // Flee/gallop (act 4)
		if (rig_kind == 0 || rig_kind == 1) {
			// Quadruped gallop: stretch the body along travel + a bounding hop
			// (prey bound higher than predators chase). Use the distance-driven
			// gait phase so the hop stays locked to the feet at any sim speed.
			float g = anim.r * 6.2831853;
			VERTEX.x *= 1.0 + 0.10 * abs(sin(g));
			float hop = (rig_kind == 0) ? 0.10 : 0.05;
			VERTEX.y += max(sin(g), 0.0) * hop;
		} else {
			// Biped flee: a forward lean sells urgency, while the small tremble
			// keeps the pose lively without making the sprite unreadable.
			float dir = mix(1.0, -1.0, step(0.5, anim.b));
			VERTEX.x += dir * 0.045;
			VERTEX.y += max(sin(anim.r * 6.2831853), 0.0) * 0.035;
			VERTEX.x += sin(animation_time * 43.0 + phase) * 0.03;
			VERTEX.y += cos(animation_time * 37.0 + phase) * 0.02;
		}
	// Sleep (act 5): the lying pose carries the read; on top just a slow,
	// deep breath — a gentle swell, about half the idle bob's rate, so a
	// sleeping cluster sways out of phase instead of twitching.
	} else if (act > 4.5 && act < 5.5) {
		float s = sin(animation_time * 6.2831853 * 0.35 + phase);
		VERTEX.y *= 1.0 - max(s, 0.0) * 0.04;
		VERTEX.x *= 1.0 + max(s, 0.0) * 0.05;
	// Drink (act 6): reuse the eat/graze silhouette, but make the head dip in
	// two distinct sips so a thirsty agent reads as drinking rather than eating.
	} else if (act > 5.5 && act < 6.5) {
		float sip = pow(max(sin(fract(animation_time * action_fps * 0.5 + phase) * 6.2831853), 0.0), 0.8);
		VERTEX.y -= sip * 0.045;
		VERTEX.x += cos(animation_time * 6.2831853 * (action_fps * 0.25) + phase) * 0.018;
	// Mate (act 7): a compact courtship bow, with a soft rebound so a pair
	// reads as acknowledging one another rather than idling in alert pose.
	} else if (act > 6.5 && act < 7.5) {
		float bow = pow(max(sin(fract(animation_time * action_fps * 0.35 + phase) * 6.2831853), 0.0), 1.2);
		VERTEX.y -= bow * 0.055;
		VERTEX.x += mix(1.0, -1.0, step(0.5, anim.b)) * bow * 0.025;
		VERTEX.x *= 1.0 + bow * 0.025;
	// Scan (act 8): an alert forager sweeps its attention side to side,
	// alternating a small lean so a hungry crowd feels observant, not frozen.
	} else if (act > 7.5 && act < 8.5) {
		float scan = sin(animation_time * 6.2831853 * 0.65 + phase);
		VERTEX.x += scan * 0.035;
		VERTEX.y += abs(scan) * 0.018;
		VERTEX.x *= 1.0 + abs(scan) * 0.018;
	// Celebrate (act 9): every rig has a dedicated pair; quadrupeds get a
	// generated raised-tail accent while primates use raised hands.
	} else if (act > 8.5 && act < 9.5) {
		float cheer = max(sin(fract(animation_time * action_fps * 0.5 + phase) * 6.2831853), 0.0);
		VERTEX.y += cheer * 0.06;
		VERTEX.x *= 1.0 - cheer * 0.035;
	// Spear thrust (act 10): the fight lunge, armed — a deeper coil on the
	// wind-up beat, then the body rides the shaft forward on the strike.
	} else if (act > 9.5 && act < 10.5) {
		float c = fract(animation_time * action_fps * 0.5 + anim.r * 2.0);
		float drive = pow(max(sin(c * 3.14159265), 0.0), 2.0);
		float dir = mix(1.0, -1.0, step(0.5, anim.b));
		float coil = smoothstep(0.0, 0.3, c) * (1.0 - smoothstep(0.3, 0.5, c));
		VERTEX.x *= 1.0 - coil * action_squash * 1.3;
		VERTEX.y *= 1.0 + coil * action_squash * 0.6;
		VERTEX.x += dir * drive * 0.13;
		VERTEX.y += drive * 0.02;
	// Bow (act 11): a slow lean-back settle while the draw frame holds, then
	// a sharp forward recoil snap the instant the loose frame lands.
	} else if (act > 10.5 && act < 11.5) {
		float c = fract(animation_time * action_fps * 0.5 + anim.r * 2.0);
		float dir = mix(1.0, -1.0, step(0.5, anim.b));
		float draw_lean = smoothstep(0.0, 0.45, c) * (1.0 - step(0.5, c));
		float loose = step(0.5, c) * pow(1.0 - fract(c * 2.0), 2.0);
		VERTEX.x -= dir * draw_lean * 0.05;
		VERTEX.y *= 1.0 - draw_lean * 0.03;
		VERTEX.x += dir * loose * 0.06;
	// Steel swing (act 12): the blade rises on a slow wind-up stretch, then
	// the whole body drops and drives forward through the cut — heavier
	// anticipation and follow-through than the fist or spear.
	} else if (act > 11.5 && act < 12.5) {
		float c = fract(animation_time * action_fps * 0.5 + anim.r * 2.0);
		float dir = mix(1.0, -1.0, step(0.5, anim.b));
		float windup = smoothstep(0.0, 0.4, c) * (1.0 - step(0.5, c));
		float cut = step(0.5, c) * pow(1.0 - fract(c * 2.0), 1.5);
		VERTEX.y *= 1.0 + windup * action_squash;
		VERTEX.x *= 1.0 - windup * action_squash * 0.7;
		VERTEX.x += dir * cut * 0.14;
		VERTEX.y -= cut * 0.04;
	}
	// Livestock idle: a slow chewing head-bob (only while standing).
	if (rig_kind == 3 && anim.g < 0.5 && act < 0.5) {
		VERTEX.y += sin(animation_time * 3.0 + phase) * 0.02;
	}
	// Quadruped idle tail/rear sway: the trailing (left) columns drift so a
	// settled animal isn't frozen. UV.x < 0.5 is the rear before the facing
	// mirror; scale by (1 - moving) so it only shows at rest.
	if ((rig_kind == 0 || rig_kind == 1) && act < 0.5) {
		float rear = clamp(0.5 - UV.x, 0.0, 0.5) * 2.0;
		VERTEX.x += sin(animation_time * 2.2 + phase) * 0.03 * rear * (1.0 - anim.g);
	}
}

void fragment() {
	// Fractional facing eases the mirror; at b=0.5 (mid-flip) the UV mix
	// collapses to the centre column — a free horizontal flip-squash.
	float u = mix(UV.x, 1.0 - UV.x, anim.b);
	float act = anim.a * act_scale;
	float fr;
	if (act > 0.5) {
		// Action pairs live below the gait rows: eat 4/5, fight 6/7, trade
		// 8/9, flee 10/11, sleep 12/13, celebrate 14/15, spear 16/17, bow
		// 18/19, steel 20/21; swap A/B at action_fps, phase-offset so a
		// trading cluster doesn't chomp in lockstep.
		float a2 = floor(mod(animation_time * action_fps + anim.r * 2.0, 2.0));
		// Drink has no dedicated atlas cells: frame 4/5 is the compatible
		// eat/graze pair; courtship uses frame 8/9, the trade/alert pair.
		float pose_act = act;
		if (act > 5.5 && act < 6.5) {
			pose_act = 1.0;
		} else if (act > 9.5) {
			pose_act = act - 3.0;  // spear 10 -> 16/17, bow 11 -> 18/19, steel 12 -> 20/21
		} else if (act > 8.5) {
			pose_act = 6.0;
		} else if (act > 6.5) {
			pose_act = 3.0;
		}
		fr = 2.0 + pose_act * 2.0 + a2;
	} else {
		// Gait: anim.r is the distance-driven cycle position, so s steps
		// through 0..3 once per stride; map to poses 1,2,3,2 = contact,
		// passing, contact, passing. Idle agents hold pose 0 (neutral).
		// Crowds de-sync because each agent seeds and accumulates its own
		// cycle, not because of a clock offset.
		float s = mod(floor(anim.r * 4.0), 4.0);
		// anim.g is a blended walk weight, but the pose index has to stay a
		// whole number — a fractional `fr` would sample between atlas cells.
		// Threshold it here and let the vertex stage use the blend.
		fr = mix(0.0, 1.0 + min(s, 4.0 - s), step(0.5, anim.g));
	}
	// Frames live in a square grid, atlas_cols per row; cells are pre-flipped
	// (the QuadMesh V axis is flipped in the 2D canvas), so a plain sample
	// draws the figure upright. Map fr -> (col, row) and offset into its cell.
	float col = mod(fr, atlas_cols);
	float row = floor(fr / atlas_cols);
	vec2 cell_uv = (vec2(col + u, row + UV.y)) * (cell_px / atlas_px);
	// Into the bucket's cell of the combined atlas.
	vec2 origin = vec2(float(bucket % int(bucket_cols)), floor(float(bucket) / bucket_cols));
	cell_uv = (origin + cell_uv) / bucket_cols;
	COLOR = texture(TEXTURE, cell_uv) * tint;
	// Wading: the cell's feet are at UV.y = 0; below the waterline the body
	// is under water, and the two rows just above it take a pale wet rim
	// so the cut reads as a surface rather than a missing half.
	if (wading == 1) {
		if (UV.y < waterline) {
			discard;
		}
		float rim = 1.0 - smoothstep(0.0, 2.0 / cell_px, UV.y - waterline);
		COLOR.rgb = mix(COLOR.rgb, vec3(0.78, 0.88, 0.96), rim * 0.7 * COLOR.a);
		// A pale wake: one texel row of foam along the waterline either
		// side of the body, so the cut reads as the water's surface.
		float wake_row = step(UV.y - waterline, 1.0 / cell_px);
		float wake_span = step(abs(UV.x - 0.5), 0.34);
		float wake = wake_row * wake_span * (1.0 - step(0.5, COLOR.a));
		COLOR = mix(COLOR, vec4(0.86, 0.93, 0.98, 0.85), wake);
	}
}
