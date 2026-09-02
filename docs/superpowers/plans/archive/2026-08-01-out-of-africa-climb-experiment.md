# Out-of-Africa Invention-Climb Experiment Plan

> **For agentic workers:** this is a research/experiment plan, not a code-feature plan. Tasks are an experiment protocol with tuning knobs, measurements, and decision gates. Steps use checkbox (`- [ ]`) syntax. Where a task changes tuning constants, treat it as a behavior change: gate behind a scenario knob and regenerate goldens.

**Goal:** Determine whether the grand-scale Out-of-Africa run can reach the era-3 milestones (Writing, Husbandry) by *emergence*, or whether seeding them (`starting_inventions`) is the honest framing — and record the decision with measured evidence.

**Hypothesis:** The climb stalls at era-1 because the era-3 IQ ceiling (`IQ_REQ_BY_ERA[2] = 0.55`) is never met at grand scale, not because discovery probability is too low. Raising cognitive capability (or lowering the era-3 IQ gate) under otherwise-grand conditions should let a run reach Writing emergently; if it cannot without also destabilizing the ecosystem, seeding is the right call.

## Background (grounded)

- Measured stall (docs/showcase-plan.md:50-57): `out-of-africa` (3000 agents) reaches only Stone Tools (t≈797) + Fire (t≈2086) even at 40k ticks; era-3 never; 0 domestications. The saga only delivers Writing+Husbandry because `starting_inventions` seeds them at t0.
- Discovery gating — `invention/mod.rs:696-716`. Probability `p = BASE_DISCOVERY(3e-5) * openness * (0.3+skill) * disc_mult / era * affinity`, capped `DISCOVERY_CAP=0.05`. Three gates:
  - **Openness** `gene_req` on Fire (0.30) — the era-1→2 barrier.
  - **skill** — culture SKILL_CHANNEL feeds `(0.3+skill)`.
  - **IQ ceiling** — `iq_permits` (`invention/mod.rs:379-381`): `!cognition_enabled || iq >= iq_req(k)`; `IQ_REQ_BY_ERA = [0.15,0.35,0.55,0.75]` (`params.rs:365`). **The hard era-3 blocker** — the primary knob.
- Structural blockers (docs/showcase-plan.md:59-80): Malthusian churn (populations crash before climbing), geographic separation (cultures don't pool skill), material-economy/population coupling.
- Tooling: `anabios-headless demo --scenario … --report-every 1000` narrates the climb; `sweep` gives per-seed final tech; the emergence scorecard (`invention_discovered`/`invention_adopted` columns) quantifies climb depth across seeds.

## Experiment protocol

### Task 1: Reproduce and instrument the baseline

- [ ] **Step 1: Baseline sweep.** `scripts/emergence.sh sweep out-of-africa --seeds 16 --ticks 20000 --out runs/ooa-baseline`. Record per-seed max era reached (derive from `invention_discovered` events in the per-seed JSONL) and final population.
- [ ] **Step 2: Confirm the stall.** Assert the finding: max era ≤ 2 across all seeds, era-3 count = 0. If any seed reaches era-3, the premise is weaker than documented — note it and narrow the hypothesis.
- [ ] **Step 3: Instrument the gate.** For one representative seed, run `demo --scenario scenarios/out-of-africa.toml --seed <s> --ticks 20000 --report-every 1000` and record: highest tech reached, whether Farming was ever discovered (era-3 prereq), and the population trajectory. This isolates whether the block is prereq (never reach Farming) or capability (reach Farming but IQ-gated out of Writing).

### Task 2: Single-knob interventions (one variable at a time)

Run each as a scenario variant (a copy of `out-of-africa.toml` changing exactly one knob), swept 16 seeds × 20000 ticks, measured identically to the baseline. **Change one knob per variant** so the effect is attributable.

- [ ] **Step 1: Lower the era-3 IQ gate.** Variant `ooa-iq3-lo`: reduce `IQ_REQ_BY_ERA[2]` 0.55→0.45 (a tuning const — gate it behind a scenario knob `era3_iq_req` if you want goldens stable, or accept a golden refresh). Measure era-3 reach.
- [ ] **Step 2: Raise cognitive capability.** Variant `ooa-cog`: enable/boost the cognition trait distribution so more agents clear the IQ ceiling (via the scenario's genome init, not the gate). Measure.
- [ ] **Step 3: Stabilize population.** Variant `ooa-stable`: soften the Malthusian churn (higher `max_population`, gentler starvation) so cultures survive long enough to climb. Measure whether longer-lived cultures accumulate skill and cross the gate.
- [ ] **Step 4: Concentrate the cradle.** Variant `ooa-cradle`: tighter founding geography so skill pools (addresses the "geographic separation" blocker). Measure.
- [ ] **Step 5: Record a knob×outcome table** — for each variant: fraction of seeds reaching era-2, era-3, and any domestication; median tick of Writing if reached; population at t=20000.

### Task 3: Decision gate

- [ ] **Step 1: Evaluate.** If any single knob (or a documented minimal pair) yields ≥50% of seeds reaching era-3 **without** collapsing the ecosystem (population doesn't crater, other emergence events don't vanish), that's the emergent path — promote the variant to a real scenario `out-of-africa-emergent.toml` with pinned seed + golden hashes.
- [ ] **Step 2: If no acceptable knob exists,** document that seeding is the honest framing: update `docs/showcase-plan.md` §2 with the knob×outcome table and the conclusion, and keep `starting_inventions` in the saga. The negative result is a valid deliverable.
- [ ] **Step 3: Either way, write up** the evidence table in `docs/showcase-plan.md` (or a new `docs/superpowers/specs/2026-08-…-ooa-climb-findings.md`).

## Measurement / "testing" plan

| What | How | Pass/record criterion |
|---|---|---|
| Baseline stall reproduced | 16-seed sweep, JSONL era analysis | era-3 count = 0 (or note exception) |
| Per-knob effect | one-variable variants, identical sweep | knob×outcome table filled |
| Emergent path found? | era-3 reach ≥50% seeds, ecosystem intact | promote scenario + goldens, OR |
| Seeding justified | no acceptable knob | documented decision + table |
| Determinism preserved | any tuning-const change gated or golden-refreshed | `tests/determinism.rs` green |

**Done when:** the knob×outcome table exists and either an `out-of-africa-emergent.toml` (with golden hashes) reaches era-3 emergently, or `docs/showcase-plan.md` records the measured decision to keep seeding. No silent tuning changes — any constant touched is gated behind a scenario knob or lands with a golden refresh in the same PR.

## Risks / notes

- Tuning `IQ_REQ_BY_ERA` or genome init changes default-adjacent behavior — keep each variant a distinct scenario so the shipped defaults' goldens never move unexpectedly.
- 16×20000-tick sweeps at 3000 agents are heavy; run on the release binary and let it parallelize over rayon (`--threads`). Budget wall-clock accordingly; consider a 8-seed pilot before the full 16.
- Confound guard: never change two knobs in one variant — the whole value of the experiment is attributable single-variable effects.
