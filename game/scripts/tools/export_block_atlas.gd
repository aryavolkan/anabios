extends SceneTree
# Migration shim (D5, Phase A item 3): export an existing hand-typed
# block-list sprite family into the imported-atlas format documented in
# game/assets/README.md, so the shader/MultiMesh contracts can eventually
# see one kind of atlas regardless of source. Writes
# game/assets/atlases/<family>.png + <family>.atlas.json; run it (and commit
# the result) whenever a source family below changes and its atlas needs
# refreshing — generated atlases are build output, not hand-edited after
# export. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/tools/export_block_atlas.gd -- terrain
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/tools/export_block_atlas.gd -- buildings
# Exits 0 on success, 1 if the family is unknown or the built atlas fails its
# own AtlasRegistry validation.

const AtlasRegistry = preload("res://scripts/atlas_registry.gd")
const TerrainSprites = preload("res://scripts/terrain_sprites.gd")
const BuildingSprites = preload("res://scripts/building_sprites.gd")

const OUT_DIR := "res://assets/atlases"


func _init() -> void:
	var args := OS.get_cmdline_user_args()
	if args.is_empty():
		push_error("usage: export_block_atlas.gd -- <terrain|buildings>")
		quit(1)
		return
	var family: String = args[0]
	var built: Dictionary
	match family:
		"terrain":
			built = _build_terrain()
		"buildings":
			built = _build_buildings()
		_:
			push_error("unknown family '%s' (want 'terrain' or 'buildings')" % family)
			quit(1)
			return
	if built.is_empty():
		quit(1)
		return
	if not _write(family, built["image"] as Image, built["manifest"] as Dictionary):
		quit(1)
		return
	quit(0)


# terrain: every ground-tile variant ("<TerrainName>_<variant>") plus every
# decoration prop ("prop_<PropName>"), at TerrainSprites' own square atlas
# layout — the exported grid is pixel-identical to TerrainSprites.build_atlas().
func _build_terrain() -> Dictionary:
	var cells := {}
	for t in TerrainSprites.TERRAIN_COUNT:
		for v in TerrainSprites.VARIANTS:
			var tile_name := "%s_%d" % [TerrainSprites.NAMES[t], v]
			cells[tile_name] = TerrainSprites.atlas_cell_for_tile(t, v)
	for p in TerrainSprites.PROP_COUNT:
		var prop_name := "prop_%s" % TerrainSprites.PROP_NAMES[p]
		cells[prop_name] = TerrainSprites.atlas_cell_for_prop(p)
	var manifest := {
		"cell_px": TerrainSprites.CELL_PX,
		"cols": TerrainSprites.ATLAS_COLS,
		"cells": cells,
		"families":
		{"source": "terrain_sprites.gd", "exported_by": "scripts/tools/export_block_atlas.gd"},
	}
	return _validated(TerrainSprites.build_atlas().get_image(), manifest, "terrain")


# buildings: one cell per landmark kind, named from BuildingSprites.NAMES.
# BuildingSprites has no shared atlas today (each kind is its own 16x16
# texture via build_image()), so this packs them into a fresh square grid
# sized to fit KIND_COUNT, in enum order. Textures are exported exactly as
# build_image() returns them (already flip_y()-ed for the settlement layer's
# flipped-V-axis QuadMesh), so nothing about current rendering changes.
func _build_buildings() -> Dictionary:
	var cell_px := 16
	var count := BuildingSprites.KIND_COUNT
	var cols := int(ceil(sqrt(float(count))))
	var side := cols * cell_px
	var image := Image.create(side, side, false, Image.FORMAT_RGBA8)
	image.fill(Color(0, 0, 0, 0))
	var cells := {}
	for k in count:
		var cell_img: Image = BuildingSprites.build_image(k)
		var cx := (k % cols) * cell_px
		var cy := int(k / float(cols)) * cell_px
		image.blit_rect(cell_img, Rect2i(0, 0, cell_px, cell_px), Vector2i(cx, cy))
		cells[BuildingSprites.NAMES[k]] = k
	var manifest := {
		"cell_px": cell_px,
		"cols": cols,
		"cells": cells,
		"families":
		{"source": "building_sprites.gd", "exported_by": "scripts/tools/export_block_atlas.gd"},
	}
	return _validated(image, manifest, "buildings")


# Run the built atlas through the same AtlasRegistry validation the runtime
# uses, so a broken export fails loudly here instead of shipping a manifest
# nothing can load. Returns {} (with push_error) on any problem, else
# {"image": Image, "manifest": Dictionary}.
func _validated(image: Image, manifest: Dictionary, family: String) -> Dictionary:
	var checked := AtlasRegistry.validate_manifest(manifest, family)
	if checked.is_empty():
		return {}
	var problems := AtlasRegistry.validate_image(image, checked)
	if not problems.is_empty():
		for problem in problems:
			push_error("%s: %s" % [family, problem])
		return {}
	return {"image": image, "manifest": checked}


func _write(family: String, image: Image, manifest: Dictionary) -> bool:
	var mk_err := DirAccess.make_dir_recursive_absolute(OUT_DIR)
	if mk_err != OK and mk_err != ERR_ALREADY_EXISTS:
		push_error("could not create %s (error %d)" % [OUT_DIR, mk_err])
		return false

	var png_path := "%s/%s.png" % [OUT_DIR, family]
	var png_err := image.save_png(png_path)
	if png_err != OK:
		push_error("could not write %s (error %d)" % [png_path, png_err])
		return false

	var json_path := "%s/%s.atlas.json" % [OUT_DIR, family]
	var f := FileAccess.open(json_path, FileAccess.WRITE)
	if f == null:
		push_error(
			"could not open %s for writing (error %d)" % [json_path, FileAccess.get_open_error()]
		)
		return false
	f.store_string(JSON.stringify(manifest, "\t"))
	f.close()

	print("export_block_atlas: wrote %s and %s" % [png_path, json_path])
	return true
