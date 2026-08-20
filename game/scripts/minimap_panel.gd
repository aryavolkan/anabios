extends Control

# Whole-world overview in the HUD corner: draws biome_renderer's world texture
# scaled into this Control, overlays the current camera viewport as a rectangle,
# and recenters the camera on click/drag. Pure viewer — no sim state touched.

@onready var sim = get_node("/root/Main/Simulation")
@onready var cam: Camera2D = get_node("/root/Main/Camera2D")
@onready var biome = get_node("/root/Main/Biome")

const BORDER := Color(0.8, 0.85, 0.9, 0.5)
const VIEWRECT := Color(1.0, 1.0, 1.0, 0.9)
const AGENT_DOT := Color(1.0, 0.75, 0.3, 0.85)


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
	# Always the biome, never the active [G] overlay: a pheromone/market ground
	# turns the world texture near-black, which left the minimap an unreadable
	# black square exactly when you most want the geography to orient by.
	var tex = biome.minimap_texture()
	if tex != null:
		draw_texture_rect(tex, Rect2(Vector2.ZERO, ms), false)
	# Viewport rectangle: the world extent currently visible, centred on the
	# (torus-wrapped) camera position, mapped world→minimap.
	var vp: Vector2 = get_viewport_rect().size
	var view_world := Vector2(vp.x / cam.zoom.x, vp.y / cam.zoom.y)
	var center := Vector2(fposmod(cam.position.x, world), fposmod(cam.position.y, world))
	var scale := ms / world
	# Agents: one small dot each at its world position, so the population's
	# location and spread read at a glance. Overlapping dots brighten into a
	# cluster (natural density cue). ≤ max_population draws, once per frame.
	var positions: PackedVector2Array = sim.alive_positions()
	for p in positions:
		var mp := Vector2(fposmod(p.x, world), fposmod(p.y, world)) * scale
		draw_rect(Rect2(mp - Vector2(1, 1), Vector2(2, 2)), AGENT_DOT)
	_draw_view_rect(center * scale, (view_world * scale).min(ms), ms)
	# Panel border.
	draw_rect(Rect2(Vector2.ZERO, ms), BORDER, false, 1.0)


# The camera box, clipped to the minimap. The world is a torus, so a view
# straddling a seam wraps to the far edge: draw the box and its three wrapped
# copies and intersect each with the panel. (It used to be drawn unclipped and
# spilled outside the minimap's frame onto the HUD whenever the camera sat near
# an edge or zoomed out past the world.)
func _draw_view_rect(center_px: Vector2, box: Vector2, ms: Vector2) -> void:
	var panel := Rect2(Vector2.ZERO, ms)
	var origin := Vector2(
		fposmod(center_px.x - box.x * 0.5, ms.x), fposmod(center_px.y - box.y * 0.5, ms.y)
	)
	# Zoomed out past a world edge the box already spans the whole map on that
	# axis; pin it so the wrap draws one rect around the map instead of two
	# abutting slivers with a seam line through the middle.
	if box.x >= ms.x:
		origin.x = 0.0
	if box.y >= ms.y:
		origin.y = 0.0
	for dx in [0.0, -ms.x]:
		for dy in [0.0, -ms.y]:
			var clipped := Rect2(origin + Vector2(dx, dy), box).intersection(panel)
			if clipped.size.x > 1.0 and clipped.size.y > 1.0:
				draw_rect(clipped, VIEWRECT, false, 2.0)


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
