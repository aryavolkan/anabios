extends RefCounted

# Module glyph palette — indexed by module type id (0..10). Single source of
# truth shared by the body-glyph layers (main.gd) and the legend color key.
const MODULE_COLORS: PackedColorArray = [
	Color(0.6, 0.8, 1.0),  # 0 Locomotor
	Color(0.4, 0.9, 0.5),  # 1 Sensor
	Color(1.0, 0.7, 0.3),  # 2 Mouth
	Color(1.0, 0.3, 0.3),  # 3 Weapon
	Color(0.7, 0.7, 0.7),  # 4 Armor
	Color(0.9, 0.9, 0.4),  # 5 Storage
	Color(0.8, 0.5, 1.0),  # 6 Communicator
	Color(0.5, 1.0, 0.9),  # 7 Pheromone
	Color(1.0, 0.5, 0.8),  # 8 Reproductive
	Color(1.0, 0.55, 0.1),  # 9 Spines
	Color(0.85, 0.1, 0.45),  # 10 Jaws
]
const MODULE_NAMES: PackedStringArray = [
	"Locomotor",
	"Sensor",
	"Mouth",
	"Weapon",
	"Armor",
	"Storage",
	"Communicator",
	"Pheromone",
	"Reproductive",
	"Spines",
	"Jaws",
]

# --- Continuous body ramps -------------------------------------------------
# The [C] body overlays (diet / energy / arousal) shade each agent along a
# two-ended scale. A straight RGB lerp between the ends drops through
# desaturated mud in the middle — blue→yellow and green→red both pass through
# grey — so mid-scale agents rendered as grey-brown blobs indistinguishable
# from the terrain, and the legend ramp read as "no data" at its midpoint.
# Interpolating hue instead keeps the whole scale saturated and ordered.
# Endpoints are (hue, sat, val) so the path is explicit; `ramp` is the single
# source of truth for both the field colours (main.gd) and the legend key.
const RAMP_DIET: Array = [[0.33, 0.70, 0.85], [0.00, 0.75, 0.95]]  # herb → carn
const RAMP_ENERGY: Array = [[0.62, 0.70, 0.80], [0.14, 0.85, 0.98]]  # low → high
const RAMP_AROUSAL: Array = [[0.55, 0.25, 0.70], [0.03, 0.80, 1.00]]  # calm → aroused


# Sample a ramp at t in [0,1]. Hue takes the short way around the wheel, so a
# ramp never detours through the far side of the colour circle.
static func ramp(r: Array, t: float) -> Color:
	var f: float = clampf(t, 0.0, 1.0)
	var h0: float = r[0][0]
	var h1: float = r[1][0]
	if absf(h1 - h0) > 0.5:
		h1 += 1.0 if h1 < h0 else -1.0
	return Color.from_hsv(
		fposmod(lerpf(h0, h1, f), 1.0), lerpf(r[0][1], r[1][1], f), lerpf(r[0][2], r[1][2], f)
	)
