extends RefCounted

# Codex event type -> world visual effects, consumed by viewer_effects. Data,
# not behavior: each entry lists effect specs applied where the event fired.
# Kinds: "fire" (ember burst + flickering light, the original fire-kind look),
# "ring" (expanding pulse, color/dur/radius), "motes" (tinted rising sparks),
# "trauma" (camera shake; the only kind that fires for loc-less events).
# Ids index codex_panel's CHAPTER_NAMES; hues echo its timeline colors so an
# event reads the same in the world as in the codex.

const FX: Dictionary = {
	0:  # Extinction — slow dark-red shockwave, felt everywhere
	[
		{"kind": "ring", "color": Color(0.85, 0.25, 0.25, 0.75), "dur": 2.6, "radius": 120.0},
		{"kind": "trauma", "amount": 0.12},
	],
	1:  # PopCrash
	[
		{"kind": "ring", "color": Color(1.0, 0.55, 0.3, 0.7), "dur": 2.0, "radius": 90.0},
		{"kind": "trauma", "amount": 0.08},
	],
	2: [{"kind": "ring", "color": Color(0.55, 0.85, 1.0, 0.6), "dur": 1.6, "radius": 70.0}],
	3: [{"kind": "ring", "color": Color(0.65, 0.75, 1.0, 0.5), "dur": 1.6, "radius": 80.0}],
	4: [{"kind": "fire"}],  # NovelModule
	7:  # CombatRaid — keeps its shake, gains a sharp local ring
	[
		{"kind": "trauma", "amount": 0.25},
		{"kind": "ring", "color": Color(1.0, 0.4, 0.25, 0.6), "dur": 0.9, "radius": 55.0},
	],
	11: [{"kind": "ring", "color": Color(0.75, 0.6, 1.0, 0.55), "dur": 1.4, "radius": 60.0}],
	12:  # MemeSweep — a dialect ring that actually travels
	[
		{"kind": "ring", "color": Color(0.75, 0.6, 1.0, 0.6), "dur": 1.8, "radius": 95.0},
		{"kind": "motes", "color": Color(0.8, 0.65, 1.0, 0.9)},
	],
	17: [{"kind": "fire"}, {"kind": "motes", "color": Color(1.0, 0.85, 0.4, 0.9)}],  # Discovery
	18: [{"kind": "motes", "color": Color(1.0, 0.85, 0.45, 0.8)}],  # Adoption
	21: [{"kind": "ring", "color": Color(1.0, 0.8, 0.45, 0.5), "dur": 1.2, "radius": 50.0}],
	22: [{"kind": "motes", "color": Color(1.0, 0.75, 0.35, 0.8)}],  # MaterialLearn
	35: [{"kind": "fire"}],  # ToolUse
	38:  # War
	[
		{"kind": "trauma", "amount": 0.25},
		{"kind": "ring", "color": Color(1.0, 0.35, 0.25, 0.65), "dur": 1.6, "radius": 110.0},
	],
	39: [{"kind": "ring", "color": Color(0.95, 0.97, 1.0, 0.5), "dur": 2.0, "radius": 90.0}],
	42:  # Settlement — hearth fire plus a founding ring
	[
		{"kind": "fire"},
		{"kind": "ring", "color": Color(1.0, 0.75, 0.4, 0.5), "dur": 1.8, "radius": 70.0},
	],
	43: [{"kind": "fire"}],  # Market
	45: [{"kind": "ring", "color": Color(0.5, 0.9, 0.8, 0.55), "dur": 1.5, "radius": 65.0}],
	47:  # Ratchet
	[
		{"kind": "ring", "color": Color(1.0, 0.85, 0.4, 0.6), "dur": 1.6, "radius": 75.0},
		{"kind": "motes", "color": Color(1.0, 0.85, 0.4, 0.9)},
	],
	51: [{"kind": "motes", "color": Color(0.6, 0.9, 0.55, 0.8)}],  # Domesticated
	53:  # Knowledge
	[
		{"kind": "ring", "color": Color(1.0, 0.85, 0.4, 0.6), "dur": 2.0, "radius": 100.0},
		{"kind": "motes", "color": Color(1.0, 0.85, 0.4, 0.9)},
	],
	54:  # MassFright
	[
		{"kind": "trauma", "amount": 0.1},
		{"kind": "ring", "color": Color(0.95, 0.95, 0.9, 0.45), "dur": 0.8, "radius": 60.0},
	],
	55:  # PanicCascade
	[
		{"kind": "trauma", "amount": 0.18},
		{"kind": "ring", "color": Color(0.95, 0.95, 0.9, 0.5), "dur": 1.0, "radius": 90.0},
	],
	58: [{"kind": "ring", "color": Color(0.6, 0.65, 0.8, 0.5), "dur": 2.8, "radius": 80.0}],
	60: [{"kind": "ring", "color": Color(0.85, 0.7, 0.45, 0.55), "dur": 1.8, "radius": 75.0}],
	61:  # Epidemic
	[
		{"kind": "ring", "color": Color(0.55, 0.85, 0.35, 0.6), "dur": 2.2, "radius": 95.0},
		{"kind": "motes", "color": Color(0.55, 0.85, 0.35, 0.85)},
	],
	62: [{"kind": "ring", "color": Color(0.7, 0.9, 1.0, 0.55), "dur": 1.6, "radius": 70.0}],
}

# Events whose `value` field carries an invention id (codex Discovery,
# Adoption, MaterialLearning), and the per-invention mote tint applied then.
# Ids mirror invention/mod.rs; hues echo the coevolution panel's series
# palette so a breakthrough reads the same in the world as in the charts.
const INVENTION_VALUE_EVENTS := [17, 18, 22]
const INVENTION_MOTES := {
	0: Color(0.7, 0.7, 0.75),  # stone tools
	1: Color(1.0, 0.5, 0.3),  # fire
	2: Color(0.5, 0.9, 0.4),  # farming
	3: Color(0.75, 0.65, 0.5),  # metalworking
	4: Color(1.0, 0.85, 0.4),  # writing
	5: Color(0.5, 1.0, 0.7),  # medicine
	6: Color(0.85, 0.7, 0.4),  # husbandry
	7: Color(0.9, 0.55, 0.3),  # machinery
	8: Color(1.0, 0.95, 0.5),  # electricity
	9: Color(0.65, 0.5, 1.0),  # nuclear
	10: Color(0.8, 0.6, 0.4),  # hafted spears
	11: Color(0.6, 0.85, 0.95),  # archery
	12: Color(0.7, 0.75, 0.55),  # fortifications
	13: Color(0.85, 0.5, 0.55),  # steel arms
}


static func spec(event_type: int) -> Array:
	return FX.get(event_type, [])


# spec(), with the event's `value` payload applied: an invention-carrying
# event recolors its motes to that invention's tint (alpha kept from the
# authored spec). Any other event, an unknown id, or the caller's -1 "no
# value" sentinel falls through to the plain table.
static func spec_with_value(event_type: int, value: float) -> Array:
	var base := spec(event_type)
	if not INVENTION_VALUE_EVENTS.has(event_type):
		return base
	var tint: Color = INVENTION_MOTES.get(roundi(value), Color(0, 0, 0, 0))
	if tint.a == 0.0:
		return base
	var out: Array = []
	for s in base:
		var d: Dictionary = s.duplicate()
		if d["kind"] == "motes":
			var a: float = (d["color"] as Color).a
			d["color"] = Color(tint.r, tint.g, tint.b, a)
		out.append(d)
	return out
