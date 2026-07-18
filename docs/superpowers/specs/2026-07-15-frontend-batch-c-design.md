# Frontend Batch (c) — Design Spec

**Date:** 2026-07-15
**Status:** Approved (brainstorming) → ready for implementation plan
**Baseline:** branch `frontend-batch-c` off `refactor-batch-b` (PR #21).

## Motivation

Batch (c) of the audit: the Godot frontend does ~14 full alive-agent iterations
per frame plus an O(RES²) biome image rebuild every frame. This spec covers the
four highest-value per-frame perf wins. Scope is deliberately tight — the frontend
has **no golden hash**, so verification is headless-boot + manual visual, which is
weaker than the core's byte-identity proof.

**Invariant:** the gdext binding stays a strict **read-only view** of `World` — no
`&mut World`, no new `World` fields. `anabios-core` is untouched, so the core
golden test still passes trivially (determinism unaffected).

## Item 1 — `biome_renderer`: throttle + `set_data`

`game/scripts/biome_renderer.gd::_process` rebuilds all RES²(=4096) cells every
frame via a per-pixel `set_pixel` double loop + `ImageTexture.update`. The biome
(plant biomass regrowth) and pheromone (decay/deposit) fields change slowly
relative to 60fps.

- **Throttle:** rebuild only every `REDRAW_EVERY = 6` frames, **and** immediately
  when the ground mode or pheromone channel changes (so `[G]`/overlay toggles feel
  instant — track the last-drawn mode/channel and force a rebuild on change).
- **Faster build:** construct the image from a `PackedByteArray` (RGBA8, one
  4-byte write per cell) via `Image.set_data(RES, RES, false, FORMAT_RGBA8, bytes)`
  instead of per-pixel `set_pixel`, then `ImageTexture` update. The colors still
  come from the existing read-only `sim.biome_colors()` / `sim.pheromone_colors(ch)`
  (PackedColorArray, RES² entries).

Net: an O(4096) per-pixel rebuild every frame → ~1-in-6 frames, built faster.

## Item 2 — `module_glyphs_all`: one pass instead of nine

`game/scripts/main.gd` calls `sim.module_glyphs(t)` for `t = 0..module_type_count()`
(9 calls), each re-walking all alive agents + their modules. Add a read-only
binding export:

```rust
#[func]
fn module_glyphs_all(&self) -> Array<PackedVector2Array>
```

that does **one** `iter_alive()` pass, appending each module's glyph world-position
into the `PackedVector2Array` for its `module_type()` index (returns an array of
`module_type_count()` entries). `main.gd::_refresh_module_layers` calls it once and
assigns `result[t]` to layer `t`. The existing per-type `module_glyphs(t)` stays in
place (harmless; minimal risk). Read-only, determinism-safe.

## Item 3 — `coevolution_panel`: cache series per draw

`game/scripts/coevolution_panel.gd::_draw` (only runs when the panel is shown)
refetches each full series via `sim.coevo_series(key)` inside `_draw_chart` — and
again for the auto-scale max pass — so each series (up to `COEVO_HISTORY_CAP =
200_000` samples) is copied across the FFI multiple times per frame.

- At the top of `_draw`, fetch each needed series **once** into a local dict
  `{key: PackedFloat32Array}` (plus `coevo_history_len()` once), and have
  `_draw_chart` / the auto-scale pass read from that dict instead of re-calling
  `sim.coevo_series`. Optionally gate the fetch on a dirty flag (rebuild the cache
  only when `coevo_history_len()` changed since last draw) — but a per-`_draw`
  single fetch already removes the duplicate re-copies and is the required change.

Removes O(history)×(series + auto-series) re-copying per open-panel frame.

## Item 4 — `population_panel` / `dit_panel`: reuse labels + phase-offset

`game/scripts/population_panel.gd` and `dit_panel.gd` both `queue_free()` all child
`Label`s and recreate them from scratch on their 6-frame refresh, and both refresh
on the *same* frame (`_frame % 6 == 0`).

- **Reuse labels:** keep the `Label` children; on refresh, update existing
  `Label.text` in place, adding new `Label`s only when the species count grows and
  hiding/removing extras when it shrinks. No per-refresh `queue_free` churn.
- **Phase-offset:** offset the two panels' refresh so their (now-cheaper) rebuilds
  don't land on the same frame (e.g. population on `_frame % 6 == 0`, dit on
  `_frame % 6 == 3`).

(Sharing one `species_stats()` call across both panels is NOT in scope — it needs a
shared owner for marginal gain; each keeps its own call.)

## Testing / verification

- **Headless boot** after each item: `godot --headless --path game
  res://scenes/main.tscn --quit-after <frames>` — no parse/script errors against
  the new binding surface, clean exit.
- **Rust binding gate** (Item 2): `rustup run stable` fmt/clippy/doc `-D warnings`;
  the core golden test still passes (anabios-core untouched).
- **Manual visual pass (acceptance gate):** in the GUI — biome + pheromone overlays
  still animate; module glyphs still render on agents; `[Y]` co-evolution panel
  curves still draw and scrub; population/DIT panels still update. Confirmed by the
  user.

## Out of scope (deferred)

- Bundling the 7 `alive_*` exports (`alive_bundle` / `collect_alive` helper).
- A `SimPanel` GDScript base class.
- Removing the old `module_glyphs(t)` export.
- Any core / determinism change.

## Success criteria

Per-frame frontend cost is materially reduced (biome rebuild throttled + faster;
9 glyph passes → 1; co-evolution series fetched once; panel label churn removed)
with the sandbox rendering identically to before, headless boot clean, and the
Rust binding gate green.
