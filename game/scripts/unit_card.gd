extends PanelContainer
# Unit card (Phase 5 of docs/superpowers/specs/2026-09-12-pixel-world-at-
# scale-design.md): the reference boards' bottom-right selected-unit panel —
# portrait, name, an HP bar from energy, a stamina bar from the basic-needs
# fatigue drive (skill when that subsystem is off) and four pips counting the
# weapon, armour, locomotor and sensor modules. Replaces the raw-text
# inspector; keeps its `pin()` API and node name so click-to-inspect, the
# capture hooks and the showcase director still find it.

const ApeSprites = preload("res://scripts/ape_sprites.gd")
const HudIcons = preload("res://scripts/hud_icons.gd")
const MammalSprites = preload("res://scripts/mammal_sprites.gd")
const SpeciesNames = preload("res://scripts/species_names.gd")
const UiTheme = preload("res://scripts/ui_theme.gd")

const REFRESH_EVERY := 6
# Energy shown as HP out of this: twice the spawn energy (agent.rs
# SPAWN_ENERGY); a well-fed agent reads as full rather than as a raw float.
const HP_FULL := 100.0
const PORTRAIT := 48
const CARD_W := 200
const CARD_H := 126
const BAR_W := 118
const BAR_H := 8
const HP_FILL := Color(0.42, 0.80, 0.36)
const STAMINA_FILL := Color(0.35, 0.62, 0.95)
const BAR_BG := Color(0.06, 0.09, 0.11)
const PIP_ICONS: PackedInt32Array = [HudIcons.SWORD, HudIcons.SHIELD, HudIcons.LEGS, HudIcons.EYE]

var pinned_id: int = -1

@onready var sim = get_node("../../Simulation")

var _card: Control
var _font: Font
var _frame: int = 0
var _info: Dictionary = {}
var _ape_tex: Array = []
var _quad_tex: Dictionary = {}


func _ready() -> void:
	_font = get_theme_default_font()
	if _font == null:
		_font = ThemeDB.fallback_font
	for i in ApeSprites.NAMES.size():
		_ape_tex.append(MammalSprites.hominin_portrait(i))
	_card = Control.new()
	_card.custom_minimum_size = Vector2(CARD_W, CARD_H)
	_card.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_card.draw.connect(_draw_card)
	add_child(_card)


func pin(id: int) -> void:
	pinned_id = id
	visible = id >= 0
	_frame = REFRESH_EVERY - 1


static func hp_fraction(energy: float) -> float:
	return clampf(energy / HP_FULL, 0.0, 1.0)


# [weapon, armour, locomotor, sensor] counts from an agent's module names:
# jaws count as weapons and spines as armour, matching the field silhouettes.
static func pip_counts(names: PackedStringArray) -> PackedInt32Array:
	var out := PackedInt32Array([0, 0, 0, 0])
	for n in names:
		match String(n):
			"Weapon", "Jaws":
				out[0] += 1
			"Armor", "Spines":
				out[1] += 1
			"Locomotor":
				out[2] += 1
			"Sensor":
				out[3] += 1
	return out


func _process(_delta: float) -> void:
	if pinned_id < 0:
		return
	_frame += 1
	if _frame % REFRESH_EVERY != 0:
		return
	_info = sim.agent_detail(pinned_id)
	_card.queue_redraw()


func _portrait() -> Dictionary:
	var sp: int = int(_info["species_id"])
	var live: bool = (
		bool(_info.get("domestication_enabled", false)) and int(_info.get("livestock_of", -1)) != -1
	)
	var arch: int = MammalSprites.archetype_for(
		float(_info.get("diet_carnivory", 0.5)), float(_info.get("size", 1.0)), live
	)
	if arch == MammalSprites.PRIMATE:
		var ape: int = MammalSprites.primate_skin_for(sp)
		return {"tex": _ape_tex[ape], "tint": Color.WHITE, "kind": ApeSprites.NAMES[ape]}
	if not _quad_tex.has(arch):
		_quad_tex[arch] = MammalSprites.portrait(arch)
	return {
		"tex": _quad_tex[arch],
		"tint": MammalSprites.coat_hue(arch, sp),
		"kind": MammalSprites.NAMES[arch],
	}


func _bar(y: int, icon: int, label: String, frac: float, fill: Color) -> void:
	_card.draw_texture_rect(HudIcons.kind_texture(icon), Rect2(PORTRAIT + 10, y, 16, 16), false)
	var bx := PORTRAIT + 30
	_card.draw_string(
		_font, Vector2(bx, y + 11), label, HORIZONTAL_ALIGNMENT_LEFT, -1, 10, UiTheme.TEXT
	)
	_card.draw_rect(Rect2(bx, y + 13, BAR_W, BAR_H), BAR_BG)
	_card.draw_rect(Rect2(bx, y + 13, BAR_W * clampf(frac, 0.0, 1.0), BAR_H), fill)
	_card.draw_rect(Rect2(bx, y + 13, BAR_W, BAR_H), UiTheme.ACCENT_DIM, false, 1.0)


func _draw_card() -> void:
	if _info.is_empty():
		return
	if not bool(_info.get("alive", false)):
		_card.draw_string(
			_font,
			Vector2(0, 18),
			"(agent %d has fallen)" % pinned_id,
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			12,
			UiTheme.TEXT_DIM
		)
		return
	var port: Dictionary = _portrait()
	_card.draw_rect(Rect2(0, 0, PORTRAIT, PORTRAIT), UiTheme.BG_ELEV)
	_card.draw_rect(Rect2(0, 0, PORTRAIT, PORTRAIT), UiTheme.ACCENT_DIM, false, 1.0)
	var tex: Texture2D = port["tex"]
	_card.draw_texture_rect(tex, Rect2(0, 0, PORTRAIT, PORTRAIT), false, port["tint"])
	var sp: int = int(_info["species_id"])
	var x := PORTRAIT + 10
	_card.draw_string(
		_font,
		Vector2(x, 13),
		"%s %s" % [SpeciesNames.name_of(sp), String(port["kind"])],
		HORIZONTAL_ALIGNMENT_LEFT,
		CARD_W - x,
		13,
		UiTheme.TEXT
	)
	_card.draw_string(
		_font,
		Vector2(x, 26),
		"sp %d · id %d · age %d" % [sp, pinned_id, int(_info["age"])],
		HORIZONTAL_ALIGNMENT_LEFT,
		CARD_W - x,
		10,
		UiTheme.TEXT_DIM
	)
	var hp: float = hp_fraction(float(_info["energy"]))
	_bar(30, HudIcons.HEART, "HP %d/%d" % [int(round(hp * HP_FULL)), int(HP_FULL)], hp, HP_FILL)
	var stamina: float
	var stamina_label: String
	if bool(_info.get("basic_needs_enabled", false)):
		stamina = 1.0 - clampf(float(_info.get("fatigue", 0.0)), 0.0, 1.0)
		stamina_label = "Stamina %d/100" % int(round(stamina * 100.0))
		if bool(_info.get("asleep", false)):
			stamina_label += " (asleep)"
	else:
		stamina = clampf(float(_info.get("skill", 0.0)), 0.0, 1.0)
		stamina_label = "Skill %d/100" % int(round(stamina * 100.0))
	_bar(54, HudIcons.DROP, stamina_label, stamina, STAMINA_FILL)
	# Module pips: icon + count, four across.
	var counts: PackedInt32Array = pip_counts(_info.get("module_names", PackedStringArray()))
	var py := 82
	for k in PIP_ICONS.size():
		var px := k * 50
		_card.draw_texture_rect(HudIcons.kind_texture(PIP_ICONS[k]), Rect2(px, py, 16, 16), false)
		_card.draw_string(
			_font,
			Vector2(px + 20, py + 13),
			str(counts[k]),
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			12,
			UiTheme.TEXT
		)
	# Footer: diet, era, mood.
	var parts: PackedStringArray = ["diet %.2f" % float(_info.get("diet_carnivory", 0.5))]
	if int(_info.get("tech_era", 0)) > 0:
		parts.append("era %d" % int(_info["tech_era"]))
	if bool(_info.get("affect_enabled", false)):
		parts.append(String(_info.get("mood", "content")))
	if bool(_info.get("disease_enabled", false)) and float(_info.get("infection", 0.0)) > 0.0:
		parts.append("infected")
	if bool(_info.get("domestication_enabled", false)) and int(_info.get("livestock_of", -1)) >= 0:
		parts.append("livestock of %d" % int(_info["livestock_of"]))
	_card.draw_string(
		_font,
		Vector2(0, py + 34),
		" · ".join(parts),
		HORIZONTAL_ALIGNMENT_LEFT,
		CARD_W,
		10,
		UiTheme.TEXT_DIM
	)
