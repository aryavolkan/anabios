# anabios

> Greek *ἀναβίωσις* — life arising.

A discovery-driven evolutionary sandbox where complex ecosystems emerge from simple agent rules. You seed worlds with terrain and starter species, then watch — and catalogue what unfolds.

Not a neuroevolution project. Agents have **simple, hand-engineered cognition** (a tiny evolvable behavior program) combined with a **float genome** and a **modular body plan**. Speciation, migration, predator/prey cycles, dialects, and named behaviors (flight, ambush, cooperation) emerge from local interactions; the **codex** records the first time each phenomenon appears in your worlds.

## Status

Design at [`docs/superpowers/specs/2026-05-23-anabios-design.md`](docs/superpowers/specs/2026-05-23-anabios-design.md). Shipped to date (git tags `m1`–`m10` plus later batches):

- **Core sim** — deterministic SoA agent simulation: uniform-grid spatial hashing, evolvable postfix behavior programs, 50-slot float genome, modular morphology, speciation
- **Interaction substrate (M11–M15)** — combat & predation, carcass scavenging, pheromone fields, communication/meme culture, kin-directed cooperation
- **Invention tree** — 10-tech cumulative culture tree (Stone Tools → Fire → Farming/Metalworking → Writing/Medicine/Husbandry → Machinery/Electricity/Nuclear Power) riding the meme channels: individual discovery (Openness + skill gated), social spread, per-holder buffs *and* debuffs (metabolism, upkeep, crowding stress, biome pollution, radiation mutation); `InventionDiscovered`/`InventionAdopted` codex events. Opt-in per scenario (`inventions_enabled`)
- **Codex** — 59 emergence detectors (extinction → herd cohesion → invention adoption → sexual selection → knowledge ratchet → affect cascades) writing a persistent event timeline
- **Experiments** — DIT gene-culture technique model; biome climate adaptation (opt-in per scenario); runtime world dimensions + living/seasonal biomes
- **Cognition** — realized IQ (metabolic cost, era gates) evolving under selection; maladaptive practices (Inbreeding, Child Sacrifice) spread by payoff-blind transmission — the measured culture-exclusion lever (O1). `cognition_enabled` / `practices_enabled`
- **Affect layer (mA–mF)** — primitive-brain affect (SEEK/FEAR/RAGE/LUST/CARE/PANIC/PLAY) with hijack, panic cascades, feeding frenzies, territorial rage, mass grief. Opt-in (`affect_enabled`)
- **Knowledge accumulation (E14)** — Writing-backed per-culture tech memory that survives population bottlenecks; `KnowledgeRatchet` event. Opt-in (`knowledge_enabled`, rides `inventions_enabled`)
- **Trade economy** — biome trade goods with bilateral barter; the late-run freeze is measured and fixed opt-in (`resources_enabled`, `conserve_goods_on_death`, `unilateral_trade` — see `docs/superpowers/specs/2026-08-02-trade-freeze-diagnosis.md`)
- **Payoff-biased learning (O2b, experimental)** — model + content bias in cultural transmission. Opt-in (`payoff_biased_learning`); measured negative on the energy proxy, see `docs/superpowers/specs/2026-08-07-o2b-payoff-biased-findings.md`
- **Sexual dimorphism (E12)** — opt-in binary sex + female mate choice: `SexualDimorphism` gene scales male upkeep/damage/display and female metabolic efficiency; `MateChoosiness` sets the female acceptance bar; `SexualSelection`/`SexRatioCollapse` codex events. Opt-in per scenario (`sexual_dimorphism_enabled`)
- **Domestication (E13)** — Husbandry holders tame wild juvenile herbivores into penned livestock (movement override toward the owner), draw per-tick milk yields from surplus adults, and herds breed born-tamed; `AnimalDomesticated`/`LivestockHerd` codex events. Opt-in per scenario (`domestication_enabled`, rides `inventions_enabled`)
- **Anthropogenic arms race** — scenario-tagged `culture_bearer` lineages ("humans") are perceptible to wild agents as tool-bearing threats (new sensor + evolvable `SenseCultureThreat` program node + the `Vigilance` gene's FEAR gain); the `HuntedAdaptation` codex event fires when a hunted prey lineage's armor/speed/vigilance co-rises with its culture predator's tech era/weapon damage. Opt-in per scenario (`anthro_race_enabled`, spec: `docs/superpowers/specs/2026-08-19-anthro-arms-race-design.md`)
- **Disease & epidemiology (H1)** — crowding-seeded SIS pathogen: zoonotic spillover in dense populations, proximity spread, energy-drain mortality via the normal starve path; Medicine finally has a counter-pressure (holders transmit 0.25× and recover 3×). `EpidemicOutbreak`/`MedicineContainment` codex events (60/61). Opt-in per scenario (`disease_enabled`, spec: `docs/superpowers/specs/2026-09-01-disease-epidemiology-design.md`)
- **Vertebrate classes** — mammal/reptile founder archetypes (`mammal_grazer`, `mammal_pursuer`, `reptile_ambusher`, `reptile_basker`) pairing class body plans with affect/cognition genome profiles: endotherm-approximated mammals (high metabolism, big-brained, social, bold) vs ectotherm-approximated reptiles (cheap idle, armored, hair-trigger freeze-fight-flight, ambush Jaws). Demo: `scenarios/mammals-vs-reptiles.toml`
- **Viewer** — Godot 4.6+ client: biome/species/pheromone overlays, inspector, codex panel, co-evolution charts, per-species tech panel
- **Tooling** — headless sweep CLI (parallel seeds → JSONL + CSV) with archive-weighted emergence scoring (`docs/emergence-corpus.md`), save/load snapshots (`docs/determinism-contract.md`), criterion benchmark suite

## Scenarios

`scenarios/` holds the curated, test-pinned set (43 TOMLs) — every file is smoke-tested by `tests/all_scenarios.rs` (parse → instantiate → 200 ticks, recursively over the tree) and most back a dedicated integration test, a viewer menu entry, or a gallery/showcase capture. **The full scenario → phenomenon → flag map is [`docs/scenarios.md`](docs/scenarios.md); a clone-to-finding walkthrough is [`docs/reproduce.md`](docs/reproduce.md).** Highlights:

| Scenario | Demonstrates |
|---|---|
| `minimal.toml` | Baseline grazing world; determinism goldens |
| `divergent.toml` / `convergent.toml` | Speciation / trait evolution |
| `predator-prey.toml` / `trophic-cascade.toml` | Predation, arms races, cascades |
| `war.toml` / `weapons-arms-race.toml` / `weapons-arena.toml` | Kin war, weapon coevolution |
| `biome-trade.toml` / `geographic-trade.toml` / `settlement.toml` | Trade economies & markets |
| `inventions.toml` / `cognitive-coevolution.toml` / `knowledge-ratchet.toml` | Invention tree, cognition, writing |
| `dimorphism.toml` / `domestication.toml` | Sexual selection, livestock |
| `anthro-race.toml` | Human-vs-animal arms race (`HuntedAdaptation`) |
| `basic-needs.toml` | Thirst + sleep drives, rivers, dehydration |
| `mammals-vs-reptiles.toml` | Vertebrate-class archetypes |
| `out-of-africa.toml` / `out-of-africa-saga.toml` | The flagship every-feature-on arc |
| `grand-theater.toml` / `sandbox-large.toml` | Staged & freeform large worlds |

`scenarios/experiments/` holds archived experiment suites (O1 exclusion ablations, DIT boundary suite, biome/climate variants) — kept runnable and smoke-tested, but out of the viewer menu; see `scenarios/experiments/README.md`.

## Testing

```bash
cargo test --workspace                      # unit + integration suite
cargo test --workspace --tests --release   # full gate incl. long emergence tests (CI)
cargo bench -p anabios-core                # criterion: tick / stages / scavenge
```

The determinism gate (`tests/determinism.rs`) pins golden state hashes at ticks 0/100/1000 of the minimal scenario. If a change is *intentionally* behavior-altering, regenerate with `UPDATE_HASHES=1 cargo test -p anabios-core --test determinism -- --nocapture` and copy the printed values into the test.

## Performance

Deterministic (bit-identical per seed) and fast enough for long runs — measured with the criterion suite in `crates/anabios-core/benches/tick_bench.rs`:

| Workload | Time |
|---|---|
| full tick @ 1k agents | ~0.75 ms |
| full tick @ 10k agents | ~2.5 ms |

(10-core machine; `sense`/`decide` run parallel over rayon, codex detectors share one fused per-species aggregation pass.)

```bash
cargo bench -p anabios-core          # tick, stages, and scavenge groups
```

## Running a sweep (headless)

Run N seeds of a scenario in parallel and dump per-run codex events + a CSV summary:

```bash
cargo build --release --bin anabios-headless
./target/release/anabios-headless sweep \
    --scenario scenarios/divergent.toml \
    --seeds 32 --ticks 5000 \
    --out runs/divergent-32
cat runs/divergent-32/summary.csv
```

The summary CSV has columns `seed, ticks, final_alive, final_biomass, state_hash, extinction, pop_crash, speciation, migration, novel_module, novel_behavior, predation, combat_raid, arms_race, territory_formation, niche_partitioning, dialect_formed, meme_sweep, alarm_call, evolved_cooperation, pack_hunting, herd_cohesion, invention_discovered, invention_adopted, practice_discovered, practice_adopted, resource_traded, material_learning, sexual_selection, sex_ratio_collapse, animal_domesticated, livestock_herd, hunted_adaptation, epidemic_outbreak, medicine_containment, emergence_score, novel_events, coverage` — pipe it into a spreadsheet or a notebook to mine for rare events. The per-seed `seed_NNNNNNNN.events.jsonl` files contain the full event stream for each run.

The last three columns are the **emergence scorecard**: `emergence_score` sums rarity weights (IDF) over the distinct event types a run fired, `coverage` is the fraction of all event types fired, and `novel_events` counts fired types never seen in the reference corpus. Pass `--archive runs/corpus-dir/` to recompute weights empirically against prior sweeps; runs firing corpus-unseen event types are copied to `<out>/novel/`. Use `emergence_score` as the metric when optimizing sweeps for discovery. See `docs/superpowers/specs/2026-07-22-e1-emergence-scorecard-design.md`.

## Watching the invention race (headless demo)

The `demo` subcommand narrates cultural advancement between competing populations — discovery/adoption events as they fire, per-culture tech tables, and final standings. Cultures are tracked by lineage ancestry (speciation splinters stay in their founders' culture):

```bash
cargo build --release --bin anabios-headless
./target/release/anabios-headless demo \
    --scenario scenarios/inventions.toml \
    --ticks 8000 --report-every 1000
```

`scenarios/inventions.toml` seeds three populations — high-Openness **innovators**, low-Openness **traditionalists** (who rarely invent but copy what diffuses in), and an acultural control group — competing for one grazing range. Expect the innovators to climb the tree (discoveries tick ~300–2700), the traditionalists to adopt each invention a few hundred ticks later via pure social diffusion, and the control group to stay at era 0. The same scenario is in the Godot viewer's menu ("Inventions — innovators vs traditionalists") with a per-species tech panel and adoption-fraction charts.

## Running the viewer

1. Build the gdext cdylib:
   ```bash
   cargo build -p anabios-godot
   ```
2. Open `game/project.godot` in Godot 4.6+ (or import via `godot --headless --import --path game/`).
3. Press F5 to run the main scene.
   - Mouse wheel: zoom; middle-drag or WASD/arrow keys: pan
   - Bottom-left buttons: pause + speed (1× / 4× / 16× / 64×)
   - Left-click an agent (within 4 world units) to pin its stats in the inspector panel
   - Scrolling list at bottom-right shows codex events as they fire
    - **R**: replay the latest codex event (rewind to a snapshot, fast-forward, pause at the moment; R/Esc resumes live) · **U**: run at max speed until the next event fires · **V**: event camera — auto-cut tour of recent event locations

## Recording a showcase video

The viewer has a cinematic **showcase director**: a JSON beat timeline (camera moves, chapter title cards, event-triggered cuts, overlay switches) played over the live sim and captured with Godot's Movie Maker:

```bash
scripts/emergence.sh record out-of-africa-saga --seed 318
# → runs/showcase/out-of-africa-saga.mp4
```

The saga timeline (`game/showcase/out-of-africa-saga.json`) narrates the out-of-africa arc in seven chapters — cradle, tools, exodus, settlement, writing, domestication, war — cutting to the latest `Market`/`Domesticated`/`War` events wherever they fire. Timelines live in `game/showcase/*.json`; pass `--timeline`/`--out`/`--max-seconds` to customize (the beat format — triggers, timeouts, actions — is documented at the top of `game/scripts/showcase_director.gd`). Needs ffmpeg for the AVI→MP4 conversion.

## Verifying emergence replay (headless)

`replay` re-simulates every codex event from periodic snapshots and asserts bit-identical reproduction — same state hash at the event tick, same event refiring at the same tick. It exits non-zero on any mismatch, so it doubles as the detector-regression gate:

```bash
./target/release/anabios-headless replay \
    --scenario scenarios/weapons-arms-race.toml \
    --ticks 2000 --snapshot-every 250
```

## Stack

- **`anabios-core`** — pure Rust simulation crate (no Godot, no I/O, deterministic)
- **`anabios-godot`** — gdext wrapper for use from the Godot project
- **`anabios-headless`** — CLI for batch runs, W&B sweeps, codex mining
- **`game/`** — Godot 4.6+ project (viewer, codex UI, world setup, scenario authoring)

See the design doc for the full architecture and agent model. **Roadmap & plans:** [`ROADMAP.md`](ROADMAP.md) (the long-horizon open-ended arc; the Q3 2026 quarterly plan's status record lives in [`docs/superpowers/specs/2026-08-07-detailed-roadmap.md`](docs/superpowers/specs/2026-08-07-detailed-roadmap.md)) + [`docs/superpowers/plans/2026-08-01-roadmap-plans-index.md`](docs/superpowers/plans/2026-08-01-roadmap-plans-index.md) (per-item implementation plans). The milestone arcs: [`docs/superpowers/specs/2026-07-22-emergence-roadmap-design.md`](docs/superpowers/specs/2026-07-22-emergence-roadmap-design.md) (E1–E10, complete) and [`docs/superpowers/specs/2026-08-02-open-ended-complexity-arc-design.md`](docs/superpowers/specs/2026-08-02-open-ended-complexity-arc-design.md) (O1–O8, in progress).
