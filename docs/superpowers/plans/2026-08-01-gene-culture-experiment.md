# Next Gene-Culture Experiment (Confound-Controlled) Plan

> **⚠️ SUPERSEDED — stale premise + resolved negative (2026-08-03).** This plan's
> target, the "Inventiveness gene," no longer exists (replaced by the invention
> tree), and the historical "cumulative-skill sweep" was a *Communicator-module*
> shift, not a genome slot. The valid current-code version (toggle only
> `gene_tech_coupling`, watch the coupled **Openness** gene) was piloted and is
> **negative**: no coupled-gene sweep (coupled ≈ decoupled; Openness selected
> down), populations boom-bust — consistent with the cognition-cost mechanism. The
> full 3-arm apparatus was NOT built (pilot gate: criterion unreachable). See
> [`../specs/2026-08-03-gene-culture-finding.md`](../specs/2026-08-03-gene-culture-finding.md).
> The confound-control *method* below is still sound; a future attempt needs a
> stable-population scenario and should re-target a real (current-code) coupled gene.

> **For agentic workers:** research/experiment plan. Tasks are an experiment protocol; the "implementation" is scenario construction + a golden-tested analysis, and the deliverable is a written finding (positive OR negative). Steps use checkbox (`- [ ]`) syntax.

**Goal:** Run the next DIT gene-culture test designed to **disentangle module-vs-gene** before crediting any sweep — i.e. determine whether an observed advantage is carried by an evolvable *gene* or merely by a cultural *module* that any lineage can adopt.

**Hypothesis (to be falsified):** Prior results show cumulative-skill culture *does* sweep the Inventiveness gene, but a first-principles culture gene does *not* sweep from standing variation, and winner-take-all caps mask differentials. The next test isolates the gene's contribution by holding the module constant across arms, so any fixation is attributable to the gene, not the module.

## Background (grounded)

- Prior findings (memory): (a) cumulative-skill culture → Inventiveness gene sweeps to fixation (confound-controlled); (b) first-principles culture gene does NOT sweep from standing variation; winner-take-all cap masks differential. "Disentangle module-vs-gene before crediting a sweep."
- Invention discovery couples gene ↔ tech via `GeneAffinity { slot, coeff }` and hard `GeneReq { slot, min }` (`invention/mod.rs:65-85`); discovery probability multiplies `discovery_affinity_weight(genome, k, coupling)` (`invention/mod.rs:707`).
- Gene-tech coupling flag: `gene_tech_coupling` (scenario), round-trip-tested (`determinism.rs:16`). Gene-requirements flag: `gene_requirements` (v23 snapshot field).
- Scenario DIT controls exist: `dit-rogers.toml`, `dit-env-{slow,fast,static}.toml`, `gene-culture{,-skill,-hunt,-alarm}.toml`, `tech-gene-coupling.toml`.
- Analysis surface: per-seed JSONL events + `summary.csv`; gene-frequency trajectories are derivable by instrumenting a headless run to dump per-species mean genome slots over time (a small analysis binary or a test that asserts fixation).

## Experiment protocol

### Task 1: Design the confound-controlled arms

The core design: three scenario arms that share the *cultural module* but differ in whether a *gene* can vary and whether it is *coupled* to the module's payoff.

- [ ] **Step 1: Define the target gene + module.** Pick the gene under test (e.g. Inventiveness / an EnvAffinity slot) and the cultural module that delivers the benefit (e.g. cumulative skill via SKILL_CHANNEL). Write down the exact genome slot and the coupling (`GeneAffinity.coeff`).
- [ ] **Step 2: Arm A — gene varies + coupled.** Scenario `gc-coupled.toml`: gene starts polymorphic (standing variation), `gene_tech_coupling = true` so the gene's value scales the module's payoff. Prediction: gene sweeps if it carries advantage.
- [ ] **Step 3: Arm B — gene varies + decoupled (module-only control).** Scenario `gc-decoupled.toml`: identical starting polymorphism, but coupling OFF so the module benefit is gene-independent (any lineage adopting the module gets it equally). Prediction: gene does NOT sweep (no differential) — this is the confound control that proves a sweep in Arm A is gene-driven, not module-driven.
- [ ] **Step 4: Arm C — gene fixed (module-only baseline).** Scenario `gc-fixed.toml`: gene monomorphic at a mid value, module active. Establishes the module's population-level effect with no gene variance at all.
- [ ] **Step 5: Guard against the winner-take-all cap.** Ensure population/`max_population` and the module's benefit are tuned so a differential is *observable* (not masked by a hard cap). Cross-reference the "winner-take-all cap masks differential" finding — if the cap is binding, widen it or lengthen the run so relative frequencies can move.

### Task 2: Instrument gene-frequency trajectories

- [ ] **Step 1: Add a measurement path.** Either a tiny `anabios-headless` subcommand or a `#[cfg(test)]` harness that runs a scenario N seeds × M ticks and dumps, per checkpoint, the population-mean and variance of the target genome slot (and its per-lineage distribution). Keep it read-only / determinism-neutral (like `record`).
- [ ] **Step 2: Define "sweep".** A pre-registered threshold: mean slot value moves from its initial ~midpoint to ≥ X (fixation) in ≥ K of N seeds by tick M, with variance collapsing. Write this down before running (avoid post-hoc goalposts).

### Task 3: Run and analyze

- [ ] **Step 1: Sweep each arm.** 16 seeds × (long enough for fixation, e.g. 15–20k ticks) per arm, via the scorecard sweep pipeline so you also get emergence context.
- [ ] **Step 2: Compare.** Arm A sweeps AND Arm B does not ⇒ the gene carries the advantage (positive, confound-controlled). Arm A sweeps AND Arm B also sweeps ⇒ the "sweep" is an artifact (drift, hitchhiking, or module-correlated selection) — NOT gene-driven; report negative. Arm A does not sweep ⇒ no gene advantage under these conditions; report negative.
- [ ] **Step 3: Pin a regression.** Encode the key comparison as a `#[cfg_attr(debug_assertions, ignore)]` release test asserting the pre-registered fixation criterion for Arm A and its absence for Arm B (mirror the emergence-floor tests in `tests/domestication.rs:108`). This makes the finding reproducible and guards against future regressions.

### Task 4: Write the finding

- [ ] **Step 1: Document** in `docs/superpowers/specs/2026-08-…-gene-culture-finding.md`: the design (3 arms), the pre-registered sweep criterion, the measured trajectories, and the conclusion (positive/negative) with the confound explicitly addressed ("Arm B rules out module-only explanation").
- [ ] **Step 2: Update memory/README** if the finding changes the project's understanding of the DIT line.

## Measurement / "testing" plan

| What | How | Criterion |
|---|---|---|
| Gene-frequency trajectory | instrumented headless run | mean/variance of target slot per checkpoint |
| Sweep in coupled arm | pre-registered threshold | ≥K/N seeds reach fixation by tick M |
| No sweep in decoupled arm | same threshold | < K/N seeds (confound ruled out) |
| Reproducibility | release regression test | Arm A passes floor, Arm B fails it |
| Determinism | arms are deterministic; instrumentation read-only | `state_hash` stable, `record`-style side-effect-free |

**Done when:** all three arms have run, the pre-registered criterion is evaluated, a release regression test pins the A-sweeps-B-doesn't (or the negative), and a written finding documents the confound-controlled conclusion.

## Risks / notes

- **This is the whole point:** do not credit a sweep from Arm A alone. The decoupled Arm B is the deliverable that makes the result trustworthy. If Arm B is skipped, the experiment is worthless.
- Standing-variation initialization matters — a gene seeded already near fixation can't "sweep." Start polymorphic and document the initial distribution.
- Long runs at population scale are heavy; pilot with 4 seeds to sanity-check the criterion is reachable before the full 16.
- Reuse existing DIT scenarios (`dit-rogers.toml`, `gene-culture-skill.toml`) as starting points rather than authoring from scratch.
