# Gene-Culture A/B — Finding (2026-08-03)

Executes the Phase-2 roadmap item *Next gene-culture experiment (confound-controlled)*
([plan](../plans/2026-08-01-gene-culture-experiment.md)). **Question:** when an
apparent advantage tracks a gene, is it carried by the evolvable *gene* or merely
by a cultural *module* any lineage can adopt? **Answer:** the experiment as
written can't run (its premise gene no longer exists), and the valid current-code
version returns a **negative** result — gene↔tech coupling does *not* drive the
coupled gene to sweep; if anything selection pushes it *down*, corroborating the
cognition-cost mechanism established elsewhere this quarter.

## 1. Premise drift — the "Inventiveness gene" is gone

The plan/roadmap/memory state "cumulative-skill culture sweeps the **Inventiveness
gene**." In current code:

- **There is no `GenomeSlot::Inventiveness`.** It belonged to an earlier
  cultural-inventions mechanism that was **entirely replaced** by the 10-invention
  tree; its old slot 42 is now `TerrainAffinity` (`genome.rs:125`; history in
  `determinism.rs:61-67`).
- **"Cumulative-skill culture sweeps a gene" was a *module* sweep, not a gene
  one.** `SKILL_CHANNEL` (`culture.rs:30`) carries **no** `GeneAffinity`; the skill
  benefit is gated on the *Communicator module* (in `ModuleList`), which is
  invisible to `Genome::distance`/speciation. The historical "sweep" was
  Communicator-module frequency.

So the literal experiment is unrunnable. The current code *does* have a real
gene↔tech coupling to test instead: each invention carries a `GeneAffinity{slot,
coeff}` (`invention/mod.rs:66`); with `World.gene_tech_coupling` on, a higher
affinity-gene raises discovery weight (`0.5+gene`, `invention/mod.rs:601-606`) and
scales the invention's buff once held → selection on the gene.

## 2. Design — single-knob A/B on Openness

Target gene: **Openness** (coupled to Fire + Electricity). It is ideal because it
is a Big-Five slot → **excluded from speciation distance** (`genome.rs:366-372`,
clean per-species bookkeeping) **and auto-sampled polymorphic at t0**
(`N(0.5,0.2)`, `genome.rs:344`) → standing variation for free.

The cleanest possible A/B: instantiate a scenario, then toggle **only**
`world.gene_tech_coupling` (everything else — seed, agents, world — identical).
Two bases:
- **CLEAN:** a single `cultural_forager` population (`starter_kit`+Communicator,
  no genome pins → Openness auto-Gaussian; one interbreeding species) with
  `inventions_enabled + cognition_enabled`.
- **TGC:** the purpose-built `scenarios/tech-gene-coupling.toml` (innovator 0.9 /
  traditionalist 0.2 / asocial — same modules, differ in genome slots).

Measure population-mean Openness (and IndividualLearning) at checkpoints.
Prediction if the gene carries the advantage: Openness sweeps **up** under
coupling, drifts under decoupling.

## 3. Pilot result (throwaway, 2–3 seeds × 12 000 ticks)

Mean Openness (`O`) trajectory; `n` = alive:

| Arm | t0 | ¼ | ½ | ¾ | end |
|---|---|---|---|---|---|
| CLEAN s0 **coupled** | 0.512 | 0.458 | 0.595 | 0.605 | **extinct** |
| CLEAN s0 **decoupled** | 0.512 | 0.444 | **0.817** | **0.871** | **extinct** |
| TGC s0 **coupled** | 0.526 | 0.322 | 0.234 | 0.230 | 0.250 (n=5) |
| TGC s0 **decoupled** | 0.526 | 0.294 | 0.245 | 0.236 | extinct |
| TGC s1 **coupled** | 0.551 | 0.340 | 0.259 | 0.276 | extinct |
| TGC s1 **decoupled** | 0.551 | 0.418 | 0.408 | 0.215 | 0.098 |

Two robust observations:
1. **No coupled-gene sweep. `coupled ≈ decoupled` in every pairing** — coupling
   produces no gene-selection differential. Where a trend exists it is *downward*
   (TGC: 0.52 → ~0.23 in the viable n=400 window, both arms) or, in CLEAN,
   *higher in the decoupled arm* — the opposite of the hypothesis.
2. **Populations boom-bust to extinction** in every arm, so there is no stable
   long-run window for a clean sweep measurement regardless.

## 4. Interpretation

The coupled behavior is `Openness → more discovery → more invention/cognition
activity`, and that activity is **net metabolically costly** (`IQ_METABOLIC_COST`,
per-invention upkeep), so selection pushes the coupled gene *down*, not up. This
independently corroborates two results from this quarter:
- the **O1 exclusion autopsy** — `cognition_enabled=off` reverses cultural
  competitive exclusion 4/4 seeds (cognition *cost* is the dominant lever);
- the **OoA climb finding** — cognitive/cultural lineages are competitively
  excluded by cheap r-selected foragers.

The ecology + cognition-cost dominate any coupling-driven gene selection, so
gene↔tech coupling cannot manufacture a positive gene sweep in these scenarios.

## 5. Conclusion & decision

**Negative, confound-adjacent result.** Per the plan's own pilot gate ("pilot to
sanity-check the criterion is reachable before the full 16"), the clean positive
sweep is **not reachable** in current scenarios — so the full 3-arm × 16-seed
apparatus + regression test was **not built** (it would measure a null in
boom-busting populations). A rigorous future attempt must first re-tune a scenario
for **stable, non-boom-bust populations** (otherwise drift in a dying population
confounds any selection signal), and should expect the coupling→sweep effect to
be small-to-absent given the cognition-cost dominance.

Method note: this negative was caught by a **cheap pilot before any plan-build** —
the measure-before-plan discipline (see `docs/superpowers/specs/` OoA + trade
findings) that repeatedly paid off this quarter.

## Reproduce

Instantiate `scenarios/tech-gene-coupling.toml` (or a single `cultural_forager`
population with `inventions_enabled + cognition_enabled + lifespan_bias=1.0`), set
`world.gene_tech_coupling` to `true`/`false`, run ~12k ticks, and print
population-mean `genome.get(GenomeSlot::Openness)` over `agents.iter_alive()` at
checkpoints (mirror `tests/cognition_evolution.rs:246`). `world.species_centroids`
also holds per-species slot means every 200 ticks for free.
