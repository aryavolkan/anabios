extends PanelContainer

# Phase 0 instrumentation for the "pixel world at scale" plan (see
# docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md, §6 Phase 0):
# an on-screen frame-time / scale readout, toggled with [F3] and hidden by
# default so it stays out of the way of normal play and captures.
#
# Doubles as the headless-ish bench harness scripts/viewer-bench.sh drives:
# when ANABIOS_BENCH_FRAMES=<n> is set (and ANABIOS_SHOT is not — the two
# harnesses never run together), the readout forces itself visible, samples
# every frame without touching pause state, and after n frames writes a CSV
# to ANABIOS_BENCH_OUT and quits the tree.
#
# note_chunk_upload() and set_resident_chunks() are Phase 2 hooks: the
# streaming ground layer will call them once it exists. Both stay at 0 until
# then. set_visible_agents() is the equivalent Phase 3 hook; until Phase 3
# culls off-screen agents, "visible agents" just mirrors the alive count.

const UiTheme = preload("res://scripts/ui_theme.gd")

@onready var sim = get_node("../../Simulation")

var _label: Label
var _last_delta: float = 1.0 / 60.0

# Phase 2/3 hooks — see the header comment above.
var _visible_agents: int = -1
var _chunk_uploads: int = 0
var _resident_chunks: int = 0

# Bench-mode state (ANABIOS_BENCH_FRAMES). _bench_frames stays 0 outside it.
var _bench_frames: int = 0
var _bench_out: String = "runs/viewer-bench.csv"
var _bench_samples: Array[Dictionary] = []


func _ready() -> void:
	name = "PerfReadout"
	visible = false
	theme = UiTheme.build()
	set_anchors_preset(Control.PRESET_TOP_LEFT)
	# Right of the minimap (x 10-210, y 70-270), clear of every other panel at
	# rest — see main.tscn for the rest of the HUD layout.
	position = Vector2(220.0, 70.0)
	custom_minimum_size = Vector2(190.0, 0.0)
	_label = Label.new()
	_label.add_theme_font_size_override("font_size", 12)
	add_child(_label)
	if OS.has_environment("ANABIOS_BENCH_FRAMES") and not OS.has_environment("ANABIOS_SHOT"):
		_bench_frames = maxi(1, int(OS.get_environment("ANABIOS_BENCH_FRAMES")))
		if OS.has_environment("ANABIOS_BENCH_OUT"):
			_bench_out = OS.get_environment("ANABIOS_BENCH_OUT")
		visible = true


func _unhandled_key_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_F3:
		visible = not visible


func _process(delta: float) -> void:
	_last_delta = delta
	if not visible:
		return
	var s: Dictionary = sample()
	_label.text = _format_readout(s)
	if _bench_frames > 0:
		_bench_samples.append(s)
		if _bench_samples.size() >= _bench_frames:
			_finish_bench()


# Phase 3 hook: the visible-set body pass will report its own count here once
# it exists. Until then sample() falls back to the alive count.
func set_visible_agents(n: int) -> void:
	_visible_agents = n


# Phase 2 hook: the streaming ground layer bumps this once per chunk texture
# it (re)uploads this frame.
func note_chunk_upload() -> void:
	_chunk_uploads += 1


# Phase 2 hook: the streaming ground layer reports its resident chunk count.
func set_resident_chunks(n: int) -> void:
	_resident_chunks = n


# One frame's worth of readout data. Called every frame while visible (by
# _process) and once per csv_row() call; side-effect-free.
func sample() -> Dictionary:
	var alive: int = int(sim.alive_count()) if sim != null else 0
	return {
		"tick": int(sim.tick()) if sim != null else 0,
		"fps": Engine.get_frames_per_second(),
		"frame_ms": _last_delta * 1000.0,
		"process_ms": Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0,
		"alive": alive,
		"visible_agents": _visible_agents if _visible_agents >= 0 else alive,
		"draw_calls": int(Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME)),
		"primitives": int(Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME)),
		"video_mem_mb": Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0,
		"chunk_uploads": _chunk_uploads,
		"resident_chunks": _resident_chunks,
	}


static func csv_header() -> String:
	return (
		"tick,fps,frame_ms,process_ms,alive,visible_agents,draw_calls,primitives,"
		+ "video_mem_mb,chunk_uploads,resident_chunks"
	)


func csv_row() -> String:
	return _format_row(sample())


func _format_readout(s: Dictionary) -> String:
	return (
		(
			"PERF [F3]\nfps %.1f · frame %.2fms · proc %.2fms\nalive %d · visible %d\n"
			+ "draw %d · prims %d · vram %.1fMB\nchunk uploads %d · resident %d"
		)
		% [
			s["fps"],
			s["frame_ms"],
			s["process_ms"],
			s["alive"],
			s["visible_agents"],
			s["draw_calls"],
			s["primitives"],
			s["video_mem_mb"],
			s["chunk_uploads"],
			s["resident_chunks"],
		]
	)


static func _format_row(s: Dictionary) -> String:
	return (
		"%d,%.2f,%.3f,%.3f,%d,%d,%d,%d,%.3f,%d,%d"
		% [
			int(s["tick"]),
			float(s["fps"]),
			float(s["frame_ms"]),
			float(s["process_ms"]),
			int(s["alive"]),
			int(s["visible_agents"]),
			int(s["draw_calls"]),
			int(s["primitives"]),
			float(s["video_mem_mb"]),
			int(s["chunk_uploads"]),
			int(s["resident_chunks"]),
		]
	)


# Mean of every numeric column across the collected bench samples (the tick
# column is dropped — a mean tick number quotes nothing useful).
static func _mean_sample(samples: Array[Dictionary]) -> Dictionary:
	var keys: PackedStringArray = [
		"fps",
		"frame_ms",
		"process_ms",
		"alive",
		"visible_agents",
		"draw_calls",
		"primitives",
		"video_mem_mb",
		"chunk_uploads",
		"resident_chunks",
	]
	var sums: Dictionary = {}
	for k in keys:
		sums[k] = 0.0
	for s in samples:
		for k in keys:
			sums[k] += float(s[k])
	var n: float = maxf(float(samples.size()), 1.0)
	for k in keys:
		sums[k] /= n
	return sums


static func _format_mean_csv_row(mean: Dictionary) -> String:
	return (
		"mean,%.2f,%.3f,%.3f,%.1f,%.1f,%.1f,%.1f,%.3f,%.1f,%.1f"
		% [
			mean["fps"],
			mean["frame_ms"],
			mean["process_ms"],
			mean["alive"],
			mean["visible_agents"],
			mean["draw_calls"],
			mean["primitives"],
			mean["video_mem_mb"],
			mean["chunk_uploads"],
			mean["resident_chunks"],
		]
	)


static func _format_mean_stdout(mean: Dictionary) -> String:
	return (
		(
			"fps=%.2f frame_ms=%.3f process_ms=%.3f alive=%.1f visible_agents=%.1f "
			+ "draw_calls=%.1f primitives=%.1f video_mem_mb=%.3f chunk_uploads=%.1f "
			+ "resident_chunks=%.1f"
		)
		% [
			mean["fps"],
			mean["frame_ms"],
			mean["process_ms"],
			mean["alive"],
			mean["visible_agents"],
			mean["draw_calls"],
			mean["primitives"],
			mean["video_mem_mb"],
			mean["chunk_uploads"],
			mean["resident_chunks"],
		]
	)


# Write the CSV (header + one row per sampled frame + a final "mean" row) to
# _bench_out, print the mean row to stdout, and quit — this is the number
# scripts/viewer-bench.sh surfaces and every later phase quotes against.
func _finish_bench() -> void:
	var mean: Dictionary = _mean_sample(_bench_samples)
	var dir_path: String = _bench_out.get_base_dir()
	if dir_path != "":
		DirAccess.make_dir_recursive_absolute(dir_path)
	var f := FileAccess.open(_bench_out, FileAccess.WRITE)
	if f == null:
		push_error(
			(
				"[viewer-bench] could not open %s for writing (err=%d)"
				% [_bench_out, FileAccess.get_open_error()]
			)
		)
	else:
		f.store_line(csv_header())
		for s in _bench_samples:
			f.store_line(_format_row(s))
		f.store_line(_format_mean_csv_row(mean))
		f.close()
	print("viewer-bench: " + _format_mean_stdout(mean))
	get_tree().quit()
