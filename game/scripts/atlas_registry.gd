extends RefCounted
# Imported-atlas registry for the D5 asset pipeline (see game/assets/README.md
# and docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md, §4 D5).
#
# An atlas family is `res://assets/atlases/<family>.png` (a SQUARE nearest-
# filtered grid of `cols x cols` cells of `cell_px` pixels — the same Metal
# MultiMesh constraint documented in ape_sprites.gd/terrain_sprites.gd) plus a
# sidecar `<family>.atlas.json` manifest: `{cell_px, cols, cells: {name:
# index}, families?}`. This module loads and validates that pair; it does not
# build atlases (see scripts/tools/export_block_atlas.gd for the block-list
# exporter) and does not render anything.

const VALID_CELL_PX: PackedInt32Array = [16, 32, 48]


# Parse and validate a `.atlas.json` file at `path` (a `res://` or `user://`
# path). Returns the validated manifest `Dictionary` — `{cell_px, cols, cells,
# families}` — or `{}` (with a `push_error`) if the file is missing, is not
# valid JSON, or fails validation. See `validate_manifest` for the checks.
static func load_manifest(path: String) -> Dictionary:
	var text := FileAccess.get_file_as_string(path)
	if text.is_empty():
		push_error("atlas manifest not found or empty: " + path)
		return {}
	var parsed: Variant = JSON.parse_string(text)
	if not (parsed is Dictionary):
		push_error("atlas manifest is not a JSON object: " + path)
		return {}
	return validate_manifest(parsed as Dictionary, path)


# Validate an already-parsed manifest `Dictionary` (the shape a `.atlas.json`
# file parses into). Split out from `load_manifest` so callers — including
# tests — can validate a synthetic manifest without touching the filesystem.
# Checks: `cell_px` in {16, 32, 48}; `cols` is a positive integer (the grid is
# `cols x cols`, so anything else cannot form a square grid); `cells` maps
# String names to integer indices, each `0 <= index < cols * cols`; and no two
# names alias the same index (each atlas cell has exactly one name). Returns
# `{}` and `push_error`s on the first problem found.
static func validate_manifest(data: Dictionary, source: String = "<manifest>") -> Dictionary:
	var cell_px_raw: Variant = data.get("cell_px")
	if not _is_int_like(cell_px_raw) or not VALID_CELL_PX.has(int(cell_px_raw)):
		push_error("%s: cell_px must be one of 16, 32, 48 (got %s)" % [source, cell_px_raw])
		return {}
	var cell_px := int(cell_px_raw)

	var cols_raw: Variant = data.get("cols")
	if not _is_int_like(cols_raw) or int(cols_raw) <= 0:
		push_error(
			(
				"%s: cols must be a positive integer forming a square cols x cols grid (got %s)"
				% [source, cols_raw]
			)
		)
		return {}
	var cols := int(cols_raw)

	var cells_raw: Variant = data.get("cells")
	if not (cells_raw is Dictionary):
		push_error("%s: cells must be an object mapping name -> index" % source)
		return {}

	var cell_count := cols * cols
	var seen_indices := {}
	var cells := {}
	for name in (cells_raw as Dictionary).keys():
		if not (name is String) or (name as String).is_empty():
			push_error("%s: cell name must be a non-empty string (got %s)" % [source, name])
			return {}
		var idx_raw: Variant = (cells_raw as Dictionary)[name]
		if not _is_int_like(idx_raw):
			push_error("%s: cell '%s' index must be an integer (got %s)" % [source, name, idx_raw])
			return {}
		var idx := int(idx_raw)
		if idx < 0 or idx >= cell_count:
			push_error(
				"%s: cell '%s' index %d is out of range [0, %d)" % [source, name, idx, cell_count]
			)
			return {}
		if seen_indices.has(idx):
			push_error(
				(
					"%s: cell index %d is claimed by both '%s' and '%s' (names must be unique per cell)"
					% [source, idx, seen_indices[idx], name]
				)
			)
			return {}
		seen_indices[idx] = name
		cells[name] = idx

	var families: Dictionary = {}
	var families_raw: Variant = data.get("families")
	if families_raw is Dictionary:
		families = families_raw as Dictionary

	return {"cell_px": cell_px, "cols": cols, "cells": cells, "families": families}


static func _is_int_like(value: Variant) -> bool:
	return (value is int or value is float) and int(value) == value


# Load `res://assets/atlases/<family>.png` + its manifest and validate the
# image against it. Returns `{texture: ImageTexture, cell_px: int, cols: int,
# cells: Dictionary}`, or `{}` (with `push_error`s) if the manifest, the PNG,
# or the image-vs-manifest validation fails.
static func load_atlas(family: String) -> Dictionary:
	var manifest_path := "res://assets/atlases/%s.atlas.json" % family
	var manifest := load_manifest(manifest_path)
	if manifest.is_empty():
		return {}

	var png_path := "res://assets/atlases/%s.png" % family
	var image := Image.load_from_file(png_path)
	if image == null:
		push_error("atlas '%s': could not load image %s" % [family, png_path])
		return {}

	var problems := validate_image(image, manifest)
	if not problems.is_empty():
		for problem in problems:
			push_error("atlas '%s': %s" % [family, problem])
		return {}

	return {
		"texture": ImageTexture.create_from_image(image),
		"cell_px": manifest["cell_px"],
		"cols": manifest["cols"],
		"cells": manifest["cells"],
	}


# Check `image` against a validated `manifest`: the image must be the exact
# square `cols * cell_px` side the manifest declares, and every named cell
# must contain at least one non-fully-transparent pixel. Returns the list of
# problems found (empty = clean); never pushes an error itself, so callers can
# decide how to report — `load_atlas` turns each into a `push_error`, tests
# assert on the list directly.
static func validate_image(image: Image, manifest: Dictionary) -> PackedStringArray:
	var problems := PackedStringArray()
	if image == null:
		problems.append("image is null")
		return problems

	var cols: int = manifest.get("cols", 0)
	var cell_px: int = manifest.get("cell_px", 0)
	var side := cols * cell_px
	if image.get_width() != side or image.get_height() != side:
		problems.append(
			(
				"image is %dx%d, want a square %dx%d grid (cols=%d * cell_px=%d)"
				% [image.get_width(), image.get_height(), side, side, cols, cell_px]
			)
		)
		return problems

	var cells: Dictionary = manifest.get("cells", {})
	var names: Array = cells.keys()
	names.sort()
	for name in names:
		var rect := cell_rect(manifest, str(name))
		if not _has_opaque_pixel(image, rect):
			problems.append("cell '%s' has zero opaque pixels" % name)
	return problems


static func _has_opaque_pixel(image: Image, rect: Rect2i) -> bool:
	for y in range(rect.position.y, rect.position.y + rect.size.y):
		for x in range(rect.position.x, rect.position.x + rect.size.x):
			if image.get_pixel(x, y).a > 0.0:
				return true
	return false


# The `Rect2i` of named cell `name` in atlas-pixel space, row-major:
# `index = row * cols + col` (the same layout `_blit_cell` already uses in
# ape_sprites.gd/terrain_sprites.gd). Returns an empty `Rect2i` and
# `push_error`s if `name` is not in the manifest.
static func cell_rect(manifest: Dictionary, name: String) -> Rect2i:
	var cells: Dictionary = manifest.get("cells", {})
	if not cells.has(name):
		push_error("atlas manifest has no cell named '%s'" % name)
		return Rect2i()
	var cols: int = manifest.get("cols", 0)
	var cell_px: int = manifest.get("cell_px", 0)
	var idx: int = cells[name]
	var col := idx % cols
	var row := int(idx / float(cols))
	return Rect2i(col * cell_px, row * cell_px, cell_px, cell_px)
