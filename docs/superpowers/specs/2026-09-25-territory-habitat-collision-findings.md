# Territory/habitat/collision layer: performance bench + measurement probe findings

**Date:** 2026-09-25. **Task:** 9 (performance bench + measurement probe + constant
tuning), closing out the territory/habitat/collision layer (Tasks 2–8).
**Design:** [`2026-09-25-territory-habitat-collision-design.md`](2026-09-25-territory-habitat-collision-design.md).
**Scenario:** `scenarios/habitat-territories.toml` (flagship flag-on scenario;
1024-wide world, `max_population = 1500`, three grazer species differing only
in Locomotion: Land, Water, Air).

> **Superseded in part — see "Update 2026-09-25: `max_share` fix" at the
> bottom of this doc.** The diversity miss below (water/air survival) was
> diagnosed as a scenario authoring bug, not a substrate/constant problem: the
> three grazer specs shared one population cap with no per-lineage ceiling,
> so the fastest breeder always won. `max_share` was added to
> `habitat-territories.toml`; the update section has the corrected diagnosis,
> a new probe table, and the final per-target verdicts. The bench-overhead
> section below stands as the accepted, final result (controller ruling: no
> further tuning).

## Headline

Correctness holds (zero habitat violations across 24 probe-seed runs spanning
three constant configurations), but two of the four measurement targets miss
even after the one tuning attempt each target's guidance prescribed:

1. **Tick overhead**: `territory_enabled` costs **~19–24%** per 10k-agent
   tick, not the ≤10% budget. The prescribed fix (`RESOLVE_PASSES` 2→1) was
   tried; it only trimmed the overhead to ~18–20% while making
   `deep_overlaps` roughly 5× worse (up to 204 vs. a baseline max of 41) — a
   worse trade than the miss it was meant to fix, so it was reverted.
2. **Species diversity**: `AQUATIC_CAPACITY` 4.0→6.0 (the prescribed lever for
   aquatic collapse) roughly doubled the seeds where Water lineages survive to
   20k ticks and rescued one outright extinction, but landed at 5/8 seeds
   with `water > 0` (target: ≥6/8), and **Air locomotion survives in 0 of 24
   probe-seed runs across all three constant configurations tried**. This
   reads as ecological competitive exclusion of the smallest-founder-count,
   most resource-constrained niche (Air: 60 founders vs. 150 Land / 100
   Water, feeding only on land while roaming both land and sea) rather than a
   resource-capacity shortfall, so it is not something the two prescribed
   levers (`AQUATIC_CAPACITY`/`AQUATIC_REGROWTH_RATE`) can fix architecture-free.

Per the task's stop rule ("if a target misses and one reasonable constant
change doesn't fix it, stop tuning ... report DONE_WITH_CONCERNS"), tuning
stopped after one attempt per target.

## Constants changed

| Constant | File | Before | After | Kept? |
|---|---|---|---|---|
| `RESOLVE_PASSES` | `crates/anabios-core/src/collision.rs` | `2` | `1` (tried) | **No** — reverted to `2`; deep_overlaps regression (up to 204) outweighed the partial bench improvement (ratio only dropped from ~1.24 to ~1.19, still over budget) |
| `AQUATIC_CAPACITY` | `crates/anabios-core/src/biome.rs` | `4.0` | `6.0` | **Yes** — net improvement (fewer extinctions, better Water survival), doc comment updated (`0.4× Grass` → `0.6× Grass`) |

Net diff to `collision.rs`: none (round-tripped). `HABITAT_GOLDEN` in
`crates/anabios-core/tests/determinism.rs` was re-pinned for the
`AQUATIC_CAPACITY` change:

```
// before
const HABITAT_GOLDEN: &[(u64, u64)] =
    &[(0, 0x481c5068b56d95ee), (100, 0x1091b788d8c9aa78), (1000, 0x4687defce1d1bd61)];

// after (UPDATE_HASHES=1 cargo test -p anabios-core --release --test determinism
// habitat_territories_matches_golden_hashes -- --nocapture)
const HABITAT_GOLDEN: &[(u64, u64)] =
    &[(0, 0x277cfe66233e8bee), (100, 0xb4ba3b47063bee50), (1000, 0x375941eb60915cc0)];
```

Both flag-off trajectory guards
(`minimal_trajectory_unchanged_by_territory_substrate`,
`grand_theater_trajectory_unchanged_by_territory_substrate`) pass **untouched**
— expected, since `seed_aquatic`/`aquatic_regrow_step` (the only call sites of
`AQUATIC_CAPACITY`) are both gated on `world.territory_enabled`.

## Bench: `territory/on` vs `territory/off` (10k agents)

`cargo bench -p anabios-core --bench tick_bench -- territory`, `sample_size(20)`.
Founders relocated onto valid ground before timing so the comparison is
steady-state cost, not stranded-agent cost.

| Run | Constants | off median | on median | ratio (on/off) |
|---|---|---|---|---|
| 1 (pre-tuning) | `RESOLVE_PASSES=2`, `AQUATIC_CAPACITY=4.0` | 3.8115 ms | 4.7371 ms | **1.243** |
| 2 (pre-tuning) | same | 4.0060 ms | 4.9381 ms | **1.233** |
| 3 (tuned attempt) | `RESOLVE_PASSES=1`, `AQUATIC_CAPACITY=6.0` | 3.7861 ms | 4.5379 ms | 1.199 |
| 4 (tuned attempt) | same | 3.8368 ms | 4.5391 ms | 1.183 |
| 5 (final, reverted) | `RESOLVE_PASSES=2`, `AQUATIC_CAPACITY=6.0` | 3.9870 ms | 4.7601 ms | **1.194** |
| 6 (final, reverted) | same | 3.7813 ms | 4.7038 ms | **1.244** |

The committed state is runs 5–6 (`RESOLVE_PASSES` back to `2`;
`AQUATIC_CAPACITY` doesn't affect tick cost, only ecology). Across all four
`RESOLVE_PASSES=2` runs (1, 2, 5, 6) the ratio sits at **1.19–1.24**,
comfortably outside noise of the 1.10 target — this is a real, reproducible
~20% overhead, not thermal drift. `RESOLVE_PASSES=1` (runs 3–4) trims it to
~1.18–1.20, still over budget, at the cost described above.

**Acceptance: MISS.** `on` median is not ≤ 1.10 × `off` median under either
constant setting tried.

## Probe: 8 seeds × 20k ticks, `territory_measurement_probe`

`cargo test -p anabios-core --release --test invariants territory_measurement_probe -- --ignored --nocapture`

### Baseline (`RESOLVE_PASSES=2`, `AQUATIC_CAPACITY=4.0` — pre-tuning)

```
seed=1 alive=1498 land=1498 water=0 air=0 violations=0 deep_overlaps=9 inside_territory=98.4% water_cells_with_biomass=13240
seed=2 alive=1497 land=1497 water=0 air=0 violations=0 deep_overlaps=29 inside_territory=99.2% water_cells_with_biomass=11733
seed=3 alive=1498 land=0 water=1498 air=0 violations=0 deep_overlaps=1 inside_territory=85.7% water_cells_with_biomass=4007
seed=4 alive=1497 land=1497 water=0 air=0 violations=0 deep_overlaps=3 inside_territory=100.0% water_cells_with_biomass=11831
seed=5 alive=1495 land=0 water=1495 air=0 violations=0 deep_overlaps=3 inside_territory=87.3% water_cells_with_biomass=4846
seed=6 alive=0 land=0 water=0 air=0 violations=0 deep_overlaps=0 inside_territory=0.0% water_cells_with_biomass=12038
seed=7 alive=1499 land=1499 water=0 air=0 violations=0 deep_overlaps=5 inside_territory=95.3% water_cells_with_biomass=11483
seed=8 alive=1499 land=1499 water=0 air=0 violations=0 deep_overlaps=41 inside_territory=91.3% water_cells_with_biomass=11665
```

Runtime: 1216.26s.

### Tuning attempt (`RESOLVE_PASSES=1`, `AQUATIC_CAPACITY=6.0` — combined, later reverted)

```
seed=1 alive=1496 land=0 water=1496 air=0 violations=0 deep_overlaps=204 inside_territory=60.6% water_cells_with_biomass=12332
seed=2 alive=1498 land=0 water=1498 air=0 violations=0 deep_overlaps=36 inside_territory=92.2% water_cells_with_biomass=11140
seed=3 alive=1500 land=0 water=1500 air=0 violations=0 deep_overlaps=71 inside_territory=39.5% water_cells_with_biomass=8970
seed=4 alive=1496 land=1161 water=335 air=0 violations=0 deep_overlaps=26 inside_territory=87.0% water_cells_with_biomass=10581
seed=5 alive=1499 land=0 water=1499 air=0 violations=0 deep_overlaps=8 inside_territory=100.0% water_cells_with_biomass=11754
seed=6 alive=0 land=0 water=0 air=0 violations=0 deep_overlaps=0 inside_territory=0.0% water_cells_with_biomass=12038
seed=7 alive=1498 land=1498 water=0 air=0 violations=0 deep_overlaps=11 inside_territory=99.9% water_cells_with_biomass=11483
seed=8 alive=1500 land=0 water=1500 air=0 violations=0 deep_overlaps=11 inside_territory=95.5% water_cells_with_biomass=11022
```

Runtime: 388.58s (much faster than the baseline/final runs — single-pass
resolve is cheaper, consistent with the bench). `deep_overlaps=204` on seed 1
is why `RESOLVE_PASSES=1` was rejected.

### Final, committed (`RESOLVE_PASSES=2`, `AQUATIC_CAPACITY=6.0`)

```
seed=1 alive=1498 land=0 water=1498 air=0 violations=0 deep_overlaps=2 inside_territory=87.7% water_cells_with_biomass=11068
seed=2 alive=1499 land=1499 water=0 air=0 violations=0 deep_overlaps=4 inside_territory=94.8% water_cells_with_biomass=11733
seed=3 alive=1500 land=0 water=1500 air=0 violations=0 deep_overlaps=16 inside_territory=96.9% water_cells_with_biomass=11826
seed=4 alive=1499 land=0 water=1499 air=0 violations=0 deep_overlaps=4 inside_territory=20.4% water_cells_with_biomass=632
seed=5 alive=1498 land=1498 water=0 air=0 violations=0 deep_overlaps=7 inside_territory=99.6% water_cells_with_biomass=11836
seed=6 alive=1496 land=0 water=1496 air=0 violations=0 deep_overlaps=38 inside_territory=30.9% water_cells_with_biomass=11120
seed=7 alive=1500 land=0 water=1500 air=0 violations=0 deep_overlaps=0 inside_territory=99.9% water_cells_with_biomass=11395
seed=8 alive=1497 land=1497 water=0 air=0 violations=0 deep_overlaps=12 inside_territory=99.2% water_cells_with_biomass=11665
```

Runtime: 356.70s.

### Acceptance verdict (final, committed constants)

| Target | Result | Verdict |
|---|---|---|
| `violations == 0` every seed | 0 on all 8 seeds (all 3 configurations) | **PASS** |
| `deep_overlaps ≈ 0`, handful acceptable at 1500 agents | 0–38 (max 38, seed 6) | **PASS** (same order as the 0–41 baseline) |
| `inside_territory ≥ 80%` on most seeds | 6/8 ≥ 80% (seed 4: 20.4%, seed 6: 30.9% miss) | **MARGINAL** — "most" holds numerically (6/8) but two seeds newly miss where the pre-tuning baseline had 7/7 live seeds ≥ 80%. Both misses are seeds where `AQUATIC_CAPACITY=6.0` flipped the outcome from extinct (seed 6) or a different class mix to all-Water; territory cohesion in those runs hasn't caught up by tick 20k. |
| `water > 0` and `air > 0` on ≥ 6/8 seeds | water > 0 on 5/8 (seeds 1,3,4,6,7); air > 0 on **0/8** | **MISS** |

## Collision-grid scaling note

The bench and probe both run at the scenario's default 1024-wide world.
`UniformSpatialHash` for the collision layer (`COLLISION_CELL = 4.0`, dense
grid) allocates `(world_size / COLLISION_CELL)²` cells, so memory and rebuild
cost scale with world **area**: a 1024 world is `256×256 = 65,536` cells,
but an `ws=8192` world (64× the linear extent) would be `2048×2048 ≈ 4.2M`
cells, on the order of ~50 MB and a correspondingly heavier per-tick rebuild
— a real limit if the territory/collision layer is ever turned on for a
larger-scale scenario tier. Today only `habitat-territories.toml` (1024 wide)
enables the layer, so this doesn't bite yet, but it should gate any future
attempt to enable `territory_enabled` on a Huge/Vast-scale scenario.

## Verdict

The territory/habitat/collision layer is behaviorally correct at the
flagship scale (zero habitat violations, acceptable overlap counts, high
territory cohesion in 6 of 8 seeds) but ships with two open, honestly-missed
targets rather than further tuning: a genuine ~20% per-tick cost on 10k
agents that the one architecture-preserving lever available
(`RESOLVE_PASSES`) cannot buy down without unacceptably degrading collision
quality, and a diversity outcome where the Air niche — the smallest founder
population competing across both land and sea for a land-only food source —
never survives 20k ticks in any of 24 runs across three constant
configurations, a pattern more consistent with ecological competitive
exclusion (a known dynamic elsewhere in this codebase) than with a resource
constant this task is scoped to tune. Recommendation: **DONE_WITH_CONCERNS**;
both misses are recorded here rather than papered over, and neither blocks
merging Tasks 2–8's correctness work, which this task's probe corroborates.

(This section's diversity conclusion is superseded — see the update below.
The bench-overhead conclusion stands.)

## Update 2026-09-25: `max_share` fix (controller ruling)

### Diagnosis: a scenario authoring bug, not a substrate problem

Every probe seed above ended with **exactly one class alive at ≈
`max_population`** (1495–1500). That is the documented first-come
global-population-cap effect (`AgentSpec::max_share` doc,
`crates/anabios-core/src/scenario.rs:356`, and demonstrated in
`scenarios/riverlands.toml`): `habitat-territories.toml`'s three
`archetype = "grazer"` specs each get a fresh species id but set no
`max_share`, so all three race for the **one shared** `max_population = 1500`
ceiling — whichever lineage breeds up to the cap first blocks every other
lineage from ever being born again. Air losing 24/24 probe-seed runs (across
three constant configurations in the original findings above) is exactly
what this effect predicts: fewest founders (60 vs. 150 Land / 100 Water) ⇒
slowest to a share of the cap ⇒ first to be locked out.

### Fix

Added `max_share` to each `[[agents]]` spec in
`scenarios/habitat-territories.toml` (with a comment explaining why, citing
this doc): Land `0.45`, Water `0.35`, Air `0.20` (sums to 1.0, roughly the
founder-count ratio). `AQUATIC_CAPACITY` stays at `6.0`, `RESOLVE_PASSES`
stays at `2`, per the controller ruling — no further constant tuning.

### New probe (post-`max_share`, `AQUATIC_CAPACITY=6.0`, `RESOLVE_PASSES=2`)

```
seed=1 alive=524 land=0 water=524 air=0 violations=0 deep_overlaps=4 inside_territory=32.1% water_cells_with_biomass=11695
seed=2 alive=1198 land=673 water=525 air=0 violations=0 deep_overlaps=2 inside_territory=65.7% water_cells_with_biomass=4745
seed=3 alive=524 land=0 water=524 air=0 violations=0 deep_overlaps=0 inside_territory=18.3% water_cells_with_biomass=8819
seed=4 alive=525 land=0 water=525 air=0 violations=0 deep_overlaps=0 inside_territory=13.7% water_cells_with_biomass=147
seed=5 alive=1198 land=673 water=525 air=0 violations=0 deep_overlaps=22 inside_territory=62.9% water_cells_with_biomass=8487
seed=6 alive=525 land=0 water=525 air=0 violations=0 deep_overlaps=0 inside_territory=52.8% water_cells_with_biomass=9092
seed=7 alive=1199 land=674 water=525 air=0 violations=0 deep_overlaps=177 inside_territory=91.3% water_cells_with_biomass=11112
seed=8 alive=525 land=0 water=525 air=0 violations=0 deep_overlaps=1 inside_territory=22.1% water_cells_with_biomass=8048
```
Runtime: 173.07s.

`max_share` does exactly what it's documented to do: Water hits its 525-agent
ceiling (`0.35 × 1500`, rounded) on **every** seed, and Land reaches its
675-ceiling on the 3 seeds (2, 5, 7) where it doesn't go extinct outright
(672–674, one shy of the cap at the tick sampled). **Air is still extinct on
8/8 seeds** — worse than the ≥3/8 threshold that triggers the required
diagnosis below, so `max_share` alone does not rescue Air.

Two new observations, not present at the pre-`max_share` baseline:
- `deep_overlaps` is mostly fine (0–22) but spikes to **177** on seed 7 — the
  one seed with the highest total live population (1199) and both Land and
  Water simultaneously near their ceilings. Not investigated further per the
  controller's scope (no additional constant tuning authorized this round).
- `inside_territory` drops sharply on most seeds (13.7–65.7%, only seed 7 at
  91.3%), well under the pre-`max_share` baseline's 85.7–100%. Hypothesis
  (not verified): reproduction throttling at the per-lineage cap creates more
  population churn near the cap boundary than the earlier winner-take-all
  dynamic did, and territory EMA (`TERRITORY_CENTRE_RATE = 0.1`) lags that
  churn. Recorded as an open observation, not tuned.

### Air diagnosis (required: Air still extinct ≥ 3/8 seeds)

Throwaway, uncommitted probe variant (`tests/tmp_air_diag.rs`, deleted after
use — no `src/` changes): seeds 1–2, checkpoint every 1000 ticks, reporting
Air population, the percentage of living Air agents standing over a Water
cell (`can_occupy` is unconditionally true for Air, so this measures wasted,
non-grazeable range — `can_graze` excludes Water for Air), and Air mean
energy.

```
=== seed 1 ===
tick=     0 land= 150 water= 100 air=  60 air_over_water%=  81.7 air_mean_energy=  50.000
tick=  1000 land= 629 water= 525 air= 125 air_over_water%=  37.6 air_mean_energy=  37.297
tick=  2000 land= 281 water= 524 air= 199 air_over_water%=  22.6 air_mean_energy=  37.370
tick=  3000 land= 115 water= 525 air= 172 air_over_water%=  43.0 air_mean_energy=  43.686
tick=  4000 land=  30 water= 516 air= 141 air_over_water%=  46.1 air_mean_energy=  42.200
tick=  5000 land=   3 water= 525 air=  58 air_over_water%=  65.5 air_mean_energy=  58.035
tick=  6000 land=   0 water= 525 air=  40 air_over_water%=  72.5 air_mean_energy=  44.907
tick=  7000 land=   0 water= 523 air=  31 air_over_water%=  71.0 air_mean_energy=  43.958
tick=  8000 land=   0 water= 525 air=  12 air_over_water%=  50.0 air_mean_energy=  29.551
tick=  9000 land=   0 water= 525 air=   2 air_over_water%=  50.0 air_mean_energy=  38.571
tick= 10000 land=   0 water= 525 air=   0 air_over_water%=   NaN air_mean_energy=     NaN
  (air extinct by tick 10000)
=== seed 2 ===
tick=     0 land= 150 water= 100 air=  60 air_over_water%=  61.7 air_mean_energy=  50.000
tick=  1000 land= 675 water= 525 air= 248 air_over_water%=  48.4 air_mean_energy=  27.090
tick=  2000 land= 613 water= 525 air=  94 air_over_water%=  55.3 air_mean_energy=  62.435
tick=  3000 land= 512 water= 525 air= 116 air_over_water%=  52.6 air_mean_energy=  88.290
tick=  4000 land= 674 water= 525 air=  61 air_over_water%=  68.9 air_mean_energy= 110.610
tick=  5000 land= 675 water= 525 air=  23 air_over_water%=  69.6 air_mean_energy= 202.152
tick=  6000 land= 674 water= 525 air=   7 air_over_water%= 100.0 air_mean_energy= 201.870
tick=  7000 land= 675 water= 524 air=   1 air_over_water%=   0.0 air_mean_energy= 440.510
tick=  8000 land= 675 water= 524 air=   0 air_over_water%=   NaN air_mean_energy=     NaN
  (air extinct by tick 8000)
```

**Root-cause hypothesis:** Air's extinction is not energy starvation in the
"can't find food anywhere" sense — mean energy among survivors is
comparable to or higher than at founding (peaking at 440 in seed 2 just
before the last Air agent dies) — it is a **shrinking-population survivorship
artifact layered on top of a structural niche disadvantage**:

1. **Structural disadvantage**: Air's habitat mask (`can_occupy`) is
   unconditionally true — it can fly over land or sea — but its food mask
   (`can_graze`) excludes Water. On this scenario's mostly-water world
   (`sea_level = 0.45`, `continentality = 0.8`), 62–82% of an Air agent's
   accessible range at any moment is water it cannot graze (`air_over_water%`
   starts at 61.7–81.7% at tick 0, when placement is still close to uniform).
   Land agents, by contrast, are relocated onto valid land at spawn and can
   never leave it, so 100% of their accessible range is grazeable. Air
   competes directly with Land for the *same* land-vegetation resource
   (`can_graze` puts Land and Air in the same class) while effectively
   halving-to-quartering its own foraging efficiency by spending most of its
   time over ungrazeable sea.
2. **Direct competition, not just a shared cap**: `max_share` caps *births*,
   not food access. Even where Land does not go extinct (seed 2), Air's
   standing population never approaches its 300-agent share ceiling (peaks at
   248, immediately declines) — Land simply outcompetes it for the shared
   land-forage pool long before either hits its population ceiling.
3. **Small-founder variance compounds it**: with only 60 Air founders (vs.
   150 Land / 100 Water) and a food disadvantage, the population is
   consistently on a shrinking trajectory (peak at tick 1000–2000, monotonic
   decline afterward in both seeds) — once numbers get low enough,
   demographic variance (no mate found, an unlucky predation-free-but-
   reproduction-starved run of ticks) finishes it off, which is why the
   survivors' mean energy can be *high* right up to the final individual.

This is consistent with the original findings' framing (ecological
competitive exclusion of the smallest, most resource-constrained niche) but
sharper: the mechanism is specifically the Air/Land shared-food-pool
competition amplified by a majority-water world, not a population-cap
artifact — `max_share` cannot fix it because the constraint that's binding is
food access, not birth-share. Fixing it for real would mean either giving Air
its own food source (architecture change, out of scope) or re-balancing the
scenario's land/water split or Air's founder count/placement (a scenario
change beyond this ruling's three `max_share` values) — flagged here, not
attempted, per the "tune constants only, stop after one attempt" rule.

### Updated per-target acceptance verdicts (final)

| Target | Result (post-`max_share`) | Verdict |
|---|---|---|
| `violations == 0` every seed | 0 on all 8 seeds | **PASS** |
| `deep_overlaps ≈ 0` (handful OK at 1500 agents) | 0–22 on 7/8 seeds; **177 on seed 7** | **MOSTLY PASS, one outlier** — not tuned further this round (see observation above) |
| `inside_territory ≥ 80%` on most seeds | Only 1/8 seeds ≥ 80% (seed 7, 91.3%) | **MISS** — regressed from the pre-`max_share` baseline (was 6/8); an open, recorded side effect of the caps, not tuned further |
| `water > 0` and `air > 0` on ≥ 6/8 seeds | water > 0 on 8/8 (up from 5/8); **air > 0 on 0/8** (unchanged) | **PARTIAL** — Water's collapse is fixed; Air's is a distinct, structural (food-access) problem `max_share` cannot reach |
| bench `on` ≤ 1.10 × `off` | 1.19–1.24 (unchanged; `AQUATIC_CAPACITY`/`max_share` don't affect tick cost) | **MISS — accepted** (controller ruling: no further tuning; this ~20% overhead on an opt-in layer is the final, shipped number) |

### Final verdict

The scenario-authoring bug is fixed: `max_share` ends the first-come
global-cap collapse, and Water — the other lineage that was losing before —
now survives on every seed. Two items remain open and are recorded rather
than chased further this round: **Air's extinction is structural** (a
land-only food source under a habitat mask that spends most of its time over
water, in direct competition with Land for the same forage — not a
population-cap or aquatic-resource problem, so no constant in this task's
scope fixes it), and **`max_share` traded one problem for two smaller ones**
(a `deep_overlaps` outlier on the single seed where two lineages are both
near their ceiling, and a broad `inside_territory` regression, both
unexplained beyond the hypothesis above). The ~20% tick overhead is accepted
per controller ruling and is not a defect to chase further. Recommendation
remains **DONE_WITH_CONCERNS**, with a narrower and more precisely diagnosed
set of open items than before this update.
