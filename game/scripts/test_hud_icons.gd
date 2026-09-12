extends SceneTree
# Headless check for the HUD icon set: every HUD glyph and every invention
# icon is a 16x16 image with a readable mark, and unknown invention keys fall
# back to the generic era gear instead of erroring. Run with:
#   godot --headless --rendering-driver dummy --path game -s res://scripts/test_hud_icons.gd

const Codex = preload("res://scripts/codex_panel.gd")
const EventLog = preload("res://scripts/event_log.gd")
const Icons = preload("res://scripts/hud_icons.gd")
const ResearchPanel = preload("res://scripts/research_panel.gd")
const SpeciesNames = preload("res://scripts/species_names.gd")
const EcoMeters = preload("res://scripts/eco_meters.gd")
const TimeControls = preload("res://scripts/time_controls.gd")
const TopBar = preload("res://scripts/top_bar.gd")
const UnitCard = preload("res://scripts/unit_card.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _init() -> void:
	for kind in Icons.KIND_COUNT:
		var img: Image = Icons.kind_texture(kind).get_image()
		_check(img.get_width() == 16 and img.get_height() == 16, "kind %d icon is 16x16" % kind)
		_check(Icons.opaque_pixels(img) >= 20, "kind %d icon has a readable mark" % kind)
	for key in Icons.INVENTION_ICONS.keys():
		var img: Image = Icons.invention_texture(key).get_image()
		_check(Icons.opaque_pixels(img) >= 20, "invention icon %s has a readable mark" % key)
	_check(
		Icons.invention_texture("no_such_invention") == Icons.named_texture("era"),
		"unknown invention key falls back to the era gear"
	)
	# Every row string is exactly 16 wide and 16 tall.
	for name in Icons._ROWS.keys():
		var rows: Array = Icons._ROWS[name]
		_check(rows.size() == 16, "%s has 16 rows" % name)
		for r in rows:
			_check((r as String).length() == 16, "%s rows are 16 wide" % name)

	# --- research adoption fractions ---
	var masks := PackedInt32Array([0b0011, 0b0001, 0b0000, 0b0011])
	var f: PackedFloat32Array = ResearchPanel.adoption_fractions(masks, 3)
	_check(f.size() == 3, "one fraction per invention")
	_check(is_equal_approx(f[0], 0.75), "bit 0 held by 3 of 4")
	_check(is_equal_approx(f[1], 0.5), "bit 1 held by 2 of 4")
	_check(is_equal_approx(f[2], 0.0), "bit 2 held by none")
	_check(
		ResearchPanel.adoption_fractions(PackedInt32Array(), 2).size() == 2, "empty masks -> zeros"
	)

	# --- research visible rows: adopted first, then a few next, capped ---
	var fr := PackedFloat32Array([0.0, 1.0, 0.0, 0.4, 0.0, 0.0, 0.0])
	var rows: PackedInt32Array = ResearchPanel.visible_rows(fr, 12, 2)
	_check(rows == PackedInt32Array([0, 1, 2, 3]), "adopted plus the next two unadopted, in order")
	_check(ResearchPanel.visible_rows(fr, 3, 2).size() == 3, "row cap holds")

	# --- day counter ---
	_check(TopBar.day_of(0) == 1, "tick 0 is day 1")
	_check(TopBar.day_of(TopBar.TICKS_PER_DAY * 3 + 5) == 4, "days advance every TICKS_PER_DAY")

	# --- species names: stable, readable, injective over a long run's ids ---
	var seen: Dictionary = {}
	for id in 8000:
		var n: String = SpeciesNames.name_of(id)
		_check(n.length() >= 4 and n[0] == n[0].to_upper(), "name %d is a capitalised word" % id)
		_check(not seen.has(n), "name %d (%s) is unique" % [id, n])
		seen[n] = true
	var seven: String = SpeciesNames.name_of(7)
	_check(SpeciesNames.name_of(7) == seven, "names are deterministic")
	_check(SpeciesNames.name_of(-3) == SpeciesNames.name_of(0), "negative ids clamp to 0")

	# --- event log: every chapter has an icon and a sentence naming the species ---
	for t in Codex.CHAPTER_NAMES.size():
		_check(Icons._ROWS.has(EventLog.icon_for(t)), "chapter %d icon exists" % t)
		var line: String = EventLog.line_for(t, 12)
		_check(line.contains(SpeciesNames.name_of(12)), "chapter %d line names the species" % t)
		_check(not line.contains("%s"), "chapter %d template is filled" % t)
	_check(Icons._ROWS.has(EventLog.icon_for(999)), "unknown chapter falls back to an icon")
	_check(EventLog.line_for(999, 1).begins_with("Event"), "unknown chapter falls back to a title")

	# --- unit card: HP scale and module pips ---
	_check(is_equal_approx(UnitCard.hp_fraction(UnitCard.HP_FULL * 3.0), 1.0), "HP caps at full")
	_check(is_equal_approx(UnitCard.hp_fraction(UnitCard.HP_FULL * 0.25), 0.25), "HP scales")
	_check(is_equal_approx(UnitCard.hp_fraction(-5.0), 0.0), "HP floors at zero")
	var pips: PackedInt32Array = UnitCard.pip_counts(
		PackedStringArray(["Weapon", "Jaws", "Armor", "Locomotor", "Locomotor", "Sensor", "Mouth"])
	)
	_check(pips == PackedInt32Array([2, 1, 2, 1]), "pips count weapon/armour/locomotor/sensor")
	_check(
		UnitCard.pip_counts(PackedStringArray()) == PackedInt32Array([0, 0, 0, 0]),
		"no modules -> zero pips"
	)

	# --- hotbar tools resolve to real keys and icons; meters clamp ---
	for tool in TimeControls.TOOLS:
		var ev: InputEventKey = TimeControls.key_event(String(tool[0]))
		_check(ev.keycode != KEY_NONE and ev.pressed, "hotbar key %s resolves" % tool[0])
		_check(int(tool[2]) >= 0 and int(tool[2]) < Icons.KIND_COUNT, "hotbar icon %s" % tool[0])
	_check(TimeControls.key_event("G").keycode == KEY_G, "G maps to KEY_G")
	_check(is_equal_approx(EcoMeters.health_of(50, 200), 0.25), "health is live over peak")
	_check(is_equal_approx(EcoMeters.health_of(300, 200), 1.0), "health caps at one")
	_check(is_equal_approx(EcoMeters.health_of(5, 0), 0.0), "no peak yet -> zero")
	_check(is_equal_approx(EcoMeters.simpson(PackedInt32Array([10])), 0.0), "one species -> 0")
	_check(is_equal_approx(EcoMeters.simpson(PackedInt32Array([5, 5])), 0.5), "two even -> 0.5")
	_check(is_equal_approx(EcoMeters.simpson(PackedInt32Array()), 0.0), "no species -> 0")

	# --- codex species helpers ---
	_check(Codex.diet_label(0.1) == "Herbivore", "low carnivory is a herbivore")
	_check(Codex.diet_label(0.5) == "Omnivore", "mid carnivory is an omnivore")
	_check(Codex.diet_label(0.9) == "Carnivore", "high carnivory is a carnivore")
	_check(Codex.diet_word(0.9) == "Meat", "trait diet word")
	_check(Codex.level_label(0.6) == "High" and Codex.level_label(0.05) == "Low", "level labels")
	_check(Codex.social_label(2, 0.1) == "Solitary", "tiny species are solitary")
	_check(Codex.social_label(40, 0.1) == "Herd", "big herbivore species form herds")
	_check(Codex.social_label(40, 0.9) == "Pack", "big carnivore species form packs")
	var ids := PackedByteArray()
	ids.resize(1000)
	ids.fill(1)
	for i in 250:
		ids[i] = 2
	var shares: PackedFloat32Array = Codex.terrain_shares(ids, 100000)
	_check(is_equal_approx(shares[2], 0.25) and is_equal_approx(shares[1], 0.75), "terrain shares")
	var sampled: PackedFloat32Array = Codex.terrain_shares(ids, 100)
	_check(absf(sampled[2] - 0.25) < 0.05, "sampled shares approximate the full scan")
	_check(Codex.terrain_shares(PackedByteArray(), 10).size() == 9, "empty world -> nine zeros")
	var hist := PackedFloat32Array([0.0, 0.6, 0.3, 0.0, 0.1, 0.0, 0.0, 0.0, 0.0])
	_check(
		Codex.habitat_names(hist) == PackedStringArray(["grass", "forest"]),
		"habitat names the two most-visited terrains"
	)
	var note: String = Codex.describe(
		{
			"speed": 0.7,
			"armour": 0.5,
			"count": 30,
			"diet": 0.1,
			"habitat": PackedStringArray(["grass"])
		}
	)
	_check(note.begins_with("Fast, armoured and social."), "field note adjectives: " + note)
	_check(note.ends_with("in herds near grass."), "field note habitat: " + note)
	_check(
		Codex.describe({"speed": 0.0, "count": 2}) == "Steady and solitary.",
		"field note without habitat"
	)

	if _failed:
		quit(1)
		return
	print("test_hud_icons: all passed")
	quit(0)
