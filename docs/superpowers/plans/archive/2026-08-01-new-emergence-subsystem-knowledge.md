# New Emergence Subsystem — Knowledge Accumulation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in **knowledge-accumulation** subsystem where Writing-holding cultures build durable, transmissible tech memory that survives population bottlenecks — turning Writing from a one-off tech into a ratchet — with its own codex detector.

**Architecture:** Mirror the domestication (E13) opt-in pattern exactly: a `knowledge_enabled` scenario flag, a self-gated `knowledge_step` tick stage, per-species accumulated-knowledge state in `CodexState`, and a `KnowledgeRatchet` codex event. Knowledge is a per-species float that rises while any member holds Writing and decays slowly otherwise; when a species that lost its inventions re-acquires one faster because of retained knowledge, the ratchet fires. This is the concrete pick from the roadmap's "one new subsystem" slot; the same skeleton applies if the Phase-2 scorecard instead points at disease or migration.

**Tech Stack:** Rust (`anabios-core`: `scenario.rs`, `world.rs`, `lib.rs`, `tick.rs`, new `src/knowledge.rs`, `codex/event.rs`, `codex/mod.rs`, new `src/codex/knowledge.rs`), new `scenarios/knowledge-ratchet.toml`.

## Global Constraints

- **Opt-in, off by default:** flag off ⇒ world byte-identical, zero RNG drawn (`if !world.knowledge_enabled { return; }` first line of the step). Goldens must not move.
- **Determinism:** all subsystem maps are `BTreeMap`/`BTreeSet` (codex determinism rule, `codex/mod.rs:8-10`); no `HashMap`. RNG only via `world.rng`.
- **Snapshot:** new persistent state ⇒ bump `FORMAT_VERSION` (`snapshot.rs:102`→24) with a changelog line; new fields serialize (no `#[serde(skip)]` unless a re-derivable cache) so `state_hash` covers them.
- **Codex enum:** append the new variant at the end (next discriminant = 53), update `EVENT_TYPE_COUNT` (auto-derived, `event.rs:225`), and add matching Godot name/color entries + the headless `score.rs` name so `ALL_EVENT_NAMES`/`EVENT_TYPE_COUNT` stay length-synced (asserts at `score.rs:359-368`).
- Effective dependency: knowledge needs `inventions_enabled` (Writing must exist). Enforce with an explicit `parse_toml` validation like `ScenarioError::InventionsDisabled` (`scenario.rs:394-398`).

## Background (grounded — the pattern to mirror)

- Flag: `#[serde(default)] pub domestication_enabled: bool` on `Scenario` (`scenario.rs:106-111`) + `World` (`world.rs:160-165`); copied in `instantiate` (`scenario.rs:446-447`).
- Tick stage: `crate::domestication::husbandry_step(world);` at `tick.rs:83-85` (stage 6e, after invention_step).
- Module: `pub mod domestication;` in `lib.rs:16`; self-gate at `domestication.rs:71-74`.
- State: `CodexState.{domesticated_species: BTreeSet, livestock_herd_streak: BTreeMap, livestock_herd_active: BTreeSet}` (`codex/mod.rs:267-271`).
- Detector: `src/codex/domestication.rs` `pub(super) fn detect_livestock_herd(world, agg)`; `mod domestication;` at `codex/mod.rs:28`; registered in `observe_all` at `codex/mod.rs:355`.
- Event: `AnimalDomesticated = 51`, `LivestockHerd = 52` (`event.rs:214-219`); direct-push from tick stage (`domestication.rs:154-164`) vs detector-latched (`edge_trigger_species`).
- Invention held-mask: `crate::invention::held_mask(&meme_vector[i])` & `bit(WRITING)` (WRITING id = 4, `invention/mod.rs:50`).
- Tests: inline `flag_off_is_inert` state-hash test (`domestication.rs:336`) + integration `tests/domestication.rs` (flag-on emergence floor + flag-off no-op + round-trip).

## File Structure

- `crates/anabios-core/src/knowledge.rs` — **Create**: `knowledge_step`, tuning consts, inline tests.
- `crates/anabios-core/src/codex/knowledge.rs` — **Create**: `detect_knowledge_ratchet`.
- `crates/anabios-core/src/lib.rs` — **Modify**: `pub mod knowledge;`.
- `crates/anabios-core/src/tick.rs` — **Modify**: call `knowledge::knowledge_step` as a new stage.
- `crates/anabios-core/src/scenario.rs` — **Modify**: `knowledge_enabled` flag + validation + wiring.
- `crates/anabios-core/src/world.rs` — **Modify**: flag field.
- `crates/anabios-core/src/codex/mod.rs` — **Modify**: `CodexState.knowledge_by_species: BTreeMap<u32,f32>` + latch set; `mod knowledge;`; register in `observe_all`.
- `crates/anabios-core/src/codex/event.rs` — **Modify**: `KnowledgeRatchet = 53`.
- `crates/anabios-core/src/snapshot.rs` — **Modify**: `FORMAT_VERSION` bump + changelog.
- `crates/anabios-headless/src/score.rs` — **Modify**: add `"knowledge_ratchet"` to `ALL_EVENT_NAMES` + `event_name` + `DEFAULT_CORPUS_NT` (n_t=0).
- `game/scripts/codex_panel.gd` — **Modify**: add the CamelCase chapter name + color so the viewer's boot assert passes.
- `scenarios/knowledge-ratchet.toml` — **Create**.
- `crates/anabios-core/tests/knowledge.rs` — **Create**.

---

## Task 1: Scenario flag + validation + wiring (goldens unchanged)

**Files:** Modify `scenario.rs`, `world.rs`, `snapshot.rs`; Test `crates/anabios-core/tests/knowledge.rs`.

**Interfaces:** Produces `World.knowledge_enabled: bool` (default false), TOML key `knowledge_enabled`; `ScenarioError::KnowledgeNeedsInventions` when set without `inventions_enabled`.

- [ ] **Step 1: Failing test** — parse+wire and the validation error:

```rust
#[test]
fn knowledge_flag_requires_inventions() {
    let bad = "world_size=64\nknowledge_enabled=true\n[[agents]]\narchetype=\"grazer\"\ncount=4\n";
    assert!(Scenario::parse_toml(bad).is_err());
    let ok = "world_size=64\ninventions_enabled=true\nknowledge_enabled=true\n[[agents]]\narchetype=\"grazer\"\ncount=4\n";
    let w = Scenario::parse_toml(ok).unwrap().instantiate();
    assert!(w.knowledge_enabled);
}
```

- [ ] **Step 2: Run — expect FAIL.** `cargo test -p anabios-core --test knowledge knowledge_flag_requires_inventions`
- [ ] **Step 3: Implement** — add `#[serde(default)] pub knowledge_enabled: bool` to `Scenario` (near `:106`) and `World` (near `:160`, default false in ctor); in `parse_toml` add the validation (mirror `scenario.rs:394-398`); in `instantiate` add `w.knowledge_enabled = self.knowledge_enabled;`. Bump `FORMAT_VERSION` to 24 + changelog line in `snapshot.rs`.
- [ ] **Step 4: Run — expect PASS; goldens unchanged** (`cargo test -p anabios-core --test determinism`).
- [ ] **Step 5: Commit.** `git commit -m "feat(knowledge): opt-in flag gated on inventions_enabled"`

---

## Task 2: `KnowledgeRatchet` event + CodexState

**Files:** Modify `codex/event.rs`, `codex/mod.rs`, `score.rs`, `game/scripts/codex_panel.gd`.

**Interfaces:** Produces `EventType::KnowledgeRatchet = 53`; `CodexState.knowledge_by_species: BTreeMap<u32,f32>` and `knowledge_ratchet_fired: BTreeSet<u32>`.

- [ ] **Step 1: Add the enum variant** at the end of `EventType` (`event.rs:220`): `KnowledgeRatchet = 53,` with a doc comment. `EVENT_TYPE_COUNT` auto-updates.
- [ ] **Step 2: Add state** to `CodexState` (`codex/mod.rs`, near `:267`): `pub knowledge_by_species: BTreeMap<u32, f32>,` and `pub knowledge_ratchet_fired: BTreeSet<u32>,`.
- [ ] **Step 3: Sync the scorer** — add `"knowledge_ratchet"` to `ALL_EVENT_NAMES` (`score.rs:42-96`), the `event_name` match (`score.rs:161-217`), and a `("knowledge_ratchet", 0)` entry to `DEFAULT_CORPUS_NT` (`score.rs:105-159`). Run the sync asserts: `cargo test -p anabios-headless --lib score`.
- [ ] **Step 4: Sync the viewer** — add the CamelCase name + a color to `game/scripts/codex_panel.gd:4-58` arrays so the boot assert (event.rs:222-224 note) passes. Verify headless scene load: build `anabios-godot`, load `main.tscn` `--quit-after 120`.
- [ ] **Step 5: Commit.** `git commit -m "feat(codex): add KnowledgeRatchet event type (53)"`

---

## Task 3: `knowledge_step` accumulation logic

**Files:** Create `src/knowledge.rs`; Modify `lib.rs`, `tick.rs`.

**Interfaces:** Produces `pub fn knowledge_step(world: &mut World)` — per species: if any alive member holds Writing, `knowledge_by_species[sp] += KNOWLEDGE_GAIN` (capped at `KNOWLEDGE_MAX`); else `*= (1 - KNOWLEDGE_DECAY)`. RNG-free. Retained knowledge boosts re-discovery by feeding a small skill bonus into the invention step (integration point documented, not necessarily wired in v1).

- [ ] **Step 1: Failing inline test** (`flag_off_is_inert`, copy `domestication.rs:336`): assert `state_hash` unchanged across `knowledge_step` when flag off; and that with the flag on + a Writing-holder seeded, `knowledge_by_species` grows.
- [ ] **Step 2: Run — expect FAIL.**
- [ ] **Step 3: Implement** `knowledge.rs`:

```rust
use crate::world::World;

pub const KNOWLEDGE_GAIN: f32 = 0.002;
pub const KNOWLEDGE_DECAY: f32 = 0.0005;
pub const KNOWLEDGE_MAX: f32 = 1.0;

/// Per-species durable tech memory. Rises while any member holds Writing,
/// decays slowly otherwise. Off by default; RNG-free.
pub fn knowledge_step(world: &mut World) {
    if !world.knowledge_enabled {
        return;
    }
    // Build the set of species with a live Writing-holder (BTreeSet for determinism).
    let writing = crate::invention::bit(crate::invention::WRITING);
    let mut has_writer: std::collections::BTreeSet<u32> = Default::default();
    for i in 0..world.agents.len() {
        if !world.agents.is_alive(i) { continue; }
        if crate::invention::held_mask(&world.agents.meme_vector[i]) & writing != 0 {
            has_writer.insert(world.agents.species_id[i]);
        }
    }
    let k = &mut world.codex.knowledge_by_species;
    for (sp, v) in k.iter_mut() {
        if has_writer.contains(sp) {
            *v = (*v + KNOWLEDGE_GAIN).min(KNOWLEDGE_MAX);
        } else {
            *v *= 1.0 - KNOWLEDGE_DECAY;
        }
    }
    for sp in has_writer {
        k.entry(sp).or_insert(0.0);
    }
}
```

Declare `pub mod knowledge;` in `lib.rs`; call `crate::knowledge::knowledge_step(world);` in `tick.rs` as a new stage after `husbandry_step` (`tick.rs:85`).

- [ ] **Step 4: Run — expect PASS.**
- [ ] **Step 5: Commit.** `git commit -m "feat(knowledge): per-species knowledge accumulation tick stage"`

---

## Task 4: `detect_knowledge_ratchet` + integration scenario

**Files:** Create `src/codex/knowledge.rs`, `scenarios/knowledge-ratchet.toml`, `tests/knowledge.rs`; Modify `codex/mod.rs`.

**Interfaces:** Produces `pub(super) fn detect_knowledge_ratchet(world: &mut World, agg: &SpeciesAggTable)` — fires `KnowledgeRatchet` (once per species, latched via `knowledge_ratchet_fired`) when a species with `knowledge_by_species[sp] >= KNOWLEDGE_RATCHET_MIN` re-acquires an invention it had lost.

- [ ] **Step 1: Write the detector** mirroring `codex/domestication.rs` structure (self-gate `if !world.knowledge_enabled { return; }`, use `centroid_of(agg, sid)` for loc, latch via the `BTreeSet`). Register: `mod knowledge;` in `codex/mod.rs:28` block, call `knowledge::detect_knowledge_ratchet(world, &agg);` in `observe_all` (`:355` block).
- [ ] **Step 2: Create `scenarios/knowledge-ratchet.toml`** — `inventions_enabled = true`, `knowledge_enabled = true`, two populations seeded near Writing (via `starting_inventions = ["stone_tools","fire","farming"]`), a bottleneck-inducing disaster (`disasters_enabled = true`) so a culture loses then regains tech.
- [ ] **Step 3: Write integration tests** in `tests/knowledge.rs` (mirror `tests/domestication.rs`): flag-on emergence (`#[cfg_attr(debug_assertions, ignore)]`, N seeds × M ticks, floor `knowledge_ratchet >= 1`); flag-off no-op (load `inventions.toml`, assert zero ratchet events); `state_hash` round-trip (save→load→step). Add the round-trip test to the determinism-hardening plan's coverage table too.
- [ ] **Step 4: Run** `cargo test -p anabios-core --test knowledge` and `--release` for the ignored emergence test.
- [ ] **Step 5: Commit.** `git commit -m "feat(codex): KnowledgeRatchet detector + knowledge-ratchet scenario"`

---

## Testing Plan (summary)

| Level | What | Where |
|---|---|---|
| Unit | `flag_off_is_inert` state-hash; accumulation grows | `src/knowledge.rs` inline |
| Unit | scorer name/enum length sync | `score.rs:359-368` |
| Integration | ratchet emerges across seeds (release) | `tests/knowledge.rs` |
| Integration | flag-off no-op | `tests/knowledge.rs` |
| Determinism | save→load→step identity; goldens unchanged (flag off) | `tests/knowledge.rs` + `determinism.rs` |
| Viewer | headless scene load with new codex entry | CI `godot` job |
| Sweep | `knowledge_ratchet` appears as a novel event type | scorecard-sweeps runbook |

**Done when:** the flag is off-by-default and byte-identical when off, `knowledge-ratchet.toml` fires `KnowledgeRatchet` across seeds, the round-trip test passes, the viewer loads with the new event, and a scorecard sweep surfaces `knowledge_ratchet` as a novel type.

## Risks / notes

- If the Phase-2 scorecard evidence points at **disease** or **migration** instead of knowledge, reuse this exact task skeleton — only the `*_step` logic, the event name, and the scenario change. The plan structure (flag → event → step → detector → tests) is subsystem-agnostic.
- Wiring retained knowledge back into faster re-discovery (a real ratchet, not just a counter) touches `invention_step`'s skill term (`invention/mod.rs:670-693`) — that changes a default-adjacent path only when `knowledge_enabled`, so keep it gated and add its own golden scenario.
