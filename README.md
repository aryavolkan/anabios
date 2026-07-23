# anabios

> Greek *ἀναβίωσις* — life arising.

A discovery-driven evolutionary sandbox where complex ecosystems emerge from simple agent rules. You seed worlds with terrain and starter species, then watch — and catalogue what unfolds.

Not a neuroevolution project. Agents have **simple, hand-engineered cognition** (a tiny evolvable behavior program) combined with a **float genome** and a **modular body plan**. Speciation, migration, predator/prey cycles, dialects, and named behaviors (flight, ambush, cooperation) emerge from local interactions; the **codex** records the first time each phenomenon appears in your worlds.

## Status

Design at [`docs/superpowers/specs/2026-05-23-anabios-design.md`](docs/superpowers/specs/2026-05-23-anabios-design.md). Shipped to date (git tags `m1`–`m10` plus later batches):

- **Core sim** — deterministic SoA agent simulation: uniform-grid spatial hashing, evolvable postfix behavior programs, 50-slot float genome, modular morphology, speciation
- **Interaction substrate (M11–M15)** — combat & predation, carcass scavenging, pheromone fields, communication/meme culture, kin-directed cooperation
- **Invention tree** — 10-tech cumulative culture tree (Stone Tools → Fire → Farming/Metalworking → Writing/Medicine/Husbandry → Machinery/Electricity/Nuclear Power) riding the meme channels: individual discovery (Openness + skill gated), social spread, per-holder buffs *and* debuffs (metabolism, upkeep, crowding stress, biome pollution, radiation mutation); `InventionDiscovered`/`InventionAdopted` codex events. Opt-in per scenario (`inventions_enabled`)
- **Codex** — 19 emergence detectors (extinction → herd cohesion → invention adoption) writing a persistent event timeline
- **Experiments** — DIT gene-culture technique model; biome climate adaptation (opt-in per scenario); runtime world dimensions + living/seasonal biomes
- **Viewer** — Godot 4.6+ client: biome/species/pheromone overlays, inspector, codex panel, co-evolution charts, per-species tech panel
- **Tooling** — headless sweep CLI (parallel seeds → JSONL + CSV), criterion benchmark suite

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

The summary CSV has columns `seed, ticks, final_alive, final_biomass, state_hash, extinction, pop_crash, speciation, migration, novel_module, novel_behavior, predation, combat_raid, arms_race, territory_formation, niche_partitioning, dialect_formed, meme_sweep, alarm_call, evolved_cooperation, pack_hunting, herd_cohesion, invention_discovered, invention_adopted, practice_discovered, practice_adopted, resource_traded, dowry_birth, emergence_score, novel_events, coverage` — pipe it into a spreadsheet or a notebook to mine for rare events. The per-seed `seed_NNNNNNNN.events.jsonl` files contain the full event stream for each run.

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

See the design doc for the full architecture, agent model, and roadmap. The forward roadmap is the emergence arc: [`docs/superpowers/specs/2026-07-22-emergence-roadmap-design.md`](docs/superpowers/specs/2026-07-22-emergence-roadmap-design.md) (milestones E1–E10).
