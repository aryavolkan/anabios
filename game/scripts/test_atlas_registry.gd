extends SceneTree
# Headless unit test for the imported-atlas registry (game/assets/README.md).
# Builds synthetic manifests and a synthetic Image in code — no PNG on disk
# is required for the bulk of this test — then, if any real atlas has been
# exported to res://assets/atlases/, cross-checks it too. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_atlas_registry.gd
# Exits 0 on success, 1 on the first failed assertion.

const AtlasRegistry = preload("res://scripts/atlas_registry.gd")

const ATLAS_DIR := "res://assets/atlases"

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _valid_manifest() -> Dictionary:
	return {
		"cell_px": 16,
		"cols": 4,
		"cells": {"Grass_0": 0, "Grass_1": 1, "prop_Oak": 15},
	}


# A cols x cols x cell_px square image, opaque everywhere except that
# `blank_cell_idx` (if >= 0) is left fully transparent.
func _synthetic_image(cols: int, cell_px: int, blank_cell_idx: int = -1) -> Image:
	var side := cols * cell_px
	var img := Image.create(side, side, false, Image.FORMAT_RGBA8)
	img.fill(Color(1, 1, 1, 1))
	if blank_cell_idx >= 0:
		var col := blank_cell_idx % cols
		var row := int(blank_cell_idx / float(cols))
		img.fill_rect(Rect2i(col * cell_px, row * cell_px, cell_px, cell_px), Color(0, 0, 0, 0))
	return img


func _init() -> void:
	# --- validate_manifest: the happy path -------------------------------
	var valid := AtlasRegistry.validate_manifest(_valid_manifest())
	_check(not valid.is_empty(), "a well-formed manifest validates")
	_check(valid.get("cell_px") == 16, "cell_px carries through")
	_check(valid.get("cols") == 4, "cols carries through")
	_check((valid.get("cells") as Dictionary).get("prop_Oak") == 15, "cell indices carry through")
	_check((valid.get("families") as Dictionary).is_empty(), "families defaults to {}")

	# families metadata, when present and a Dictionary, passes through.
	var with_families := _valid_manifest()
	with_families["families"] = {"note": "exported from terrain_sprites.gd"}
	var wf := AtlasRegistry.validate_manifest(with_families)
	_check(
		(wf.get("families") as Dictionary).get("note") == "exported from terrain_sprites.gd",
		"families metadata passes through when present"
	)

	# --- validate_manifest: rejections ------------------------------------
	var bad_px := _valid_manifest()
	bad_px["cell_px"] = 24
	_check(
		AtlasRegistry.validate_manifest(bad_px).is_empty(), "cell_px outside {16,32,48} is rejected"
	)

	var zero_cols := _valid_manifest()
	zero_cols["cols"] = 0
	_check(
		AtlasRegistry.validate_manifest(zero_cols).is_empty(),
		"cols <= 0 is rejected (not a square grid)"
	)

	var frac_cols := _valid_manifest()
	frac_cols["cols"] = 3.5
	_check(
		AtlasRegistry.validate_manifest(frac_cols).is_empty(),
		"non-integer cols is rejected (not a square grid)"
	)

	var oob := _valid_manifest()
	(oob["cells"] as Dictionary)["prop_Oak"] = 16  # cols=4 -> valid range is [0, 16)
	_check(AtlasRegistry.validate_manifest(oob).is_empty(), "a cell index >= cols*cols is rejected")

	var neg_idx := _valid_manifest()
	(neg_idx["cells"] as Dictionary)["prop_Oak"] = -1
	_check(AtlasRegistry.validate_manifest(neg_idx).is_empty(), "a negative cell index is rejected")

	var dup := _valid_manifest()
	(dup["cells"] as Dictionary)["prop_Oak"] = 0  # collides with Grass_0's index
	_check(
		AtlasRegistry.validate_manifest(dup).is_empty(),
		"two cell names sharing one index is rejected"
	)

	var no_cells := _valid_manifest()
	no_cells.erase("cells")
	_check(AtlasRegistry.validate_manifest(no_cells).is_empty(), "a missing cells map is rejected")

	# --- validate_image: wrong size ----------------------------------------
	var manifest := _valid_manifest()
	var right_size := _synthetic_image(4, 16)
	_check(
		AtlasRegistry.validate_image(right_size, manifest).is_empty(),
		"a correctly-sized, fully opaque image validates clean"
	)
	var wrong_size := Image.create(48, 48, false, Image.FORMAT_RGBA8)
	var size_problems := AtlasRegistry.validate_image(wrong_size, manifest)
	_check(
		size_problems.size() == 1 and size_problems[0].findn("48x48") >= 0,
		"a wrong-size image is flagged"
	)

	# --- validate_image: an empty named cell --------------------------------
	var blank_grass1 := _synthetic_image(4, 16, 1)  # Grass_1 is index 1
	var blank_problems := AtlasRegistry.validate_image(blank_grass1, manifest)
	_check(blank_problems.size() == 1, "exactly one cell is flagged empty")
	_check(
		not blank_problems.is_empty() and blank_problems[0].findn("Grass_1") >= 0,
		"the flagged cell is named in the problem"
	)

	# --- cell_rect -----------------------------------------------------------
	var rect := AtlasRegistry.cell_rect(manifest, "prop_Oak")  # index 15, cols=4 -> (3, 3)
	_check(rect == Rect2i(48, 48, 16, 16), "cell_rect maps index to row-major pixel rect")
	var rect0 := AtlasRegistry.cell_rect(manifest, "Grass_0")  # index 0 -> (0, 0)
	_check(rect0 == Rect2i(0, 0, 16, 16), "cell_rect handles index 0")
	var missing_rect := AtlasRegistry.cell_rect(manifest, "NoSuchCell")
	_check(missing_rect == Rect2i(), "cell_rect on an unknown name returns an empty rect")

	# --- load_manifest: missing file --------------------------------------
	var missing_manifest_path := "res://assets/atlases/__does_not_exist__.atlas.json"
	_check(
		AtlasRegistry.load_manifest(missing_manifest_path).is_empty(),
		"load_manifest on a missing file returns {}"
	)

	# --- load_atlas: missing family ----------------------------------------
	_check(
		AtlasRegistry.load_atlas("__does_not_exist__").is_empty(),
		"load_atlas on a missing family returns {}"
	)

	# --- any real atlas on disk loads and validates against its PNG --------
	var checked_any := false
	var dir := DirAccess.open(ATLAS_DIR)
	if dir != null:
		for fname in dir.get_files():
			if not fname.ends_with(".atlas.json"):
				continue
			checked_any = true
			var family := fname.substr(0, fname.length() - ".atlas.json".length())
			var loaded := AtlasRegistry.load_atlas(family)
			_check(not loaded.is_empty(), "atlas '%s' loads and validates against its PNG" % family)
	if not checked_any:
		print("test_atlas_registry: no atlases on disk — skipping the load_atlas cross-check")

	if _failed:
		quit(1)
		return
	print("test_atlas_registry: all passed")
	quit(0)
