# M-F: Observability & Showcase + Determinism Hardening — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the primitive-brain affect layer *legible*. Add four append-only codex detectors for the emergent affective phenomena (PANIC CASCADE, FEEDING FRENZY, TERRITORIAL RAGE, MASS GRIEF), surface each agent's arousal to the Godot viewer as a body tint, ship a flag-ON showcase scenario that visibly produces a panic cascade / feeding frenzy, and run a full save/load + determinism hardening pass over every serialized affect and detector column.

**Architecture:** The detectors are pure observers in the codex stage (`observe_all`, tick stage 9), reading the serialized `world.agents.affect` column (written earlier the same tick by `affect::develop_all`, post-sense/pre-decide) plus a new per-species affect aggregate on `SpeciesAgg`. They push new `EventType`s and latch/streak state into `CodexState` (serialized). All detector work is gated on `world.affect_enabled` so flag-off is a strict no-op (empty new fields ⇒ layout growth only, byte-identical trajectory). The viewer reads a new `alive_arousal()` FFI accessor (mirroring `alive_energy`), which is naturally neutral when the flag is off because the affect column is all-zero. A new showcase scenario TOML sets `affect_enabled = true`.

**Tech Stack:** Rust (anabios-core, anabios-godot/gdext), GDScript (game/), cargo test, godot headless.

## Global Constraints
- Single Xoshiro256++ RNG; codex detectors are observer-only, ZERO tick-RNG impact; new EventType appends at END of enum; all detector state feeding hashed state must be serialized (never #[serde(skip)]); flag-off byte-identical (determinism+cognition+inventions); viewer affect surfacing neutral when affect_enabled off; golden refresh UPDATE_HASHES=1; bump FORMAT_VERSION if serialized layout changes; controller runs cargo/git/golden/godot/commit gates.

## Dependencies
- Requires M-A..M-D merged (all affect systems present): the `affect` column + `affect_prev_crowding` column on `AgentBuffers`, `affect::develop_all`/`affect::arousal`, the `affect_enabled` world+scenario flag, and the SEEK/FEAR/RAGE/CARE/PANIC activations that these detectors read. M-E (PLAY) optional — no detector reads PLAY.
- Interface contract: `docs/superpowers/plans/2026-08-02-affect-layer-interface-contract.md`. This milestone CONSUMES `AffectState`/`AFFECT_SYSTEMS`/`SEEK..PANIC` indices and `affect::arousal`; it does NOT rename or re-signature them.

---

## File Structure

Rust core (anabios-core):
- `crates/anabios-core/src/codex/event.rs` — append 4 `EventType` variants (`PanicCascade`, `FeedingFrenzy`, `TerritorialRage`, `MassGrief`) at the END; bump `EVENT_TYPE_COUNT` to track the new last variant.
- `crates/anabios-core/src/codex/params.rs` — append the detector tuning constants.
- `crates/anabios-core/src/codex/agg.rs` — add per-species affect aggregates to `SpeciesAgg` (sums + high-activation member counts), wired into `build`, `Default`, and `reset`.
- `crates/anabios-core/src/codex/affect_events.rs` — NEW module: the 4 detectors + their unit tests.
- `crates/anabios-core/src/codex/mod.rs` — register `mod affect_events;`, add the `CodexState` latch/streak/history fields, and call the detectors from `observe_all` (gated on `world.affect_enabled`).
- `crates/anabios-core/src/snapshot.rs` — bump `FORMAT_VERSION` + `///` changelog entry.

Viewer (anabios-godot + game/):
- `crates/anabios-godot/src/lib.rs` — add `#[func] alive_arousal()` (mirrors `alive_energy`, ~:500) + an `arousal`/`dominant_affect` entry in `agent_detail` (~:563).
- `game/scripts/overlay_manager.gd` — add `BODY_AFFECT` body mode.
- `game/scripts/main.gd` — add the `BODY_AFFECT` branch to `_body_colors` (~:582).
- `game/scripts/legend_panel.gd` — extend `BODY_NAMES` + `_rebuild_key` (~:9, ~:59).
- `game/scripts/codex_panel.gd` — extend `CHAPTER_NAMES` + `CHAPTER_COLORS` by the 4 new event types (~:4, ~:61).

Scenario + tests:
- `scenarios/affect-showcase.toml` — NEW flag-ON scenario producing a visible panic cascade / feeding frenzy.
- `crates/anabios-core/tests/codex_events.rs` — flag-ON integration test that the showcase scenario fires ≥1 affect event.
- `crates/anabios-core/tests/determinism.rs` — save→load→step round-trip over affect + detector state; golden refresh.
- `crates/anabios-core/tests/cognition.rs`, `crates/anabios-core/tests/inventions.rs` — golden refresh (layout growth).

---

## Task 1 — Append the four EventType variants

- [ ] **Files:** `crates/anabios-core/src/codex/event.rs`
- [ ] **Interfaces:** four new `EventType` discriminants **appended after M-B's `MassFright = 53`**: `PanicCascade = 54`, `FeedingFrenzy = 55`, `TerritorialRage = 56`, `MassGrief = 57`; `EVENT_TYPE_COUNT` re-derived from the new last variant. **At execution time confirm the current last discriminant** (`git grep -n "= 5" crates/anabios-core/src/codex/event.rs`) and append after it — M-B (MassFright) precedes M-F, so do NOT reuse 53.
- [ ] Write the failing test first (append to `event.rs`'s `#[cfg(test)]` block — create one if absent, mirroring the enum-adjacent tests elsewhere):

```rust
#[cfg(test)]
mod affect_event_tests {
    use super::*;
    #[test]
    fn affect_event_discriminants_are_appended_at_end() {
        // Append-only invariant: the pre-existing tail keeps its discriminant,
        // and the four new affect events follow it in order.
        assert_eq!(EventType::LivestockHerd as u8, 52);
        assert_eq!(EventType::MassFright as u8, 53); // from M-B
        assert_eq!(EventType::PanicCascade as u8, 54);
        assert_eq!(EventType::FeedingFrenzy as u8, 55);
        assert_eq!(EventType::TerritorialRage as u8, 56);
        assert_eq!(EventType::MassGrief as u8, 57);
        assert_eq!(EVENT_TYPE_COUNT, EventType::MassGrief as usize + 1);
    }
}
```

- [ ] Run: `cargo test -p anabios-core --lib codex::event` → **fails** (variants absent).
- [ ] Implement: after M-B's `MassFright = 53,` (the current last variant) add the four `///`-doc'd variants with explicit discriminants 54–57. Update `EVENT_TYPE_COUNT` to `EventType::MassGrief as usize + 1`.
- [ ] Run: `cargo test -p anabios-core --lib codex::event` → **passes**.
- [ ] Commit: `feat(codex): append affect EventTypes (panic cascade, feeding frenzy, territorial rage, mass grief)`.

## Task 2 — Per-species affect aggregate on SpeciesAgg

- [ ] **Files:** `crates/anabios-core/src/codex/agg.rs`
- [ ] **Interfaces:** new `SpeciesAgg` fields — `affect_sum: [f64; crate::affect::AFFECT_SYSTEMS]` (per-system activation sums over alive members, for species means) and `high_fear: u32`, `high_seek: u32`, `high_rage: u32`, `high_panic: u32` (member counts above the per-system HIGH thresholds from params). These are `#[serde(skip)]`-adjacent scratch (the whole table lives behind `World.codex_agg` `#[serde(skip)]`), so they never enter the snapshot or hash — only the detector *outputs* do.
- [ ] Write the failing test first (add to agg.rs `#[cfg(test)]`):

```rust
#[test]
fn build_aggregates_affect_activations() {
    use crate::affect::{FEAR, SEEK};
    let mut w = World::new(11);
    w.affect_enabled = true;
    let a = w.spawn_agent(Vec2::new(100.0, 100.0), Genome::neutral());
    let b = w.spawn_agent(Vec2::new(120.0, 100.0), Genome::neutral());
    // Stamp distinctive activations directly on the serialized column.
    w.agents.affect[a as usize][FEAR] = 0.9;
    w.agents.affect[b as usize][FEAR] = 0.1;
    w.agents.affect[a as usize][SEEK] = 0.8;
    let mut agg = SpeciesAggTable::default();
    agg.build(&w);
    let e = agg.get(0).expect("species 0");
    assert!((e.affect_sum[FEAR] - 1.0).abs() < 1e-5);
    assert!((e.affect_sum[SEEK] - 0.8).abs() < 1e-5);
    // Only `a` is above the HIGH-fear threshold (see params HIGH_* consts).
    assert_eq!(e.high_fear, 1);
}
```

- [ ] Run: `cargo test -p anabios-core --lib codex::agg::tests::build_aggregates_affect_activations` → **fails** (fields absent).
- [ ] Implement in `agg.rs`:
  - Add the fields to `SpeciesAgg` (after `crowding_sum`, agg.rs:50), to `Default` (agg.rs:88), and to `reset` (agg.rs:117) — all three, or the `reset_restores_default_state` guard test will catch the omission.
  - In `build` (agg.rs:167), after the `crowding_sum` accumulation (agg.rs:212), add a gated block:

```rust
if world.affect_enabled && world.agents.affect.len() > i {
    let af = &world.agents.affect[i];
    for (k, s) in e.affect_sum.iter_mut().enumerate() {
        *s += af[k] as f64;
    }
    use crate::affect::{FEAR, PANIC, RAGE, SEEK};
    use super::{HIGH_FEAR, HIGH_PANIC, HIGH_RAGE, HIGH_SEEK};
    if af[FEAR] >= HIGH_FEAR { e.high_fear += 1; }
    if af[SEEK] >= HIGH_SEEK { e.high_seek += 1; }
    if af[RAGE] >= HIGH_RAGE { e.high_rage += 1; }
    if af[PANIC] >= HIGH_PANIC { e.high_panic += 1; }
}
```

  (The `HIGH_*` consts land in Task 3's params; if implementing strictly in order, temporarily inline the literals and replace them when params exist, or land Task 3's params first — controller's call.)
- [ ] Run the new test + `cargo test -p anabios-core --lib codex::agg` (the `reset_restores_default_state` guard must still pass) → **passes**.
- [ ] Commit: `feat(codex): aggregate per-species affect activations in SpeciesAgg`.

## Task 3 — Detector params

- [ ] **Files:** `crates/anabios-core/src/codex/params.rs`
- [ ] **Interfaces:** append (documented) constants. No standalone test — verified through the detector tests in Tasks 4–7. Suggested values:

```rust
/// Per-system "high activation" thresholds (leaky-integrator value in [0,1]).
pub const HIGH_FEAR: f32 = 0.6;
pub const HIGH_SEEK: f32 = 0.6;
pub const HIGH_RAGE: f32 = 0.6;
pub const HIGH_PANIC: f32 = 0.6;

/// PANIC CASCADE: high-FEAR member count must jump by >= this within the window.
pub const CASCADE_MIN_SPREAD: u32 = 5;
/// Ticks over which the high-FEAR count is compared for a cascade rise.
pub const AFFECT_CASCADE_WINDOW: usize = 20;
/// Min species members before a cascade is meaningful.
pub const CASCADE_MIN_MEMBERS: u32 = 8;

/// FEEDING FRENZY: converged high-SEEK members required.
pub const FRENZY_MIN_MEMBERS: u32 = 6;
/// Max RMS spatial spread (world units) for the high-SEEK members to count as
/// "converged" on one patch.
pub const FRENZY_SPREAD_MAX: f32 = 90.0;

/// TERRITORIAL RAGE: species mean RAGE at/above this counts as an angry cluster.
pub const RAGE_CLUSTER_MEAN: f32 = 0.5;
/// Max RMS spread for the cluster to be territorial (co-located aggression).
pub const RAGE_CLUSTER_SPREAD_MAX: f32 = 120.0;
/// Ticks of sustained angry-cluster before TerritorialRage fires.
pub const RAGE_WINDOW: u32 = 60;
/// Min members for a rage cluster.
pub const RAGE_MIN_MEMBERS: u32 = 5;

/// MASS GRIEF: species mean PANIC at/above this after a die-off.
pub const GRIEF_MEAN_PANIC: f32 = 0.45;
/// Population-drop fraction (over the grief window) that qualifies as a die-off.
pub const GRIEF_DROP_FRAC: f32 = 0.3;
/// Ticks over which the die-off drop is measured.
pub const GRIEF_WINDOW: usize = 60;
/// Min pre-die-off population for grief to be meaningful.
pub const GRIEF_MIN_PEAK: u32 = 12;
```

- [ ] Run: `cargo build -p anabios-core` → compiles (constants unused until Tasks 4–7 land; acceptable within the batch, or land this immediately before Task 2's `build` block references `HIGH_*`).
- [ ] Commit: `feat(codex): affect-detector tuning params`.

## Task 4 — FEEDING FRENZY detector

- [ ] **Files:** `crates/anabios-core/src/codex/affect_events.rs` (new), `crates/anabios-core/src/codex/mod.rs` (register `mod affect_events;` in the module list, mod.rs:21-40).
- [ ] **Interfaces:** `pub(super) fn detect_feeding_frenzy(world: &mut World, agg: &SpeciesAggTable)`. Fires (edge-triggered per species via `edge_trigger_species`, mod.rs:401) when `high_seek >= FRENZY_MIN_MEMBERS` AND the high-SEEK members' RMS spatial spread `<= FRENZY_SPREAD_MAX` (they have converged on a patch). `value` = high-SEEK member count; `loc` = high-SEEK centroid. New `CodexState` field `frenzy_active: BTreeSet<u32>` (Task 8).
- [ ] Write the failing test first in `affect_events.rs` `#[cfg(test)]` (follow the signatures.rs test idiom: build a `World`, stamp the `affect` column, hand-build an `agg` or call `agg.build`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::affect::SEEK;
    use crate::genome::Genome;
    use crate::prelude::Vec2;

    fn frenzy_world(n: u32, seek: f32, spread: f32) -> (World, SpeciesAggTable) {
        let mut w = World::new(3);
        w.affect_enabled = true;
        for k in 0..n {
            // Tight cluster: x within `spread` of the centroid.
            let x = 500.0 + (k as f32 / n as f32) * spread;
            let id = w.spawn_agent(Vec2::new(x, 500.0), Genome::neutral());
            w.agents.affect[id as usize][SEEK] = seek;
        }
        let mut agg = SpeciesAggTable::default();
        agg.build(&w);
        (w, agg)
    }

    #[test]
    fn converged_high_seek_fires_and_latches() {
        let (mut w, agg) = frenzy_world(8, 0.9, 40.0);
        detect_feeding_frenzy(&mut w, &agg);
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::FeedingFrenzy));
        let before = w.codex.events.len();
        detect_feeding_frenzy(&mut w, &agg); // latched: no re-fire
        assert_eq!(w.codex.events.len(), before);
    }

    #[test]
    fn scattered_seekers_do_not_frenzy() {
        let (mut w, agg) = frenzy_world(8, 0.9, 400.0); // spread > FRENZY_SPREAD_MAX
        detect_feeding_frenzy(&mut w, &agg);
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn calm_species_does_not_frenzy() {
        let (mut w, agg) = frenzy_world(8, 0.1, 40.0); // SEEK below HIGH_SEEK
        detect_feeding_frenzy(&mut w, &agg);
        assert!(w.codex.events.is_empty());
    }
}
```

- [ ] Run: `cargo test -p anabios-core --lib codex::affect_events` → **fails** (module/detector absent).
- [ ] Implement `detect_feeding_frenzy`: iterate `agg.active()`, skip species with `high_seek < FRENZY_MIN_MEMBERS`; recompute the high-SEEK members' centroid + RMS spread by scanning `agg.get(sid).member_idx` and reading `world.agents.affect[i][SEEK] >= HIGH_SEEK` (positions from `world.agents.position[i]`, torus-aware via `crate::spatial::torus_distance` against the centroid, matching TerritoryFormation's spread math); if spread `<= FRENZY_SPREAD_MAX`, `edge_trigger_species(&mut world.codex.frenzy_active, sid, true, || CodexEvent{..})`. Collect events into a `Vec` then push (borrow discipline as in signatures.rs).
- [ ] Run: `cargo test -p anabios-core --lib codex::affect_events` → **passes**.
- [ ] Commit: `feat(codex): FeedingFrenzy detector (converged high-SEEK cluster)`.

## Task 5 — TERRITORIAL RAGE detector

- [ ] **Files:** `crates/anabios-core/src/codex/affect_events.rs`, `crates/anabios-core/src/codex/mod.rs` (CodexState field in Task 8).
- [ ] **Interfaces:** `pub(super) fn detect_territorial_rage(world: &mut World, agg: &SpeciesAggTable)`. Streak-based (mirrors settlement/kin): each tick, a species whose `count >= RAGE_MIN_MEMBERS`, mean RAGE (`affect_sum[RAGE]/count`) `>= RAGE_CLUSTER_MEAN`, and RMS spread `<= RAGE_CLUSTER_SPREAD_MAX` advances `rage_streak[sid] += 1`, else resets to 0; when the streak crosses `RAGE_WINDOW`, fire via `edge_trigger_species(&mut world.codex.rage_active, ..)`. New fields `rage_streak: BTreeMap<u32,u32>`, `rage_active: BTreeSet<u32>`.
- [ ] Write the failing test first:

```rust
#[test]
fn sustained_angry_cluster_fires_after_window() {
    use crate::affect::RAGE;
    let mut w = World::new(4);
    w.affect_enabled = true;
    for k in 0..6 {
        let id = w.spawn_agent(Vec2::new(500.0 + k as f32 * 8.0, 500.0), Genome::neutral());
        w.agents.affect[id as usize][RAGE] = 0.8;
    }
    let mut agg = SpeciesAggTable::default();
    // Advance the streak past RAGE_WINDOW; agg is rebuilt each tick in the sim,
    // here we reuse a fresh build each iteration.
    for _ in 0..=crate::codex::RAGE_WINDOW {
        agg.build(&w);
        detect_territorial_rage(&mut w, &agg);
        w.tick += 1;
    }
    assert!(w.codex.events.iter().any(|e| e.event_type == EventType::TerritorialRage));
}

#[test]
fn brief_anger_does_not_fire() {
    use crate::affect::RAGE;
    let mut w = World::new(4);
    w.affect_enabled = true;
    for k in 0..6 {
        let id = w.spawn_agent(Vec2::new(500.0 + k as f32 * 8.0, 500.0), Genome::neutral());
        w.agents.affect[id as usize][RAGE] = 0.8;
    }
    let mut agg = SpeciesAggTable::default();
    agg.build(&w);
    for _ in 0..5 { detect_territorial_rage(&mut w, &agg); w.tick += 1; }
    assert!(w.codex.events.is_empty());
}
```

- [ ] Run → **fails**. Implement the streak detector (read `agg.get(sid)`; spread computed like Task 4 over all members, not just high-RAGE). Run → **passes**.
- [ ] Commit: `feat(codex): TerritorialRage detector (sustained high-RAGE cluster)`.

## Task 6 — PANIC CASCADE detector

- [ ] **Files:** `crates/anabios-core/src/codex/affect_events.rs`, `crates/anabios-core/src/codex/mod.rs` (CodexState field in Task 8).
- [ ] **Interfaces:** `pub(super) fn detect_panic_cascade(world: &mut World, agg: &SpeciesAggTable)`. Contagion signal: per species, push the tick's `high_fear` count into a rolling `fear_count_history: BTreeMap<u32, VecDeque<u32>>` (window `AFFECT_CASCADE_WINDOW`). Fire (edge-triggered `cascade_active: BTreeSet<u32>`) when `count >= CASCADE_MIN_MEMBERS` and the high-FEAR count *rose* by `>= CASCADE_MIN_SPREAD` from the window front to the window back (fear propagated through the cluster in a short span). Re-arm when the high-FEAR count falls back below `CASCADE_MIN_SPREAD`. `value` = current high-FEAR count.
- [ ] Write the failing test first (drive the history: low high-FEAR early, high later):

```rust
#[test]
fn fear_spreading_through_cluster_fires_cascade() {
    use crate::affect::FEAR;
    let mut w = World::new(6);
    w.affect_enabled = true;
    let ids: Vec<u32> = (0..10)
        .map(|k| w.spawn_agent(Vec2::new(500.0 + k as f32 * 5.0, 500.0), Genome::neutral()))
        .collect();
    let mut agg = SpeciesAggTable::default();
    // Window front: nobody afraid.
    agg.build(&w);
    detect_panic_cascade(&mut w, &agg);
    w.tick += 1;
    // Window back: fear has spread to the whole cluster.
    for &id in &ids { w.agents.affect[id as usize][FEAR] = 0.9; }
    for _ in 0..(crate::codex::AFFECT_CASCADE_WINDOW as u64) {
        agg.build(&w);
        detect_panic_cascade(&mut w, &agg);
        w.tick += 1;
    }
    assert!(w.codex.events.iter().any(|e| e.event_type == EventType::PanicCascade));
}

#[test]
fn steady_low_fear_does_not_cascade() {
    let mut w = World::new(6);
    w.affect_enabled = true;
    for k in 0..10 { w.spawn_agent(Vec2::new(500.0 + k as f32 * 5.0, 500.0), Genome::neutral()); }
    let mut agg = SpeciesAggTable::default();
    for _ in 0..(crate::codex::AFFECT_CASCADE_WINDOW as u64 + 2) {
        agg.build(&w);
        detect_panic_cascade(&mut w, &agg);
        w.tick += 1;
    }
    assert!(w.codex.events.is_empty());
}
```

- [ ] Run → **fails**. Implement (push into history, prune to `AFFECT_CASCADE_WINDOW`, compare front vs back; guard `count >= CASCADE_MIN_MEMBERS`). Run → **passes**.
- [ ] Commit: `feat(codex): PanicCascade detector (fear contagion through a cluster)`.

## Task 7 — MASS GRIEF detector

- [ ] **Files:** `crates/anabios-core/src/codex/affect_events.rs`, `crates/anabios-core/src/codex/mod.rs` (CodexState field in Task 8).
- [ ] **Interfaces:** `pub(super) fn detect_mass_grief(world: &mut World, agg: &SpeciesAggTable)`. Reuses the crash detector's `world.codex.pop_history` (already maintained by `update_pop_history`, which runs first in `observe_all`): compute the population drop over the last `GRIEF_WINDOW` samples; fire (edge-triggered `grief_active: BTreeSet<u32>`) when peak-in-window `>= GRIEF_MIN_PEAK`, drop-fraction `>= GRIEF_DROP_FRAC`, AND current species mean PANIC (`affect_sum[PANIC]/count`) `>= GRIEF_MEAN_PANIC`. `value` = mean PANIC. Re-arm when mean PANIC falls back below the threshold.
- [ ] Write the failing test first (seed `pop_history` directly, like population.rs tests do via `update_pop_history`; stamp PANIC on survivors):

```rust
#[test]
fn dieoff_with_high_panic_fires_grief() {
    use crate::affect::PANIC;
    use std::collections::VecDeque;
    let mut w = World::new(8);
    w.affect_enabled = true;
    // A handful of grieving survivors.
    for k in 0..4 {
        let id = w.spawn_agent(Vec2::new(500.0 + k as f32 * 6.0, 500.0), Genome::neutral());
        w.agents.affect[id as usize][PANIC] = 0.8;
    }
    // Seed a population window that fell from 20 → 4 (a die-off).
    let mut buf: VecDeque<u32> = VecDeque::new();
    for _ in 0..(crate::codex::GRIEF_WINDOW - 1) { buf.push_back(20); }
    buf.push_back(4);
    w.codex.pop_history.insert(0, buf);
    let mut agg = SpeciesAggTable::default();
    agg.build(&w);
    detect_mass_grief(&mut w, &agg);
    assert!(w.codex.events.iter().any(|e| e.event_type == EventType::MassGrief));
}

#[test]
fn dieoff_without_panic_is_not_grief() {
    use std::collections::VecDeque;
    let mut w = World::new(8);
    w.affect_enabled = true;
    for k in 0..4 { w.spawn_agent(Vec2::new(500.0 + k as f32 * 6.0, 500.0), Genome::neutral()); }
    let mut buf: VecDeque<u32> = VecDeque::new();
    for _ in 0..(crate::codex::GRIEF_WINDOW - 1) { buf.push_back(20); }
    buf.push_back(4);
    w.codex.pop_history.insert(0, buf);
    let mut agg = SpeciesAggTable::default();
    agg.build(&w);
    detect_mass_grief(&mut w, &agg); // PANIC == 0 ⇒ no grief
    assert!(w.codex.events.is_empty());
}
```

- [ ] Run → **fails**. Implement (read `world.codex.pop_history`, window slice `GRIEF_WINDOW`). Run → **passes**.
- [ ] Commit: `feat(codex): MassGrief detector (population PANIC after a die-off)`.

## Task 8 — Wire CodexState fields + register in observe_all (flag-gated)

- [ ] **Files:** `crates/anabios-core/src/codex/mod.rs`
- [ ] **Interfaces:** add the serialized detector state to `CodexState` (mod.rs:64-274), grouped at the end before `events`:

```rust
/// Species currently latched as feeding-frenzy (re-arms on drop).
pub frenzy_active: BTreeSet<u32>,
/// Per-species sustained-angry-cluster streak (TerritorialRage).
pub rage_streak: BTreeMap<u32, u32>,
pub rage_active: BTreeSet<u32>,
/// Rolling per-species high-FEAR member counts (PanicCascade contagion).
pub fear_count_history: BTreeMap<u32, VecDeque<u32>>,
pub cascade_active: BTreeSet<u32>,
/// Species currently latched as mass-grieving (re-arms when PANIC subsides).
pub grief_active: BTreeSet<u32>,
```

  (All are ordinary serialized fields — `CodexState` derives `Serialize`/`Deserialize` and marks NO field `#[serde(skip)]`; being in `CodexState` they are hashed. This is the serde-skip-footgun avoidance for the detector state.)
- [ ] Register `pub(crate) mod affect_events;` in the module list (mod.rs:21-40).
- [ ] In `observe_all` (mod.rs:326), after `update_pop_history` (so MassGrief sees the fresh window) — and gated on the flag so flag-off is a strict no-op — add:

```rust
if world.affect_enabled {
    affect_events::detect_feeding_frenzy(world, &agg);
    affect_events::detect_territorial_rage(world, &agg);
    affect_events::detect_panic_cascade(world, &agg);
    affect_events::detect_mass_grief(world, &agg);
}
```

- [ ] Write the failing test first (add to `affect_events.rs` `#[cfg(test)]`) proving flag-off is inert through the real `observe_all`:

```rust
#[test]
fn observe_all_is_inert_when_affect_disabled() {
    let mut w = World::new(9);
    debug_assert!(!w.affect_enabled);
    for k in 0..12 { w.spawn_agent(Vec2::new(500.0 + k as f32 * 4.0, 500.0), Genome::neutral()); }
    for _ in 0..5 { crate::tick::step(&mut w); }
    assert!(!w.codex.events.iter().any(|e| matches!(
        e.event_type,
        EventType::FeedingFrenzy | EventType::TerritorialRage
            | EventType::PanicCascade | EventType::MassGrief
    )));
    assert!(w.codex.frenzy_active.is_empty() && w.codex.rage_streak.is_empty());
}
```

- [ ] Run: `cargo test -p anabios-core --lib codex` → **fails** then, after adding fields + registration, **passes**. Also confirm `cargo test -p anabios-core --lib codex::agg::tests::reset_restores_default_state` still passes (no SpeciesAgg field left out of `reset`).
- [ ] Commit: `feat(codex): register affect detectors in observe_all (flag-gated, serialized state)`.

## Task 9 — Flag-ON showcase scenario + integration test

- [ ] **Files:** `scenarios/affect-showcase.toml` (new), `crates/anabios-core/tests/codex_events.rs`
- [ ] **Interfaces:** the scenario sets `affect_enabled = true` at top level (the field is threaded through `Scenario` + `instantiate()` in M-A, mirroring `cognition_enabled` at scenario.rs:55/433). Design it to reliably produce a panic cascade / feeding frenzy: a dense same-species cluster near a rich food patch (drives synchronized high-SEEK → FeedingFrenzy), plus a predator/threat archetype whose approach spikes cluster FEAR (drives PanicCascade). Base it on an existing dense scenario (e.g. `cooperation.toml` / `territories.toml`) so agent archetypes + placement are known-good; only add `affect_enabled = true` and tighten the cluster.
- [ ] Write the failing test first (append to `tests/codex_events.rs`, mirroring `divergent_scenario_emits_speciation_event`):

```rust
const AFFECT_SHOWCASE: &str = include_str!("../../../scenarios/affect-showcase.toml");

#[test]
fn affect_showcase_emits_an_affect_event() {
    let scenario = Scenario::parse_toml(AFFECT_SHOWCASE).expect("parse affect showcase");
    let mut world = scenario.instantiate();
    assert!(world.affect_enabled, "showcase scenario must enable affect");
    for _ in 0..800 {
        step(&mut world);
    }
    let saw = world.codex.events.iter().any(|e| matches!(
        e.event_type,
        EventType::FeedingFrenzy | EventType::PanicCascade
            | EventType::TerritorialRage | EventType::MassGrief,
    ));
    assert!(
        saw,
        "expected an affect event; got {:?}",
        world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
    );
}
```

- [ ] Run: `cargo test -p anabios-core --test codex_events affect_showcase` → iterate on the scenario (cluster density, patch richness, threat timing, tick budget) until it **passes**. Note determinism is unaffected: the scenario is opt-in and absent from all golden scenarios.
- [ ] Commit: `feat(scenarios): affect-showcase (panic cascade + feeding frenzy) + integration test`.

## Task 10 — Viewer FFI: export per-agent arousal

- [ ] **Files:** `crates/anabios-godot/src/lib.rs`
- [ ] **Interfaces:** `#[func] fn alive_arousal(&self) -> PackedFloat32Array` (mirror `alive_energy`, lib.rs:500-510), pushing `anabios_core::affect::arousal(&w.agents.affect[id as usize])` per alive agent in `iter_alive` order (same order as `alive_positions`). Naturally neutral (all `0.0`) when `affect_enabled` is off because the affect column is all-zero. Also add to `agent_detail` (lib.rs:563 area): `d.set("affect_enabled", w.affect_enabled);` and `d.set("arousal", anabios_core::affect::arousal(&w.agents.affect[i]));` (guard the column length like the E12/E13 inspector lines at lib.rs:566-579 do).
- [ ] **Headless verification** (gdext `#[func]`s aren't unit-tested from cargo; determinism is unaffected — this is export-only, read after the tick):
  - `cargo build -p anabios-godot` (the gdext cdylib must compile against the new core symbols).
  - Boot the viewer headless to confirm the binding loads and the scene wires up: `godot --headless --path game res://scenes/main.tscn --quit-after 120` → expect a clean exit (exit code 0), no "Invalid call to function 'alive_arousal'" / nonexistent-method errors in stderr. (Per the project's headless-boot check; the exact scene path is `res://scenes/main.tscn`.)
- [ ] Commit: `feat(godot): export per-agent arousal (alive_arousal + agent_detail)`.

## Task 11 — GDScript BODY_AFFECT tint + legend + codex names/colors

- [ ] **Files:** `game/scripts/overlay_manager.gd`, `game/scripts/main.gd`, `game/scripts/legend_panel.gd`, `game/scripts/codex_panel.gd`
- [ ] **Interfaces / edits:**
  - `overlay_manager.gd` (:12-16): add `const BODY_AFFECT := 4` and bump `const BODY_MAX := 5` so `[C]` cycling (`body_mode = (body_mode + 1) % BODY_MAX`, :76) reaches it.
  - `main.gd` `_body_colors` (:582): add a `overlay.BODY_AFFECT:` branch that reads `var ar: PackedFloat32Array = sim.alive_arousal()` and maps arousal `[0,1]` to a calm→aroused ramp (e.g. `Color(0.55,0.6,0.7).lerp(Color(1.0,0.35,0.25), clampf(ar[i], 0.0, 1.0))`). When `affect_enabled` is off, `ar[i]` is `0.0` for every agent ⇒ the neutral calm color (folds cleanly into the existing tier-body coloring).
  - `legend_panel.gd`: extend `BODY_NAMES` (:9) to `["species", "dialect", "diet", "energy", "arousal"]`; add a `4:` arm to `_rebuild_key` (:59) with an arousal ramp row (`_ramp_row(calm, aroused, "calm", "aroused")`).
  - `codex_panel.gd`: append the 4 new display names to `CHAPTER_NAMES` (:57, after `"LivestockHerd"`) — e.g. `"PanicCascade"`, `"FeedingFrenzy"`, `"TerritorialRage"`, `"MassGrief"` — and 4 matching colors to `CHAPTER_COLORS` (:114). This keeps `CHAPTER_NAMES.size() == CHAPTER_COLORS.size() == event_type_count()` so the `_ready` assert/warning (codex_panel.gd:136-149) stays green.
- [ ] **Headless verification** (GDScript coloring can't be cargo-tested; determinism unaffected — pure render path):
  - `godot --headless --path game res://scenes/main.tscn --quit-after 120` → clean exit, and stderr must NOT contain the `codex: N display event types vs M core EventTypes` warning (proves the name/color arrays match the new `EVENT_TYPE_COUNT`) nor any BODY-mode index error.
  - Manual/optional: with the affect-showcase scenario loaded, cycling `[C]` to arousal tints the dense cluster warm during a cascade — a visual smoke check, not a gate.
- [ ] Commit: `feat(godot): arousal body-tint mode + codex names/colors for affect events`.

## Task 12 — Determinism & save/load hardening pass

- [ ] **Files:** `crates/anabios-core/src/snapshot.rs`, `crates/anabios-core/tests/determinism.rs`, `crates/anabios-core/tests/cognition.rs`, `crates/anabios-core/tests/inventions.rs`
- [ ] **FORMAT_VERSION + changelog:** the new `CodexState` fields grow the serialized `World` layout, so bump `FORMAT_VERSION` (snapshot.rs:102) by **incrementing whatever value is present at execution time — do NOT hardcode.** M-A (→24) and M-D (→25) both bump it before M-F, so M-F lands on **26** (confirm with `git grep -n "FORMAT_VERSION" crates/anabios-core/src/snapshot.rs`). Add a `///` changelog entry above it (use the actual `vNN`):

```
/// vNN: M-F affect observability — CodexState.{frenzy_active, rage_streak,
///      rage_active, fear_count_history, cascade_active, grief_active} +
///      EventType::{PanicCascade, FeedingFrenzy, TerritorialRage, MassGrief}.
///      All detectors are gated on `affect_enabled` (off in every golden
///      scenario), so they never fire there — behavior byte-identical; only
///      the serialized layout grew. (The affect columns themselves were added
///      in M-A.)
```

- [ ] **serde-skip audit (documented, no code change expected):** confirm — and note in the commit — that (a) `affect` and `affect_prev_crowding` on `AgentBuffers` are plain serialized columns (added M-A/M-D, NOT `#[serde(skip)]`); (b) every new `CodexState` field above is serialized (no field-level skip in `CodexState`); (c) the `SpeciesAgg` affect aggregates are the ONLY new non-serialized state, and that is correct — they are per-tick scratch behind `World.codex_agg` `#[serde(skip)]`, rebuilt every tick by `agg.build`, feeding only the (serialized) detector outputs. Any path-dependent accumulator that fed hashed state and was skipped would be the still-ticks/v13 footgun.
- [ ] **Save→load→step round-trip for ALL affect + detector state** — add to `tests/determinism.rs` (model the existing `gene_tech_coupling_survives_save_load_step`, determinism.rs:16-36, and `ambush_and_signal_accumulators_survive_roundtrip`, snapshot.rs:274):

```rust
/// The affect layer's serialized state — the per-agent `affect` activations,
/// `affect_prev_crowding`, and every M-F codex detector latch/streak/history —
/// must survive a snapshot. A restored world stepped one tick must reproduce the
/// continuous run's state hash; any affect column or detector field silently
/// dropped on load would diverge here (the serde-skip replay footgun).
#[test]
fn affect_showcase_survives_save_load_step() {
    const AFFECT: &str = include_str!("../../../scenarios/affect-showcase.toml");
    let mut world = Scenario::parse_toml(AFFECT).expect("parse affect showcase").instantiate();
    assert!(world.affect_enabled, "scenario must enable affect");
    // Warm past the detectors' windows so latches/streaks/history are populated.
    for _ in 0..800 {
        step(&mut world);
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&world), state_hash(&reloaded), "load must restore identical state");
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "affect world diverged after save→load→step (hidden non-serialized affect/detector state?)",
    );
}
```

- [ ] Run: `cargo test -p anabios-core --test determinism affect_showcase_survives_save_load_step` → **passes** (fails loudly if any affect/detector column is `#[serde(skip)]`).
- [ ] **Flag-off byte-identity / golden refresh (layout growth only):** run each golden test under `UPDATE_HASHES=1`, confirm ONLY tick-0 (or the layout-growth ticks) move for a layout reason — a non-layout move = a determinism bug to fix, not to bless — and paste the printed values back with a dated changelog note:
  - `UPDATE_HASHES=1 cargo test -p anabios-core --test determinism minimal_scenario_matches_golden_hashes -- --nocapture` → paste into `GOLDEN` (determinism.rs:162); add note: `// Refreshed 2026-08-02 (M-F affect observability, FORMAT_VERSION →NN): new CodexState affect-detector fields; affect_enabled off in minimal ⇒ detectors never fire — layout growth only.`
  - `UPDATE_HASHES=1 cargo test -p anabios-core --test cognition cognitive_scenario_matches_golden_hashes -- --nocapture` → paste into `COGNITIVE_GOLDEN` (cognition.rs:93) + same-dated note.
  - `UPDATE_HASHES=1 cargo test -p anabios-core --test inventions inventions_scenario_matches_golden_hashes -- --nocapture` → paste into `INVENTIONS_GOLDEN` (inventions.rs:1071) + same-dated note.
- [ ] **Parallelism:** run `cargo test -p anabios-core --test determinism parallel_matches_serial_across_thread_counts` — the affect aggregates are read from the serialized column inside the single-threaded `observe_all` (no new `par_iter`), so this must stay green.
- [ ] Run the full core suite once: `cargo test -p anabios-core` (catches any inline unit-test caller of a changed signature, per the BiomeCell golden-rehash memory).
- [ ] Commit: `test(affect): FORMAT_VERSION bump + golden refresh + affect save/load/step hardening`.

## Self-review notes

- **Spec §7 coverage:** §7.1's four detectors are Tasks 4–7 (FeedingFrenzy = synchronized high-SEEK convergence; TerritorialRage = sustained high-RAGE cluster; PanicCascade = FEAR contagion through a cluster; MassGrief = population PANIC after a die-off), each with a documented firing condition and a unit test as its "done-when" bar. §7.2's viewer surfacing is Tasks 10–11 (per-agent arousal export + `BODY_AFFECT` tint, neutral when the flag is off) with a flag-ON showcase scenario (Task 9). §6 determinism hardening is Task 12 (FORMAT_VERSION bump, serde-skip audit, save→load→step over all affect + detector state, flag-off golden refresh across determinism/cognition/inventions, parallelism check).
- **Type-consistency vs contract:** detectors read `world.agents.affect` (`AffectState = [f32; AFFECT_SYSTEMS]`) and the `SEEK/FEAR/RAGE/PANIC` indices, and the viewer calls `affect::arousal(&AffectState) -> f32` — all exactly as declared in the interface contract; M-F consumes, never renames. `affect_prev_crowding` is referenced only in the round-trip test's coverage claim (it is exercised by stepping the flag-ON scenario), not re-declared.
- **Observer-only / zero tick-RNG:** every detector is invoked solely from `observe_all` (stage 9, after the tick's RNG draws) and reads already-computed serialized state; none draws RNG or mutates agent physics, so enabling the detectors cannot perturb draw order — consistent with the codex's existing observer discipline.
- **Append-only:** the four `EventType`s append at discriminants 53–56 after `LivestockHerd = 52`; `EVENT_TYPE_COUNT` re-derives from the new last variant; no existing discriminant moves.
- **serde-skip footgun:** the only new non-serialized state is the per-tick `SpeciesAgg` affect scratch (correctly `#[serde(skip)]` via `World.codex_agg`, rebuilt every tick and guarded by `reset_restores_default_state`); every field that feeds a hashed detector output is a plain serialized `CodexState` field. Task 12's round-trip test is the executable guard.
- **Placeholder scan:** no `TODO`/`unimplemented!()`/`...` — every task carries real Rust/GDScript and runnable `cargo`/`godot` commands. The one ordering caveat (Task 2's `build` block references Task 3's `HIGH_*` consts) is called out inline with a resolution (land params first or inline literals).
- **Flag-off inertness:** detectors are gated on `world.affect_enabled` in `observe_all` (Task 8) AND are naturally inert on the affect column being all-zero; the viewer tint is neutral for the same reason. Golden refresh is therefore layout-growth-only — a non-layout hash move is explicitly flagged as a bug, not a rubber-stamp.
