# Gene↔Culture Co-evolution Time-Series — Design Spec

**Date:** 2026-07-13
**Status:** Approved (brainstorming) → ready for implementation plan
**Branch target:** new feature branch off `main` (or continue on a polish branch)

## Motivation

The anabios frontend today is entirely **top-down spatial rendering + text panels**.
There is no time-series, no charting, no lineage view — **no way to watch anything
evolve over time**. Yet the simulation's richest story is *gene–culture
co-evolution*: the Communicator module (heritable) gates culture; the skill meme
(ch5) grants up to a 3.5× feeding bonus; so communication-enabling genes and the
memes they carry should rise together. You cannot currently see this happen.

This spec delivers the **measurement instrument**: a live time-series panel that
plots gene-side and meme-side signals against a shared time axis, with codex event
markers pinning cause to effect. It is deliberately the *first* of several planned
cycles — biome enrichment, domestication, and writing/meme-persistence are
**out of scope here** and each get their own later spec→plan cycle. This panel
becomes the lens used to validate those later mechanics.

## Scope

**In scope**
- A read-only Rust aggregate export `coevo_metrics()` computing per-tick scalars.
- A view-only per-tick **history buffer** owned by the `Simulation` gdext node
  (outside `World`), sampled inside `step_n`.
- A new GDScript `CoevolutionPanel` (`Control` + custom `_draw`) rendering a
  vertical stack of small-multiple mini-charts sharing one time axis.
- Codex event markers (Speciation / MemeSweep / DialectFormed / Extinction) as
  vertical lines across all panels.
- Interaction: full-run history with adaptive downsampling, a scrub cursor with
  value readout, per-series legend toggles, hotkey toggle (`T`).
- Rust unit tests for the metric scalars; headless boot check; manual visual pass.

**Out of scope (deferred, each its own later cycle)**
- Biome enrichment (regions/patches/larger world).
- Domestication mechanic.
- Writing / meme-persistence mechanic.
- The other three co-evolution view types: 2D gene×meme phase-space, phylogeny/
  lineage tree, spatial co-evo overlay.

## The load-bearing invariant (unchanged)

All `World` reads stay `&self`. **No new `World` fields. No `&mut World`.** The
history buffer lives on the `Simulation` node struct — which is *not* `World` — so
golden state hashes stay byte-identical
(`0x58807132956798b1` / `0xa020c143eccfb4eb` / `0xfd21efef4e1619e4`).
**No golden refresh.**

## Architecture

Three layers, one source of truth.

```
World (unchanged, &self only)
   │  reads
   ▼
coevo_metrics(&self) -> Dictionary      [Rust, pure aggregate, determinism-safe]
   │  sampled every tick inside step_n
   ▼
Simulation.history: Vec<CoevoSample>    [Rust, view-only, OUTSIDE World]
   │  exposed read-only to GDScript
   ▼
CoevolutionPanel (_draw polylines)      [GDScript, chart rendering]
```

### Layer 1 — Rust aggregate metrics (single source of truth)

Add to `crates/anabios-godot/src/lib.rs`:

```rust
#[func]
fn coevo_metrics(&self) -> Dictionary { /* pure &self aggregate of World */ }
```

Returns one flat dictionary of scalars for the current tick. The aggregation
**reuses the exact math the codex detectors use** (notably the west/east L2 meme
divergence from the `DialectFormed` detector in `codex.rs`) so the chart can never
disagree with the event log. Keys:

| Key | Meaning | Range |
|-----|---------|-------|
| `tick` | current tick | u64 |
| `communicator_frac` | fraction of live agents with a Communicator module | [0,1] |
| `mean_social_learning` | mean genome slot 29 (SocialLearning) over live agents | [0,1] |
| `mean_individual_learning` | mean genome slot 28 (IndividualLearning) over live agents | [0,1] |
| `genetic_diversity` | mean per-slot genome variance over live agents (summed variance across the 50 slots ÷ 50) — cheap, O(agents), deterministic | ≥0 |
| `mean_skill` | mean meme ch5 (SKILL_CHANNEL) over *communicators* | [0,1] |
| `mean_tech_match` | mean `technique_match(tech, env_optimum)` over communicators (0 when env off) | [0,1] |
| `meme_divergence` | west/east L2 meme distance (same computation as DialectFormed) | ≥0 |
| `live_count` | live agent count | u32 |
| `species_count` | number of live species | u32 |
| `env_optimum` | current env optimum, or -1 when inactive | [0,1] or -1 |

Aggregations guard against division by zero (empty population → 0.0, as
`species_stats()` already does). `mean_skill`/`mean_tech_match` average over
communicators only (non-communicators keep memes at 0 and would dilute the signal);
if there are zero communicators the value is 0.0.

### Layer 2 — Per-tick history buffer (view-only, outside World)

Extend the `Simulation` struct (which owns `inner: Option<World>`) with a
view-only history:

```rust
struct CoevoSample { /* the scalars above, as a plain struct */ }

pub struct Simulation {
    base: Base<Node>,
    inner: Option<World>,
    history: Vec<CoevoSample>,   // NEW — not part of World, determinism-safe
}
```

Sampling happens inside the existing `step_n` loop, once per tick, so resolution is
**per-tick regardless of frame rate** (smooth curves even at 64× speed):

```rust
fn step_n(&mut self, n: i64) {
    if let Some(w) = self.inner.as_mut() {
        for _ in 0..n.max(0) {
            anabios_core::tick::step(w);
            // sample AFTER stepping; push a CoevoSample computed from &*w
        }
    }
}
```

`history` is cleared whenever a world is (re)created (`new_world` /
`load_scenario*`). Because a sample is a handful of f32s, a full 2500-tick run is
~tens of KB — no cap needed, but the buffer is bounded by run length (a soft cap,
e.g. 200k samples, guards pathological infinite runs and simply stops appending
past it; this is logged, not silently truncated).

Read-only exports for the panel:

```rust
#[func] fn coevo_history_len(&self) -> i64
#[func] fn coevo_series(&self, key: GString) -> PackedFloat32Array  // one series, full history
#[func] fn coevo_sample_at(&self, index: i64) -> Dictionary          // scrub readout
```

Series are returned as parallel arrays keyed by name so GDScript pulls exactly the
lines it draws. The tick axis comes from `coevo_series("tick")`.

Codex event markers reuse the **existing** `take_codex_events()` drain that
`codex_panel.gd` already consumes. To avoid two consumers racing over the drained
buffer, the `CoevolutionPanel` reads a shared cached event list maintained by
`codex_panel.gd` (which already keeps recent events with their tick + type), rather
than draining independently. (Plan will confirm the cleanest sharing seam.)

### Layer 3 — GDScript chart panel

New `game/scripts/coevolution_panel.gd` (`Control`, custom `_draw()`), added to the
`UI` CanvasLayer in `main.tscn`, following the existing `population_panel` /
`dit_panel` pattern (read-only, toggleable, same styling).

**Layout — vertical stack of small-multiples sharing one x (time) axis:**

1. **Flagship (0–1 axis, overlaid):** `communicator_frac` vs `mean_skill`
   (bold), with `mean_social_learning` / `mean_individual_learning` as fainter
   lines. This is the "watch nature and culture rise together" panel.
2. **Cultural divergence (0–1):** `meme_divergence` + `mean_tech_match`.
3. **Population context (auto-scaled):** `live_count` + `species_count`.
4. **Genetic diversity (auto-scaled):** `genetic_diversity`.

**Event markers:** thin, color-coded vertical lines across *all* panels at the
ticks of Speciation / MemeSweep / DialectFormed / Extinction events.

**Interaction:**
- Full-run history with **adaptive downsampling** to panel pixel width (min/max
  decimation per column so spikes survive), so the whole run stays on screen with
  bounded draw cost.
- A movable **scrub cursor**: vertical line + a readout box showing exact values of
  every visible series at that tick (via `coevo_sample_at`).
- Per-series **legend toggles** (click to show/hide a line).
- Hotkey **`T`** toggles the whole panel (collapsed by default so it doesn't crowd
  the existing HUD); respects existing pause/speed controls.

Rendering uses `draw_polyline` / `draw_line` / `draw_string` directly — Godot has
no chart library and none may be added (self-contained constraint).

## Data flow (per frame)

```
_process(dt):
  if not paused: sim.step_n(ticks_per_frame)   # Rust samples history per tick
  if panel visible:
    for each active series: sim.coevo_series(name) -> polyline
    read cached codex events -> vertical markers
    draw scrub cursor + legend
```

The panel pulls the *full* series each draw only when visible and dirty (history
length changed), caching the downsampled polylines between draws to keep frame cost
flat.

## Testing

- **Rust unit tests** (`crates/anabios-godot` or a small core helper): on a
  hand-built tiny world, assert `communicator_frac`, `mean_skill`, and
  `meme_divergence` equal hand-computed values; assert all frequencies ∈ [0,1];
  assert empty-population and zero-communicator cases return 0.0 without panicking.
- **Determinism check:** run the existing golden-hash test(s) — hashes must be
  byte-identical (no `World` change). Explicitly confirm no golden refresh.
- **Headless boot:** `godot --headless --path game res://scenes/main.tscn
  --quit-after <frames>` boots clean with the new panel present.
- **Manual visual pass:** `dialects`, `cooperation`, `gene-culture-skill`, and one
  DIT scenario (`dit-env-slow`) — confirm the flagship curves move, event markers
  line up with the curve that moved, scrub readout is correct, toggles work.
- **CI gate** (per project norm): `rustup run stable` fmt/clippy/doc with
  `-D warnings`; commit fmt output; escape `[0,1]`/`[N]` in doc comments.

## Risks & mitigations

| Risk | Mitigation |
|------|-----------|
| Two consumers drain `take_codex_events()` → events lost from one panel | Panel reads codex_panel.gd's cached event list; single drain site. |
| History buffer grows unbounded on very long runs | Soft cap (~200k samples) with a logged notice; not silently truncated. |
| Different-unit series overlaid become unreadable | Small-multiples stack; only same-unit series share a panel. |
| Aggregate math drifts from detector math | `coevo_metrics` reuses the detectors' own divergence computation. |
| Per-tick sampling slows fast-forward | Sample is a few f32s; O(agents) once/tick, same order as one tick's other passes. Measure; if hot, sample every k ticks (still per-tick-labeled). |

## Success criteria

Running any gene-culture scenario in the Godot GUI, pressing `T` shows the panel;
over a run you can watch `communicator_frac` and `mean_skill` rise together, see a
`MemeSweep` marker land on the tick a curve jumps, scrub to read exact values, and
toggle series — all with golden hashes unchanged and CI green.
