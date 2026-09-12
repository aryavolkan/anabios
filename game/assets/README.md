# Imported atlas pipeline

This is the D5 asset pipeline from
`docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md` (§4 D5,
§6 Phase A, branch `claude/graphics-larger-maps-bra60w`). It sits *alongside*
the existing hand-typed block-list families (`ape_sprites.gd`,
`terrain_sprites.gd`, `building_sprites.gd`) — nothing here replaces those
until a family is migrated on purpose. Its job is to give the viewer a second,
higher-density source of sprite art (imported PNGs) without giving up the
determinism and reviewability of the block lists: every shipped atlas is
either hand-authored in Aseprite and committed as a real asset, or exported
byte-for-byte from a block-list family by a checked-in tool, never edited by
hand after export.

## Format

An atlas family is two files, both under `game/assets/atlases/`:

- `<family>.png` — a **square** grid of `cols × cols` cells, each `cell_px`
  pixels, RGBA8, nearest-filtered when displayed (see Import settings below).
  Square grids only: an extreme-aspect atlas corrupts on the Metal MultiMesh
  path (the same constraint documented in `ape_sprites.gd` and
  `terrain_sprites.gd` — `ATLAS_COLS` stays square there too).
- `<family>.atlas.json` — the manifest:

  ```json
  {
    "cell_px": 16,
    "cols": 8,
    "cells": {
      "Grass_0": 0,
      "Grass_1": 1,
      "prop_Oak": 27
    },
    "families": {
      "note": "optional free-form metadata, e.g. source family / export tool version"
    }
  }
  ```

  - `cell_px` — one of `16`, `32` or `48` (the D1 visual-scale tiers: ground
    tiles at 16 px, canopy/props/huts at 32 px, landmarks at 32–48 px).
  - `cols` — the grid is `cols × cols` cells (square, per the Metal
    constraint above). `cols * cell_px` is the PNG's width and height.
  - `cells` — cell name → flat index (row-major: `index = row * cols + col`,
    matching the `_blit_cell` convention already used by
    `terrain_sprites.gd`/`ape_sprites.gd`). Every index must satisfy
    `0 <= index < cols * cols`, and no two names may share one index (each
    cell has exactly one name).
  - `families` — optional, free-form. Not read by `AtlasRegistry`; a place
    for export provenance or grouping metadata a tool wants to leave behind.

Naming convention used by the block-list exporter (§ below), kept so hand
authored atlases can follow the same scheme: `"<TerrainName>_<variant>"` for
ground tiles (e.g. `"Forest_0"`) and `"prop_<PropName>"` for decoration props
(e.g. `"prop_Oak"`); building/landmark atlases use one cell per kind, named
from that family's own `NAMES` array (e.g. `"Market"`, `"Metalworking"`).

## Sources

Aseprite source files (`.aseprite`) live under `game/assets/src/`, one per
family, and are **not shipped** in the exported atlas or read by the game at
runtime — they are the editable origin the PNG is exported from. Likewise,
the reference concept boards used for Phase 0 design QA (§6 "Phase 0" in the
design doc) are reference material only and never ship as game assets or
land under `game/assets/atlases/`.

## Import settings

Godot 4's importer keeps its settings in a `.import` sidecar next to each
PNG; `.import` files are regenerated on import and are gitignored
(`*.import` in the repo `.gitignore`), so they are never hand-edited or
committed. For every atlas PNG the importer must be set to:

- `compress/mode = 0` (Lossless) — no block compression artefacts at pixel
  scale.
- `mipmaps/generate = false` — mipmaps blur the exact-pixel look the D4
  pixel-perfect pipeline depends on.

Nearest-filtering is **not** an import setting in Godot 4; it is set per node
(`texture_filter = TEXTURE_FILTER_NEAREST` on the `Sprite2D`/`CanvasItem`
consuming the texture), the same way the existing block-list textures are
displayed nearest-filtered today.

## `AtlasRegistry`

`game/scripts/atlas_registry.gd` is the runtime API:

- `AtlasRegistry.load_manifest(path)` — read and JSON-parse a `.atlas.json`
  file, then validate it (see `validate_manifest` below); returns `{}` and
  calls `push_error` on any validation failure.
- `AtlasRegistry.validate_manifest(data, source = "<manifest>")` — the
  validation `load_manifest` runs, factored out so it can be called directly
  on an already-parsed `Dictionary` (what a test builds in code, with no file
  on disk): `cell_px` in {16, 32, 48}; `cols` a positive integer (the grid is
  `cols x cols`, so anything else cannot form a square grid); every cell
  index in `[0, cols * cols)`; and no two cell names sharing one index.
  Returns `{}` and `push_error`s on the first problem found.
- `AtlasRegistry.load_atlas(family)` — load
  `res://assets/atlases/<family>.png` + its manifest and return
  `{ "texture": ImageTexture, "cell_px": int, "cols": int, "cells": Dictionary }`.
- `AtlasRegistry.validate_image(image, manifest)` — check a loaded `Image`
  against a manifest (right square size, no named cell that is fully
  transparent) and return the list of problems found (empty = clean).
- `AtlasRegistry.cell_rect(manifest, name)` — the `Rect2i` of a named cell in
  atlas-pixel space.

## Exporting the block-list families

`game/scripts/tools/export_block_atlas.gd` is a headless SceneTree tool that
writes an existing block-list family into this format, so the shader/MultiMesh
contracts can eventually see one kind of atlas regardless of source. Run it
from the repo root with Godot on `PATH` (this container does not have Godot
installed, so this has not been run here):

```sh
godot --headless --rendering-driver dummy --path game \
  -s res://scripts/tools/export_block_atlas.gd -- terrain
godot --headless --rendering-driver dummy --path game \
  -s res://scripts/tools/export_block_atlas.gd -- buildings
```

Each run writes `game/assets/atlases/<family>.png` and
`game/assets/atlases/<family>.atlas.json`. The tool is what the project owner
or CI runs to (re)generate atlases from the current block lists; **generated
PNGs and manifests are not committed by this change** — they are build
output, reproducible at any time by re-running the tool against the source
`.gd` files.

## Tests

`game/scripts/test_atlas_registry.gd` is the headless test for the registry
itself (manifest validation, image validation, `cell_rect`, and — when any
`res://assets/atlases/*.atlas.json` exists on disk — that every shipped
atlas loads and validates against its PNG). Run it the same way as the other
sprite module tests:

```sh
scripts/godot-smoke.sh scripts test_atlas_registry
```

or directly:

```sh
godot --headless --rendering-driver dummy --path game \
  -s res://scripts/test_atlas_registry.gd
```

It is wired into the "sprite module tests" step of `.github/workflows/ci.yml`
alongside the existing sprite module tests.
