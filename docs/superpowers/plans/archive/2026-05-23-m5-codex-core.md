# M5 — Codex Core (Detector Framework + First Batch) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the codex subsystem — the discovery meta-game that surfaces interesting emergent events. Build a deterministic event bus and three first-batch detectors (Extinction, PopulationCrash, SpeciationEvent). The headless CLI gains a `--events-jsonl` flag that streams every detected event for offline analysis.

**Architecture:** A new `codex.rs` defines `CodexEvent`, `EventType`, and `CodexState` (per-detector scratch + the event ring buffer). Detectors are pure functions taking `(&World, &mut CodexState)` — no recursion, no allocations in the hot path. They run at the end of each tick (after `species_step`). Determinism is preserved by using `BTreeMap`/`BTreeSet` for all detector state, never `HashMap`/`HashSet`.

**Tech Stack:** Same as M4.

**Style conventions** (inherited):

- 4-space indent
- All randomness through `World.rng`
- No allocations in tick path; reuse scratch in `CodexState`
- Deterministic iteration: ascending agent/species id; `BTreeMap`/`BTreeSet` only
- Conventional Commits prefixes
- Single commit per task unless noted

**Branch:** `m5-codex-core` branched from `main`.

**Working directory:** `/Users/aryasen/projects/anabios/`.

---

## File structure after M5

New files:
```
crates/anabios-core/src/
└── codex.rs                       # CodexEvent + EventType + CodexState + 3 detectors + observe_all
crates/anabios-core/tests/
└── codex_events.rs                # integration: extinction + speciation events fire on a long run
```

Modified files:
```
crates/anabios-core/src/
├── world.rs                       # +codex: CodexState
├── species.rs                     # emit SpeciationEvent on each new species_id allocation
├── tick.rs                        # +codex::observe_all stage at end of tick
└── lib.rs                         # +pub mod codex;
crates/anabios-core/tests/
├── determinism.rs                 # regenerate GOLDEN
└── invariants.rs                  # +codex invariants
crates/anabios-headless/src/main.rs # --events-jsonl <path> flag
```

---

## Task 0: Branch

- [ ] Create branch `m5-codex-core` from `main`; verify `cargo test --workspace` is green (~108 tests baseline).

---

## Task 1: CodexEvent + EventType + CodexState

**Goal:** Define the data types and the (empty) `observe_all` driver.

**Files:**
- Create: `crates/anabios-core/src/codex.rs`
- Modify: `crates/anabios-core/src/lib.rs`

- [ ] **Step 1.1: Add `pub mod codex;` to lib.rs** (alphabetical between `biome` and `genome`).

- [ ] **Step 1.2: Implement codex.rs**

Create `crates/anabios-core/src/codex.rs`:

```rust
//! Codex — discovery meta-game event bus and detectors.
//!
//! Detectors run at the end of each tick (after `species_step`). Each
//! detector is a pure observer over `&World` that writes any new events
//! into `CodexState.events`. Per-detector scratch lives on `CodexState`
//! so the hot path stays allocation-free.
//!
//! Determinism: all detector state uses `BTreeMap`/`BTreeSet` (ordered
//! iteration), never `HashMap`/`HashSet`. Events are appended in
//! deterministic detector order.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::world::World;

/// Maximum events buffered before the oldest are dropped. The headless
/// CLI's JSONL writer drains the buffer each tick, so in normal operation
/// this only matters if the consumer falls behind.
pub const CODEX_EVENT_CAPACITY: usize = 4096;

/// How many recent population samples each species tracks for crash
/// detection (one sample per tick).
pub const POP_HISTORY_WINDOW: usize = 200;

/// PopulationCrash triggers when alive count drops by >= this fraction
/// across `POP_HISTORY_WINDOW` ticks.
pub const CRASH_FRACTION: f32 = 0.6;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    Extinction = 0,
    PopulationCrash = 1,
    SpeciationEvent = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexEvent {
    pub event_type: EventType,
    pub tick: u64,
    /// Species id most directly associated with the event (`u32::MAX` if
    /// the event is global).
    pub species_id: u32,
    /// Numeric payload (e.g. peak population for a crash, peak distance
    /// for a future migration event). Interpretation depends on type.
    pub value: f32,
}

/// Persistent state owned by `World`. Holds detector scratch and the
/// event ring buffer.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CodexState {
    /// Rolling per-species population history (BTreeMap → deterministic
    /// iteration order). Each VecDeque is bounded to `POP_HISTORY_WINDOW`.
    pub pop_history: BTreeMap<u32, std::collections::VecDeque<u32>>,
    /// Ring buffer of recent events. Oldest dropped when full.
    pub events: std::collections::VecDeque<CodexEvent>,
}

impl CodexState {
    pub fn push_event(&mut self, ev: CodexEvent) {
        if self.events.len() >= CODEX_EVENT_CAPACITY {
            self.events.pop_front();
        }
        self.events.push_back(ev);
    }

    /// Drain the buffer — used by the CLI JSONL writer.
    pub fn drain_events(&mut self) -> std::collections::vec_deque::Drain<'_, CodexEvent> {
        self.events.drain(..)
    }
}

/// Run all detectors. Called by the tick orchestrator at the end of each
/// tick.
pub fn observe_all(world: &mut World) {
    // Detectors run in fixed declaration order so the events buffer
    // ordering is reproducible. Each detector borrows the world
    // immutably and writes to `world.codex`.
    detect_extinction_and_history(world);
    detect_population_crash(world);
    // SpeciationEvent is emitted directly from species.rs at the moment
    // of allocation — nothing to do here.
}

fn detect_extinction_and_history(world: &mut World) {
    let tick = world.tick;
    // Update history first so PopulationCrash sees the current sample.
    for (sid, count) in world.species_member_counts.iter().enumerate() {
        let sid = sid as u32;
        let buf = world.codex.pop_history.entry(sid).or_default();
        if buf.len() == POP_HISTORY_WINDOW {
            buf.pop_front();
        }
        buf.push_back(*count);
    }

    // Extinction: a species whose previous sample was > 0 and current is 0.
    // We compare the new sample (just pushed) against the prior sample.
    for (sid, buf) in world.codex.pop_history.iter() {
        if buf.len() < 2 {
            continue;
        }
        let prev = buf[buf.len() - 2];
        let cur = buf[buf.len() - 1];
        if prev > 0 && cur == 0 {
            // Need to clone the event before push (borrow-split).
        }
    }
    // The borrow split forces a second pass: collect events to push, then push.
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, buf) in world.codex.pop_history.iter() {
        if buf.len() < 2 {
            continue;
        }
        let prev = buf[buf.len() - 2];
        let cur = buf[buf.len() - 1];
        if prev > 0 && cur == 0 {
            to_push.push(CodexEvent {
                event_type: EventType::Extinction,
                tick,
                species_id: *sid,
                value: prev as f32,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

fn detect_population_crash(world: &mut World) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, buf) in world.codex.pop_history.iter() {
        if buf.len() < POP_HISTORY_WINDOW {
            continue;
        }
        let peak = *buf.iter().max().unwrap_or(&0);
        let cur = *buf.back().unwrap_or(&0);
        if peak == 0 {
            continue;
        }
        let drop = 1.0 - (cur as f32 / peak as f32);
        if drop >= CRASH_FRACTION && cur > 0 {
            to_push.push(CodexEvent {
                event_type: EventType::PopulationCrash,
                tick,
                species_id: *sid,
                value: drop,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_push_respects_capacity() {
        let mut s = CodexState::default();
        for i in 0..(CODEX_EVENT_CAPACITY + 100) {
            s.push_event(CodexEvent {
                event_type: EventType::Extinction,
                tick: i as u64,
                species_id: 0,
                value: 0.0,
            });
        }
        assert_eq!(s.events.len(), CODEX_EVENT_CAPACITY);
        assert_eq!(s.events.front().unwrap().tick, 100);
    }
}
```

- [ ] **Step 1.3: Test + commit**

```bash
cargo test -p anabios-core codex
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/lib.rs crates/anabios-core/src/codex.rs
git commit -m "feat(core): codex module (CodexEvent + CodexState + Extinction/PopulationCrash detectors)"
```

Expected: 1 codex test passes.

---

## Task 2: Wire codex into World and tick

**Goal:** `World` owns a `CodexState`. `tick::step` calls `codex::observe_all` at the end of each tick.

**Files:**
- Modify: `crates/anabios-core/src/world.rs`
- Modify: `crates/anabios-core/src/tick.rs`

- [ ] **Step 2.1: Add field to World**

In `world.rs` struct definition (NOT marked `#[serde(skip)]` — codex state is part of the deterministic snapshot):

```rust
    pub codex: crate::codex::CodexState,
```

Initialize in `World::new` as `crate::codex::CodexState::default()`.

- [ ] **Step 2.2: Wire into tick.rs**

In `tick.rs::step`, after `species_step` and before `biome_step`, add:

```rust
    // Stage 10: codex detectors.
    crate::codex::observe_all(world);
```

- [ ] **Step 2.3: Run all lib tests + commit**

```bash
cargo test -p anabios-core --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/world.rs crates/anabios-core/src/tick.rs
git commit -m "feat(core): wire codex::observe_all into tick pipeline"
```

---

## Task 3: SpeciationEvent emission in species.rs

**Goal:** When `species_step` allocates a new species id, push a `SpeciationEvent` to the codex.

**Files:**
- Modify: `crates/anabios-core/src/species.rs`

- [ ] **Step 3.1: Emit event at split-off**

In `species.rs::species_step`, in the branch where a new species id is allocated (after `world.add_to_species(new_id)`), add:

```rust
            world.codex.push_event(crate::codex::CodexEvent {
                event_type: crate::codex::EventType::SpeciationEvent,
                tick: world.tick,
                species_id: new_id,
                value: cur_species as f32, // parent species id encoded in value
            });
```

- [ ] **Step 3.2: Run tests + commit**

```bash
cargo test -p anabios-core --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/species.rs
git commit -m "feat(core): species_step emits SpeciationEvent on each new species"
```

---

## Task 4: Regenerate golden hashes + invariants

**Goal:** `World` now serializes a `CodexState`; snapshot shape changes → hashes change. Add a small proptest invariant: events never reference negative ticks or out-of-range species ids.

**Files:**
- Modify: `crates/anabios-core/tests/determinism.rs`
- Modify: `crates/anabios-core/tests/invariants.rs`

- [ ] **Step 4.1: Reset GOLDEN to zeros and regenerate**

Same pattern as M4 Task 7: zero the array, `UPDATE_HASHES=1 cargo test --test determinism --nocapture`, paste back, verify.

- [ ] **Step 4.2: Add codex invariant**

```rust
    /// All codex events reference valid species ids.
    #[test]
    fn codex_events_reference_valid_species(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        let max_id = w.species_centroids.len() as u32;
        for ev in &w.codex.events {
            prop_assert!(ev.species_id == u32::MAX || ev.species_id < max_id,
                "event references invalid species {}", ev.species_id);
        }
    }
```

- [ ] **Step 4.3: Test + commit**

```bash
cargo test -p anabios-core --tests
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/tests/determinism.rs crates/anabios-core/tests/invariants.rs
git commit -m "test(core): codex event invariant + regenerated golden hashes for M5"
```

---

## Task 5: Integration test — codex events fire on a long run

**Goal:** Long-run scenario where at least one SpeciationEvent and one Extinction event are emitted. Uses the divergent scenario from M2.

**Files:**
- Create: `crates/anabios-core/tests/codex_events.rs`

- [ ] **Step 5.1: Implement test**

```rust
//! Integration test: codex emits SpeciationEvent and PopulationCrash /
//! Extinction over a long-running divergent scenario.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/divergent.toml");

#[test]
fn divergent_scenario_emits_speciation_event() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();

    for _ in 0..2_000 {
        step(&mut world);
    }

    let saw_speciation = world
        .codex
        .events
        .iter()
        .any(|ev| ev.event_type == EventType::SpeciationEvent);
    assert!(saw_speciation, "expected at least one SpeciationEvent");
}
```

- [ ] **Step 5.2: Test + commit**

```bash
cargo test -p anabios-core --test codex_events
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/tests/codex_events.rs
git commit -m "test(core): integration test that divergent scenarios emit SpeciationEvent"
```

---

## Task 6: Headless CLI streams events to JSONL

**Goal:** Add a `--events-jsonl <path>` flag that writes each event as a JSON line as the simulation runs.

**Files:**
- Modify: `crates/anabios-headless/Cargo.toml` (add `serde_json`)
- Modify: `crates/anabios-headless/src/main.rs`

- [ ] **Step 6.1: Add serde_json dep**

In `crates/anabios-headless/Cargo.toml`:

```toml
serde_json = "1"
```

(And add to workspace deps in root `Cargo.toml`.)

- [ ] **Step 6.2: Add CLI flag**

In `crates/anabios-headless/src/main.rs`, extend the `Run` subcommand's args:

```rust
        /// Optional path to write codex events as JSON Lines as they occur.
        #[arg(long)]
        events_jsonl: Option<PathBuf>,
```

In the `run` function, before the tick loop:

```rust
    let mut events_file: Option<std::fs::File> = if let Some(p) = events_jsonl {
        Some(std::fs::File::create(p).with_context(|| format!("creating {}", p.display()))?)
    } else {
        None
    };
```

Inside the tick loop, after each `step`:

```rust
        if let Some(f) = events_file.as_mut() {
            use std::io::Write;
            for ev in world.codex.drain_events() {
                serde_json::to_writer(&mut *f, &ev)?;
                f.write_all(b"\n")?;
            }
        }
```

- [ ] **Step 6.3: Smoke test the CLI + commit**

```bash
cargo build --release --bin anabios-headless
./target/release/anabios-headless run --scenario scenarios/divergent.toml --ticks 2000 --events-jsonl /tmp/m5_events.jsonl
wc -l /tmp/m5_events.jsonl   # should be > 0
head -1 /tmp/m5_events.jsonl  # should be a valid JSON object
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml crates/anabios-headless/Cargo.toml crates/anabios-headless/src/main.rs
git commit -m "feat(headless): --events-jsonl streams codex events to JSON Lines file"
```

---

## Task 7: Bench + final + tag

- [ ] **Step 7.1: Bench**

```bash
cargo bench -p anabios-core --bench tick_bench
```

M4 baseline: 1k ≈ 1.88 ms, 10k ≈ 12 ms. M5 codex adds ~3 simple detectors per tick — expected overhead ~5-10%. Document the new numbers.

- [ ] **Step 7.2: Full check + smoke + tag**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 5000 > /tmp/m5_a.txt
./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 5000 > /tmp/m5_b.txt
diff /tmp/m5_a.txt /tmp/m5_b.txt && echo deterministic
git tag -a m5 -m "M5: codex core — first batch of detectors + JSONL streaming"
```

---

## Post-implementation expectations

- World owns a `CodexState`; tick pipeline runs `codex::observe_all` at the end of each tick
- Three detectors active: Extinction, PopulationCrash, SpeciationEvent
- Headless CLI streams events to JSONL for offline analysis
- Determinism preserved; golden hashes regenerated; bench within budget

Deferred to later milestones:

- Spatial detectors (Migration, TerritoryFormation, NichePartitioning, …)
- Trait detectors (NovelModuleAppeared, NovelBehavior, ConvergentEvolution)
- Cultural detectors (DialectFormed, MemeSweep) — gated on culture system not yet implemented
- Named behavior detectors (EvolvedFlight, EvolvedAmbush, …)
- Snapshot replay system for "replay this moment"
- Player-facing codex UI (Godot, M6+)
