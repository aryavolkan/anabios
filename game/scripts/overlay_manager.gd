extends Node

# Ground layer modes. PHEROMONE_0..3 are contiguous (1..4).
const GROUND_BIOME := 0
const GROUND_PHEROMONE_0 := 1
const GROUND_ENV_OPTIMUM := 5
const GROUND_SUCCESSION := 6
const GROUND_MARKETS := 7
const GROUND_MAX := 8  # count of ground modes

# Body color modes.
const BODY_SPECIES := 0
const BODY_DIALECT := 1
const BODY_DIET := 2
const BODY_ENERGY := 3
const BODY_MAX := 4

var ground_mode: int = GROUND_BIOME
var body_mode: int = BODY_SPECIES

@onready var sim = get_node("/root/Main/Simulation")

func _ready() -> void:
	ground_mode = GameConfig.default_ground
	body_mode = GameConfig.default_body

func ground_is_biome() -> bool:
	return ground_mode == GROUND_BIOME

func ground_is_optimum() -> bool:
	return ground_mode == GROUND_ENV_OPTIMUM

func ground_is_succession() -> bool:
	return ground_mode == GROUND_SUCCESSION

func ground_is_markets() -> bool:
	return ground_mode == GROUND_MARKETS

# Pheromone channel for the current ground mode, or -1 if not a pheromone mode.
func ground_channel() -> int:
	if ground_mode >= GROUND_PHEROMONE_0 and ground_mode <= GROUND_PHEROMONE_0 + 3:
		return ground_mode - GROUND_PHEROMONE_0
	return -1

func _unhandled_key_input(event: InputEvent) -> void:
	if not (event is InputEventKey) or not event.pressed or event.echo:
		return
	var k := event as InputEventKey
	if k.keycode == KEY_G:
		_cycle_ground()
	elif k.keycode == KEY_C:
		_cycle_body()

func _cycle_ground() -> void:
	ground_mode = (ground_mode + 1) % GROUND_MAX
	# Skip ENV_OPTIMUM when the env mechanism is inactive.
	if ground_mode == GROUND_ENV_OPTIMUM and not bool(sim.env_active()):
		ground_mode = GROUND_BIOME
	# Skip SUCCESSION when disasters are disabled.
	if ground_mode == GROUND_SUCCESSION and not bool(sim.disasters_active()):
		ground_mode = GROUND_BIOME
	# Skip MARKETS when the trade economy is disabled.
	if ground_mode == GROUND_MARKETS and not bool(sim.resources_active()):
		ground_mode = GROUND_BIOME

func _cycle_body() -> void:
	body_mode = (body_mode + 1) % BODY_MAX
