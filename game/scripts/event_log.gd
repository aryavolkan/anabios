extends PanelContainer
# Event log (Phase 5 of docs/superpowers/specs/2026-09-12-pixel-world-at-
# scale-design.md): the reference boards' bottom-left feed — one icon and one
# sentence per codex event, newest at the bottom, coloured as the codex
# timeline colours them; click a line to jump the camera there (the old codex
# stream's `V` behaviour). The chapter tables stay in codex_panel.gd, which the
# replay and showcase scripts already read; this panel only renders them.

const Codex = preload("res://scripts/codex_panel.gd")
const PixelFont = preload("res://scripts/pixel_font.gd")
const HudIcons = preload("res://scripts/hud_icons.gd")
const SpeciesNames = preload("res://scripts/species_names.gd")
const UiTheme = preload("res://scripts/ui_theme.gd")

const MAX_LINES := 7
const ROW_H := 19
const ICON := 16

# Chapter id -> HUD icon name (hud_icons.gd _ROWS). Anything unlisted wears
# the codex book.
const _ICONS: Dictionary = {
	0: "skull",  # Extinction
	1: "skull",  # PopCrash
	2: "dna",  # Speciation
	3: "legs",  # Migration
	4: "dna",  # NovelModule
	5: "paw",  # NovelBehavior
	6: "meat",  # Predation
	7: "sword",  # CombatRaid
	8: "shield",  # ArmsRace
	9: "home",  # Territory
	10: "leaf",  # NichePartition
	11: "people",  # Dialect
	12: "people",  # MemeSweep
	13: "eye",  # AlarmCall
	14: "heart",  # Cooperation
	15: "paw",  # PackHunting
	16: "paw",  # HerdCohesion
	17: "era",  # Discovery
	18: "era",  # Adoption
	19: "era",  # BadIdea
	20: "era",  # BadAdopt
	21: "trade",  # Trade
	22: "era",  # MaterialLearn
	23: "people",  # PopCycle
	24: "people",  # BoomBust
	25: "leaf",  # CarryingCap
	26: "meat",  # TrophicCascade
	27: "legs",  # RangeExpand
	28: "people",  # Segregation
	29: "legs",  # Corridor
	30: "leaf",  # Succession
	31: "dna",  # TraitFixed
	32: "dna",  # RapidAdapt
	33: "dna",  # Convergent
	34: "sword",  # Ambush
	35: "era",  # ToolUse
	36: "legs",  # Flight
	37: "eye",  # Signaling
	38: "sword",  # War
	39: "shield",  # WarEnded
	40: "heart",  # Alliance
	41: "heart",  # KinNetwork
	42: "home",  # Settlement
	43: "trade",  # Market
	44: "people",  # Specialists
	45: "book",  # Tradition
	46: "dna",  # Radiation
	47: "era",  # Ratchet
	48: "skull",  # Maladaptation
	49: "heart",  # SexSelect
	50: "people",  # SexRatio
	51: "paw",  # Domesticated
	52: "paw",  # LivestockHerd
	53: "book",  # Knowledge
	54: "eye",  # MassFright
	55: "eye",  # PanicCascade
	56: "meat",  # FeedingFrenzy
	57: "sword",  # TerritorialRage
	58: "skull",  # MassGrief
	59: "shield",  # HuntedAdapt
	60: "drop",  # Dehydration
	61: "skull",  # Epidemic
	62: "heart",  # MedContain
}

# Chapter id -> sentence template; %s is the species name. Unlisted chapters
# read as "<Chapter>: <species>".
const _LINES: Dictionary = {
	0: "%s has gone extinct",
	1: "Population crash among %s",
	2: "A new species has emerged: %s",
	3: "%s are migrating",
	4: "%s evolved a new body part",
	5: "%s learned a new behaviour",
	6: "Predator spotted: %s",
	7: "%s raided a neighbour",
	8: "Arms race escalates: %s",
	9: "%s claimed a territory",
	10: "%s carved out a niche",
	11: "A new dialect among %s",
	12: "An idea swept through %s",
	13: "Alarm calls from %s",
	14: "%s are cooperating",
	15: "%s hunt in packs",
	16: "Herd behaviour detected (%s)",
	17: "New technology discovered by %s",
	18: "%s adopted a technology",
	21: "Trade caravan arrived (%s)",
	22: "%s learned to work a material",
	24: "Boom and bust for %s",
	25: "%s reached carrying capacity",
	27: "%s expanded their range",
	30: "New growth where %s graze",
	31: "A trait fixed in %s",
	32: "%s adapted rapidly",
	34: "%s ambushed their prey",
	35: "%s are using tools",
	36: "%s fled",
	37: "%s are signalling",
	38: "War breaks out: %s",
	39: "The war of %s has ended",
	40: "An alliance formed with %s",
	42: "Settlement founded by %s",
	43: "A market opened among %s",
	44: "%s trained specialists",
	45: "A tradition took hold in %s",
	46: "%s radiated into new forms",
	48: "%s are stranded by change",
	51: "%s domesticated an animal",
	52: "A herd of livestock for %s",
	53: "%s preserved their knowledge",
	54: "Panic among %s",
	55: "Panic cascades through %s",
	56: "A feeding frenzy of %s",
	57: "Territorial rage: %s",
	58: "Grief spreads among %s",
	60: "%s are dehydrating",
	61: "An epidemic among %s",
	62: "%s contained an epidemic",
}

@onready var sim = get_node("../../Simulation")
@onready var camera: Camera2D = get_node("../../Camera2D")

var _recent: Array[Dictionary] = []
var _cursor: int = 0
var _rows: Control


func _ready() -> void:
	_rows = Control.new()
	_rows.custom_minimum_size = Vector2(360, ROW_H * MAX_LINES)
	_rows.draw.connect(_draw_rows)
	_rows.gui_input.connect(_on_gui_input)
	add_child(_rows)


static func icon_for(type: int) -> String:
	return String(_ICONS.get(type, "book"))


static func line_for(type: int, species_id: int) -> String:
	var who: String = SpeciesNames.name_of(species_id)
	if _LINES.has(type):
		return String(_LINES[type]) % who
	var title: String = Codex.CHAPTER_NAMES[type] if type < Codex.CHAPTER_NAMES.size() else "Event"
	return "%s: %s" % [title, who]


func _process(_delta: float) -> void:
	if sim.codex_event_count() < _cursor:
		_cursor = 0
		_recent.clear()
		_rows.queue_redraw()
	var events: Array = sim.codex_events_since(_cursor)
	if events.is_empty():
		return
	for ev in events:
		_cursor = int(ev["index"]) + 1
		_recent.append(ev)
		while _recent.size() > MAX_LINES:
			_recent.pop_front()
	_rows.queue_redraw()


func _draw_rows() -> void:
	var font: Font = PixelFont.build()
	var y := 0
	for ev in _recent:
		var t: int = int(ev["type"])
		var col: Color = (
			Codex.CHAPTER_COLORS[t] if t < Codex.CHAPTER_COLORS.size() else UiTheme.TEXT
		)
		_rows.draw_texture_rect(
			HudIcons.named_texture(icon_for(t)), Rect2(0, y + 2, ICON, ICON), false
		)
		_rows.draw_string(
			font,
			Vector2(ICON + 8, y + 14),
			line_for(t, int(ev["species_id"])),
			HORIZONTAL_ALIGNMENT_LEFT,
			330,
			12,
			col
		)
		y += ROW_H


func _on_gui_input(event: InputEvent) -> void:
	if not (event is InputEventMouseButton):
		return
	var mb := event as InputEventMouseButton
	if not mb.pressed or mb.button_index != MOUSE_BUTTON_LEFT:
		return
	var row: int = int(mb.position.y / ROW_H)
	if row < 0 or row >= _recent.size():
		return
	var loc: Vector2 = _recent[row]["loc"]
	if loc != Vector2.ZERO:
		camera.position = loc
