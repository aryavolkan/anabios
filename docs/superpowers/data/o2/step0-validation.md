# O2 Step 0 — Instrument validation against the O1 practices finding

**Date:** 2026-08-03
**Binary:** `target/release/anabios-headless autopsy` (commit `0150f41`)
**Protocol:** seeds 1/2/3, `--ticks 2500 --window 500 --tag founder --mutant cultural`
on both O1 scenarios. Two metrics printed per run: `invasion_fitness` (absolute
log-growth of the rare strategy) and `invasion_fitness_share` (share-relative,
population-change-robust). All raw ledgers are the sibling `step0-*.csv` files.

O1 baseline claim (from the plan): practices ON → cultural **excluded** (~40%
invade-fraction); practices OFF → cultural **invades** (~90%).

---

## 1. Both metrics, per condition / seed (`--tag founder`)

| Condition (scenario)            | Seed | `invasion_fitness` (abs) | `invasion_fitness_share` | Verdict  | Cultural founder count @ tick 2000 / 2500 |
|---------------------------------|------|--------------------------|--------------------------|----------|-------------------------------------------|
| baseline — practices **ON**     | 1    | -0.97296                 | -0.97346                 | EXCLUDED | 0 / 0 (extinct)                           |
| baseline — practices **ON**     | 2    | -0.80472                 | -0.80472                 | EXCLUDED | 1 / 1                                      |
| baseline — practices **ON**     | 3    | -0.74893                 | -0.65154                 | EXCLUDED | 1 / 1                                      |
| lever — practices **OFF**       | 1    | -1.07974                 | -1.03031                 | EXCLUDED | 0 / 0 (extinct)                           |
| lever — practices **OFF**       | 2    | -0.76753                 | -0.63932                 | EXCLUDED | 2 / 0 (extinct)                           |
| lever — practices **OFF**       | 3    | -0.20906                 | -0.15628                 | EXCLUDED | 12 / 13 (persists)                        |

**Means:**

| Condition       | mean abs `r` | mean share `r` |
|-----------------|--------------|----------------|
| baseline (ON)   | **-0.8422**  | **-0.8099**    |
| practices (OFF) | **-0.6854**  | **-0.6086**    |

Share delta (OFF − ON) = **+0.2013** → on the mean, culture does *relatively
better* with practices OFF. Direction of the O1 lever effect is preserved.

`share` vs `abs`: identical or near-identical except where the total population
is shrinking (asocial declines under the lever). The gap is largest exactly
there — practices-off-3 (abs -0.209 vs share -0.156) and baseline-3
(abs -0.749 vs share -0.652) — i.e. the share metric is doing its job of not
crediting the mutant for a population it merely failed to shrink alongside. It
does not flip any verdict.

---

## 2. Module-vs-founder tag drift (baseline seed 1)

Same run, only the strategy key differs. Cultural counts per window:

| tick | `--tag module` | `--tag founder` | drift (module − founder) |
|------|----------------|-----------------|--------------------------|
| 500  | 148            | 28              | +120                     |
| 1000 | 114            | 9               | +105                     |
| 1500 | 142            | 4               | +138                     |
| 2000 | 109            | 0 (extinct)     | **+109**                 |
| 2500 | 79             | 0 (extinct)     | +79                      |

Reported fitness for the same run:

| tag     | abs `r`  | share `r` | reads as                          |
|---------|----------|-----------|-----------------------------------|
| module  | -0.15694 | -0.15711  | "mild exclusion, culture hangs on"|
| founder | -0.97296 | -0.97346  | "decisive exclusion, lineage dies"|

**Drift is severe and it flips the qualitative reading.** The seeded cultural
founder lineage is entirely extinct by tick 2000, yet the module tag still counts
109 "cultural" agents — asocial-lineage agents that acquired the Communicator
module by mutation/learning, not descendants of the culture founders. Even at
tick 500 the module tag over-counts ~5× (148 vs 28). The old tag's apparent
persistence (`r ≈ -0.16`) is almost entirely module contamination.

---

## 3. Verdict: **CONCLUSION PARTIALLY CHANGES — reported prominently**

1. **Lever direction holds (weakly).** Mean share-fitness is less negative with
   practices OFF (-0.609) than baseline ON (-0.810), Δ = +0.20. So culture does
   relatively better without practices — consistent with O1's *direction*.

2. **The strong O1 magnitude does NOT reproduce.** Under the clean founder tag,
   culture is **EXCLUDED in all six runs** (all `r < 0`); it never invades. Final
   founder counts are 0/1/1 (ON) and 0/0/13 (OFF). Only practices-off seed 3
   *persists* (13 agents, rising energy 144 > asocial 128) — persistence, not the
   "~90% invasion" the O1 framing asserts. The result is also noisy: practices-off
   seed 1 is the single most-negative run of all six.

3. **The module tag was materially inflating cultural counts** (109 vs 0 at tick
   2000). Any O1 invade-fraction computed on the module tag is suspect and likely
   over-stated the cultural strategy's success. The founder tag is the correct
   instrument going forward.

**Bottom line for O2a/O2b:** the fixed instruments are trustworthy and behave as
designed (share metric cancels the pop-change confound; founder tag removes module
drift). But they reveal the O1 "culture invades at ~90% with practices off"
headline is **not reproduced** at 2500 ticks / 3 seeds — culture is excluded
throughout, doing only relatively (not absolutely) better with practices off.
Downstream work should re-derive the O1 invade-fraction with `--tag founder`
before relying on it, and consider whether a longer horizon than 2500 ticks is
needed (practices-off-3's founder lineage was still climbing at cutoff).

### Caveats

- 3 seeds per condition is a small sample; the O1 "~40% / ~90% invade-fraction"
  is a many-replicate statistic and is not directly the single-run log-growth
  `r` measured here. This validation tests instrument behavior and the lever
  *direction*, not a precise invade-fraction reproduction.
- 2500 ticks may be short: practices-off-3 shows the founder lineage persisting
  and gaining energy at the cutoff. A longer run could change the persistence
  picture (though not the module-drift finding).
