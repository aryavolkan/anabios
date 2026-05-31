# anabios

> Greek *ἀναβίωσις* — life arising.

A discovery-driven evolutionary sandbox where complex ecosystems emerge from simple agent rules. You seed worlds with terrain and starter species, then watch — and catalogue what unfolds.

Not a neuroevolution project. Agents have **simple, hand-engineered cognition** (a tiny evolvable behavior program) combined with a **float genome** and a **modular body plan**. Speciation, migration, predator/prey cycles, dialects, and named behaviors (flight, ambush, cooperation) emerge from local interactions; the **codex** records the first time each phenomenon appears in your worlds.

## Status

Design at [`docs/superpowers/specs/2026-05-23-anabios-design.md`](docs/superpowers/specs/2026-05-23-anabios-design.md). M1–M6 shipped (see git tags `m1`–`m6`); M7+ adds module sprite layers, overlays, camera modes, and full codex UI.

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

The summary CSV has columns `seed, ticks, final_alive, final_biomass, state_hash, extinction, pop_crash, speciation, migration, novel_module, novel_behavior` — pipe it into a spreadsheet or a notebook to mine for rare events. The per-seed `seed_NNNNNNNN.events.jsonl` files contain the full event stream for each run.

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
- **`game/`** — Godot 4.5+ project (viewer, codex UI, world setup, scenario authoring)

See the design doc for the full architecture, agent model, and roadmap.
