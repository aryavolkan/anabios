# M13 — Pheromone Fields & Territorial Competition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give pheromones real substrate — per-channel decaying grids agents deposit into and smell — and add the `TerritoryFormation` / `NichePartitioning` detectors, producing the first spatial/territorial emergence in anabios.

**Architecture:** A new `World.pheromones: PheromoneField` holds 4 channels of a 128×128 grid (same resolution/indexing as the biome). During `interact()`, an agent with a `Pheromone` module deposits its `emit_intent[ch]` into the field cell at its position; a new `pheromone_decay_step` tick stage multiplies every cell by `(1 − PHEROMONE_DECAY)` each tick. During `sense()`, an agent with a `Smell` `Sensor` module reads the local cell's per-channel concentration into `SensorRegister.pheromone`, exposed to programs via a new `SensePheromone(channel)` node. Two centroid/terrain-based detectors (mirroring `detect_migration`) recognize the resulting spatial patterns. `EmitPheromone`, `PheromoneChannel`, `Module::Pheromone`, and `SensorType::Smell` already exist — M13 wires them.

**Tech Stack:** Rust (`anabios-core` pure-sim crate, `anabios-headless` CLI), `glam::Vec2`, `serde`/`bincode` snapshots, `BTreeMap`/`VecDeque` detector state.

## Global Constraints

- **Determinism (design §7.2):** tick-path/detector iteration is id-ordered or over `BTreeMap`/`BTreeSet`/`VecDeque`; **no `HashMap`** in tick/detector paths; no unordered float reductions; pheromone deposition iterates alive ids ascending; decay is a pure per-cell map.
- **`EventType` variants appended at the END, in order:** `TerritoryFormation = 9`, `NichePartitioning = 10` (current tail is `ArmsRace = 8`). bincode encodes by positional index — never insert mid-enum. (`codex.rs`.)
- **`Node` variants appended at the END:** `SensePheromone(u8)` after `SenseCrowding` (node_kind 40). Like M11/M12, the new sense node is **excluded from the `random_node` mutation grammar** (seeded via starters only) so evolved programs are unchanged — keeps the golden-tick refresh limited to the one layout change below.
- **Snapshot / golden-tick:** `World.pheromones` (serialized) and new `CodexState` fields change the snapshot layout. Per spec §2.3 the committed golden-tick hashes in `tests/determinism.rs` are **refreshed** by the controller (subagents cannot run cargo). `minimal.toml` agents have no `Pheromone` module (no deposition) and no `Smell`-gated behavior change, so the field stays all-zero there — the refresh is a pure layout bump.
- **Grid resolution:** pheromone grid is `BIOME_RES` (128) × `BIOME_RES`, indexed by `biome::cell_coords(pos)` → `biome::cell_index(col,row)` (row-major `row*128+col`). Reuse those helpers; do not re-derive indexing.
- **Channels:** `PHEROMONE_CHANNELS = 4` (`program.rs`), matching `PheromoneChannel { Alarm=0, Mate=1, Trail=2, Marker=3 }` (`module.rs`).

---

## File Structure

- `crates/anabios-core/src/pheromone.rs` — **new**: `PheromoneField` (grid + deposit/sample/decay), constants.
- `crates/anabios-core/src/lib.rs` — `pub mod pheromone;`.
- `crates/anabios-core/src/world.rs` — add serialized `pheromones` field; init.
- `crates/anabios-core/src/module.rs` — `effective_pheromone_strength`, `has_smell` helpers.
- `crates/anabios-core/src/interact.rs` — deposition pass.
- `crates/anabios-core/src/tick.rs` — `pheromone_decay_step` stage.
- `crates/anabios-core/src/sense.rs` — `SensorRegister.pheromone`; sample field gated by `Smell`; `sense_all` signature gains `&PheromoneField`.
- `crates/anabios-core/src/program.rs` — `SensePheromone(u8)` node (arity/node_kind/evaluate); `EvalContext.pheromone_sample`; `starter_marker` program.
- `crates/anabios-core/src/behavior.rs` — thread `sensor.pheromone` into `EvalContext`.
- `crates/anabios-core/src/codex.rs` — `EventType` variants; `CodexState` fields; `detect_territory_formation`, `detect_niche_partitioning` + pure helpers; wire into `observe_all`.
- `crates/anabios-core/src/scenario.rs` — `marker` archetype in `archetype_kit`.
- `crates/anabios-headless/src/sweep.rs` — 2 new event names + CSV columns.
- `crates/anabios-core/tests/pheromone_territory.rs` — **new**: mechanism tests.
- `crates/anabios-core/tests/territory_emergence.rs` — **new**: multi-seed emergence test.
- `scenarios/territories.toml` — **new**.

---

## Task 1: Pheromone field substrate

**Files:**
- Create: `crates/anabios-core/src/pheromone.rs`
- Modify: `crates/anabios-core/src/lib.rs` (`pub mod pheromone;`)
- Modify: `crates/anabios-core/src/world.rs` (`pheromones` field + init)
- Modify: `crates/anabios-core/src/module.rs` (`effective_pheromone_strength`, `has_smell`)
- Test: `crates/anabios-core/tests/pheromone_territory.rs` (new)

**Interfaces:**
- Produces: `pheromone::PheromoneField { cells: Vec<[f32; PHEROMONE_CHANNELS]> }` (len `BIOME_RES*BIOME_RES`), derives `Debug, Clone, Serialize, Deserialize`.
  - `PheromoneField::new() -> Self` (all-zero).
  - `deposit(&mut self, pos: Vec2, channel: usize, amount: f32)` — adds `amount` to the cell at `pos` on `channel` (clamped index).
  - `sample(&self, pos: Vec2, channel: usize) -> f32` — reads the cell at `pos` on `channel`.
  - `decay_step(&mut self)` — multiplies every cell/channel by `(1.0 - PHEROMONE_DECAY)`.
- Produces: `pheromone::PHEROMONE_DECAY: f32 = 0.05`, `pheromone::PHEROMONE_EMIT_THRESHOLD: f32 = 0.5`, `pheromone::PHEROMONE_DEPOSIT_SCALE: f32 = 1.0`.
- Produces: `World.pheromones: PheromoneField` (serialized).
- Produces: `module::effective_pheromone_strength(&ModuleList) -> f32` (max `strength` over `Pheromone` modules, `0.0` if none); `module::has_smell(&ModuleList) -> bool` (has a `Sensor` with `sensor_type == SensorType::Smell`).
- Consumes: `biome::{cell_coords, cell_index, BIOME_RES}`, `PHEROMONE_CHANNELS` (`program.rs`).

- [ ] **Step 1: Write the failing test** — create `crates/anabios-core/tests/pheromone_territory.rs`:

```rust
//! M13 mechanism tests: pheromone deposition, decay, sensing, and detectors.

use anabios_core::genome::Genome;
use anabios_core::module::{Module, ModuleType, PheromoneChannel, SensorType};
use anabios_core::pheromone::{PheromoneField, PHEROMONE_DECAY};
use anabios_core::prelude_test::Vec2;
use anabios_core::program::{Node, Program, PHEROMONE_CHANNELS};
use anabios_core::tick::step;
use anabios_core::world::World;

#[test]
fn deposit_then_sample_reads_back_on_the_right_channel() {
    let mut f = PheromoneField::new();
    let p = Vec2::new(400.0, 400.0);
    f.deposit(p, 3, 2.0);
    assert!((f.sample(p, 3) - 2.0).abs() < 1e-6, "channel 3 holds the deposit");
    assert_eq!(f.sample(p, 0), 0.0, "other channels untouched");
    // A far-away cell is unaffected.
    assert_eq!(f.sample(Vec2::new(10.0, 10.0), 3), 0.0);
}

#[test]
fn decay_step_multiplies_every_cell_by_one_minus_decay() {
    let mut f = PheromoneField::new();
    let p = Vec2::new(200.0, 200.0);
    f.deposit(p, 1, 10.0);
    f.decay_step();
    let expected = 10.0 * (1.0 - PHEROMONE_DECAY);
    assert!((f.sample(p, 1) - expected).abs() < 1e-4, "one decay step");
}

#[test]
fn world_starts_with_an_empty_pheromone_field() {
    let w = World::new(1);
    assert_eq!(w.pheromones.sample(Vec2::new(500.0, 500.0), 0), 0.0);
    let _ = (Module::Pheromone {
        channel: PheromoneChannel::Marker,
        strength: 1.0,
        decay: 0.1,
    }, ModuleType::Pheromone, SensorType::Smell, PHEROMONE_CHANNELS, Genome::neutral(), Program::from_slice(&[Node::Idle]));
    let _ = step; // ensure imports are exercised by later tests
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --test pheromone_territory`
Expected: FAIL to compile (`no module pheromone`, `no field pheromones`).

- [ ] **Step 3: Create the pheromone module** — `crates/anabios-core/src/pheromone.rs`:

```rust
//! Per-channel pheromone fields: 128×128 grids (one value per channel per cell)
//! that agents with a `Pheromone` module deposit into and `Smell`-sensored
//! agents read. Fields decay exponentially each tick (design §3.6, §3.7 step 9).

use serde::{Deserialize, Serialize};

use crate::biome::{cell_coords, cell_index, BIOME_RES};
use crate::prelude::Vec2;
use crate::program::PHEROMONE_CHANNELS;

/// Fraction of every cell's pheromone that dissipates each tick.
pub const PHEROMONE_DECAY: f32 = 0.05;
/// `emit_intent[ch]` above this triggers a deposit that tick.
pub const PHEROMONE_EMIT_THRESHOLD: f32 = 0.5;
/// Scales `emit_intent * strength` into deposited concentration.
pub const PHEROMONE_DEPOSIT_SCALE: f32 = 1.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PheromoneField {
    /// Row-major `BIOME_RES*BIOME_RES` cells; each holds one value per channel.
    pub cells: Vec<[f32; PHEROMONE_CHANNELS]>,
}

impl Default for PheromoneField {
    fn default() -> Self {
        Self::new()
    }
}

impl PheromoneField {
    pub fn new() -> Self {
        Self { cells: vec![[0.0; PHEROMONE_CHANNELS]; BIOME_RES * BIOME_RES] }
    }

    #[inline]
    fn idx(pos: Vec2) -> usize {
        let (col, row) = cell_coords(pos);
        cell_index(col, row)
    }

    /// Add `amount` to the cell at `pos` on `channel` (index clamped).
    pub fn deposit(&mut self, pos: Vec2, channel: usize, amount: f32) {
        let ch = channel.min(PHEROMONE_CHANNELS - 1);
        self.cells[Self::idx(pos)][ch] += amount;
    }

    /// Read the concentration at `pos` on `channel` (index clamped).
    pub fn sample(&self, pos: Vec2, channel: usize) -> f32 {
        let ch = channel.min(PHEROMONE_CHANNELS - 1);
        self.cells[Self::idx(pos)][ch]
    }

    /// Exponential per-tick decay across every cell and channel.
    pub fn decay_step(&mut self) {
        let keep = 1.0 - PHEROMONE_DECAY;
        for cell in self.cells.iter_mut() {
            for v in cell.iter_mut() {
                *v *= keep;
            }
        }
    }
}
```

Add `pub mod pheromone;` to `crates/anabios-core/src/lib.rs`. Confirm `biome::cell_coords`/`cell_index`/`BIOME_RES` are `pub` (map: they are). Confirm `crate::prelude::Vec2` path is used within-crate.

- [ ] **Step 4: Add the `pheromones` field to World** — in `crates/anabios-core/src/world.rs`, near the serialized `carcasses` field:

```rust
    /// Per-channel pheromone grids (deposited in `interact`, decayed each tick).
    pub pheromones: crate::pheromone::PheromoneField,
```

Init in `World::new`: `pheromones: crate::pheromone::PheromoneField::new(),`.

- [ ] **Step 5: Add the module helpers** — in `crates/anabios-core/src/module.rs`, near the other `effective_*` helpers:

```rust
/// Max `Pheromone.strength`, or `0.0` if the agent has no `Pheromone` module.
#[inline]
pub fn effective_pheromone_strength(modules: &ModuleList) -> f32 {
    modules
        .iter()
        .filter_map(|m| match m {
            Module::Pheromone { strength, .. } => Some(*strength),
            _ => None,
        })
        .fold(0.0_f32, f32::max)
}

/// `true` iff the agent has a `Sensor` module of type `Smell` (gates pheromone
/// perception, design §3.6).
#[inline]
pub fn has_smell(modules: &ModuleList) -> bool {
    modules.iter().any(|m| matches!(m, Module::Sensor { sensor_type: SensorType::Smell, .. }))
}
```

Confirm `SensorType` is in scope in `module.rs` (it's defined there).

- [ ] **Step 6: Simplify the test to only what Task 1 provides** — replace the throwaway `world_starts_with_an_empty_pheromone_field` body's last two lines (the import-exercising junk) with just the assertion. Final version:

```rust
#[test]
fn world_starts_with_an_empty_pheromone_field() {
    let w = World::new(1);
    assert_eq!(w.pheromones.sample(Vec2::new(500.0, 500.0), 0), 0.0);
}
```

Remove now-unused imports from the test file top (keep only what the three tests use: `PheromoneField`, `PHEROMONE_DECAY`, `Vec2`, `World`). Later tasks re-add imports as needed.

- [ ] **Step 7: Run to verify pass**

Run: `cargo test -p anabios-core --test pheromone_territory`
Expected: PASS (3 tests).
Note (controller): `World.pheromones` changes the serialized snapshot → refresh the golden hashes. Run `cargo test -p anabios-core --test determinism`, then `UPDATE_HASHES=1 cargo test -p anabios-core --test determinism -- --nocapture`, copy the 3 printed values into `GOLDEN` at `tests/determinism.rs`, and confirm PASS + stable on a second run. (`minimal.toml` has no `Pheromone` module, so the field stays all-zero; this is a pure layout bump that stays stable for the rest of the milestone.)

- [ ] **Step 8: Commit**

```bash
git add crates/anabios-core/src/pheromone.rs crates/anabios-core/src/lib.rs \
        crates/anabios-core/src/world.rs crates/anabios-core/src/module.rs \
        crates/anabios-core/tests/pheromone_territory.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M13 pheromone field substrate — grids + deposit/sample/decay

Refresh golden-tick hashes for the new World.pheromones snapshot field (§2.3).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Deposition in interact + decay stage

**Files:**
- Modify: `crates/anabios-core/src/interact.rs` (deposition pass)
- Modify: `crates/anabios-core/src/tick.rs` (`pheromone_decay_step` stage)
- Test: `crates/anabios-core/tests/pheromone_territory.rs` (append)

**Interfaces:**
- Consumes: `World.actions[i].emit_intent`, `module::{has, effective_pheromone_strength}`, `ModuleType::Pheromone`, `World.pheromones.deposit`.
- Produces: a deposition pass inside `interact_all` (after combat/scavenge, id-ordered).

- [ ] **Step 1: Write the failing test** — append to `crates/anabios-core/tests/pheromone_territory.rs`:

```rust
use anabios_core::module::{Module, PheromoneChannel, SensorType};

/// Build a pheromone-marking kit: Locomotor + Vision + Mouth + Pheromone(Marker).
fn marker_kit() -> anabios_core::module::ModuleList {
    let mut m = anabios_core::module::ModuleList::new();
    m.push(Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 });
    m.push(Module::Sensor { sensor_type: SensorType::Vision, radius: 0.6, acuity: 0.6 });
    m.push(Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 });
    m.push(Module::Pheromone { channel: PheromoneChannel::Marker, strength: 1.0, decay: 0.1 });
    m
}

#[test]
fn agent_with_pheromone_module_deposits_on_emit() {
    let mut w = World::new(2);
    let id = w.spawn_agent(Vec2::new(600.0, 600.0), Genome::neutral());
    w.agents.modules[id as usize] = marker_kit();
    // Emit strongly on channel 3 (Marker).
    w.agents.program[id as usize] =
        Program::from_slice(&[Node::Const(5.0), Node::EmitPheromone(3)]);
    let pos = w.agents.position[id as usize];
    step(&mut w);
    assert!(w.pheromones.sample(pos, 3) > 0.0, "marker deposited at the agent's cell");
}

#[test]
fn agent_without_pheromone_module_deposits_nothing() {
    let mut w = World::new(2);
    let id = w.spawn_agent(Vec2::new(600.0, 600.0), Genome::neutral());
    // Default starter_kit has NO Pheromone module.
    w.agents.program[id as usize] =
        Program::from_slice(&[Node::Const(5.0), Node::EmitPheromone(3)]);
    let pos = w.agents.position[id as usize];
    step(&mut w);
    assert_eq!(w.pheromones.sample(pos, 3), 0.0, "no Pheromone module → no deposit (gating)");
}
```

Add `use anabios_core::genome::Genome;` / `Node`/`Program`/`step`/`World` imports at the top if not already present.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --test pheromone_territory`
Expected: FAIL (`agent_with_pheromone_module_deposits` — nothing is deposited yet).

- [ ] **Step 3: Add the deposition pass** — in `crates/anabios-core/src/interact.rs`, inside `interact_all`, add a `deposit_pass` after the scavenge pass and define it. Add to the pass sequence:

```rust
    feed_pass(world, &alive_ids);
    combat_pass(world, &alive_ids);
    scavenge_pass(world, &alive_ids);
    deposit_pass(world, &alive_ids);
```

And the function:

```rust
/// Pheromone deposition: an agent with a `Pheromone` module writes each of its
/// above-threshold `emit_intent` channels into the field cell at its position,
/// scaled by the module's strength. Gated on the `Pheromone` module.
fn deposit_pass(world: &mut World, alive_ids: &[u32]) {
    use crate::pheromone::{PHEROMONE_DEPOSIT_SCALE, PHEROMONE_EMIT_THRESHOLD};
    for &id in alive_ids {
        let i = id as usize;
        if !module::has(&world.agents.modules[i], ModuleType::Pheromone) {
            continue;
        }
        let strength = module::effective_pheromone_strength(&world.agents.modules[i]);
        if strength <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        for ch in 0..crate::program::PHEROMONE_CHANNELS {
            let intent = world.actions[i].emit_intent[ch];
            if intent > PHEROMONE_EMIT_THRESHOLD {
                world.pheromones.deposit(pos, ch, intent * strength * PHEROMONE_DEPOSIT_SCALE);
            }
        }
    }
}
```

Confirm `module::has`, `ModuleType`, `effective_pheromone_strength` are imported/pathed as in the existing passes.

- [ ] **Step 4: Add the decay stage** — in `crates/anabios-core/src/tick.rs`, after the carcass stage (`carcass_step(world)`):

```rust
    // Stage 8c: pheromone field decay (design §3.7 step 9).
    world.pheromones.decay_step();
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p anabios-core --test pheromone_territory`
Expected: PASS (all Task 1 + 2 tests).
Note (controller): `minimal.toml` agents have no `Pheromone` module → no deposition; decay of the all-zero field is a no-op. Golden hashes from Task 1 stay valid — run `cargo test -p anabios-core --test determinism` to confirm PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/interact.rs crates/anabios-core/src/tick.rs \
        crates/anabios-core/tests/pheromone_territory.rs
git commit -m "feat(core): M13 pheromone deposition (interact) + decay stage (tick)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Pheromone sensing (`SensePheromone` node, Smell-gated)

**Files:**
- Modify: `crates/anabios-core/src/sense.rs` (`SensorRegister.pheromone`; sample gated by `Smell`; `sense_all` signature + call site)
- Modify: `crates/anabios-core/src/tick.rs` (pass `&world.pheromones` to `sense_all`)
- Modify: `crates/anabios-core/src/program.rs` (`SensePheromone(u8)` node; `EvalContext.pheromone_sample`)
- Modify: `crates/anabios-core/src/behavior.rs` (thread `sensor.pheromone` into `EvalContext`)
- Test: `crates/anabios-core/tests/pheromone_territory.rs` (append)

**Interfaces:**
- Produces: `SensorRegister.pheromone: [f32; PHEROMONE_CHANNELS]` (serde-skip via the existing `sensors` scratch — no snapshot impact).
- Produces: `Node::SensePheromone(u8)` appended at the enum END; `arity => 0`; `node_kind => 40`; `evaluate` pushes `ctx.pheromone_sample[ch]`.
- Produces: `EvalContext.pheromone_sample: [f32; PHEROMONE_CHANNELS]`.
- Changes: `sense_all(agents, biome, spatial, registers)` → `sense_all(agents, biome, pheromones, spatial, registers)`.
- Consumes: `module::has_smell`, `PheromoneField::sample`.

- [ ] **Step 1: Write the failing test** — append:

```rust
#[test]
fn smell_sensored_agent_reads_local_pheromone_sensorless_reads_zero() {
    // Sensor agent: has a Smell sensor; a plant marker is pre-seeded at its cell.
    let mut w = World::new(2);
    let smeller = w.spawn_agent(Vec2::new(700.0, 700.0), Genome::neutral());
    let mut kit = marker_kit();
    // marker_kit's Sensor is Vision; swap to Smell so sensing is gated ON.
    for m in kit.iter_mut() {
        if let Module::Sensor { sensor_type, .. } = m {
            *sensor_type = SensorType::Smell;
        }
    }
    w.agents.modules[smeller as usize] = kit;
    // Program: move_x = SensePheromone(2). Plant a Trail (channel 2) at its cell.
    let pos = w.agents.position[smeller as usize];
    w.pheromones.deposit(pos, 2, 3.0);
    w.agents.program[smeller as usize] =
        Program::from_slice(&[Node::SensePheromone(2), Node::MoveTowardX]);
    step(&mut w);
    // A positive pheromone read drives move_x > 0 → normalized to +1 on x.
    assert!(w.desired_direction[smeller as usize].x > 0.9, "Smell agent reads the pheromone");

    // Sensorless agent (no Smell) reads zero → no movement from the same program.
    let mut w2 = World::new(2);
    let blind = w2.spawn_agent(Vec2::new(700.0, 700.0), Genome::neutral());
    // Default starter_kit Sensor is Vision (not Smell).
    let pos2 = w2.agents.position[blind as usize];
    w2.pheromones.deposit(pos2, 2, 3.0);
    w2.agents.program[blind as usize] =
        Program::from_slice(&[Node::SensePheromone(2), Node::MoveTowardX]);
    step(&mut w2);
    assert_eq!(w2.desired_direction[blind as usize].x, 0.0, "no Smell → reads zero (gating)");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p anabios-core --test pheromone_territory`
Expected: FAIL to compile (`Node::SensePheromone` doesn't exist).

- [ ] **Step 3: Add the `pheromone` field to SensorRegister** — in `crates/anabios-core/src/sense.rs`, add to `SensorRegister`:

```rust
    /// Local pheromone concentration per channel (0 unless the agent has a
    /// `Smell` sensor). Read by `Node::SensePheromone`.
    pub pheromone: [f32; crate::program::PHEROMONE_CHANNELS],
```

Add to `SensorRegister::default()`: `pheromone: [0.0; crate::program::PHEROMONE_CHANNELS],`.

- [ ] **Step 4: Sample the field in `sense_all`** — change the signature and add sampling. New signature:

```rust
pub fn sense_all(
    agents: &AgentBuffers,
    biome: &BiomeField,
    pheromones: &crate::pheromone::PheromoneField,
    spatial: &UniformSpatialHash,
    registers: &mut [SensorRegister],
) {
```

Inside the per-agent loop, after the existing biome/neighbor population, add:

```rust
        // Pheromone perception is gated by a Smell sensor module.
        if crate::module::has_smell(&agents.modules[i]) {
            let pos = agents.position[i];
            for ch in 0..crate::program::PHEROMONE_CHANNELS {
                registers[i].pheromone[ch] = pheromones.sample(pos, ch);
            }
        } else {
            registers[i].pheromone = [0.0; crate::program::PHEROMONE_CHANNELS];
        }
```

(Use the loop's existing agent index binding — match the existing variable name for `i`/`id`.)

Update the call site in `crates/anabios-core/src/tick.rs`:

```rust
    sense_all(&world.agents, &world.biome, &world.pheromones, &world.spatial, &mut world.sensors);
```

- [ ] **Step 5: Add the `SensePheromone` node** — in `crates/anabios-core/src/program.rs`:

Append to the `Node` enum END (after `SenseCrowding`):

```rust
    /// Local pheromone concentration on the given channel (Smell-gated). M13.
    SensePheromone(u8),
```

`arity`: add `Node::SensePheromone(_) => 0` to the arity-0 group.

`node_kind`: add `Node::SensePheromone(_) => 40` (next after `SenseCrowding => 39`).

`evaluate`: add an arm:

```rust
        Node::SensePheromone(ch) => {
            scratch.push(ctx.pheromone_sample[(ch as usize).min(PHEROMONE_CHANNELS - 1)])
        }
```

Add to `EvalContext`:

```rust
    pub pheromone_sample: [f32; PHEROMONE_CHANNELS],
```

**Do NOT add `SensePheromone` to `random_node`** (mutation grammar) — it's seeded via starters only, keeping evolved programs unchanged (Global Constraints).

- [ ] **Step 6: Thread the sample through `decide`** — in `crates/anabios-core/src/behavior.rs`, add to the `EvalContext { ... }` literal:

```rust
        pheromone_sample: sensor.pheromone,
```

- [ ] **Step 7: Run to verify pass**

Run: `cargo test -p anabios-core --test pheromone_territory`
Expected: PASS.
Note (controller): `SensorRegister`/`EvalContext` are not serialized (sensors is `#[serde(skip)]`); `SensePheromone` is appended and excluded from the grammar, so `minimal.toml` programs are unchanged. Golden hashes stay valid — run `cargo test -p anabios-core --test determinism` to confirm PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/anabios-core/src/sense.rs crates/anabios-core/src/tick.rs \
        crates/anabios-core/src/program.rs crates/anabios-core/src/behavior.rs \
        crates/anabios-core/tests/pheromone_territory.rs
git commit -m "feat(core): M13 pheromone sensing — SensePheromone node, Smell-gated

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: `TerritoryFormation` detector

**Files:**
- Modify: `crates/anabios-core/src/codex.rs` (EventType variant; CodexState fields; detector + pure helper; wire in)
- Test: `crates/anabios-core/tests/pheromone_territory.rs` (append)

**Design:** A **territory** is a `Pheromone`-bearing species whose members stay spatially tight (low RMS spread around their centroid) and persistent over a window. This is the measurable proxy for "a clustered, persistent footprint" (the pheromone field is the mechanism; spread is the signal). Torus-aware spread reuses `spatial::torus_distance`.

**Interfaces:**
- Produces: `EventType::TerritoryFormation = 9`.
- Produces: `CodexState.territory_spread: BTreeMap<u32, VecDeque<f32>>`, `CodexState.territory_active: BTreeSet<u32>`.
- Produces: `codex::TERRITORY_WINDOW: usize = 60`, `codex::TERRITORY_SPREAD_MAX: f32 = 120.0`, `codex::TERRITORY_MIN_MEMBERS: u32 = 5`.
- Produces (pure, testable): `codex::species_spread(positions: &[Vec2]) -> f32` — RMS torus distance of points from their torus-naive centroid (mean of coords).
- Consumes: `compute_centroids` result (for event loc), `world.agents.{position, species_id, modules}`, `module::has(.., Pheromone)`.

- [ ] **Step 1: Write the failing test** — append:

```rust
use anabios_core::codex::{species_spread, EventType, TERRITORY_SPREAD_MAX};

#[test]
fn species_spread_is_small_for_a_tight_cluster_large_for_a_dispersed_one() {
    let tight = [
        Vec2::new(500.0, 500.0),
        Vec2::new(505.0, 500.0),
        Vec2::new(500.0, 505.0),
    ];
    let dispersed = [
        Vec2::new(100.0, 100.0),
        Vec2::new(900.0, 100.0),
        Vec2::new(500.0, 900.0),
    ];
    assert!(species_spread(&tight) < TERRITORY_SPREAD_MAX);
    assert!(species_spread(&dispersed) > TERRITORY_SPREAD_MAX);
}

#[test]
fn territory_formation_fires_for_a_clustered_marking_species() {
    use anabios_core::codex::observe_all;
    let mut w = World::new(5);
    // Spawn a tight cluster of pheromone-markers as their own species.
    let mut ids = Vec::new();
    for k in 0..6 {
        let id = w.spawn_agent(Vec2::new(500.0 + k as f32, 500.0), Genome::neutral());
        w.agents.modules[id as usize] = marker_kit(); // has a Pheromone module
        ids.push(id);
    }
    // Move them all into one fresh species so they are measured together.
    let sid = w.species_centroids.len() as u32;
    w.species_centroids.push(Genome::neutral());
    w.species_parents.push(Some(0));
    w.species_member_counts.push(0);
    w.next_species_id = sid + 1;
    for &id in &ids {
        w.remove_from_species(w.agents.species_id[id as usize]);
        w.agents.species_id[id as usize] = sid;
        w.add_to_species(sid);
    }
    // Run observe_all for a full window without moving them (tight cluster persists).
    let mut fired = false;
    for _ in 0..(anabios_core::codex::TERRITORY_WINDOW + 2) {
        observe_all(&mut w);
        w.tick += 1;
        if w.codex.events.iter().any(|e| e.event_type == EventType::TerritoryFormation) {
            fired = true;
            break;
        }
    }
    assert!(fired, "a tight, persistent marking species forms a territory");
}
```

- [ ] **Step 2: Run to verify failure** — FAIL to compile (`species_spread`, `TerritoryFormation` missing).

- [ ] **Step 3: Implement** — in `crates/anabios-core/src/codex.rs`:

Append the variant (after `ArmsRace = 8`):

```rust
    /// A pheromone-marking species maintains a tight, persistent spatial cluster.
    TerritoryFormation = 9,
```

Constants:

```rust
/// Ticks a species must stay clustered to count as a formed territory.
pub const TERRITORY_WINDOW: usize = 60;
/// Max RMS spread (world units) for a species to count as "clustered".
pub const TERRITORY_SPREAD_MAX: f32 = 120.0;
/// Min members before territory clustering is meaningful.
pub const TERRITORY_MIN_MEMBERS: u32 = 5;
```

CodexState fields (before `events`):

```rust
    /// Rolling per-species RMS spatial spread (for TerritoryFormation).
    pub territory_spread: BTreeMap<u32, VecDeque<f32>>,
    /// Species currently latched as having a formed territory.
    pub territory_active: BTreeSet<u32>,
```

Pure helper (module-level `pub fn`):

```rust
/// RMS distance (torus-aware) of `positions` from their coordinate mean.
/// Returns 0.0 for fewer than 2 points.
pub fn species_spread(positions: &[glam::Vec2]) -> f32 {
    if positions.len() < 2 {
        return 0.0;
    }
    let n = positions.len() as f32;
    let mut cx = 0.0f64;
    let mut cy = 0.0f64;
    for p in positions {
        cx += p.x as f64;
        cy += p.y as f64;
    }
    let centroid = glam::Vec2::new((cx / n as f64) as f32, (cy / n as f64) as f32);
    let mut sumsq = 0.0f64;
    for p in positions {
        let d = crate::spatial::torus_distance(*p, centroid);
        sumsq += (d as f64) * (d as f64);
    }
    ((sumsq / n as f64).sqrt()) as f32
}
```

Detector:

```rust
/// TerritoryFormation: a pheromone-marking species that stays clustered (spread
/// ≤ TERRITORY_SPREAD_MAX) for TERRITORY_WINDOW consecutive ticks. Edge-
/// triggered per species; re-arms when the cluster disperses.
fn detect_territory_formation(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Gather per-species member positions and whether the species marks (has a
    // Pheromone module member). BTreeMap → deterministic.
    let mut positions: BTreeMap<u32, Vec<glam::Vec2>> = BTreeMap::new();
    let mut marks: BTreeSet<u32> = BTreeSet::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        positions.entry(sid).or_default().push(world.agents.position[i]);
        if crate::module::has(&world.agents.modules[i], crate::module::ModuleType::Pheromone) {
            marks.insert(sid);
        }
    }
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, ps) in positions.iter() {
        if (ps.len() as u32) < TERRITORY_MIN_MEMBERS || !marks.contains(sid) {
            world.codex.territory_spread.remove(sid);
            world.codex.territory_active.remove(sid);
            continue;
        }
        let spread = species_spread(ps);
        let buf = world.codex.territory_spread.entry(*sid).or_default();
        if buf.len() == TERRITORY_WINDOW {
            buf.pop_front();
        }
        buf.push_back(spread);
        let clustered = buf.len() == TERRITORY_WINDOW && buf.iter().all(|&s| s <= TERRITORY_SPREAD_MAX);
        if clustered && !world.codex.territory_active.contains(sid) {
            let (lx, ly) = centroid_of(centroids, *sid);
            to_push.push(CodexEvent {
                event_type: EventType::TerritoryFormation,
                tick,
                species_id: *sid,
                value: *buf.back().unwrap(),
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.territory_active.insert(*sid);
        } else if !clustered {
            world.codex.territory_active.remove(sid);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}
```

Wire into `observe_all` after `detect_arms_race`:

```rust
    detect_arms_race(world, &centroids);
    detect_territory_formation(world, &centroids);
```

Confirm `BTreeSet` is imported in `codex.rs` (map: yes).

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p anabios-core --test pheromone_territory`
Expected: PASS.
Note (controller): new serialized `CodexState` fields → refresh golden hashes (`UPDATE_HASHES=1`), update `GOLDEN`, confirm stable.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/codex.rs crates/anabios-core/tests/pheromone_territory.rs \
        crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M13 TerritoryFormation detector — clustered marking species

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: `NichePartitioning` detector

**Files:**
- Modify: `crates/anabios-core/src/codex.rs` (EventType variant; CodexState field; detector + pure helper; wire in)
- Test: `crates/anabios-core/tests/pheromone_territory.rs` (append)

**Design:** Two species partition a niche when their distributions over **terrain types** (the biome cell each member occupies) overlap below a threshold, sustained over a window. Overlap = histogram intersection `Σ min(fa_t, fb_t)` over terrain discriminants (1.0 = identical, 0.0 = disjoint). Track a per-pair low-overlap streak; fire (edge-triggered) when a pair's streak reaches the window.

**Interfaces:**
- Produces: `EventType::NichePartitioning = 10`.
- Produces: `CodexState.niche_streak: BTreeMap<(u32, u32), u32>`, `CodexState.niche_active: BTreeSet<(u32, u32)>`.
- Produces: `codex::NICHE_WINDOW: u32 = 60`, `codex::NICHE_OVERLAP_MAX: f32 = 0.35`, `codex::NICHE_MIN_MEMBERS: u32 = 5`.
- Produces (pure, testable): `codex::histogram_overlap(a: &BTreeMap<u8, f32>, b: &BTreeMap<u8, f32>) -> f32`.
- Consumes: `world.biome` (terrain at each member's cell), `world.agents.{position, species_id}`.

- [ ] **Step 1: Write the failing test** — append:

```rust
use anabios_core::codex::histogram_overlap;
use std::collections::BTreeMap;

#[test]
fn histogram_overlap_is_one_for_identical_zero_for_disjoint() {
    let mut a: BTreeMap<u8, f32> = BTreeMap::new();
    a.insert(0, 0.5);
    a.insert(1, 0.5);
    let identical = a.clone();
    assert!((histogram_overlap(&a, &identical) - 1.0).abs() < 1e-6);

    let mut b: BTreeMap<u8, f32> = BTreeMap::new();
    b.insert(2, 1.0); // disjoint terrain type
    assert_eq!(histogram_overlap(&a, &b), 0.0);
}
```

(The detector's firing is covered end-to-end by the emergence test in Task 9; the mechanism test here pins the pure overlap function, which is the part that could silently regress.)

- [ ] **Step 2: Run to verify failure** — FAIL to compile (`histogram_overlap`, `NichePartitioning` missing).

- [ ] **Step 3: Implement** — in `crates/anabios-core/src/codex.rs`:

Append the variant (after `TerritoryFormation = 9`):

```rust
    /// Two species occupy divergent terrain-type distributions (low overlap).
    NichePartitioning = 10,
```

Constants:

```rust
/// Ticks two species must stay below the overlap threshold to partition.
pub const NICHE_WINDOW: u32 = 60;
/// Max terrain-distribution overlap for two species to count as partitioned.
pub const NICHE_OVERLAP_MAX: f32 = 0.35;
/// Min members per species for niche comparison to be meaningful.
pub const NICHE_MIN_MEMBERS: u32 = 5;
```

CodexState fields (before `events`):

```rust
    /// Per species-pair consecutive-tick streak below the overlap threshold.
    pub niche_streak: BTreeMap<(u32, u32), u32>,
    /// Species pairs currently latched as niche-partitioned.
    pub niche_active: BTreeSet<(u32, u32)>,
```

Pure helper:

```rust
/// Histogram intersection of two normalized terrain distributions
/// (`Σ min(a_t, b_t)`): 1.0 identical, 0.0 disjoint.
pub fn histogram_overlap(a: &BTreeMap<u8, f32>, b: &BTreeMap<u8, f32>) -> f32 {
    let mut overlap = 0.0f32;
    for (t, av) in a.iter() {
        if let Some(bv) = b.get(t) {
            overlap += av.min(*bv);
        }
    }
    overlap
}
```

Detector:

```rust
/// NichePartitioning: two ≥NICHE_MIN_MEMBERS species whose terrain-type
/// distributions overlap ≤ NICHE_OVERLAP_MAX for NICHE_WINDOW consecutive ticks.
fn detect_niche_partitioning(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Per-species normalized terrain histogram (terrain discriminant → fraction).
    let mut counts: BTreeMap<u32, BTreeMap<u8, f32>> = BTreeMap::new();
    let mut totals: BTreeMap<u32, u32> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        let (col, row) = crate::biome::cell_coords(world.agents.position[i]);
        let terrain = world.biome.at(col, row).terrain as u8;
        *counts.entry(sid).or_default().entry(terrain).or_insert(0.0) += 1.0;
        *totals.entry(sid).or_insert(0) += 1;
    }
    // Normalize and keep only species with enough members.
    let mut hist: BTreeMap<u32, BTreeMap<u8, f32>> = BTreeMap::new();
    for (sid, h) in counts.into_iter() {
        let n = *totals.get(&sid).unwrap_or(&0);
        if n < NICHE_MIN_MEMBERS {
            continue;
        }
        let nf = n as f32;
        hist.insert(sid, h.into_iter().map(|(t, c)| (t, c / nf)).collect());
    }
    let sids: Vec<u32> = hist.keys().copied().collect();
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for ai in 0..sids.len() {
        for bi in (ai + 1)..sids.len() {
            let (a, b) = (sids[ai], sids[bi]); // a < b (ascending keys)
            let overlap = histogram_overlap(&hist[&a], &hist[&b]);
            let key = (a, b);
            if overlap <= NICHE_OVERLAP_MAX {
                let s = world.codex.niche_streak.entry(key).or_insert(0);
                *s += 1;
                if *s >= NICHE_WINDOW && !world.codex.niche_active.contains(&key) {
                    let (lx, ly) = centroid_of(centroids, a);
                    to_push.push(CodexEvent {
                        event_type: EventType::NichePartitioning,
                        tick,
                        species_id: a,
                        value: overlap,
                        loc_x: lx,
                        loc_y: ly,
                    });
                    world.codex.niche_active.insert(key);
                }
            } else {
                world.codex.niche_streak.remove(&key);
                world.codex.niche_active.remove(&key);
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}
```

Wire into `observe_all` after `detect_territory_formation`:

```rust
    detect_territory_formation(world, &centroids);
    detect_niche_partitioning(world, &centroids);
```

- [ ] **Step 4: Run to verify pass** — PASS. Controller: refresh golden hashes (new CodexState fields), confirm stable.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/codex.rs crates/anabios-core/tests/pheromone_territory.rs \
        crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M13 NichePartitioning detector — divergent terrain occupancy

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: Sweep integration (event names + CSV columns)

**Files:**
- Modify: `crates/anabios-headless/src/sweep.rs`
- Test: `crates/anabios-headless/src/sweep.rs` (extend the `event_name` test)

**Interfaces:** Consumes `EventType::{TerritoryFormation, NichePartitioning}`.

- [ ] **Step 1: Write the failing test** — extend the existing `event_name_covers_m12_events` test (or add `event_name_covers_m13_events`):

```rust
    #[test]
    fn event_name_covers_m13_events() {
        use anabios_core::codex::EventType;
        assert_eq!(super::event_name(EventType::TerritoryFormation), "territory_formation");
        assert_eq!(super::event_name(EventType::NichePartitioning), "niche_partitioning");
    }
```

- [ ] **Step 2: Run to verify failure** — FAIL (non-exhaustive match; `anabios-headless` won't compile with the 2 new variants).

- [ ] **Step 3: Extend `event_name`** — add:

```rust
        EventType::TerritoryFormation => "territory_formation",
        EventType::NichePartitioning => "niche_partitioning",
```

- [ ] **Step 4: Extend the CSV** — append `,territory_formation,niche_partitioning` to the header string; add two `{}` placeholders to the row format string; add `g("territory_formation"), g("niche_partitioning")` to the row args.

- [ ] **Step 5: Run to verify pass** — `cargo test -p anabios-headless` PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-headless/src/sweep.rs
git commit -m "feat(headless): M13 sweep — territory_formation/niche_partitioning columns

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Determinism lock + snapshot round-trip + workspace gate

Controller verification gate (no new production code).

- [ ] **Step 1:** `cargo test -p anabios-core --test determinism` → PASS (stable). If it fails, GOLDEN wasn't refreshed after the last snapshot-affecting task — regenerate and re-run.
- [ ] **Step 2:** `cargo test -p anabios-core --lib roundtrip_preserves_state` → PASS (new `pheromones` + CodexState fields survive save/load via generic bincode round-trip).
- [ ] **Step 3:** `cargo test --workspace` → all PASS; `cargo clippy --workspace --all-targets -- -D warnings` → clean; `cargo fmt --check` → clean. Fix any unused-import / unused-variable warnings in the new tests.
- [ ] **Step 4:** Commit only if anything changed here.

```bash
git add -A
git commit -m "test(core): M13 determinism + snapshot verification; fmt + clippy cleanup

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 8: `marker` archetype + `starter_marker` program

**Files:**
- Modify: `crates/anabios-core/src/program.rs` (`starter_marker`; add to `starter_library`)
- Modify: `crates/anabios-core/src/module.rs` (`marker_kit` module constructor)
- Modify: `crates/anabios-core/src/scenario.rs` (`marker` case in `archetype_kit`)
- Test: `crates/anabios-core/src/program.rs` / `scenario.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces: `module::marker_kit() -> ModuleList` — Locomotor + `Smell` Sensor + herbivore Mouth + `Pheromone { channel: Marker, strength: 1.0, decay: 0.1 }`.
- Produces: `program::starter_marker() -> Program` — deposit Marker pheromone every tick and cohere toward same-species (herd), so members cluster while marking.
- Produces: `"marker"` arm in `scenario::archetype_kit` → `(marker_kit(), starter_marker())`.

- [ ] **Step 1: Write the failing test** — in `scenario.rs` tests:

```rust
    #[test]
    fn marker_archetype_has_pheromone_and_smell_modules() {
        let text = r#"
name = "t"
seed = 1
[[agents]]
count = 5
archetype = "marker"
placement = { kind = "uniform" }
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        let w = s.instantiate();
        let id = w.agents.iter_alive().next().expect("one agent");
        let mods = &w.agents.modules[id as usize];
        assert!(crate::module::has(mods, crate::module::ModuleType::Pheromone));
        assert!(crate::module::has_smell(mods));
    }
```

- [ ] **Step 2: Run to verify failure** — FAIL (`"marker"` archetype falls through to grazer default → no Pheromone module).

- [ ] **Step 3: Add `marker_kit`** — in `module.rs`, after `predator_kit`:

```rust
/// A pheromone-marking herbivore: mobile, smells pheromones, grazes, and marks
/// territory on the Marker channel. Used by the `marker` scenario archetype.
pub fn marker_kit() -> ModuleList {
    smallvec![
        Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 },
        Module::Sensor { sensor_type: SensorType::Smell, radius: 0.7, acuity: 0.6 },
        Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 },
        Module::Pheromone { channel: PheromoneChannel::Marker, strength: 1.0, decay: 0.1 },
    ]
}
```

Confirm `PheromoneChannel` is imported/in scope in `module.rs` (it's defined there).

- [ ] **Step 4: Add `starter_marker`** — in `program.rs`, after the other starters:

```rust
/// Marker: emit Marker pheromone (channel 3) each tick and cohere toward the
/// nearest same-species neighbor (herd), so the group clusters while marking.
pub fn starter_marker() -> Program {
    Program::from_slice(&[
        // deposit a strong marker every tick
        Node::Const(1.0),
        Node::EmitPheromone(3),
        // cohesion toward same-species
        Node::SenseSameDirX,
        Node::MoveTowardX,
        Node::SenseSameDirY,
        Node::MoveTowardY,
    ])
}
```

Add `starter_marker` to the `starter_library()` slice (append at the END so existing founder indices are unchanged).

- [ ] **Step 5: Add the archetype arm** — in `scenario.rs` `archetype_kit`:

```rust
        "marker" => (marker_kit(), starter_marker()),
```

Add `marker_kit` to the `use crate::module::{...}` and `starter_marker` to the `use crate::program::{...}` imports in that function.

- [ ] **Step 6: Run to verify pass** — `cargo test -p anabios-core scenario` PASS. Controller: this doesn't touch `minimal.toml`; determinism stays green (new starter appended to library doesn't change existing founders).

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/program.rs crates/anabios-core/src/module.rs \
        crates/anabios-core/src/scenario.rs
git commit -m "feat(core): M13 marker archetype + starter_marker (mark-and-herd)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 9: Emergence scenario + multi-seed test

**Files:**
- Create: `scenarios/territories.toml`
- Create: `crates/anabios-core/tests/territory_emergence.rs`

**Emergence-test discipline (spec §2.2):** deterministic, release-gated. The controller measures the ACTUAL rate first, then sets the floor below observed.

- [ ] **Step 1: Write the scenario** — `scenarios/territories.toml`: two `marker` species seeded in separate clusters on a patchy biome, plus a plain grazer species, so distinct clustered footprints (and divergent terrain occupancy) can form:

```toml
name = "territories"
seed = 0

# Marking species A — clustered NW.
[[agents]]
count = 30
archetype = "marker"
placement = { kind = "cluster", center_x = 300.0, center_y = 300.0, radius = 90.0 }
[agents.traits]
lifespan_bias = 1.0

# Marking species B — clustered SE.
[[agents]]
count = 30
archetype = "marker"
placement = { kind = "cluster", center_x = 720.0, center_y = 720.0, radius = 90.0 }
[agents.traits]
lifespan_bias = 1.0
```

- [ ] **Step 2: Write the (initially failing) emergence test** — `crates/anabios-core/tests/territory_emergence.rs`, mirroring `predator_prey_emergence.rs`:

```rust
//! M13 emergence: seeded marking species form clustered territories.
//! Release-gated (ignored in debug) per spec §2.2.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/territories.toml");
const SEEDS: u64 = 16;
const TICKS: u32 = 400;
/// Floor set below the measured rate (Step 4 records the real number).
const TERRITORY_FLOOR: u64 = 10;

#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn territories_form_across_seeds() {
    let mut with_territory = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse territories");
        s.seed = seed;
        let mut w = s.instantiate();
        for _ in 0..TICKS {
            step(&mut w);
        }
        let formed = w
            .codex
            .events
            .iter()
            .any(|e| e.event_type == EventType::TerritoryFormation);
        if formed {
            with_territory += 1;
        }
    }
    assert!(
        with_territory >= TERRITORY_FLOOR,
        "TerritoryFormation in only {with_territory}/{SEEDS} seeds (floor {TERRITORY_FLOOR})"
    );
}
```

- [ ] **Step 3: Measure (controller, release):** `cargo test -p anabios-core --release --test territory_emergence -- --nocapture`. Temporarily add an `eprintln!` counting `TerritoryFormation` and `NichePartitioning` seeds to read the real rate.

- [ ] **Step 4: Tune (controller judgment):** set `TERRITORY_FLOOR` a few below the observed count. If territories are unreliable, adjust the scenario (tighter `radius`, more members, longer `TICKS`) or the detector windows. If `TerritoryFormation` is marginal but `NichePartitioning` is strong, assert on whichever is robust (or both). Record the observed rate in a comment. Remove the temporary `eprintln!`.

- [ ] **Step 5: Verify gating:** release run PASS; debug run shows `0 passed / 1 ignored`.

- [ ] **Step 6: Commit**

```bash
git add scenarios/territories.toml crates/anabios-core/tests/territory_emergence.rs
git commit -m "test(core): M13 territory emergence — TerritoryFormation across seeds

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review (author checklist — completed)

**Spec coverage (spec §M13):**
- Pheromone fields (per-channel 128×128, decay) → Task 1 (+ deposit/decay Task 2). ✅
- Pheromone sensing gated by `smell`, `SensePheromone(channel)` node → Task 3. ✅
- `TerritoryFormation` detector → Task 4; `NichePartitioning` → Task 5. ✅
- Mechanism tests (emit→cell gains; no module→nothing; decay by exact rate; smell reads planted value, sensorless reads zero; detectors fire on clustered/disjoint, not uniform/overlap) → Tasks 1–5. ✅
- Emergence scenario + multi-seed test → Tasks 8 (marker archetype) + 9. ✅
- Sweep integration → Task 6. ✅
- Golden-tick refresh (§2.3) → Tasks 1/4/5 (controller) + Task 7 lock. ✅

**Placeholder scan:** every code step has full code; the only judgment step (Task 9 Step 4 floor tuning) is an explicit measure-then-set controller action. ✅

**Type consistency:** `PheromoneField` API (`deposit/sample/decay_step`) consistent Tasks 1↔2↔3. `pheromone_sample`/`SensorRegister.pheromone` consistent Tasks 3. `species_spread`/`histogram_overlap` pure helpers consistent Tasks 4/5↔tests. `EventType` appended 9/10; `Node::SensePheromone` appended (node_kind 40). `marker_kit`/`starter_marker` consistent Tasks 8↔9. ✅

## Deviation notes (for reviewers)

- **Field decay is a global `PHEROMONE_DECAY` constant** (per-channel identical), not driven by the per-module `Pheromone.decay` parameter — a shared field can't have a per-depositor decay. The module `decay` field is currently unused by the field; documented, tuning deferred to M16.
- **`SensePheromone` returns the local cell's scalar concentration** (per channel), gated by a `Smell` sensor — this satisfies the spec's own mechanism test ("smell-sensored reads a planted value, sensorless reads zero"). Directional gradient-following nodes are not added; territorial *avoidance* behavior is not required for the detectors (which measure spatial clustering / terrain divergence directly).
- **`TerritoryFormation` measures persistent spatial clustering of a pheromone-bearing species** (RMS spread ≤ threshold over a window) as the proxy for "a clustered, persistent footprint others avoid." The pheromone field is the in-world mechanism; the detector's signal is the members' spatial tightness. Documented; can be sharpened in M16 balancing.
