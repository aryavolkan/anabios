# Web showcase — design

**Date:** 2026-07-30
**Status:** implemented (this PR)

## Problem

The showcase for anabios is 42 static PNGs in `gallery/`. That is the wrong medium
for an *emergence* simulation: the whole payoff is watching simple rules bloom into
fire, migration, farming, writing, and war *over time*. A still frame throws that away.
The only way to see the sim move is to install Godot and run the live viewer, so the
showcase can't be shared as a link.

## Goal

A browser-native, animated, shareable showcase that plays a **real recorded run** with
a narrative arc — no Godot, no server, no build to view.

## Approach (chosen)

Two pieces, connected by a compact data file:

1. **`anabios-headless record`** — a new subcommand that runs a scenario deterministically
   and dumps a compact replay stream. It reuses the exact `step` loop of every other
   subcommand and only *reads* the agent SoA columns (position, species, diet) plus drained
   codex events, so recording does not perturb the run. The emitted `state_hash` must match
   `anabios-headless run` for the same scenario/seed/ticks — the determinism receipt.

2. **`showcase/index.html`** — a self-contained canvas player + scrollytelling page that
   loads the replay via a `<script>` tag (so `file://` works) and plays it back.

Rejected alternatives: a WASM build of the core (heaviest — determinism across the WASM
boundary, real toolchain) and Godot video export (no interactivity, not a frontend). The
recorder path is the smallest change that yields a shareable, animated showcase, and a
WASM live mode could later reuse the same player.

## Replay format

`window.ANABIOS_REPLAY = { meta, species, frames, events }`:

- `meta`: scenario, seed, ticks, sample, world_size, **stride** (agent subsampling factor,
  so the player can rescale the shown count to the true population), frame/event counts,
  and `state_hash`.
- `species`: known species-id → archetype display name (splinters omitted).
- `frames[]`: `{ t, id[], x[], y[], sp[], d[] }` — parallel arrays; positions in world
  units rounded to ints, diet quantised to 0..255. `id` is the agent slot, stable across
  frames so the player can interpolate between samples.
- `events[]`: `{ t, type, sid, v, x, y }` — flattened codex events, snake_case type names.
- `biome`: `{ res, grids[] }` — the sim's Whittaker biome field downsampled to `res`²
  RGB cells, base64-encoded, sampled at `biome_frames` keyframes across the run (the map
  lushes/pollutes/scars over time). Colour mapping is ported cell-for-cell from the Godot
  bridge `biome_colors`, so the web plate matches the live viewer's ground.

Per-species genome centroids were tried as the agent palette but collapse to near-neutral
teal on this scenario (many are padding neutrals), so the player colours agents by a
stable golden-angle hue per species id (the viewer's "species" colour mode), warmed by diet.

Size control (committed file ≈ 1.7 MB, comparable to a few gallery PNGs):
- **Stable-stride agent subsampling** (`--max-agents`): keep every stride-th slot. A fixed
  stride keeps identity stable (a slot is always in or always out), which interpolation needs.
- **Event thinning** (`--max-events`): always keep each event type's *first* occurrence
  (the "first emergence" milestones), then uniformly sample the rest. Chronology preserved.
- **Frame sampling** (`--sample`): one frame every N ticks; the player interpolates.

## Player

- **Signature: strata rail** — a geological core-sample down the left edge, one band per
  era, with a depth marker tracking the tick. Deep time, rendered as sediment.
- **Scroll = descent through eras.** The six era boundaries are derived from the run's real
  event stream (first invention → Tools & Fire, first speciation → Exodus, first
  settlement/market → Farming & Trade, first meme-sweep → Writing, first raid → War & Kin),
  falling back to even spacing when a signal never fires. The current era is driven by
  scroll; its window auto-plays and loops so the world is always alive. Space pauses.
- **Codex ticker** streams the real events in each window, colour-coded by narrative kind
  (fire / trade / grow / war / mind) derived from the event type name. On-canvas accents
  (fire glows, war rings) are tied to actual event locations.
- **Hero** is a clean title moment: the world dims behind the wordmark, and the instrument
  HUD fades in only once you descend.
- Palette derived from the sim's own channels (ochre = fire/tools, glacial = trade/dialect,
  chlorophyll = farming, ember = war) on an excavation-at-night basalt ground.
- Quality floor: responsive to mobile, `prefers-reduced-motion` honoured, `?era=N` deep-link.

## Blast radius

Additive only. New crate module `record.rs` + one clap subcommand in `main.rs`; no changes
to `anabios-core`, so determinism goldens are unaffected. New `showcase/` directory.
