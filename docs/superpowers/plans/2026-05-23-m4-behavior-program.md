# M4 — Evolvable Behavior Program Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hardcoded forage/wander/mate-seek function in `behavior.rs` with an evolvable expression tree per agent. Each agent owns a small program (5-40 nodes, hard cap 64) that reads sensors and the genome, computes an action register each tick, and inherits + mutates during reproduction. After M4 the simulation's behavior is genuinely emergent — different lineages discover different strategies through evolution rather than the engine prescribing them.

**Architecture:** New `program.rs` holds a `Program` struct (a `SmallVec` of `Node`s in postfix evaluation order, with each node referencing its arguments by index). The evaluator runs a stack machine (no recursion → no stack overflow on weird mutants, and the eval is allocation-free in the hot loop). Mutation operators: per-node point mutation, subtree-replace, insert, delete, and crossover. A `starter_library()` returns a small set of canned programs (`Grazer`, `Wanderer`, `Stalker`) used by founders and `random_any` mutation.

**Crucially, M4 does NOT introduce new world state** — `SenseMeme`, `EmitPheromone`, `Broadcast` nodes are present in the AST grammar so structural mutation can produce them, but their evaluator outputs are no-ops until M5+ wires the underlying systems. Mutation will still favor agents whose programs use the *implemented* sensor/action subset.

**Tech Stack:** Same as M3.

**Style conventions** (inherited):

- 4-space indent
- All randomness through `World.rng`
- No allocations in tick path; program eval uses a `Vec<f32>` scratch in `World`
- Deterministic iteration (ascending agent id)
- Conventional Commits prefixes
- Single commit per task unless noted

**Branch:** `m4-behavior-program` branched from `main`.

**Working directory:** `/Users/aryasen/projects/anabios/`.

---

## File structure after M4

New files:
```
crates/anabios-core/src/
└── program.rs                     # Node enum + Program + evaluator + mutation + starter library
crates/anabios-core/tests/
└── program_evolution.rs           # integration: evolved programs survive past hardcoded baseline
```

Modified files:
```
crates/anabios-core/src/
├── agent.rs                       # +program: Vec<Program>; spawn signature
├── world.rs                       # +eval_stack scratch buffer
├── behavior.rs                    # decide() delegates to program evaluator
├── reproduce.rs                   # crossover+mutate program during birth
├── scenario.rs                    # founders get starter Grazer program
├── tick.rs                        # decide_all wired to program evaluation
└── lib.rs                         # +pub mod program;
crates/anabios-core/tests/
├── determinism.rs                 # regenerate GOLDEN
└── invariants.rs                  # +program-size invariants
```

---

## Task 0: Branch + workspace prep

**Goal:** Create `m4-behavior-program` branch from `main`. No code changes.

- [ ] **Step 0.1: Branch**

```bash
git checkout main
git pull
git checkout -b m4-behavior-program
```

- [ ] **Step 0.2: Verify clean state**

```bash
cargo test --workspace 2>&1 | tail -3
```

Expected: 102 tests pass (M3 baseline).

No commit.

---

## Task 1: Program data structure

**Goal:** Define the `Node` enum (sensors, operators, outputs), the `Program` struct (postfix `SmallVec`), and constants. Pure data + size validation, no evaluator yet.

**Files:**
- Create: `crates/anabios-core/src/program.rs`
- Modify: `crates/anabios-core/src/lib.rs` (add `pub mod program;`, re-export `Program`)

- [ ] **Step 1.1: Add module declaration**

In `crates/anabios-core/src/lib.rs`, insert `pub mod program;` after `pub mod module;`. Add `pub use program::Program;` at the bottom of the re-exports.

- [ ] **Step 1.2: Implement program.rs**

Create `crates/anabios-core/src/program.rs`:

```rust
//! Evolvable behavior program.
//!
//! Each agent owns a `Program` — a small expression tree stored in postfix
//! order. Evaluation runs a stack machine: leaves push values, operators pop
//! and push, output nodes pop and write to an `ActionRegister`. No recursion
//! anywhere, so weird mutants cannot blow the stack.
//!
//! Nodes that reference world systems not yet implemented (pheromone field,
//! meme transmission) evaluate to zero / no-op in M4 but remain in the
//! grammar so structural mutation can produce them. Later milestones wire
//! them up by editing only the evaluator.

use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};

use crate::genome::{GenomeSlot, GENOME_LEN};
use crate::rng::Rng;

/// Hard cap on program node count. Programs exceeding this are truncated.
pub const PROGRAM_MAX_NODES: usize = 64;
/// Typical bounded inline storage for the `SmallVec`.
pub const PROGRAM_INLINE: usize = 16;

/// Mutation probabilities applied during reproduction. Tuned to give a few
/// nodes per generation a chance to change without dissolving viable
/// strategies between generations.
pub const POINT_MUTATE_PROB: f32 = 0.04;
pub const INSERT_NODE_PROB: f32 = 0.03;
pub const DELETE_NODE_PROB: f32 = 0.03;
pub const SUBTREE_REPLACE_PROB: f32 = 0.02;

/// Sigma for Gaussian perturbation of `Const(f32)` values.
pub const CONST_SIGMA: f32 = 0.1;

/// AST node. Operators reference operands by negative offset into the
/// stack at eval time, so the same `Program` struct works for any tree
/// topology without explicit child indices.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Node {
    // Inputs — push one value onto the stack
    SenseEnergy,
    SenseAge,
    SenseGenome(u8),                 // slot index 0..GENOME_LEN
    SenseNearestDistance,
    SenseNearestDirX,
    SenseNearestDirY,
    SensePlantDirX,
    SensePlantDirY,
    SenseLocalBiomass,
    SenseMeme(u8),                   // M5+; evaluates to 0.0 in M4
    Const(f32),                      // literal in [-1, 1]

    // Operators — pop N, push 1
    Add,                             // pop 2, push sum
    Sub,                             // pop 2 (b, a), push a - b
    Mul,                             // pop 2, push product
    Min,                             // pop 2, push min
    Max,                             // pop 2, push max
    Neg,                             // pop 1, push negation
    Tanh,                            // pop 1, push tanh (squash to [-1,1])
    ThresholdGt(f32),                // pop 1, push 1.0 if > threshold else 0.0
    IfThenElse,                      // pop 3 (else, then, cond); push then if cond > 0 else else
    Lerp,                            // pop 3 (b, a, t); push a + (b-a)*t.clamp(0,1)

    // Outputs — pop 1, write to action register, push 0.0 (no value)
    MoveTowardX,                     // pop 1; add to action.move_x
    MoveTowardY,                     // pop 1; add to action.move_y
    MoveAwayX,                       // pop 1; subtract from action.move_x
    MoveAwayY,                       // pop 1; subtract from action.move_y
    Feed,                            // pop 1; add to action.feed_intent
    Mate,                            // pop 1; add to action.mate_intent
    FireWeapon,                      // pop 1; M5+; no-op in M4
    EmitPheromone(u8),               // pop 1; M5+; no-op in M4 (channel 0..3)
    Broadcast(u8),                   // pop 1, meme slot 0..7; M5+; no-op
    Idle,                            // pop 1, discard
}

/// What an agent wants to do this tick, produced by the evaluator.
#[derive(Debug, Clone, Copy, Default)]
pub struct ActionRegister {
    pub move_x: f32,
    pub move_y: f32,
    pub feed_intent: f32,
    pub mate_intent: f32,
}

/// One agent's behavior program. Nodes are stored in evaluation order
/// (postfix); evaluating left-to-right with a value stack yields the same
/// result a recursive tree walk would.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub nodes: SmallVec<[Node; PROGRAM_INLINE]>,
}

impl Program {
    /// Empty program — evaluator returns a zeroed action register.
    pub fn empty() -> Self {
        Self { nodes: SmallVec::new() }
    }

    /// Construct from a node slice. Truncates to `PROGRAM_MAX_NODES`.
    pub fn from_slice(nodes: &[Node]) -> Self {
        let mut sv: SmallVec<[Node; PROGRAM_INLINE]> = SmallVec::new();
        for &n in nodes.iter().take(PROGRAM_MAX_NODES) {
            sv.push(n);
        }
        Self { nodes: sv }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Total inputs a node consumes from the eval stack.
    pub fn arity(node: Node) -> usize {
        match node {
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
            | Node::Const(_) => 0,
            Node::Add | Node::Sub | Node::Mul | Node::Min | Node::Max => 2,
            Node::Neg | Node::Tanh | Node::ThresholdGt(_) => 1,
            Node::IfThenElse | Node::Lerp => 3,
            Node::MoveTowardX
            | Node::MoveTowardY
            | Node::MoveAwayX
            | Node::MoveAwayY
            | Node::Feed
            | Node::Mate
            | Node::FireWeapon
            | Node::EmitPheromone(_)
            | Node::Broadcast(_)
            | Node::Idle => 1,
        }
    }

    /// Whether the node writes to the action register (and discards the
    /// popped value, pushing nothing).
    pub fn is_output(node: Node) -> bool {
        matches!(
            node,
            Node::MoveTowardX | Node::MoveTowardY | Node::MoveAwayX | Node::MoveAwayY
                | Node::Feed | Node::Mate | Node::FireWeapon
                | Node::EmitPheromone(_) | Node::Broadcast(_) | Node::Idle
        )
    }
}

/// One canned starter program: a basic herbivore that moves toward
/// plants when its energy is low.
pub fn starter_grazer() -> Program {
    // Postfix: SenseEnergy Const(20) ThresholdGt(0) [hungry?]
    //         SensePlantDirX  IfThenElse [steer x toward plant if hungry, else 0]
    //         MoveTowardX
    //         SenseEnergy Const(20) ThresholdGt(0)
    //         SensePlantDirY  IfThenElse
    //         MoveTowardY
    //         SenseEnergy Const(35) ThresholdGt(0)  [well-fed?]
    //         Mate
    Program::from_slice(&[
        Node::SenseEnergy, Node::Const(20.0), Node::ThresholdGt(0.0),
        Node::Const(0.0), Node::SensePlantDirX, Node::IfThenElse,
        Node::MoveTowardX,

        Node::SenseEnergy, Node::Const(20.0), Node::ThresholdGt(0.0),
        Node::Const(0.0), Node::SensePlantDirY, Node::IfThenElse,
        Node::MoveTowardY,

        Node::SenseEnergy, Node::Const(35.0), Node::ThresholdGt(0.0),
        Node::Mate,
    ])
}

/// Returns the small fixed library of starter programs. Founders use index 0
/// (the grazer); random structural mutation can produce variants over time.
pub fn starter_library() -> &'static [fn() -> Program] {
    // function-pointer slice so callers can spawn fresh copies cheaply
    &[starter_grazer]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_grazer_is_bounded() {
        let g = starter_grazer();
        assert!(g.len() > 0);
        assert!(g.len() <= PROGRAM_MAX_NODES);
    }

    #[test]
    fn arity_matches_eval_contract() {
        // Sample a few:
        assert_eq!(Program::arity(Node::Const(0.0)), 0);
        assert_eq!(Program::arity(Node::Add), 2);
        assert_eq!(Program::arity(Node::IfThenElse), 3);
        assert_eq!(Program::arity(Node::MoveTowardX), 1);
    }

    #[test]
    fn is_output_matches_action_nodes() {
        assert!(Program::is_output(Node::MoveTowardX));
        assert!(Program::is_output(Node::Mate));
        assert!(Program::is_output(Node::Idle));
        assert!(!Program::is_output(Node::Const(0.0)));
        assert!(!Program::is_output(Node::Add));
    }

    #[test]
    fn from_slice_truncates_to_max() {
        let huge = vec![Node::Const(0.0); 200];
        let p = Program::from_slice(&huge);
        assert_eq!(p.len(), PROGRAM_MAX_NODES);
    }
}
```

- [ ] **Step 1.3: Test + commit**

```bash
cargo test -p anabios-core program
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/lib.rs crates/anabios-core/src/program.rs
git commit -m "feat(core): Program AST (Node enum, postfix layout, starter library)"
```

Expected: 4 program tests pass.

---

## Task 2: Evaluator

**Goal:** A pure function `evaluate(program, ctx, scratch_stack) -> ActionRegister`. Uses a caller-provided `Vec<f32>` to avoid per-tick allocations. Skips operators whose arity exceeds the current stack depth (defensive against malformed mutants).

**Files:**
- Modify: `crates/anabios-core/src/program.rs`

- [ ] **Step 2.1: Add EvalContext and evaluate**

Append to `program.rs` just before `#[cfg(test)] mod tests`:

```rust
/// Per-agent inputs the evaluator reads. Caller fills this from the
/// agent's sensor register + genome each tick. Cheap to construct.
#[derive(Debug, Clone, Copy)]
pub struct EvalContext<'a> {
    pub energy: f32,
    pub age: u32,
    pub genome: &'a crate::genome::Genome,
    pub nearest_distance: f32,
    pub nearest_dir: glam::Vec2,
    pub plant_dir: glam::Vec2,
    pub local_biomass: f32,
}

/// Evaluate `program` against `ctx`. Returns the populated action register.
/// `scratch` is the value stack — caller owns and reuses it across agents.
pub fn evaluate(program: &Program, ctx: EvalContext, scratch: &mut Vec<f32>) -> ActionRegister {
    scratch.clear();
    let mut action = ActionRegister::default();

    for node in program.nodes.iter().copied() {
        let arity = Program::arity(node);
        if scratch.len() < arity {
            // Underflow — skip the node. Keeps malformed mutants safe.
            continue;
        }
        match node {
            Node::SenseEnergy => scratch.push(ctx.energy),
            Node::SenseAge => scratch.push(ctx.age as f32),
            Node::SenseGenome(slot) => {
                let s = (slot as usize).min(GENOME_LEN - 1);
                let g_slot = match s {
                    0 => GenomeSlot::Size,
                    _ => {
                        // Read raw genome via index — safe because we clamped.
                        scratch.push(ctx.genome.0[s]);
                        continue;
                    }
                };
                scratch.push(ctx.genome.get(g_slot));
            }
            Node::SenseNearestDistance => scratch.push(ctx.nearest_distance.min(1e6)),
            Node::SenseNearestDirX => scratch.push(ctx.nearest_dir.x),
            Node::SenseNearestDirY => scratch.push(ctx.nearest_dir.y),
            Node::SensePlantDirX => scratch.push(ctx.plant_dir.x),
            Node::SensePlantDirY => scratch.push(ctx.plant_dir.y),
            Node::SenseLocalBiomass => scratch.push(ctx.local_biomass),
            Node::SenseMeme(_) => scratch.push(0.0),
            Node::Const(v) => scratch.push(v),

            Node::Add => {
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                scratch.push(a + b);
            }
            Node::Sub => {
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                scratch.push(a - b);
            }
            Node::Mul => {
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                scratch.push(a * b);
            }
            Node::Min => {
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                scratch.push(a.min(b));
            }
            Node::Max => {
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                scratch.push(a.max(b));
            }
            Node::Neg => {
                let a = scratch.pop().unwrap();
                scratch.push(-a);
            }
            Node::Tanh => {
                let a = scratch.pop().unwrap();
                // Use a libm-backed tanh approximation: tanh(x) = (e^2x - 1) / (e^2x + 1).
                let e2x = crate::mathf::expf(2.0 * a);
                scratch.push((e2x - 1.0) / (e2x + 1.0));
            }
            Node::ThresholdGt(thr) => {
                let a = scratch.pop().unwrap();
                scratch.push(if a > thr { 1.0 } else { 0.0 });
            }
            Node::IfThenElse => {
                let else_v = scratch.pop().unwrap();
                let then_v = scratch.pop().unwrap();
                let cond = scratch.pop().unwrap();
                scratch.push(if cond > 0.0 { then_v } else { else_v });
            }
            Node::Lerp => {
                let b = scratch.pop().unwrap();
                let a = scratch.pop().unwrap();
                let t = scratch.pop().unwrap().clamp(0.0, 1.0);
                scratch.push(a + (b - a) * t);
            }

            Node::MoveTowardX => {
                action.move_x += scratch.pop().unwrap();
            }
            Node::MoveTowardY => {
                action.move_y += scratch.pop().unwrap();
            }
            Node::MoveAwayX => {
                action.move_x -= scratch.pop().unwrap();
            }
            Node::MoveAwayY => {
                action.move_y -= scratch.pop().unwrap();
            }
            Node::Feed => {
                action.feed_intent += scratch.pop().unwrap();
            }
            Node::Mate => {
                action.mate_intent += scratch.pop().unwrap();
            }
            Node::FireWeapon | Node::EmitPheromone(_) | Node::Broadcast(_) | Node::Idle => {
                scratch.pop();
            }
        }
    }

    action
}
```

- [ ] **Step 2.2: Add evaluator tests**

Append to `program.rs::tests`:

```rust
    use crate::genome::Genome;

    fn dummy_ctx<'a>(genome: &'a Genome) -> EvalContext<'a> {
        EvalContext {
            energy: 30.0,
            age: 100,
            genome,
            nearest_distance: 5.0,
            nearest_dir: glam::Vec2::new(1.0, 0.0),
            plant_dir: glam::Vec2::new(0.0, 1.0),
            local_biomass: 8.0,
        }
    }

    #[test]
    fn empty_program_yields_zero_action() {
        let p = Program::empty();
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert_eq!(a.move_x, 0.0);
        assert_eq!(a.move_y, 0.0);
        assert_eq!(a.feed_intent, 0.0);
        assert_eq!(a.mate_intent, 0.0);
    }

    #[test]
    fn const_plus_move_writes_action_register() {
        let p = Program::from_slice(&[Node::Const(1.0), Node::MoveTowardX]);
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert_eq!(a.move_x, 1.0);
    }

    #[test]
    fn arithmetic_chain_works() {
        // Compute 1 + 2 * 3 = 7, then MoveTowardX of that.
        let p = Program::from_slice(&[
            Node::Const(1.0),
            Node::Const(2.0),
            Node::Const(3.0),
            Node::Mul,
            Node::Add,
            Node::MoveTowardX,
        ]);
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let a = evaluate(&p, dummy_ctx(&g), &mut stack);
        assert_eq!(a.move_x, 7.0);
    }

    #[test]
    fn underflow_is_safe() {
        // Add with empty stack — should not panic.
        let p = Program::from_slice(&[Node::Add, Node::MoveTowardX]);
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let _a = evaluate(&p, dummy_ctx(&g), &mut stack);
    }

    #[test]
    fn grazer_steers_toward_plant_when_hungry() {
        let p = starter_grazer();
        let g = Genome::neutral();
        let mut stack = Vec::new();
        // Energy below 20 → hungry → steer toward plant_dir.
        let ctx = EvalContext { energy: 5.0, ..dummy_ctx(&g) };
        let a = evaluate(&p, ctx, &mut stack);
        assert!(a.move_y > 0.0, "hungry grazer should head +y toward plant: {:?}", a);
    }

    #[test]
    fn grazer_mates_when_well_fed() {
        let p = starter_grazer();
        let g = Genome::neutral();
        let mut stack = Vec::new();
        let ctx = EvalContext { energy: 50.0, ..dummy_ctx(&g) };
        let a = evaluate(&p, ctx, &mut stack);
        assert!(a.mate_intent > 0.0);
    }
```

- [ ] **Step 2.3: Test + commit**

```bash
cargo test -p anabios-core program
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/program.rs
git commit -m "feat(core): program evaluator (stack machine, underflow-safe, action register)"
```

Expected: 10 program tests pass.

---

## Task 3: Per-agent program field

**Goal:** Add `program: Vec<Program>` to `AgentBuffers`. Extend `spawn` to take a `Program`. Founders get `starter_grazer()`.

**Files:**
- Modify: `crates/anabios-core/src/agent.rs`
- Modify: `crates/anabios-core/src/world.rs`
- Modify: `crates/anabios-core/src/reproduce.rs` (placeholder: inherit parent A's program; Task 5 swaps in crossover)

- [ ] **Step 3.1: Extend AgentBuffers**

Add to imports in `agent.rs`: `use crate::program::Program;`. Add field `pub program: Vec<Program>` between `modules` and `alive`:

```rust
    pub modules: Vec<ModuleList>,
    pub program: Vec<Program>,
    pub alive: BitVec,
```

- [ ] **Step 3.2: Extend spawn signature**

Replace `spawn` to take an additional `program: Program` parameter and store it. For both the reuse-slot and push branches:

- reuse: `self.program[i] = program;`
- push: `self.program.push(program);`

Update the 5 `agent::tests` to pass `Program::empty()` as the new argument.

- [ ] **Step 3.3: Update World::spawn_agent**

In `world.rs`, change `spawn_agent` to:

```rust
    pub fn spawn_agent(&mut self, position: Vec2, genome: Genome) -> AgentId {
        let lineage = self.next_lineage();
        let id = self.agents.spawn(
            position,
            genome,
            lineage,
            [LINEAGE_NONE; 2],
            0,
            crate::module::starter_kit(),
            crate::program::starter_grazer(),
        );
        self.add_to_species(0);
        id
    }
```

- [ ] **Step 3.4: Reproduce placeholder**

In `reproduce.rs`, where the child is spawned, pass `world.agents.program[i].clone()` for now (Task 5 replaces with crossover+mutate):

```rust
        let child_program = world.agents.program[i].clone();
        let lineage = world.next_lineage();
        let child_id = world.agents.spawn(
            child_pos, child_genome, lineage, [a_lineage, b_lineage],
            a_species, child_modules, child_program,
        );
        world.add_to_species(a_species);
```

- [ ] **Step 3.5: Test + commit**

```bash
cargo test -p anabios-core --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/agent.rs crates/anabios-core/src/world.rs crates/anabios-core/src/reproduce.rs
git commit -m "feat(core): add Program field to AgentBuffers; founders get starter_grazer"
```

Expected: every lib test still passes (program present but `decide` still uses the M1 hardcoded function for now).

---

## Task 4: Wire decide_all to the program evaluator

**Goal:** Replace `behavior.rs::decide` with one that calls `program::evaluate` and converts the action register to a unit direction. The hardcoded forage/wander/mate function is removed.

**Files:**
- Modify: `crates/anabios-core/src/behavior.rs`
- Modify: `crates/anabios-core/src/tick.rs`
- Modify: `crates/anabios-core/src/world.rs` (add `eval_stack: Vec<f32>` scratch)

- [ ] **Step 4.1: Add eval_stack scratch to World**

In `world.rs`, add to the World struct:

```rust
    #[serde(skip)]
    pub eval_stack: Vec<f32>,
```

Initialize `eval_stack: Vec::new()` in `World::new`.

- [ ] **Step 4.2: New decide that delegates to evaluate**

Replace the body of `behavior.rs::decide`:

```rust
//! Behavior dispatch — delegates to each agent's evolvable program.

use crate::program::{evaluate, ActionRegister, EvalContext, Program};
use crate::sense::SensorRegister;
use crate::prelude::Vec2;

/// Run the program for one agent. Returns the desired unit direction (or
/// zero if the program asked to be still). `eval_stack` is the world's
/// scratch buffer.
pub fn decide(
    program: &Program,
    genome: &crate::genome::Genome,
    sensor: &SensorRegister,
    energy: f32,
    age: u32,
    eval_stack: &mut Vec<f32>,
) -> Vec2 {
    let ctx = EvalContext {
        energy,
        age,
        genome,
        nearest_distance: sensor.nearest_neighbor_dist,
        nearest_dir: sensor.nearest_neighbor_dir,
        plant_dir: sensor.plant_direction,
        local_biomass: sensor.local_plant_biomass,
    };
    let ActionRegister { move_x, move_y, .. } = evaluate(program, ctx, eval_stack);

    let v = Vec2::new(move_x, move_y);
    let len = v.length();
    if len < 1e-4 {
        Vec2::ZERO
    } else {
        v / len
    }
}
```

Delete the test module from `behavior.rs` (every test relied on the M1/M2 hardcoded behavior; equivalent coverage is in `program.rs` tests).

- [ ] **Step 4.3: Update tick.rs::decide_all**

```rust
fn decide_all(world: &mut World) {
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
        let dir = decide(
            &world.agents.program[i],
            &world.agents.genome[i],
            &world.sensors[i],
            world.agents.energy[i],
            world.agents.age[i],
            &mut world.eval_stack,
        );
        world.desired_direction[i] = dir;
    }
}
```

- [ ] **Step 4.4: Test + commit**

```bash
cargo test -p anabios-core --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/behavior.rs crates/anabios-core/src/tick.rs crates/anabios-core/src/world.rs
git commit -m "feat(core): decide_all delegates to per-agent program evaluator"
```

Expected: lib tests still pass (founders all start with the grazer program, which has equivalent intent to the M2 forage/mate behavior).

---

## Task 5: Mutation operators

**Goal:** Implement `mutate_program` (in-place perturbation) and `crossover_programs` (two-parent → child) in `program.rs`. Truncate to `PROGRAM_MAX_NODES` after each op.

**Files:**
- Modify: `crates/anabios-core/src/program.rs`

- [ ] **Step 5.1: Append mutation functions**

In `program.rs`, before `#[cfg(test)]`:

```rust
/// Random node, drawn from the full grammar (used by structural mutation).
pub fn random_node(rng: &mut Rng) -> Node {
    match rng.index(20) {
        0 => Node::SenseEnergy,
        1 => Node::SenseAge,
        2 => Node::SenseGenome((rng.index(GENOME_LEN)) as u8),
        3 => Node::SenseNearestDistance,
        4 => Node::SenseNearestDirX,
        5 => Node::SenseNearestDirY,
        6 => Node::SensePlantDirX,
        7 => Node::SensePlantDirY,
        8 => Node::SenseLocalBiomass,
        9 => Node::Const(rng.f32_range(-1.0, 1.0)),
        10 => Node::Add,
        11 => Node::Sub,
        12 => Node::Mul,
        13 => Node::Max,
        14 => Node::Tanh,
        15 => Node::IfThenElse,
        16 => Node::MoveTowardX,
        17 => Node::MoveTowardY,
        18 => Node::Feed,
        _ => Node::Mate,
    }
}

/// Perturb each node with probability `POINT_MUTATE_PROB`. Const values
/// get a Gaussian nudge; structural nodes get replaced with a random node.
pub fn point_mutate(program: &mut Program, rng: &mut Rng) {
    for node in program.nodes.iter_mut() {
        if rng.f32_unit() >= POINT_MUTATE_PROB {
            continue;
        }
        *node = match *node {
            Node::Const(v) => Node::Const((v + rng.gaussian(0.0, CONST_SIGMA)).clamp(-2.0, 2.0)),
            Node::ThresholdGt(v) => {
                Node::ThresholdGt((v + rng.gaussian(0.0, CONST_SIGMA)).clamp(-2.0, 2.0))
            }
            _ => random_node(rng),
        };
    }
}

/// Structural mutation — insert / delete / subtree-replace one node each
/// with the corresponding probability. Bounded to `[1, PROGRAM_MAX_NODES]`.
pub fn structural_mutate(program: &mut Program, rng: &mut Rng) {
    if program.nodes.len() < PROGRAM_MAX_NODES && rng.f32_unit() < INSERT_NODE_PROB {
        let pos = rng.index(program.nodes.len().max(1));
        program.nodes.insert(pos.min(program.nodes.len()), random_node(rng));
    }
    if program.nodes.len() > 1 && rng.f32_unit() < DELETE_NODE_PROB {
        let pos = rng.index(program.nodes.len());
        program.nodes.remove(pos);
    }
    if !program.nodes.is_empty() && rng.f32_unit() < SUBTREE_REPLACE_PROB {
        let pos = rng.index(program.nodes.len());
        program.nodes[pos] = random_node(rng);
    }
    while program.nodes.len() > PROGRAM_MAX_NODES {
        program.nodes.pop();
    }
}

/// Single-point crossover: pick a split index, take parent A's prefix and
/// parent B's suffix. Then apply point + structural mutation.
pub fn crossover_and_mutate(a: &Program, b: &Program, rng: &mut Rng) -> Program {
    let max_len = a.len().max(b.len());
    let split = if max_len == 0 { 0 } else { rng.index(max_len + 1) };
    let mut nodes: SmallVec<[Node; PROGRAM_INLINE]> = SmallVec::new();
    for &n in a.nodes.iter().take(split) {
        if nodes.len() < PROGRAM_MAX_NODES {
            nodes.push(n);
        }
    }
    for &n in b.nodes.iter().skip(split) {
        if nodes.len() < PROGRAM_MAX_NODES {
            nodes.push(n);
        }
    }
    let mut child = Program { nodes };
    point_mutate(&mut child, rng);
    structural_mutate(&mut child, rng);
    child
}
```

- [ ] **Step 5.2: Tests**

```rust
    #[test]
    fn point_mutate_preserves_length() {
        let mut rng = Rng::from_seed(7);
        let mut p = starter_grazer();
        let len = p.len();
        for _ in 0..50 {
            point_mutate(&mut p, &mut rng);
        }
        assert_eq!(p.len(), len);
    }

    #[test]
    fn structural_mutate_stays_in_bounds() {
        let mut rng = Rng::from_seed(11);
        let mut p = starter_grazer();
        for _ in 0..1000 {
            structural_mutate(&mut p, &mut rng);
            assert!(!p.is_empty());
            assert!(p.len() <= PROGRAM_MAX_NODES);
        }
    }

    #[test]
    fn crossover_with_identical_parents_stays_in_bounds() {
        let mut rng = Rng::from_seed(13);
        let p = starter_grazer();
        for _ in 0..200 {
            let c = crossover_and_mutate(&p, &p, &mut rng);
            assert!(c.len() <= PROGRAM_MAX_NODES);
        }
    }

    #[test]
    fn crossover_is_deterministic() {
        let p = starter_grazer();
        let mut r1 = Rng::from_seed(99);
        let mut r2 = Rng::from_seed(99);
        let c1 = crossover_and_mutate(&p, &p, &mut r1);
        let c2 = crossover_and_mutate(&p, &p, &mut r2);
        assert_eq!(c1, c2);
    }
```

- [ ] **Step 5.3: Test + commit**

```bash
cargo test -p anabios-core program
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/program.rs
git commit -m "feat(core): program mutation operators (point, structural, crossover)"
```

Expected: 14 program tests pass.

---

## Task 6: Reproduction inherits + mutates program

**Goal:** Replace the parent-A-clone placeholder in `reproduce.rs` with proper crossover_and_mutate.

**Files:**
- Modify: `crates/anabios-core/src/reproduce.rs`

- [ ] **Step 6.1: Swap in crossover**

```rust
        let a_program = world.agents.program[i].clone();
        let b_program = world.agents.program[j].clone();
        let child_program =
            crate::program::crossover_and_mutate(&a_program, &b_program, &mut world.rng);

        let lineage = world.next_lineage();
        let child_id = world.agents.spawn(
            child_pos, child_genome, lineage, [a_lineage, b_lineage],
            a_species, child_modules, child_program,
        );
        world.add_to_species(a_species);
```

- [ ] **Step 6.2: Test + commit**

```bash
cargo test -p anabios-core --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/reproduce.rs
git commit -m "feat(core): reproduce inherits + mutates parent programs via crossover"
```

---

## Task 7: Regenerate golden hashes + program invariants

**Goal:** Programs change snapshot shape → hashes change. Plus 2 new proptest invariants.

**Files:**
- Modify: `crates/anabios-core/tests/determinism.rs`
- Modify: `crates/anabios-core/tests/invariants.rs`

- [ ] **Step 7.1: Reset GOLDEN to zeros and regenerate**

```rust
const GOLDEN: &[(u64, u64)] =
    &[(0, 0x0000000000000000), (100, 0x0000000000000000), (1000, 0x0000000000000000)];
```

```bash
UPDATE_HASHES=1 cargo test -p anabios-core --test determinism -- --nocapture
```

Paste the printed values into GOLDEN; verify with `cargo test -p anabios-core --test determinism`.

- [ ] **Step 7.2: Program invariants**

Append to `invariants.rs`:

```rust
    #[test]
    fn alive_agents_have_program_within_bounds(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for id in w.agents.iter_alive() {
            let len = w.agents.program[id as usize].len();
            prop_assert!(len <= anabios_core::program::PROGRAM_MAX_NODES,
                "agent {id} program too long: {len}");
        }
    }
```

- [ ] **Step 7.3: Test + commit**

```bash
cargo test -p anabios-core --tests
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/tests/determinism.rs crates/anabios-core/tests/invariants.rs
git commit -m "test(core): program invariants + regenerated golden hashes for M4"
```

---

## Task 8: Program evolution integration test

**Goal:** Long-run test: starts with all-grazer founders, runs for 5,000 ticks, asserts at least one alive agent has a program that differs from the starter (proves crossover/mutation drift is happening).

**Files:**
- Create: `crates/anabios-core/tests/program_evolution.rs`

- [ ] **Step 8.1: Write the test**

```rust
//! Integration test: program mutation drifts over generations.

use anabios_core::program::{starter_grazer, Program};
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

#[test]
fn at_least_one_program_diverges_from_starter_within_5000_ticks() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    let starter: Program = starter_grazer();

    for _ in 0..5_000 {
        step(&mut world);
        let any_divergent = world.agents.iter_alive().any(|id| {
            world.agents.program[id as usize] != starter
        });
        if any_divergent {
            return;
        }
    }
    panic!("no program diverged from the starter in 5000 ticks");
}
```

- [ ] **Step 8.2: Test + commit**

```bash
cargo test -p anabios-core --test program_evolution
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/tests/program_evolution.rs
git commit -m "test(core): integration test that program divergence happens within 5000 ticks"
```

---

## Task 9: Bench + final + tag

- [ ] **Step 9.1: Bench**

```bash
cargo bench -p anabios-core --bench tick_bench
```

Record 1k and 10k medians. M3: 1k = 1.00 ms / 10k = 7.00 ms. M4 adds program evaluation per agent per tick — expected +20-30% (1k ≈ 1.2-1.3 ms, 10k ≈ 9-10 ms). If 10k exceeds 15 ms, profile.

- [ ] **Step 9.2: Full check + smoke + tag**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cargo build --release --bin anabios-headless
./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 5000 > /tmp/m4_a.txt
./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 5000 > /tmp/m4_b.txt
diff /tmp/m4_a.txt /tmp/m4_b.txt && echo deterministic

git tag -a m4 -m "M4: evolvable behavior program"
```

Do NOT push branch/tag — controller handles that.

---

## Post-implementation expectations

After M4:

- The hardcoded `decide()` is gone; agents act according to their evolved program
- Founders start with the `Grazer` program; offspring inherit + mutate
- Programs diverge across lineages within thousands of ticks
- Mutation can introduce nodes referencing world systems not yet present (`SenseMeme`, `EmitPheromone`); they evaluate to zero / no-op in M4 and become live in M5+
- Determinism preserved; golden hashes regenerated; bench stays in budget

Deferred to M5+:

- Codex detectors (the meta-game)
- Pheromone field
- Culture / meme transmission
- Combat (Weapon module effects)
- Godot rendering
