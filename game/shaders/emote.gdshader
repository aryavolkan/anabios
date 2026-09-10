shader_type canvas_item;

// Emote pictograms (emote_sprites.gd atlas) floated above acting agents.
// One MultiMesh (emote_layer.gd) draws every emote; per-instance state rides
// INSTANCE_CUSTOM:
//   r = kind / kind_scale (emote_sprites enum: 1 Zzz, 2 heart, 3 droplet,
//       4 alert, 5 star)
//   g = per-agent phase (0..1) so a sleeping herd's Zzz drift out of step
//   b = presence weight (0..1, eased CPU-side) — emotes grow/fade in and out
//       instead of popping with the action threshold
//   a = final draw alpha (presence weight x the kind's animated fade, all
//       computed CPU-side in emote_layer.gd)
// The glyph cells live in the same square 64x64 / 16px-cell grid as the
// figure atlases (Metal-safe shape, pre-flipped for the QuadMesh V axis).
//
// Fade plumbing — hard-won Metal constraint: in this shader's varying
// layout, EVERY channel except the single flat `anim` varying reaches the
// fragment corrupted — a second flat varying, a vertex write into anim.a or
// COLOR.a, and even the untouched built-in COLOR itself all arrive as
// garbage (probes showed COLOR containing a mini render of the atlas). So
// the fragment must not read COLOR at all: the glyphs are self-coloured and
// the alpha rides INSTANCE_CUSTOM.a, written CPU-side. The vertex stage and
// INSTANCE_CUSTOM passthrough are the only proven-safe data paths.

const float ATLAS_COLS = 4.0;
const float ATLAS_PX = 64.0;
const float CELL_PX = 16.0;

// Divisor emote_layer.gd uses to fit the kind id into the custom-data
// channel (emote_sprites.KIND_SCALE); a uniform so encode/decode stay paired.
uniform float kind_scale = 8.0;
// Driven by main.gd's clock so emotes freeze with the simulation.
uniform float animation_time = 0.0;

varying flat vec4 anim;

void vertex() {
	anim = INSTANCE_CUSTOM;
	float kind = anim.r * kind_scale;
	float phase = anim.g * 6.2831853;
	float w = anim.b;
	// Grow in from 60% size as the presence weight eases up, so an emote
	// blooms above the agent rather than blinking on.
	VERTEX *= 0.6 + 0.4 * w;
	// Gentle shared hover; each kind layers its own character on top.
	VERTEX.y += sin(animation_time * 5.0 + phase) * 0.03;
	if (kind > 0.5 && kind < 1.5) {
		// Zzz: a slow upward drift (the matching near-the-top alpha fade is
		// computed CPU-side into the instance colour, same cycle constants).
		float c = fract(animation_time * 0.30 + anim.g);
		VERTEX.y += c * 0.22;
	} else if (kind > 1.5 && kind < 2.5) {
		// Heart: a soft double-thump pulse.
		float p = pow(max(sin(animation_time * 6.9 + phase), 0.0), 2.0);
		VERTEX *= 1.0 + 0.10 * p;
	} else if (kind > 2.5 && kind < 3.5) {
		// Droplet: dips like a sip being taken.
		VERTEX.y -= pow(max(sin(animation_time * 5.6 + phase), 0.0), 1.5) * 0.05;
	} else if (kind > 3.5 && kind < 4.5) {
		// Alert: urgent side-to-side shiver plus a sharp pulse.
		VERTEX.x += sin(animation_time * 40.0 + phase * 2.7) * 0.03;
		VERTEX *= 1.0 + 0.06 * max(sin(animation_time * 9.0 + phase), 0.0);
	} else if (kind > 4.5) {
		// Star: twinkle by squashing one axis against the other (the alpha
		// half of the twinkle rides the CPU-side instance colour).
		float tw = sin(animation_time * 8.0 + phase);
		VERTEX.x *= 1.0 + 0.08 * tw;
		VERTEX.y *= 1.0 - 0.08 * tw;
	}
}

void fragment() {
	float kind = floor(anim.r * kind_scale + 0.5);
	float col = mod(kind, ATLAS_COLS);
	float row = floor(kind / ATLAS_COLS);
	vec2 cell_uv = (vec2(col + UV.x, row + UV.y)) * (CELL_PX / ATLAS_PX);
	vec4 t = texture(TEXTURE, cell_uv);
	COLOR = vec4(t.rgb, t.a * anim.a);
}
