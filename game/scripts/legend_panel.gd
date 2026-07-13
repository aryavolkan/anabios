extends PanelContainer

const GROUND_NAMES := ["biome", "phero-0", "phero-1", "phero-2", "phero-3", "env-optimum"]
const BODY_NAMES := ["species", "dialect", "diet", "energy"]

@onready var overlay = get_node("/root/Main/OverlayManager")
@onready var label: Label = $Label

# The panel is always visible so the [H] affordance never vanishes; H toggles
# between the full legend and a one-line collapsed hint.
var _expanded: bool = true

func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_H:
		_expanded = not _expanded

func _process(_delta: float) -> void:
	if not _expanded:
		label.text = "[H] show controls"
		return
	var g: int = clampi(overlay.ground_mode, 0, GROUND_NAMES.size() - 1)
	var b: int = clampi(overlay.body_mode, 0, BODY_NAMES.size() - 1)
	label.text = (
		"[G] ground: %s\n[C] body: %s\n[H] hide this\nWASD/drag pan · wheel zoom · click inspect"
	) % [GROUND_NAMES[g], BODY_NAMES[b]]
