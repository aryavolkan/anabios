extends Node

# Selected scenario + seed, set by the menu and read by the viewer.
var scenario_path: String = "res://../scenarios/minimal.toml"
var seed: int = 12345
var ui_scale: float = 1.0
# Default display modes for the chosen scenario (see overlay_manager.gd enums).
# 0 = BIOME (ground) / SPECIES (body).
var default_ground: int = 0
var default_body: int = 0
# Set by the showcase director (ANABIOS_SHOWCASE): locks manual camera/time
# input and focus-loss pausing so a recording runs hands-free.
var showcase_active: bool = false
