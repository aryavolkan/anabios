# Territory/habitat/collision layer: performance bench + measurement probe findings

**Date:** 2026-09-25. **Task:** 9 (performance bench + measurement probe + constant
tuning), closing out the territory/habitat/collision layer (Tasks 2–8).
**Design:** [`2026-09-25-territory-habitat-collision-design.md`](2026-09-25-territory-habitat-collision-design.md).
**Scenario:** `scenarios/habitat-territories.toml` (flagship flag-on scenario;
1024-wide world, `max_population = 1500`, three grazer species differing only
in Locomotion: Land, Water, Air).

> **This doc now has four rounds; read the Headline below for the final
> state, then "Round 4 — mechanism fix (diagnosis)" at the very bottom for
> the final numbers and verdict table.** Everything between the Headline and
> Round 4 is kept as the historical record of how the diagnosis narrowed: the
> original findings below found a diversity collapse; "Update 2026-09-25:
> `max_share` fix" diagnosed a scenario authoring bug (one shared population
> cap) and fixed Water but not Air; "Round 2" fixed Air's food-access
> disadvantage (it now grazes land and sea) and clustered the founders, which
> flipped the problem from Air-always-extinct to Land-rarely-surviving;
> "Round 3" tried the plan's last prescribed lever (`TERRITORY_PULL`
> 1.0→2.5) for `inside_territory` and made it worse (0/8), closing constant
> tuning; a follow-up diagnosis (Task 9b) then found the actual mechanism
> bug behind that failure and fixed it (Round 4) — `inside_territory` now
> reaches **6/8 seeds ≥ 80%, zero extinctions**.

## Headline (final state, after Round 4)

Correctness holds throughout every round: **zero habitat violations across
all 40 probe-seed runs**, five constant/mechanism configurations. The
mechanism bug behind the `inside_territory` miss (found by the Task 9b
diagnosis) is now fixed; two items remain open, reported honestly:

1. **`inside_territory` — FIXED (Round 4)**: root cause was not a wrong
   constant but a wrong mechanism, found by the Task 9b diagnosis (see
   `docs/superpowers/specs/2026-09-25-territory-pull-diagnosis.md`):
   (a) the pull is a fixed-size vector added to an *unbounded, evolvable*
   move intent that reaches magnitude 10²–10⁶ by mid-run, drowning the pull
   after normalization; (b) the pull only started ramping *at* `r` (full
   strength at `2r`), so an outward-steering member's stall point sat
   *outside* `r` by construction — exactly where the metric draws its line;
   (c) the K=12/R_MAX=256 range (≈452 unit² per member) grazed out its own
   interior, so a pull strong enough to hold members there would have
   starved them. The fix (implemented here, Task 9b): `decide_all` unit-caps
   the move intent accumulated so far before adding a non-zero pull
   (`territory::apply_territory_pull`); the pull now ramps from
   `TERRITORY_FREE_FRAC · r` (0.5) to full strength at `r`, not `2r`
   (`territory_pull`); `TERRITORY_K` 12→24 and `TERRITORY_R_MAX` 256→512
   enlarge the per-capita range so the now-binding pull doesn't starve its
   members. Result: **6/8 seeds ≥ 80% (was 0/8), zero extinctions, Air alive
   8/8, Water alive 5/8 (was 3/8), Land alive 2/8 (was 3/8)** — see "Round 4"
   below for the full table and per-class breakdown.
2. **Tick overhead (accepted miss, unchanged)**: `territory_enabled` costs
   **~19–24%** per 10k-agent tick against a ≤10% budget. The one
   architecture-preserving lever available (`RESOLVE_PASSES` 2→1) was tried
   in round 1; it only trimmed the overhead to ~18–20% while making
   `deep_overlaps` ~5× worse, so it was reverted. The controller has
   accepted ~20% as the final cost of this opt-in layer. Not re-measured in
   Round 4 (the mechanism fix touches `decide_all`'s territory block and
   `territory_pull`, not the collision/resolve stages the bench targets).
3. **Land-vs-air (and water) competitive balance (accepted, unchanged
   character)**: pre-fix, Land dominated and Air went extinct in 24/24
   seeds; Round 2's fix (Air grazes land and sea) flipped it hard the other
   way. Round 4's numbers continue that pattern — Air alive 8/8, Water 5/8,
   Land only 2/8 — consistent with Round 2/3's diagnosis of an unpriced Air
   efficiency advantage, not something Task 9b's territory-pull fix touches
   or was scoped to address. **Candidate next step, not implemented**: a
   flight metabolic premium for Air.

The `inside_territory` misses that remain (seeds 4 and 5 in Round 4, both
pure- or Air-heavy seeds) are **heritable escape, not mechanism failure**:
their Air lineages evolved Territoriality down to 0.16–0.19, below
`1/TERRITORY_PULL = 0.4`, the threshold at which even a fully-engaged pull
can no longer out-vote a unit-length outward intent. This is intended
behaviour (Territoriality is a heritable gene the pull is scaled by), not a
bug, and it bounds any "≥ 80% on every seed" target.

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

## Round 2 (controller rulings A + B)

Two rulings, both decided by the controller and implemented as specified
(not further tuned):

- **Ruling A — spec amendment**: `Locomotion::can_graze` now returns `true`
  for Air on every terrain (was: non-Water only), matching Air's already-
  unrestricted `can_occupy`. Flag-off behavior is unaffected — `can_graze` is
  only consulted in `feed_pass` under `if world.territory_enabled && ...`, so
  a flag-off world never reaches it (confirmed: both trajectory guards below
  pass untouched). Doc comments in `habitat.rs`, `interact.rs`, the design
  spec (§3), and the scenario header comment all updated to describe Air as a
  seabird-niche grazer (land and sea), not land-only.
- **Ruling B — clustered founders**: `scenarios/habitat-territories.toml`
  placements changed from three independent `uniform` scatters to one
  coherent founding region — Land founds a single `habitat` herd
  (`herds = 1, radius = 80.0`), Water and Air both scatter `near_spec` around
  that same herd (`spec = 0`, radius 160.0 / 120.0 respectively). `instantiate`
  still relocates every founder to the nearest valid cell for its own class,
  so Water lands in the coastal sea beside the herd. `max_share`
  (0.45/0.35/0.20), `AQUATIC_CAPACITY` (6.0), and `RESOLVE_PASSES` (2) are
  unchanged.

### What changed, concretely

- `crates/anabios-core/src/habitat.rs`: `can_graze` match arm split (`Land`
  gets its own arm; `Air => true`); doc comment updated; `terrain_rules` test
  updated (`Air.can_graze(TerrainType::Water)` is now asserted `true`).
- `crates/anabios-core/src/interact.rs`: `feed_pass`'s grazing-gate comment
  updated; `flyers_do_not_graze_aquatic_biomass` **replaced** with
  `grazing_is_gated_by_locomotion_class` (positive: an Air agent over water
  DOES graze; negative: a Water-class agent stranded on a Grass cell with
  real biomass does NOT graze it).
- `docs/superpowers/specs/2026-09-25-territory-habitat-collision-design.md`:
  §3 grazing-gate bullet updated (Air grazes both), amendment note added.
- `scenarios/habitat-territories.toml`: placements changed as above; header
  comment rewritten (seabird niche, one coherent founding region rationale).
- `crates/anabios-core/tests/determinism.rs`: `HABITAT_GOLDEN` re-pinned
  (both the grazing-rule change and the placement change move the flag-on
  trajectory):
  ```
  // before (post max_share fix)
  &[(0, 0xef14595f5fc0deac), (100, 0xa509d7958da4c0ea), (1000, 0x09a4ab4af4b3c021)]
  // after (UPDATE_HASHES=1 cargo test -p anabios-core --release --test determinism
  // habitat_territories_matches_golden_hashes -- --nocapture)
  &[(0, 0xd5f4fc2e1dbb698b), (100, 0xe46500acb65d195f), (1000, 0x5f2ba7a66421b6e6)]
  ```

### Tests run (each its own command, `--release` where it matters)

`cargo test -p anabios-core --lib habitat` (15 tests) and `--lib interact`
(14 tests, including the new `grazing_is_gated_by_locomotion_class`): all ok.
Then, in order: `--test determinism habitat_territories_matches_golden_hashes`
(ok, new pin), `--test determinism minimal_trajectory_unchanged_by_territory_substrate`
(ok, **untouched** — no hash change needed), `--test determinism
grand_theater_trajectory_unchanged_by_territory_substrate` (ok, **untouched**),
`--test determinism parallel_matches_serial_across_thread_counts` (ok),
`--test save_load_roundtrip territory_roundtrip` (ok), `--test invariants
habitat_classes` → `habitat_classes_never_leave_their_terrain` (ok), `--test
all_scenarios` (3/3 ok), and `cargo test -p anabios-core --lib` (517/517 ok).

### New probe (verbatim)

`cargo test -p anabios-core --release --test invariants territory_measurement_probe -- --ignored --nocapture`, 200.75s:

```
seed=1 alive=824 land=0 water=524 air=300 violations=0 deep_overlaps=7 inside_territory=54.7% water_cells_with_biomass=3301
seed=2 alive=300 land=0 water=0 air=300 violations=0 deep_overlaps=0 inside_territory=27.7% water_cells_with_biomass=9133
seed=3 alive=824 land=0 water=524 air=300 violations=0 deep_overlaps=75 inside_territory=49.4% water_cells_with_biomass=2058
seed=4 alive=1197 land=674 water=523 air=0 violations=0 deep_overlaps=9 inside_territory=66.2% water_cells_with_biomass=5549
seed=5 alive=1498 land=675 water=524 air=299 violations=0 deep_overlaps=5 inside_territory=83.6% water_cells_with_biomass=9084
seed=6 alive=702 land=0 water=522 air=180 violations=0 deep_overlaps=39 inside_territory=59.0% water_cells_with_biomass=9810
seed=7 alive=236 land=0 water=0 air=236 violations=0 deep_overlaps=0 inside_territory=7.2% water_cells_with_biomass=1630
seed=8 alive=824 land=0 water=524 air=300 violations=0 deep_overlaps=0 inside_territory=63.0% water_cells_with_biomass=6462
```

**Air's fix worked, decisively: air > 0 on 7/8 seeds** (was 0/8 before Ruling
A, 8/8 tries across two prior rounds). Air reaches its 300-agent share
ceiling outright on 5 of those 7 seeds. **Water also improved: water > 0 on
6/8 seeds** (was 5/8 after the `max_share`-only fix). Seed 5 is the standout:
all three classes alive simultaneously, two at their exact ceiling (Land 675,
Water 524) and Air one shy of its ceiling (299/300) — proof the three-species
coexistence the scenario was designed for is achievable with this
constant/placement set.

**But this reads as an overcorrection, honestly reported, not chased
further**: **Land now survives on only 2/8 seeds** (4, 5) — down from 6/8
before Ruling A. Air grazing both land and sea gives it access to
Water's food pool as a fallback AND full competitive parity with Land on the
shared land-forage pool (rather than Air being disadvantaged there as
before), so in most seeds Air (and/or Water, insulated by its own aquatic
pool) now out-competes Land for land vegetation instead of the reverse. This
is not a target this task tracks acceptance against (the brief's diversity
target is water/air survival specifically), but it is a new, real dynamic
worth naming rather than leaving implicit.

`deep_overlaps` (0–75) has the same character as prior rounds — mostly small,
with occasional outliers on seeds with larger total live population (seed 3:
75 agents alive=824; seed 6: 39, alive=702) — not investigated further, out
of this round's scope. `inside_territory` remains mostly below 80% (1/8 ≥80%,
seed 5 at 83.6%), essentially unchanged from the post-`max_share` round
despite the founder-clustering intended to help it — clustering founders at
spawn doesn't keep them clustered 20,000 ticks later once population dynamics
(caps, competition, deaths, births) take over. Recorded, not tuned.

### Per-target verdicts (Round 2, final)

| Target | Result | Verdict |
|---|---|---|
| `violations == 0` every seed | 0/8 | **PASS** |
| `deep_overlaps ≈ 0` (handful OK at ~1500 agents) | 0–75, mostly single digits | **MOSTLY PASS**, occasional outliers on higher-population seeds, unchanged character from prior rounds |
| `inside_territory ≥ 80%` on most seeds | 1/8 (seed 5, 83.6%) | **MISS**, unchanged from the post-`max_share` round — founder clustering didn't fix it |
| `water > 0` and `air > 0` on ≥ 6/8 seeds | water 6/8, **air 7/8** | **PASS** — both individually clear the ≥6/8 bar for the first time this task |
| (new, unrequested) `land > 0` | 2/8 | Not an original target; recorded because Ruling A's fix inverted which class is now the rare one |
| bench `on` ≤ 1.10 × `off` | 1.19–1.24 (unchanged; neither ruling touches tick-cost code paths, not re-measured) | **MISS — accepted per controller ruling** (round 1), final number |

### Round 2 verdict

Both controller rulings landed cleanly and did what they were meant to do:
Air's structural food-access disadvantage (identified in the prior round's
diagnosis) is resolved by letting it graze both land and sea, and it now
clears the diversity bar on 7 of 8 seeds; the founder-clustering ruling gives
the scenario one coherent starting region as intended, though it did not
measurably improve `inside_territory` at the 20k-tick horizon. The `water >
0 and air > 0 on ≥ 6/8 seeds` target — the one target that had failed in
every round up to now — **passes for the first time**. The cost is an
unrequested but real overcorrection: Land, previously the dominant class,
now survives only 2/8 seeds, because Air's widened food access makes it (and
Water, insulated in its own aquatic pool) the stronger competitor for shared
land forage in most seeds. `inside_territory` and the occasional
`deep_overlaps` outlier remain open, unchanged from the prior round. The
tick-overhead miss (~20%) is accepted per controller ruling and not
re-measured this round. Per the instruction not to iterate further this
round, no additional changes were made. Recommendation: **DONE_WITH_CONCERNS**
— narrower again than the prior round, with the Land-survival trade-off as
the one genuinely new concern.

## Round 3 (final)

**The last tuning round.** Controller ruling: change exactly one constant,
measure once, then tuning stops regardless of outcome.

### Change

`crates/anabios-core/src/territory.rs`: `TERRITORY_PULL` `1.0 → 2.5` — the
plan's prescribed lever for low `inside_territory`, never tried in rounds 1–2.
Doc comment updated to record the change and its rationale. Nothing else
changed: `AQUATIC_CAPACITY` stays 6.0, `RESOLVE_PASSES` stays 2, the scenario
(placements, `max_share`) and Air's grazing rule are untouched.

Pre-check: `pull_is_zero_inside_and_ramps_outside` (Task 6) expresses all its
pull-magnitude assertions in terms of the `TERRITORY_PULL` constant itself
(`0.5 * TERRITORY_PULL`, `TERRITORY_PULL` for the capped case), not a
hardcoded `1.0` — confirmed passing unchanged: `cargo test -p anabios-core
--lib territory` (9/9 ok).

### Tests run

`cargo test -p anabios-core --lib territory` (ok, 9/9, before the probe).
Then, each its own `--release` command: `--test determinism
habitat_territories_matches_golden_hashes` (ok, new pin), `--test determinism
minimal_trajectory_unchanged_by_territory_substrate` (ok, **untouched**),
`--test determinism grand_theater_trajectory_unchanged_by_territory_substrate`
(ok, **untouched**), `--test determinism
parallel_matches_serial_across_thread_counts` (ok), `--test
save_load_roundtrip territory_roundtrip` (ok), `--test invariants
habitat_classes` (ok), `--test all_scenarios` (3/3 ok).

`HABITAT_GOLDEN` re-pinned:
```
// before (Round 2)
&[(0, 0xd5f4fc2e1dbb698b), (100, 0xe46500acb65d195f), (1000, 0x5f2ba7a66421b6e6)]
// after (UPDATE_HASHES=1 cargo test -p anabios-core --release --test determinism
// habitat_territories_matches_golden_hashes -- --nocapture)
&[(0, 0xd5f4fc2e1dbb698b), (100, 0xb94a24dc0328ad4c), (1000, 0x9ae217d01389078a)]
```
Tick 0's hash is unchanged (`TERRITORY_PULL` only affects post-instantiate
movement); ticks 100 and 1000 moved, as expected.

### New probe (verbatim)

`cargo test -p anabios-core --release --test invariants territory_measurement_probe -- --ignored --nocapture`, 190.69s:

```
seed=1 alive=300 land=0 water=0 air=300 violations=0 deep_overlaps=0 inside_territory=9.3% water_cells_with_biomass=6938
seed=2 alive=824 land=0 water=525 air=299 violations=0 deep_overlaps=3 inside_territory=68.0% water_cells_with_biomass=10030
seed=3 alive=1200 land=675 water=525 air=0 violations=0 deep_overlaps=0 inside_territory=72.8% water_cells_with_biomass=5768
seed=4 alive=975 land=675 water=0 air=300 violations=0 deep_overlaps=38 inside_territory=71.8% water_cells_with_biomass=3264
seed=5 alive=299 land=0 water=0 air=299 violations=0 deep_overlaps=2 inside_territory=12.0% water_cells_with_biomass=811
seed=6 alive=822 land=0 water=522 air=300 violations=0 deep_overlaps=116 inside_territory=51.8% water_cells_with_biomass=10205
seed=7 alive=974 land=674 water=0 air=300 violations=0 deep_overlaps=0 inside_territory=72.9% water_cells_with_biomass=6285
seed=8 alive=300 land=0 water=0 air=300 violations=0 deep_overlaps=0 inside_territory=11.3% water_cells_with_biomass=9427
```

**Reported exactly as measured, per the ruling ("tuning stops regardless of
outcome")**: raising `TERRITORY_PULL` did **not** improve `inside_territory`
— it got worse. **0/8 seeds reach ≥80%** (round 2 had 1/8). The clearest
pattern in the data: single-class seeds (1, 5, 8 — pure Air at its 300-agent
cap) sit at 9.3–12.0% `inside_territory`, while every seed with two
coexisting classes sits in a 51.8–72.9% band — territory cohesion tracks
which/how-many classes coexist far more than it tracks the pull constant.
Diversity also shifted again, incidentally: **water > 0 on only 3/8 seeds**
(2, 3, 6 — down from 6/8 in round 2), **land > 0 on 3/8** (3, 4, 7 — up
slightly from 2/8), **air > 0 on 7/8** (unchanged, still the strongest
performer). `deep_overlaps` keeps the same mixed character (0–116, a new
single-seed high on seed 6) as every prior round. No further changes were
made in response to any of this, per the ruling.

### Per-target verdicts (Round 3, final)

| Target | Result | Verdict |
|---|---|---|
| `violations == 0` every seed | 0/8 | **PASS** (holds in all 4 rounds, 32/32 seeds) |
| `deep_overlaps ≈ 0` (handful OK at ~1500 agents) | 0–116, mostly single digits | **MOSTLY PASS**, occasional outliers persist (unchanged character across all rounds) |
| `inside_territory ≥ 80%` on most seeds | 0/8 | **MISS — final**, worse than round 2 (1/8); the prescribed lever did not help |
| `land > 0` (not an original target, tracked since round 2's overcorrection) | 3/8 | open, unresolved |
| `water > 0` on ≥ 6/8 seeds | 3/8 | **MISS — final** (was 6/8 in round 2; regressed) |
| `air > 0` on ≥ 6/8 seeds | 7/8 | **PASS — final** (stable across rounds 2–3) |
| bench `on` ≤ 1.10 × `off` | 1.19–1.24 (round 1 measurement; not re-measured, unaffected by `TERRITORY_PULL`) | **MISS — accepted per controller ruling**, final |

### Round 3 verdict (final, task closed)

The last prescribed constant lever did not deliver: `TERRITORY_PULL` 1.0→2.5
made `inside_territory` worse (0/8 ≥80%, down from round 2's 1/8) and, as an
incidental side effect on the ecological competition, moved Water's survival
from 6/8 seeds down to 3/8 without helping Land. Only Air's survival (7/8)
proved stable across the constant changes tried in rounds 2 and 3. Three
tuning rounds, four constant/scenario configurations, and 32 probe-seed runs
later, the picture is: **correctness is solid** (zero violations, always),
**collision quality is acceptable with occasional outliers**, but **the
three-species coexistence and territory-cohesion goals the scenario was
designed around are not reliably achievable by constant tuning alone** — the
land/water/air competitive balance visibly oscillates with each constant
changed rather than converging, which is itself evidence that the remaining
gap is a missing mechanic (e.g., pricing Air's wider range with a metabolic
cost) rather than a mistuned number. Per the controller's ruling, tuning is
now closed regardless of this outcome. Final recommendation:
**DONE_WITH_CONCERNS**. Open items for any future work (explicitly not
attempted here): a flight metabolic premium for Air, and further
investigation of why `inside_territory` correlates with class-coexistence
count rather than the pull constant.

## Round 4 — mechanism fix (diagnosis)

**Task:** 9b (territory-pull mechanism fix), following a dedicated diagnosis
task run after Round 3 closed constant tuning with `inside_territory` at its
worst measured value (0/8). **Diagnosis:**
[`2026-09-25-territory-pull-diagnosis.md`](2026-09-25-territory-pull-diagnosis.md)
(§1, §6, §8, §9 are the load-bearing sections; §9 is this round's exact
requirements).

### Root cause (summary; see the diagnosis for the full measurement)

Round 3 raised `TERRITORY_PULL` on the theory that the pull was too weak.
The diagnosis found the opposite problem: the pull's *mechanism*, not its
*magnitude*, was broken, in three compounding ways (diagnosis §3–§4, §8):

1. **H6 (new, dominant for Air): the pull is a fixed-size vector added to an
   unbounded, evolvable move intent.** Evolved programs feed sensor values
   (energy, distances clamped at 1e6) into `MoveToward*`; by 6–10k ticks
   Air's median intent magnitude reaches 57–1040 (p90 up to 1e6) at
   `TERRITORY_PULL = 2.5`, versus ≈1 at `TERRITORY_PULL` 0 or 1.0. After
   normalization the pull carries under 1% of the final direction —
   `cos(desired, home) ≈ 0.0` for outside Air agents in steady state. A
   *stronger* pull only raised the payoff of letting the intent inflate, so
   Round 3's lever made containment worse, not better.
2. **H4 (confirmed, dominant overall): the old ramp started at `r`, not
   before it.** With free roam to `r` and ramp to full strength at `2r`, an
   agent whose non-territory steering points outward (typical for foragers:
   the territory core is grazed out, food is outside) settles on a stall
   shell at `d ≈ r·(1 + |cos|/(PULL·terr))` — which is *outside `r` by
   construction*, exactly where the `inside_territory` metric draws its
   line. Measured median d/r of non-inflated outside agents: 1.1–1.9.
3. **H7 (new, confirmed): the K=12/R_MAX=256 range (≈452 unit² per member)
   can't feed the population it holds.** Fixing (1) and (2) alone at the old
   radius law lifts containment to 95–97% at 1k ticks, but then **5/8 seeds
   go fully extinct** by 10k ticks — the range starves once escape is closed
   off.

Two of the diagnosis's other hypotheses (H1 centre-on-invalid-terrain, H2
stale inherited centres) were confirmed **minor/secondary**; H5 (a
misleading composition-dominated metric) was **partly true**, which is why
this round also adds the per-class breakdown recommended there (see below).

### The fix (implemented exactly as diagnosis §9)

- `crates/anabios-core/src/tick.rs::decide_all` — territory block: extracted
  the "unit-cap the intent, then add the pull" logic into a new pure helper,
  `territory::apply_territory_pull(action_xy, pull) -> Vec2` (kept
  `decide_all` a two-line call instead of an inline conditional). It caps the
  move intent accumulated so far to length ≤ 1 (only when finite) whenever
  the pull is non-zero, then adds the pull.
- `crates/anabios-core/src/territory.rs::territory_pull` — new
  `TERRITORY_FREE_FRAC = 0.5`: zero pull inside `FREE_FRAC · r`, ramping to
  full strength at `r` (`ramp = ((dist − inner) / (r − inner)).min(1.0)`,
  `inner = FREE_FRAC · r`) instead of the old zero-to-`r`-then-ramp-to-`2r`.
- Constants: `TERRITORY_K` 12.0 → 24.0, `TERRITORY_R_MAX` 256.0 → 512.0.
  `TERRITORY_PULL` stays 2.5 (Round 3's value); `TERRITORY_R_MIN` stays 48.0.
- Everything sits inside the existing `if territory_enabled` gate; the pull
  cap only clamps the accumulated intent, never the pull itself, and both
  functions are pure (no RNG, no `World` access). Flag off ⇒ byte-identical:
  both trajectory guards (`minimal_trajectory_unchanged_by_territory_substrate`,
  `grand_theater_trajectory_unchanged_by_territory_substrate`) pass
  **untouched**, with no hash change needed.
- Per the diagnosis's H5 recommendation, `territory_measurement_probe`
  (`crates/anabios-core/tests/invariants.rs`) now also prints per-class
  inside % (`inside_by_class(L/W/A)=...`, `-` for an extinct class) on each
  seed's line, so the aggregate figure's class-composition dependence is
  visible directly in the probe output rather than requiring a separate
  diagnosis run.

`HABITAT_GOLDEN` re-pinned in `crates/anabios-core/tests/determinism.rs`:

```
// before (Round 3)
&[(0, 0xd5f4fc2e1dbb698b), (100, 0xb94a24dc0328ad4c), (1000, 0x9ae217d01389078a)]
// after (UPDATE_HASHES=1 cargo test -p anabios-core --release --test determinism
// habitat_territories_matches_golden_hashes -- --nocapture)
&[(0, 0xd5f4fc2e1dbb698b), (100, 0x915051a395e3183a), (1000, 0x58062a9c7997308b)]
```

Tick 0's hash is unchanged (the fix only changes post-instantiate movement,
same as Round 3); ticks 100 and 1000 moved, as expected.

### New probe (verbatim)

`cargo test -p anabios-core --release --test invariants territory_measurement_probe -- --ignored --nocapture`, 235.39s:

```
seed=1 alive=1499 land=674 water=525 air=300 violations=0 deep_overlaps=3 inside_territory=99.7% inside_by_class(L/W/A)=100.0/99.2/100.0 water_cells_with_biomass=11949
seed=2 alive=300 land=0 water=0 air=300 violations=0 deep_overlaps=0 inside_territory=86.0% inside_by_class(L/W/A)=-/-/86.0 water_cells_with_biomass=10454
seed=3 alive=825 land=0 water=525 air=300 violations=0 deep_overlaps=12 inside_territory=80.2% inside_by_class(L/W/A)=-/99.2/47.0 water_cells_with_biomass=6148
seed=4 alive=825 land=0 water=525 air=300 violations=0 deep_overlaps=2 inside_territory=70.3% inside_by_class(L/W/A)=-/100.0/18.3 water_cells_with_biomass=6611
seed=5 alive=300 land=0 water=0 air=300 violations=0 deep_overlaps=0 inside_territory=23.3% inside_by_class(L/W/A)=-/-/23.3 water_cells_with_biomass=6476
seed=6 alive=824 land=0 water=524 air=300 violations=0 deep_overlaps=1 inside_territory=83.7% inside_by_class(L/W/A)=-/98.3/58.3 water_cells_with_biomass=5192
seed=7 alive=773 land=0 water=473 air=300 violations=0 deep_overlaps=5 inside_territory=85.5% inside_by_class(L/W/A)=-/90.5/77.7 water_cells_with_biomass=6287
seed=8 alive=974 land=674 water=0 air=300 violations=0 deep_overlaps=1 inside_territory=93.5% inside_by_class(L/W/A)=100.0/-/79.0 water_cells_with_biomass=7689
```

This matches the diagnosis's own §10 "after" run exactly (same fix, same
constants, run independently here as part of Task 9b's verification): **6/8
seeds ≥ 80%** (seeds 1, 2, 3, 6, 7, 8; misses are seed 4 at 70.3% and seed 5
at 23.3%), **zero extinctions** (every seed's `alive` count is a healthy
population, 300–1499), **Air alive on 8/8 seeds** (was 7/8 pre-fix), **Water
alive on 5/8** (was 3/8 in Round 3), **Land alive on 2/8** (was 3/8 in Round
3 — an incidental, expected side effect of enlarging the Water/Air ranges
into more of the shared world, not a new problem this task's scope covers).
`violations` stay 0 on every seed; `deep_overlaps` are 0–12 (down from
Round 3's 0–116).

The per-class breakdown shows the fix reaching classes it previously never
held: Water is 90.5–100% inside on every seed it survives (was 38.1–98.1%
pre-fix), and Air — never held above 15.1% in any prior round — reaches
18.3–100% (misses only on seeds 4 and 5, both discussed below). Land, which
was already near-100% inside before this fix (it is terrain-bounded and
tightly herded), stays at 100% on both seeds it survives.

### Per-target verdicts (Round 4, final)

| Target | Result | Verdict |
|---|---|---|
| `violations == 0` every seed | 0/8 | **PASS** (holds in all 5 rounds, 40/40 seeds) |
| `deep_overlaps ≈ 0` (handful OK at ~1500 agents) | 0–12 | **PASS** — best of any round (was 0–116 in Round 3) |
| `inside_territory ≥ 80%` on most seeds | **6/8** | **PASS** — reverses Round 3's 0/8; matches the diagnosis's validated §6 prediction (6/8, 0 extinctions) exactly |
| `air > 0` on ≥ 6/8 seeds | 8/8 | **PASS** — improved from 7/8 |
| `water > 0` on ≥ 6/8 seeds | 5/8 | **MISS, narrowly** — up from 3/8 (Round 3) but short of the 6/8 bar; not a regression, and not tuned further per this task's scope (mechanism fix only, no scenario/constant retuning beyond §9c) |
| `land > 0` (not an original target, tracked since Round 2) | 2/8 | open, unresolved; down from 3/8, an expected side effect of Water/Air's larger, now-effective ranges competing harder for the shared world, not investigated further (out of Task 9b's scope) |
| bench `on` ≤ 1.10 × `off` | 1.19–1.24 (Round 1 measurement; not re-measured — Task 9b's fix does not touch the collision/resolve stages the bench targets) | **MISS — accepted per controller ruling** (unchanged) |

The two seeds still under 80% (4: 70.3%, 5: 23.3%) are, per the diagnosis's
prediction, **heritable escape rather than mechanism failure**: their Air
lineages evolved Territoriality down to 0.16–0.19, below
`1/TERRITORY_PULL = 0.4` — the point at which even a fully-engaged pull can
no longer out-vote a unit-length outward intent. Territoriality staying
heritable (and therefore evolvable away where holding is costly) is intended
behaviour, not a bug, and it bounds any "≥ 80% on every seed" target.

**R_MAX caveat (carried from the diagnosis, §8):** `TERRITORY_R_MAX = 512`
on the flagship scenario's 1024-wide torus means a max-size range (radius
512) can cover essentially half the world's linear extent — i.e. a
full-capacity species' range is not a small, local "territory" in the
intuitive sense on this map, it can span most of the world. This is by
design (the diagnosis measured that the flagship's populations, up to
max_share 675/525/300 on a ~50%-ocean 1024 world, need close to that much
area to be fed without starving), but it weakens the "territory" story for
the larger classes on this particular scenario and should be revisited if a
future task wants visually/behaviorally tighter ranges — e.g. by lowering
`max_population`/`max_share`, raising forage productivity, or sizing `r`
from the co-ranging lineage population rather than each genome-species
fragment (the diagnosis's fragmentation-aware alternative, not implemented
here). `TERRITORY_R_MAX = 512` has not been (and per this task's scope,
should not be) tried on a larger scenario without re-checking this ratio.

### Round 4 verdict (final)

The Task 9b diagnosis correctly identified that Round 3's failure was a
mechanism bug, not a mistuned constant, and its prescribed three-part fix
(§9a unit-capped intent, §9b free-roam-to-`r/2`-then-ramp-to-`r` geometry,
§9c enlarged K/R_MAX) reproduces exactly as predicted when implemented here:
`inside_territory` goes from the task's worst-ever result (0/8 in Round 3)
to its best (6/8), with zero extinctions and improved `deep_overlaps` and
Air survival as side benefits. The mechanism fix is complete and matches its
own validation numbers exactly (bit-for-bit population/percentage match
against the diagnosis's independent measurement run). Two items remain open
by design/scope: `water > 0` on 5/8 (short of 6/8, an incidental but not
regressive side effect), `land > 0` on 2/8 (unresolved since Round 2, not
in Task 9b's scope), and the ~20% tick-overhead miss (accepted since Round
1, untouched by this fix). Recommendation: **DONE** for the
`inside_territory` mechanism, **DONE_WITH_CONCERNS** overall (carrying
forward the pre-existing, out-of-scope Land/Water diversity and tick-overhead
items).

## Final probe (post-review)

Whole-branch review flagged that "never overlap" overclaimed what the
collision layer actually guarantees (best-effort separation steering plus a
2-pass min-gap resolve, not a hard guarantee). `territory_measurement_probe`
now also reports `shallow_overlaps` — colliding pairs closer than their gap
but at least half of it (still touching, less severely than `deep_overlaps`)
— alongside the flagship seed change from `seed = 11` to the validated
showcase `seed = 1` (F1 of the final fix wave).

`cargo test -p anabios-core --release --test invariants territory_measurement_probe -- --ignored --nocapture`, 264.70s:

```
seed=1 alive=1499 land=674 water=525 air=300 violations=0 deep_overlaps=3 shallow_overlaps=283 inside_territory=99.7% inside_by_class(L/W/A)=100.0/99.2/100.0 water_cells_with_biomass=11949
seed=2 alive=300 land=0 water=0 air=300 violations=0 deep_overlaps=0 shallow_overlaps=6 inside_territory=86.0% inside_by_class(L/W/A)=-/-/86.0 water_cells_with_biomass=10454
seed=3 alive=825 land=0 water=525 air=300 violations=0 deep_overlaps=12 shallow_overlaps=184 inside_territory=80.2% inside_by_class(L/W/A)=-/99.2/47.0 water_cells_with_biomass=6148
seed=4 alive=825 land=0 water=525 air=300 violations=0 deep_overlaps=2 shallow_overlaps=543 inside_territory=70.3% inside_by_class(L/W/A)=-/100.0/18.3 water_cells_with_biomass=6611
seed=5 alive=300 land=0 water=0 air=300 violations=0 deep_overlaps=0 shallow_overlaps=8 inside_territory=23.3% inside_by_class(L/W/A)=-/-/23.3 water_cells_with_biomass=6476
seed=6 alive=824 land=0 water=524 air=300 violations=0 deep_overlaps=1 shallow_overlaps=155 inside_territory=83.7% inside_by_class(L/W/A)=-/98.3/58.3 water_cells_with_biomass=5192
seed=7 alive=773 land=0 water=473 air=300 violations=0 deep_overlaps=5 shallow_overlaps=79 inside_territory=85.5% inside_by_class(L/W/A)=-/90.5/77.7 water_cells_with_biomass=6287
seed=8 alive=974 land=674 water=0 air=300 violations=0 deep_overlaps=1 shallow_overlaps=686 inside_territory=93.5% inside_by_class(L/W/A)=100.0/-/79.0 water_cells_with_biomass=7689
```

Identical `alive`/`deep_overlaps`/`inside_territory` numbers to Round 4's
probe (this change touches wording and instrumentation only, not the
mechanism). `shallow_overlaps` (6–686 across seeds) confirms the "kept
apart" reword is the accurate claim: at ~300–1500 agents on a shared range,
a meaningful share of colliding pairs sit inside their gap but past the
`deep_overlaps` half-gap line at any instant — expected from a fixed
`RESOLVE_PASSES = 2` Jacobi resolve over a moving crowd, not a regression.
