# Scenarios → phenomena

Which scenario demonstrates what, and the opt-in flags it enables. Every file
is smoke-tested by `tests/all_scenarios.rs` (parse → instantiate → 200 ticks,
recursively). `scenarios/experiments/` holds archived ablation suites (O1/O2,
DIT boundary, biome/climate variants) — see its own README; this table covers
the curated root set. Run any of these with
`scripts/emergence.sh view <name>` (viewer), `run <name>` (one tally), or
`sweep <name>` (scorecard).

| Scenario | Phenomenon | Flags on |
|---|---|---|
| `minimal.toml` | Baseline grazing world; determinism goldens | — |
| `divergent.toml` | Speciation from one founder stock | — |
| `convergent.toml` | Trait convergence under shared selection | — |
| `cooperation.toml` | Kin-directed cooperation | — |
| `predator-prey.toml` | Predation, collapse-and-recovery cycles | — |
| `trophic-cascade.toml` | Top-down cascades through three trophic levels | — |
| `territories.toml` | Pheromone territories & scent marking | — |
| `dialects.toml` | Meme divergence between clusters; dialect formation | — |
| `traditions.toml` | E9 meme-lineage variants: traditions sweeping a culture | inventions, settlement, living_biome |
| `war.toml` | Kin-group warfare (`War`/`WarEnded` events) | war |
| `weapons-arena.toml` | Weapon-trait tournaments | inventions |
| `weapons-arms-race.toml` | Weapon/armor coevolution (`ArmsRace`) | inventions |
| `gene-culture.toml` | Gene-culture coevolution baseline (DIT) | — |
| `gene-culture-skill.toml` / `gene-culture-hunt.toml` / `gene-culture-alarm.toml` | DIT technique-channel variants | — |
| `tool-users.toml` | Invention adoption in a foraging band | inventions |
| `inventions.toml` | The 10-tech invention race (discovery → adoption → ratchet) | inventions |
| `cognitive-coevolution.toml` | Cognition (IQ) evolving alongside culture | inventions, cognition |
| `tech-gene-coupling.toml` | `gene_tech_coupling`: Openness ↔ invention spread | inventions, cognition, gene_tech_coupling |
| `knowledge-ratchet.toml` | E14 knowledge accumulation: Writing-backed tech memory survives bottlenecks | inventions, knowledge |
| `disease.toml` | Disease: crowding spillover → outbreak in a susceptible herd; medicine-bearing innovators resist (`EpidemicOutbreak`, `MedicineContainment`) | disease, inventions |
| `dimorphism.toml` | Sexual selection via female mate choice | sexual_dimorphism |
| `domestication.toml` | Taming, milk herds, born-tamed livestock | inventions, domestication |
| `anthro-race.toml` | Anthropogenic arms race: tagged culture-bearers hunt herds that evolve aimed vigilance (`HuntedAdaptation`) | inventions, affect, war, anthro_race |
| `biome-adaptation.toml` | Terrain-affinity adaptation to biomes | biome_adaptation |
| `foraging-selection.toml` | Nutrient/soil gradients driving foraging traits | nutrient_variation, soil_fertility |
| `disturbance.toml` | Disasters + succession in a living biome | disasters, living_biome |
| `living-sandbox-coevolution.toml` | Living biome + seasonal regrowth at scale | living_biome, season_period |
| `sandbox-coevolution.toml` | Freeform coevolution sandbox | living_biome, season_period, inventions |
| `sandbox-large.toml` | 2048² world (custom dims; save/load round-trip pin) | living_biome, season_period |
| `riverlands.toml` | 4096² world with mountains + a river network; herds auto-sited on watered forage, predators seeded onto the herds (terrain-aware placement) | living_biome, season_period, basic_needs |
| `biome-trade.toml` | Biome trade-goods economy (freezes ~t10k — the baseline) | resources, living_biome |
| `geographic-trade.toml` | Terrain-sorted trade (sputters, never fully freezes) | terrain_habitat, resources |
| `unilateral-trade.toml` | The O2.6 freeze fix: surplus gifts + goods conserved on death | resources, conserve_goods_on_death, unilateral_trade, living_biome |
| `settlement.toml` | Settlements & market formation | terrain_habitat, settlement, resources |
| `affect-seeking.toml` | SEEKING drive shaping foraging | affect |
| `affect-social.toml` | CARE/PANIC/PLAY social affect | affect |
| `affect-threat.toml` | FEAR/RAGE threat responses | affect |
| `affect-play.toml` | Juvenile PLAY enrichment | affect, cognition |
| `affect-showcase.toml` | M-F observability: panic cascades, feeding frenzies | affect |
| `mammals-vs-reptiles.toml` | Vertebrate-class archetypes (endotherm vs ectotherm profiles) | affect, cognition, biome_adaptation |
| `grand-theater.toml` | Everything-on staged world (strongest round-trip guard) | env_period, climate_drift_rate, season_period, biome_adaptation, living_biome, nutrient_variation, soil_fertility, disasters, terrain_habitat, resources, settlement, inventions, gene_tech_coupling, cognition, war |
| `out-of-africa.toml` | The flagship grand arc — measured to stall at era 1 (see `docs/showcase-plan.md`) | same set as grand-theater + sexual_dimorphism, domestication |
| `out-of-africa-saga.toml` | The showcase cut: era-3 tech seeded at t0, downstream tech emerges | same set as `out-of-africa` |

## Terrain-aware placement

Most scenarios place founders with `kind = "cluster"` and a literal
`center_x`/`center_y`. Those coordinates are only correct for the seed they
were scouted against — `continental.toml` documents the manual densest-patch
scout run that produced its pair — so re-seeding such a scenario can drop the
cohort in the ocean.

Two placements re-derive their sites from the generated world instead, which
is what makes a large procedural world seedable at all:

- `kind = "habitat"` — anchors `herds` sites on cells that carry forage and
  sit within `max_water_dist` of a drinkable cell (`needs::drinkable_cell`,
  the same predicate the thirst drive uses), then scatters agents within
  `radius`. Absent `max_water_dist` defaults to two cells of the actual
  field. Anchors are spread apart best-effort. If the world grows nothing at
  all, it falls back to uniform rather than failing the run.
- `kind = "near_spec"` — scatters within `radius` of the agents an *earlier*
  `[[agents]]` spec already placed, so predators find their prey wherever the
  terrain put it. `spec` is an index into the `[[agents]]` array and must be
  strictly less than the spec's own index (specs are placed in file order);
  a forward or self reference is rejected at load.

Both are demonstrated by `riverlands.toml` and covered by
`crates/anabios-core/tests/habitat_placement.rs`, which asserts the siting
properties across a span of seeds rather than one.

A caution that outlives this feature: `river_threshold` in a `[climate]`
block thresholds a flow accumulation counted in *upstream grid cells*, so it
is meaningless without its `biome_res`. Roughly 1% of the map becomes river
at threshold ~40 (res 128), ~80 (256), ~150 (512), ~220 (1024) — the same
150 that works at 512 carves five cells in a whole 128-res world. Run
`cargo run --release -p anabios-core --example river_scaling` before copying a
`[climate]` block between worlds of different scale.

Notes:

- Flags not listed for a scenario are off/absent (all opt-in flags default
  off, except `practices_enabled`, which defaults on wherever cognition is
  on — see the O1 finding, `docs/superpowers/specs/2026-08-03-o1-exclusion-findings.md`).
- `payoff_biased_learning` (O2b, measured negative) has no curated root
  scenario; its experiment lives at
  `scenarios/experiments/o2-payoff-biased-learning.toml`.

## Decks tier (`scenarios/decks/`)

The showcase garden — deck-dedicated scenarios that back the web replay player and the
cinematic decks in `game/showcase/` — see [`scenarios/decks/README.md`](../scenarios/decks/README.md)
for the tier conventions and the current deck → scenario · seed · asset pin registry. Unlike
the core set, garden scenarios are pinned to a *recording* rather than a phenomenon claim;
`crates/anabios-core/tests/deck_scenarios.rs` enforces the pin contract (curated deck →
`scenario=<name>` + `seed` resolve and run 200 ticks at the pinned seed). The current
decks back onto core scenarios:

| deck | scenario · seed | asset |
|---|---|---|
| `out-of-africa-saga.json` | `out-of-africa-saga.toml` · 318 | web replay (`showcase/replay.js`) + `runs/showcase/out-of-africa-saga.mp4` |
| `predator-prey.json` | `predator-prey.toml` · 0 | `runs/showcase/predator-prey.mp4` |
| `dialects.json` | `dialects.toml` · 0 | `runs/showcase/dialects.mp4` |
| `inventions.json` | `inventions.toml` · 0 | `runs/showcase/inventions.mp4` |
