extends PanelContainer

# Cap the module names listed so a module-heavy agent doesn't grow the panel
# past its slot into the species panel below it.
const MAX_MODULE_NAMES: int = 5

var pinned_id: int = -1

@onready var sim = get_node("/root/Main/Simulation")
@onready var label: Label = $VBoxContainer/Label

func _ready() -> void:
	# Long lines (the module list) were clipping off the panel's right edge;
	# wrap them within the panel width instead.
	label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	# Smaller font so the (up to ~8-line) detail always fits the panel's fixed
	# slot instead of growing down into the species panel below it.
	label.add_theme_font_size_override("font_size", 12)

func pin(id: int) -> void:
	pinned_id = id
	visible = id >= 0

func _process(_delta: float) -> void:
	if pinned_id < 0:
		return
	var info: Dictionary = sim.agent_detail(pinned_id)
	if info.is_empty() or not info.get("alive", false):
		label.text = "(agent %d is dead)" % pinned_id
		return
	# Cap the module list so a module-heavy agent's wrapped names don't grow the
	# panel down into the species panel below (the total count is on the line
	# above). The full count is "module_count".
	var mods: Array = info["module_names"]
	var mod_str: String
	if mods.size() <= MAX_MODULE_NAMES:
		mod_str = ", ".join(mods)
	else:
		mod_str = ", ".join(mods.slice(0, MAX_MODULE_NAMES)) + " +%d" % (mods.size() - MAX_MODULE_NAMES)
	var lines: PackedStringArray = [
		"id %d   species %d   lineage %d" % [pinned_id, info["species_id"], info["lineage_id"]],
		"energy %.1f   age %d" % [info["energy"], info["age"]],
		"program %d   modules %d" % [info["program_len"], info["module_count"]],
		"diet %.2f (0=herb 1=carn)" % info["diet_carnivory"],
		"skill %.2f   technique %.2f" % [info["skill"], info["technique"]],
		"learn: indiv=%s social=%s" % [str(info["indiv_learn"]), str(info["social_learn"])],
		"modules: %s" % mod_str,
	]
	var held: Array = info.get("inventions", [])
	if not held.is_empty():
		lines.append("tech era %d: %s" % [int(info["tech_era"]), ", ".join(held)])
	label.text = "\n".join(lines)
