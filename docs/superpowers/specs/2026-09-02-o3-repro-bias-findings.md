# O3 findings: repro-biased learning — culture becomes viable as a strategy; the founder bar stands unmet

**Date:** 2026-09-02. **Milestone:** O3 (roadmap Horizon 1 flagship).
**Design + pre-registration:**
[`2026-09-02-o3-corrected-apparatus-repro-bias-design.md`](2026-09-02-o3-corrected-apparatus-repro-bias-design.md)
(v1.1 vertical addendum pre-registered in
[`../data/o3/probe-log.md`](../data/o3/probe-log.md)).
**Data:** `docs/superpowers/data/o3/o3-measurement-summary.csv` (all 120
runs), raw ledgers in `docs/superpowers/data/o3/raw/`. Protocol throughout:
`autopsy --mutant cultural`, 2500 ticks, window 500, seeds 1–10, on the
diet-matched apparatus (`scenarios/experiments/o3-invasion-cultural-into-omnivore.toml`),
practices present unless stated.

## Headline

1. **The O1/O2a apparatus was diet-confounded at HEAD** (apes-only reclass
   `8dd9239` made the cultural mutant an omnivore among herbivore
   residents). On the corrected, diet-matched apparatus, culture is still
   excluded — but mildly (founder share-r **-0.087**, 10/10 negative, no
   extinctions), not catastrophically (O2a's -0.778 was mostly the diet gap).
2. **Food-side niche construction is null with a mechanistic explanation**
   (both canonical O3 levers, two doses each): with `reproduction_threshold
   = 0.2` and population pinned at cap, energy does not bind founder share.
   The margin lives on the **birth/death ledger**, where the
   maladaptive-practice birth tax is the dominant term (practices-off flips
   founder share-r > 0 in 6/10 seeds, mean +0.066).
3. **The shipped mechanism, `repro_biased_learning`** (content-bias-only
   practice rejection keyed on observed birth outcomes; horizontal adoption
   filter + vertical inheritance filter; no model bias), **makes the
   cultural strategy decisively viable** — but does **not** rescue the
   seeded founder lineage, so the pre-registered founder bar is honestly
   **missed**.

## The two-level result (the finding that matters)

| readout (n=10, paired seeds) | baseline | repro-bias v1.1 |
|---|---|---|
| **founder lineage** share-r > 0 | 0/10 (mean -0.087) | 1/10 (mean -0.122) — **bar missed** |
| **phenotype (module tag)** share-r > 0 | 9/10 (mean +0.321) | 9/10 (mean **+0.469**) |
| phenotype growth ×(t500→t2500) | ×3.05 | ×**4.66** (v1: ×5.50) |
| phenotype end share of world | 9.2% | **15.3%** (v1: 17.1%; up to 42% single-seed) |
| practice saturation among Communicators (seed-1 diag, t2000) | ~95% (118–123/126) | ~20–25% (82–102/428) |
| founder mean_skill@2000 (adoption control) | 0.409 | 0.498 — **not suppressed** |

Raw module-tag share-r is inflated in *both* conditions by mutational
inflow (`ADD_MODULE_PROB = 0.02` mints Communicators from the resident base
continuously — the O2a drift channel), so the honest strategy-level claim
is the **paired contrast**: Δshare-r +0.15, growth ×1.5, end-share +6
points, practice burden collapsed ~4×, with skill transmission clean. The
attribution is confound-guarded: one flag apart, same seeds, diet-matched
apparatus, practices present.

**Why the founders specifically are not rescued:** the mechanism raises the
selective value of the Communicator *phenotype*, and the phenotype's entrant
flow is dominated by resident-lineage module mutation (~1000-agent base ×
0.02/birth) over founder descent (20 agents) by roughly 50:1. The `[o3diag]`
instrument also shows the early-window trap: practice 1 reaches 52/66
Communicators by t500, before any birth evidence exists — evidence-gated
filters cannot act during initial fixation, and the founder cluster is
exactly where that early fixation happens. The v1.1 vertical filter (children
of bereaved families decline the custom) fixes the *population-level* burden
but arrives too late for the founders' first generations too.

## Verdicts against the pre-registered bars

- **Bar (founder share-r > 0 in a majority of seeds): NOT met** — v1: 2/10;
  v1.1: 1/10. Recorded as a negative per the roadmap's done-when.
- **Control (adoption not suppressed): met** (skill 0.498 vs 0.409 — the
  O2b failure mode does not recur).
- **Ceiling reference:** practices-off reaches 6/10 founder-positive; the
  transmission filters do not reach it on the founder metric despite
  *exceeding* it at the phenotype level — deleting the antagonist helps the
  founders directly; filtering its spread helps whoever joins culture next.

## Interpretation for the O-track

The arc's question was "can culture stop losing?" On the corrected
apparatus, with this flag: **the cultural strategy stops losing** — its
population share roughly doubles under selection with the birth tax still
present in the world. What does *not* happen is dynastic rescue: the 20
seeded founders are not the beneficiaries, because culture in this substrate
propagates primarily by recurrent module mutation into a now-favorable
niche, not by founder demography. "Culture wins" and "the founder lineage
wins" have come apart — the founder-lineage instrument (built in O2 Step 0
to kill drift artifacts) is the right tool for *invasion* claims and the
wrong sole criterion for *selection-on-phenotype* claims. Future O-track
bars should state which of the two they mean.

## What ships

- `repro_biased_learning` flag (Scenario + World, default false),
  `AgentBuffers.births_ok`/`births_failed` (serialized, flag-gated counting),
  horizontal + vertical content bias, FORMAT_VERSION 34→35 (layout growth;
  determinism/cognition/inventions/affect×3 goldens regenerated; flag-off
  byte-identical), round-trip coverage, 4 culture tests + 2 reproduction
  tests, `omnivore_forager` control archetype, 3 experiment scenarios, the
  env-gated `[o3diag]` autopsy instrument. Full workspace release suite
  green (40 suites).
- The fertility/tier probe scaffolding was throwaway (commit `dcfa222`);
  removed before merge — reproduce probe rows from that commit.

## Era-climb probe (grand run, same session)

`scenarios/out-of-africa.toml` (unseeded) ± the flag
(`scenarios/experiments/o3-ooa-repro-bias.toml`), seeds {318,1,2,3,4,5},
20000 ticks, module tag (`runs → data/o3/raw/ooa-*`):

| seed | baseline end cultural share | repro-bias end cultural share |
|---|---|---|
| 318 | 0.000 (extinct) | 0.007 |
| 1 | 0.003 | 0.003 |
| 2 | 0.006 | **0.688** (n=2059) |
| 3 | 0.001 | **0.942** (n=2824) |
| 4 | 0.011 | 0.001 |
| 5 | 0.014 | **0.138** |

**The ecological exclusion of the OoA grand run is breakable**: 0/6 →
**3/6 seeds end culture-dominant**, up to a 94%-cultural world sustained to
t20000. This is the first committed-code lever that flips the grand run's
outcome (`2026-08-02-ooa-climb-findings` recorded it as unreachable).

**The era climb still does not start** — peak `max_era` ≤ 1 in all 12 runs,
era 0 in the dominant worlds — and the `[o3diag]` ape-count shows why: the
winning cultural population is **non-ape** (`comm_ape` 241 → 0 by t12000
while `comm` holds ~1000-1800 in the seed-2 dominant run). Every
`communicator_kit` cohort and every mutation-minted Communicator from
herbivore stock is outside the `is_ape` band, so `enforce_ape_only` zeroes
their invention channels — the culture that wins is invention-incapable by
composition. The ape cultural cohorts (innovator/traditionalist/
cultural_forager) pay the omnivore-diet + large-size tax against herbivore
communicators who enjoy culture's benefits without it, and die out.

Secondary observation (mechanism behaving as designed): late in the
dominant run, Inbreeding re-saturates the winners (1002/1006 holders at
t10000) while Child Sacrifice stays suppressed (~1%) — in a large outbred
population Inbreeding's closeness-scaled cost is near zero, so there is no
birth-failure evidence against it and the *evidence-based* filter honestly
permits it. The bias suppresses customs in proportion to their observed
harm, not categorically.

**Blocker handoff:** the climb's binding constraint has moved from
*ecological exclusion of culture* (broken here) to **ape-composition of the
surviving culture** — the next lever must make the ape cultural lineages
the ones that flourish (an income stream or niche non-apes cannot touch),
measured on ape-communicator share, before era progression is testable.

## Honest next moves (not started here)

1. **Founder-rescue variant, if wanted:** the binding factor is the
   evidence-free early window. A prior (innate wariness of practices whose
   *variant* is young/unproven, or evidence shared via the E9 institutional
   memory rather than per-agent counters) could act before fixation —
   pre-register on the same protocol before building.
2. **Era-climb re-attempt (OoA-Earth stage 2):** with the birth tax
   neutralized at the strategy level, re-measure whether era progression
   becomes reachable — the probe evidence says resource-tier levers only
   matter after this fix, so the ordering is now testable.
3. **`SelectiveLearning` detector:** the mechanism measures positive at the
   strategy level; if it ships as part of the era-climb attempt, add the
   detector with the standard evidence trio (deferred here to keep this PR's
   scope to the adjudicated mechanism).
