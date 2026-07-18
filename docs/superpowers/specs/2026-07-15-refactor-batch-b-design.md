# Refactor Batch (b) — Design Spec

**Date:** 2026-07-15
**Status:** Approved (brainstorming) → ready for implementation plan
**Baseline:** branch `refactor-batch-b` off `perf-batch-a` (PR #20). Depends on batch (a).

## Motivation

Batch (b) of the codebase audit: pay down **core refactoring debt**, all
behavior-preserving. The two biggest files (`codex.rs` 1221 lines, `program.rs`
1124) mix many responsibilities, `codex.rs` repeats two logic shapes ~11×, and
`module.rs` has 6 copy-pasted accessors. None of this changes behavior.

**Load-bearing invariant:** every task must leave the golden state hash
**byte-identical** to the current pinned values
(`(0,0x446874c3858b4b55),(100,0x09e6b5822e9f7e4b),(1000,0xbdad4b8a324ae764)` from
batch (a)). These are pure code motion / extraction — "golden didn't move" **is**
the behavior-preservation proof. **No golden refresh anywhere in this batch.**
`observe_all`'s detector call order is preserved exactly (determinism depends on
it).

The codex perf-fusion (fewer full-population scans/tick) is **deferred** — it's
determinism-sensitive and needs profiling; the per-species-aggregation helper
built here is its foundation.

## Item 1 — De-duplicate the codex detectors

`codex.rs` repeats two shapes across ~11 detectors:

- **Edge-trigger latch:** `if fired && !world.codex.<X>_active.contains(sid) {
  push CodexEvent; active.insert(sid) } else if !fired { active.remove(sid) }`.
- **Per-species alive-aggregation header:** `for id in world.agents.iter_alive() {
  let sid = species_id[i]; map.entry(sid).or_default()… }`.

Extract two helpers (in the codex module root):
- `fn edge_trigger_species(active: &mut BTreeSet<u32>, sid: u32, fired: bool,
  make: impl FnOnce() -> CodexEvent) -> Option<CodexEvent>` — returns
  `Some(event)` on the rising edge and updates `active`; `None` otherwise. Callers
  push the returned event.
- A per-species aggregation combinator, e.g.
  `fn per_species_alive<T: Default>(world: &World, mut f: impl FnMut(&mut T, usize, u32)) -> BTreeMap<u32, T>`
  that runs one ascending-id `iter_alive()` pass and folds each agent into its
  species bucket. Detectors that currently hand-roll this header use it.

Route territory/niche/dialect/meme_sweep/cooperation/pack/herd/arms/novel detectors
through these. **Byte-identical output required** — same BTreeMap ordering, same
event-push order, same float ops.

## Item 2 — Split the two giant files by responsibility

**`codex.rs` → `codex/` module directory** (pure code motion):
- `codex/mod.rs`: `CodexState`, `EventType`, `CodexEvent`, `observe_all`, the pure
  signal fns (`species_spread`, `meme_l2`, `histogram_overlap`, `compute_centroids`,
  `centroid_of`, etc.), the new Item-1 helpers, and the module constants.
- `codex/spatial.rs`: `detect_territory_formation`, `detect_niche_partitioning`,
  `detect_migration`, `detect_herd_cohesion`.
- `codex/combat.rs`: `detect_predation`, `detect_combat_raid`, `detect_arms_race`,
  `detect_pack_hunting`.
- `codex/culture.rs`: `detect_dialect_formed`, `detect_meme_sweep`,
  `detect_alarm_call`, `detect_evolved_cooperation`.
- `codex/population.rs`: `detect_extinction`, `detect_population_crash`,
  `detect_novel_modules`, `detect_novel_behavior`, `detect_speciation` (whichever
  live here).

`observe_all` stays in `mod.rs` and calls the detectors in the **identical order**
as today. Detector fns become `pub(crate)` or `pub(super)` as needed. Existing
`#[cfg(test)]` tests move with their code or stay referencing the re-exported
paths — external test files (`tests/*.rs`) must keep compiling unchanged (the
public surface `anabios_core::codex::{EventType, CodexEvent, …}` is preserved via
`mod.rs` re-exports).

**`program.rs` → `program/` module directory:**
- `program/mod.rs`: `Node`, `Program`, `evaluate`, mutation/crossover, constants
  (`PHEROMONE_CHANNELS`, `MEME_CHANNELS`, `NO_TARGET`, `ActionRegister`).
- `program/starters.rs`: the `starter_*` functions + `starter_library`.

Public paths (`anabios_core::program::{Node, Program, ActionRegister, …}`) stay
identical via `mod.rs` re-exports.

## Item 3 — `module::effective_*` accessor helper

The 6 identical "max over an extracted module field" accessors
(`effective_perception_radius`, `effective_bite_size`, `effective_diet_carnivory`,
`effective_pheromone_strength`, `effective_armor_protection`,
`effective_communicator_range`) collapse behind:

```rust
fn max_param(modules: &ModuleList, extract: impl Fn(&Module) -> Option<f32>) -> f32 {
    modules.iter().filter_map(extract).fold(0.0, f32::max)
}
```

`effective_speed_max` (sum-fold) and `effective_weapon` (max_by damage) stay
bespoke. Identical float reduction → byte-identical.

## Item 4 — Dead-code hygiene (doc-only)

Add a one-line doc comment to each of the 9 behavior-inert genome slots
(`ImmuneStrength`, `KinPreference`, `Territoriality`, `ExploreVsExploit`,
`AmbushPreference`, `CommunicationStrength`, `OffspringInvestment`,
`MateChoosiness`, `SexualDimorphism`) noting "declared; not yet read by behavior."
**No rename, no index change** — keeps semantic intent and hash stability.
`feed_intent`/`mate_intent` are left as-is (already documented by the personality
work).

## Testing / verification

- After **each** task: golden test passes **byte-identical** (no refresh); full
  workspace `--lib --tests` green; `clippy`/`doc -D warnings` clean; external
  `tests/*.rs` compile unchanged (public paths preserved).
- The pure signal functions surfaced by the codex split (`species_spread`,
  `meme_l2`, `histogram_overlap`, etc.) MAY get a couple of direct unit tests now
  that they're accessible — nice-to-have, not required.
- CI gate: `rustup run stable` fmt/clippy/doc; commit fmt output.

## Out of scope

- Codex `observe_all` perf-fusion (deferred; needs profiling — Item 1's helper is
  its foundation).
- `sense_all` `scan_neighbors` extraction (deferred).
- Dropping `feed_intent`/`mate_intent` fields.
- Any behavior change / golden refresh.
- Godot frontend (batch c).

## Success criteria

`codex.rs` and `program.rs` are split into focused modules; the 11× codex
duplication and 6× module duplication are gone; inert slots are documented; the
simulation is **byte-for-byte identical** (golden unchanged) and CI is green.
