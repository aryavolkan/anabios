extends Node

# Selected scenario + seed, set by the menu and read by the viewer.
var scenario_path: String = "res://../scenarios/minimal.toml"
var seed: int = 12345
# HUD scale from the menu, 0.5..1.0. Capped at 1.0 because the HUD is laid out
# in absolute pixels for a 1280x800 viewport: scaling the UI layer up shrinks
# the logical viewport below the design size, which pushed the species rail, the
# legend and the whole time-control bar off the screen. main.gd _layout_hud()
# re-places the edge-anchored panels for scales below 1.0.
var ui_scale: float = 1.0
# Default display modes for the chosen scenario (see overlay_manager.gd enums).
# 0 = BIOME (ground) / SPECIES (body).
var default_ground: int = 0
var default_body: int = 0
# Set by the showcase director (ANABIOS_SHOWCASE): locks manual camera/time
# input and focus-loss pausing so a recording runs hands-free.
var showcase_active: bool = false
