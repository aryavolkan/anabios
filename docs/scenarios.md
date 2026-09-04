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
