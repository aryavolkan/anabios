# O2a findings: the O1 invasion/lever matrix, re-measured with fixed instruments

**Date:** 2026-08-07
**Milestone:** O2a (corrected measurement), part of the O-track in the
[detailed roadmap](2026-08-07-detailed-roadmap.md) §4.1. Follows O2 Step 0
(`docs/superpowers/data/o2/step0-validation.md`), which built the instruments
this measurement uses.
**Protocol:** every run is `anabios-headless autopsy --tag founder` (the
lineage-locked founder tag) with both `invasion_fitness` (absolute) and
`invasion_fitness_share` (population-change-robust) recorded. Scenarios are
the committed O1 files under `scenarios/experiments/`, unmodified. All raw
ledgers (`*.csv`) and stdout captures (`*.log`) are in
`docs/superpowers/data/o2/o2a/`; summary tables are the sibling
`o2a-*-founder.csv` files.

O1 measured this matrix with the per-tick Communicator-module tag and showed
(Step 0 validation) that the tag over-counts "cultural" agents ~5× by
crediting asocial lineages that mutate a Communicator module. This document
re-runs the load-bearing claims under the lineage-locked tag: the n=10
practices decomposition, the n=4 cognition confirmation, and the 3-seed
bidirectional invasion.

---

## 1. Headline corrections

| O1 claim (module tag) | O2a result (founder tag) | Verdict |
|---|---|---|
| Practices-off invade-fraction **90%** (vs 40% baseline) | **0/10 in BOTH conditions** — the cultural founder lineage never gains share while rare at the 2000-tick horizon | **Not reproduced.** Direction holds (see §2), magnitude was a tag artifact |
| Cognition-OFF reverses exclusion, **4/4** seeds gain share | **1/4** seeds gain share (seed 4, retention 3.99, share-r +0.262); 2/4 less-negative; 1/4 unchanged-extinct | **Weakened, not reversed** |
| Asocial **invades** cultural-resident worlds (seeds 1, 2 up_invades) | Asocial founder lineage **extinct 3/3** — the "invasion" was entirely module mutation off the cultural resident line | **Not reproduced — artifact confirmed** |
| Cultural excluded from asocial worlds, 3/3 unconfounded | Cultural founder lineage **extinct 3/3** (shares collapse within the first 2–4 windows) | **Reproduced and strengthened** |

**The symmetric picture the module tag hid:** in the dense mixed cradle,
*neither* strategy invades the other — every rare founder lineage, in both
directions, all six seeds, goes extinct. The O1 asymmetry ("culture can't
invade, asocial can") was real for the cultural direction and fabricated by
module mutation for the asocial direction. What actually distinguishes the
strategies is not invasibility but **resident robustness**: asocial-resident
worlds hold ~3000 agents to t20000 in 2/3 seeds (collapse in 1); cultural-
resident worlds collapse entirely in 2/3 seeds and limp at 84 agents in the
third. Culture's problem is demographic fragility as a *resident*, not
failure as an *invader*.

## 2. Corrected practices decomposition (n=10, ticks=2500, window=500)

Metric: cultural-founder share retention `f2000/f500` plus both invasion
fitnesses. Data: `o2a-practices-decomposition-founder.csv`.

| condition | invade-fraction (retention > 1) | mean retention | mean share-r | extinct by t2000 |
|---|---|---|---|---|
| baseline (practices ON) | **0/10** | 0.079 | **-0.778** | 3/10 |
| practices OFF | **0/10** | 0.244 | **-0.583** | 2/10 |

The O1 module-tag table read 40% / 90% on this exact contrast. Under the
founder tag **no seed in either condition gains share** — the "invading
cultural" agents O1 counted were asocial-lineage module mutants. The lever's
*direction* survives intact and is visible in three independent reads:

- mean retention 3.1× higher with practices off (0.244 vs 0.079);
- mean share-fitness less negative by +0.195 (-0.583 vs -0.778);
- the two best runs overall are both practices-off (seeds 3, 4: share-r
  -0.156 / -0.083, 12 and 16 founder descendants at t2000 and still roughly
  holding, vs a baseline best of 4).

**Corrected mechanism statement:** disabling maladaptive practices does not
let culture *invade*; it slows the cultural founder lineage's exclusion and,
in the best seeds, buys it a small persistent foothold. The O1 §6 magnitude
(40%→90%) was tag drift; the O1 §6 *direction and mechanism ranking*
(practices is the dominant lever) is consistent with what survives here.

## 3. Corrected cognition confirmation (n=4, ticks=2500)

Data: `o2a-cognition-confirmation-founder.csv` (cognition-ON rows are the
seed-1..4 decomposition baseline runs — same scenario, same tag).

| seed | ON retention | OFF retention | OFF share-r | OFF founders @t2500 |
|---|---|---|---|---|
| 1 | 0.000 | 0.000 | -0.936 | 0 (extinct) |
| 2 | 0.048 | 0.239 | -0.399 | 3 |
| 3 | 0.060 | 0.277 | -0.402 | 5 |
| 4 | 0.194 | **3.985** | **+0.262 INVADES** | 57 |

O1 read 4/4 cognition-OFF share gains (1.59–4.10). Founder-tag: **1/4**
genuine gains. Seed 4 is the single run in the entire O2a matrix where a
rare cultural founder lineage grows while rare (20 → 80 descendants by
t2000, positive share-fitness) — proof the outcome is *reachable*, not the
default. Removing the whole cognition subsystem (IQ cost + gates +
practices) is strictly stronger than removing practices alone, and seed 4
shows the compounded effect crossing zero — consistent with O1's lever
*ranking*, again at a fraction of the claimed magnitude.

## 4. Corrected bidirectional invasion (seeds 318/1/2, 20000 ticks)

Data: `o2a-bidirectional-invasion-founder.csv`.

| direction | seed | mutant founder fate | resident fate @t20000 |
|---|---|---|---|
| cul→aso | 318 | extinct (by t~2500) | world collapse @t15000 |
| cul→aso | 1 | extinct | asocial stable, 2997 |
| cul→aso | 2 | extinct | asocial stable, 2997 |
| aso→cul | 318 | extinct | world collapse @t7500 |
| aso→cul | 1 | extinct | cultural limps, 84 |
| aso→cul | 2 | extinct | world collapse @t8500 |

- O1's cultural-excluded 3/3 is reproduced cleanly — in fact the founder tag
  shows the exclusion is *faster* than the module tag suggested (share
  collapses within 2–4 windows of t500).
- O1's two "asocial invades" seeds are resolved as pure module-mutation
  artifacts: the asocial founder lineage is extinct in all three seeds; the
  late-run "asocial" counts in the module-tag ledgers were cultural-resident
  descendants that mutated/lost modules. O1 §2's own parsimony argument
  suspected exactly this; the founder tag confirms it directly.
- New, cleaner framing for the demographic fragility O1 noted: it is a
  **cultural-resident** property (3/3 cultural-resident worlds collapse or
  nearly collapse; only 1/3 asocial-resident worlds do). A rare lineage of
  *either* strategy dies at density; only an asocial *resident* reliably
  persists.

## 5. Implications for O2b (payoff-biased learning)

1. **The bar is lower and better-defined than O2b's design assumed.** The
   pre-registered criterion should not be "reproduce the 90% invade-fraction"
   — that number never existed. The honest target: flip cultural-founder
   share-fitness from reliably negative (best O2a run without levers:
   -0.412) to **> 0 in a majority of seeds**, with practices *present* in
   the world. Cognition-off seed 4 (+0.262) and practices-off seeds 3/4
   (-0.156, -0.083) bracket what partial mechanisms achieve.
2. **Measure with `--tag founder` + `invasion_fitness_share` only.** The
   module tag is now quantifiably misleading on exactly the contrast O2b
   will run (payoff-biased transmission changes *who carries modules*, which
   is precisely the drift channel).
3. **Resident fragility is the deeper problem.** Even a perfect
   transmission filter leaves culture as a collapse-prone resident (§4).
   If O2b's payoff-biased learning fixes invasion but cultural-resident
   worlds still collapse, the OoA story needs a demographic mechanism, not
   just a learning one. Worth one diagnostic run in O2b: a cultural-resident
   world under the new flag.

## Artifacts

- Summary tables: `docs/superpowers/data/o2/o2a-practices-decomposition-founder.csv`,
  `o2a-cognition-confirmation-founder.csv`, `o2a-bidirectional-invasion-founder.csv`.
- Raw ledgers + stdout captures: `docs/superpowers/data/o2/o2a/` (34 runs:
  20 decomposition, 8 cognition confirmation incl. reused ON rows, 6
  bidirectional invasion).
- Instruments: `crates/anabios-headless/src/founder.rs`,
  `ledger::invasion_fitness_share` (O2 Step 0, plan archived).
- Prior: `docs/superpowers/data/o2/step0-validation.md` (instrument
  validation), `2026-08-03-o1-exclusion-findings.md` §6 correction notice.
