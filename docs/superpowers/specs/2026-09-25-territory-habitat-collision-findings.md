# Territory/habitat/collision layer: performance bench + measurement probe findings

**Date:** 2026-09-25. **Task:** 9 (performance bench + measurement probe + constant
tuning), closing out the territory/habitat/collision layer (Tasks 2–8).
**Design:** [`2026-09-25-territory-habitat-collision-design.md`](2026-09-25-territory-habitat-collision-design.md).
**Scenario:** `scenarios/habitat-territories.toml` (flagship flag-on scenario;
1024-wide world, `max_population = 1500`, three grazer species differing only
in Locomotion: Land, Water, Air).

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
