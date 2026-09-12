extends PanelContainer
# Codex book (Phase 5 of docs/superpowers/specs/2026-09-12-pixel-world-at-
# scale-design.md): the reference boards' left-hand tabbed panel. Tabs:
# Research (the invention rows of research_panel.gd, when the tree is on),
# Species (portrait, diet, size, generation, a generated field note, traits
# and related species from phylogeny()), Biomes (terrain legend with tile
# thumbnails and world shares) and Culture (population means of the base
# meme channels). The codex *event* stream lives in event_log.gd; the chapter
# name/colour tables below stay here because replay_manager.gd,
# showcase_director.gd and settlement_layer.gd preload them from this script.

const ApeSprites = preload("res://scripts/ape_sprites.gd")
const HudIcons = preload("res://scripts/hud_icons.gd")
const MammalSprites = preload("res://scripts/mammal_sprites.gd")
const ResearchPanel = preload("res://scripts/research_panel.gd")
const SpeciesNames = preload("res://scripts/species_names.gd")
const TerrainSprites = preload("res://scripts/terrain_sprites.gd")
const UiTheme = preload("res://scripts/ui_theme.gd")

const CHAPTER_NAMES: PackedStringArray = [
	"Extinction",
	"PopCrash",
	"Speciation",
	"Migration",
	"NovelModule",
	"NovelBehavior",
	"Predation",
	"CombatRaid",
	"ArmsRace",
	"Territory",
	"NichePartition",
	"Dialect",
	"MemeSweep",
	"AlarmCall",
	"Cooperation",
	"PackHunting",
	"HerdCohesion",
	"Discovery",
	"Adoption",
	"BadIdea",
	"BadAdopt",
	"Trade",
	"MaterialLearn",
	"PopCycle",
	"BoomBust",
	"CarryingCap",
	"TrophicCascade",
	"RangeExpand",
	"Segregation",
	"Corridor",
	"Succession",
	"TraitFixed",
	"RapidAdapt",
	"Convergent",
	"Ambush",
	"ToolUse",
	"Flight",
	"Signaling",
	"War",
	"WarEnded",
	"Alliance",
	"KinNetwork",
	"Settlement",
	"Market",
	"Specialists",
	"Tradition",
	"Radiation",
	"Ratchet",
	"Maladaptation",
	"SexSelect",
	"SexRatio",
	"Domesticated",
	"LivestockHerd",
	"Knowledge",
	"MassFright",
	"PanicCascade",
	"FeedingFrenzy",
	"TerritorialRage",
	"MassGrief",
	"HuntedAdapt",
	"Dehydration",
	"Epidemic",
	"MedContain"
]
# One color per event type so the timeline is scannable at a glance (matches
# the co-evolution chart's marker hues where they overlap).
const CHAPTER_COLORS: PackedColorArray = [
	Color(1.0, 0.42, 0.42),  # 0 Extinction  — red
	Color(1.0, 0.62, 0.35),  # 1 PopCrash    — orange
	Color(0.55, 0.85, 1.0),  # 2 Speciation  — cyan
	Color(0.65, 0.75, 1.0),  # 3 Migration   — blue
	Color(1.0, 0.85, 0.4),  # 4 NovelModule — amber
	Color(0.55, 0.95, 0.6),  # 5 NovelBehavior — green
	Color(1.0, 0.5, 0.5),  # 6 Predation   — salmon
	Color(1.0, 0.55, 0.3),  # 7 CombatRaid  — deep orange
	Color(1.0, 0.5, 0.85),  # 8 ArmsRace    — magenta
	Color(0.45, 0.9, 0.85),  # 9 Territory   — teal
	Color(0.7, 0.95, 0.5),  # 10 NichePartition — lime
	Color(1.0, 0.7, 0.35),  # 11 Dialect    — light orange
	Color(1.0, 0.9, 0.4),  # 12 MemeSweep  — yellow
	Color(1.0, 0.6, 0.75),  # 13 AlarmCall  — pink
	Color(0.6, 1.0, 0.7),  # 14 Cooperation — mint
	Color(0.85, 0.7, 0.5),  # 15 PackHunting — tan
	Color(0.6, 0.8, 1.0),  # 16 HerdCohesion — sky
	Color(1.0, 0.9, 0.35),  # 17 Discovery  — gold
	Color(0.55, 0.95, 1.0),  # 18 Adoption   — light sky
	Color(0.95, 0.45, 0.55),  # 19 PracticeDiscovered — rose
	Color(0.85, 0.3, 0.3),  # 20 PracticeAdopted    — dark red
	Color(0.95, 0.8, 0.45),  # 21 ResourceTraded     — wheat
	Color(0.8, 0.95, 0.9),  # 22 MaterialLearning    — pale mint
	Color(0.55, 0.75, 1.0),  # 23 PopCycle       — sky blue
	Color(1.0, 0.6, 0.25),  # 24 BoomBust       — hot orange
	Color(0.5, 0.9, 0.55),  # 25 CarryingCap    — steady green
	Color(0.75, 0.55, 1.0),  # 26 TrophicCascade — violet
	Color(0.45, 0.85, 0.5),  # 27 RangeExpand    — spring green
	Color(0.95, 0.75, 0.4),  # 28 Segregation    — sand
	Color(0.6, 0.7, 0.95),  # 29 Corridor       — periwinkle
	Color(0.55, 0.9, 0.75),  # 30 Succession     — new growth
	Color(0.85, 0.85, 0.5),  # 31 TraitFixed     — olive
	Color(1.0, 0.55, 0.45),  # 32 RapidAdapt     — vermillion
	Color(0.5, 0.8, 0.9),  # 33 Convergent     — steel blue
	Color(0.9, 0.5, 0.4),  # 34 Ambush        — ember
	Color(0.8, 0.8, 0.65),  # 35 ToolUse       — bone
	Color(0.55, 0.9, 1.0),  # 36 Flight        — sky
	Color(0.95, 0.85, 0.5),  # 37 Signaling     — beacon
	Color(1.0, 0.35, 0.3),  # 38 War           — blood red
	Color(0.7, 0.7, 0.75),  # 39 WarEnded      — ash
	Color(0.55, 0.95, 0.8),  # 40 Alliance      — pact teal
	Color(0.9, 0.8, 0.55),  # 41 KinNetwork    — family gold
	Color(0.85, 0.65, 0.4),  # 42 Settlement    — hearth
	Color(1.0, 0.8, 0.3),  # 43 Market        — amber market
	Color(0.65, 0.85, 0.95),  # 44 SpecializationSplit — craftsman blue
	Color(0.9, 0.75, 0.5),  # 45 Tradition      — parchment
	Color(0.7, 0.6, 1.0),  # 46 Radiation      — violet bloom
	Color(0.5, 0.85, 0.7),  # 47 Ratchet        — institutional green
	Color(0.8, 0.45, 0.35),  # 48 Maladaptation  — stranded rust
	Color(1.0, 0.55, 0.75),  # 49 SexualSelection — display pink
	Color(0.65, 0.55, 0.9),  # 50 SexRatioCollapse — faint violet
	Color(0.85, 0.7, 0.45),  # 51 AnimalDomesticated — leather tan
	Color(0.6, 0.85, 0.55),  # 52 LivestockHerd     — pasture green
	Color(0.4, 0.7, 0.95),  # 53 Knowledge         — archive blue
	Color(0.85, 0.5, 0.9),  # 54 MassFright        — startled violet
	Color(1.0, 0.4, 0.35),  # 55 PanicCascade      — alarm red
	Color(1.0, 0.75, 0.3),  # 56 FeedingFrenzy     — hungry amber
	Color(0.9, 0.3, 0.2),  # 57 TerritorialRage    — furious rust
	Color(0.5, 0.55, 0.75),  # 58 MassGrief        — mourning slate
	Color(1.0, 0.85, 0.45),  # 59 HuntedAdaptation — hunted gold
	Color(0.4, 0.8, 0.9),  # 60 Dehydration       — parched aqua
	Color(0.75, 0.95, 0.4),  # 61 EpidemicOutbreak    — fever chartreuse
	Color(0.4, 0.9, 0.7),  # 62 MedicineContainment — apothecary teal
]

enum { TAB_RESEARCH, TAB_SPECIES, TAB_BIOMES, TAB_CULTURE }
const TAB_NAMES: PackedStringArray = ["Research", "Species", "Biomes", "Culture"]
const TAB_ICONS: PackedInt32Array = [HudIcons.ERA, HudIcons.PAW, HudIcons.LEAF, HudIcons.BOOK]
const PAGE_W := 262
const REFRESH_EVERY := 45
# Culture and biome data change slowly; the whole-world terrain scan is the
# costliest query here, so it runs rarely and on a sample (see terrain_shares).
const BIOME_REFRESH_EVERY := 300
const TERRAIN_SAMPLES := 16384
# Members scanned per species for the field note's habitat and trait shares.
const MEMBER_SCAN := 256
const PORTRAIT := 64
const RELATED_TILE := 40
const RELATED_MAX := 4
const BAR_BG := Color(0.06, 0.09, 0.11)
const BAR_FILL := Color(0.42, 0.80, 0.36)
const BAR_CULTURE := Color(0.60, 0.55, 0.95)
# Base meme channels (meme_channel_catalog order) shown on the Culture tab.
const CULTURE_CHANNELS: Dictionary = {
	0: "Alarm calls", 1: "Dialect", 2: "Cooperation", 3: "Hunting lore", 5: "Skill", 6: "Technique"
}

@onready var sim = get_node("../../Simulation")

var _tab: int = TAB_SPECIES
var _tab_buttons: Array[Button] = []
var _pages: Array[Control] = []
var _research: PanelContainer
var _font: Font
var _frame: int = 0

# Species tab state.
var _species: int = -1
var _profile: Dictionary = {}  # per-species means/shares plus id, gen, era, habitat, portrait
var _related: Array[Dictionary] = []  # [{id, count, tex, tint}]
var _related_rects: Array[Rect2] = []
var _portrait_cache: Dictionary = {}  # archetype -> ImageTexture
var _ape_tex: Array = []
# Biomes tab state.
var _shares: PackedFloat32Array = PackedFloat32Array()
var _tile_tex: Array = []
# Culture tab state.
var _meme_means: PackedFloat32Array = PackedFloat32Array()


func _ready() -> void:
	assert(CHAPTER_NAMES.size() == CHAPTER_COLORS.size(), "codex name/color arrays out of sync")
	var core_count: int = int(sim.event_type_count())
	if CHAPTER_NAMES.size() != core_count:
		push_warning(
			(
				"codex: %d display event types vs %d core EventTypes — add the missing name/color"
				% [CHAPTER_NAMES.size(), core_count]
			)
		)
	_font = get_theme_default_font()
	if _font == null:
		_font = ThemeDB.fallback_font
	for i in ApeSprites.NAMES.size():
		_ape_tex.append(ApeSprites.build(i))
	for t in TerrainSprites.NAMES.size():
		_tile_tex.append(ImageTexture.create_from_image(TerrainSprites.tile_image(t, 0)))
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 4)
	add_child(box)
	var title := HBoxContainer.new()
	title.add_theme_constant_override("separation", 6)
	var icon := TextureRect.new()
	icon.texture = HudIcons.kind_texture(HudIcons.BOOK)
	icon.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	icon.stretch_mode = TextureRect.STRETCH_KEEP_CENTERED
	icon.custom_minimum_size = Vector2(16, 16)
	title.add_child(icon)
	var lbl := Label.new()
	lbl.text = "CODEX"
	lbl.add_theme_font_size_override("font_size", 13)
	lbl.add_theme_color_override("font_color", UiTheme.ACCENT)
	title.add_child(lbl)
	box.add_child(title)
	var tabs := HBoxContainer.new()
	tabs.add_theme_constant_override("separation", 3)
	for i in TAB_NAMES.size():
		var b := Button.new()
		b.text = TAB_NAMES[i]
		b.icon = HudIcons.kind_texture(TAB_ICONS[i])
		b.add_theme_font_size_override("font_size", 11)
		b.pressed.connect(_select_tab.bind(i))
		tabs.add_child(b)
		_tab_buttons.append(b)
	box.add_child(tabs)
	# Pages share one slot; only the active one is visible.
	_research = ResearchPanel.new()
	_research.embedded = true
	_pages.append(_research)
	for spec in [[_draw_species, 296], [_draw_biomes, 234], [_draw_culture, 152]]:
		var page := Control.new()
		page.custom_minimum_size = Vector2(PAGE_W, int(spec[1]))
		page.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		page.draw.connect(spec[0])
		_pages.append(page)
	_pages[TAB_SPECIES].gui_input.connect(_on_species_input)
	for p in _pages:
		box.add_child(p)
	if OS.has_environment("ANABIOS_CODEX_TAB"):
		_tab = clampi(int(OS.get_environment("ANABIOS_CODEX_TAB")), 0, TAB_NAMES.size() - 1)
	_select_tab(_tab)


func _select_tab(i: int) -> void:
	_tab = i
	for k in _pages.size():
		_pages[k].visible = k == i
		var col: Color = UiTheme.ACCENT if k == i else UiTheme.TEXT_DIM
		_tab_buttons[k].add_theme_color_override("font_color", col)
	_frame = REFRESH_EVERY - 1  # refresh on the next frame


func _process(_delta: float) -> void:
	var inventions: bool = bool(sim.inventions_enabled())
	_tab_buttons[TAB_RESEARCH].visible = inventions
	if _tab == TAB_RESEARCH and not inventions:
		_select_tab(TAB_SPECIES)
		return
	_frame += 1
	if _frame % REFRESH_EVERY != 0:
		return
	match _tab:
		TAB_SPECIES:
			_refresh_species()
		TAB_BIOMES:
			if _shares.is_empty() or _frame % BIOME_REFRESH_EVERY == 0:
				_shares = terrain_shares(sim.biome_terrain_ids(), TERRAIN_SAMPLES)
			_pages[TAB_BIOMES].queue_redraw()
		TAB_CULTURE:
			var snap: Dictionary = sim.helix_snapshot()
			_meme_means = snap.get("meme_means", PackedFloat32Array())
			_pages[TAB_CULTURE].queue_redraw()


# --- pure helpers (headless-tested) ---------------------------------------


static func diet_label(d: float) -> String:
	if d < MammalSprites.HERB_MAX:
		return "Herbivore"
	if d < MammalSprites.CARN_MIN:
		return "Omnivore"
	return "Carnivore"


# The Traits row's food word for a mean carnivory.
static func diet_word(d: float) -> String:
	if d < MammalSprites.HERB_MAX:
		return "Plants"
	if d < MammalSprites.CARN_MIN:
		return "Mixed"
	return "Meat"


static func level_label(share: float) -> String:
	if share >= 0.5:
		return "High"
	if share >= 0.2:
		return "Medium"
	return "Low"


static func social_label(count: int, diet: float) -> String:
	if count < 6:
		return "Solitary"
	if diet >= MammalSprites.CARN_MIN:
		return "Pack"
	if diet >= MammalSprites.HERB_MAX:
		return "Band"
	return "Herd"


# Terrain shares over a sample of the whole-world id buffer (0..8 per
# terrain_sprites.NAMES); at most `max_samples` cells are read.
static func terrain_shares(ids: PackedByteArray, max_samples: int) -> PackedFloat32Array:
	var out := PackedFloat32Array()
	out.resize(TerrainSprites.NAMES.size())
	out.fill(0.0)
	if ids.is_empty():
		return out
	var step: int = maxi(1, int(ceil(float(ids.size()) / float(maxi(1, max_samples)))))
	var n := 0
	var i := 0
	while i < ids.size():
		var t: int = ids[i]
		if t >= 0 and t < out.size():
			out[t] += 1.0
		n += 1
		i += step
	for t in out.size():
		out[t] /= float(n)
	return out


# The two most-visited terrains of a species, lowercase, for the field note.
static func habitat_names(hist: PackedFloat32Array) -> PackedStringArray:
	var order: Array[int] = []
	for t in hist.size():
		if hist[t] > 0.0:
			order.append(t)
	order.sort_custom(func(a, b): return hist[a] > hist[b])
	var out := PackedStringArray()
	for k in mini(2, order.size()):
		out.append(TerrainSprites.NAMES[order[k]].to_lower())
	return out


# One or two sentences from a species profile (see profile_of): the
# reference's "Fast, agile and social. Often found in herds near ...".
static func describe(p: Dictionary) -> String:
	var adj: PackedStringArray = []
	adj.append("Fast" if float(p.get("speed", 0.0)) >= 0.5 else "Steady")
	if float(p.get("armour", 0.0)) >= 0.3:
		adj.append("armoured")
	if float(p.get("spines", 0.0)) >= 0.3:
		adj.append("spiny")
	if float(p.get("jaws", 0.0)) >= 0.3:
		adj.append("sharp-jawed")
	var count: int = int(p.get("count", 0))
	adj.append("social" if count >= 12 else "solitary")
	var first: String
	if adj.size() == 1:
		first = adj[0]
	else:
		first = ", ".join(adj.slice(0, adj.size() - 1)) + " and " + adj[adj.size() - 1]
	var text: String = first + "."
	var habitat: PackedStringArray = p.get("habitat", PackedStringArray())
	if not habitat.is_empty():
		var group: String = social_label(count, float(p.get("diet", 0.5))).to_lower()
		var where: String = " and ".join(habitat)
		if group == "solitary":
			text += " Often found alone near %s." % where
		else:
			text += " Often found in %ss near %s." % [group, where]
	return text


# --- species tab -----------------------------------------------------------


# One pass over the live population: per-species means and shares, plus the
# habitat histogram of the selected species. Returns
# {sp: {count, diet, size, speed, armour, spines, jaws, storage, livestock}}
# and fills `habitat` for `focus` (fractions over terrain ids).
func _scan(focus: int) -> Dictionary:
	var sps: PackedInt32Array = sim.alive_species_ids()
	var diets: PackedFloat32Array = sim.alive_diet()
	var sizes: PackedFloat32Array = sim.alive_sizes()
	var tags: PackedInt32Array = sim.alive_body_tags()
	var live: PackedInt32Array = sim.alive_livestock_flags()
	var pos: PackedVector2Array = sim.alive_positions()
	var ids: PackedByteArray = PackedByteArray()
	var res: int = 0
	var world: float = float(sim.world_size())
	var hist := PackedFloat32Array()
	hist.resize(TerrainSprites.NAMES.size())
	hist.fill(0.0)
	var scanned := 0
	var acc: Dictionary = {}
	for i in sps.size():
		var sp: int = sps[i]
		var a: Dictionary = acc.get(sp, {})
		if a.is_empty():
			a = {
				"count": 0,
				"diet": 0.0,
				"size": 0.0,
				"speed": 0.0,
				"armour": 0.0,
				"spines": 0.0,
				"jaws": 0.0,
				"storage": 0.0,
				"livestock": 0.0,
			}
			acc[sp] = a
		a["count"] += 1
		a["diet"] += diets[i] if i < diets.size() else 0.5
		a["size"] += sizes[i] if i < sizes.size() else 1.0
		var tg: int = tags[i] if i < tags.size() else 0
		a["speed"] += 1.0 if tg & MammalSprites.TAG_LOCOMOTOR2 else 0.0
		a["armour"] += 1.0 if tg & MammalSprites.TAG_ARMOR else 0.0
		a["spines"] += 1.0 if tg & MammalSprites.TAG_SPINES else 0.0
		a["jaws"] += 1.0 if tg & MammalSprites.TAG_JAWS else 0.0
		a["storage"] += 1.0 if tg & MammalSprites.TAG_STORAGE else 0.0
		a["livestock"] += 1.0 if i < live.size() and live[i] != 0 else 0.0
		if sp == focus and scanned < MEMBER_SCAN and i < pos.size():
			if ids.is_empty():
				ids = sim.biome_terrain_ids()
				res = int(sim.biome_resolution())
			if res > 0 and world > 0.0:
				var cx: int = posmod(int(pos[i].x / world * float(res)), res)
				var cy: int = posmod(int(pos[i].y / world * float(res)), res)
				var t: int = ids[cy * res + cx]
				if t >= 0 and t < hist.size():
					hist[t] += 1.0
				scanned += 1
	for sp in acc.keys():
		var a: Dictionary = acc[sp]
		var n: float = float(a["count"])
		for k in ["diet", "size", "speed", "armour", "spines", "jaws", "storage", "livestock"]:
			a[k] = float(a[k]) / n
	if scanned > 0:
		for t in hist.size():
			hist[t] /= float(scanned)
	acc["_habitat"] = hist
	return acc


# The species to show: the pinned agent's, else the one already shown while
# it lives, else the most populous.
func _pick_species(acc: Dictionary) -> int:
	var insp: Node = get_node_or_null("../Inspector")
	if insp != null and int(insp.get("pinned_id")) >= 0:
		var info: Dictionary = sim.get_agent_info(int(insp.get("pinned_id")))
		if bool(info.get("alive", false)):
			return int(info["species_id"])
	if _species >= 0 and acc.has(_species):
		return _species
	var best := -1
	var best_n := 0
	for sp in acc.keys():
		if sp is int and int(acc[sp]["count"]) > best_n:
			best = sp
			best_n = int(acc[sp]["count"])
	return best


func _portrait_for(sp: int, a: Dictionary) -> Dictionary:
	var tags := 0
	if float(a["armour"]) >= 0.5:
		tags |= MammalSprites.TAG_ARMOR
	if float(a["spines"]) >= 0.5:
		tags |= MammalSprites.TAG_SPINES
	if float(a["jaws"]) >= 0.5:
		tags |= MammalSprites.TAG_JAWS
	if float(a["storage"]) >= 0.5:
		tags |= MammalSprites.TAG_STORAGE
	if float(a["speed"]) >= 0.5:
		tags |= MammalSprites.TAG_LOCOMOTOR2
	var arch: int = MammalSprites.archetype_for(
		float(a["diet"]), float(a["size"]), float(a["livestock"]) >= 0.5, tags
	)
	if arch == MammalSprites.PRIMATE:
		var ape: int = MammalSprites.primate_skin_for(sp)
		return {"tex": _ape_tex[ape], "tint": Color.WHITE, "kind": ApeSprites.NAMES[ape]}
	var key := "%d" % arch
	if not _portrait_cache.has(key):
		_portrait_cache[key] = MammalSprites.portrait(arch)
	return {
		"tex": _portrait_cache[key],
		"tint": MammalSprites.coat_hue(arch, sp),
		"kind": MammalSprites.NAMES[arch],
	}


func _refresh_species() -> void:
	var acc: Dictionary = _scan(_species)
	var sp: int = _pick_species(acc)
	if sp != _species:
		_species = sp
		if sp >= 0:
			acc = _scan(sp)  # habitat for the newly picked species
	if sp < 0 or not acc.has(sp):
		_profile = {}
		_related.clear()
		_pages[TAB_SPECIES].queue_redraw()
		return
	var a: Dictionary = acc[sp]
	var gen := 0
	var parent := -1
	var kin: Array[int] = []
	for d in sim.phylogeny():
		var id: int = int(d["id"])
		if id == sp:
			gen = int(d["depth"])
			parent = int(d["parent"])
	for d in sim.phylogeny():
		var id: int = int(d["id"])
		if id == sp or not acc.has(id):
			continue
		if int(d["parent"]) == sp or id == parent or (parent >= 0 and int(d["parent"]) == parent):
			kin.append(id)
	kin.sort_custom(func(x, y): return int(acc[x]["count"]) > int(acc[y]["count"]))
	var era := 0
	for st in sim.species_stats():
		if int(st["species_id"]) == sp:
			era = int(st["tech_era"])
	var port: Dictionary = _portrait_for(sp, a)
	_profile = a.duplicate()
	_profile["id"] = sp
	_profile["gen"] = gen
	_profile["era"] = era
	_profile["habitat"] = habitat_names(acc["_habitat"])
	_profile["tex"] = port["tex"]
	_profile["tint"] = port["tint"]
	_profile["kind"] = port["kind"]
	_related.clear()
	for id in kin.slice(0, RELATED_MAX):
		var rp: Dictionary = _portrait_for(id, acc[id])
		_related.append(
			{"id": id, "count": int(acc[id]["count"]), "tex": rp["tex"], "tint": rp["tint"]}
		)
	_pages[TAB_SPECIES].queue_redraw()


func _draw_species() -> void:
	var page: Control = _pages[TAB_SPECIES]
	_related_rects.clear()
	if _profile.is_empty():
		page.draw_string(
			_font,
			Vector2(0, 20),
			"No living species",
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			12,
			UiTheme.TEXT_DIM
		)
		return
	# Portrait stage.
	page.draw_rect(Rect2(0, 0, PORTRAIT, PORTRAIT), UiTheme.BG_ELEV)
	page.draw_rect(Rect2(0, 0, PORTRAIT, PORTRAIT), UiTheme.ACCENT_DIM, false, 1.0)
	var tex: Texture2D = _profile["tex"]
	page.draw_texture_rect(tex, Rect2(0, 0, PORTRAIT, PORTRAIT), false, _profile["tint"])
	var x := PORTRAIT + 10
	var sp: int = int(_profile["id"])
	page.draw_string(
		_font,
		Vector2(x, 15),
		SpeciesNames.name_of(sp),
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		14,
		UiTheme.TEXT
	)
	var diet: float = float(_profile["diet"])
	var diet_icon: int = HudIcons.LEAF if diet < MammalSprites.CARN_MIN else HudIcons.MEAT
	page.draw_texture_rect(HudIcons.kind_texture(diet_icon), Rect2(x, 22, 16, 16), false)
	page.draw_string(
		_font,
		Vector2(x + 20, 34),
		diet_label(diet),
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		12,
		UiTheme.TEXT
	)
	page.draw_string(
		_font,
		Vector2(x, 50),
		"Size: %.1f m" % float(_profile["size"]),
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		12,
		UiTheme.TEXT
	)
	page.draw_string(
		_font,
		Vector2(x, 64),
		"Gen: %d   sp %d   n=%d" % [int(_profile["gen"]), sp, int(_profile["count"])],
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		12,
		UiTheme.TEXT_DIM
	)
	var y := PORTRAIT + 18
	page.draw_multiline_string(
		_font,
		Vector2(0, y),
		describe(_profile),
		HORIZONTAL_ALIGNMENT_LEFT,
		PAGE_W,
		12,
		3,
		UiTheme.TEXT
	)
	y += 46
	page.draw_line(Vector2(0, y), Vector2(PAGE_W, y), UiTheme.ACCENT_DIM, 1.0)
	y += 14
	page.draw_string(
		_font, Vector2(0, y), "Traits", HORIZONTAL_ALIGNMENT_LEFT, -1, 12, UiTheme.ACCENT
	)
	var traits: PackedStringArray = [
		"Speed: %s" % level_label(float(_profile["speed"])),
		"Defence: %s" % level_label(float(_profile["armour"]) + float(_profile["spines"])),
		"Diet: %s" % diet_word(diet),
		"Social: %s" % social_label(int(_profile["count"]), diet),
	]
	if int(_profile["era"]) > 0:
		traits.append("Tech: era %d" % int(_profile["era"]))
	for k in traits.size():
		var col: int = k % 2
		var row: int = int(k / 2.0)
		page.draw_string(
			_font,
			Vector2(4 + col * 128, y + 15 + row * 14),
			traits[k],
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			11,
			UiTheme.TEXT
		)
	y += 15 + int(ceil(traits.size() / 2.0)) * 14 + 4
	page.draw_line(Vector2(0, y), Vector2(PAGE_W, y), UiTheme.ACCENT_DIM, 1.0)
	y += 14
	page.draw_string(
		_font, Vector2(0, y), "Related Species", HORIZONTAL_ALIGNMENT_LEFT, -1, 12, UiTheme.ACCENT
	)
	y += 6
	if _related.is_empty():
		page.draw_string(
			_font,
			Vector2(4, y + 14),
			"none alive",
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			11,
			UiTheme.TEXT_DIM
		)
	for k in _related.size():
		var r := Rect2(k * (RELATED_TILE + 6), y, RELATED_TILE, RELATED_TILE)
		_related_rects.append(r)
		page.draw_rect(r, UiTheme.BG_ELEV)
		page.draw_rect(r, UiTheme.ACCENT_DIM, false, 1.0)
		var rt: Texture2D = _related[k]["tex"]
		page.draw_texture_rect(rt, r.grow(-4), false, _related[k]["tint"])


func _on_species_input(event: InputEvent) -> void:
	if not (event is InputEventMouseButton):
		return
	var mb := event as InputEventMouseButton
	if not mb.pressed or mb.button_index != MOUSE_BUTTON_LEFT:
		return
	for k in _related_rects.size():
		if _related_rects[k].has_point(mb.position):
			_species = int(_related[k]["id"])
			_refresh_species()
			return


# --- biomes tab ------------------------------------------------------------


func _draw_biomes() -> void:
	var page: Control = _pages[TAB_BIOMES]
	var y := 0
	for t in TerrainSprites.NAMES.size():
		page.draw_texture_rect(_tile_tex[t], Rect2(0, y, 24, 24), false)
		page.draw_rect(Rect2(0, y, 24, 24), UiTheme.ACCENT_DIM, false, 1.0)
		page.draw_string(
			_font,
			Vector2(32, y + 16),
			TerrainSprites.NAMES[t],
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			12,
			UiTheme.TEXT
		)
		var share: float = _shares[t] if t < _shares.size() else 0.0
		page.draw_rect(Rect2(120, y + 8, 96, 8), BAR_BG)
		page.draw_rect(Rect2(120, y + 8, 96.0 * clampf(share, 0.0, 1.0), 8), BAR_FILL)
		page.draw_rect(Rect2(120, y + 8, 96, 8), UiTheme.ACCENT_DIM, false, 1.0)
		page.draw_string(
			_font,
			Vector2(222, y + 16),
			"%d%%" % int(round(share * 100.0)),
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			11,
			UiTheme.TEXT_DIM
		)
		y += 26


# --- culture tab -----------------------------------------------------------


func _draw_culture() -> void:
	var page: Control = _pages[TAB_CULTURE]
	var y := 0
	page.draw_string(
		_font,
		Vector2(0, 12),
		"Population mean per meme channel",
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		11,
		UiTheme.TEXT_DIM
	)
	y += 20
	for ch in CULTURE_CHANNELS.keys():
		var v: float = _meme_means[ch] if ch < _meme_means.size() else 0.0
		page.draw_texture_rect(
			HudIcons.kind_texture(HudIcons.PEOPLE), Rect2(0, y + 2, 16, 16), false
		)
		page.draw_string(
			_font,
			Vector2(22, y + 14),
			CULTURE_CHANNELS[ch],
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			12,
			UiTheme.TEXT
		)
		page.draw_rect(Rect2(120, y + 6, 96, 8), BAR_BG)
		page.draw_rect(Rect2(120, y + 6, 96.0 * clampf(v, 0.0, 1.0), 8), BAR_CULTURE)
		page.draw_rect(Rect2(120, y + 6, 96, 8), UiTheme.ACCENT_DIM, false, 1.0)
		page.draw_string(
			_font,
			Vector2(222, y + 14),
			"%.2f" % v,
			HORIZONTAL_ALIGNMENT_LEFT,
			-1,
			11,
			UiTheme.TEXT_DIM
		)
		y += 22
