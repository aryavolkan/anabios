extends PanelContainer

var pinned_id: int = -1

@onready var sim = get_node("/root/Main/Simulation")
@onready var label: Label = $VBoxContainer/Label

func pin(id: int) -> void:
	pinned_id = id
	visible = id >= 0

func _process(_delta: float) -> void:
	if pinned_id < 0:
		return
	var info: Dictionary = sim.get_agent_info(pinned_id)
	if info.is_empty() or not info.get("alive", false):
		label.text = "(agent %d is dead)" % pinned_id
		return
	label.text = (
		"id %d\nspecies %d  lineage %d\nenergy %.1f  age %d\nprogram %d  modules %d"
	) % [
		pinned_id, info["species_id"], info["lineage_id"],
		info["energy"], info["age"],
		info["program_len"], info["module_count"],
	]
