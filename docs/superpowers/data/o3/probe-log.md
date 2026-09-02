# O3 probe log — cultural niche construction (running notes)

**Date started:** 2026-09-02. Branch `claude/o3-niche-construction` (worktree).
Protocol: `anabios-headless autopsy --tag founder --mutant cultural`, 2500
ticks, window 500, n=10 seeds (1–10), metrics = `invasion_fitness_share`
(share-r), retention `f2000/f500`, founder counts. Probes are throwaway
env-gated knobs in `probe_o3.rs` (exact identity when unset; determinism
goldens verified green with probes compiled in).

## Finding 1 — the O1/O2a apparatus is diet-confounded at HEAD

Commit `8dd9239` (2026-08-11, apes-only inventions) reclassed
`cultural_forager` as an omnivore (`make_omnivore`: Mouth `diet_affinity =
0.5`) while the `asocial_forager` resident stayed herbivore. The O1 invasion
scenario is therefore no longer one-variable-apart: the mutant differs by
Communicator AND trophic niche.

Measured consequence (`runs/o3-probe/`, o1-invasion-cultural-into-asocial,
n=10): the "cultural" founder lineage is no longer excluded — mean share-r
**+0.140** (vs O2a's **-0.778**), 5/10 seeds retention>1, 0/10 extinct
(O2a: 3/10), founder counts up to 233 at t2000. Cultural founders' mean_era
stays 0.000 throughout — no inventions fire — so the persistence is
trophic (omnivory → scavenging/predation access), not cultural.

## Finding 2 — diet-matched baseline: exclusion survives, but mild

New control archetype `omnivore_forager` (starter_kit + make_omnivore, no
Communicator) + `scenarios/experiments/o3-invasion-cultural-into-omnivore.toml`
(resident=omnivore_forager, all else verbatim from the O1 file). Now the
mutant differs by the Communicator alone.

Matched baseline (`runs/o3-matched/baseline-*`, n=10): share-r < 0 in
**10/10 seeds**, mean **-0.087**, mean retention 0.811, 0/10 extinct,
founders decline 20 → ~11-17 by t2500. Culture is consistently but *gently*
excluded — the O2a catastrophic exclusion (-0.778, extinctions) was mostly
the diet gap, not culture's cost. The O3 target on the corrected apparatus:
flip share-r > 0 in a majority of seeds.

## Finding 3 — probe round 1 on the matched apparatus

- **Lever A v1** (skilled Communicators enrich cell fertility on successful
  graze; GAIN=0.01, SKILL_MIN=0.5, CAP=3.0): **misfired** — bit-identical to
  baseline in 8/10 seeds. Cause: with `env_period=400` skill learn-by-doing
  is disabled (`env_period == 0` gate in feed_pass), cultural mean_skill
  plateaus ~0.13, below the 0.5 deposit gate. The 2 seeds where it did fire
  (4, 5): seed 4 flipped positive (+0.147, retention 1.30), seed 5 ~null.
- **Lever B v1** (Communicator-only bite ×1.5): ~null on the matched
  apparatus (mean -0.067 vs baseline -0.087); on the *unmatched* apparatus it
  was actively harmful (mean -0.093 vs +0.140 — biggest damage in the best
  seeds, e.g. seed 9 +1.81 → +0.03). Hypothesis: extra bite accelerates
  local depletion of the shared cells (the free-rider commons), hurting the
  clustered founders themselves.

## Finding 4 — probe round 2: both food levers are null, with a mechanism

- **Lever A v2** (gate opened, SKILL_MIN=0.0 — every Communicator deposits;
  runs now diverge from baseline in all seeds): mean share-r **-0.092** vs
  baseline -0.087, 0/10 positive. Null. The constructed fertility is ~98%
  free-ridden by the omnivore residents sharing the cradle cells.
- **Lever B v2** (×2.0 bite tier): mean **-0.098** — flat-to-negative dose
  response. Null.
- **Why food can't work here:** `reproduction_threshold = 0.2` and the
  population pinned at `max_population = 3000` mean energy is not the
  binding constraint on founder share (residents' mean energy *rises*
  through the run; founders reproduce freely regardless). Share at cap is
  decided on the birth/death ledger, not the energy ledger. Founders'
  energy does bleed (38.6→28.5 while residents 30.6→64.4) — Communicator
  upkeep (+0.005/tick differential) plus practice costs — but subsidizing
  the energy side leaves the margin unmoved.

## Finding 5 — the birth-tax diagnosis: practices-off flips the margin

`o3-lever-practices-off-matched.toml` (matched apparatus, one variable:
`practices_enabled = false`), n=10 (`runs/o3-matched/practoff-*`):
share-r > 0 in **6/10 seeds**, mean **+0.066** (baseline: 0/10, -0.087);
9/10 seeds improve pairwise (mean Δ +0.153); best seed +0.398 with founders
20→114. On the diet-matched apparatus the maladaptive-practice **birth tax**
(Inbreeding stillbirths, Child-Sacrifice culls — culture-only, payoff-blind
transmission) is the dominant driver of the remaining exclusion, exactly
O1 §6's antagonist in its honest, corrected magnitude.

## Adjudication → the O3 mechanism

Food-side niche construction (both canonical levers from the arc spec) is
measured null with a mechanistic explanation. The lever that moves the
corrected margin acts on the *birth* ledger. The honest flagged mechanism —
prescribed by the O2b handoff, never built — is payoff-biased learning keyed
on a **reproductive-success proxy** (the only fitness signal that can see a
stillbirth/cull cost), with practices still present in the world.

**Pre-registered bar (same protocol):** on
`o3-invasion-cultural-into-omnivore.toml`, flag on, practices present,
n=10 founder-tag: founder share-r **> 0 in a majority of seeds**, with the
practices-off run (+0.066, 6/10) as the honest ceiling reference, and an
"invention/skill adoption not suppressed" control (the O2b failure mode).

## Finding 6 — repro-bias v1 (horizontal-only): founder bar missed, strategy-level win

v1 measurement (`runs/o3-matched/reprobias-*`, n=10): founder share-r > 0 in
**2/10** (mean -0.079 vs baseline -0.087; paired Δ +0.007). **Bar missed.**
Controls clean: founder mean_skill@2000 0.498 vs baseline 0.409 — no
adoption suppression.

But the env-gated `[o3diag]` instrument (autopsy, `ANABIOS_O3_DIAG=1`) shows
the mechanism biting hard at the population level (seed 1): practice
saturation among Communicators drops from ~95% (118-123/126 baseline
holders at t2000) to ~20-25% (82-102/428), while the Communicator
population **quadruples** (109 → 437 at t2500). Module-tag n=10 confirms:
phenotype growth ×5.50 vs ×3.05, end-share 17.1% vs 9.2%, higher in 9/10
seeds (`runs/o3-matched/mod{base,rb}-*`). The benefit accrues to the
cultural *phenotype* (largely resident-derived Communicator mutants, ~50×
the founders' mutational inflow), not the seeded founder lineage.

**Identified bypass:** children are *born* holding practices
(`inherit_child_meme` parent-averages every channel), so the horizontal
adoption filter never sees the main transmission channel during the early
fixation window (52/66 Communicators hold practice 1 by t500, before any
birth evidence exists).

## Pre-registration addendum — v1.1 vertical content bias (2026-09-02)

Close the vertical bypass, same flag: at birth, when the parents' combined
observed `births_failed >= births_ok` **and** `births_failed > 0`, the child
declines (zeroes) every inherited practice channel — "the custom dies with
the grieving family." Parameter-free, deterministic, zero-only, applied
after `inherit_meme` so RNG draw counts are unchanged. Same n=10 protocol,
same bar (founder share-r > 0 in a majority of seeds), same skill control,
plus the strategy-level module-tag readout now recorded alongside.
