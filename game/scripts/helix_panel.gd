extends Control

# Dual-inheritance helix — the two inheritance channels of DIT drawn as a
# double helix. Left strand: genome slots (population means). Right strand:
# meme channels (adoption levels). Rungs are the couplings between them:
# solid = invention affinity (colored by the live selection differential),
# dashed = hard genetic prerequisite, thin blue = DIT learning arm.
# Toggle with [X]. Read-only.

@onready var sim = get_node("/root/Main/Simulation")

# Curated genome slots (the coupling-relevant ones): the ten invention
# affinity slots plus the DIT learning/climate slots.
const GENE_SLOTS := [28, 12, 21, 15, 23, 16, 10, 20, 6, 29, 40, 41]
# Curated meme channels: skill, DIT technique, the invention block, practices.
const MEME_CHANNELS_LIST := [5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
const INVENTION_CHANNEL_BASE := 8

const TOP := 64.0
const BOTTOM_PAD := 46.0
const TWIST_TURNS := 1.6  # sine turns across the panel height
const REFRESH_EVERY := 5

const GENE_COLOR := Color(0.35, 0.75, 1.0)
const MEME_COLOR := Color(1.0, 0.75, 0.25)
const REQ_COLOR := Color(0.72, 0.6, 1.0, 0.55)
const DIT_COLOR := Color(0.5, 0.7, 0.9, 0.4)
const POS_COLOR := Color(0.55, 0.9, 0.55)
const NEG_COLOR := Color(0.95, 0.45, 0.55)
const IDLE_COLOR := Color(0.6, 0.65, 0.7, 0.25)

var _shown: bool = false
var _font: Font
var _frame: int = 0
var _snap: Dictionary = {}
# Static rung list built once from the catalogs:
#   {gene_slot: int, kind: "affinity"|"req"|"dit", inv: int (-1 for dit)}
var _edges: Array[Dictionary] = []
var _slot_names: PackedStringArray = []
var _channel_names: PackedStringArray = []


func _ready() -> void:
	visible = false
	_font = ThemeDB.fallback_font
	position = Vector2(392, 20)
	# Right edge at 1035, matching the co-evolution panel: at 600 wide it stopped
	# 38px short of the codex panel behind it, leaving a sliver of event buttons
	# poking out along its edge like a torn seam.
	size = Vector2(643, 660)
	_slot_names = sim.genome_slot_catalog()
	_channel_names = sim.meme_channel_catalog()
	var inv_cat: Array = sim.invention_catalog()
	for k in range(inv_cat.size()):
		var inv: Dictionary = inv_cat[k]
		var aff: Dictionary = inv["affinity"]
		if not aff.is_empty():
			(
				_edges
				. append(
					{
						"gene_slot": _slot_index_of(String(aff["slot"])),
						"kind": "affinity",
						"inv": k,
					}
				)
			)
		var req: Dictionary = inv["gene_req"]
		if not req.is_empty():
			(
				_edges
				. append(
					{
						"gene_slot": _slot_index_of(String(req["slot"])),
						"kind": "req",
						"inv": k,
					}
				)
			)
	# DIT learning arms: the innate strategy and both learning propensities all
	# couple the genome to the culturally-transmitted technique channel.
	for slot in [40, 28, 29]:
		_edges.append({"gene_slot": slot, "kind": "dit", "inv": -1})


func _slot_index_of(slot_name: String) -> int:
	for i in range(_slot_names.size()):
		if _slot_names[i] == slot_name:
			return i
	return -1


func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_X:
		_shown = not _shown
		visible = _shown
		if _shown:
			_snap = sim.helix_snapshot()
			queue_redraw()


func _process(_delta: float) -> void:
	_frame += 1
	if _shown and _frame % REFRESH_EVERY == 0:
		_snap = sim.helix_snapshot()
		queue_redraw()


# Strand point for a node at parameter t in [0,1]. side -1 = gene (left),
# +1 = meme (right).
func _strand_point(t: float, side: float) -> Vector2:
	var h: float = size.y - TOP - BOTTOM_PAD
	var cx: float = size.x * 0.5
	var amp: float = size.x * 0.21
	var phase: float = t * TWIST_TURNS * TAU
	return Vector2(cx + side * amp * sin(phase), TOP + t * h)


func _draw() -> void:
	draw_rect(Rect2(Vector2.ZERO, size), Color(0.035, 0.05, 0.065, 0.985))
	draw_string(
		_font,
		Vector2(12, 22),
		"DUAL INHERITANCE — genome × memome",
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		14,
		Color(0.85, 0.9, 1.0)
	)
	draw_string(_font, Vector2(12, 40), "genes", HORIZONTAL_ALIGNMENT_LEFT, -1, 11, GENE_COLOR)
	draw_string(
		_font, Vector2(size.x - 66, 40), "memes", HORIZONTAL_ALIGNMENT_LEFT, -1, 11, MEME_COLOR
	)
	var gene_means: PackedFloat32Array = _snap.get("gene_means", PackedFloat32Array())
	if gene_means.is_empty():
		draw_string(
			_font,
			Vector2(12, 66),
			"waiting for a running world…",
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			12,
			Color.WHITE
		)
		return
	var meme_means: PackedFloat32Array = _snap["meme_means"]
	var affinity_diff: PackedFloat32Array = _snap["affinity_diff"]

	# Backbones: sample each strand continuously so the winding reads as one
	# curve, not a polyline through sparse nodes.
	for side in [-1.0, 1.0]:
		var pts := PackedVector2Array()
		for s in range(81):
			pts.push_back(_strand_point(float(s) / 80.0, side))
		var col := GENE_COLOR if side < 0.0 else MEME_COLOR
		draw_polyline(pts, Color(col.r, col.g, col.b, 0.35), 2.0, true)

	# Node positions: each list is spread across the shared helix parameter.
	var gene_pos := {}
	for i in range(GENE_SLOTS.size()):
		gene_pos[GENE_SLOTS[i]] = _strand_point((float(i) + 0.5) / float(GENE_SLOTS.size()), -1.0)
	var meme_pos := {}
	for j in range(MEME_CHANNELS_LIST.size()):
		meme_pos[MEME_CHANNELS_LIST[j]] = _strand_point(
			(float(j) + 0.5) / float(MEME_CHANNELS_LIST.size()), 1.0
		)

	# Rungs (under the nodes).
	for e in _edges:
		var slot: int = e["gene_slot"]
		if slot < 0 or not gene_pos.has(slot):
			continue
		var ch: int
		if e["inv"] >= 0:
			ch = INVENTION_CHANNEL_BASE + int(e["inv"])
		else:
			ch = 6  # DIT technique channel
		if not meme_pos.has(ch):
			continue
		var a: Vector2 = gene_pos[slot]
		var b: Vector2 = meme_pos[ch]
		match String(e["kind"]):
			"affinity":
				var diff: float = affinity_diff[int(e["inv"])]
				var strength: float = clampf(absf(diff) * 8.0, 0.0, 1.0)
				var col := IDLE_COLOR
				if strength > 0.05:
					var base := POS_COLOR if diff > 0.0 else NEG_COLOR
					col = Color(base.r, base.g, base.b, 0.25 + 0.65 * strength)
				draw_line(a, b, col, 1.0 + 3.0 * strength, true)
			"req":
				_draw_dashed(a, b, REQ_COLOR)
			_:
				draw_line(a, b, DIT_COLOR, 1.0, true)

	# Nodes + labels. Labels live in fixed side columns (never crossing the
	# strands) with a thin connector to their node, so the winding backbone
	# can't swing a node into its own label.
	for slot in GENE_SLOTS:
		var p: Vector2 = gene_pos[slot]
		var v: float = gene_means[slot]
		var r: float = 3.0 + 7.0 * clampf(v, 0.0, 1.0)
		draw_circle(
			p, r, Color(GENE_COLOR.r, GENE_COLOR.g, GENE_COLOR.b, 0.35 + 0.55 * clampf(v, 0.0, 1.0))
		)
		draw_arc(p, r, 0.0, TAU, 24, GENE_COLOR, 1.0, true)
		var lp := Vector2(12.0, p.y + 4.0)
		draw_line(
			Vector2(160.0, p.y), p, Color(GENE_COLOR.r, GENE_COLOR.g, GENE_COLOR.b, 0.15), 1.0
		)
		draw_string(
			_font,
			lp,
			"%s %.2f" % [_slot_names[slot], v],
			HORIZONTAL_ALIGNMENT_LEFT,
			148,
			9,
			Color(0.7, 0.85, 1.0)
		)
	for ch in MEME_CHANNELS_LIST:
		var p: Vector2 = meme_pos[ch]
		var v: float = meme_means[ch]
		var r: float = 3.0 + 7.0 * clampf(v, 0.0, 1.0)
		draw_circle(
			p, r, Color(MEME_COLOR.r, MEME_COLOR.g, MEME_COLOR.b, 0.35 + 0.55 * clampf(v, 0.0, 1.0))
		)
		draw_arc(p, r, 0.0, TAU, 24, MEME_COLOR, 1.0, true)
		var lp := Vector2(size.x - 128.0, p.y + 4.0)
		draw_line(
			p,
			Vector2(size.x - 132.0, p.y),
			Color(MEME_COLOR.r, MEME_COLOR.g, MEME_COLOR.b, 0.15),
			1.0
		)
		draw_string(
			_font,
			lp,
			"%s %.2f" % [_channel_names[ch], v],
			HORIZONTAL_ALIGNMENT_LEFT,
			124,
			9,
			Color(1.0, 0.9, 0.7)
		)

	# Legend.
	var ly: float = size.y - 30.0
	draw_line(Vector2(14, ly - 4), Vector2(34, ly - 4), POS_COLOR, 3.0)
	draw_string(
		_font,
		Vector2(40, ly),
		"holders carry more gene",
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		9,
		Color(0.75, 0.8, 0.85)
	)
	_draw_dashed(Vector2(178, ly - 4), Vector2(198, ly - 4), REQ_COLOR)
	draw_string(
		_font,
		Vector2(204, ly),
		"genetic prerequisite",
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		9,
		Color(0.75, 0.8, 0.85)
	)
	draw_line(Vector2(326, ly - 4), Vector2(346, ly - 4), DIT_COLOR, 1.0)
	draw_string(
		_font,
		Vector2(352, ly),
		"DIT learning arm",
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		9,
		Color(0.75, 0.8, 0.85)
	)


func _draw_dashed(a: Vector2, b: Vector2, col: Color) -> void:
	var segments := 10
	for s in range(0, segments, 2):
		var p0: Vector2 = a.lerp(b, float(s) / float(segments))
		var p1: Vector2 = a.lerp(b, float(s + 1) / float(segments))
		draw_line(p0, p1, col, 1.0, true)
