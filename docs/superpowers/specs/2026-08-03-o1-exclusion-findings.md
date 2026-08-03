# O1 findings: competitive exclusion of culture — autopsy conclusions

Milestone: O1 (out-of-africa competitive-exclusion autopsy), part of the
[open-ended-complexity arc](2026-08-02-open-ended-complexity-arc-design.md).
This document is the synthesis deliverable for Tasks 1-7; it resolves arc
open question #1 (§7).

## Summary

The out-of-africa world's cultural strategy loses to the asocial strategy —
that much was known going in. This autopsy asked *why*, and specifically
whether the blocker is the **era-3 IQ ceiling** (the leading hypothesis in
the prior `2026-08-01-out-of-africa-climb-experiment.md` plan) or
**competitive exclusion** (the hypothesis recorded in the project history).

**Verdict: competitive exclusion, not the IQ ceiling.** The baseline run
never gets anywhere near era 3 — `max_era = 0` for both strategies across
the entire 20000-tick run, and peak mean IQ (0.4761) never even reaches the
era-3 gate (0.55). The IQ ceiling cannot be the binding constraint on a run
that never approaches it. Culture instead loses a population-share contest
against asocial foragers well before era progression becomes relevant.

A one-variable lever scan then asked *what drives the exclusion margin*, and
found a single dominant, reversible lever: **the cognition/IQ subsystem
itself**. Turning cognition off does not just soften the exclusion — it
reverses it, in 2/2 pilot seeds and confirmed at matched n=4. The IQ
tax/gating that cultural agents pay to be culture-capable is what tips the
balance against them, not a ceiling they never reach.

## 1. Fitness-ledger baseline (Task 4)

Scenario: `scenarios/out-of-africa.toml`, seed 318, 20000 ticks, mutant =
cultural. Full ledger: `docs/superpowers/data/o1/ooa-baseline-ledger.csv`.

- Tool verdict: `invasion_fitness mutant=cultural r=-0.03759 EXCLUDED`.
- End-of-run frequency: asocial **0.9735** (1067 agents) vs. cultural
  **0.0265** (29 agents).
- Cultural frequency declines **monotonically from the very first sampled
  window** (tick 500, freq 0.3200) through tick 6500 (freq 0.0263) — 12
  consecutive windows, 6000 ticks, no reversal — then settles into a noisy
  low plateau (0.013-0.042) for the remaining 13500 ticks. Exclusion is
  immediate-onset and durable, not a late-game or transient effect.
- Culture is not simply non-functional: cultural `mean_skill` rises from
  0.8275 to a peak of **1.6463 at tick 6500** (asocial `mean_skill` is
  exactly 0.0000 in all 40 windows, by construction — skill accrual is
  gated on the Communicator module). The trait pays off individually even
  as its carrier sub-population is squeezed out. This is a "works but
  loses" pattern, not a "never gets going" one.

## 2. Bidirectional invasion (Task 5)

Two one-variable-apart dense-cradle scenarios (1000 resident + ~20 rare
mutant, mixed from tick 0), 3 seeds each direction (318, 1, 2). Full data:
`docs/superpowers/data/o1/inv-{cul-into-aso,aso-into-cul}-{318,1,2}.csv`,
`invasion-share-analysis.csv`.

**The tool's absolute-count `invasion_fitness` is unreliable here.** It
computes log-growth on raw mutant count, which is only valid if total world
population is roughly stationary. It was not: 3/6 runs collapsed the entire
cluster (both strategies) to zero population, which produces a negative r
that reflects the shared crash, not competitive dynamics. Reading the
absolute numbers naively erases the asymmetry the milestone was built to
find.

**The correct metric is share-while-rare** (mutant freq trend while
freq ≤ 0.10), which is invariant to what the total population is doing:

| direction | seed | trend |
|---|---|---|
| cul→aso | 318, 1, 2 | **down_excluded, all 3** |
| aso→cul | 318 | down_excluded (but world-collapse, uninformative) |
| aso→cul | 1 | up_invades — **but confounded** (see below) |
| aso→cul | 2 | up_invades — clean |

**Cultural cannot invade a stable asocial world: 3/3 seeds, unconfounded.**
Asocial invading a cultural world is **weaker evidence**: 1 clean invasion
(seed 2), 1 collapse-dominated non-result (seed 318, world hits zero
population), and 1 confounded reappearance (seed 1) — the asocial-tagged
mutant count is **literal zero for 16 consecutive windows (ticks
4000-11500)**, a genuine multi-thousand-tick lineage extinction, before
reappearing and growing. Because the cultural/asocial strategy tag is read
off per-individual Communicator-module presence — not a lineage marker, and
recombined/mutated at every birth (`crossover_and_mutate`,
`crates/anabios-core/src/module/mod.rs`) — the more parsimonious
explanation for the post-gap reappearance is recurrent module-mutation from
the ~1000-strong cultural resident line, not survival of the original 20
asocial invaders. This result is reported as ambiguous, not as invasion
evidence.

**Population stability is not uniformly asymmetric — apply the same caveat
in both directions.** Two of the three cul→aso (asocial-resident) worlds
are genuinely stable: seed 1 holds 2993/2998/2997 agents across the run,
essentially flat; seed 2 has a transient dip to 2320 at t2000 then fully
recovers. But **seed 318 also collapses**: total population (both
strategies) falls from 2990 at tick 500 to zero from tick ~15000 through
20000 — the asocial resident itself goes 2699→0 over that span, the same
collapse phenomenon already noted for the aso→cul direction. So it is not
"asocial-resident worlds are stable, cultural-resident worlds collapse" —
collapse occurs in both directions (1/3 cul→aso worlds, all 3/3 aso→cul
worlds), and culture-bearing cultural-resident worlds collapse more often,
not exclusively. This does not change the cultural-exclusion verdict for
seed 318, though: its cultural share was already in steep, monotone
decline well before the crash (0.0973→0.0080, a ~12x drop over the first
~5000 ticks, cultural count down to single digits by tick 4000) — the
exclusion signal is established on its own timescale, independent of and
prior to the eventual population collapse, so seed 318 still counts as a
clean, unconfounded contributor to the cultural-excluded 3/3 result. Only
the blanket "asocial-resident worlds are stable" framing is wrong; the
underlying share-based exclusion finding is unaffected. Culture, as
currently modeled, is demographically fragile at density in general — this
shows up as outright collapse in all 3/3 cultural-resident worlds and in
1/3 asocial-resident worlds.

**Net bidirectional verdict:** the cultural-excluded direction is confirmed
and strong (3/3, unconfounded). The asocial-invades direction is
directionally consistent with asymmetric exclusion but rests on thinner
evidence (1/3 clean) and is not itself a load-bearing result of this
milestone.

## 3. IQ-ceiling vs. competitive-exclusion adjudication

This is the headline call the milestone was commissioned to make.

- The era-3 IQ gate is `IQ_REQ_BY_ERA[2] = 0.55`.
- Across the entire baseline run (40 windows, both strategies), `max_era =
  0` in every single window. No agent of either strategy advances past era
  0, let alone reaches era 3.
- The highest `mean_iq` observed anywhere in the run is **0.4761**
  (cultural, tick 500 — its own peak, before declining) — below 0.55, and
  that's a population mean, not even asking whether individually-qualifying
  agents exist.
- Since the population never reaches era 1, the era-3 gate is not merely
  "not yet crossed" — it is **never approached, never tested**. It
  structurally cannot be the mechanism blocking the climb in this run.

**Adjudication: competitive exclusion is the binding constraint, not the
IQ ceiling.** This settles the disagreement between the
`out-of-africa-climb-experiment` plan (which assumed the IQ-ceiling
hypothesis) and the project's recorded competitive-exclusion hypothesis:
the recorded hypothesis is correct. Cultural agents lose the population
contest to asocial agents on a timescale (first few thousand ticks) that
precedes any era-progression dynamics by a wide margin.

## 4. Knob × margin: the lever scan (Task 6)

Four scenarios, each one variable apart from the cul→aso invasion baseline,
seeds 1 and 2, metric = cultural share retention `freq(t3000)/freq(t500)`
(>1 = culture gains share, weaker exclusion). Full data:
`docs/superpowers/data/o1/lever-share-retention.csv`.

| lever | knob | seed 1 | seed 2 | read |
|---|---|---|---|---|
| baseline | (unmodified cul→aso) | 0.439 | 0.407 | excluded |
| density | mutant radius 90→40 | 0.542 | 0.370 | ~neutral |
| ceiling | `resources_enabled=false` | 0.612 | 0.185 | mixed/noisy |
| **cognition** | `cognition_enabled=false` | **3.759** | **2.468** | **reversal — culture GAINS share** |
| mixing | mutant count 20→120 | 0.118 | 0.283 | worse |

Cognition is the only lever that flips the outcome (crosses retention = 1.0)
in both seeds, and does so by a wide margin (2.5-3.8x share growth vs.
0.19-0.61x for every other lever/seed cell). Density, ceiling, and mixing
all leave cultural share shrinking in every seed tested.

The absolute-r confound from Task 5 recurs here: cognition-seed-1's
absolute `invasion_fitness` r is **-0.11570** — reads as "worse than
baseline" by the tool's own verdict line — while the identical run's share
retention is 3.759 (cultural share nearly quadrupling). Trusting the
tool's own verdict here would produce the opposite conclusion from the
correct one; see §6.

**Operational finding:** `cognition_enabled=false` removes the IQ-based
population attrition, so population pins at the 3000 cap and the dense
single-cluster war/settlement/pheromone interactions run at maximum
expense for the whole window. `lever-cognition-2` at the planned
20000-tick horizon ran ~46 minutes before being killed; an 8000-tick
retry on the same seed timed out around ~3000 ticks. Its row in
`lever-share-retention.csv` is recorded at a `3000_timeout` horizon (a
genuine read of the ticks it reached, not extrapolated) rather than the
standard window used elsewhere. This is a wall-time/scenario-sizing
consideration for anyone re-running cognition-off scenarios, not a
data-quality issue with the retention number.

## 5. Dominant lever: cognition, confirmed at n=4 (Task 7)

The lever scan is a 2-seed pilot. Task 7 ran a matched cognition-on/off
comparison at n=4 seeds (1, 2, 3, 4) to check whether the reversal holds up.
Metric: retention `freq(t2000)/freq(t500)`. Full data:
`docs/superpowers/data/o1/cognition-confirmation.csv`.

| seed | cognition ON | cognition OFF |
|---|---|---|
| 1 | 0.737 | 3.660 |
| 2 | 0.649 | 1.591 |
| 3 | 0.793 | 4.104 |
| 4 | 1.863 | 3.485 |

**In 4/4 seeds, cognition-OFF retention is substantially higher than
cognition-ON retention for the same seed.** Cognition-OFF: culture gains
share in all 4 seeds (retention 1.59-4.10). Cognition-ON: culture declines
in 3/4 seeds (0.65-0.79) and grows only modestly in seed 4 (1.86) — the one
ON-seed that doesn't show exclusion, though still far below its own
OFF-seed counterpart (1.863 vs. 3.485).

This is a matched, within-seed, n=4 confirmation: the cognition subsystem
causally and robustly controls the cultural invasion margin. It is not
merely correlated with some other varying condition — for each seed, only
the `cognition_enabled` flag changes between the ON and OFF run.

## 6. Two instrument-improvement findings

Both surfaced independently in Task 5 and reconfirmed in Task 6/7; recorded
here as findings for future milestones, not implemented in this one.

1. **Absolute-count `invasion_fitness` is confounded by non-stationary
   population.** It reports a log-growth trend on raw mutant count, valid
   only when total world population is roughly stationary. It was not, in
   half or more of the runs across Tasks 5 and 6 (population collapse to
   zero, transient dips, or pinning at the cap under cognition-off). This
   produced at least one directly misleading read (Task 6, cognition-seed-1:
   abs r says "worse," share retention says "3.76x better"). The tool
   should offer a frequency/share-relative variant (log-growth of mutant
   *share*, or share-slope while `freq ≤ threshold`, as manually reproduced
   for this milestone in `invasion-share-analysis.csv` and
   `lever-share-retention.csv`) as a first-class output.

2. **The cultural/asocial strategy tag is not lineage-locked.** It is a
   per-tick readout of Communicator-module presence, which is recombined
   and mutated at every birth (`crossover_and_mutate`,
   `crates/anabios-core/src/module/mod.rs`, invoked from
   `crates/anabios-core/src/reproduce.rs`). This confounded the Task-5
   aso→cul seed-1 result (a genuine 16-window lineage extinction followed by
   a mutation-seeded reappearance, indistinguishable from true invasion
   under the current tag). A clean bidirectional invasion test needs a
   heritable "founder population" ID assigned at scenario start and
   inherited (not recombined/mutated) across generations, separate from the
   mutable module-presence phenotype.

## 7. Handoff to O2/O3

O2 (lifetime learning) and O3 (make culture pay) should target the
cognition-culture cost coupling first: the dominant, causally-confirmed
lever is the IQ tax/gating that cultural (Communicator-bearing) agents pay
under `cognition_enabled=true`, so reducing culture's IQ tax and/or
decoupling culture's skill benefit from the IQ gate — letting culture pay
for itself at low eras rather than being squeezed out before era
progression is even reachable — is the highest-leverage next move. Fix the
two instrument issues from §6 before O3's emergent-era-3 attempt, since a
share-relative invasion-fitness output and a lineage-locked founder tag
will both be needed to cleanly measure whether O3's changes actually let
culture establish and climb, rather than re-litigating the same confounds
this milestone had to work around manually. And note that the era-3 IQ
ceiling itself remains untested, not disproven, as a future constraint —
it simply isn't the *current* blocker; once O2/O3 changes let culture
establish population share and era progression becomes reachable, the
era-3 gate (0.55) may yet turn out to matter, but this milestone only shows
it plays no role in the world as currently tuned.

## Artifacts

- Scenarios: `scenarios/o1-invasion-cultural-into-asocial.toml`,
  `scenarios/o1-invasion-asocial-into-cultural.toml`,
  `scenarios/o1-lever-{density,ceiling,cognition,mixing}.toml`.
- Data: `docs/superpowers/data/o1/ooa-baseline-ledger.csv`,
  `inv-{cul-into-aso,aso-into-cul}-{318,1,2}.csv`, `invasion-share-analysis.csv`,
  `lever-{density,ceiling,cognition,mixing}-{1,2}.csv`,
  `lever-share-retention.csv`, `confirm-cogON-{1,2,3,4}.csv`/`confirm-cogOFF-{1,2,3,4}.csv`
  (seeds 1-2 ON/OFF pairs live in `lever-cognition-{1,2}.csv` and
  `cognition-1`/`2` rows; seeds 3-4 are the dedicated `confirm-*` files),
  `cognition-confirmation.csv`.
- Task reports: `.superpowers/sdd/2026-08-02-o1-exclusion-autopsy/task-{4,5,6}-report.md`.
