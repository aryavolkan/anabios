# Scenarios → phenomena

`scenarios/` holds twelve worlds, each named for its setting rather than for a
single phenomenon. The scenario schema defaults every feature knob **on**
(`Scenario`, `scenarios/*.toml`) — a curated world only lists what it turns
**off**, so every file below runs the full engine stack unless its row says
otherwise. (The engine layer, `World::new`/`World::with_dims`, defaults to
nothing; see `docs/determinism-contract.md`'s "Two default layers" for the
split.) To opt a subsystem out for your own run, copy a scenario and set its
`*_enabled` knob to `false` (or `season_period = 0`) — the flag-off trajectory
guards in `tests/determinism.rs` pin that this is byte-identical to the
subsystem never having existed.

Four knobs stay off by default everywhere as **experiment levers**, not
curated-world features: `env_period` (DIT environmental-variability sweep),
`climate_drift_rate` (secular climate drift on top of it), `payoff_biased_learning`
(O2b, measured negative) and `unilateral_trade` (the O2.6 trade-freeze fix,
superseded by the current material economy). Three worlds below turn the
first two on deliberately; none turns on the latter two — see "Retired
experiments".

Every file is smoke-tested by `tests/all_scenarios.rs` (parse → instantiate →
200 ticks) and has a `tests/save_load_roundtrip.rs` round-trip test. Read a
world's own header comment for the full story — placement technique, tuned
seeds, measured caveats; this table is the index.

| World | Setting | Shows | Opt-outs / levers | Seed | Cost (ms/tick) |
|---|---|---|---|---|---|
| `minimal.toml` | 1024² plains, one lineage | Baseline grazing world; the determinism goldens pin on this scenario. | — | 12345 | — |
| `predator-prey.toml` | Four regional herds: a mixed cohort at the centre, a mammal grazer/pursuer pair north, a reptile ambusher/basker pair east, a thin forager background | Predation cycles and population crashes (`PopCrash`), the trophic cascade when predators collapse, mammal-vs-reptile niche sorting on the drier east, and foraging/climate-affinity selection in the background stock. Absorbed: trophic-cascade, grazers-and-wolves, mammals-vs-reptiles, foraging-selection, biome-adaptation. | — | 0 | — |
| `speciation.toml` | One herbivore stock seeded as two body-size morphs at the west/east ends, plus a communicator, a scent-marker, a cooperator and a hunter-prey lineage | Speciation from a founder stock (`Divergence`/`Convergence`), dialect formation between the communicator clusters, pheromone territories between the marker clusters, kin cooperation, and the four DIT learning-strategy pairings. Absorbed: divergent, convergent, cooperation, territories, dialects, gene-culture, gene-culture-skill, gene-culture-hunt, gene-culture-alarm. | — | 4242 | — |
| `tribes.toml` | Ape-tier omnivore innovators, traditionalists and a stone-tool hunter band among grazer herds, armoured/spined prey and predator packs | Discovery and adoption of inventions (`Discovery`/`Adoption`/`KnowledgeRatchet`), traditions, IQ-tech coevolution, weapons and war (`War`/`WarEnded`), domestication pens, sexual dimorphism, epidemics (`Epidemic`/`MedContain`), the anthropogenic arms race, the affect layer's moods and play, and thirst/sleep. Absorbed: inventions, tool-users, weapons-arena, weapons-arms-race, war, traditions, cognitive-coevolution, tech-gene-coupling, knowledge-ratchet, domestication, dimorphism, disease, anthro-race, affect-play, affect-seeking, affect-showcase, affect-social, affect-threat, basic-needs. | — | 60623 | — |
| `markets.toml` | Four terrain-affinity forager lineages at a biome junction beside five goods-producing grazer lineages | Home-range anchoring and settlements, resource harvesting, bilateral barter at the predetermined trade hubs, and the trade-flow/market events. Absorbed: settlement, biome-trade, geographic-trade, trade-hubs, unilateral-trade (its lever is exercised by an inline fixture in `tests/trade.rs`, not this world). | — | 424242 | — |
| `habitat-territories.toml` | Three grazer species differing only in Locomotion (land/water/air), founded together at one site | Locomotion-gated habitat selection, species territories kept apart by collision-aware separation steering, and per-lineage `max_share` stopping the shared population cap from sterilizing the smaller founders. | — | 4 | — |
| `grand-theater.toml` | Everything-on staged world at the tuned geographic-trade terrain (seed 424242) | Environment, disturbance, gene-culture, economy, conflict and communication all colliding in one shared world — the strongest save/load round-trip guard. | `env_period = 400`, `climate_drift_rate = 0.00005` | 424242 | — |
| `out-of-africa-saga.toml` | The grand-theater cast on a human-dispersal geography (climate-driven worldgen) | The showcase cut: era-3 tech (Stone Tools/Fire/Farming/Writing/Husbandry) seeded on the Quarry innovators from tick 0 so downstream tech and on-camera taming emerge without stalling at era 1. | `env_period = 400`, `climate_drift_rate = 0.00005` | 318 | — |
| `out-of-africa-earth.toml` | The saga's founders re-anchored onto a real-Earth elevation/temperature/precipitation map (`world_map = "earth"`) | The same DIT/cognition/war/domestication stack, but the exodus runs through the real African corridors (Sinai/Bab-el-Mandeb, Gibraltar); dispersal is emergent, not scripted. | `env_period = 400`, `climate_drift_rate = 0.00005` | 318 | — |
| `sandbox.toml` | 2048² world, 8k population cap, no staging | The open-ended run for watching the tech tree, gene-vs-culture and long ecological cycles at scale. Absorbed: sandbox-large, sandbox-coevolution, living-sandbox-coevolution. | `season_period = 2500` | 7 | — |
| `riverlands.toml` | 4096² self-siting continent: mountains, rain-shadow, a hydrology-carved river network | Terrain-aware placement (`kind = "habitat"`/`"near_spec"`) so herds and a persistent predator pack (`max_share` + `mate_seeking`) find water and each other regardless of seed. Absorbed: continental. | `season_period = 3000` | 7 | — |
| `huge-steppe.toml` | 8192² world (biome grid 1024²), 6k population budget | Phase-1 "Huge" scale tier: world-scale (not population-scale) throughput at the same population budget as the prior largest world. | `season_period = 5000` | 21 | — |

## Running

```
scripts/emergence.sh view  <world>   # windowed Godot sandbox — watch it live
scripts/emergence.sh run   <world>   # one headless run, tally emergent events
scripts/emergence.sh sweep <world>   # multi-seed emergence scorecard
```

`<world>` is any name above (with or without `.toml`), a file name, or a full
path. `scripts/emergence.sh` also has `capture`, `record`/`record-web` (the
showcase deck pipeline), `replay`, `soak` and `demo` — see its own header.

## Retired experiments

The one-feature demos and the O1–O3 / DIT / worldgen probes that preceded the
twelve-world consolidation are retired; `scenarios/experiments/` is gone. The
retired one-feature demos live on as inline fixtures under
`crates/anabios-core/tests/common/` where a suite still needs their exact
configuration. Seven of them pin the four experiment levers plus three other
retired mechanisms, each round-tripped in `tests/save_load_roundtrip.rs`'s
`experiment_fixtures_roundtrip`:

| Fixture | What it probes | Findings / design |
|---|---|---|
| `biome-step-interval` | Phase-1 "scale fields": the `biome_step_interval` cadence knob, trading regrowth resolution for tick throughput on huge worlds | [`2026-09-12-pixel-world-at-scale-design.md`](superpowers/specs/2026-09-12-pixel-world-at-scale-design.md) |
| `dit-env-slow` | The DIT environmental-variability axis (`env_period`): genes can't track a moving foraging optimum, a cultural critical-learner can | [`2026-07-12-dit-boundary-suite-design.md`](superpowers/specs/2026-07-12-dit-boundary-suite-design.md) |
| `drifting-climate` | `env_period`'s seasonal sweep plus a secular `climate_drift_rate` baseline wander that never stationarizes (E11 maladaptation) | [`2026-07-24-e11-maladaptation-design.md`](superpowers/specs/2026-07-24-e11-maladaptation-design.md) |
| `gene-requirements` | `gene_requirements`: invention discovery gated on genome slots (the dual-inheritance helix) | [`2026-07-31-invention-requirements-helix-design.md`](superpowers/specs/2026-07-31-invention-requirements-helix-design.md) |
| `o1-lever-practices-off` | O1 exclusion autopsy: the `practices_enabled`-off variant of the out-of-africa cradle invasion | [`2026-08-03-o1-exclusion-findings.md`](superpowers/specs/2026-08-03-o1-exclusion-findings.md) |
| `o2-payoff-biased-learning` | O2b payoff-biased social learning — measured negative | [`2026-08-07-o2b-payoff-biased-findings.md`](superpowers/specs/2026-08-07-o2b-payoff-biased-findings.md) |
| `o3-repro-biased-learning` | O3 repro-biased learning — culture becomes a viable strategy | [`2026-09-02-o3-repro-bias-findings.md`](superpowers/specs/2026-09-02-o3-repro-bias-findings.md) |

Two more retired root files round-trip as fixtures for a different reason —
not an experiment lever, but state the absorbing world doesn't reach within
its warm-up (`retired_state_fixtures_roundtrip`): `unilateral-trade` (the
lever stays off in every curated world; `markets` absorbed its founders) and
`knowledge-ratchet` (`tribes`, which absorbed it, holds no Writing within its
warm-up).
