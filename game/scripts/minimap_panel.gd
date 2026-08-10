extends Control

# Whole-world overview in the HUD corner: draws biome_renderer's world texture
# scaled into this Control, overlays the current camera viewport as a rectangle,
# and recenters the camera on click/drag. Pure viewer — no sim state touched.

@onready var sim = get_node("/root/Main/Simulation")
@onready var cam: Camera2D = get_node("/root/Main/Camera2D")
@onready var biome = get_node("/root/Main/Biome")

const BORDER := Color(0.8, 0.85, 0.9, 0.5)
const VIEWRECT := Color(1.0, 1.0, 1.0, 0.9)


func _ready() -> void:
	# STOP so clicks on the minimap are consumed by _gui_input and never fall
	# through to the world agent-pick handler in main.gd:_unhandled_input.
	mouse_filter = Control.MOUSE_FILTER_STOP


func _process(_dt: float) -> void:
	queue_redraw()  # viewport rect tracks the camera each frame; the panel is tiny


func _draw() -> void:
	var world: float = float(sim.world_size())
	if world <= 0.0:
		return
	var ms: Vector2 = size
	var tex = biome.world_texture()
	if tex != null:
		draw_texture_rect(tex, Rect2(Vector2.ZERO, ms), false)
	# Viewport rectangle: the world extent currently visible, centred on the
	# (torus-wrapped) camera position, mapped world→minimap. Clips at edges when
	# the view straddles a wrap seam (acceptable v1).
	var vp: Vector2 = get_viewport_rect().size
	var view_world := Vector2(vp.x / cam.zoom.x, vp.y / cam.zoom.y)
	var center := Vector2(fposmod(cam.position.x, world), fposmod(cam.position.y, world))
	var scale := ms / world
	var top_left := (center - view_world * 0.5) * scale
	draw_rect(Rect2(top_left, view_world * scale), VIEWRECT, false, 2.0)
	# Panel border.
	draw_rect(Rect2(Vector2.ZERO, ms), BORDER, false, 1.0)


func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_LEFT and event.pressed:
			_jump_to(event.position)
	elif event is InputEventMouseMotion:
		if event.button_mask & MOUSE_BUTTON_MASK_LEFT:
			_jump_to(event.position)


# Map a local minimap point to a world position and recenter the camera there.
func _jump_to(local: Vector2) -> void:
	var world: float = float(sim.world_size())
	if world <= 0.0 or size.x <= 0.0 or size.y <= 0.0:
		return
	cam.position = Vector2(local.x / size.x, local.y / size.y) * world
