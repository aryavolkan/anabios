# Sense-stage performance + clarity refactor

**Date:** 2026-08-02
**Scope:** `crates/anabios-core/src/sense.rs`
**Constraint:** byte-identical simulation output. The golden `state_hash` constants
in `tests/{determinism,inventions,cognition}.rs` must NOT change. Ship only
benchmark-verified, above-noise wins.

## Motivation

Stage-level baseline (`cargo bench --bench tick_bench`, 10k agents, this machine):

| Stage | Time | Share of a codex-on tick (~3.6 ms) |
|---|---|---|
| **sense** | 1.17 ms | ~32% |
| codex (observe_all) | 0.97 ms | ~27% |
| spatial_rebuild | 56 µs | ~1.5% |
| interact (worst case) | 0.20 ms | ~5% |

`sense` is the #1 hot path. codex, the #2, already has a config lever
(`codex_interval`) so it is out of scope here. The bench population carries the
`starter_kit` (which includes a `Sensor`), so every agent runs the full sense
body each tick: a ≤49-cell biome scan plus a 9-cell neighbor query.

## Changes

### P1 — `best_plant_direction`: squared-distance reject
The scan computes `offset.length()` (a `sqrt`) on every biomass-positive cell
solely to evaluate `if dist > radius { continue }`. The distance is never
stored — the returned value is `best_offset.normalize_or_zero()`, selected by
max biomass. Replace the reject with `offset.length_squared() > radius * radius`.

- **Identity:** both sides are non-negative and the map `x -> x*x` is monotonic
  on `[0, ∞)`, so the reject set is identical. The returned direction is
  unchanged. No stored float moves.
- **Win:** removes one `sqrt` per scanned biomass-positive cell (up to ~49/agent).

### P2 — neighbor scan: defer direction + relative-metric math
The scan calls `torus_direction` (a `normalize_or_zero` → `sqrt`) for **every**
neighbor in the 9-cell ring, and computes `rel_size`/`rel_energy` on every
improvement of `nearest`. Only the final nearest / nearest-same / nearest-other
directions (≤3) and the final nearest's rel-metrics are ever stored.

Defer: during the scan keep only distances + winner ids (cheap compares). After
the loop, compute the ≤3 directions and the nearest's `rel_size`/`rel_energy`
once, from the stored winner ids.

- **Identity:** winner selection is unchanged (same distance compares, same tie
  order — first-seen-wins via strict `<`). Directions and rel-metrics are
  recomputed from identical inputs, producing identical bits.
- **Win:** removes up to `(crowding − 3)` `normalize` sqrts and all but one
  rel-metric division per agent. Scales with local crowding.

### R1 — clarity refactor (serves P2)
P2 is clean only if the ~12 mutable bookkeeping locals become a small
`NearestNeighbors` tracker that owns the "closest overall / same-species /
other-species" logic. `sense_one` then reads as: build the tracker from the
query, then read its winners. Scope stays inside `sense.rs`; no unrelated
refactoring.

## Verification (per change)

1. `cargo test -p anabios-core` — full suite green, **zero golden-hash edits**
   (this is the byte-identity proof).
2. `cargo bench --bench tick_bench` before/after on `sense/{1000,10000}` and
   `tick/{1000,10000}`. If a change lands within noise, drop it.
3. CI gates: `cargo fmt --check`, `cargo clippy -- -D warnings`, rustdoc
   `-D warnings`.

## Non-goals
- codex / observe_all (config lever already exists).
- Any change that moves a golden hash.

## Addendum — extended scope (second pass)

After the initial two-change PR, the work was extended with more measured,
byte-identical wins on the same hot path plus an audit of the other per-tick
stages. Identity basis is stated per change: **provable** = pure
reordering/deferral/hoisting of identical values; **golden-verified** =
equivalent in exact arithmetic, confirmed identical by the determinism +
inventions + cognition golden hashes and the full `all_scenarios` suite (the
repo's behavioral-identity contract), with a noted floating-point boundary
caveat.

### P3 — defer the per-neighbor distance `sqrt` (`sense.rs`, `spatial.rs`)
Add `torus_distance_sq` (and refactor `torus_distance = torus_distance_sq().sqrt()`,
bit-identical). The neighbor scan compares **squared** distances and takes one
`sqrt` per winner (≤3) instead of one per ring neighbor. Winner compares
(`d_sq` vs `d_sq`) are **provable**; the radius reject (`d_sq > radius²`) is
**golden-verified** (same idiom as P1). Bench: 10k −20.6% (p=0.00) on top of
P1+P2.

### P4 — hoist `row`/center-y out of `best_plant_direction`'s inner loop
Loop-invariant across the inner `dx` loop. **Provable** (LLVM likely already
LICMs it; primarily clarity).

### Cross-stage audit wins (`integrate.rs`, `age.rs`, `interact.rs`) — all provable
- `integrate_all`: compute `held_mask` once instead of 2–3× per agent/tick.
- `age_and_starve`: compute `lifespan` lazily, after the `energy <= 0.0`
  starvation short-circuit that leaves it unused.
- `trade_pass`/`pick_swap`: hoist the `want(_, give)` terms out of the inner loop.
Impact is workload-dependent (largest with inventions active / starvation-heavy
ticks); small on the default scattered bench. Kept as provably-no-op cleanups.

### Cumulative result & measurement caveat
Sense stage, chaining clean cool-regime back-to-back A/Bs: ≈ **−40%** at both 1k
and 10k (each step p=0.00). A single clean cumulative and a trustworthy
tick-level number require a quiescent/CI machine: the dev laptop thermally
throttles under sustained bench load (world-clone variance + throttling swamp
the ~10% tick signal), so per-stage deltas are the reliable instrument.
