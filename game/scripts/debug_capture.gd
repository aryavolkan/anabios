extends Node

# Env-gated screenshot harness for automated visual inspection.
# Inert unless ANABIOS_SHOT is set to an output PNG path.
#   ANABIOS_SHOT        -> output path (also the on/off switch)
#   ANABIOS_SHOT_FRAMES -> frames to wait before capture (default 180)

func _ready() -> void:
	if not OS.has_environment("ANABIOS_SHOT"):
		return
	# Fail fast: the capture reads the viewport texture after frame_post_draw,
	# which never completes on the headless dummy renderer — without this guard
	# the run hangs forever instead of producing a shot.
	if DisplayServer.get_name() == "headless":
		push_error("[capture] ANABIOS_SHOT requires a windowed run; --headless cannot read back the viewport")
		get_tree().quit(1)
		return
	# Optional scenario/overlay override (autoloads run before the main scene
	# reads GameConfig), so we can screenshot any scenario headlessly.
	if OS.has_environment("ANABIOS_SCENARIO"):
		GameConfig.scenario_path = OS.get_environment("ANABIOS_SCENARIO")
	# Seed override: scenarios tuned around a specific biome field (e.g.
	# geographic-trade's four-way junction hub) only show that behavior on
	# their own seed, which the viewer's default GameConfig.seed would mask.
	if OS.has_environment("ANABIOS_SEED"):
		GameConfig.seed = int(OS.get_environment("ANABIOS_SEED"))
	if OS.has_environment("ANABIOS_GROUND"):
		GameConfig.default_ground = int(OS.get_environment("ANABIOS_GROUND"))
	if OS.has_environment("ANABIOS_BODY"):
		GameConfig.default_body = int(OS.get_environment("ANABIOS_BODY"))
	var path := OS.get_environment("ANABIOS_SHOT")
	var wait_frames := 180
	if OS.has_environment("ANABIOS_SHOT_FRAMES"):
		wait_frames = int(OS.get_environment("ANABIOS_SHOT_FRAMES"))
	# Freeze the scene tree while the scene builds so the sim does not tick
	# before step_n runs: the capture lands on exactly SHOT_TICKS + SHOT_FRAMES
	# (previously the build wait leaked ~30 ticks, drifting every capture).
	process_mode = Node.PROCESS_MODE_ALWAYS
	get_tree().paused = true
	_run(path, wait_frames)

func _run(path: String, wait_frames: int) -> void:
	# Let the scene build (tree is paused: nodes process no ticks, but node
	# setup and this ALWAYS-mode coroutine still run).
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
		# Optionally reveal the [T] evolution panel for the capture.
		if OS.has_environment("ANABIOS_EVO"):
			var evo := main.get_node_or_null("UI/EvolutionPanel")
			if evo != null:
				evo.set("_shown", true)
				evo.visible = true
		# Optionally pin an agent so the inspector panel is visible.
		if OS.has_environment("ANABIOS_INSPECT"):
			var sim2 := main.get_node_or_null("Simulation")
			var insp := main.get_node_or_null("UI/Inspector")
			if sim2 != null and insp != null:
				var pin_pos := Vector2(512, 512)
				if OS.has_environment("ANABIOS_INSPECT_X"):
					pin_pos = Vector2(
						float(OS.get_environment("ANABIOS_INSPECT_X")),
						float(OS.get_environment("ANABIOS_INSPECT_Y")))
				var id: int = int(sim2.call("agent_near", pin_pos, 400.0))
				if id >= 0:
					insp.call("pin", id)
		# Optionally override the camera: ANABIOS_CAM_X/_CAM_Y take world coords,
		# ANABIOS_CAM_ZOOM takes a zoom factor (1.0 = 1 world unit per pixel).
		if OS.has_environment("ANABIOS_CAM_ZOOM"):
			var cam := main.get_node_or_null("Camera2D")
			if cam != null:
				var z := float(OS.get_environment("ANABIOS_CAM_ZOOM"))
				cam.set("zoom", Vector2(z, z))
				if OS.has_environment("ANABIOS_CAM_X"):
					cam.set("position", Vector2(
						float(OS.get_environment("ANABIOS_CAM_X")),
						float(OS.get_environment("ANABIOS_CAM_Y"))))
		# Optionally exercise the E2 event camera (replay is triggered after
		# the unfreeze below — it needs the snapshot ring warmed first).
		var rm := main.get_node_or_null("ReplayManager")
		if rm != null and OS.has_environment("ANABIOS_EVENT_CAM"):
			rm.call("start_event_cam")
	# Unfreeze only for the capture wait, so the sim advances exactly
	# wait_frames ticks past the step_n jump before the shot.
	get_tree().paused = false
	# Replay needs the ring to have captured since the jump — force one
	# capture at the exact post-jump tick (Main steps before ReplayManager in
	# tree order, so the first organic capture lands one tick late and would
	# miss an event stamped at the jump tick itself), then trigger replay
	# mid-wait.
	var rm2 := main.get_node_or_null("ReplayManager") if main != null else null
	if rm2 != null and OS.has_environment("ANABIOS_REPLAY"):
		rm2.call("_capture_ring")
		for _i in 5:
			await get_tree().process_frame
		rm2.call("start_replay")
	for _i in wait_frames:
		await get_tree().process_frame
	await RenderingServer.frame_post_draw
	var img := get_viewport().get_texture().get_image()
	var err := img.save_png(path)
	print("[capture] saved ", path, " err=", err, " size=", img.get_size())
	get_tree().quit()
