# Scenario consolidation: full stack by default, twelve worlds

- Date: 2026-09-26
- Status: design approved in conversation; implementation plan pending
- Branch: `claude/scenario-consolidation`
- Related: `docs/scenarios.md` (rewritten by this work), `docs/determinism-contract.md`
  (gains the two-layer default rule), `docs/superpowers/specs/2026-09-25-territory-habitat-collision-design.md`
  (the last opt-in layer; the source of the flag-off discipline this design keeps)

## Goal

Scenarios stop being one-feature demos. A scenario file that says nothing runs the
whole engine; the curated set shrinks from 81 files (52 root, 29 experiments) to
twelve worlds organised by setting and founders; every reader of the scenario set
(tests, Godot menu, web atlas, decks, docs) follows.

## Decisions (made with the maintainer, 2026-09-26)

1. **Defaults flip at the scenario schema, not the engine.** `Scenario` feature
   knobs default to on; `World::new` stays all-off.
2. **Twelve worlds by setting** (table below), not one per feature group.
3. **`scenarios/experiments/` is deleted.** The four TOMLs that tests pin are inlined
   into the tests that need them. Findings stay in `docs/superpowers` and memory.
4. **Tests are re-targeted, with fixtures where needed.** No suite is weakened
   silently: a phenomenon that no longer appears in any world keeps its old scenario as
   an inline fixture in its test.
5. **The out-of-africa saga is converted and re-recorded.** `showcase/replay.js` is
   regenerated from the converted scenario (the publish workflow already does this from
   source); the Godot deck is event-driven and re-validated by running it.
6. **Four knobs stay off as experiment levers**: `env_period`, `climate_drift_rate`,
   `payoff_biased_learning`, `unilateral_trade`.

## 1. Defaults

### Schema layer (`crates/anabios-core/src/scenario.rs`)

Every feature knob gets `#[serde(default = "default_on")]` (a `fn default_on() -> bool
{ true }`) and its doc comment is rewritten from "Opt-in … `false` (default)" to
"On by default … set `false` to opt out". Numeric feature knobs get a feature default:

| Knob | New default | Note |
|---|---|---|
| `season_period` | `2000` | grand-theater's value; `0` opts out |
| `env_period` | `0` | experiment lever, unchanged |
| `climate_drift_rate` | `0.0` | experiment lever, unchanged |
| `payoff_biased_learning` | `false` | superseded by `repro_biased_learning` (O2/O3) |
| `unilateral_trade` | `false` | exchange-primitive experiment (PR #103 line) |

On by default (26): `biome_adaptation`, `terrain_habitat`, `inventions_enabled`,
`gene_tech_coupling`, `gene_requirements`, `cognition_enabled`, `affect_enabled`,
`living_biome`, `season_period`, `nutrient_variation`, `soil_fertility`,
`resources_enabled`, `conserve_goods_on_death`, `disasters_enabled`, `war_enabled`,
`settlement_enabled`, `sexual_dimorphism_enabled`, `domestication_enabled`,
`knowledge_enabled`, `practices_enabled`, `basic_needs_enabled`,
`mate_seeking_enabled`, `territory_enabled`, `disease_enabled`,
`anthro_race_enabled`, `repro_biased_learning`.

`Scenario::default()` (used by `parse_toml` of a bare `name`/`seed` file and by the
unit tests inside `scenario.rs`) therefore builds a full-stack world. Validation rules
that already exist (`knowledge_enabled` requires `inventions_enabled`,
`culture_bearer` requires `anthro_race_enabled`, `starting_inventions` requires
`inventions_enabled`) become trivially satisfied by the defaults; a scenario that opts
out of `inventions_enabled` while keeping `knowledge_enabled` on still fails to parse,
as today — the error message is updated to mention opting out.

### Engine layer (`World::new`, `World::with_dims`)

Unchanged: every subsystem flag is `false`. This keeps:

- the ~530 core unit tests that build a `World` directly and assert per-subsystem
  arithmetic or byte-identity with the flag off;
- the `*_is_a_noop_with_the_flag_off` style tests;
- the trajectory guards in `tests/determinism.rs`, which pin the flag-off engine at
  `minimal` and `grand-theater` — these keep their scenarios' flags **explicitly off**
  (see "Tests") so the pins stay meaningful after the schema flip.

`docs/determinism-contract.md` gains a section "Two default layers": the scenario schema
defaults to the full stack, the engine defaults to nothing; a subsystem's flag-off
byte-identity guarantee is still stated and tested at the engine layer.

## 2. The twelve worlds

`scenarios/` contains exactly these files after the change. Each keeps its own seed,
world size, founders and placement; each omits every feature knob it does not opt
out of. "Absorbs" lists the deleted files whose distinct ingredient (founders, map,
placement) the world inherits — not their flags, which are now universal.

| File | Setting | Absorbs |
|---|---|---|
| `minimal.toml` | 200 grazers on the 1024 default map; golden baseline and smoke | — |
| `predator-prey.toml` | grazers, wolves and a third trophic level on one map | `trophic-cascade`, `grazers-and-wolves`, `mammals-vs-reptiles`, `foraging-selection`, `biome-adaptation` |
| `speciation.toml` | one founder stock on a climate gradient with clustered founders | `divergent`, `convergent`, `cooperation`, `territories`, `dialects`, `gene-culture`, `gene-culture-skill`, `gene-culture-hunt`, `gene-culture-alarm` |
| `tribes.toml` | tool-using omnivore lineages (ape tier) among wild grazers and predators | `inventions`, `tool-users`, `weapons`, `weapons-arena`, `weapons-arms-race`, `war`, `traditions`, `cognitive-coevolution`, `tech-gene-coupling`, `knowledge-ratchet`, `domestication`, `dimorphism`, `disease`, `anthro-race`, `affect-play`, `affect-seeking`, `affect-showcase`, `affect-social`, `affect-threat`, `basic-needs` |
| `markets.toml` | a terrain junction with resource nodes and predetermined hubs | `settlement`, `biome-trade`, `geographic-trade`, `trade-hubs`, `unilateral-trade` |
| `habitat-territories.toml` | Land / Water / Air grazers, per-lineage `max_share` | — |
| `grand-theater.toml` | staged emergence, 3k cap | — |
| `out-of-africa-saga.toml` | the 1024 saga; the recorded web replay | `out-of-africa` |
| `out-of-africa-earth.toml` | the Earth raster at 4096 | — |
| `sandbox.toml` | 2048 map, 8k cap, tech tree, gene-vs-culture founders | `sandbox-large`, `sandbox-coevolution`, `living-sandbox-coevolution` |
| `riverlands.toml` | 4096 procedural continents with rivers | `continental` |
| `huge-steppe.toml` | 8192 scale test (`snapshot_size` pins it) | — |

Deleted outright (no world inherits a distinct ingredient): everything under
`scenarios/experiments/` and the root files listed in "Absorbs".

Founder design per merged world is part of the implementation plan, not this spec, but
the rule is fixed here: a world's founders are the union of the distinct founder
*kinds* of the files it absorbs (e.g. `tribes` keeps an ape-tier omnivore lineage, a
grazer herd and a predator pack; `speciation` keeps a single founder stock), with
counts and `max_share` set so the per-lineage caps from the territory layer keep every
kind alive through the validation horizon.

## 3. Tests

Rules, applied per suite in `crates/anabios-core/tests/`:

1. **Re-target by ingredient.** A suite that pinned a deleted file points at the world
   that absorbed it (the table above) and is re-run with the full stack.
2. **Fixture when the phenomenon vanishes.** If the detector or property the suite
   asserts no longer appears in any world within the suite's horizon, the deleted file's
   TOML is inlined into that suite as a `const FIXTURE: &str` with a comment naming the
   world that replaced it and why the fixture is kept. The fixture opts out of nothing
   beyond what the original file had (its flags are written explicitly so the schema
   flip does not change it).
3. **Flag-off pins are explicit.** The trajectory guards keep separate flag-off copies
   of `minimal` and `grand-theater` as inline fixtures with every knob written `false`,
   so the pinned values `0xd1133dd8d119e894` / `0x56819428b6cd2bf0` do not move. The
   golden (`state_hash`) suites run on the real files and are re-pinned once, with
   `UPDATE_HASHES`, after the guards confirm the engine did not move.
4. **Experiment TOMLs** pinned by `save_load_roundtrip` (`biome-step-interval`,
   `dit-env-slow`, `drifting-climate`, `gene-requirements`, `o1-lever-practices-off`,
   `o2-payoff-biased-learning`, `o3-repro-biased-learning`) and `dit_boundary` become
   inline fixtures in those suites; their flags are written explicitly.
5. **`all_scenarios`** enumerates the twelve. `deck_scenarios` follows the menu.
6. **Coverage floor** (anabios-core >= 95% lines) is checked with the `cfg(coverage)`
   horizon; no suite is deleted.

## 4. Readers of the scenario set

- **Godot menu** (`game/scripts/menu.gd`): twelve entries, grouped Foundations /
  Worlds / Scale; labels drop the milestone prefixes ("E7 —", "M13 —").
- **Godot decks** (`showcase_director.gd`): event-driven; re-validated by running the
  saga deck to its final chapter.
- **Web atlas** (`web/scripts/manifest.mjs`): follows the directory; the `flags` list
  it extracts from top-level `*_enabled = true` lines is replaced by "all except the
  explicit opt-outs", read from `= false` lines, so the atlas shows what is *off*.
- **Web replay** (`showcase/replay.js`): regenerated locally with
  `anabios-headless record --scenario scenarios/out-of-africa-saga.toml --seed <validated>`
  and committed; the publish workflow regenerates it again from source.
- **`docs/scenarios.md`**: rewritten as a twelve-row table keyed by setting, with the
  phenomena each world shows and the explicit opt-outs; the experiments paragraph goes.
- **`scenarios/experiments/README.md`**: deleted with the directory; its summary moves
  to a short "Retired experiments" appendix in `docs/scenarios.md` pointing at the
  findings docs.
- **`scripts/emergence.sh`** and `.github/workflows` references (`minimal`,
  `predator-prey`, the saga) are checked and updated.

## 5. Validation protocol

Nothing in this design is declared done on the strength of the flip alone; the memory
of this project says emergence hypotheses fail constantly.

1. **Engine unchanged:** the flag-off trajectory guards pass before any golden is
   re-pinned.
2. **Every world, eight seeds:** `scripts/emergence.sh sweep <world>` (or the headless
   tally) over seeds 1–8 at each world's horizon; the world's headline phenomena (the
   detectors its absorbed suites asserted) must fire on a majority of seeds, and no
   world may go extinct or collapse to one founder kind on a majority of seeds.
3. **Known interactions to tune for:** territory on makes every neutral-genome agent
   Land (Earth-map migration only via land bridges; founders in water are relocated);
   cognition on suppresses culture unless the ape tier and repro-biased learning carry it
   (O1/O3 findings); disease plus disasters can crash small founder sets. Founder counts,
   `max_share`, `max_population` and placement are the levers; engine constants are not.
4. **Cost:** `bench_territory`-style A/B of `minimal` old vs new flags at 10k agents,
   and a wall-clock per-tick figure for each world at its founder population, recorded
   in `docs/scenarios.md`.
5. **Viewer and atlas:** headless smoke on the twelve; a windowed capture of the saga
   deck and of `habitat-territories`.

## 6. Out of scope

- Any engine constant or mechanism change. If a world cannot be made to show a
  phenomenon by founder design alone, the phenomenon keeps its fixture test and the gap
  is recorded in `docs/scenarios.md`.
- New rendering for the web atlas.
- A scenario inheritance or include mechanism.

## 7. Rollout

One branch, one PR, commits in this order so each is reviewable and the tree is green
at the end (goldens move once, in the last code commit):

1. `feat(scenario)`: schema defaults flip + knob docs + determinism-contract section.
2. `feat(scenarios)`: the twelve files (new founders for the merged worlds); delete the
   rest and `experiments/`.
3. `test`: re-targeting, fixtures, flag-off guard fixtures, golden re-pin.
4. `feat(viewer,web)`: menu, manifest flags, replay re-record.
5. `docs`: `docs/scenarios.md` rewrite, memory notes.

FORMAT_VERSION does not change (no serialized layout moves).
