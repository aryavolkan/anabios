extends RefCounted
# Hand-authored 24 px hominin masters (the biped counterpart of
# hero_rigs.gd). One master per hominin in ApeSprites.NAMES order, facing
# right, feet on row 21, two rows of headroom for raised arms. Pixels carry
# a PART tag (head, torso, front/back arm, front/back leg) and a zone key
# resolved through the species' FIELD_ZONE_COLORS, so a chimp and a sapiens
# share the recipes but keep their own coats, skin and accents. The 22 pose
# cells the field shader expects are derived by moving parts; the spear,
# bow and steel poses draw their weapon from the front hand's position.
#
# Key legend: c torso coat / a torso accent; H head coat / S face skin /
# e eye; R front arm / r front hand; L back arm / l back hand; F front leg /
# f front foot; B back leg / b back foot.

const ApeSprites = preload("res://scripts/ape_sprites.gd")

const PX := 24
const CELL_COUNT := ApeSprites.POSE_COUNT

enum Part { BODY, HEAD, ARM_F, ARM_B, LEG_F, LEG_B }

# key -> [part, zone key of FIELD_ZONE_COLORS ("k" = fixed eye black)].
const KEYS := {
	"c": [Part.BODY, "c"],
	"a": [Part.BODY, "a"],
	"H": [Part.HEAD, "c"],
	"S": [Part.HEAD, "s"],
	"e": [Part.HEAD, "k"],
	"R": [Part.ARM_F, "c"],
	"r": [Part.ARM_F, "s"],
	"L": [Part.ARM_B, "c"],
	"l": [Part.ARM_B, "s"],
	"F": [Part.LEG_F, "c"],
	"f": [Part.LEG_F, "s"],
	"B": [Part.LEG_B, "c"],
	"b": [Part.LEG_B, "s"],
}
const DRAW_ORDER: Array = [Part.ARM_B, Part.LEG_B, Part.BODY, Part.LEG_F, Part.HEAD, Part.ARM_F]

const MASTERS: Array = [
	[
		"........................",
		"........................",
		"..........HHHHH.........",
		".........HHHHHHHH.......",
		"........HHHSSSSSH.......",
		".......HHHSSeSSSS.......",
		".......HHHSSSSSSS.......",
		"........HHSSSSSSS.......",
		".........HHHSSSS........",
		"........cccccccc........",
		".......Lcccccccc........",
		"......LLccaaaaccR.......",
		"......LLcccaacccR.......",
		"......LLccccccccRR......",
		"......LLcccccccc.RR.....",
		".......Lcccccccc..RR....",
		".......lccccccccc..r....",
		".......l.BBB.FFF...r....",
		".........BBB.FFF........",
		".........BBB.FFF........",
		".........BBB..FFF.......",
		"........bbbb..ffff......",
		"........................",
		"........................",
	],
	[
		"........................",
		"..........HHHH..........",
		".........HHHHHHH........",
		"........HHHHHHHHH.......",
		"........HHHSSSSSH.......",
		".......HHHSSeSSSSH......",
		".......HHHSSSSSSS.......",
		"........HHHSSSSS........",
		".....cccccccccccc.......",
		"....LLcccccccccccR......",
		"...LLLcccccccccccRR.....",
		"...LLLccaaaaaacccRR.....",
		"...LLLcccaaaaccc.RRR....",
		"...LLLcccccccccc.RRR....",
		"...LLL.cccccccc...RRR...",
		"...lll.cccccccc...RRR...",
		"...lll.cccccccc....rrr..",
		".......BBBB.FFFF...rrr..",
		".......BBBB.FFFF........",
		".......BBBB.FFFF........",
		".......BBBB..FFFF.......",
		"......bbbbb..fffff......",
		"........................",
		"........................",
	],
	[
		"........................",
		"........................",
		"..........HHHHH.........",
		"........HHHHHHHHH.......",
		".......HHHHSSSSSHH......",
		".......HHHSSeSSSSH......",
		".......HHHSSSSSSSH......",
		"........HHHSSSSSS.......",
		".........HHHSSSS........",
		"......ccccccccccc.......",
		".....LLcccccccccccR.....",
		"....LLLcccccccccccRR....",
		"....LLLccccaacccc.RRR...",
		"...LLL.cccccccccc..RRR..",
		"...LLL.cccccccccc..RRR..",
		"...LLL..cccccccc....RRR.",
		"...lll..cccccccc....rrr.",
		"...lll..BBBB.FFF....rrr.",
		"........BBBB.FFF........",
		"........BBBB.FFF........",
		"........BBB...FFF.......",
		".......bbbb...ffff......",
		"........................",
		"........................",
	],
	[
		"........................",
		"........................",
		"..........HHHH..........",
		".........HHHHHHH........",
		".........HHHSSSSS.......",
		"........HHHSSeSSS.......",
		"........HHHSSSSSS.......",
		".........HHSSSSS........",
		"..........HSSSS.........",
		"........cccccccc........",
		".......Lcccccccccc......",
		".......LccccccccccR.....",
		".......LcccaaacccRR.....",
		".......LccccccccccR.....",
		".......Lcccccccc..R.....",
		".......lcccccccc..r.....",
		".........cccccc.........",
		".........BBB.FFF........",
		".........BBB.FFF........",
		".........BBB.FFF........",
		".........BBB..FFF.......",
		"........bbbb..ffff......",
		"........................",
		"........................",
	],
	[
		"........................",
		"..........HHHHH.........",
		".........HHHHHHH........",
		".........HHSSSSS........",
		".........HSSeSSSS.......",
		".........HSSSSSSS.......",
		"..........SSSSSS........",
		"...........SSSS.........",
		"..........aaaaaa........",
		".........Laaaaaaaa......",
		".........LaaaaaaaaR.....",
		".........LaaaaaaaaR.....",
		".........LaaaaaaaaR.....",
		".........Laaaaaaaa.R....",
		".........laaaaaaaa.r....",
		"..........aaaaaaaa......",
		"..........cccccc........",
		"..........BBB.FFF.......",
		"..........BBB.FFF.......",
		"..........BBB.FFF.......",
		"..........BBB..FFF......",
		".........bbbb..ffff.....",
		"........................",
		"........................",
	],
]


static func has(species: int) -> bool:
	return species >= 0 and species < MASTERS.size()


# Zone key -> Color for one hominin (PAL through FIELD_ZONE_COLORS; the eye
# is fixed near-black).
static func palette(species: int) -> Dictionary:
	var zones: Dictionary = ApeSprites.FIELD_ZONE_COLORS[species]
	var out: Dictionary = {"k": Color(ApeSprites.PAL["K"])}
	for z in zones:
		out[z] = Color(ApeSprites.PAL[zones[z]])
	return out


# Master pixels by part: Part -> Array of [x, y, zone_key, is_hand].
static func parts_of(rows: Array) -> Dictionary:
	var out: Dictionary = {}
	for p in Part.values():
		out[p] = []
	for y in rows.size():
		var row: String = rows[y]
		for x in row.length():
			var ch: String = row[x]
			if ch == "." or not KEYS.has(ch):
				continue
			var spec: Array = KEYS[ch]
			out[spec[0]].append([x, y, spec[1], ch == "r"])
	return out


# Per pose: part entries [dx, dy] or, for arms, [dx, dy, degrees] (a swing
# about the shoulder; negative = forward and up, -90 straight ahead, -180
# straight up), optional "fold" (legs keep that share, the body drops),
# "eyes_closed", and "weapon" ("spear_a", ...).
static func pose_recipe(index: int) -> Dictionary:
	match index:
		1:
			return {
				Part.LEG_F: [1, 0],
				Part.LEG_B: [-1, 0],
				Part.ARM_F: [0, 0, 25],
				Part.ARM_B: [0, 0, -25],
			}
		2:
			return {
				Part.BODY: [0, -1], Part.HEAD: [0, -1], Part.ARM_F: [0, -1], Part.ARM_B: [0, -1]
			}
		3:
			return {
				Part.LEG_F: [-1, 0],
				Part.LEG_B: [1, 0],
				Part.ARM_F: [0, 0, -25],
				Part.ARM_B: [0, 0, 25],
			}
		4:
			return {Part.ARM_F: [-2, 0, -140]}
		5:
			return {Part.ARM_F: [-2, 0, -155], Part.HEAD: [0, 1]}
		6:
			return {Part.ARM_F: [-2, 0, -75]}
		7:
			return {Part.ARM_F: [-3, 0, -95], Part.BODY: [1, 0], Part.HEAD: [1, 0]}
		8:
			return {Part.ARM_F: [-2, 0, -70]}
		9:
			return {Part.ARM_F: [-2, 0, -90]}
		10:
			return {
				Part.LEG_F: [2, 0],
				Part.LEG_B: [-2, 0],
				Part.BODY: [1, -1],
				Part.HEAD: [2, -1],
				Part.ARM_F: [-1, -1, -45],
				Part.ARM_B: [1, -1, 35],
			}
		11:
			return {
				Part.LEG_F: [-1, 0],
				Part.LEG_B: [1, 0],
				Part.HEAD: [1, 0],
				Part.ARM_F: [0, 0, 25],
				Part.ARM_B: [0, 0, -40],
			}
		12:
			return {"fold": 0.35, Part.HEAD: [1, 2]}
		13:
			return {"fold": 0.35, Part.HEAD: [1, 3], "eyes_closed": true}
		14:
			return {
				Part.ARM_F: [-1, 0, -175],
				Part.ARM_B: [1, 0, 175],
				Part.BODY: [0, -1],
				Part.HEAD: [0, -1],
			}
		15:
			return {Part.ARM_F: [-1, 0, -160], Part.ARM_B: [1, 0, 160]}
		# Weapon poses: the weapon is drawn from the front hand, and the
		# arm + weapon group is nudged back into the cell when a long-armed
		# hominin would push it over the edge (see _fit).
		16:
			return {Part.ARM_F: [-1, 0, -35], "weapon": "spear_a"}
		17:
			return {
				Part.ARM_F: [-3, 0, -60], Part.BODY: [1, 0], Part.HEAD: [1, 0], "weapon": "spear_b"
			}
		18:
			return {Part.ARM_F: [-3, 0, -90], Part.ARM_B: [-1, 0, -100], "weapon": "bow_a"}
		19:
			return {Part.ARM_F: [-3, 0, -90], Part.ARM_B: [-3, 0, -80], "weapon": "bow_b"}
		20:
			return {Part.ARM_F: [-2, 0, -120], "weapon": "steel_a"}
		21:
			return {
				Part.ARM_F: [-2, 0, -40], Part.BODY: [1, 0], Part.HEAD: [1, 0], "weapon": "steel_b"
			}
		_:
			return {}


# Swing an arm's pixels about its shoulder (the centre of its top row).
# The arm is re-rasterised by inverse mapping so it stays solid at any
# angle; the hand pixel farthest from the shoulder is flagged as the tip
# (entry [x, y, zone, is_hand, is_tip]).
static func rotate_arm(pixels: Array, degrees: float) -> Array:
	if pixels.is_empty():
		return pixels
	var top := PX
	for px in pixels:
		top = mini(top, int(px[1]))
	var src: Dictionary = {}
	var sx := 0.0
	var n := 0
	for px in pixels:
		src[Vector2i(int(px[0]), int(px[1]))] = px
		if int(px[1]) == top:
			sx += float(px[0])
			n += 1
	var pivot := Vector2(sx / float(n), float(top))
	var reach := 0.0
	for px in pixels:
		reach = maxf(reach, Vector2(float(px[0]), float(px[1])).distance_to(pivot))
	var ang := deg_to_rad(degrees)
	var r: int = int(ceil(reach)) + 1
	var out: Array = []
	var tip := -1
	var tip_d := -1.0
	for y in range(int(pivot.y) - r, int(pivot.y) + r + 1):
		for x in range(int(pivot.x) - r, int(pivot.x) + r + 1):
			var here := Vector2(float(x), float(y))
			var s: Vector2 = pivot + (here - pivot).rotated(-ang)
			var key := Vector2i(int(round(s.x)), int(round(s.y)))
			if not src.has(key):
				continue
			var e: Array = src[key]
			out.append([x, y, e[2], e[3], false])
			var d := here.distance_to(pivot)
			if bool(e[3]) and d > tip_d:
				tip_d = d
				tip = out.size() - 1
	if tip >= 0:
		out[tip][4] = true
	return out


# Weapon pixels for a hand at (hx, hy): [dx, dy, zone] with zone "w" (shaft)
# or "f" (flint / blade), see FIELD_ZONE_COLORS.
static func weapon_pixels(kind: String) -> Array:
	var out: Array = []
	match kind:
		"spear_a":
			# Held upright: shaft rising from the hand, flint tip on top.
			for i in range(1, 8):
				out.append([0, -i, "w"])
			out.append([0, -8, "f"])
			out.append([0, -9, "f"])
			for i in range(1, 4):
				out.append([0, i, "w"])  # the butt below the hand
		"spear_b":
			# Thrust: the shaft leans steeply forward from the hand.
			for i in range(1, 8):
				out.append([(i + 1) / 2, -i, "w"])
			out.append([4, -8, "f"])
			out.append([5, -9, "f"])
			out.append([-1, 1, "w"])
			out.append([-1, 2, "w"])
		"bow_a", "bow_b":
			for i in range(-3, 4):
				out.append([1 if absi(i) == 3 else 0, i, "f"])  # string
			for i in range(-3, 4):
				out.append([2 if absi(i) == 3 else 3, i, "w"])  # stave
			if kind == "bow_a":
				out.append([1, 0, "f"])
				out.append([2, 0, "f"])
			else:
				for i in range(3, 7):
					out.append([i, 0, "f"])  # loosed arrow
		"steel_a":
			for i in range(-7, 0):
				out.append([1, i, "f"])
			out.append([0, -1, "w"])
			out.append([2, -1, "w"])
		"steel_b":
			# Swung forward and down: a diagonal blade with its guard at the hand.
			for i in range(1, 6):
				out.append([i, i, "f"])
			out.append([0, 1, "w"])
			out.append([1, 0, "w"])
	return out


# One pose cell (unflipped, shaded, outlined) for a hominin.
static func build_cell(species: int, index: int) -> Image:
	var rows: Array = MASTERS[species]
	var pal: Dictionary = palette(species)
	var parts: Dictionary = parts_of(rows)
	var recipe: Dictionary = pose_recipe(index)
	var img := Image.create(PX, PX, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	var drop := 0
	var fold: float = float(recipe.get("fold", 0.0))
	if fold > 0.0:
		var leg_top := PX
		var leg_bottom := 0
		for p in [Part.LEG_F, Part.LEG_B]:
			for px in parts[p]:
				leg_top = mini(leg_top, int(px[1]))
				leg_bottom = maxi(leg_bottom, int(px[1]))
		var leg_h: int = leg_bottom - leg_top + 1
		var keep: int = maxi(1, int(round(float(leg_h) * fold)))
		drop = leg_h - keep
		for p in [Part.LEG_F, Part.LEG_B]:
			var kept: Array = []
			for px in parts[p]:
				if int(px[1]) < leg_top + keep:
					kept.append(px)
			parts[p] = kept
	var eyes_closed: bool = bool(recipe.get("eyes_closed", false))
	var weapon: String = String(recipe.get("weapon", ""))
	for p in DRAW_ORDER:
		var shift: Array = recipe.get(p, [0, 0])
		var dx: int = int(shift[0])
		var dy: int = int(shift[1]) + drop
		var pixels: Array = parts[p]
		if shift.size() > 2:
			pixels = rotate_arm(pixels, float(shift[2]))
		var hand := Vector2i(-1, -1)
		var placed: Array = []
		for px in pixels:
			var zone: String = px[2]
			if eyes_closed and zone == "k":
				zone = "s"
			var x: int = int(px[0]) + dx
			var y: int = int(px[1]) + dy
			placed.append([x, y, zone])
			if not bool(px[3]):
				continue
			if px.size() > 4:
				if bool(px[4]):
					hand = Vector2i(x, y)  # the tip of a swung arm
			elif x > hand.x or (x == hand.x and y > hand.y):
				hand = Vector2i(x, y)  # a hanging arm: its lowest, foremost pixel
		if p == Part.ARM_F and weapon != "" and hand.x >= 0:
			for w in weapon_pixels(weapon):
				placed.append([hand.x + int(w[0]), hand.y + int(w[1]), w[2]])
			_fit(placed)
		for q in placed:
			_put(img, int(q[0]), int(q[1]), pal[q[2]])
	ApeSprites.shade(img, PX)
	ApeSprites.outline(img, PX)
	return img


static func _put(img: Image, x: int, y: int, col: Color) -> void:
	if x >= 0 and y >= 0 and x < PX and y < PX:
		img.set_pixel(x, y, col)


# Nudge a pixel group back inside the cell when it overflows an edge.
static func _fit(placed: Array) -> void:
	var lo := Vector2i(PX, PX)
	var hi := Vector2i(-1, -1)
	for q in placed:
		lo = Vector2i(mini(lo.x, int(q[0])), mini(lo.y, int(q[1])))
		hi = Vector2i(maxi(hi.x, int(q[0])), maxi(hi.y, int(q[1])))
	var dx: int = mini(0, PX - 1 - hi.x) if hi.x >= PX else maxi(0, -lo.x)
	var dy: int = mini(0, PX - 1 - hi.y) if hi.y >= PX else maxi(0, -lo.y)
	if dx == 0 and dy == 0:
		return
	for q in placed:
		q[0] = int(q[0]) + dx
		q[1] = int(q[1]) + dy


static func build_cells(species: int) -> Array:
	var out: Array = []
	for i in CELL_COUNT:
		out.append(build_cell(species, i))
	return out
