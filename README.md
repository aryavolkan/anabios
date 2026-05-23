# anabios

> Greek *ἀναβίωσις* — life arising.

A discovery-driven evolutionary sandbox where complex ecosystems emerge from simple agent rules. You seed worlds with terrain and starter species, then watch — and catalogue what unfolds.

Not a neuroevolution project. Agents have **simple, hand-engineered cognition** (a tiny evolvable behavior program) combined with a **float genome** and a **modular body plan**. Speciation, migration, predator/prey cycles, dialects, and named behaviors (flight, ambush, cooperation) emerge from local interactions; the **codex** records the first time each phenomenon appears in your worlds.

## Status

Pre-implementation. Design is at [`docs/superpowers/specs/2026-05-23-anabios-design.md`](docs/superpowers/specs/2026-05-23-anabios-design.md).

## Stack

- **`anabios-core`** — pure Rust simulation crate (no Godot, no I/O, deterministic)
- **`anabios-godot`** — gdext wrapper for use from the Godot project
- **`anabios-headless`** — CLI for batch runs, W&B sweeps, codex mining
- **`game/`** — Godot 4.5+ project (viewer, codex UI, world setup, scenario authoring)

See the design doc for the full architecture, agent model, and roadmap.
