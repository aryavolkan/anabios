extends Node

# Env-gated screenshot harness for automated visual inspection.
# Inert unless ANABIOS_SHOT is set to an output PNG path.
#   ANABIOS_SHOT        -> output path (also the on/off switch)
#   ANABIOS_SHOT_FRAMES -> frames to wait before capture (default 180)

func _ready() -> void:
	if not OS.has_environment("ANABIOS_SHOT"):
		return
	# Optional scenario/overlay override (autoloads run before the main scene
	# reads GameConfig), so we can screenshot any scenario headlessly.
	if OS.has_environment("ANABIOS_SCENARIO"):
		GameConfig.scenario_path = OS.get_environment("ANABIOS_SCENARIO")
	if OS.has_environment("ANABIOS_GROUND"):
		GameConfig.default_ground = int(OS.get_environment("ANABIOS_GROUND"))
	if OS.has_environment("ANABIOS_BODY"):
		GameConfig.default_body = int(OS.get_environment("ANABIOS_BODY"))
	var path := OS.get_environment("ANABIOS_SHOT")
	var wait_frames := 180
	if OS.has_environment("ANABIOS_SHOT_FRAMES"):
		wait_frames = int(OS.get_environment("ANABIOS_SHOT_FRAMES"))
	_run(path, wait_frames)

func _run(path: String, wait_frames: int) -> void:
	# Let the scene build.
	for _i in 30:
		await get_tree().process_frame
	# The viewer pauses when unfocused; force it to run and (optionally) jump
	# ahead a fixed number of ticks so we can inspect an evolved state.
	var main := get_tree().root.get_node_or_null("Main")
	if main != null:
		main.set("paused", false)
		if OS.has_environment("ANABIOS_SHOT_TICKS"):
			var ticks := int(OS.get_environment("ANABIOS_SHOT_TICKS"))
			var sim := main.get_node_or_null("Simulation")
			if sim != null and ticks > 0:
				sim.call("step_n", ticks)
		# Optionally reveal the [Y] co-evolution chart for the capture.
		if OS.has_environment("ANABIOS_COEVO"):
			var coevo := main.get_node_or_null("UI/CoevolutionPanel")
			if coevo != null:
				coevo.set("_shown", true)
				coevo.visible = true
	for _i in wait_frames:
		await get_tree().process_frame
	await RenderingServer.frame_post_draw
	var img := get_viewport().get_texture().get_image()
	var err := img.save_png(path)
	print("[capture] saved ", path, " err=", err, " size=", img.get_size())
	get_tree().quit()
