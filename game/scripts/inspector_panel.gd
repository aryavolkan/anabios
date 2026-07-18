extends PanelContainer

var pinned_id: int = -1

@onready var sim = get_node("/root/Main/Simulation")
@onready var label: Label = $VBoxContainer/Label

func _ready() -> void:
	# Long lines (the module list) were clipping off the panel's right edge;
	# wrap them within the panel width instead.
	label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART

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
	var lines: PackedStringArray = [
		"id %d   species %d   lineage %d" % [pinned_id, info["species_id"], info["lineage_id"]],
		"energy %.1f   age %d" % [info["energy"], info["age"]],
		"program %d   modules %d" % [info["program_len"], info["module_count"]],
		"diet %.2f (0=herb 1=carn)" % info["diet_carnivory"],
		"skill %.2f   technique %.2f" % [info["skill"], info["technique"]],
		"learn: indiv=%s social=%s" % [str(info["indiv_learn"]), str(info["social_learn"])],
		"modules: %s" % ", ".join(info["module_names"]),
	]
	label.text = "\n".join(lines)
