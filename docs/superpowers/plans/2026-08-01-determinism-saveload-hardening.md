# Determinism & Save/Load Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Guarantee every opt-in subsystem survives a save→load→step round-trip bit-identically, and audit `#[serde(skip)]` fields so no path-dependent accumulator can silently break replay.

**Architecture:** `state_hash` (FNV-1a over the bincode-serialized `World`) hashes exactly the *serialized* fields — anything `#[serde(skip)]` is invisible to the hash yet can change future ticks (the v13 `still_ticks` footgun). Two workstreams: (1) add one save→load→step test per opt-in flag, copying the domestication template; (2) a documented audit + a guard test asserting each skipped cache is either pure scratch or re-derived in `load_from_bytes`.

**Tech Stack:** Rust (`anabios-core`: `snapshot.rs`, `world.rs`, `tests/*`), bincode, FNV-1a.

## Global Constraints

- Round-trip tests must **warm up** the world enough that the subsystem's state is non-trivial before saving (invention discovery, taming, etc. take hundreds of ticks).
- Each test must guard the flag: `assert!(world.<flag>, "scenario must enable <flag>")` so a scenario silently dropping the flag fails loudly (pattern from `determinism.rs:20`).
- No new golden hashes required — these tests assert *self-consistency* (`state_hash(world) == state_hash(reloaded)`), not pinned values.
- `FORMAT_VERSION` (`snapshot.rs:102`, currently 23) only bumps if the audit forces a field to become serialized (like the v13 fix).

## Background (grounded)

- Save/load — `snapshot.rs:118-135`; `Envelope { format_version, payload }`; two load-time re-derivations: `pheromones.refresh_nonzero()` and `agents.track_livestock = domestication_enabled`.
- `state_hash` — `snapshot.rs:137-154` (FNV-1a over `bincode::serialize(world)`).
- Golden test + `UPDATE_HASHES=1` — `determinism.rs:211-248`; GOLDEN table `:149-162`.
- Existing round-trip tests (the template): `gene_tech_coupling_survives_save_load_step` (`determinism.rs:11-36`), `dimorphism_state_survives_save_load_step` (`dimorphism.rs:59-75`), `livestock_state_survives_save_load_step` (`domestication.rs:65-88`), plus in-module `loaded_world_continues_bit_identically` and `ambush_and_signal_accumulators_survive_roundtrip` (`snapshot.rs:242-304`).
- **Coverage gap:** only 3 of 18 opt-in flags have round-trip tests. Missing (flag → enabling scenario): `env_period`→dit-env-slow, `biome_adaptation`→biome-adaptation, `terrain_habitat`→geographic-trade, `inventions_enabled`→inventions, `gene_requirements`→gene-requirements, `cognition_enabled`→cognitive-coevolution, `living_biome`→living-sandbox-coevolution, `season_period`→sandbox-large, `climate_drift_rate`→drifting-climate, `nutrient_variation`/`soil_fertility`→foraging-selection, `resources_enabled`→biome-trade, `disasters_enabled`→disturbance, `war_enabled`→war, `settlement_enabled`→settlement.
- Current `#[serde(skip)]` inventory (all currently safe, but the audit must lock it): `world.rs` scratch/spatial/viewer fields (`codex_interval`, `spatial`, `sensors`, `desired_direction`, `actions`, `codex_agg`, `combat_*`, `combat_streaks`, `trade_routes`, `total_trades`), `agent.rs` `scratch_ids`/`track_livestock`, `sense.rs` `SensorRegister`, `pheromone.rs` `nonzero`, `codex/agg.rs` `SpeciesAggTable`. `CodexState` has **zero** skips.

## File Structure

- `crates/anabios-core/tests/save_load_roundtrip.rs` — **Create**: one round-trip test per opt-in flag, table-driven.
- `crates/anabios-core/tests/serde_skip_audit.rs` — **Create**: a guard test that documents/enforces the skip inventory (see Task 3).
- `docs/determinism-contract.md` — **Create**: the skip rules + "adding a subsystem" checklist.

---

## Task 1: Round-trip test harness + first three gap subsystems

**Files:**
- Create: `crates/anabios-core/tests/save_load_roundtrip.rs`

**Interfaces:**
- Consumes: `Scenario::parse_toml`, `instantiate`, `step`, `save_to_bytes`, `load_from_bytes`, `state_hash` (all existing).
- Produces: a reusable `fn roundtrip(scenario_src: &str, warm: u64, flag: fn(&World) -> bool)` helper.

- [ ] **Step 1: Write the helper + first failing test** (for `resources_enabled` via biome-trade):

```rust
use anabios_core::{scenario::Scenario, snapshot::{save_to_bytes, load_from_bytes, state_hash}, tick::step, world::World};

fn roundtrip(src: &str, warm: u64, flag: fn(&World) -> bool) {
    let mut world = Scenario::parse_toml(src).expect("parse").instantiate();
    assert!(flag(&world), "scenario must enable the subsystem flag");
    for _ in 0..warm { step(&mut world); }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&world), state_hash(&reloaded), "load must restore identical state");
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(state_hash(&world), state_hash(&reloaded),
        "diverged after save→load→step — hidden non-serialized state feeding the sim?");
}

#[test]
fn resources_roundtrip() {
    roundtrip(include_str!("../../../scenarios/biome-trade.toml"), 400, |w| w.resources_enabled);
}
```

- [ ] **Step 2: Run — expect PASS or FAIL.** Run: `cargo test -p anabios-core --test save_load_roundtrip resources_roundtrip`. If it FAILS, you've found a real footgun — go to Task 3, fix the skipped field (serialize it or re-derive on load), then this passes. If it PASSES, the subsystem is clean; keep the test as a regression guard.
- [ ] **Step 3: Add two more** (`war_enabled`→war.toml @ 600 ticks; `settlement_enabled`→settlement.toml @ 800 ticks), same pattern.
- [ ] **Step 4: Run all three.** Run: `cargo test -p anabios-core --test save_load_roundtrip`.
- [ ] **Step 5: Commit.**

```bash
git add crates/anabios-core/tests/save_load_roundtrip.rs
git commit -m "test(determinism): save→load→step round-trip for resources/war/settlement"
```

---

## Task 2: Round-trip coverage for the remaining opt-in flags

Add one test per remaining flag from the coverage-gap list. Group them so a reviewer can approve the batch; each is ~5 lines given the Task-1 helper.

**Files:**
- Modify: `crates/anabios-core/tests/save_load_roundtrip.rs`

- [ ] **Step 1: Add tests** for: `env_period` (dit-env-slow, 300t), `biome_adaptation` (biome-adaptation, 400t), `terrain_habitat` (geographic-trade, 400t), `inventions_enabled` (inventions, 500t), `gene_requirements` (gene-requirements, 500t), `cognition_enabled` (cognitive-coevolution, 400t), `living_biome` (living-sandbox-coevolution, 400t), `season_period` (sandbox-large, 300t), `climate_drift_rate` (drifting-climate, 400t — flag is `f32`, guard `w.climate_drift_rate > 0.0`), `nutrient_variation`+`soil_fertility` (foraging-selection, 400t), `disasters_enabled` (disturbance, 400t). Choose warm-up ticks long enough that the subsystem has fired (cross-check the subsystem's own integration test for the tick range it uses).

- [ ] **Step 2: Run the whole file.** Run: `cargo test -p anabios-core --test save_load_roundtrip`. Any FAIL is a real bug → Task 3.
- [ ] **Step 3: Add an "everything-on" integration round-trip** using `grand-theater.toml` (warms all subsystems at once) at 500 ticks — the strongest single guard.
- [ ] **Step 4: Commit.**

```bash
git add crates/anabios-core/tests/save_load_roundtrip.rs
git commit -m "test(determinism): round-trip coverage for all opt-in subsystems"
```

---

## Task 3: `#[serde(skip)]` audit + guard

Document every skip and enforce the rule that a skipped field is either (a) pure per-tick scratch rebuilt before read, (b) viewer/HUD-only and hash-excluded by design, or (c) a cache re-derived in `load_from_bytes`.

**Files:**
- Create: `crates/anabios-core/tests/serde_skip_audit.rs`
- Create: `docs/determinism-contract.md`

**Interfaces:**
- Produces: a compile-time-adjacent guard — a test that loads a warmed world, saves, loads, and asserts the two documented re-derivation caches (`pheromones.nonzero`, `agents.track_livestock`) are actually restored (not left default), catching a future removal of a `load_from_bytes` re-derivation line.

- [ ] **Step 1: Write the guard test:**

```rust
#[test]
fn load_rederives_skipped_caches() {
    // A pheromone-active + domestication scenario, warmed so both caches are non-default.
    let mut w = Scenario::parse_toml(include_str!("../../../scenarios/domestication.toml"))
        .unwrap().instantiate();
    for _ in 0..300 { step(&mut w); }
    let reloaded = load_from_bytes(&save_to_bytes(&w).unwrap()).unwrap();
    // track_livestock must be re-derived from the persisted flag, not left false.
    assert_eq!(reloaded.agents.track_livestock, reloaded.domestication_enabled);
    // pheromone nonzero cache must match a fresh recompute (decay would no-op otherwise).
    assert_eq!(state_hash(&w), state_hash(&reloaded));
}
```

- [ ] **Step 2: Run — expect PASS** (both re-derivations exist today; this is a regression lock).
- [ ] **Step 3: Write `docs/determinism-contract.md`** enumerating: the three-category skip rule; the current skip inventory (from the audit table above) with each field's category; the two load-time re-derivations that are load-bearing; and the **checklist for adding a subsystem** — "any new `#[serde(skip)]` field must be justified in one of the three categories; if it's a cache derived from serialized state, add the re-derivation to `load_from_bytes` AND a round-trip test."
- [ ] **Step 4: Commit.**

```bash
git add crates/anabios-core/tests/serde_skip_audit.rs docs/determinism-contract.md
git commit -m "test(determinism): guard load-time cache re-derivation; document skip contract"
```

---

## Testing Plan (summary)

| Level | What | Where |
|---|---|---|
| Integration | save→load→step identity per opt-in flag (18 flags) | `tests/save_load_roundtrip.rs` |
| Integration | everything-on grand-theater round-trip | `tests/save_load_roundtrip.rs` |
| Guard | load re-derives skipped caches | `tests/serde_skip_audit.rs` |
| Existing | golden ticks 0/100/1000; parallel==serial | `tests/determinism.rs` (unchanged) |
| Docs | skip contract + subsystem checklist | `docs/determinism-contract.md` |

**Done when:** every opt-in flag has a passing round-trip test, `grand-theater` round-trips clean, the skip-audit guard passes, and the determinism contract is documented. Any round-trip that failed en route was fixed by serializing the offending field (bump `FORMAT_VERSION`, changelog line, regenerate goldens) — recorded in the PR.

## Risks

- A round-trip FAIL mid-plan means a real replay bug in that subsystem — treat it as the primary deliverable, not a blocker: serialize/re-derive the field, add the fix's own golden refresh, and note it. This is exactly what the plan is designed to surface.
- Warm-up tick counts that are too short give false-green (subsystem never activated). Cross-check each against the subsystem's integration test tick range.
