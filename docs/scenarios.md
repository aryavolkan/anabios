# Scenarios → phenomena

`scenarios/` holds twelve worlds, each named for its setting rather than for a
single phenomenon. The scenario schema defaults every feature knob **on**
(`Scenario`, `scenarios/*.toml`) — a curated world only lists what it turns
**off**, so every file below runs the full engine stack unless its row says
otherwise. (The engine layer, `World::new`/`World::with_dims`, defaults to
nothing; see `docs/determinism-contract.md`'s "Two default layers" for the
split.) To opt a subsystem out for your own run, copy a scenario and set its
`*_enabled` knob to `false` (or `season_period = 0`). Opting out is meant to
be a no-op at the engine layer; that is pinned per subsystem by its own
flag-off tests (for example `tests/disease.rs::flag_off_is_noop`,
`territory::tests::step_is_a_noop_with_the_flag_off`), and for two whole
worlds by the flag-off trajectory guards in `tests/determinism.rs`, which
pin `minimal` and `grand-theater` as they were before the schema flip
(inline fixtures: `minimal` with every knob off, `grand-theater` with only
its own pre-flip flags on).

Four knobs stay off by default everywhere as **experiment levers**, not
curated-world features: `env_period` (DIT environmental-variability sweep),
`climate_drift_rate` (secular climate drift on top of it), `payoff_biased_learning`
(O2b, measured negative) and `unilateral_trade` (the O2.6 trade-freeze fix,
superseded by the current material economy). Three worlds below turn the
first two on deliberately; none turns on the latter two — see "Retired
experiments".

One knob changes how every agent moves rather than adding a subsystem:
`gait_enabled` (on by default like the rest; `crates/anabios-core/src/gait.rs`).
With it an agent's speed follows its urgency instead of always being its
Locomotor maximum: fleeing, fighting and hunting agents sprint (a carnivore
carrying a weapon and closing on another species is hunting), the seeking
moods walk (0.6×), a content grazer ambles (0.3×) — scaled down further by how
hard its program pushes — and the move cost carries a superlinear `1 + frac²`
factor, so a sprint costs twice per unit distance what a crawl does. Measured
on `predator-prey` (500-agent cap, ticks 300–350, `tests/gait.rs`): the mean
step over alive agents drops from 1.98 to 0.81 world units, and the share of
agent-ticks at 90% or more of the agent's top speed from 85% to 14% — the
fleeing prey and hunting pursuers. Opting out is pinned as a no-op by the
`gait_*` unit tests in `tick.rs` / `integrate.rs` (flag off: unit direction,
full step, linear move cost) and by the trajectory guards above.

`growth_enabled` (on by default, like the feature knobs; 2026-09-28) adds
growth and juveniles: an agent is born at about a third of its adult size
and grows to it over the first 15% of its lifespan — its collision body
radius, grazing bite, basal metabolism, move cost and (mildly) speed scale
with the body, and nobody breeds before maturity
(`crates/anabios-core/src/growth.rs`). Founders start at age 0, so a world's
first births come after its founders' maturity window (about 480 ticks for
`minimal`'s `lifespan_bias = 0.6`). Opt out with `growth_enabled = false`;
the engine default is off, and flag-off is byte-identical
(`growth::tests::flag_off_is_exactly_the_adult_identity` and the trajectory
guards above). The knob postdates every row's validation sweep and cost
figure below; neither has been re-run under it.

Every file is smoke-tested by `tests/all_scenarios.rs` (parse → instantiate →
200 ticks) and has a `tests/save_load_roundtrip.rs` round-trip test. The cost
column is wall-clock milliseconds per tick of `anabios-headless run --ticks
2000` at the scenario seed (release build, `real` × 1000 / 2000, instantiate
included; best of two runs on a 10-core Apple M5 laptop) — it tracks the live
population the world settles at far more than its map size. The full stack
itself costs about 4.3× the bare engine: `minimal` runs at 3.5 ms/tick against
0.81 ms/tick for its pre-flip, all-off copy
(`crates/anabios-core/tests/common/minimal.pre-flip.toml` with the
`opt-out-all.toml` knobs inserted), both at ~2000 agents by the same method.
Of that, the collision resolve (up to eight converging Jacobi passes per tick,
`crates/anabios-core/src/collision.rs`) is the largest single stage on crowded
worlds: switching it from two fixed passes to the converging form (2026-09-27)
raised these figures by roughly 15–35% (55% on `predator-prey`, whose herds
are the densest), and in exchange leaves no pair closer than half its gap and
0–3% of agents closer than 90% of it, where two passes left 11–28% of agents
touching. Read a world's own header comment for the full story — placement
technique, tuned seeds, measured caveats; this table is the index.

| World | Setting | Shows | Opt-outs / levers | Seed | Cost (ms/tick) |
|---|---|---|---|---|---|
| `minimal.toml` | 1024² plains, one lineage | Baseline grazing world; the determinism goldens pin on this scenario. | — | 12345 | 3.5 |
| `predator-prey.toml` | Four regional herds: a mixed cohort at the centre, a mammal grazer/pursuer pair north, a reptile ambusher/basker pair east, a thin forager background | Predation cycles and population crashes (`PopCrash`), the trophic cascade when predators collapse (rare: 0/8 seeds at 5000 ticks; unit-tested), mammal-vs-reptile niche sorting on the drier east, and foraging/climate-affinity selection in the background stock. Absorbed: trophic-cascade, grazers-and-wolves, mammals-vs-reptiles, foraging-selection, biome-adaptation. | Founder tuning: the mammal pursuer pack starts at 30 founders on the herd it hunts (`near_spec`, 10% share), clear of the ~10-hunter Allee floor it fell under at 14 | 0 | 2.0 |
| `speciation.toml` | One herbivore stock seeded as two body-size morphs at the west/east ends, plus a communicator, a scent-marker, a cooperator and a hunter-prey lineage | Speciation from a founder stock (`Divergence`/`Convergence`), dialect formation between the communicator clusters, pheromone territories between the marker clusters, kin cooperation, and the four DIT learning-strategy pairings. Absorbed: divergent, convergent, cooperation, territories, dialects, gene-culture, gene-culture-skill, gene-culture-hunt, gene-culture-alarm. | — | 4242 | 1.4 |
| `tribes.toml` | Ape-tier omnivore innovators, traditionalists and a stone-tool hunter band among grazer herds, armoured/spined prey and predator packs | Discovery and adoption of inventions (`Discovery`/`Adoption`; the `KnowledgeRatchet` needs Writing, discovered on 0/8 seeds in 5000 ticks), traditions, IQ-tech coevolution, weapons and war (`War`/`WarEnded`), domestication pens (they need Husbandry, likewise 0/8), sexual dimorphism, epidemics (`Epidemic`; `MedContain` needs Medicine, likewise 0/8), the anthropogenic arms race, the affect layer's moods and play, and thirst/sleep. Absorbed: inventions, tool-users, weapons, weapons-arena, weapons-arms-race, war, traditions, cognitive-coevolution, tech-gene-coupling, knowledge-ratchet, domestication, dimorphism, disease, anthro-race, affect-play, affect-seeking, affect-showcase, affect-social, affect-threat, basic-needs. | — | 60623 | 1.7 |
| `markets.toml` | Four terrain-affinity forager lineages at a biome junction beside five goods-producing grazer lineages | Home-range anchoring and settlements, resource harvesting, bilateral barter at the predetermined trade hubs, and the trade-flow/market events. Absorbed: settlement, biome-trade, geographic-trade, trade-hubs, unilateral-trade (its lever is exercised by an inline fixture in `tests/trade.rs`, not this world). | — | 424242 | 4.3 |
| `habitat-territories.toml` | Three grazer species differing only in Locomotion (land/water/air), founded together at four shared sites | Locomotion-gated habitat selection, species territories kept apart by collision-aware separation steering, and per-lineage `max_share` stopping the shared population cap from sterilizing the smaller founders. | `sexual_dimorphism_enabled = false`, the only curated opt-out: female mate choice sterilizes the size-0.3 flyers (display 0.39 against a 0.40 bar), so Air never breeds. Founder tuning: Land founds four habitat herds (as one 150-strong herd, Land dwindles on half the seeds) | 4 | 2.9 |
| `grand-theater.toml` | Everything-on staged world at the tuned geographic-trade terrain (seed 424242) | Environment, disturbance, gene-culture, economy, conflict and communication all colliding in one shared world — the strongest save/load round-trip guard. | `env_period = 400`, `climate_drift_rate = 0.00005` | 424242 | 8.8 |
| `out-of-africa-saga.toml` | The grand-theater cast on a human-dispersal geography (climate-driven worldgen) | The showcase cut: era-3 tech (Stone Tools/Fire/Farming/Writing/Husbandry) seeded on the Quarry innovators from tick 0 so downstream tech and on-camera taming emerge without stalling at era 1. | `env_period = 400`, `climate_drift_rate = 0.00005` | 318 | 7.8 |
| `out-of-africa-earth.toml` | The saga's founders re-anchored onto a real-Earth elevation/temperature/precipitation map (`world_map = "earth"`) | The same DIT/cognition/war/domestication stack, but the exodus runs through the real African corridors (Sinai/Bab-el-Mandeb, Gibraltar); dispersal is emergent, not scripted. | `env_period = 400`, `climate_drift_rate = 0.00005` | 318 | 6.6 |
| `sandbox.toml` | 2048² world, 8k population cap, no staging; seasons slowed to `season_period = 2500` | The open-ended run for watching the tech tree, gene-vs-culture and long ecological cycles at scale. Absorbed: sandbox-large, sandbox-coevolution, living-sandbox-coevolution. | — | 7 | 11.4 |
| `riverlands.toml` | 4096² self-siting continent: mountains, rain-shadow, a hydrology-carved river network; seasons slowed to `season_period = 3000` | Terrain-aware placement (`kind = "habitat"`/`"near_spec"`) so herds and a persistent predator pack (`max_share` + `mate_seeking`) find water and each other regardless of seed. Absorbed: continental. | — | 7 | 5.9 |
| `huge-steppe.toml` | 8192² world (biome grid 1024²), 6k population budget; seasons slowed to `season_period = 5000` | Phase-1 "Huge" scale tier: world-scale (not population-scale) throughput at the same population budget as the prior largest world. | — | 21 | 12.5 |

## Validation

The consolidation's bar (spec §5): each world's headline detectors fire on at
least 5 of 8 seeds, and on at least 5 of 8 seeds no founder kind is extinct and
the population ends above half its founder count — measured with
`anabios-headless sweep` over seeds 0–7 at 5000 ticks (seeds 0–3 at 2000 ticks
for `out-of-africa-earth` and `huge-steppe`). Founder counts, `max_share`,
placement and `max_population` are the levers, plus one documented opt-out
(`habitat-territories` turns sexual dimorphism off; see its row); no engine
constant moves. Two limits hold wherever the kinds concerned are founded, and
one kind depends on the world and seed:

- **Sterile kits.** `stalker`, `pack_hunter`, `marker`, `communicator`,
  `cultural_cooperator`, `culture_prey`, `skilled_forager`, `fast_hunter`,
  `slow_hunter` and the three DIT learners carry no Reproductive module. They
  act for one founder lifetime and die out by design, so the founder-kind bar
  counts the breeding kinds.
- **Spined prey.** `spiner` dies out on nearly every seed wherever it is
  founded (`tribes`, `grand-theater`, the two out-of-africa worlds) — at 10
  to 40 founders and under a doubled population cap (`grand-theater`), and
  no single opt-out among thirteen tried rescues it (`tribes`, 1500 ticks).
  No founder lever tried holds it on a majority of seeds.
- **Armoured prey.** `bruiser` persistence depends on the world and seed: it
  survives on 4/4 seeds of the unmodified `out-of-africa-earth` (2000 ticks),
  but on 1–2 of 8 in `tribes`, `grand-theater` and `out-of-africa-saga`;
  raising its founders reaches 4 of 8 in `tribes` (40 founders) and 4 of 6
  in a tripled-count saga probe (30 founders) — never a majority of 8.

Armed-prey persistence as a mechanism (Spines- and Jaws-bearing agents still
alive at tick 1500 on `tribes`) is covered by `tests/emergence.rs`
(`weapons_arms_race`).

| World | Bar | Documented gaps |
|---|---|---|
| `minimal` | met, 8/8 | — |
| `predator-prey` | met after tuning, 6/8 (was 3/8: the pursuer pack faded on 4/8 seeds) | Does not reliably show `TrophicCascade` (0/8); the detector's crash→boom→drop ordering is covered by the unit tests in `src/codex/cycles.rs`, and `tests/emergence.rs` (`population_dynamics`) asserts the E3 family on this world. |
| `speciation` | met, 6/8 (two seeds end below half the founders) | Does not show culture out-growing its asocial control in the four gene-culture pairings (A 0/12, alarm 0/12, hunt 3/10, skill-C 4/20 in the report-only `tests/gene_culture.rs` harnesses): every cultural founder is a sterile kit, and co-locating the skilled foragers with the control lifts skill-C only to 6/20. |
| `tribes` | not met, 1/8 (spiner 1/8, bruiser 2/8; 5/8 without the armed prey) | Does not reliably show adoption of a non-seeded invention (2/8), domestication (0/8), `KnowledgeRatchet` (0/8), `MedContain` (0/8) or `InstitutionalRatchet` (0/8, the deck's Ratchet chapter) within 5000 ticks. Adoption is covered by the synthetic detector test `invention_discovered_fires_once_and_adopted_fires_at_majority` in `tests/inventions.rs`; domestication and the `KnowledgeRatchet` by the fixture tests `domestication_emerges_across_seeds` in `tests/domestication.rs` and `knowledge_ratchet_emerges_across_seeds` in `tests/knowledge.rs`; `MedContain` and `InstitutionalRatchet` by the unit tests in `src/codex/disease.rs` and `src/codex/traditions.rs`. Spines/Jaws persistence to tick 1500 is covered by `tests/emergence.rs` (`weapons_arms_race`). |
| `markets` | met, 5/8 (three seeds end below half the 1282 founders) | — |
| `habitat-territories` | met after tuning, 8/8 (was 1/8: Air never bred and the world fell to a median of 25 agents) | — |
| `grand-theater` | not met, 0/8 (spiner 1/8, bruiser 2/8, sentinel 5/8, cooperator 5/8; per-lineage shares hold the cooperators but not the other three and drop the population bar to 5/8) | Does not reliably keep the sentinel and cooperator kinds; no fixture covers their persistence. A variant tripling the small breeding kinds (sentinel, herd, asocial prey, cooperator, spiner, bruiser) held sentinel and cooperator on 4/4 finished seeds but was not adopted, because spiner and bruiser still die out (and the archetype-free stock died on one seed), so the bar stays unmet. Does not reliably show invention discovery (1/8) or `KnowledgeRatchet` (0/8); the fixture tests `innovators_discover_before_traditionalists_in_demo_scenario` in `tests/inventions.rs` and `knowledge_ratchet_emerges_across_seeds` in `tests/knowledge.rs` cover them, and `out-of-africa-saga` shows both (7/8, 8/8). |
| `out-of-africa-saga` | not met, 0/8 (spiner 1/8, bruiser 1/8, sentinel 5/8, herd 6/8, asocial prey 6/8) | Does not reliably keep the sentinel, herd and asocial-prey kinds; no fixture covers their persistence. The same tripled-count variant held them on 6/6 finished seeds but was not adopted, because spiner still dies out (alive on 1/6), so the bar stays unmet while the showcase replay would need re-recording. Does not show `EvolvedTool` (0/8; it needs species-level Metalworking adoption), so the deck's ToolUse chapter times out; the unit test in `src/codex/signatures.rs` covers the detector. |
| `sandbox` | met, 8/8 | — |
| `riverlands` | met, 8/8 | — |
| `out-of-africa-earth` | not met, 0/4 (spiner 0/4, sentinel 1/4, asocial prey 2/4) | Does not reliably keep the sentinel and asocial-prey kinds; no fixture covers their persistence. The tripled-count variant was not probed on this map (it held those kinds on the saga's cast, which this world shares) and was not adopted, because spiner still dies out there. Does not reliably show invention discovery within 2000 ticks (2/4); covered as for `grand-theater`. |
| `huge-steppe` | met, 4/4 | — |

The collision resolve's move from two fixed passes to the converging form
(2026-09-27) shifts every trajectory, so the bar was re-run with the same
harness on the two worlds it moves most: `habitat-territories` holds 8/8 and
`predator-prey` holds 6/8 (pursuer and ambusher each alive on 7/8). The
predator-prey showcase deck's seed moved from 14 to 0 in the same change,
because seed 14 no longer produces the `PopulationCycleDetected` event its
PopCycle chapter waits on under the new resolve (seed 0 does, at tick 650).
The other rows date from the consolidation sweep under the two-pass resolve.

The collision audit of 2026-09-28 (every world, 2000 ticks, every tick
checked for colliding pairs closer than half their gap — the numbers are in
the pull request that landed it) moved every trajectory again, through five
changes that together leave no pair closer than half its gap on any world
and no pair passing through another: every move is swept to its first
contact and slides along the body it meets (`collision::sweep_moves`; before
it two agents stepping two body lengths in opposite directions swapped
through each other tens of thousands of times per world per 2000 ticks); a
newborn is placed clear of both parents' bodies and, if that spot holds a
third body, at the nearest free spot instead of on its parents' midpoint
(births run after the resolve, so every birth was a stacked pair for a
tick); the trade-hub pull is off inside `HUB_TRADE_RANGE` and fades in
beyond it (a constant pull to the hub's centre pressed 225 bodies into a
6-unit radius on `grand-theater`, a pile no bounded resolve can unpack);
the water pull stops once the agent can already drink where it stands (it
pinned shoreline crowds against the coast); and the resolve's pass cap rose
from eight to thirty-two, which a calm tick never reaches. Herds move less
freely now that bodies cannot walk through one another (the median step on
`predator-prey` fell from 2.3 to 0.6 units; the fraction of agents moving
at all is unchanged), and their populations settle slightly lower. The
validation bar was not re-run for this change.

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
retired mechanisms, each round-tripped by its own test in
`tests/save_load_roundtrip.rs`'s `experiment_fixtures` module:

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
its warm-up (the `retired_state_fixtures` module): `unilateral-trade` (the
lever stays off in every curated world; `markets` absorbed its founders) and
`knowledge-ratchet` (`tribes`, which absorbed it, holds no Writing within its
warm-up).
