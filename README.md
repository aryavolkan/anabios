# anabios

> Greek *ἀναβίωσις* — life arising.

A discovery-driven evolutionary sandbox where complex ecosystems emerge from simple agent rules. You seed worlds with terrain and starter species, then watch — and catalogue what unfolds.

Not a neuroevolution project. Agents have **simple, hand-engineered cognition** (a tiny evolvable behavior program) combined with a **float genome** and a **modular body plan**. Speciation, migration, predator/prey cycles, dialects, and named behaviors (flight, ambush, cooperation) emerge from local interactions; the **codex** records the first time each phenomenon appears in your worlds.

## Status

Design at [`docs/superpowers/specs/2026-05-23-anabios-design.md`](docs/superpowers/specs/2026-05-23-anabios-design.md). Shipped to date (git tags `m1`–`m10` plus later batches):

- **Core sim** — deterministic SoA agent simulation: uniform-grid spatial hashing, evolvable postfix behavior programs, 50-slot float genome, modular morphology, speciation
- **Interaction substrate (M11–M15)** — combat & predation, carcass scavenging, pheromone fields, communication/meme culture, kin-directed cooperation
- **Codex** — 17 emergence detectors (extinction → herd cohesion) writing a persistent event timeline
- **Experiments** — DIT gene-culture technique model; biome climate adaptation (opt-in per scenario)
- **Viewer** — Godot 4.6+ client: biome/species/pheromone overlays, inspector, codex panel, co-evolution charts
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

The summary CSV has columns `seed, ticks, final_alive, final_biomass, state_hash, extinction, pop_crash, speciation, migration, novel_module, novel_behavior, predation, combat_raid, arms_race, territory_formation, niche_partitioning, dialect_formed, meme_sweep, alarm_call, evolved_cooperation, pack_hunting, herd_cohesion` — pipe it into a spreadsheet or a notebook to mine for rare events. The per-seed `seed_NNNNNNNN.events.jsonl` files contain the full event stream for each run.

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

## Stack

- **`anabios-core`** — pure Rust simulation crate (no Godot, no I/O, deterministic)
- **`anabios-godot`** — gdext wrapper for use from the Godot project
- **`anabios-headless`** — CLI for batch runs, W&B sweeps, codex mining
- **`game/`** — Godot 4.6+ project (viewer, codex UI, world setup, scenario authoring)

See the design doc for the full architecture, agent model, and roadmap.
