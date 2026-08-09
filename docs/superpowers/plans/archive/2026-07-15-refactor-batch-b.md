# Refactor Batch (b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pay down core refactoring debt (split the two giant files, de-duplicate the codex latch logic, factor the module accessors, document inert slots) with zero behavior change.

**Architecture:** Pure code motion + small extractions in `anabios-core`. The golden state-hash test is the behavior-preservation proof — it must stay byte-identical after every task (no refresh anywhere in this batch). `observe_all`'s detector call order is preserved exactly.

**Tech Stack:** Rust (`anabios-core`).

## Global Constraints

- **Golden hash BYTE-IDENTICAL after every task** — pinned values
  `(0,0x446874c3858b4b55),(100,0x09e6b5822e9f7e4b),(1000,0xbdad4b8a324ae764)`.
  These are code motion / extraction; "golden didn't move" IS the correctness
  proof. **No golden refresh in this batch.** If a task moves the hash, it has a
  behavior bug — fix it, don't refresh.
- **`observe_all` detector call order is fixed** (determinism depends on it):
  `compute_centroids → update_pop_history → detect_extinction → detect_population_crash
  → detect_migration → detect_novel_modules → detect_novel_behavior → detect_predation
  → detect_combat_raid → detect_arms_race → detect_territory_formation →
  detect_niche_partitioning → detect_dialect_formed → detect_meme_sweep →
  detect_alarm_call → detect_evolved_cooperation → detect_pack_hunting →
  detect_herd_cohesion`.
- **Public API preserved:** `anabios_core::codex::{EventType, CodexEvent, CodexState,
  observe_all, species_spread, arms_race_signal, histogram_overlap, meme_l2, …}` and
  `anabios_core::program::{Node, Program, ActionRegister, PHEROMONE_CHANNELS,
  MEME_CHANNELS, NO_TARGET, evaluate, starter_library, …}` must resolve unchanged so
  external `tests/*.rs` compile without edits. Re-export from the new `mod.rs`.
- All work in `anabios-core`. CI gate — stable toolchain: `rustup run stable cargo
  fmt --all --check` / `clippy --workspace --all-targets -- -D warnings` /
  `RUSTDOCFLAGS="-D warnings" ... doc --workspace --no-deps --document-private-items`
  / `test --workspace --lib --tests`. Commit fmt output.
- **Scoping note:** the per-species-aggregation *combinator* (the second Item-1
  helper in the spec) is DEFERRED to the future codex perf-fusion (it is fiddly to
  make byte-identical across the varied detector aggregations, and it is the
  fusion's foundation). This batch does the clean, safe `edge_trigger_species`
  latch dedup only.

---

### Task 1: Split `codex.rs` into a `codex/` module

Move the 1221-line file into a directory of focused files. Pure code motion — golden must stay byte-identical and external tests must compile unchanged.

**Files:**
- Delete: `crates/anabios-core/src/codex.rs`
- Create: `crates/anabios-core/src/codex/mod.rs`, `codex/population.rs`, `codex/combat.rs`, `codex/spatial.rs`, `codex/culture.rs`

**Interfaces:**
- Produces: same public `codex::*` surface via `mod.rs` re-exports. Detector fns become `pub(super)`.

- [ ] **Step 1: Create the directory and move the file to `mod.rs`**

```bash
mkdir -p crates/anabios-core/src/codex
git mv crates/anabios-core/src/codex.rs crates/anabios-core/src/codex/mod.rs
```

- [ ] **Step 2: Carve detectors into submodule files**

Move these function bodies (verbatim, cut from `mod.rs`) into the new files, and change each moved `fn detect_*`/`fn update_pop_history` from `fn` to `pub(super) fn`:

- `codex/population.rs`: `update_pop_history`, `detect_extinction`, `detect_population_crash`, `detect_migration`, `detect_novel_modules`, `detect_novel_behavior`.
- `codex/combat.rs`: `detect_predation`, `detect_combat_raid`, `detect_arms_race`, `detect_pack_hunting`.
- `codex/spatial.rs`: `detect_territory_formation`, `detect_niche_partitioning`, `detect_herd_cohesion`.
- `codex/culture.rs`: `detect_dialect_formed`, `detect_meme_sweep`, `detect_alarm_call`, `detect_evolved_cooperation`.

**Keep in `codex/mod.rs`:** `CodexState`, `EventType`, `CodexEvent`, `CombatDeath`/`CombatHit` structs, all module constants, `observe_all`, `compute_centroids`, `centroid_of`, and the pure signal fns `species_spread`, `arms_race_signal`, `histogram_overlap`, `meme_l2`. Change `compute_centroids`/`centroid_of` (and any other helper a submodule calls) to `pub(super)`. Move each moved fn's associated `#[cfg(test)]` unit tests, if any, into the same submodule.

- [ ] **Step 3: Wire the submodules in `mod.rs`**

At the top of `codex/mod.rs`, after the existing `use`s, declare:

```rust
mod combat;
mod culture;
mod population;
mod spatial;
```

Update `observe_all` to call the detectors through their modules **in the exact same order** (Global Constraints), e.g. `population::detect_extinction(world, &centroids);`, `combat::detect_predation(world);`, `spatial::detect_territory_formation(world, &centroids);`, `culture::detect_dialect_formed(world, &centroids);`, etc. `update_pop_history` → `population::update_pop_history(world);`.

- [ ] **Step 4: Add the per-submodule `use` headers (compiler-guided)**

Each submodule needs imports for what it references. Start each with:
```rust
use super::*;
use crate::world::World;
```
Then `rustup run stable cargo build -p anabios-core` and add exactly what the compiler reports missing (e.g. `use crate::genome::GenomeSlot;`, `use crate::module::{self, ModuleType};`, `std::collections::{BTreeMap, BTreeSet, VecDeque}`, `glam::Vec2`). Iterate until it builds. Remove any now-unused imports from `mod.rs` that clippy flags.

- [ ] **Step 5: Verify golden byte-identical + external tests compile**

Run: `rustup run stable cargo build -p anabios-core`
Expected: clean.

Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: PASS, byte-identical (no refresh). If it FAILS, the move reordered something — fix.

Run: `rustup run stable cargo test -p anabios-core --lib --tests`
Expected: PASS — including all `tests/*.rs` that reference `codex::…` (public paths preserved).

- [ ] **Step 6: Lint + commit**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/codex
git commit -m "refactor(core): split codex.rs into codex/ submodules by detector group

Pure code motion; observe_all order preserved; golden byte-identical.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Extract the `edge_trigger_species` latch helper

De-duplicate the per-species edge-trigger latch repeated across ~7 detectors.

**Files:**
- Modify: `crates/anabios-core/src/codex/mod.rs` (add helper), `codex/spatial.rs`, `codex/culture.rs` (route detectors through it)

**Interfaces:**
- Produces: `pub(super) fn edge_trigger_species(active: &mut BTreeSet<u32>, sid: u32, fired: bool, make: impl FnOnce() -> CodexEvent) -> Option<CodexEvent>`

- [ ] **Step 1: Add the helper to `codex/mod.rs`**

```rust
/// Per-species edge-trigger latch. On the rising edge (`fired` true and `sid`
/// not already active) records `sid` active and returns the event to push; on a
/// falling edge (`!fired`) clears `sid`. Returns `None` when there is nothing to
/// emit. Centralizes the latch logic the detectors previously hand-rolled.
pub(super) fn edge_trigger_species(
    active: &mut std::collections::BTreeSet<u32>,
    sid: u32,
    fired: bool,
    make: impl FnOnce() -> CodexEvent,
) -> Option<CodexEvent> {
    if fired {
        if active.insert(sid) {
            return Some(make());
        }
    } else {
        active.remove(&sid);
    }
    None
}
```

- [ ] **Step 2: Route the `BTreeSet<u32>`-latched detectors through it**

For each detector whose latch state is a `BTreeSet<u32>` — `detect_territory_formation` (`territory_active`), `detect_herd_cohesion` (`herd_active`) in `spatial.rs`; `detect_dialect_formed` (`dialect_active`), `detect_evolved_cooperation` (`cooperation_active`) in `culture.rs` — replace the inline `if fired && !active.contains(&sid) { push; insert } else if !fired { remove }` block with:

```rust
if let Some(ev) = super::edge_trigger_species(&mut world.codex.<the_active_set>, sid, fired, || CodexEvent {
    event_type: EventType::<Variant>,
    tick,
    species_id: sid,
    value: <value>,
    loc_x: lx,
    loc_y: ly,
}) {
    to_push.push(ev);
}
```
preserving the EXACT event fields and the EXACT `fired` predicate each detector already computes. Do NOT touch detectors whose latch key is a tuple (`niche_active: BTreeSet<(u32,u32)>`, `meme_sweep_active: BTreeSet<(u32,u8)>`) or a plain `bool` (`raid_active`, `pack_active`, `arms_race_active`, `predation_emitted`, `alarm_emitted`) — the helper is keyed on `u32` species id only; those keep their current inline logic (out of scope for this helper).

- [ ] **Step 3: Verify golden byte-identical**

Run: `rustup run stable cargo build -p anabios-core`
Run: `rustup run stable cargo test -p anabios-core --test determinism`
Expected: PASS byte-identical. If it moves, an event field or predicate changed — fix.

Run: `rustup run stable cargo test -p anabios-core --lib --tests`
Expected: PASS.

- [ ] **Step 4: Lint + commit**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/codex
git commit -m "refactor(core): extract edge_trigger_species latch helper

Collapses the per-species edge-trigger latch across ~4 species-keyed detectors.
Golden byte-identical.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Split `program.rs` into a `program/` module

**Files:**
- Delete: `crates/anabios-core/src/program.rs`
- Create: `crates/anabios-core/src/program/mod.rs`, `program/starters.rs`

**Interfaces:**
- Produces: same public `program::*` surface via `mod.rs` re-exports.

- [ ] **Step 1: Create the directory and move**

```bash
mkdir -p crates/anabios-core/src/program
git mv crates/anabios-core/src/program.rs crates/anabios-core/src/program/mod.rs
```

- [ ] **Step 2: Move the starter library into `starters.rs`**

Cut these `pub fn` from `mod.rs` into `program/starters.rs` (verbatim): `starter_grazer`, `starter_stalker`, `starter_pack_hunter`, `starter_sentinel`, `starter_herd`, `starter_marker`, `starter_communicator`, `starter_cooperator`, `starter_cultural_cooperator`, `starter_asocial_forager`, `starter_culture_prey`, `starter_asocial_prey`, `starter_cultural_hunter`, `starter_library`, plus any `#[cfg(test)]` tests that exercise only the starters.

Keep in `program/mod.rs`: `Node`, `ActionRegister`, `Program`, `EvalContext`, all constants (`PROGRAM_MAX_NODES`, `PHEROMONE_CHANNELS`, `MEME_CHANNELS`, `NO_TARGET`, mutation-prob consts), `evaluate`, `random_node`, `point_mutate`, `structural_mutate`, `crossover_and_mutate`, and any `Program`/`Node` impls.

- [ ] **Step 3: Wire + re-export**

In `program/mod.rs` add `mod starters;` and, so external callers keep resolving `program::starter_library` etc., re-export:
```rust
pub use starters::*;
```
Add `use super::*;` (plus whatever the compiler asks for) to the top of `starters.rs`.

- [ ] **Step 4: Verify + lint + commit**

Run: `rustup run stable cargo build -p anabios-core`; `... test -p anabios-core --test determinism` (byte-identical); `... test -p anabios-core --lib --tests` (PASS, external tests referencing `program::starter_*` / `program::Node` still resolve).

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/program
git commit -m "refactor(core): split program.rs — evaluator/mutation vs starter library

Pure code motion; public paths preserved via re-export; golden byte-identical.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Factor the `module::effective_*` accessors

**Files:**
- Modify: `crates/anabios-core/src/module.rs`

**Interfaces:**
- Produces: `fn max_param(modules: &ModuleList, extract: impl Fn(&Module) -> Option<f32>) -> f32` (private).

- [ ] **Step 1: Add the helper**

In `crates/anabios-core/src/module.rs`, add near the accessors:
```rust
/// Fold the extracted per-module parameter with `f32::max`, defaulting to 0.0
/// when no module contributes. Shared by the "strongest module wins" accessors.
fn max_param(modules: &ModuleList, extract: impl Fn(&Module) -> Option<f32>) -> f32 {
    modules.iter().filter_map(extract).fold(0.0, f32::max)
}
```

- [ ] **Step 2: Rewrite the 6 max-accessors through it**

Rewrite `effective_perception_radius`, `effective_bite_size`, `effective_diet_carnivory`, `effective_pheromone_strength`, `effective_armor_protection`, `effective_communicator_range` so each body is a single `max_param(modules, |m| match m { Module::<Variant> { <field>, .. } => Some(*<field>), _ => None })`, using the SAME variant/field each currently matches. **Do NOT touch** `effective_speed_max` (sum-fold) or `effective_weapon` (max_by damage) — their reduction differs. The float result must be identical (`filter_map(...).fold(0.0, f32::max)` matches the existing `iter().filter_map(...).fold(0.0, f32::max)` bodies).

- [ ] **Step 3: Verify + lint + commit**

Run: `rustup run stable cargo test -p anabios-core --test determinism` (byte-identical); `... --lib module::` (module tests pass).
```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/module.rs
git commit -m "refactor(core): factor module::effective_* max-accessors behind max_param

Golden byte-identical.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Document the inert genome slots

**Files:**
- Modify: `crates/anabios-core/src/genome.rs`

- [ ] **Step 1: Add doc comments**

In the `GenomeSlot` enum, add a one-line `///` doc comment above each of these 9 variants (do NOT rename them, do NOT change indices): `ImmuneStrength`, `KinPreference`, `Territoriality`, `ExploreVsExploit`, `AmbushPreference`, `CommunicationStrength`, `OffspringInvestment`, `MateChoosiness`, `SexualDimorphism`. Each comment states it is declared but not yet wired, e.g.:
```rust
    /// Declared; not yet read by behavior. Reserved for a future kin-biased
    /// cooperation rule.
    KinPreference = 14,
```
Write a slot-appropriate one-liner for each (e.g. `Territoriality` → "future territory-defense drive"; `ImmuneStrength` → "future disease-resistance modifier"; `CommunicationStrength` → "future Communicator-module gain"; `OffspringInvestment`/`MateChoosiness`/`SexualDimorphism` → "future sexual-selection knob"; `ExploreVsExploit` → "future foraging-strategy bias"; `AmbushPreference` → "future ambush-vs-pursuit bias"). Any doc-comment escaping rule applies (`` `[0,1]` ``).

- [ ] **Step 2: Verify + commit**

Run: `RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc -p anabios-core --no-deps --document-private-items` (clean); `rustup run stable cargo test -p anabios-core --test determinism` (byte-identical — comments don't change bytes).
```bash
rustup run stable cargo fmt --all
git add crates/anabios-core/src/genome.rs
git commit -m "docs(core): mark the 9 behavior-inert genome slots as declared-not-wired

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Full verification

- [ ] **Step 1: Full CI gate**
```bash
rustup run stable cargo fmt --all --check
rustup run stable cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc --workspace --no-deps --document-private-items
rustup run stable cargo test --workspace --lib --tests
```
Expected: all PASS.

- [ ] **Step 2: Determinism byte-identity (the whole-batch proof)**
Run: `rustup run stable cargo test -p anabios-core --test determinism` (twice)
Expected: PASS against the UNCHANGED pinned hashes — confirming the entire batch was behavior-preserving.

- [ ] **Step 3: File-size sanity**
Run: `wc -l crates/anabios-core/src/codex/*.rs crates/anabios-core/src/program/*.rs`
Expected: no single file near the old 1221/1124; each detector group is a focused file.

---

## Self-Review

**Spec coverage:**
- Item 1 codex dedup: `edge_trigger_species` latch → Task 2. The per-species-aggregation *combinator* is DEFERRED to the perf-fusion (documented in Global Constraints scoping note) — a deliberate reduction because it is byte-identity-risky and is the fusion's foundation. ⚠️ conscious deviation from the spec's "two helpers", flagged.
- Item 2 splits: codex → Task 1; program → Task 3. ✅
- Item 3 module helper → Task 4. ✅
- Item 4 inert-slot doc comments → Task 5. ✅
- Invariant (golden byte-identical every task, observe_all order, public paths) → Global Constraints + each task's verify step. ✅ CI gate → Task 6. ✅
- Out-of-scope (perf-fusion, sense extraction, feed/mate drop, godot) → absent. ✅

**Placeholder scan:** No TBD/TODO. Import lists in Tasks 1/3 are explicitly compiler-guided (correct for code motion — the exact set can't be pre-enumerated without reading every detector body, and the compiler enumerates it precisely). Not a placeholder.

**Type consistency:** `edge_trigger_species` signature identical in Task 2 def and use. Detector→file assignments in Task 1 match the `observe_all` order in Global Constraints. `max_param` signature consistent (Task 4). Module declarations (`mod combat; mod culture; mod population; mod spatial;` / `mod starters;`) match the created files.
