# E3 — Population Dynamics Completion — Implementation Plan

**Goal:** +4 population-dynamics detectors (`PopulationCycleDetected`, `BoomAndBust`, `CarryingCapacityReached`, `TrophicCascade`) per `docs/superpowers/specs/2026-07-22-e3-population-dynamics-design.md`.

**Determinism:** detectors are pure observers, but the serialized layout grows (CodexState scratch) and the event buffer gains events → `FORMAT_VERSION` 8→9 + one deliberate golden regeneration (Task P3). No behavior change; no `HashMap`; detectors join the fused-agg pattern.

---

## Task P1: EventType variants + constants + CodexState fields
**Files:** `codex/mod.rs`.
- Variants 23–26 with doc comments (`EVENT_TYPE_COUNT` auto-derives).
- Constants: `CYCLE_WINDOW=400`, `CYCLE_CHECK_INTERVAL=10`, `CYCLE_PERIOD_MIN=40`, `CYCLE_PERIOD_MAX=200`, `CYCLE_MIN_AMPLITUDE=0.25`, `BOOM_AMPLITUDE=3.0`, `CARRYING_MIN_POP=20`, `CARRYING_MAX_CV=0.05`, `CASCADE_WINDOW=150`, `CASCADE_CRASH_FRAC=0.5`, `CASCADE_MIN_PREDATORS=5`, `CASCADE_LAG=120`, `CASCADE_HERB_RISE=0.3`, `CASCADE_PLANT_DROP=0.3`.
- `CodexState` += fields per spec §5 (all serde, all `Default`-friendly).

## Task P2: `codex/cycles.rs` detectors
**Files:** `codex/cycles.rs` (new), `codex/mod.rs` (wire `mod cycles;` + `observe_all` calls after `detect_population_crash`).
- `update_cycle_history(world, agg)` — push per-species counts into 400-window VecDeques (also record 0 for species that went extinct? No: only active species get samples; extinct species' buffers age out — keep them, detectors require full windows of *current* membership).
- `detect_cycles(world, agg)` — every `CYCLE_CHECK_INTERVAL` ticks (gate on `world.tick % 10 == 0`): zero-crossing analysis per spec §2; latch/re-arm via `cycle_active`/`boom_active`; events carry species centroid loc, `value` = period (cycle) or ratio (boom).
- `detect_carrying_capacity(world, agg)` — same gate; CV over full window; latch/re-arm.
- `SpeciesAgg` += `diet_sum: f64` (`mod.rs` build pass: `effective_diet_carnivory(&modules)` accumulated ascending-id; reset in `reset()`).
- `detect_trophic_cascade(world, agg)` — per tick: classify active species by mean carnivory ≥ 0.5; sum carnivore/herbivore pops; drive the staged machine per spec §4 (carnivore peak from `cascade_carn_history` 150-window).
- Unit tests in `cycles.rs` (stuffed histories, positive + negative per spec §7). Tests construct a minimal `World`, write `codex.cycle_history` / cascade fields directly, call the detect fns, and assert on `codex.events`.

## Task P3: snapshot FORMAT_VERSION 8→9 + golden regen
**Files:** `snapshot.rs` (version + changelog), `tests/determinism.rs`, `tests/inventions.rs` (golden values).
- Bump, `UPDATE_HASHES=1 cargo test -p anabios-core --test determinism -- --nocapture` and `--test inventions -- --nocapture`, paste values, note behavior-unchanged rationale.

## Task P4: wiring (headless + viewer)
**Files:** `score.rs` (names 23→27, weights +4 `NOVELTY_BONUS`, test count), `sweep.rs` (`event_name` +4, CSV header/row +4 before scorecard columns, name tests), `codex_panel.gd` (`CHAPTER_NAMES`/`CHAPTER_COLORS` +4: PopCycle sky-blue, BoomBust hot orange, CarryingCap steady green, TrophicCascade violet).
- Update the E1 scorecard spec reference? No — weights table gains entries at bonus; `WEIGHTS_VERSION` stays `e1.1` (corpus unchanged; new types are definitionally unseen). Note in plan.

## Task P5: `scenarios/trophic-cascade.toml`
- Predator/prey tuned for deep oscillation: 80 grazers (cluster center), 12 stalkers, `max_population` 1500, default biome. Base on `predator-prey.toml` with higher predator pressure.
- Menu entry in `menu.gd` SCENARIOS ("E3 — Trophic cascade").

## Task P6: tests + sweep evidence + gallery
- `cargo test --workspace` green; `tests/codex_events.rs`-style integration: trophic-cascade 3000 ticks fires ≥1 new event type (pick whichever the run reliably produces; assert ≥1 of the four, print all).
- `anabios-headless replay` on the same scenario (500 ticks) — all PASS.
- Sweep 16 seeds × 6000 ticks → record per-type counts in completion notes.
- Gallery capture: codex panel with new chapters visible (`gallery/e3-cascade-events.png`).

## Task P7: gate + PR
- fmt, clippy `-D warnings`, full workspace tests, `cargo build -p anabios-godot`.
- Branch `e3-population-dynamics` stacked on `e2-replay-event-camera`; PR base accordingly.

---

## Completion notes (2026-07-23)

All tasks complete. Evidence:

- **Sweep (16 seeds × 4000 ticks, `trophic-cascade.toml`):** CarryingCapacityReached 16/16 runs, PopulationCycleDetected 14/16, TrophicCascade 9/16, BoomAndBust 8/16.
- **Design revision (guild series):** per-species cycle detection alone fired zero times in real runs — species churn under 200-tick reclustering kills every 400-tick window. Added herbivore-guild / carnivore-guild / world-total series (SpeciesAgg `diet_sum`); guild events carry the largest member species as representative. This is the path that fires in reality; per-species detection remains for calm worlds.
- **Cascade tuning (instrumented runs):** the staged machine opened correctly but real cascades died on the plant leg — grazer release takes ~1000 ticks to graze the field down. Fix: plant reference resets at boom confirmation and the plant leg gets `CASCADE_PLANT_LAG = 900` (vs 300 for the prey leg). predator-prey seed 0 now fires the cascade at t=1690 (stalkers crash → grazers 555→9,989 → plants 109k→13k).
- **Replay verification:** `replay --event 11416` on predator-prey (2000 ticks) → `PASS trophic_cascade tick=1690 hash_ok=true refired=true`.
- **Tests:** 10 stuffed-history unit tests (positive + negative per detector, incl. cascade timeout/out-of-order) + `tests/population_dynamics.rs` integration (8.5 s debug, cap pinned to 500).
- **Determinism:** `FORMAT_VERSION` 8→9 (CodexState scratch); golden hashes regenerated once with the layout-growth note; no behavior change.
- **Gallery:** `gallery/e3-population-dynamics.png` — tally line with all four new chapters live at t=1821, 9,992 alive.
