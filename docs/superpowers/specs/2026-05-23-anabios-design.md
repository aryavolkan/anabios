# anabios — design

**Status:** approved, pre-implementation
**Date:** 2026-05-23

## 1. Vision

anabios is a single-player discovery-driven evolutionary sandbox. The player is an observer / gardener: they seed a world (terrain, climate, starter species), then watch ecosystems emerge from local agent rules. The meta-game is a **codex** — a persistent journal of the first times each emergent phenomenon (population cycles, speciation, dialects, novel behaviors like flight or ambush) appears across all worlds the player runs.

Unlike the sibling projects in this workspace (`evolve`, `chess-evolve`, `neurogrid`, `tile-empire`), anabios is **not a neuroevolution project**. Agents have hand-engineered cognition substrates combined with three evolvable genetic layers. Emergence comes from the *interactions* of many such agents in a heterogeneous world, not from training neural networks.

### Foundational design choices

| Decision | Choice |
|---|---|
| Emergence foundation | Agent-based ecology |
| Player role | Observer / Gardener |
| Scale | ~2k–10k agents |
| World | Continuous 2D, bounded torus |
| Session shape | Discovery-driven, codex meta-game |
| Targeted phenomena | Population dynamics; spatial/territorial patterns; trait evolution & speciation; social/cultural emergence |
| Tech stack | Godot 4.5+ front-end + Rust core via gdext |
| Agent representation | 50-float genome + modular morphology + evolvable behavior program |
| Rendering | Procedural per-module sprite assembly via instanced MultiMeshes |

## 2. Repository layout

`anabios/` is a Cargo workspace at the root with a nested Godot project:

```
anabios/
├── Cargo.toml                       # workspace
├── crates/
│   ├── anabios-core/                # pure sim, no Godot, no I/O
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── world.rs             # World, BiomeField, time, RNG
│   │   │   ├── agent.rs             # SoA agent buffers
│   │   │   ├── genome.rs            # trait IDs, mutation, crossover, distance
│   │   │   ├── module.rs            # module library + structural mutation
│   │   │   ├── program.rs           # behavior program AST + evaluator + mutation
│   │   │   ├── behavior.rs          # sense/decide/act orchestration
│   │   │   ├── spatial.rs           # uniform-grid spatial hash
│   │   │   ├── reproduction.rs      # mating, birth, lineage
│   │   │   ├── species.rs           # online genetic clustering
│   │   │   ├── culture.rs           # meme vectors, transmission, drift
│   │   │   ├── codex.rs             # event detectors, CodexEvent bus
│   │   │   ├── snapshot.rs          # save/load deterministic state
│   │   │   └── tick.rs              # master step function
│   │   └── tests/                   # property tests, golden-tick replays
│   ├── anabios-godot/               # gdext crate
│   │   └── src/lib.rs               # Simulation node, buffer views, signals
│   └── anabios-headless/            # CLI binary
│       └── src/main.rs              # batch runs, JSONL events, W&B
├── game/                            # Godot 4.5+ project
│   ├── project.godot
│   ├── scenes/                      # main, viewer, codex, scenario editor
│   ├── ui/                          # GDScript UI
│   └── shaders/                     # multimesh agent shader, biome shader
└── docs/
    └── superpowers/specs/
```

### Boundary rules

- `anabios-core` has zero knowledge of Godot, files, threads, or wall-clock time. Pure functions over state buffers. Deterministic given seed + initial conditions.
- `anabios-godot` is the only place that touches gdext types. It owns a `Simulation` instance and exposes packed buffers (positions, colors, species ids, module instances) to GDScript per frame.
- `anabios-headless` calls `anabios-core` directly for batch experiments and writes JSONL/Parquet for analysis.
- GDScript in `game/` does UI, codex display, scenario authoring — never simulation logic.

## 3. Simulation model

### 3.1 Data layout

Struct-of-Arrays. Each agent property is a `Vec<T>` indexed by agent id. Agent ids are stable `u32` indices; deaths use a free-list.

| Field | Type | Notes |
|---|---|---|
| `position` | `Vec<Vec2>` | continuous, world is bounded torus |
| `velocity` | `Vec<Vec2>` | updated each tick from desired direction × speed |
| `energy` | `Vec<f32>` | depletes with action, replenished by feeding |
| `age` | `Vec<u32>` | ticks since birth |
| `genome` | `Vec<[f32; 50]>` | 50-trait float genome |
| `modules` | `Vec<SmallVec<[Module; 8]>>` | variable-length module list |
| `program` | `Vec<Program>` | behavior expression tree (≤64 nodes) |
| `species_id` | `Vec<u32>` | assigned by online clustering |
| `lineage_id` | `Vec<u64>` | unique per individual, never reused |
| `parent_ids` | `Vec<[u64; 2]>` | ancestry / kin recognition |
| `meme_vector` | `Vec<[f32; 8]>` | the culture carrier |
| `alive` | `BitVec` | dense liveness mask |

### 3.2 Layer 1 — 50-float genome

Categories (≈10 slots each):

- **Body modifiers:** size, color_hue/sat/val, lifespan_bias, basal_metabolism, mutation_rate, immune_strength
- **Drive levels:** aggression, fearfulness, curiosity, social_affinity, kin_preference, territoriality
- **Behavioral biases:** explore_vs_exploit, risk_tolerance, ambush_preference, communication_strength, altruism
- **Reproductive:** reproduction_threshold, offspring_investment, mate_choosiness, sexual_dimorphism
- **Sensory weighting:** per-channel weights applied to sensor inputs in the behavior program

All values in `[0, 1]`. Slot meanings are hardcoded; values mutate via Gaussian perturbation scaled by the self-evolving `mutation_rate` slot.

### 3.3 Layer 2 — Modular morphology

Each agent owns a `SmallVec<[Module; 8]>` (typically 3–12 modules). Modules are the agent's body plan and capability surface. Each carries a type, parameters, and an energy upkeep cost.

| Module | Effect | Parameters |
|---|---|---|
| `Locomotor` | enables motion | max_speed, terrain_affinity (land/water/both) |
| `Sensor` | provides one channel of perception | type (vision/smell/heat/sound), radius, acuity |
| `Mouth` | enables feeding | bite_size, diet_affinity (plant 0 ↔ meat 1) |
| `Weapon` | inflicts damage on contact | damage, energy_cost |
| `Spines` | inflicts damage at standoff range (3–8 units) | damage, energy_cost, range |
| `Jaws` | inflicts heavy damage point-blank | damage, energy_cost |
| `Armor` | reduces incoming damage | protection, mass_penalty |
| `Storage` | increases energy capacity | capacity |
| `Communicator` | emits/receives meme signals | range, channel_id |
| `Pheromone` | leaves chemical mark on biome | type, strength, decay |
| `Reproductive` | enables reproduction | viability, brood_size_bias |

Mutation operators on the module list:
- `Duplicate` — clone a module with parameter perturbation
- `Delete` — remove a module (some lethal, some neutral)
- `Mutate` — perturb parameters
- `Replace` — swap module type (rare, high impact)
- `Innovate` — once new module types are added to the library, allow them to appear via low-rate mutation

Energy upkeep makes module count self-regulating — useless modules are selected against.

### 3.4 Layer 3 — Evolvable behavior program

Each agent carries an **expression tree**, typically 5–40 nodes, hard-capped at 64 to prevent bloat.

**Node types:**

- **Inputs:** `SenseNearest(type)`, `SenseEnergy`, `SenseAge`, `SenseGenome(slot)`, `SenseBiome(channel)`, `SenseMeme(slot)`, `Const(f32)`
- **Operators:** `Add`, `Sub`, `Mul`, `Min`, `Max`, `Threshold(cmp, val)`, `IfThenElse`, `Lerp`, `Tanh`
- **Outputs:** `MoveToward(vec)`, `MoveAway(vec)`, `FireWeapon`, `Feed`, `Mate`, `EmitPheromone(channel)`, `Broadcast(meme_slot, val)`, `Idle`

Output nodes write to an action register. Multiple movement outputs vector-sum and clamp; mutually exclusive actions resolve by hardcoded priority.

Mutation operators on the program (genetic-programming style):
- `PointMutate` — change a constant or operator type
- `SubtreeMutate` — replace a random subtree with a freshly-generated random subtree
- `Insert` — wrap an existing node in a new parent
- `Delete` — collapse a node, promoting one child
- `Crossover` (sexual reproduction) — swap subtrees between parent programs

A library of **starter programs** (`Grazer`, `Browser`, `Generalist`, `Stalker`, `Pack Hunter`, `Filter Feeder`) seeds initial populations. Evolution proceeds from there.

### 3.5 How the three layers compose

```
sense()   → reads world via the agent's Sensor modules,
            writes per-channel values into the agent's sensor register
decide()  → evaluates the behavior program with those sensor values
            and genome accessible via SenseGenome nodes,
            yielding an action register
act()     → resolves the action register against the agent's modules:
              Feed only works if a Mouth exists
              FireWeapon only if a Weapon module is present
              MoveToward only if a Locomotor exists
            No matching module → action is a no-op.
```

Module presence **gates** which actions the program can take. An agent can evolve `FireWeapon` in its program, but without a Weapon module the action wastes energy at most. Evolution sorts these mismatches out over generations.

### 3.6 World state

- **Bounds:** 1024×1024 world units, wrapped (torus). Avoids edge artifacts.
- **Biome field:** 128×128 coarse grid sampled by position. Each cell: `terrain_type` (water/grass/forest/desert/rock), `temperature`, `nutrient_density`, `plant_biomass`. Plants regrow as a continuous density field, not as individual entities — scales much better.
- **Pheromone fields:** a separate set of 128×128 grids, one per pheromone `channel_id` (initially 4 channels). Pheromone modules write to the cell containing the agent's position each tick (during `interact()`); values decay each tick at a per-channel rate. Read by other agents via `SenseBiome(pheromone_channel)` in the behavior program — **gated by possession of a Sensor module of type `smell`** (consistent with the module-gates-action rule from §3.5). This is a **simulation-side** structure — distinct from any rendering texture that visualizes it.
- **Spatial hash:** uniform grid over agent positions, cell size ≈ max perception radius, rebuilt each tick. O(1) neighbor queries.

### 3.7 Tick pipeline

Ordered, deterministic, single-threaded outer loop; sub-stages multithreaded via `rayon` where safe:

```
1.  spatial_hash.rebuild()              // place agents in grid
2.  sense()                  [rayon]    // gather neighbors + biome + pheromone samples
3.  decide()                 [rayon]    // evaluate behavior programs
4.  integrate()                         // position += velocity; clamp/wrap; spend energy
5.  interact()                          // feeding, combat, mating, pheromone emission (deterministic)
6.  reproduce()                         // birth new agents; allocate ids from freelist
7.  culture_step()                      // meme transmission
8.  age_and_starve()                    // increment age; death checks
9.  pheromone_decay()                   // per-channel exponential decay on pheromone fields
10. species_step()           [every 200 ticks] // recluster by genetic distance
11. biome_step()             [every 10 ticks]  // regrow plants, diffuse nutrients
12. codex_detectors.observe()           // emit CodexEvents
```

Stages with shared-state mutation (interact, reproduce) stay serial; sensing/decision are embarrassingly parallel and use rayon batches over agent slices.

### 3.8 Reproduction & speciation

- **Sexual reproduction.** Two parents combine via uniform crossover on genome + Gaussian mutation (sigma scaled by `mutation_rate`). Module list child = random subset from each parent with mutation. Program tree = subtree crossover + mutation. Meme vector = parent average + jitter.
- **Speciation.** Every 200 ticks, incremental clustering on genomes (L2 distance, threshold ≈ 0.6) reassigns `species_id`. New cluster → new species id; parent-of-cluster recorded in phylogeny tree.

## 4. Emergence mechanisms & codex detectors

We don't program the phenomena; we arrange substrates that allow them. Each detector observes rolling state buffers and emits structured `CodexEvent`s.

### 4.1 Population dynamics

**Substrate:** plant biomass with logistic regrowth, carnivory via diet_affinity + Mouth modules, reproduction energy cost, death by starvation/age/combat.

**Detectors:** `PopulationCycleDetected`, `PopulationCrash`, `BoomAndBust`, `Extinction`, `CarryingCapacityReached`, `TrophicCascade`.

### 4.2 Spatial / territorial patterns

**Substrate:** heterogeneous biome creates natural barriers via terrain-affinity in Locomotor modules. Pheromone modules mark territory. Kin and herd cohesion traits cluster individuals.

**Detectors:** `Migration`, `TerritoryFormation`, `NichePartitioning`, `CorridorUse`, `SegregationEmerged`, `RangeExpansion`.

### 4.3 Trait evolution & speciation

**Substrate:** mutation across all three layers + sexual reproduction + geographic isolation + reclustering.

**Detectors:** `SpeciationEvent`, `ConvergentEvolution`, `TraitFixation`, `RapidAdaptation`, `NovelModuleAppeared`, `NovelBehavior`, `EvolvedFlight`, `EvolvedAmbush`, `EvolvedTool`, `EvolvedCooperation`.

The named-behavior detectors recognize specific signatures (e.g., agents staying still until prey enters perception and then triggering `FireWeapon` = ambush; agents emitting structured pheromone patterns that other agents respond to = signaling).

### 4.4 Social / cultural emergence

**Substrate:** Communicator modules transmit `meme_vector` to neighbors with imperfect copy (drift). Memes are accessible in the behavior program via `SenseMeme(slot)` — so memes can become adaptive, not just decorative. Geographic isolation lets meme populations diverge.

**Detectors:** `DialectFormed`, `MemeSweep`, `KinNetworkStable`, `Cooperation`, `WarOrRaid`, `TraditionPreserved`.

### 4.5 Codex event pipeline

```
sim tick ─► detectors observe rolling state buffers
         ─► detectors emit CodexEvent { type, tick, species_ids,
                                        location_bbox, snapshot_hash,
                                        description_template_params }
         ─► event bus (in anabios-core, no I/O)
         ─► Godot side polls each frame, adds entries to codex UI,
            triggers non-blocking screenshot, bookmarks tick for replay
         ─► headless mode writes events as JSONL for analysis / sweeps
```

When an event fires, `anabios-core` writes a compact world snapshot keyed by event id. Player can later "replay this moment" — sim rewinds to the snapshot and plays forward with a highlight overlay.

Detector functions are pure (`state buffers → optional events`). Players can register custom detectors as GDScript — each new named-emergence detector becomes a new codex chapter to fill.

## 5. Rendering

The simulation has no rendering knowledge; the Godot side reads agent buffers and packs draw commands.

### 5.1 Composition per agent

- **Body** — base disc/blob, colored from genome `color_hue` × species tint, sized from `size` trait, oriented toward velocity. Slight squash/stretch with motion.
- **Modules** — each module renders as a small sprite attached around the body at a fixed slot (8 evenly-spaced perimeter slots). Sprites are stylized but distinct:
  - `Locomotor` → fin/leg/wheel, ventral side; animated phase tied to speed
  - `Sensor` → eye/antenna, dorsal; size scales with `acuity`, color hints `type`
  - `Mouth` → mandible/beak, front; color shifts with `diet_affinity`
  - `Weapon` → spike/claw, length scales with `damage`
  - `Armor` → plate overlay on body, alpha scales with `protection`
  - `Storage` → ventral bulge, scales with `capacity`
  - `Communicator` → small horn/bell, top
  - `Pheromone` → trailing particle wisp on the agent itself; the render layer also samples the sim-side pheromone field (§3.6) into a screen-sized texture per channel and additively blends it under the agent layers
  - `Reproductive` → ornament/flower when energy near reproduction threshold
- **Outline** — thin colored ring shows species id (auto-assigned palette).
- **Effects** — fade-in on birth, fade-out + sink on death, combat flash, mating-pair glow.

### 5.2 GPU pipeline

- One `MultiMeshInstance2D` per visual layer:
  1. Body layer (one mesh, color + transform per instance)
  2. One layer per module sprite type (≈9 layers; sparse — only agents possessing that module type contribute an instance)
  3. Effects layer (births / deaths / combat sparks)
  4. Pheromone visualization layer (per-channel screen-sized texture sampled from the sim-side pheromone fields described in §3.6, updated each tick by a fragment shader)
- Each tick, Rust packs `PackedFloat32Array`s of `[x, y, rot, size, color_r, color_g, color_b, …]` per layer. GDScript memcopies them into the MultiMesh instance transform/color buffers.
- LOD: when an agent covers fewer than ~3 pixels (camera zoomed out), module layers cull and only the body disc renders. Simulation runs identically regardless of camera.

### 5.3 Animation

- Body orientation low-pass filtered toward velocity direction.
- Locomotor sprites animate via a per-agent phase (`agent.phase += speed * dt`) sampled into a 4-frame sprite atlas — one shader uniform.
- Module slot positions wobble subtly (sin of phase) so creatures don't look rigid.

### 5.4 Cost ballpark

- 10k agents × ~4 modules avg → ~50k MultiMesh instances. Godot 4 handles this comfortably.
- Per-frame upload: ~50k × 32 bytes ≈ 1.6 MB packed buffers. Negligible.

## 6. Player-facing systems

### 6.1 Main shell

Three top-level screens: **Worlds**, **Viewer**, **Codex**. Persistent dock holds time controls, the live event ticker, and overlay toggles.

### 6.2 World setup

- **Terrain** — seed; sliders for water/mountain/forest/desert %; presets (Pangaea, Archipelago, Mountainous Continent, Hostile Desert).
- **Climate** — global temperature, seasonality strength, disaster frequency (fires, droughts, freezes).
- **Seed populations** — drop 1–N starter species onto the map. Each starter is a prebuilt template editable for genome floats, module list, starting program. "Random viable" button rolls a starter and places it.
- **Advanced rules tweaker** — mutation rates, structural-mutation probability per layer, max program nodes, species clustering threshold, codex sensitivity.

### 6.3 Viewer

- **World canvas** with pan/zoom (mouse, keyboard, pinch). LOD per §5.2.
- **Camera modes:** Free, Follow Individual, Follow Species, Event Camera (auto-cuts to recent events for ~15 s each — screensaver-friendly).
- **Overlay toggles:** biome heatmap, pheromone trails, species territories, density, lineage paint, trait paint.
- **Inspector** — click an agent to pin it. Shows species, lineage tree, age, energy, module list (visual), behavior program (expandable node graph), genome radar vs species mean, recent decisions, nearby kin. Buttons: Follow, Show ancestry.
- **Event ticker** — bottom of screen. Codex events flash with type icon; click to jump-cam and pause.
- **Stats panel** — live charts: per-species population, total alive, average genome drift over time, biome stress, codex events / minute. Shared timeline.

### 6.4 Time controls

- Pause, 0.5×, 1×, 4×, 16×, 64×, 256× speeds; max speed runs as fast as CPU allows and renders every N-th tick.
- **Timeline scrubber** along the bottom with event-marker icons. Click any marker to jump there via snapshot rewind + deterministic re-simulation.
- **Run until next event** — runs at max speed; auto-pauses when a codex-worthy event fires.

### 6.5 Codex (meta-game)

Cross-world journal, persisted across all player worlds.

- **Chapters** by family: Population, Spatial, Evolution, Culture, Named Behaviors, Lineage Hall.
- **Entries** carry: one-line headline, real-time + sim-tick timestamp, framed screenshot, replay button, linked species page (phylogeny, last sighted, peak population), tags (world id, biome at event).
- **Progress** — each chapter shows known events with hidden entries marked `???` until found.
- **Sharing** — export an entry as a card image.

### 6.6 Scenario authoring (phase-2)

Saved initial conditions + tweaker settings as a `.anascen` file. Loading runs that exact seed deterministically. Reserved `format_version` field for forward compatibility.

### 6.7 Accessibility & polish

Colorblind-safe palettes; pause-on-focus-loss; "Calm mode" overlay theme; 60-second tutorial on first launch.

## 7. Testing & determinism

### 7.1 Testing layers (`anabios-core`)

- **Unit tests** — spatial hash, genome distance, mutation distributions, biome diffusion stability, snapshot round-trip, program eval against handcrafted inputs.
- **Property tests** (`proptest`) — invariants for any seed:
  - Total energy never increases except by plant regrowth
  - Agent ids never reused while alive
  - Phylogeny is a DAG
  - Genome distance is a metric (positivity, symmetry, sampled triangle inequality)
  - All alive agents have positions inside world bounds after each tick
- **Golden-tick replay tests** — fixed seed + scenario; snapshot hashes at ticks 0, 100, 1000, 10000 committed. Hash mismatch fails CI.
- **Detector tests** — handcrafted minimal worlds that should fire each codex detector exactly once.
- **Stress tests** — `cargo bench` on 10k-agent ticks; >10% perf regression fails CI.

`anabios-godot` has a minimal smoke test (boot sim, run 100 ticks, assert buffer view contracts). `game/` has GDScript unit tests via the project's standard `test_runner.gd` plus one end-to-end viewer smoke test.

### 7.2 Determinism rules

- Single `Xoshiro256++` RNG owned by `World`. Every stochastic step pulls in a fixed, documented order.
- No floating-point reductions over unordered collections. Order agents by id when summing, averaging, clustering.
- No `HashMap` iteration in the tick path. Use `BTreeMap` or pre-sorted `Vec`.
- No wall-clock reads inside the sim.
- Behavior-program evaluator is `f32` deterministic. Use `mul_add` consistently; wrap `sin`/`cos`/`exp` in project-internal helpers for cross-build consistency.

**Verification:** golden-tick tests + a `headless-determinism` CI job that runs the same scenario twice and asserts byte-identical snapshots.

**Cross-platform:** target identical behavior on macOS and Linux x86_64/arm64. Windows is best-effort.

## 8. Performance budgets

**Target on a 2024-era M-series MacBook:**
- 5k agents @ 60 sim ticks/s, 60 render fps — comfortable
- 10k agents @ 30 sim ticks/s, 60 render fps — comfortable
- 10k agents @ 60 sim ticks/s — stretch goal

**Per-tick budget at 10k agents @ 30 Hz (~33 ms):**

| Stage | Budget |
|---|---|
| Spatial hash rebuild | 1 ms |
| Sense (rayon) | 6 ms |
| Decide (rayon) — program eval is the hotspot | 10 ms |
| Integrate + interact + reproduce | 6 ms |
| Culture step | 2 ms |
| Species reclustering (amortized) | 1 ms/tick |
| Biome step (amortized) | 2 ms/tick |
| Codex detectors | 3 ms |

**Hot-path discipline:**
- No allocations in the tick. All scratch buffers owned by `World` and reused.
- Behavior-program eval uses an explicit stack (`Vec<f32>` of fixed capacity), not recursion.
- Spatial hash uses dense `Vec<u32>` buckets.
- Module lists are `SmallVec<[Module; 8]>` to avoid heap traffic for typical agents.

Profiling via the `tracing` crate with `tracing-chrome` exporter on demand. A `--profile` flag on `anabios-headless` runs a fixed scenario and dumps a flamegraph.

## 9. Build pipeline & CI

### 9.1 Local workflow

- `just sim` — build and run headless smoke test
- `just game` — open Godot editor pointing at `game/`
- `just test` — run all Rust + GDScript tests
- `just bench` — run the perf bench suite

`justfile` mirrors the style of `evolve` and `tile-empire`.

### 9.2 CI (GitHub Actions)

1. **Rust** — fmt + clippy + test + bench-comparison (>10% regression warns)
2. **Godot** — install Godot 4.5 headless, run GDScript tests via `test_runner.gd`
3. **gdext build** — cross-compile `anabios-godot` for macOS arm64, macOS x86_64, Linux x86_64 (Windows later); upload dylibs as artifacts
4. **Determinism** — run the headless binary twice with the same seed; byte-compare output
5. **Golden-tick** — run tick-hash test; warn on changes (manual approval to update)

### 9.3 Release packaging

- Per-platform Godot exports with gdext binaries bundled. macOS notarization for distribution.
- Headless binary published as a separate artifact for sweep/cluster use.

## 10. Headless / W&B integration

`anabios-headless` reuses `shared-evolve-utils` for W&B logging:

- `anabios-headless run --scenario worlds/pangaea.toml --ticks 100000 --seed 42 --out runs/foo/`
  - Streams codex events to `runs/foo/events.jsonl`
  - Periodic snapshots to `runs/foo/snapshots/`
  - Optional `--wandb-project anabios` streams summary stats live
- `anabios-headless sweep --config sweeps/diversity.toml` launches a W&B sweep across scenario parameters and reports per-run codex coverage as the metric to optimize.

This enables "run 1000 worlds overnight, surface the rare events" — a natural workflow for a discovery game.

## 11. Crash safety & saves

- Sim state serialized via `bincode` + versioned `format_version` field. Snapshots ≈ 5 MB for a 10k-agent world.
- Auto-save every 60 sim-seconds of world time. Crash recovery on next launch.
- Codex persisted to a SQLite DB at the platform's app-support path (`~/Library/Application Support/anabios/codex.db` on macOS), WAL mode for safe concurrent reads while the sim writes.

## 12. Roadmap (high-level milestones)

The implementation plan lives in a separate document, but the milestone shape is:

1. **M1 — Headless core, agents move and eat plants.** `anabios-core` skeleton; 50-float genome only (no modules or program yet — they default to constants); spatial hash; tick pipeline; determinism + property tests passing.
2. **M2 — Reproduction, mutation, speciation.** Sexual reproduction; mutation on genome only; species clustering; phylogeny tracking.
3. **M3 — Modular morphology layer.** Module struct, library, mutation operators, action gating. Energy upkeep balancing.
4. **M4 — Behavior program layer.** AST, evaluator, mutation, starter program library.
5. **M5 — Codex core.** Detector framework + first batch of detectors (population, spatial, evolution).
6. **M6 — Godot viewer MVP.** gdext wiring, MultiMesh body rendering, time controls, inspector.
7. **M7 — Full rendering.** Module sprites, animations, biome shader, pheromone trail buffer.
8. **M8 — Codex UI + named-behavior detectors + replay.**
9. **M9 — World setup + scenario authoring + accessibility polish.**
10. **M10 — Headless sweep tooling, W&B integration, performance hardening.**

Each milestone gets its own plan and is shippable as a playable demo (even if minimal).
