# Cognition threshold: a world to graph what the IQ gates are worth

**Date:** 2026-10-09
**Status:** shipped (scenario, knobs, instrument, plots, readings)
**Readings:** `docs/scenarios.md`, "Cognition threshold"

## Question

The cognitive layer gives every agent a realized IQ and five gates read it:
a maladaptive practice needs `PRACTICE_IQ_REQ` (0.10), an invention of era
1..4 needs `IQ_REQ_BY_ERA` (0.15 / 0.35 / 0.55 / 0.75). `tribes` shows the
layer running, but nothing in the repo answered the two questions a
threshold raises: how much fitness does an agent gain by clearing a gate,
and what does the height of the ladder do to the population as a whole?

## Design

Three pieces, each small, together one instrument.

### 1. The ladder is a knob

`World` gains `iq_req_by_era: [f32; 4]` and `practice_iq_req: f32`,
serialized after `cognition_enabled` and defaulting to the constants
(`FORMAT_VERSION` 46 → 47; every trajectory unchanged, only the layout
grew). The gates route through `World::invention_iq_permits` and
`World::practice_iq_permits`; the free functions keep the constant ladder
for callers and tests that want it (`invention::iq_permits_at`,
`practice::iq_permits_at` take an explicit ladder). A scenario sets them
with `iq_req_by_era = [..]` / `practice_iq_req = ..`, each entry finite and
in `[0, 1]` or the file is refused at parse (`ScenarioError::InvalidIqReq`).

### 2. A world where the gate is the only difference

`scenarios/cognition-threshold.toml`: six lineages of the `innovator` kit
(omnivore grazer, Communicator, Reproductive kept) at `cognitive_potential`
0.0, 0.2, 0.4, 0.6, 0.8, 1.0, 40 founders each, Openness 1.0, Stone Tools
and Fire seeded so the era-2 candidates are open from tick 0. One `habitat`
anchor and five `near_spec` scatters put every tier on the same ground,
so a discovery by one tier reaches every tier whose IQ clears the gate —
the gate, not geography, decides who learns. Realized IQ is half gene, half
juvenile enrichment, so the rungs land about 0.1 apart from 0.23 to 0.75 at
maturity: one below the era-2 gate, two between era 2 and 3, two between
era 3 and 4, one on the era-4 gate.

Opt-outs remove the other acquisition gates (material baskets, genome
prerequisites and affinity scaling) and the noise layers (disasters, war,
disease, domestication, dimorphism, the arms race). The realism layers
(gait, growth, gestation, chase, turning, collision, needs, affect,
practices, knowledge, settlement) stay on.

### 3. A read-only instrument

`anabios-headless cognition` steps the world and reads columns; it writes
nothing into the sim and drains nothing from the codex. Per tick it diffs
the agent columns against a shadow copy: a slot that was alive and is now
dead or re-occupied closes a lifetime (realized IQ, age, `births_ok`,
highest era held, practice held); a slot newly alive is a birth, labelled
with its first parent's tier (founders are labelled by species at tick 0,
so speciation splinters stay with their founders). Outputs are long-format
CSVs keyed by `seed` and `scale`: `series.csv` (per window and tier),
`fitness.csv` (lifetimes by 0.05 realized-IQ bin), `tiers.csv` (lifetimes
by tier) and `gates.csv` (one row per run). `--gate-scales` multiplies the
whole ladder by each factor and runs once per factor, in parallel over
seeds and factors. `scripts/cognition_plot.py` draws the five SVGs from the
CSVs with the standard library alone.

## Findings (8 seeds × 6000 ticks)

The full readings with the charts are in `docs/scenarios.md`. In short:

1. Realized IQ orders the tiers on every seed and every tier sinks 0.10–0.15
   over the run as the shared site is grazed out.
2. The tier under the era-2 gate is excluded: 45 agents at tick 6000 summed
   over 8 seeds against 297–694 for the other tiers, 436 births against
   908–1424, 1.17 offspring per matured lifetime against 1.32–1.65.
3. Offspring per matured lifetime against realized IQ is a hump: 0.55–0.61
   below the practice gate, 1.2–1.5 between the era-1 and era-2 gates,
   1.5–2.05 between the era-2 and era-3 gates, 1.1–1.4 above the era-3
   gate, where era-3 tech's upkeep and stress shorten lives by a fifth.
4. The default ladder is the worst height for the whole population: 766
   births per run against 1159 with every gate open and 1000 at twice the
   height. A gate makes tech a differential advantage; the mixed-era
   population is the smaller one.

## Verification

- `tests/cognition_threshold.rs`: the knobs parse, move the ladder the
  world reads, refuse values outside `[0, 1]`; the six rungs are distinct
  cultured breeding species on an ascending gene ladder; realized IQ comes
  out in gene order after maturation with gene 0 under the era-2 gate.
- `tests/all_scenarios.rs` and `tests/save_load_roundtrip.rs` cover the
  file; `crates/anabios-headless/tests/cognition_csv.rs` covers the CSVs.
- Golden hashes were not regenerated: the layout grew, so the opt-in
  `--ignored` golden tables are stale until their next `UPDATE_HASHES=1`
  run. No trajectory moved.
