extends Node

# Ground layer modes. PHEROMONE_0..3 are contiguous (1..4).
const GROUND_BIOME := 0
const GROUND_PHEROMONE_0 := 1
const GROUND_ENV_OPTIMUM := 5
const GROUND_SUCCESSION := 6
const GROUND_MARKETS := 7
const GROUND_TERRITORY := 8
const GROUND_MAX := 9  # count of ground modes

# Body color modes.
const BODY_SPECIES := 0
const BODY_DIALECT := 1
const BODY_DIET := 2
const BODY_ENERGY := 3
const BODY_AFFECT := 4
const BODY_MOOD := 5
const BODY_INFECTION := 6
const BODY_MAX := 7

var ground_mode: int = GROUND_BIOME
var body_mode: int = BODY_SPECIES

@onready var sim = get_node("../Simulation")


func _ready() -> void:
	ground_mode = GameConfig.default_ground
	body_mode = GameConfig.default_body
	# Deferred: Main._ready loads the scenario after this node readies, and
	# the affect gate reads the loaded world's flag. Catches a saved/env
	# default_body pointing at an affect-dependent mode in a flag-off world.
	call_deferred("_validate_body_mode")


func ground_is_biome() -> bool:
	return ground_mode == GROUND_BIOME


func ground_is_optimum() -> bool:
	return ground_mode == GROUND_ENV_OPTIMUM


func ground_is_succession() -> bool:
	return ground_mode == GROUND_SUCCESSION


func ground_is_markets() -> bool:
	return ground_mode == GROUND_MARKETS


func ground_is_territory() -> bool:
	return ground_mode == GROUND_TERRITORY


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
	# Step on past every mode whose subsystem is off in this world until one
	# is live. A gated mode used to reset straight to BIOME instead, so in
	# every world without the env mechanism [G] went biome -> phero 0..3 ->
	# biome and never reached the succession, markets or territory overlays
	# those worlds do run. BIOME and the pheromone modes are always live, so
	# the loop terminates.
	ground_mode = (ground_mode + 1) % GROUND_MAX
	while not _ground_mode_live(ground_mode):
		ground_mode = (ground_mode + 1) % GROUND_MAX


# Whether ground mode m has data to show in the loaded world: ENV_OPTIMUM
# needs the env mechanism, SUCCESSION disasters, MARKETS the trade economy,
# TERRITORY the territory layer.
func _ground_mode_live(m: int) -> bool:
	match m:
		GROUND_ENV_OPTIMUM:
			return bool(sim.env_active())
		GROUND_SUCCESSION:
			return bool(sim.disasters_active())
		GROUND_MARKETS:
			return bool(sim.resources_active())
		GROUND_TERRITORY:
			return bool(sim.territory_active())
	return true


func _cycle_body() -> void:
	body_mode = (body_mode + 1) % BODY_MAX
	_validate_body_mode()


# Skip the affect-dependent modes when the affect layer is disabled — arousal
# reads all-calm and mood reads all-content there. Resets to the base mode
# (it also validates a saved default_body, where BODY_SPECIES is the fallback).
func _validate_body_mode() -> void:
	if body_mode == BODY_AFFECT and not bool(sim.affect_active()):
		body_mode = BODY_SPECIES
	if body_mode == BODY_MOOD and not bool(sim.affect_active()):
		body_mode = BODY_SPECIES
	if body_mode == BODY_INFECTION and not bool(sim.disease_active()):
		body_mode = BODY_SPECIES
