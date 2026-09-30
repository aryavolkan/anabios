extends SceneTree
# Headless check for the HUD icon set: every HUD glyph and every invention
# icon is a 16x16 image with a readable mark, and unknown invention keys fall
# back to the generic era gear instead of erroring. Also pins the HUD layout
# (main.tscn's authored rects and the panels that place themselves), so no two
# panels open on top of each other. Run with:
#   godot --headless --rendering-driver dummy --path game -s res://scripts/test_hud_icons.gd

const Codex = preload("res://scripts/codex_panel.gd")
const EventLog = preload("res://scripts/event_log.gd")
const Icons = preload("res://scripts/hud_icons.gd")
const ResearchPanel = preload("res://scripts/research_panel.gd")
const ReplayManager = preload("res://scripts/replay_manager.gd")
const SpeciesNames = preload("res://scripts/species_names.gd")
const EcoMeters = preload("res://scripts/eco_meters.gd")
const TimeControls = preload("res://scripts/time_controls.gd")
const TopBar = preload("res://scripts/top_bar.gd")
const UnitCard = preload("res://scripts/unit_card.gd")
const BrandPanel = preload("res://scripts/brand_panel.gd")
const CoevolutionPanel = preload("res://scripts/coevolution_panel.gd")
const EvolutionPanel = preload("res://scripts/evolution_panel.gd")
const HelixPanel = preload("res://scripts/helix_panel.gd")
const LegendPanel = preload("res://scripts/legend_panel.gd")
const UiTheme = preload("res://scripts/ui_theme.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


# The authored numeric properties (offsets, grow directions) of every direct
# child of UI in main.tscn, by node name. Read as text: loading the scene would
# compile main.gd and friends, which need the GameConfig autoload that a -s
# script run does not have.
static func _ui_props(path: String) -> Dictionary:
	var out: Dictionary = {}
	var cur: Dictionary = {}
	var node_re := RegEx.create_from_string('^\\[node name="(\\w+)"[^\\]]*parent="UI"\\]')
	var prop_re := RegEx.create_from_string("^(offset_\\w+|grow_\\w+) = (-?[0-9.]+)$")
	for line in FileAccess.get_file_as_string(path).split("\n"):
		if line.begins_with("["):
			var m := node_re.search(line)
			cur = {}
			if m != null:
				out[m.get_string(1)] = cur
			continue
		var p := prop_re.search(line)
		if p != null:
			cur[p.get_string(1)] = float(p.get_string(2))
	return out


static func _rect_of(props: Dictionary) -> Rect2:
	var r := Rect2(Vector2(props.get("offset_left", 0.0), props.get("offset_top", 0.0)), Vector2())
	r.end = Vector2(props.get("offset_right", 0.0), props.get("offset_bottom", 0.0))
	return r


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
	# The replay / event-camera banner names the event in the log's words, not
	# the codex's CamelCase chapter id ("PanicCascade").
	var cascade := {"type": 55, "species_id": 12}
	_check(
		ReplayManager.event_title(cascade) == EventLog.line_for(55, 12), "banner uses the log line"
	)
	_check(
		not ReplayManager.event_title(cascade).contains(Codex.CHAPTER_NAMES[55]), "banner not an id"
	)
	_check(ReplayManager.event_title({}) == "event", "banner falls back without an event type")
	# A burst of one event folds into one "×n" row instead of flooding the feed.
	var feed: Array[Dictionary] = []
	for i in 30:
		EventLog.fold_event(
			feed,
			{"type": 54, "species_id": 3, "tick": 300 + floori(i / 10.0), "loc": Vector2(i, 0)},
			7
		)
	_check(feed.size() == 1 and int(feed[0]["count"]) == 30, "a panic burst is one row")
	_check(EventLog.row_text(feed[0]) == EventLog.line_for(54, 3) + " ×30", "row shows ×n")
	_check(feed[0]["loc"] == Vector2(29, 0), "a folded row jumps to the latest location")
	EventLog.fold_event(feed, {"type": 2, "species_id": 3, "tick": 303, "loc": Vector2.ZERO}, 7)
	EventLog.fold_event(feed, {"type": 54, "species_id": 4, "tick": 303, "loc": Vector2.ZERO}, 7)
	_check(feed.size() == 3, "another type or species gets its own row")
	EventLog.fold_event(
		feed,
		{"type": 54, "species_id": 3, "tick": 303 + EventLog.REPEAT_TICKS + 1, "loc": Vector2.ZERO},
		7
	)
	_check(feed.size() == 4, "a repeat after REPEAT_TICKS starts a new row")
	_check(EventLog.row_text(feed[3]) == EventLog.line_for(54, 3), "a single event has no count")
	for i in 10:
		EventLog.fold_event(feed, {"type": i, "species_id": 9, "tick": 500, "loc": Vector2.ZERO}, 7)
	_check(feed.size() == 7, "the feed keeps MAX_LINES rows")

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

	# --- top bar population delta ---
	_check(TopBar.delta_text(120, 100) == " (+20)", "positive delta")
	_check(TopBar.delta_text(90, 100) == " (-10)", "negative delta")
	_check(TopBar.delta_text(100, 100) == "", "no change, no delta")

	# --- hotbar tools resolve to real keys and icons; meters clamp ---
	for tool in TimeControls.TOOLS:
		var ev: InputEventKey = TimeControls.key_event(String(tool[0]))
		_check(ev.keycode != KEY_NONE and ev.pressed, "hotbar key %s resolves" % tool[0])
		_check(int(tool[2]) >= 0 and int(tool[2]) < Icons.KIND_COUNT, "hotbar icon %s" % tool[0])
	_check(TimeControls.key_event("G").keycode == KEY_G, "G maps to KEY_G")
	# The pause glyph and speed accent follow main's state whoever set it.
	var hotbar: HBoxContainer = TimeControls.new()
	for node_name in ["PauseButton", "Speed1", "Speed16"]:
		var btn := Button.new()
		btn.name = node_name
		hotbar.add_child(btn)
	hotbar._speed_btns = {1: hotbar.get_node("Speed1"), 16: hotbar.get_node("Speed16")}
	hotbar.sync_to(false, 1)
	_check(hotbar.get_node("PauseButton").text == "⏸", "running shows the pause glyph")
	hotbar.sync_to(true, 16)  # e.g. the top bar's ▶▶ then a focus-loss auto-pause
	_check(hotbar.get_node("PauseButton").text == "▶", "paused shows the play glyph")
	_check(
		hotbar.get_node("Speed16").get_theme_color("font_color") == UiTheme.ACCENT,
		"the live speed is marked"
	)
	_check(
		hotbar.get_node("Speed1").get_theme_color("font_color") == UiTheme.TEXT,
		"the old speed is unmarked"
	)
	hotbar.free()
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
	# The adjective and the group sentence agree with the Traits row's label.
	for n in [2, 5, 6, 10, 11, 12, 40]:
		var small: String = Codex.describe(
			{"count": n, "diet": 0.9, "habitat": PackedStringArray(["savanna"])}
		)
		var alone: bool = Codex.social_label(n, 0.9) == "Solitary"
		_check(small.contains("solitary") == alone, "n=%d adjective: %s" % [n, small])
		_check(small.contains("alone") == alone, "n=%d group sentence: %s" % [n, small])

	# --- HUD layout: the panels keep out of each other's way ---
	var ui: Dictionary = _ui_props("res://scenes/main.tscn")
	var top_bar: Rect2 = _rect_of(ui["TopBar"])
	# The bar's counters widen it past the authored 500px in a populous world;
	# it must grow away from the minimap, not into it.
	_check(
		int(ui["TopBar"].get("grow_horizontal", 1)) == Control.GROW_DIRECTION_BEGIN,
		"top bar grows leftward"
	)
	_check(top_bar.end.x < _rect_of(ui["Minimap"]).position.x, "top bar ends left of the minimap")
	# The codex hangs under the brand block at the block's real (content) height.
	var codex: Rect2 = _rect_of(ui["CodexPanel"])
	var brand := BrandPanel.new()
	brand.theme = UiTheme.build()
	brand._ready()  # builds the rows; not in the tree yet at _init time
	var brand_bottom: float = _rect_of(ui["Brand"]).position.y + brand.get_combined_minimum_size().y
	brand.free()
	_check(codex.position.y >= brand_bottom + 4.0, "codex clears the brand (%.0f)" % brand_bottom)
	# Only bottom-edge panels shift down at UI scale < 1; the codex is top-left.
	# (main.gd needs the GameConfig autoload to compile, so read its source.)
	for line in FileAccess.get_file_as_string("res://scripts/main.gd").split("\n"):
		if line.begins_with("const HUD_BOTTOM"):
			_check(not line.contains("CodexPanel"), "codex is not a bottom-edge panel")
	# The [Y] and [X] charts open under the top bar and clear of the event log.
	var coevo: Rect2 = _rect_of(ui["CoevolutionPanel"])
	_check(coevo.position.y > top_bar.end.y, "co-evolution chart starts under the top bar")
	# Its legend rows never pack tighter than they can be read, however many
	# series a sub-chart has (as _draw sizes them over the authored height).
	var coevo_charts: Array = CoevolutionPanel.CHARTS
	var chart_plot_h: float = (coevo.size.y - 24.0) / float(coevo_charts.size()) - 8.0
	for chart in coevo_charts:
		var n_series: int = (chart["series"] as Array).size()
		var lay: Dictionary = CoevolutionPanel.legend_layout(n_series, chart_plot_h)
		_check(
			float(lay["step"]) >= CoevolutionPanel.LEGEND_MIN_STEP,
			"%s legend rows %.1fpx apart" % [chart["title"], float(lay["step"])]
		)
		_check(int(lay["cols"]) * int(lay["rows"]) >= n_series, "every series has a legend slot")
	_check(HelixPanel.PANEL_RECT.position.y > top_bar.end.y, "helix starts under the top bar")
	_check(HelixPanel.value_label("farming", -0.0001) == "farming 0.00", "helix: no -0.00")
	_check(HelixPanel.value_label("fire", -0.25) == "fire -0.25", "helix keeps real negatives")
	_check(
		HelixPanel.PANEL_RECT.position.x > _rect_of(ui["EventLog"]).end.x,
		"helix clears the event log"
	)
	# The [T] panel opens beside the codex, not on top of it.
	_check(EvolutionPanel.LEFT_X > codex.end.x, "evolution panel sits right of the codex")
	_check(EvolutionPanel.TOP_Y > top_bar.end.y, "evolution panel starts under the top bar")
	# [H] swaps the bottom-left slot with the event log instead of stacking on it.
	var ui_box := Control.new()
	var log_panel := Control.new()
	log_panel.name = "EventLog"
	ui_box.add_child(log_panel)
	var legend: Control = LegendPanel.new()
	ui_box.add_child(legend)
	legend.visible = false  # as _ready leaves it
	legend.visibility_changed.connect(legend._swap_with_event_log)
	legend.visible = true
	_check(not log_panel.visible, "showing the legend hides the event log")
	legend.visible = false
	_check(log_panel.visible, "hiding the legend brings the event log back")
	ui_box.free()
	for line in LegendPanel.EMOTES_KEY.split("\n"):
		_check(line.length() <= 50, "legend emote line fits the slot: " + line)

	if _failed:
		quit(1)
		return
	print("test_hud_icons: all passed")
	quit(0)
