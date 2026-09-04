# O3 design: corrected invasion apparatus + reproductive-success payoff bias

**Date:** 2026-09-02. **Milestone:** O3 (cultural niche construction — roadmap
Horizon 1 flagship). **Prior:** O1 exclusion findings, O2a corrected
decomposition, O2b energy-proxy negative (its handoff prescribes exactly the
mechanism built here). **Probe evidence:** `docs/superpowers/data/o3/probe-log.md`.

## 1. What the probes established (measure-before-plan)

1. **The O1/O2a apparatus is diet-confounded at HEAD.** Commit `8dd9239`
   (apes-only inventions) made `cultural_forager` an omnivore while the
   `asocial_forager` resident stayed herbivore. On today's code the O1
   scenario's "cultural" mutant persists trophically (mean founder share-r
   +0.140, no inventions ever fire) — the invasion contrast no longer
   isolates culture.
2. **Corrected apparatus:** new `omnivore_forager` control archetype
   (diet-matched, no Communicator) + `o3-invasion-cultural-into-omnivore.toml`.
   On it, culture is excluded 10/10 seeds but mildly (mean share-r -0.087,
   no extinctions) — O2a's catastrophic numbers were mostly the diet gap.
3. **Food-side niche construction is null with a mechanism.** Both canonical
   O3 levers (culture-driven fertility enrichment; culture-only food tier)
   measured null at n=10, two doses each. Cause: `reproduction_threshold =
   0.2` + population at cap ⇒ energy does not bind founder share; the margin
   is decided on the **birth/death ledger**.
4. **The binding term is the maladaptive-practice birth tax.**
   `practices_enabled = false` on the matched apparatus flips share-r > 0 in
   6/10 seeds (mean +0.066, Δ +0.153 pairwise, 9/10 seeds improve). This is
   O1 §6's antagonist at its honest magnitude.

Deleting the antagonist is not a mechanism. The honest general form (O2b
handoff): culture learns to **reject** practices whose holders demonstrably
suffer — keyed on a fitness signal that can actually see a stillbirth/cull
cost. Energy cannot (O2b's measured failure); reproductive success can.

## 2. Mechanism — `repro_biased_learning` (opt-in, default off)

**State:** two per-agent counters on `AgentBuffers`, serialized (still-ticks
footgun — accumulators feeding behavior must round-trip):

- `births_ok: Vec<u16>` — surviving offspring credited to each parent.
- `births_failed: Vec<u16>` — offspring lost at birth to a practice cost
  (Inbreeding stillbirth, Child-Sacrifice cull) credited to each parent.

Both are incremented in `reproduce` **only when the flag is on** (flag-off
worlds carry all-zero columns → behavior byte-identical, layout-growth-only
golden rehash, same contract as the `payoff_biased_learning` ship).

**Transmission rule** (in `culture_step`'s neighbour scan + a pre-pass like
`apply_payoff_bias`): **content bias only, scoped to practice channels** —
no model bias (O2b's model bias measurably suppressed skill adoption; we do
not repeat it). For each practice `p`, aggregate over Communicator
neighbours: holders' and non-holders' summed `births_ok`/`births_failed`.
Decline adoption (zero the copy target) when both groups are present, at
least one birth outcome has been observed in each group, and the holders'
birth-failure fraction `failed/(ok+failed)` exceeds the non-holders'.
Deterministic, RNG-free, O(neighbours) — rides the existing scan.

**Interpretation:** agents observe that families holding the practice bury
more infants. That is observational content bias on the exact channel the
practice damages, with practices still present in the world.

## 3. Pre-registered measurement (before implementation finishes)

Protocol identical to O2a/O2b: `autopsy --tag founder --mutant cultural`,
2500 ticks, window 500, n=10 seeds (1–10), on
`o3-invasion-cultural-into-omnivore.toml` + `repro_biased_learning = true`
(one variable apart; practices present).

- **Bar:** founder `invasion_fitness_share` > 0 in a **majority of seeds**
  (matched baseline: 0/10, mean -0.087).
- **Ceiling reference:** practices-off (+0.066 mean, 6/10 positive).
- **Control:** invention/skill adoption not suppressed vs baseline (founder
  `mean_skill` at t2000 within noise of baseline) — the O2b failure mode.
- **Negative result is a result:** if the bar is missed, the write-up
  records the measured margin and the fitness ledger, per the roadmap's
  done-when.

## 4. Engineering contract

- Scenario flag `repro_biased_learning` (serde default false) → `World`
  field; `FORMAT_VERSION` 35→36 (AgentBuffers layout growth); goldens
  regenerated in the same PR; save→load→step round-trip test for the new
  counters; `cargo fmt`/clippy/rustdoc gates green.
- Tests (mirror `tests/culture.rs` O2b suite): (a) positive — a practice
  whose holders show worse birth outcomes is declined under the flag;
  (b) flag-off control — same setup transmits; (c) negative control — a
  practice with no observed birth-outcome difference still transmits;
  (d) skill/invention channels untouched by the bias.
- Detector (`SelectiveLearning`) stays deferred until the mechanism
  measures positive (O2b precedent) — ship it with the working variant only.
- Perf: counters are two `u16` columns; the scan additions run only when
  the flag is on. No new passes.

## 5. Scope guard

This lands the O3 *invasion-margin* deliverable on the corrected apparatus.
The era-3 climb attempt (OoA-Earth stage 2) and the O3 detectors
(`NicheConstructed`/`ResourceTierUnlocked`) are follow-on work: the probe
evidence says era-gated resource tiers only matter once the birth-ledger
handicap is neutralized, so this mechanism is the prerequisite, not a rival.
