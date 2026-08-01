extends PanelContainer

const ApeSprites = preload("res://scripts/ape_sprites.gd")

# Cap the module names listed so a module-heavy agent doesn't grow the panel
# past its slot into the species panel below it.
const MAX_MODULE_NAMES: int = 5

var pinned_id: int = -1

@onready var sim = get_node("/root/Main/Simulation")
@onready var label: Label = $VBoxContainer/Label

# Avatar header: the pinned agent rendered as its hominin (the agents ARE the
# apes of DIT), plus a lineage-hue chip. Built once; only the texture reference
# and text change per frame.
var _avatar: TextureRect
var _hue: ColorRect
var _ident: Label
var _ape_tex: Array = []


func _ready() -> void:
	# Long lines (the module list) were clipping off the panel's right edge;
	# wrap them within the panel width instead.
	label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	# Smaller font so the (up to ~8-line) detail always fits the panel's fixed
	# slot instead of growing down into the species panel below it.
	label.add_theme_font_size_override("font_size", 12)
	_build_header()


# A header row above the detail: [lineage-hue chip][ape sprite][name + ids].
func _build_header() -> void:
	for i in ApeSprites.NAMES.size():
		_ape_tex.append(ApeSprites.build(i))
	var header := HBoxContainer.new()
	header.add_theme_constant_override("separation", 8)
	_hue = ColorRect.new()
	_hue.custom_minimum_size = Vector2(5, 46)
	# Portrait tile: a slightly lifted, warm-neutral backdrop so dark hominins
	# (chimp/gorilla) read as silhouettes against the near-black panel.
	var stage := Panel.new()
	stage.custom_minimum_size = Vector2(46, 46)
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color("2b342f")
	sb.set_corner_radius_all(4)
	stage.add_theme_stylebox_override("panel", sb)
	_avatar = TextureRect.new()
	_avatar.set_anchors_preset(Control.PRESET_FULL_RECT)
	_avatar.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_avatar.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	_avatar.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	stage.add_child(_avatar)
	_ident = Label.new()
	_ident.add_theme_font_size_override("font_size", 12)
	_ident.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	header.add_child(_hue)
	header.add_child(stage)
	header.add_child(_ident)
	var vb: VBoxContainer = $VBoxContainer
	vb.add_child(header)
	vb.move_child(header, 0)


func pin(id: int) -> void:
	pinned_id = id
	visible = id >= 0


func _process(_delta: float) -> void:
	if pinned_id < 0:
		return
	var info: Dictionary = sim.agent_detail(pinned_id)
	if info.is_empty() or not info.get("alive", false):
		label.text = "(agent %d is dead)" % pinned_id
		_ident.text = "—"
		return
	# Avatar: pick the hominin from the species id, wash the chip with a stable
	# per-species hue so a lineage keeps a recognizable colour.
	var sp: int = int(info["species_id"])
	var ape: int = ApeSprites.ape_for_species(sp)
	_avatar.texture = _ape_tex[ape]
	_hue.color = Color.from_hsv(fmod(float(sp) * 0.61803, 1.0), 0.5, 0.95)
	_ident.text = (
		"%s\nsp %d · lin %d · id %d" % [ApeSprites.NAMES[ape], sp, info["lineage_id"], pinned_id]
	)
	# Cap the module list so a module-heavy agent's wrapped names don't grow the
	# panel down into the species panel below (the total count is on the line
	# above). The full count is "module_count".
	var mods: Array = info["module_names"]
	var mod_str: String
	if mods.size() <= MAX_MODULE_NAMES:
		mod_str = ", ".join(mods)
	else:
		mod_str = (
			", ".join(mods.slice(0, MAX_MODULE_NAMES)) + " +%d" % (mods.size() - MAX_MODULE_NAMES)
		)
	var lines: PackedStringArray = [
		"energy %.1f   age %d" % [info["energy"], info["age"]],
		"program %d   modules %d" % [info["program_len"], info["module_count"]],
		"diet %.2f (0=herb 1=carn)" % info["diet_carnivory"],
		(
			"skill %.2f   technique %.2f   IQ %.2f"
			% [info["skill"], info["technique"], info.get("iq", 0.0)]
		),
		"learn: indiv=%s social=%s" % [str(info["indiv_learn"]), str(info["social_learn"])],
		"modules: %s" % mod_str,
	]
	if info.get("dimorphism_enabled", false):
		lines.append(
			(
				"sex %s   dimorphism %.2f"
				% ["male" if info["sex_male"] else "female", info["dimorphism"]]
			)
		)
	if info.get("domestication_enabled", false):
		var owner: int = info["livestock_of"]
		lines.append("livestock of agent %d" % owner if owner >= 0 else "wild (not livestock)")
	var held: Array = info.get("inventions", [])
	if not held.is_empty():
		lines.append("tech era %d: %s" % [int(info["tech_era"]), ", ".join(held)])
	if info.get("gene_requirements", false):
		var gated: Array = info.get("gated_inventions", [])
		if not gated.is_empty():
			lines.append("gene-gated: %s" % ", ".join(gated))
	label.text = "\n".join(lines)
