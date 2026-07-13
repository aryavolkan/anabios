extends PanelContainer

const GROUND_NAMES := ["biome", "phero-0", "phero-1", "phero-2", "phero-3", "env-optimum"]
const BODY_NAMES := ["species", "dialect", "diet", "energy"]

@onready var overlay = get_node("/root/Main/OverlayManager")
@onready var label: Label = $Label

func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_H:
		visible = not visible

func _process(_delta: float) -> void:
	if not visible:
		return
	var g: int = clampi(overlay.ground_mode, 0, GROUND_NAMES.size() - 1)
	var b: int = clampi(overlay.body_mode, 0, BODY_NAMES.size() - 1)
	label.text = (
		"[G] ground: %s\n[C] body: %s\n[H] hide this\nWASD/drag pan · wheel zoom · click inspect"
	) % [GROUND_NAMES[g], BODY_NAMES[b]]
