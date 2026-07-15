# Perf/Refactor Batch (a) — Design Spec

**Date:** 2026-07-15
**Status:** Approved (brainstorming) → ready for implementation plan
**Baseline:** `main` @ merge of PR #19 (Big Five personality)

## Motivation

A three-dimension audit of the codebase (perf hot-path, core refactoring, Godot
layer) surfaced a prioritized backlog. This spec covers **batch (a)** — the
highest value-to-risk items: one real correctness bug plus the two cheapest,
safest per-tick allocation wins. Baseline `tick_bench`: **0.95 ms/tick @ 1k
agents, 6.15 ms @ 10k** (O(n)-dominated; this is constant-factor reduction, not a
scaling fix).

Determinism guardrail: golden hashes are pinned in
`crates/anabios-core/tests/determinism.rs`. Items 2 and 3 are exactly hash-safe
(pure allocation changes; identical values/order). Item 1 is behavior-identical
but changes a *serialized-but-never-read* genome value, so it needs **one
deliberate golden refresh**.

## Item 1 — Remove the SpeedMax/DietCarnivory write-only trap

**The bug.** `TraitOverrides` lets scenarios set `speed_max` and
`diet_carnivory`, which write genome slots 25 (`SpeedMax`) and 27
(`DietCarnivory`). **Neither slot is ever read by behavior** — effective speed
comes from `module::effective_speed_max` (Locomotor `max_speed`, `integrate.rs`)
and effective diet from `module::effective_diet_carnivory` (Mouth
`diet_affinity`, `interact.rs:53,182`). So a scenario author who writes
`speed_max = 0.4` gets **zero effect**. Four shipped scenarios do this:
`minimal` (speed_max, diet_carnivory), `divergent` (2× each),
`gene-culture-alarm` (diet_carnivory), `predator-prey` (diet_carnivory).

**Decision (from brainstorming): remove the dead knobs.** Modules stay the single
source of truth for speed/diet.

**Changes:**
- `crates/anabios-core/src/scenario.rs`: delete the `speed_max` and
  `diet_carnivory` fields from `struct TraitOverrides` and their two `if let
  Some(v) = …` blocks in `TraitOverrides::apply`.
- Strip the now-ignored `speed_max` / `diet_carnivory` lines from the 4 scenario
  TOMLs (8 lines total).
- **Keep** the `GenomeSlot::SpeedMax` and `GenomeSlot::DietCarnivory` enum
  variants — the enum indices are load-bearing (serde layout) and `#[cfg(test)]`
  helpers use those slots as arbitrary examples. Only the `TraitOverrides` knob
  and the TOML lines go away.

**Determinism.** `TraitOverrides` has no `deny_unknown_fields`, so unknown TOML
keys are ignored — but stripping the lines is still correct hygiene. Removing the
`apply` writes means `minimal`'s genome slots 25/27 are no longer set to
`0.4`/`0.0`; they stay at the neutral `0.5`. That value is **never read by
behavior** (the simulation is byte-for-byte identical in every observable way),
but it *is* part of `bincode(World)` → the golden state hash changes. This is a
**deliberate, cosmetic golden refresh**: run `UPDATE_HASHES=1`, copy the new
triple into `determinism.rs`, and note in the commit that behavior is unchanged
(only a never-read genome slot's serialized value moved 0.4→0.5).

## Item 2 — Eliminate per-tick `iter_alive().collect()` allocations

**The cost.** ~6 hot sites heap-allocate a fresh `Vec<u32>` of alive ids every
tick (up to 2000 ids = 8 KB each): `tick.rs::decide_all`, `integrate.rs`,
`interact.rs`, `reproduce.rs`, `culture.rs`, `age.rs`, `species.rs`. These
snapshots exist to release the `&world.agents` borrow before the loop body
mutates `world` (or to snapshot the alive set before `reproduce` grows it).

**Fix.** Add a `World`-owned reusable buffer and thread it through the sites via
the take/refill/restore pattern, which preserves the borrow-release the current
`collect()` gives while reusing the allocation across ticks:

```rust
// on World:  pub scratch_ids: Vec<u32>,   (init empty)
let mut ids = std::mem::take(&mut world.scratch_ids);
ids.clear();
ids.extend(world.agents.iter_alive());
for &id in &ids {
    // ... existing loop body, mutating world ...
}
world.scratch_ids = ids; // restore (keeps capacity for next tick)
```

- The buffer is one shared field: the sites are sequential top-level tick stages,
  each takes/restores before the next runs, so a single buffer suffices. (If the
  plan finds a site where a nested borrow makes one shared buffer awkward, a
  second scratch field is acceptable — no behavior impact either way.)
- Where the loop does **not** mutate the alive set and doesn't need a snapshot
  (`integrate.rs`, `age.rs`), the plan may instead iterate `iter_alive()`
  directly and drop the `collect()` — but only if the borrow checker allows it
  without a snapshot; otherwise use the buffer. Whichever is chosen, iteration
  order (ascending id) MUST be identical.
- `scratch_ids` is scratch state on `World`. It is either `#[serde(skip)]` or
  otherwise excluded from `state_hash` so it never affects the golden hash.

**Determinism.** Same ids, same ascending-id order, no RNG touched → **byte-for-
byte identical hash**. This must NOT change the (refreshed) golden hash.

## Item 3 — Reuse the spatial-hash counts buffer

**The cost.** `UniformSpatialHash::rebuild` (`spatial.rs:47`) allocates
`let mut counts = vec![0_u32; total_cells]` (4096 cells = 16 KB) every tick.

**Fix.** Add a persistent `counts: Vec<u32>` field to `UniformSpatialHash`
(sized `total_cells` in the constructor), and in `rebuild` reset it with
`self.counts.fill(0)` instead of re-allocating. Same values, same algorithm.

**Determinism.** Pure allocation reuse → identical bucket layout → **no hash
change**.

## Testing / verification

- **Golden refresh once** for Item 1; then Items 2 and 3 must leave the golden
  hash **byte-identical** to the refreshed baseline (this is the proof that the
  allocation changes are behavior-preserving). Run the determinism test after
  each item.
- Full workspace suite green (`--lib --tests`).
- **Benchmark before/after:** run `cargo bench -p anabios-core --bench tick_bench`
  and record the delta at 1k and 10k agents in the final commit / PR. This
  quantifies the allocation win.
- CI gate (per project norm): `rustup run stable` fmt/clippy/doc `-D warnings`;
  commit fmt output; escape `` `[0,1]` ``/`` `[N]` `` in doc comments.

## Out of scope (later batches)

- The codex `observe_all` fused-pass perf refactor + codex/program file splits
  (batch b).
- Godot frontend per-frame wins (module_glyphs bundling, biome throttle,
  coevolution series caching, panel base class) (batch c).
- Wiring speed/diet as genome modulators (explicitly rejected in brainstorming —
  modules are the source of truth).
- The `scavenge_pass` O(agents×carcasses) question (needs profiling first).

## Success criteria

The SpeedMax/DietCarnivory footgun is gone (no scenario can request a silent
no-op); per-tick allocation churn is measurably reduced in `tick_bench`; the
simulation behaves identically (golden hash refreshed once for Item 1, then held
byte-stable through Items 2–3); CI green.
