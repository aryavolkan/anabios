# M8 — Codex UI + More Detectors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Grow the codex from 3 detectors to a richer set, and surface it in the Godot viewer as a proper panel (event counts by chapter + a recent-events list you can click to focus the camera). Adds three new detectors — `Migration`, `NovelModuleAppeared`, `NovelBehaviorPattern` — and a codex panel that groups events into chapters with running totals.

**Architecture:** New detectors extend the M5 `codex.rs` framework (pure observers over `&mut World`, BTreeMap scratch). `Migration` tracks per-species centroid history; `NovelModuleAppeared` and `NovelBehaviorPattern` track per-species "first seen" sets. The Godot side adds a `codex_panel.gd` that accumulates events into per-chapter counts and a clickable recent list; clicking an event with a location centers the camera there (a lightweight "jump to event" — full deterministic snapshot replay is deferred).

**Tech Stack:** Same as M7.

**Branch:** `m8-codex-ui-detectors` from `main`.

**Working directory:** `/Users/aryasen/projects/anabios/`.

**Scope note (medium effort):** Three new detectors + a codex panel + jump-to-event camera focus. Deferred: deterministic snapshot replay ("rewind and replay this moment"), the full cross-world codex DB, named-behavior signature detectors (EvolvedFlight/Ambush — those need richer behavior analysis), and codex card export.

---

## File structure after M8

Modified:
```
crates/anabios-core/src/codex.rs   # +Migration, NovelModuleAppeared, NovelBehaviorPattern + event location
crates/anabios-godot/src/lib.rs    # codex events expose location (x,y)
crates/anabios-core/tests/determinism.rs  # regenerate GOLDEN
crates/anabios-core/tests/invariants.rs   # detector invariants
game/scripts/event_ticker.gd       # (kept; or fold into panel)
game/scenes/main.tscn              # +CodexPanel
```
New:
```
game/scripts/codex_panel.gd        # chapter counts + clickable recent list + jump-to-event
crates/anabios-core/tests/codex_detectors.rs  # unit tests for the 3 new detectors
```

---

## Task 0: Branch

- [ ] `git checkout main && git pull && git checkout -b m8-codex-ui-detectors`
- [ ] `cargo test --workspace 2>&1 | tail -3` — baseline green.

---

## Task 1: Add event location + EventType variants

**Goal:** Extend `CodexEvent` with a world location (so the UI can jump there) and add three new `EventType` variants.

**Files:** Modify `crates/anabios-core/src/codex.rs`

- [ ] **Step 1.1: Extend EventType + CodexEvent**

```rust
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    Extinction = 0,
    PopulationCrash = 1,
    SpeciationEvent = 2,
    Migration = 3,
    NovelModuleAppeared = 4,
    NovelBehaviorPattern = 5,
}
```

Add `loc_x: f32, loc_y: f32` to `CodexEvent` (default 0.0 for events without a natural location). Update the existing `push_event` call sites (extinction/crash/speciation) to set `loc_x/loc_y` to the species centroid where available, else 0.0.

- [ ] **Step 1.2: Add per-species centroid + first-seen scratch to CodexState**

```rust
    /// Per-species centroid position history for migration detection.
    pub centroid_history: BTreeMap<u32, VecDeque<(f32, f32)>>,
    /// Per-species set of module-type discriminants already observed.
    pub seen_modules: BTreeMap<u32, std::collections::BTreeSet<u8>>,
    /// Per-species set of program-node-kind discriminants already observed.
    pub seen_node_kinds: BTreeMap<u32, std::collections::BTreeSet<u8>>,
```

(Document the migration window + threshold constants: `MIGRATION_WINDOW = 200`, `MIGRATION_DISTANCE = 150.0`.)

- [ ] **Step 1.3: Build + commit**

```bash
cargo build -p anabios-core
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/codex.rs
git commit -m "feat(core): codex event location + Migration/NovelModule/NovelBehavior event types"
```

---

## Task 2: Migration detector

**Goal:** Detect when a species' population centroid moves more than `MIGRATION_DISTANCE` world units over `MIGRATION_WINDOW` ticks.

**Files:** Modify `crates/anabios-core/src/codex.rs`

- [ ] **Step 2.1: Compute per-species centroid each observe, push history, detect**

In `observe_all`, add a `detect_migration` pass that:
1. Computes each species' current centroid (mean alive position, ascending id order, f64 accumulator for determinism).
2. Pushes into `centroid_history` (bounded to `MIGRATION_WINDOW`).
3. If the buffer is full and `torus_distance(first, last) >= MIGRATION_DISTANCE`, emits a `Migration` event located at the current centroid, then clears that species' history (so it doesn't re-fire every tick).

Use the existing `crate::spatial::torus_distance`.

- [ ] **Step 2.2: Unit test + commit**

Create `crates/anabios-core/tests/codex_detectors.rs` with a test that hand-builds a world, marches a single-species population steadily across the world, and asserts a Migration event fires. fmt + clippy + commit.

```bash
git add crates/anabios-core/src/codex.rs crates/anabios-core/tests/codex_detectors.rs
git commit -m "feat(core): Migration detector (species centroid drift)"
```

---

## Task 3: NovelModuleAppeared + NovelBehaviorPattern detectors

**Goal:** Emit an event the first time a species exhibits a module type or program node kind it hasn't shown before.

**Files:** Modify `crates/anabios-core/src/codex.rs`, `crates/anabios-core/tests/codex_detectors.rs`

- [ ] **Step 3.1: Detect novel modules**

For each alive agent, for each module, insert `module_type as u8` into `seen_modules[species_id]`. If the insert is new (returns true) AND the species has been observed before (not the very first tick of that species), emit `NovelModuleAppeared` with `value` = the module type discriminant, located at the agent's position.

To avoid a flood at world start, seed `seen_modules` with the founder kit's types on the first observation of each species (i.e., only emit for genuinely new types after the species' debut tick). Simplest deterministic rule: on a species' first appearance, record all its current module types silently; on subsequent ticks, emit for any newly-appearing type.

- [ ] **Step 3.2: Detect novel program node kinds**

Same pattern with `seen_node_kinds[species_id]`, keyed by a `node_kind(node) -> u8` discriminant (add a small helper in `program.rs` returning a stable kind per `Node` variant). Emit `NovelBehaviorPattern` with `value` = node kind.

- [ ] **Step 3.3: Tests + commit**

Add unit tests: a world seeded with the starter kit should NOT emit novel-module events on tick 1 (all founder types pre-seeded); injecting an agent with a Weapon module into an established species should emit one. fmt + clippy + commit.

```bash
git add crates/anabios-core/src/codex.rs crates/anabios-core/src/program.rs crates/anabios-core/tests/codex_detectors.rs
git commit -m "feat(core): NovelModuleAppeared + NovelBehaviorPattern detectors"
```

---

## Task 4: Regenerate golden hashes + invariants

**Files:** Modify `crates/anabios-core/tests/determinism.rs`, `crates/anabios-core/tests/invariants.rs`

- [ ] **Step 4.1:** Reset GOLDEN to zeros, `UPDATE_HASHES=1` regenerate, paste back, verify.
- [ ] **Step 4.2:** Extend the existing `codex_events_reference_valid_species` invariant to cover the new event types (no change likely needed — they all set species_id). Add an invariant that event `loc_x/loc_y` are finite and within `[0, WORLD_SIZE]` or exactly 0.0.
- [ ] **Step 4.3:** `cargo test --workspace`; fmt + clippy; commit.

```bash
git commit -m "test(core): regenerate golden hashes + codex location invariant for M8 detectors"
```

---

## Task 5: gdext — expose event location

**Files:** Modify `crates/anabios-godot/src/lib.rs`

- [ ] **Step 5.1:** In `take_codex_events`, add `d.set("loc", Vector2::new(ev.loc_x, ev.loc_y))` to each event dict.
- [ ] **Step 5.2:** Build + fmt + clippy + commit.

```bash
git commit -m "feat(godot): codex event dict includes world location"
```

---

## Task 6: Godot codex panel

**Goal:** A panel showing per-chapter event counts and a clickable recent-events list; clicking an event centers the camera on its location.

**Files:**
- Create: `game/scripts/codex_panel.gd`
- Modify: `game/scenes/main.tscn` (add the panel; optionally remove the old ticker or keep both)

- [ ] **Step 6.1: codex_panel.gd**

Maintains an int array of per-type counts (6 chapters) and a bounded list of recent events `{type, tick, species, loc}`. Each `_process`, drains `sim.take_codex_events()`, updates counts + recent list, and renders:
- A header line per chapter: `Extinction: N   PopCrash: N   Speciation: N ...`
- A scrolling recent list (last ~30) as clickable `Button`s or `RichTextLabel` lines with metadata.

On click of a recent event, set the Camera2D's `position` to the event's `loc` (jump-to-event). Wire to `get_node("/root/Main/Camera2D")`.

- [ ] **Step 6.2: Add CodexPanel to main.tscn**

Place a `PanelContainer` named `CodexPanel` on the right side, below the inspector. Replace the bottom `EventTicker` or keep it — recommend replacing the ticker with the panel for a cleaner UI.

- [ ] **Step 6.3: Smoke test + commit**

```bash
cargo build -p anabios-godot
godot --headless --quit --path game/ 2>&1 | tail -10
git add game/scripts/codex_panel.gd game/scenes/main.tscn
git commit -m "feat(game): codex panel with chapter counts + jump-to-event"
```

---

## Task 7: Smoke test + tag

- [ ] **Step 7.1:** Headless project check (no errors).
- [ ] **Step 7.2:** Interactive smoke test (local): run a divergent scenario, confirm the codex panel accumulates counts across all 6 chapters and clicking an event jumps the camera.
- [ ] **Step 7.3:** `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`; tag `m8`.

```bash
git tag -a m8 -m "M8: codex UI + Migration/NovelModule/NovelBehavior detectors"
```

---

## Post-implementation expectations

- Codex has 6 detectors (Extinction, PopulationCrash, SpeciationEvent, Migration, NovelModuleAppeared, NovelBehaviorPattern)
- Events carry a world location
- The viewer shows per-chapter counts + a clickable recent list that jumps the camera
- Determinism preserved; golden hashes regenerated

Deferred to M9+:
- Deterministic snapshot replay ("rewind to this moment and play forward")
- Cross-world persistent codex DB (SQLite per design §6.5)
- Named-behavior signature detectors (EvolvedFlight, EvolvedAmbush, EvolvedCooperation)
- Codex card image export
- Cultural detectors (gated on the culture/meme system, not yet built)
