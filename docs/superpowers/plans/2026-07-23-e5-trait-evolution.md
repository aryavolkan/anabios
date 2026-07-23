# E5 — Trait-Evolution Instruments — Implementation Plan

**Goal:** Genome-moment history + TraitFixation/RapidAdaptation/ConvergentEvolution detectors + evolution panel, per `docs/superpowers/specs/2026-07-23-e5-trait-evolution-design.md`.

**Determinism:** pure observers; `FORMAT_VERSION` 10→11 + one golden regen (Task T3). No behavior change.

---

## Task T1: EventType 31–33 + moments plumbing
**Files:** `codex/mod.rs`.
- Variants TraitFixation=31, RapidAdaptation=32, ConvergentEvolution=33; `EVENT_TYPE_COUNT` bump.
- Constants: `MOMENT_SPAN=400`, `MOMENT_RING=40`, `FIX_POLY_VAR=0.02`, `FIX_COLLAPSE_VAR=0.005`, `FIX_MIN_MEMBERS=10`, `RAPID_WINDOW=10` (samples), `RAPID_MIN_DELTA=0.15`, `RAPID_SIGMA_MULT=3.0`, `RAPID_COOLDOWN=400`, `CONVERGE_MEAN_TOL=0.15`, `FIXATION_ARCHIVE_CAP=500`.
- `SpeciesAgg` += `genome_sums`/`genome_sumsq: [f64;50]` (+reset, +build accumulation).
- `CodexState` += `genome_moments`, `fixation_latches: BTreeSet<(u32,u8)>`, `rapid_cooldown: BTreeMap<(u32,u8), u64>`, `fixation_archive: VecDeque<(u32,u8,f32)>`.
- `TraitMoments { tick: u64, mean: [f32;50], var: [f32;50] }` (serde).

## Task T2: `codex/traits.rs`
- `update_genome_moments(world, agg)` — 10-tick cadence, push sample, prune stale species.
- `detect_trait_fixation` (+ archive append), `detect_rapid_adaptation`, `detect_convergent_evolution` (runs on new fixations; LCA helper over `species_parents` with depth cap + cycle guard; independent ⇔ LCA is root 0).
- observe_all wiring; stuffed-moments unit tests per spec §6.

## Task T3: FORMAT_VERSION 10→11 + goldens
- Bump + changelog; regen all three suites with notes.

## Task T4: wiring
- `score.rs` (34 names, +3 bonus), `sweep.rs` (event_name +3, CSV +3), `codex_panel.gd` (+3: TraitFixed / RapidAdapt / Convergent).

## Task T5: gdext + evolution panel
- `species_trait_series(sid, slot)`, `phylogeny()` bindings.
- `game/scripts/evolution_panel.gd` ([T]): trait-drift lines for dominant species (Size/BasalMetabolism/PerceptionRadius/Openness) + phylogeny indent list; legend line; menu entry for convergent.toml.

## Task T6: scenario + evidence + gate + PR
- `scenarios/convergent.toml` (two identical-trait archetype stocks, opposite ends).
- Integration test; replay one event; 16-seed sweep; gallery panel capture.
- Gate; branch `e5-trait-evolution` stacked on `e4-disturbance-succession`; PR.

---

## Completion notes (2026-07-23)

All tasks complete. Evidence:

- **Sweep (16 seeds × 8000 ticks, `convergent.toml`):** TraitFixation 14/16 runs (earliest t=490 on seed 5), RapidAdaptation 8/16, ConvergentEvolution 1/16. Convergence is deliberately the rarest detector — sister-splinter re-fixations are LCA-rejected, so only genuine independent-lineage matches count; the E1 scorecard's novelty bonus rewards exactly this rarity.
- **Replay verification:** `replay --seed 5 --ticks 600 --event 1168` → `PASS trait_fixation tick=490 hash_ok=true refired=true` (slot 11, Neuroticism). A full 600-tick replay of all 1907 events: 0 failures.
- **Tests:** 7 stuffed-moments unit tests (positive + negative incl. never-polymorphic, slow-drift, and sister-splinter LCA negatives) + `tests/trait_evolution.rs` integration (13 s debug, sweep-derived seed-5 window).
- **Viewer:** [T] evolution panel (trait drift + phylogeny) exercised via the `ANABIOS_EVO=1` capture path; gallery `e5-evolution-panel.png`.
- **Determinism:** `FORMAT_VERSION` 10→11 (CodexState scratch); all three golden suites regenerated once with layout-growth notes.
