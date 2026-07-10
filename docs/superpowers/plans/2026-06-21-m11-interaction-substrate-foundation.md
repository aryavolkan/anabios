# M11 — Interaction Substrate Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the inert collaboration/competition AST nodes real and give programs richer perception (kin/threat/crowding), so M12–M15 can build combat, pheromones, communication, and cooperation on top — without changing any existing simulation behavior.

**Architecture:** Three additive layers in `anabios-core`. (1) `SensorRegister` gains same-species/other-species/relative-size/relative-energy/crowding channels computed in the existing spatial-hash neighbor scan. (2) The program grammar gains matching live `Sense*` input nodes, and the three previously-no-op output nodes (`FireWeapon`, `EmitPheromone`, `Broadcast`) now write to an extended `ActionRegister`. (3) `decide()` returns the full `ActionRegister` (intents + a sensor-derived target id), stored in a new per-agent `world.actions` scratch buffer that M12 will consume. Plus four new social starter programs.

**Tech Stack:** Rust, `anabios-core` crate, `glam::Vec2`, `smallvec`, `cargo test`.

**Determinism contract (read before starting):** This milestone is designed to be **hash-neutral**. `starter_grazer` is untouched; the new sense nodes are deliberately *not* added to the mutation grammar (`random_node`); and all new fields live in `#[serde(skip)]` scratch buffers (`sensors`, `actions`) that are not part of `state_hash`. Therefore `tests/determinism.rs` must continue to **pass unchanged** — do NOT regenerate the golden hashes. If it fails, you introduced an unintended behavior change; debug it rather than refreshing. (This is a deliberate, safe deviation from design spec §2.3, which anticipated a hash refresh; M11 turned out to be fully additive.)

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `crates/anabios-core/src/sense.rs` | per-agent perception | Modify: add channels to `SensorRegister`, populate in `sense_all` |
| `crates/anabios-core/src/program.rs` | AST, evaluator, starters | Modify: new `Sense*` nodes, extended `ActionRegister`, channel constants, evaluator arms, 4 starters |
| `crates/anabios-core/src/behavior.rs` | decide dispatch | Modify: `decide()` returns `ActionRegister` with target |
| `crates/anabios-core/src/world.rs` | root state | Modify: add `actions` scratch buffer + resize |
| `crates/anabios-core/src/tick.rs` | tick orchestration | Modify: `decide_all` stores actions + normalizes direction |
| `crates/anabios-core/tests/social_substrate.rs` | M11 mechanism tests | Create |

---

## Task 1: Extend `SensorRegister` with kin/threat/crowding channels

**Files:**
- Modify: `crates/anabios-core/src/sense.rs`

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests` block in `crates/anabios-core/src/sense.rs` (after `isolated_agent_has_no_neighbor`):

```rust
    #[test]
    fn distinguishes_same_and_other_species() {
        let mut w = World::new(1);
        let me = w.spawn_agent(Vec2::new(100.0, 100.0), Genome::neutral());
        let kin = w.spawn_agent(Vec2::new(106.0, 100.0), Genome::neutral()); // same species 0
        let foe = w.spawn_agent(Vec2::new(103.0, 100.0), Genome::neutral());
        w.agents.species_id[foe as usize] = 1; // make foe another species
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.spatial, &mut regs);
        let r = regs[me as usize];
        assert_eq!(r.nearest_same_id, kin);
        assert!((r.nearest_same_dist - 6.0).abs() < 1e-3);
        assert!(r.nearest_same_dir.x > 0.9);
        assert_eq!(r.nearest_other_id, foe);
        assert!((r.nearest_other_dist - 3.0).abs() < 1e-3);
        assert!(r.nearest_other_dir.x > 0.9);
        // Overall nearest is the foe (3 < 6).
        assert_eq!(r.nearest_neighbor_id, foe);
    }

    #[test]
    fn relative_size_and_energy_of_nearest() {
        let mut w = World::new(1);
        let mut big = Genome::neutral();
        big.set(GenomeSlot::Size, 1.0);
        let mut small = Genome::neutral();
        small.set(GenomeSlot::Size, 0.5);
        let me = w.spawn_agent(Vec2::new(200.0, 200.0), small);
        let other = w.spawn_agent(Vec2::new(204.0, 200.0), big);
        w.agents.energy[me as usize] = 20.0;
        w.agents.energy[other as usize] = 40.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.spatial, &mut regs);
        let r = regs[me as usize];
        assert!((r.nearest_rel_size - 2.0).abs() < 1e-3, "1.0/0.5 = 2.0, got {}", r.nearest_rel_size);
        assert!((r.nearest_rel_energy - 2.0).abs() < 1e-3, "40/20 = 2.0, got {}", r.nearest_rel_energy);
    }

    #[test]
    fn crowding_counts_neighbors_in_radius() {
        let mut w = World::new(1);
        let me = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let _ = w.spawn_agent(Vec2::new(303.0, 300.0), Genome::neutral());
        let _ = w.spawn_agent(Vec2::new(300.0, 303.0), Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.spatial, &mut regs);
        assert_eq!(regs[me as usize].crowding, 2);
    }
```

The test module already imports `World`, `Genome`, `Vec2`, `SensorRegister`, and `super::*`. Add `use crate::genome::GenomeSlot;` to the test module's `use` lines if not present (it is not — add it next to `use crate::world::World;`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p anabios-core --lib sense::tests::distinguishes_same_and_other_species`
Expected: FAIL — `no field nearest_same_id on type SensorRegister` (compile error).

- [ ] **Step 3: Add the new fields to `SensorRegister` and its `Default`**

In `crates/anabios-core/src/sense.rs`, add a sentinel constant next to `NO_NEIGHBOR_SPECIES`:

```rust
/// Sentinel in `SensorRegister` id fields meaning "no such neighbor".
pub const NO_NEIGHBOR_ID: u32 = u32::MAX;
```

Extend the `SensorRegister` struct (append these fields after `nearest_neighbor_species`):

```rust
    /// Id of the nearest neighbor of any species, or `NO_NEIGHBOR_ID`.
    pub nearest_neighbor_id: u32,
    /// Distance to the nearest neighbor of the SAME species, or `f32::INFINITY`.
    pub nearest_same_dist: f32,
    /// Unit direction to the nearest same-species neighbor.
    pub nearest_same_dir: Vec2,
    /// Id of the nearest same-species neighbor, or `NO_NEIGHBOR_ID`.
    pub nearest_same_id: u32,
    /// Distance to the nearest neighbor of a DIFFERENT species.
    pub nearest_other_dist: f32,
    /// Unit direction to the nearest other-species neighbor.
    pub nearest_other_dir: Vec2,
    /// Id of the nearest other-species neighbor, or `NO_NEIGHBOR_ID`.
    pub nearest_other_id: u32,
    /// `other.size / self.size` of the overall-nearest neighbor; 0.0 if none.
    pub nearest_rel_size: f32,
    /// `other.energy / self.energy` of the overall-nearest neighbor; 0.0 if none.
    pub nearest_rel_energy: f32,
    /// Count of alive neighbors within perception radius.
    pub crowding: u32,
```

Extend the `Default` impl (append after `nearest_neighbor_species`):

```rust
            nearest_neighbor_id: NO_NEIGHBOR_ID,
            nearest_same_dist: f32::INFINITY,
            nearest_same_dir: Vec2::ZERO,
            nearest_same_id: NO_NEIGHBOR_ID,
            nearest_other_dist: f32::INFINITY,
            nearest_other_dir: Vec2::ZERO,
            nearest_other_id: NO_NEIGHBOR_ID,
            nearest_rel_size: 0.0,
            nearest_rel_energy: 0.0,
            crowding: 0,
```

- [ ] **Step 4: Populate the new fields in `sense_all`**

In `sense_all`, replace the neighbor-scan block (the `let mut nearest_dist ...` declarations through the `spatial.query(...)` closure and the `registers[i] = SensorRegister { ... };` assignment) with:

```rust
        let self_species = agents.species_id[i];
        let self_size = genome.get(GenomeSlot::Size).max(1e-3);
        let self_energy = agents.energy[i].max(1e-3);

        let mut nearest_dist = f32::INFINITY;
        let mut nearest_dir = Vec2::ZERO;
        let mut has_neighbor = false;
        let mut nearest_species: u32 = NO_NEIGHBOR_SPECIES;
        let mut nearest_id: u32 = NO_NEIGHBOR_ID;
        let mut nearest_rel_size = 0.0_f32;
        let mut nearest_rel_energy = 0.0_f32;
        let mut same_dist = f32::INFINITY;
        let mut same_dir = Vec2::ZERO;
        let mut same_id: u32 = NO_NEIGHBOR_ID;
        let mut other_dist = f32::INFINITY;
        let mut other_dir = Vec2::ZERO;
        let mut other_id: u32 = NO_NEIGHBOR_ID;
        let mut crowding: u32 = 0;

        spatial.query(pos, radius, |oid| {
            if oid == id {
                return;
            }
            let other_pos = agents.position[oid as usize];
            let d = torus_distance(pos, other_pos);
            if d > radius {
                return;
            }
            crowding += 1;
            let dir = torus_direction(pos, other_pos);
            let other_species = agents.species_id[oid as usize];
            if d < nearest_dist {
                nearest_dist = d;
                nearest_dir = dir;
                has_neighbor = true;
                nearest_species = other_species;
                nearest_id = oid;
                nearest_rel_size = agents.genome[oid as usize].get(GenomeSlot::Size) / self_size;
                nearest_rel_energy = agents.energy[oid as usize] / self_energy;
            }
            if other_species == self_species {
                if d < same_dist {
                    same_dist = d;
                    same_dir = dir;
                    same_id = oid;
                }
            } else if d < other_dist {
                other_dist = d;
                other_dir = dir;
                other_id = oid;
            }
        });

        registers[i] = SensorRegister {
            local_plant_biomass: local_cell.plant_biomass,
            plant_direction,
            nearest_neighbor_dist: nearest_dist,
            nearest_neighbor_dir: nearest_dir,
            has_neighbor,
            nearest_neighbor_species: nearest_species,
            nearest_neighbor_id: nearest_id,
            nearest_same_dist: same_dist,
            nearest_same_dir: same_dir,
            nearest_same_id: same_id,
            nearest_other_dist: other_dist,
            nearest_other_dir: other_dir,
            nearest_other_id: other_id,
            nearest_rel_size,
            nearest_rel_energy,
            crowding,
        };
```

Note: the original loop only counted the single nearest; the new version still iterates every in-radius neighbor in deterministic spatial-hash order, so ordering/determinism is preserved.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p anabios-core --lib sense::tests`
Expected: PASS — all existing sense tests plus the three new ones. (The existing `agent_finds_neighbor_within_perception` still passes because `nearest_neighbor_*` keep their meaning.)

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/sense.rs
git commit -m "feat(core): M11 sense — kin/threat/relative/crowding channels"
```

---

## Task 2: Add live `Sense*` AST nodes for the new channels

**Files:**
- Modify: `crates/anabios-core/src/program.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/anabios-core/src/program.rs`:

```rust
    #[test]
    fn new_sense_nodes_push_context_values() {
        let g = Genome::neutral();
        let ctx = EvalContext {
            same_distance: 6.0,
            same_dir: glam::Vec2::new(1.0, 0.0),
            other_distance: 3.0,
            other_dir: glam::Vec2::new(0.0, 1.0),
            rel_size: 2.0,
            rel_energy: 0.5,
            crowding: 4.0,
            ..dummy_ctx(&g)
        };
        let mut stack = Vec::new();
        // MoveTowardX of SenseOtherDirY (=0.0) -> 0; check via SenseRelSize.
        let p = Program::from_slice(&[Node::SenseRelSize, Node::MoveTowardX]);
        let a = evaluate(&p, ctx, &mut stack);
        assert_eq!(a.move_x, 2.0);
        let p2 = Program::from_slice(&[Node::SenseCrowding, Node::MoveTowardY]);
        let a2 = evaluate(&p2, ctx, &mut stack);
        assert_eq!(a2.move_y, 4.0);
    }
```

This will not compile until `EvalContext` has the new fields and `Node` has the new variants.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib program::tests::new_sense_nodes_push_context_values`
Expected: FAIL — `EvalContext` has no field `same_distance` / no variant `SenseRelSize`.

- [ ] **Step 3: Add the node variants**

In the `Node` enum (`program.rs`), append the variants at the **very end of the enum, after `Idle`** (NOT in the Inputs group). Serde/bincode encodes enum variants by positional index, so inserting mid-enum shifts the discriminants of every later variant and changes the serialized bytes of existing agent programs — breaking the golden-tick hashes. Appending keeps M11 hash-neutral; logical discriminants come from `node_kind`, which is position-independent. Also: because `decide()` in `behavior.rs` builds an `EvalContext` struct literal, you MUST populate the new `EvalContext` fields there in this same task (see below) or the crate won't compile.

```rust
    SenseSameDist,
    SenseSameDirX,
    SenseSameDirY,
    SenseOtherDist,
    SenseOtherDirX,
    SenseOtherDirY,
    SenseRelSize,
    SenseRelEnergy,
    SenseCrowding,
```

- [ ] **Step 4: Update `arity`, `node_kind`, and `EvalContext`**

In `Program::arity`, add the nine new variants to the arity-0 input list (the `| Node::SenseLocalBiomass | Node::SenseMeme(_) | Node::Const(_) => 0,` arm). The arm becomes:

```rust
            Node::SenseEnergy
            | Node::SenseAge
            | Node::SenseGenome(_)
            | Node::SenseNearestDistance
            | Node::SenseNearestDirX
            | Node::SenseNearestDirY
            | Node::SensePlantDirX
            | Node::SensePlantDirY
            | Node::SenseLocalBiomass
            | Node::SenseMeme(_)
            | Node::SenseSameDist
            | Node::SenseSameDirX
            | Node::SenseSameDirY
            | Node::SenseOtherDist
            | Node::SenseOtherDirX
            | Node::SenseOtherDirY
            | Node::SenseRelSize
            | Node::SenseRelEnergy
            | Node::SenseCrowding
            | Node::Const(_) => 0,
```

In `Program::node_kind`, the existing discriminants run 0..=30 (`Idle => 30`). Append the new kinds after the `Node::Idle => 30,` line:

```rust
            Node::SenseSameDist => 31,
            Node::SenseSameDirX => 32,
            Node::SenseSameDirY => 33,
            Node::SenseOtherDist => 34,
            Node::SenseOtherDirX => 35,
            Node::SenseOtherDirY => 36,
            Node::SenseRelSize => 37,
            Node::SenseRelEnergy => 38,
            Node::SenseCrowding => 39,
```

In `EvalContext`, append the new fields after `local_biomass`:

```rust
    pub same_distance: f32,
    pub same_dir: glam::Vec2,
    pub other_distance: f32,
    pub other_dir: glam::Vec2,
    pub rel_size: f32,
    pub rel_energy: f32,
    pub crowding: f32,
```

- [ ] **Step 5: Add evaluator arms and fix the test helper**

In `evaluate`, after the `Node::SenseLocalBiomass => ...` arm (and before `Node::SenseMeme(_) => ...`), add:

```rust
            Node::SenseSameDist => scratch.push(ctx.same_distance.min(1e6)),
            Node::SenseSameDirX => scratch.push(ctx.same_dir.x),
            Node::SenseSameDirY => scratch.push(ctx.same_dir.y),
            Node::SenseOtherDist => scratch.push(ctx.other_distance.min(1e6)),
            Node::SenseOtherDirX => scratch.push(ctx.other_dir.x),
            Node::SenseOtherDirY => scratch.push(ctx.other_dir.y),
            Node::SenseRelSize => scratch.push(ctx.rel_size),
            Node::SenseRelEnergy => scratch.push(ctx.rel_energy),
            Node::SenseCrowding => scratch.push(ctx.crowding),
```

The test helper `dummy_ctx` must initialize the new fields. Replace the `EvalContext { ... }` literal in `dummy_ctx` with:

```rust
        EvalContext {
            energy: 30.0,
            age: 100,
            genome,
            nearest_distance: 5.0,
            nearest_dir: glam::Vec2::new(1.0, 0.0),
            plant_dir: glam::Vec2::new(0.0, 1.0),
            local_biomass: 8.0,
            same_distance: f32::INFINITY,
            same_dir: glam::Vec2::ZERO,
            other_distance: f32::INFINITY,
            other_dir: glam::Vec2::ZERO,
            rel_size: 0.0,
            rel_energy: 0.0,
            crowding: 0.0,
        }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p anabios-core --lib program::tests`
Expected: PASS — including `new_sense_nodes_push_context_values`.

Note: `random_node` is intentionally NOT modified — the new senses stay out of the mutation grammar to keep M11 hash-neutral (see the Determinism contract). A later milestone adds them when mutation should reach them.

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/program.rs
git commit -m "feat(core): M11 program — live kin/threat/crowding sense nodes"
```

---

## Task 3: Wire the inert output nodes into an extended `ActionRegister`

**Files:**
- Modify: `crates/anabios-core/src/program.rs`

- [ ] **Step 1: Write the failing test**

Add to `program.rs` test module:

```rust
    #[test]
    fn fire_emit_broadcast_write_intents() {
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let p = Program::from_slice(&[
            Node::Const(0.8),
            Node::FireWeapon,
            Node::Const(0.5),
            Node::EmitPheromone(2),
            Node::Const(0.9),
            Node::Broadcast(1),
        ]);
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert_eq!(a.fire_intent, 0.8);
        assert_eq!(a.emit_intent[2], 0.5);
        assert_eq!(a.broadcast_intent[1], 0.9);
        assert_eq!(a.target_id, NO_TARGET);
    }

    #[test]
    fn out_of_range_channels_clamp() {
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let p = Program::from_slice(&[
            Node::Const(1.0),
            Node::EmitPheromone(250), // clamps to last pheromone channel
            Node::Const(1.0),
            Node::Broadcast(250), // clamps to last meme channel
        ]);
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert_eq!(a.emit_intent[PHEROMONE_CHANNELS - 1], 1.0);
        assert_eq!(a.broadcast_intent[MEME_CHANNELS - 1], 1.0);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib program::tests::fire_emit_broadcast_write_intents`
Expected: FAIL — `ActionRegister` has no field `fire_intent`; `NO_TARGET` not found.

- [ ] **Step 3: Add channel constants and extend `ActionRegister`**

In `program.rs`, near the top constants (after `pub const CONST_SIGMA: f32 = 0.1;`), add:

```rust
/// Number of pheromone channels (design §3.6). Wired by M13.
pub const PHEROMONE_CHANNELS: usize = 4;
/// Number of meme/broadcast channels (design §3.1). Wired by M14.
pub const MEME_CHANNELS: usize = 8;
/// Sentinel in `ActionRegister.target_id` meaning "no action target".
pub const NO_TARGET: u32 = u32::MAX;
```

Replace the `ActionRegister` definition. Change the derive line (drop `Default`) and add fields + a manual `Default`:

```rust
/// What an agent wants to do this tick, produced by the evaluator.
#[derive(Debug, Clone, Copy)]
pub struct ActionRegister {
    pub move_x: f32,
    pub move_y: f32,
    pub feed_intent: f32,
    pub mate_intent: f32,
    pub fire_intent: f32,
    pub emit_intent: [f32; PHEROMONE_CHANNELS],
    pub broadcast_intent: [f32; MEME_CHANNELS],
    /// Agent this action is directed at (combat/share target), derived from
    /// the nearest-neighbor sense. `NO_TARGET` when there is no neighbor.
    pub target_id: u32,
}

impl Default for ActionRegister {
    fn default() -> Self {
        Self {
            move_x: 0.0,
            move_y: 0.0,
            feed_intent: 0.0,
            mate_intent: 0.0,
            fire_intent: 0.0,
            emit_intent: [0.0; PHEROMONE_CHANNELS],
            broadcast_intent: [0.0; MEME_CHANNELS],
            target_id: NO_TARGET,
        }
    }
}
```

- [ ] **Step 4: Split the no-op output arm in `evaluate`**

Replace the combined arm:

```rust
            Node::FireWeapon | Node::EmitPheromone(_) | Node::Broadcast(_) | Node::Idle => {
                scratch.pop();
            }
```

with:

```rust
            Node::FireWeapon => action.fire_intent += scratch.pop().unwrap(),
            Node::EmitPheromone(ch) => {
                let v = scratch.pop().unwrap();
                action.emit_intent[(ch as usize).min(PHEROMONE_CHANNELS - 1)] += v;
            }
            Node::Broadcast(ch) => {
                let v = scratch.pop().unwrap();
                action.broadcast_intent[(ch as usize).min(MEME_CHANNELS - 1)] += v;
            }
            Node::Idle => {
                scratch.pop();
            }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p anabios-core --lib program::tests`
Expected: PASS — including the two new tests. (Existing `empty_program_yields_zero_action` etc. still pass since `Default` produces the same zeroed move/feed/mate.)

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/program.rs
git commit -m "feat(core): M11 program — wire FireWeapon/EmitPheromone/Broadcast intents"
```

---

## Task 4: `decide()` returns full `ActionRegister`; add `world.actions` buffer

**Files:**
- Modify: `crates/anabios-core/src/behavior.rs`
- Modify: `crates/anabios-core/src/world.rs`
- Modify: `crates/anabios-core/src/tick.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/anabios-core/src/tick.rs` (it already imports `World`, `Genome`, `GenomeSlot`, `Vec2`, `step`):

```rust
    #[test]
    fn decide_populates_action_buffer_with_target() {
        use crate::program::{Node, Program, NO_TARGET};
        let mut w = World::new(1);
        // A program that always fires with intent 1.0.
        let prog = Program::from_slice(&[Node::Const(1.0), Node::FireWeapon]);
        let a = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
        let b = w.spawn_agent(Vec2::new(404.0, 400.0), Genome::neutral());
        w.agents.program[a as usize] = prog;
        // One tick runs sense -> decide; afterwards actions[a] reflects the program.
        step(&mut w);
        assert!(w.actions[a as usize].fire_intent > 0.0);
        // a's nearest neighbor is b, so target should be b (not NO_TARGET).
        assert_eq!(w.actions[a as usize].target_id, b);
        assert_ne!(w.actions[a as usize].target_id, NO_TARGET);
    }
```

Note: this test lives in `tick.rs`'s inline `mod tests`, inside the `anabios-core` crate, so it refers to the crate via `crate::` (as shown). The `..dummy_ctx` style is not needed here — this drives the public `step()`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib tick::tests::decide_populates_action_buffer_with_target`
Expected: FAIL — `World` has no field `actions`.

- [ ] **Step 3: Change `decide()` to return `ActionRegister` with target**

Replace the body of `decide()` in `crates/anabios-core/src/behavior.rs` so it returns the full register (imports at top already include `ActionRegister`, `EvalContext`, `Program`). New file content for the function and imports:

```rust
use crate::genome::Genome;
use crate::program::{evaluate, ActionRegister, EvalContext, Program, NO_TARGET};
use crate::sense::{SensorRegister, NO_NEIGHBOR_ID};

/// Evaluate one agent's program and return its full action register, including
/// a sensor-derived `target_id` (the overall-nearest neighbor). The integrate
/// stage consumes `move_x/move_y`; M12+ consumes the intents and target.
pub fn decide(
    program: &Program,
    genome: &Genome,
    sensor: &SensorRegister,
    energy: f32,
    age: u32,
    eval_stack: &mut Vec<f32>,
) -> ActionRegister {
    let ctx = EvalContext {
        energy,
        age,
        genome,
        nearest_distance: sensor.nearest_neighbor_dist,
        nearest_dir: sensor.nearest_neighbor_dir,
        plant_dir: sensor.plant_direction,
        local_biomass: sensor.local_plant_biomass,
        same_distance: sensor.nearest_same_dist,
        same_dir: sensor.nearest_same_dir,
        other_distance: sensor.nearest_other_dist,
        other_dir: sensor.nearest_other_dir,
        rel_size: sensor.nearest_rel_size,
        rel_energy: sensor.nearest_rel_energy,
        crowding: sensor.crowding as f32,
    };
    let mut action = evaluate(program, ctx, eval_stack);
    action.target_id = if sensor.nearest_neighbor_id == NO_NEIGHBOR_ID {
        NO_TARGET
    } else {
        sensor.nearest_neighbor_id
    };
    action
}
```

- [ ] **Step 4: Add the `actions` buffer to `World`**

In `crates/anabios-core/src/world.rs`, add a field after `desired_direction` (keep the `#[serde(skip)]` group):

```rust
    /// Per-agent action register from `decide()`. Scratch, recomputed each
    /// tick. Consumed by `interact` starting in M12.
    #[serde(skip)]
    pub actions: Vec<crate::program::ActionRegister>,
```

In `World::new`, initialize it after `desired_direction: Vec::new(),`:

```rust
            actions: Vec::new(),
```

In `resize_scratch`, add after the `desired_direction` resize block:

```rust
        if self.actions.len() < cap {
            self.actions.resize(cap, crate::program::ActionRegister::default());
        }
```

- [ ] **Step 5: Update `decide_all` in `tick.rs` to store actions and normalize direction**

Replace the body of `decide_all` in `crates/anabios-core/src/tick.rs`:

```rust
fn decide_all(world: &mut World) {
    use crate::prelude::Vec2;
    // Deterministic order: ascending id. Programs are evaluated against the
    // shared `world.eval_stack` scratch buffer.
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
        let action = decide(
            &world.agents.program[i],
            &world.agents.genome[i],
            &world.sensors[i],
            world.agents.energy[i],
            world.agents.age[i],
            &mut world.eval_stack,
        );
        // Normalize the movement intent to a unit direction (identical to the
        // pre-M11 logic that lived inside `decide`).
        let v = Vec2::new(action.move_x, action.move_y);
        let len = v.length();
        world.desired_direction[i] = if len < 1e-4 { Vec2::ZERO } else { v / len };
        world.actions[i] = action;
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p anabios-core`
Expected: PASS — the whole core suite, including the new tick test. Crucially, `tests::determinism` still passes with the committed golden hashes (movement math is byte-identical and the new buffers are scratch).

If `determinism` fails: STOP. You changed observable behavior. The most likely cause is the normalization differing from the original `decide()` — confirm it matches exactly (`< 1e-4` threshold, `v / len`).

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/behavior.rs crates/anabios-core/src/world.rs crates/anabios-core/src/tick.rs
git commit -m "feat(core): M11 decide returns ActionRegister; world.actions buffer"
```

---

## Task 5: Add the social starter programs

**Files:**
- Modify: `crates/anabios-core/src/program.rs`

- [ ] **Step 1: Write the failing test**

Add to `program.rs` test module:

```rust
    #[test]
    fn social_starters_are_bounded_and_evaluable() {
        let g = Genome::neutral();
        let mut stack = Vec::new();
        for make in [starter_stalker, starter_pack_hunter, starter_sentinel, starter_herd] {
            let p = make();
            assert!(!p.is_empty());
            assert!(p.len() <= PROGRAM_MAX_NODES);
            // Must evaluate without panicking against a populated context.
            let _ = evaluate(&p, dummy_ctx(&g), &mut stack);
        }
    }

    #[test]
    fn herd_moves_toward_same_species() {
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let ctx = EvalContext { same_dir: glam::Vec2::new(1.0, 0.0), ..dummy_ctx(&g) };
        let a = evaluate(&starter_herd(), ctx, &mut stack);
        assert!(a.move_x > 0.0, "herd should move toward same-species: {:?}", a);
    }

    #[test]
    fn starter_library_has_all_starters() {
        assert_eq!(starter_library().len(), 5);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib program::tests::social_starters_are_bounded_and_evaluable`
Expected: FAIL — `cannot find function starter_stalker`.

- [ ] **Step 3: Implement the four starters and extend the library**

In `program.rs`, after `starter_grazer()` and before `starter_library()`, add:

```rust
/// Stalker: approach the nearest other-species agent and fire a weapon when
/// within ~3 units. (`FireWeapon` is inert until M12 wires combat.)
pub fn starter_stalker() -> Program {
    Program::from_slice(&[
        Node::SenseOtherDirX,
        Node::MoveTowardX,
        Node::SenseOtherDirY,
        Node::MoveTowardY,
        // fire when other_dist < 3  ==  (-other_dist) > -3
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-3.0),
        Node::FireWeapon,
    ])
}

/// Pack hunter: approach prey, broadcast its presence on channel 0 when near,
/// and fire when adjacent. (Broadcast/FireWeapon inert until M14/M12.)
pub fn starter_pack_hunter() -> Program {
    Program::from_slice(&[
        Node::SenseOtherDirX,
        Node::MoveTowardX,
        Node::SenseOtherDirY,
        Node::MoveTowardY,
        // broadcast presence when other_dist < 5
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-5.0),
        Node::Broadcast(0),
        // fire when other_dist < 3
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-3.0),
        Node::FireWeapon,
    ])
}

/// Sentinel: flee from the nearest other-species agent and raise an alarm on
/// channel 1 when one is within ~8 units. (Broadcast inert until M14.)
pub fn starter_sentinel() -> Program {
    Program::from_slice(&[
        Node::SenseOtherDirX,
        Node::MoveAwayX,
        Node::SenseOtherDirY,
        Node::MoveAwayY,
        // alarm when other_dist < 8
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-8.0),
        Node::Broadcast(1),
    ])
}

/// Herd: move toward the nearest same-species neighbor (cohesion).
pub fn starter_herd() -> Program {
    Program::from_slice(&[
        Node::SenseSameDirX,
        Node::MoveTowardX,
        Node::SenseSameDirY,
        Node::MoveTowardY,
    ])
}
```

Replace `starter_library` with:

```rust
/// Library of starter programs. Founders use index 0 (`starter_grazer`).
pub fn starter_library() -> &'static [fn() -> Program] {
    &[starter_grazer, starter_stalker, starter_pack_hunter, starter_sentinel, starter_herd]
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p anabios-core --lib program::tests`
Expected: PASS — including the three new starter tests.

- [ ] **Step 5: Verify the determinism guard still holds**

Run: `cargo test -p anabios-core --test determinism`
Expected: PASS unchanged. (`starter_library` is only indexed at 0 by `World::spawn_agent`, which still calls `starter_grazer()` directly, so default spawns are unaffected.)

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/program.rs
git commit -m "feat(core): M11 social starter programs (stalker/pack/sentinel/herd)"
```

---

## Task 6: End-to-end mechanism integration test

**Files:**
- Create: `crates/anabios-core/tests/social_substrate.rs`

- [ ] **Step 1: Write the integration test**

Create `crates/anabios-core/tests/social_substrate.rs`:

```rust
//! M11 mechanism tests: kin/threat sensing and action-intent plumbing end to
//! end through the public API.

use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude::Vec2;
use anabios_core::program::{Node, Program, NO_TARGET};
use anabios_core::tick::step;
use anabios_core::world::World;

/// A predator program that fires whenever an other-species agent is in range,
/// driven through the full sense -> decide pipeline.
#[test]
fn predator_program_produces_fire_intent_and_target() {
    let mut w = World::new(5);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let prey = w.spawn_agent(Vec2::new(503.0, 500.0), Genome::neutral());
    w.agents.species_id[prey as usize] = 1;
    // fire_intent = 1 when other_dist < 6
    w.agents.program[pred as usize] = Program::from_slice(&[
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-6.0),
        Node::FireWeapon,
    ]);
    step(&mut w);
    assert!(w.actions[pred as usize].fire_intent > 0.0);
    assert_eq!(w.actions[pred as usize].target_id, prey);
}

/// With no neighbor in range, the target resolves to NO_TARGET and intents are
/// zero.
#[test]
fn lone_agent_has_no_target_and_no_intents() {
    let mut w = World::new(5);
    let solo = w.spawn_agent(Vec2::new(100.0, 100.0), Genome::neutral());
    w.agents.program[solo as usize] = Program::from_slice(&[
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-6.0),
        Node::FireWeapon,
    ]);
    step(&mut w);
    let a = w.actions[solo as usize];
    assert_eq!(a.target_id, NO_TARGET);
    assert_eq!(a.fire_intent, 0.0); // other_dist is INFINITY -> condition false
}

/// Emit/broadcast intents reach the action buffer through the pipeline, on the
/// correct channels.
#[test]
fn emit_and_broadcast_intents_reach_action_buffer() {
    let mut w = World::new(5);
    let id = w.spawn_agent(Vec2::new(700.0, 700.0), Genome::neutral());
    w.agents.program[id as usize] = Program::from_slice(&[
        Node::Const(1.0),
        Node::EmitPheromone(0),
        Node::Const(1.0),
        Node::Broadcast(2),
    ]);
    step(&mut w);
    assert_eq!(w.actions[id as usize].emit_intent[0], 1.0);
    assert_eq!(w.actions[id as usize].broadcast_intent[2], 1.0);
}

/// Relative-size sensing drives a flee-from-bigger behavior through decide.
#[test]
fn relative_size_channel_is_visible_to_programs() {
    let mut w = World::new(5);
    let mut small = Genome::neutral();
    small.set(GenomeSlot::Size, 0.3);
    let mut big = Genome::neutral();
    big.set(GenomeSlot::Size, 1.0);
    let me = w.spawn_agent(Vec2::new(800.0, 800.0), small);
    let _bigger = w.spawn_agent(Vec2::new(804.0, 800.0), big);
    // move_x = rel_size (will be >1 because neighbor is bigger)
    w.agents.program[me as usize] =
        Program::from_slice(&[Node::SenseRelSize, Node::MoveTowardX]);
    step(&mut w);
    // desired_direction is normalized, so just assert it points +x (toward the
    // computed positive intent) — rel_size > 0 means a positive move_x.
    assert!(w.desired_direction[me as usize].x > 0.9);
}
```

- [ ] **Step 2: Run the integration tests to verify they pass**

Run: `cargo test -p anabios-core --test social_substrate`
Expected: PASS — all four tests. (They compile against the public API only; if any symbol is private, that's a signal to make it `pub` — `Node`, `Program`, `NO_TARGET`, `World.actions`, `World.desired_direction`, and `agents.program/species_id` must be public, which they are.)

- [ ] **Step 3: Commit**

```bash
git add crates/anabios-core/tests/social_substrate.rs
git commit -m "test(core): M11 mechanism tests for sensing + action plumbing"
```

---

## Task 7: Full verification + determinism guard

**Files:** none (verification only)

- [ ] **Step 1: Run the entire core test suite**

Run: `cargo test -p anabios-core`
Expected: PASS — every unit + integration test, including `determinism::minimal_scenario_matches_golden_hashes` with the **unchanged** committed hashes.

- [ ] **Step 2: Run clippy and fmt (CI parity, design §9.2)**

Run: `cargo clippy -p anabios-core --all-targets -- -D warnings`
Expected: no warnings.

Run: `cargo fmt --check`
Expected: no diff. (If it reports a diff, run `cargo fmt` and amend the relevant commit.)

- [ ] **Step 3: Confirm the headless determinism behavior is intact**

Run: `cargo build --release --bin anabios-headless && ./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 1000 --seed 42`
Expected: completes without error and prints summary metrics (no new events — M11 adds no detectors).

- [ ] **Step 4: Final commit (only if fmt/clippy required fixes)**

```bash
git add -A
git commit -m "chore(core): M11 fmt/clippy cleanup"
```

---

## Self-Review (completed during planning)

**Spec coverage (against design §3 M11 in `2026-06-21-collaboration-competition-design.md`):**
- "Extend `SensorRegister` + `sense.rs` (same/other species, relative size/energy, crowding)" → Task 1. ✅
- "Add matching live `Sense*` AST nodes" → Task 2. ✅
- "Extend `ActionRegister` (fire/emit/broadcast intents + resolved target)" → Tasks 3–4. ✅
- "Plumbed and stored in M11, consumed starting M12" → Task 4 stores into `world.actions`; nothing in M11 reads it. ✅
- "Expand the starter-program library (Stalker, PackHunter, Sentinel, Herd)" → Task 5. ✅
- "Mechanism tests: sense channels exact, action plumbing, starters parse/bounded/eval, golden-tick" → Tasks 1/5/6 (mechanism) + Task 7 (golden-tick, as a *guard* rather than a refresh — see deviation note). ✅
- "Detectors: none" → no detector tasks. ✅

**Deviation from spec:** §2.3 anticipated a golden-tick *refresh*; M11 is fully additive/hash-neutral, so the plan keeps the golden test as a passing guard instead (documented in the Determinism contract). Safe direction.

**Placeholder scan:** none — every code step is concrete. The `anabios_core_self` token in Task 4 Step 1 is explicitly flagged as a reminder to use `crate::`.

**Type consistency:** `ActionRegister` fields (`fire_intent`, `emit_intent[PHEROMONE_CHANNELS]`, `broadcast_intent[MEME_CHANNELS]`, `target_id`), sentinels (`NO_NEIGHBOR_ID`, `NO_TARGET`), and `EvalContext`/`SensorRegister` field names are used identically across Tasks 1–6.
