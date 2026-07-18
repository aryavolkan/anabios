extends RefCounted

# Module glyph palette — indexed by module type id (0..8). Single source of
# truth shared by the body-glyph layers (main.gd) and the legend color key.
const MODULE_COLORS: PackedColorArray = [
	Color(0.6, 0.8, 1.0),   # 0 Locomotor
	Color(0.4, 0.9, 0.5),   # 1 Sensor
	Color(1.0, 0.7, 0.3),   # 2 Mouth
	Color(1.0, 0.3, 0.3),   # 3 Weapon
	Color(0.7, 0.7, 0.7),   # 4 Armor
	Color(0.9, 0.9, 0.4),   # 5 Storage
	Color(0.8, 0.5, 1.0),   # 6 Communicator
	Color(0.5, 1.0, 0.9),   # 7 Pheromone
	Color(1.0, 0.5, 0.8),   # 8 Reproductive
]
const MODULE_NAMES: PackedStringArray = [
	"Locomotor", "Sensor", "Mouth", "Weapon", "Armor",
	"Storage", "Communicator", "Pheromone", "Reproductive",
]
