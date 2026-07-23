# E5 — Trait-Evolution Instruments — Design Spec

**Date:** 2026-07-23
**Status:** Approved
**Milestone:** E5 of the emergence roadmap (`2026-07-22-emergence-roadmap-design.md`)
**Crate:** `anabios-core` (moments + detectors), `anabios-godot` (series/phylogeny bindings), `game/` (evolution panel). Detectors only — `FORMAT_VERSION` 10→11 (CodexState scratch), goldens regenerated once, behavior unchanged.

## 1. Goal & success criteria

Make *evolution itself* visible (design §4.3). Three new event types: `TraitFixation` (31), `RapidAdaptation` (32), `ConvergentEvolution` (33). Plus the instruments that make them legible: per-species trait-drift series and a phylogeny view.

Success criteria:

1. Per-species genome-moment history (mean + variance per slot, 10-tick cadence, 400-tick span) computed in the fused agg pass; memory bounded by pruning extinct species.
2. Each detector has positive + negative handcrafted tests; a 16-seed sweep of `convergent.toml` fires `ConvergentEvolution` in a meaningful minority of runs.
3. Viewer: [T] evolution panel — trait-drift lines for the dominant species + phylogeny tree.
4. Golden suites pass (layout-only regen).

## 2. Genome-moment history

- `SpeciesAgg` gains `genome_sums: [f64; 50]` and `genome_sumsq: [f64; 50]` (ascending-id accumulation in the fused pass).
- `CodexState.genome_moments: BTreeMap<u32, VecDeque<TraitMoments>>`; `TraitMoments { tick, mean: [f32;50], var: [f32;50] }`, one sample per species per 10 ticks, ring of 40. Species absent from the active set with a newest sample older than 400 ticks are pruned (memory bound).

## 3. Detectors (`codex/traits.rs`)

- **`TraitFixation`** — full history; some slot was polymorphic (var ≥ 0.02 in ≥half of the first-half samples) and has collapsed (var ≤ 0.005 in all of the last 10 samples), species ≥ 10 members. `value` = slot id. Latched per (species, slot); re-arms if variance re-opens. Every fixation appends `(species, slot, mean)` to a bounded **fixation archive** (cap 500).
- **`RapidAdaptation`** — over the last 100 ticks (10 samples): |Δmean| ≥ 0.15 **and** ≥ 3× the slot's recent σ. `value` = slot id; per-(species, slot) 400-tick cooldown. Shock correlation (disasters) strengthens the story but is not required — the detector is specified to work in shock-free worlds (roadmap §10).
- **`ConvergentEvolution`** — on each new fixation, scan the archive for a matching fixation (same slot, |Δmean| ≤ 0.15) by a species whose lineage is **independent**: LCA of the two species is the universal root (species 0) — i.e., neither descendant of the other and no shared post-founder ancestor. `species_id` = the newer fixer, `value` = slot id, loc = its centroid.

## 4. Viewer

- gdext: `species_trait_series(sid, slot) -> PackedFloat32Array` (mean history), `phylogeny() -> Array[VarDictionary]` (`id, parent, count, depth`).
- `game/scripts/evolution_panel.gd` ([T]): trait-drift lines (Size, BasalMetabolism, PerceptionRadius, Openness) for the dominant species + an indented phylogeny list (top species by count). Legend entry.

## 5. Scenario

`scenarios/convergent.toml`: two archetype stocks (distinct founder species, independent roots) with identical trait overrides, placed at opposite ends of the world in matched terrain — matched niches select for matched fixes. Menu entry.

## 6. Testing & evidence

- Unit: stuffed-moments tests — polymorphic→collapsed fires fixation once; always-collapsed does not (never polymorphic); rapid shift fires; slow drift does not; convergence between independent lineages fires, between sister splinters does not (LCA discipline).
- Integration: convergent.toml long run; replay-verify one event.
- Sweep: 16 seeds × 8000 ticks, per-type counts in completion notes.
- Gallery: evolution panel capture (trait drift + phylogeny).
