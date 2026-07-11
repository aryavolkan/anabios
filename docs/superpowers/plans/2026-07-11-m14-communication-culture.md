# M14 — Communication & Culture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make memes real — a per-agent `meme_vector`, a `culture_step` that transmits meme values between `Communicator`-equipped neighbors with imperfect copy, `SenseMeme`/`Broadcast` wiring, meme inheritance on reproduction — and add the `DialectFormed` / `MemeSweep` / `AlarmCall` detectors: the first cultural/information-flow emergence in anabios.

**Architecture:** A serialized `AgentBuffers.meme_vector: Vec<[f32; MEME_CHANNELS]>` holds each agent's cultural state. In `decide`, `SenseMeme(ch)` reads the agent's own `meme_vector[ch]`; `Broadcast(ch)` (already wired) writes to `ActionRegister.broadcast_intent[ch]`. A new `culture_step` tick stage (after reproduce) has each `Communicator` agent blend its `meme_vector` toward the mean of nearby `Communicator` neighbors' `broadcast_intent` (a deterministic lerp — the "imperfect copy"). On reproduction, a child's `meme_vector` = the parents' average + RNG jitter (only when the child has a `Communicator`). Three detectors (mirroring the M12/M13 per-species-history pattern) recognize dialects, meme sweeps, and alarm→flee correlation. **All meme operations are gated on the `Communicator` module**, so `minimal.toml` (no communicators) is behaviorally unchanged and the golden-tick refresh is limited to the one buffer-layout change.

**Tech Stack:** Rust (`anabios-core` pure-sim crate, `anabios-headless` CLI), `glam::Vec2`, `serde`/`bincode` snapshots, `BTreeMap`/`VecDeque` detector state, single `Xoshiro256++` RNG (`world.rng`).

## Global Constraints

- **Determinism (design §7.2):** all tick/detector iteration is id-ordered or over `BTreeMap`/`BTreeSet`/`VecDeque`; **no `HashMap`** in tick/detector paths; no unordered float reductions; RNG draws (meme inheritance jitter) go through `world.rng` in ascending-id order only. `culture_step` transmission uses **no RNG** (deterministic lerp).
- **`EventType` variants appended at the END, in order:** `DialectFormed = 11`, `MemeSweep = 12`, `AlarmCall = 13` (current tail `NichePartitioning = 10`). bincode encodes by positional index — never insert mid-enum.
- **No new `Node` variants:** `SenseMeme(u8)` and `Broadcast(u8)` already exist; M14 only changes `SenseMeme`'s evaluation (was hardcoded `0.0`). No `random_node` grammar change (both are already excluded).
- **Meme ops gated on `Communicator`:** transmission and inheritance jitter run only for agents with a `Communicator` module. This keeps `minimal.toml` (starter_kit has no Communicator) byte-identical, so the only golden-tick refresh needed for the substrate is the `meme_vector` buffer addition (Task 1). Detector `CodexState` additions (Tasks 5–6) also refresh.
- **Snapshot / golden-tick:** `AgentBuffers.meme_vector` (serialized) and new `CodexState` fields change the snapshot layout. Per spec §2.3 the controller refreshes the committed golden hashes and verifies stability.
- **Channels:** `MEME_CHANNELS = 8` (`program.rs`). The alarm meme channel is `ALARM_MEME_CHANNEL = 0`.
- **Spatial query radius** must be `≤ PERCEPTION_MAX_RADIUS` (16.0); `effective_communicator_range` is clamped to it.

---

## File Structure

- `crates/anabios-core/src/culture.rs` — **new**: constants, `culture_step`, meme-inheritance helper.
- `crates/anabios-core/src/lib.rs` — `pub mod culture;`.
- `crates/anabios-core/src/agent.rs` — `meme_vector` buffer + spawn init.
- `crates/anabios-core/src/module.rs` — `effective_communicator_range` helper.
- `crates/anabios-core/src/program.rs` — `SenseMeme` reads `ctx.meme_sample`; `EvalContext.meme_sample`.
- `crates/anabios-core/src/behavior.rs` — `decide` gains a `meme` param, threads it into `EvalContext`.
- `crates/anabios-core/src/tick.rs` — pass meme to `decide`; add `culture_step` stage.
- `crates/anabios-core/src/reproduce.rs` — child meme inheritance.
- `crates/anabios-core/src/codex.rs` — `EventType` variants; `CodexState` fields; `detect_dialect_formed`, `detect_meme_sweep`, `detect_alarm_call` + pure helpers; wire into `observe_all`.
- `crates/anabios-core/src/scenario.rs` — `communicator` archetype in `archetype_kit`.
- `crates/anabios-headless/src/sweep.rs` — 3 new event names + CSV columns.
- `crates/anabios-core/tests/culture.rs` — **new**: mechanism tests.
- `crates/anabios-core/tests/dialect_emergence.rs` — **new**: multi-seed emergence test.
- `scenarios/dialects.toml` — **new**.

---

## Task 1: `meme_vector` buffer + `culture.rs` skeleton

**Files:**
- Create: `crates/anabios-core/src/culture.rs` (constants only this task)
- Modify: `crates/anabios-core/src/lib.rs` (`pub mod culture;`)
- Modify: `crates/anabios-core/src/agent.rs` (`meme_vector` field + spawn init)
- Modify: `crates/anabios-core/src/module.rs` (`effective_communicator_range`)
- Test: `crates/anabios-core/tests/culture.rs` (new)

**Interfaces:**
- Produces: `AgentBuffers.meme_vector: Vec<[f32; MEME_CHANNELS]>` (serialized, parallel to the other agent buffers), default `[0.0; MEME_CHANNELS]` on every spawn (both freelist-reuse and push paths).
- Produces: `culture::MEME_COPY_RATE: f32 = 0.25`, `culture::MEME_BROADCAST_THRESHOLD: f32 = 0.5`, `culture::MEME_INHERIT_JITTER: f32 = 0.05`, `culture::ALARM_MEME_CHANNEL: usize = 0`.
- Produces: `module::effective_communicator_range(&ModuleList) -> f32` — max `Communicator.range`, `0.0` if none; caller clamps to `PERCEPTION_MAX_RADIUS`.
- Consumes: `program::MEME_CHANNELS`.

- [ ] **Step 1: Write the failing test** — create `crates/anabios-core/tests/culture.rs`:

```rust
//! M14 mechanism tests: meme transmission, sensing, inheritance, and detectors.

use anabios_core::genome::Genome;
use anabios_core::module::{Module, ModuleType};
use anabios_core::prelude_test::Vec2;
use anabios_core::program::MEME_CHANNELS;
use anabios_core::world::World;

/// A kit with a Communicator (so meme ops are enabled) + basics.
fn communicator_kit() -> anabios_core::module::ModuleList {
    let mut m = anabios_core::module::ModuleList::new();
    m.push(Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 });
    m.push(Module::Sensor { sensor_type: anabios_core::module::SensorType::Vision, radius: 0.6, acuity: 0.6 });
    m.push(Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 });
    m.push(Module::Communicator { range: 10.0, channel_id: 0 });
    m
}

#[test]
fn new_agent_has_zeroed_meme_vector() {
    let mut w = World::new(1);
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    assert_eq!(w.agents.meme_vector[id as usize], [0.0; MEME_CHANNELS]);
}

#[test]
fn effective_communicator_range_reports_max() {
    let kit = communicator_kit();
    assert_eq!(anabios_core::module::effective_communicator_range(&kit), 10.0);
    // A kit without a Communicator reports 0.
    let mut bare = anabios_core::module::ModuleList::new();
    bare.push(Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 });
    assert_eq!(anabios_core::module::effective_communicator_range(&bare), 0.0);
    // Silence unused warning until later tasks use it.
    let _ = ModuleType::Communicator;
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p anabios-core --test culture` → FAIL (`no field meme_vector`, `no fn effective_communicator_range`).

- [ ] **Step 3: Create `culture.rs` constants** — `crates/anabios-core/src/culture.rs`:

```rust
//! Culture: per-agent meme vectors transmitted between Communicator-equipped
//! neighbors with imperfect copy (design §3.1, §3.7 step 7, §4.4). Meme ops are
//! gated on the `Communicator` module.

/// Fraction each receiver moves its meme toward the neighbor mean per tick
/// (the "imperfect copy" — < 1.0 means partial adoption).
pub const MEME_COPY_RATE: f32 = 0.25;
/// `broadcast_intent[ch]` above this counts as an active broadcast this tick.
pub const MEME_BROADCAST_THRESHOLD: f32 = 0.5;
/// Std-dev of the per-channel jitter added to an inherited meme vector.
pub const MEME_INHERIT_JITTER: f32 = 0.05;
/// The meme channel used for alarm calls (AlarmCall detector).
pub const ALARM_MEME_CHANNEL: usize = 0;
```

Add `pub mod culture;` to `crates/anabios-core/src/lib.rs`.

- [ ] **Step 4: Add the `meme_vector` buffer** — in `crates/anabios-core/src/agent.rs`, add to `AgentBuffers` (after `program`, before `alive`):

```rust
    /// Per-agent cultural state; transmitted by `culture_step`, read by
    /// `SenseMeme`. Zeroed on spawn; only Communicator agents change it.
    pub meme_vector: Vec<[f32; crate::program::MEME_CHANNELS]>,
```

In `spawn`, the freelist-reuse branch: `self.meme_vector[i] = [0.0; crate::program::MEME_CHANNELS];`. The push branch: `self.meme_vector.push([0.0; crate::program::MEME_CHANNELS]);`. (Place these alongside the corresponding `program` assignments/pushes.)

`AgentBuffers` derives `Default`, so an empty `meme_vector` Vec is fine for a fresh buffer.

- [ ] **Step 5: Add `effective_communicator_range`** — in `crates/anabios-core/src/module.rs`, near the other `effective_*` helpers:

```rust
/// Max `Communicator.range`, or `0.0` if the agent has no `Communicator`.
#[inline]
pub fn effective_communicator_range(modules: &ModuleList) -> f32 {
    modules
        .iter()
        .filter_map(|m| match m {
            Module::Communicator { range, .. } => Some(*range),
            _ => None,
        })
        .fold(0.0_f32, f32::max)
}
```

- [ ] **Step 6: Run to verify pass** — `cargo test -p anabios-core --test culture` → PASS (2 tests).
Note (controller): `AgentBuffers.meme_vector` changes the serialized snapshot → refresh the golden hashes (`UPDATE_HASHES=1`), update `GOLDEN` in `tests/determinism.rs`, confirm PASS + stable. (`minimal.toml` agents get a zeroed meme buffer that never changes — a pure layout bump.)

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/culture.rs crates/anabios-core/src/lib.rs \
        crates/anabios-core/src/agent.rs crates/anabios-core/src/module.rs \
        crates/anabios-core/tests/culture.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M14 meme_vector buffer + culture constants + communicator-range helper

Refresh golden-tick hashes for the new serialized AgentBuffers.meme_vector (§2.3).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `SenseMeme` wiring (reads own meme vector)

**Files:**
- Modify: `crates/anabios-core/src/program.rs` (`EvalContext.meme_sample`; `SenseMeme` evaluate arm)
- Modify: `crates/anabios-core/src/behavior.rs` (`decide` gains a `meme` param → `EvalContext.meme_sample`)
- Modify: `crates/anabios-core/src/tick.rs` (pass `&world.agents.meme_vector[i]` to `decide`)
- Test: `crates/anabios-core/tests/culture.rs` (append)

**Interfaces:**
- Changes: `behavior::decide(program, genome, sensor, meme: &[f32; MEME_CHANNELS], energy, age, eval_stack) -> ActionRegister` (new `meme` param, inserted after `sensor`).
- Produces: `EvalContext.meme_sample: [f32; MEME_CHANNELS]`; `SenseMeme(ch)` pushes `ctx.meme_sample[(ch as usize).min(MEME_CHANNELS-1)]` (was `0.0`).
- Hash-neutral: `EvalContext` is not serialized; `minimal.toml`'s `starter_grazer` contains no `SenseMeme`, and its meme buffer is all-zero anyway.

- [ ] **Step 1: Write the failing test** — append to `crates/anabios-core/tests/culture.rs`:

```rust
use anabios_core::program::{Node, Program};
use anabios_core::tick::step;

#[test]
fn sense_meme_reads_the_agents_own_meme_vector() {
    let mut w = World::new(2);
    let id = w.spawn_agent(Vec2::new(700.0, 700.0), Genome::neutral());
    // Plant a meme value on channel 2, then program move_x = SenseMeme(2).
    w.agents.meme_vector[id as usize][2] = 1.0;
    w.agents.program[id as usize] =
        Program::from_slice(&[Node::SenseMeme(2), Node::MoveTowardX]);
    step(&mut w);
    // Positive meme read → move_x > 0 → normalized to +1 on x.
    assert!(w.desired_direction[id as usize].x > 0.9, "SenseMeme reads the meme vector");
}
```

- [ ] **Step 2: Run to verify failure** — FAIL (SenseMeme still returns 0.0 → no movement).

- [ ] **Step 3: Add `meme_sample` to `EvalContext`** — in `crates/anabios-core/src/program.rs`, add to `EvalContext`:

```rust
    pub meme_sample: [f32; MEME_CHANNELS],
```

Change the `SenseMeme` evaluate arm from `Node::SenseMeme(_) => scratch.push(0.0),` to:

```rust
        Node::SenseMeme(ch) => {
            scratch.push(ctx.meme_sample[(ch as usize).min(MEME_CHANNELS - 1)])
        }
```

- [ ] **Step 4: Thread meme through `decide`** — in `crates/anabios-core/src/behavior.rs`, add the `meme` parameter to `decide` (after `sensor`):

```rust
pub fn decide(
    program: &Program,
    genome: &Genome,
    sensor: &SensorRegister,
    meme: &[f32; crate::program::MEME_CHANNELS],
    energy: f32,
    age: u32,
    eval_stack: &mut Vec<f32>,
) -> ActionRegister {
```

Add to the `EvalContext { ... }` literal: `meme_sample: *meme,`.

- [ ] **Step 5: Update the call site** — in `crates/anabios-core/src/tick.rs` `decide_all`, pass the agent's meme vector:

```rust
        let action = decide(
            &world.agents.program[i],
            &world.agents.genome[i],
            &world.sensors[i],
            &world.agents.meme_vector[i],
            world.agents.energy[i],
            world.agents.age[i],
            &mut world.eval_stack,
        );
```

Update any other `decide(...)` callers (e.g. unit tests in `behavior.rs`/`program.rs`) to pass a `&[0.0; MEME_CHANNELS]` meme argument.

- [ ] **Step 6: Run to verify pass** — `cargo test -p anabios-core --test culture` → PASS. Determinism stays green (run `--test determinism`).

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/program.rs crates/anabios-core/src/behavior.rs \
        crates/anabios-core/src/tick.rs crates/anabios-core/tests/culture.rs
git commit -m "feat(core): M14 SenseMeme reads the agent's own meme vector

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: `culture_step` transmission

**Files:**
- Modify: `crates/anabios-core/src/culture.rs` (`culture_step`)
- Modify: `crates/anabios-core/src/tick.rs` (add the stage)
- Test: `crates/anabios-core/tests/culture.rs` (append)

**Design:** For each alive agent `i` with a `Communicator`, query neighbors within `min(effective_communicator_range, PERCEPTION_MAX_RADIUS)`. Over neighbors `j != i` that also have a `Communicator`, accumulate per-channel broadcast sums and a count. For each channel with `count > 0`, set `received = sum/count` and lerp: `meme[i][ch] += MEME_COPY_RATE * (received - meme[i][ch])`. **No RNG** (deterministic). The source is `broadcast_intent` (fixed this tick), never other agents' `meme_vector`, so in-place update is order-independent-safe; iterate ascending id anyway.

**Interfaces:**
- Produces: `culture::culture_step(world: &mut World)`.
- Consumes: `module::{has, effective_communicator_range}`, `ModuleType::Communicator`, `world.spatial.query`, `world.actions[j].broadcast_intent`, `spatial::PERCEPTION_MAX_RADIUS`.

- [ ] **Step 1: Write the failing test** — append:

```rust
#[test]
fn culture_step_transmits_broadcast_toward_receiver_meme() {
    let mut w = World::new(3);
    let sender = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let receiver = w.spawn_agent(Vec2::new(503.0, 500.0), Genome::neutral()); // within range
    w.agents.modules[sender as usize] = communicator_kit();
    w.agents.modules[receiver as usize] = communicator_kit();
    // Sender broadcasts a high value on channel 1 every tick; receiver just reads.
    w.agents.program[sender as usize] =
        Program::from_slice(&[Node::Const(4.0), Node::Broadcast(1)]);
    w.agents.program[receiver as usize] = Program::from_slice(&[Node::Idle]);
    let before = w.agents.meme_vector[receiver as usize][1];
    step(&mut w);
    let after = w.agents.meme_vector[receiver as usize][1];
    assert!(after > before, "receiver's meme[1] moved toward the sender's broadcast");
}

#[test]
fn no_communicator_means_no_transmission() {
    let mut w = World::new(3);
    let sender = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let receiver = w.spawn_agent(Vec2::new(503.0, 500.0), Genome::neutral());
    w.agents.modules[sender as usize] = communicator_kit();
    // Receiver has the DEFAULT kit — no Communicator.
    w.agents.program[sender as usize] =
        Program::from_slice(&[Node::Const(4.0), Node::Broadcast(1)]);
    step(&mut w);
    assert_eq!(w.agents.meme_vector[receiver as usize][1], 0.0, "no Communicator → no receive (gating)");
}
```

- [ ] **Step 2: Run to verify failure** — FAIL (`no fn culture_step` / transmission doesn't happen).

- [ ] **Step 3: Implement `culture_step`** — in `crates/anabios-core/src/culture.rs`:

```rust
use crate::module::{self, ModuleType};
use crate::program::MEME_CHANNELS;
use crate::spatial::PERCEPTION_MAX_RADIUS;
use crate::world::World;

/// Transmit memes between Communicator neighbors: each receiver lerps its meme
/// vector toward the mean of nearby communicators' broadcasts. Deterministic
/// (no RNG); iterates alive ids ascending. The received value comes from
/// `broadcast_intent` (fixed this tick), so in-place updates don't interfere.
pub fn culture_step(world: &mut World) {
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for &id in &alive_ids {
        let i = id as usize;
        if !module::has(&world.agents.modules[i], ModuleType::Communicator) {
            continue;
        }
        let range = module::effective_communicator_range(&world.agents.modules[i])
            .min(PERCEPTION_MAX_RADIUS);
        if range <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let mut sum = [0.0f32; MEME_CHANNELS];
        let mut count = [0u32; MEME_CHANNELS];
        world.spatial.query(pos, range, |oid| {
            if oid == id {
                return;
            }
            let j = oid as usize;
            if !module::has(&world.agents.modules[j], ModuleType::Communicator) {
                return;
            }
            for ch in 0..MEME_CHANNELS {
                sum[ch] += world.actions[j].broadcast_intent[ch];
                count[ch] += 1;
            }
        });
        for ch in 0..MEME_CHANNELS {
            if count[ch] > 0 {
                let received = sum[ch] / count[ch] as f32;
                let cur = world.agents.meme_vector[i][ch];
                world.agents.meme_vector[i][ch] = cur + MEME_COPY_RATE * (received - cur);
            }
        }
    }
}
```

Note: neighbors are counted whenever they have a Communicator (their `broadcast_intent[ch]` may be 0 — that legitimately pulls the receiver toward 0 on silent channels, i.e. memes fade without reinforcement). This is intentional and keeps the mean well-defined.

- [ ] **Step 4: Add the tick stage** — in `crates/anabios-core/src/tick.rs`, after `reproduce_all(world)` (stage 6) and before `age_and_starve(world)` (stage 7):

```rust
    // Stage 6b: culture — meme transmission between communicators (§3.7 step 7).
    crate::culture::culture_step(world);
```

- [ ] **Step 5: Run to verify pass** — PASS. Determinism stays green (`minimal.toml` has no Communicator → `culture_step` is a no-op).

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/culture.rs crates/anabios-core/src/tick.rs \
        crates/anabios-core/tests/culture.rs
git commit -m "feat(core): M14 culture_step — meme transmission between communicators

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Meme inheritance on reproduction

**Files:**
- Modify: `crates/anabios-core/src/reproduce.rs` (child meme = parent average + jitter, Communicator-gated)
- Modify: `crates/anabios-core/src/culture.rs` (inheritance helper)
- Test: `crates/anabios-core/tests/culture.rs` (append)

**Interfaces:**
- Produces: `culture::inherit_meme(a: &[f32; MEME_CHANNELS], b: &[f32; MEME_CHANNELS], rng: &mut Rng) -> [f32; MEME_CHANNELS]` — per-channel `(a+b)/2 + jitter`, jitter drawn from `rng` scaled by `MEME_INHERIT_JITTER`.
- Consumes: `world.rng` (deterministic, ascending-id order), `module::has(child, Communicator)`.

- [ ] **Step 1: Write the failing test** — append:

```rust
#[test]
fn child_inherits_parent_meme_average_with_jitter() {
    use anabios_core::rng::Rng;
    let a = [1.0f32; MEME_CHANNELS];
    let b = [3.0f32; MEME_CHANNELS];
    let mut rng = Rng::new(42);
    let child = anabios_core::culture::inherit_meme(&a, &b, &mut rng);
    // Average is 2.0; jitter is small, so each channel is near 2.0.
    for &v in &child {
        assert!((v - 2.0).abs() < 0.5, "child meme near parent average ({v})");
    }
}
```

(Confirm the RNG type/constructor path — the map shows `world.rng: Rng`; use the crate's real `Rng` type and a seeding constructor. If `Rng::new` differs, the implementer adapts the test to the real constructor and notes it.)

- [ ] **Step 2: Run to verify failure** — FAIL (`no fn inherit_meme`).

- [ ] **Step 3: Add `inherit_meme`** — in `crates/anabios-core/src/culture.rs` (add the `Rng` import to match the crate):

```rust
use crate::rng::Rng;

/// Child meme = per-channel parent average plus small Gaussian-ish jitter.
/// Jitter uses a centered uniform draw scaled by MEME_INHERIT_JITTER (matches
/// the codebase's `perturb` style; determinism via the shared `rng`).
pub fn inherit_meme(
    a: &[f32; MEME_CHANNELS],
    b: &[f32; MEME_CHANNELS],
    rng: &mut Rng,
) -> [f32; MEME_CHANNELS] {
    let mut out = [0.0f32; MEME_CHANNELS];
    for ch in 0..MEME_CHANNELS {
        let jitter = (rng.f32_unit() - 0.5) * 2.0 * MEME_INHERIT_JITTER;
        out[ch] = 0.5 * (a[ch] + b[ch]) + jitter;
    }
    out
}
```

(Use the real RNG method for a unit float — the map/`reproduce.rs` uses `world.rng`; check `rng.rs` for `f32_unit`/`f32_range` and match it. Keep the draw count fixed at `MEME_CHANNELS` per inheriting birth.)

- [ ] **Step 4: Wire inheritance into `reproduce_all`** — in `crates/anabios-core/src/reproduce.rs`, after the child is spawned (`child_id` known), set the child's meme **only if the child has a Communicator** (keeps non-communicator lineages RNG-free → baseline determinism preserved):

```rust
        if crate::module::has(
            &world.agents.modules[child_id as usize],
            crate::module::ModuleType::Communicator,
        ) {
            let a_meme = world.agents.meme_vector[i];
            let b_meme = world.agents.meme_vector[j];
            world.agents.meme_vector[child_id as usize] =
                crate::culture::inherit_meme(&a_meme, &b_meme, &mut world.rng);
        }
```

(Place this after the `spawn` + `add_to_species`, using the parent indices `i`/`j` captured earlier. `i`/`j` are still valid parent slot indices.)

- [ ] **Step 5: Run to verify pass** — `cargo test -p anabios-core --test culture` PASS. Determinism stays green (`minimal.toml` agents have no Communicator → no meme RNG draws → RNG stream unchanged).

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/reproduce.rs crates/anabios-core/src/culture.rs \
        crates/anabios-core/tests/culture.rs
git commit -m "feat(core): M14 meme inheritance — child = parent average + jitter (communicator-gated)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: `DialectFormed` + `MemeSweep` detectors

**Files:**
- Modify: `crates/anabios-core/src/codex.rs` (2 EventType variants; CodexState fields; 2 detectors + pure helpers; wire in)
- Test: `crates/anabios-core/tests/culture.rs` (append)

**Design:**
- **DialectFormed (11):** split a Communicator-bearing species into two spatial halves by the sign of `x - species_centroid_x`; compute each half's mean meme vector; if the L2 distance between the two half-means exceeds `DIALECT_DIVERGENCE_MIN` for `DIALECT_WINDOW` consecutive ticks (and each half has ≥ `DIALECT_MIN_HALF` members), fire once (latched per species).
- **MemeSweep (12):** track each Communicator species' mean meme value per channel; when a channel's mean rises from ≤ `MEME_SWEEP_LOW` to ≥ `MEME_SWEEP_HIGH` within `MEME_SWEEP_WINDOW`, fire once for that (species, channel) — latched.

**Interfaces:**
- Produces: `EventType::DialectFormed = 11`, `EventType::MemeSweep = 12`.
- Produces: `CodexState.dialect_divergence: BTreeMap<u32, VecDeque<f32>>`, `dialect_active: BTreeSet<u32>`, `meme_mean_history: BTreeMap<(u32, u8), VecDeque<f32>>`, `meme_sweep_active: BTreeSet<(u32, u8)>`.
- Produces constants `DIALECT_WINDOW=50`, `DIALECT_DIVERGENCE_MIN=0.4`, `DIALECT_MIN_HALF=3`, `MEME_SWEEP_WINDOW=80`, `MEME_SWEEP_LOW=0.2`, `MEME_SWEEP_HIGH=0.6`, `MEME_SWEEP_MIN_MEMBERS=5`.
- Produces (pure): `codex::meme_l2(a: &[f32; MEME_CHANNELS], b: &[f32; MEME_CHANNELS]) -> f32`.
- Consumes: `world.agents.{position, species_id, modules, meme_vector}`, `module::has(.., Communicator)`, `compute_centroids`/`centroid_of`.

- [ ] **Step 1: Write the failing test** — append (pins the pure helper + a constructed DialectFormed fire):

```rust
use anabios_core::codex::{meme_l2, EventType};

#[test]
fn meme_l2_is_zero_for_equal_positive_for_divergent() {
    let a = [0.0f32; MEME_CHANNELS];
    let b = [0.0f32; MEME_CHANNELS];
    assert_eq!(meme_l2(&a, &b), 0.0);
    let mut c = [0.0f32; MEME_CHANNELS];
    c[0] = 1.0;
    assert!(meme_l2(&a, &c) > 0.5);
}

#[test]
fn dialect_formed_fires_for_two_divergent_halves() {
    use anabios_core::codex::{observe_all, DIALECT_WINDOW};
    let mut w = World::new(9);
    // West half at x=300 with meme[0]=0; east half at x=700 with meme[0]=1.
    let mut ids = Vec::new();
    for k in 0..4 {
        let id = w.spawn_agent(Vec2::new(300.0, 500.0 + k as f32), Genome::neutral());
        w.agents.modules[id as usize] = communicator_kit();
        ids.push(id);
    }
    for k in 0..4 {
        let id = w.spawn_agent(Vec2::new(700.0, 500.0 + k as f32), Genome::neutral());
        w.agents.modules[id as usize] = communicator_kit();
        w.agents.meme_vector[id as usize][0] = 1.0;
        ids.push(id);
    }
    // Put all 8 in one fresh species.
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
    // Drive observe_all for a full window WITHOUT stepping (memes/positions fixed).
    let mut fired = false;
    for _ in 0..(DIALECT_WINDOW + 2) {
        observe_all(&mut w);
        w.tick += 1;
        if w.codex.events.iter().any(|e| e.event_type == EventType::DialectFormed) {
            fired = true;
            break;
        }
    }
    assert!(fired, "two divergent meme halves form a dialect");
}
```

- [ ] **Step 2: Run to verify failure** — FAIL (`meme_l2`, `DialectFormed` missing).

- [ ] **Step 3: Implement** — in `crates/anabios-core/src/codex.rs`:

Append variants (after `NichePartitioning = 10`):

```rust
    /// Two spatial halves of a communicating species hold divergent memes.
    DialectFormed = 11,
    /// A meme value rises from rare to dominant across a species.
    MemeSweep = 12,
```

Constants:

```rust
pub const DIALECT_WINDOW: usize = 50;
pub const DIALECT_DIVERGENCE_MIN: f32 = 0.4;
pub const DIALECT_MIN_HALF: u32 = 3;
pub const MEME_SWEEP_WINDOW: usize = 80;
pub const MEME_SWEEP_LOW: f32 = 0.2;
pub const MEME_SWEEP_HIGH: f32 = 0.6;
pub const MEME_SWEEP_MIN_MEMBERS: u32 = 5;
```

CodexState fields (before `events`):

```rust
    /// Rolling per-species east/west meme-divergence (for DialectFormed).
    pub dialect_divergence: BTreeMap<u32, VecDeque<f32>>,
    /// Species currently latched as having a formed dialect.
    pub dialect_active: BTreeSet<u32>,
    /// Rolling per (species, channel) mean meme value (for MemeSweep).
    pub meme_mean_history: BTreeMap<(u32, u8), VecDeque<f32>>,
    /// (species, channel) pairs currently latched as swept.
    pub meme_sweep_active: BTreeSet<(u32, u8)>,
```

Pure helper:

```rust
/// L2 distance between two meme vectors.
pub fn meme_l2(a: &[f32; MEME_CHANNELS], b: &[f32; MEME_CHANNELS]) -> f32 {
    let mut s = 0.0f32;
    for ch in 0..MEME_CHANNELS {
        let d = a[ch] - b[ch];
        s += d * d;
    }
    s.sqrt()
}
```

(Import `MEME_CHANNELS` in codex.rs.)

`detect_dialect_formed(world, centroids)`: gather per-Communicator-species member indices; skip species with no Communicator member. For each such species: compute centroid x; split members into west (`pos.x < cx`) / east; require each half ≥ `DIALECT_MIN_HALF`; compute each half's mean meme vector (per-channel average); `div = meme_l2(west_mean, east_mean)`; push into `dialect_divergence[sid]` (bounded `DIALECT_WINDOW`); if full window all `≥ DIALECT_DIVERGENCE_MIN` and not latched → push `DialectFormed` (value = latest div, loc = centroid), latch; else if not diverged, unlatch + (optionally) clear the buffer. Use `BTreeMap`/`BTreeSet`; iterate ascending. Clean up buffers for species that drop below the half-size gate (mirror `detect_territory_formation`).

`detect_meme_sweep(world, _centroids)`: for each Communicator species with ≥ `MEME_SWEEP_MIN_MEMBERS`, for each channel, compute the species mean meme on that channel; push into `meme_mean_history[(sid, ch)]` (bounded `MEME_SWEEP_WINDOW`); if the buffer is full, its front ≤ `MEME_SWEEP_LOW`, its back ≥ `MEME_SWEEP_HIGH`, and not latched → push `MemeSweep` (species_id = sid, value = back, loc = centroid), latch `(sid, ch)`; when back drops below `MEME_SWEEP_LOW` again, unlatch.

Wire both into `observe_all` after `detect_niche_partitioning`:

```rust
    detect_niche_partitioning(world, &centroids);
    detect_dialect_formed(world, &centroids);
    detect_meme_sweep(world, &centroids);
```

(The plan intentionally gives the detector bodies as detailed prose + the exact helper/field/constant signatures rather than 120 lines of transcription — the implementer follows the `detect_territory_formation`/`detect_niche_partitioning` templates already in `codex.rs`, which use the identical bounded-window + latch pattern. Match those idioms exactly.)

- [ ] **Step 4: Run to verify pass** — PASS. Controller: refresh golden hashes (new CodexState fields), confirm stable.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/codex.rs crates/anabios-core/tests/culture.rs \
        crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M14 DialectFormed + MemeSweep detectors

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: `AlarmCall` detector

**Files:**
- Modify: `crates/anabios-core/src/codex.rs` (EventType variant; CodexState field; detector; wire in)
- Test: `crates/anabios-core/tests/culture.rs` (append)

**Design:** AlarmCall recognizes "an alarm broadcast co-occurs with nearby same-species agents fleeing a threat." For each Communicator agent broadcasting on `ALARM_MEME_CHANNEL` above `MEME_BROADCAST_THRESHOLD`, query same-species neighbors within communicator range; count a **response** for each neighbor whose movement opposes its sensed other-species threat (`desired_direction · nearest_other_dir < 0` with a finite `nearest_other_dist`). Accumulate responses into a rolling count; fire `AlarmCall` once (latched) when the cumulative response count reaches `ALARM_MIN_RESPONSES`. This is a same-tick proxy (documented); true temporal precedence is an M16 refinement, and per spec the standalone AlarmCall emergence is deferred to M15.

**Interfaces:**
- Produces: `EventType::AlarmCall = 13`; `CodexState.alarm_responses: u32`, `CodexState.alarm_emitted: bool`; constant `ALARM_MIN_RESPONSES: u32 = 15`.
- Consumes: `world.actions[i].broadcast_intent[ALARM_MEME_CHANNEL]`, `world.desired_direction`, `world.sensors[j].{nearest_other_dir, nearest_other_dist}`, `module::{has, effective_communicator_range}`, `world.spatial.query`.

- [ ] **Step 1: Write the failing test** — append (constructs an alarm broadcaster + a fleeing same-species neighbor and drives `observe_all`):

```rust
#[test]
fn alarm_call_fires_on_broadcast_plus_nearby_flee() {
    use anabios_core::codex::{observe_all, ALARM_MIN_RESPONSES};
    let mut w = World::new(11);
    let caller = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let responder = w.spawn_agent(Vec2::new(503.0, 500.0), Genome::neutral());
    w.agents.modules[caller as usize] = communicator_kit();
    w.agents.modules[responder as usize] = communicator_kit();
    // The controller drives observe_all directly with a hand-set world state:
    // caller broadcasts on the alarm channel; responder flees a threat.
    // (Set world.actions / world.desired_direction / world.sensors on the
    // slots each iteration, since step() would overwrite them — see the
    // detector's inputs. Simplest: set them, call observe_all, repeat.)
    let mut fired = false;
    for _ in 0..(ALARM_MIN_RESPONSES + 5) {
        // Rebuild the spatial hash so the query finds the responder.
        w.spatial.rebuild(&w.agents.position, |k| w.agents.is_alive(k as u32));
        w.resize_scratch();
        w.actions[caller as usize].broadcast_intent[0] = 1.0;
        // Responder senses a threat to its +x and flees to -x.
        w.sensors[responder as usize].nearest_other_dist = 4.0;
        w.sensors[responder as usize].nearest_other_dir = Vec2::new(1.0, 0.0);
        w.desired_direction[responder as usize] = Vec2::new(-1.0, 0.0);
        observe_all(&mut w);
        w.tick += 1;
        if w.codex.events.iter().any(|e| e.event_type == EventType::AlarmCall) {
            fired = true;
            break;
        }
    }
    assert!(fired, "alarm broadcast + nearby flee triggers AlarmCall");
}
```

(If driving `observe_all` with hand-set scratch proves awkward because `resize_scratch`/field visibility differs, the implementer may instead expose a tiny test seam or adjust the setup to the real field APIs — note any deviation in the report. The intent: a broadcaster on channel 0 + a same-species neighbor moving opposite its threat, repeated until the response count crosses the floor.)

- [ ] **Step 2: Run to verify failure** — FAIL (`AlarmCall`, `ALARM_MIN_RESPONSES` missing).

- [ ] **Step 3: Implement** — in `crates/anabios-core/src/codex.rs`:

Append the variant (after `MemeSweep = 12`):

```rust
    /// Alarm broadcasts reliably co-occur with nearby same-species fleeing.
    AlarmCall = 13,
```

Constant + CodexState fields (before `events`):

```rust
pub const ALARM_MIN_RESPONSES: u32 = 15;
```

```rust
    /// Cumulative alarm→flee co-occurrences (for AlarmCall).
    pub alarm_responses: u32,
    /// Latch: the AlarmCall event has been emitted.
    pub alarm_emitted: bool,
```

`detect_alarm_call(world)`: if `alarm_emitted`, return. Collect alive ids. For each caller `i` with a Communicator whose `world.actions[i].broadcast_intent[ALARM_MEME_CHANNEL] > MEME_BROADCAST_THRESHOLD`, query same-species neighbors `j` within `effective_communicator_range.min(PERCEPTION_MAX_RADIUS)`; for each `j != i` with `species_id[j] == species_id[i]` and finite `sensors[j].nearest_other_dist` and `desired_direction[j].dot(sensors[j].nearest_other_dir) < 0.0`, increment `world.codex.alarm_responses`. After the scan, if `alarm_responses >= ALARM_MIN_RESPONSES`, push `AlarmCall` (species_id = the first such caller's species, loc = caller pos) and set `alarm_emitted = true`. Use `crate::culture::{ALARM_MEME_CHANNEL, MEME_BROADCAST_THRESHOLD}` and `crate::spatial::PERCEPTION_MAX_RADIUS`. Deterministic (ascending ids; query order fixed).

Wire into `observe_all` after `detect_meme_sweep`:

```rust
    detect_meme_sweep(world, &centroids);
    detect_alarm_call(world);
```

- [ ] **Step 4: Run to verify pass** — PASS. Controller: refresh golden hashes (new CodexState fields), confirm stable.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/codex.rs crates/anabios-core/tests/culture.rs \
        crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M14 AlarmCall detector — alarm broadcast + nearby flee

Detector ships in M14; standalone emergence deferred to M15 (spec §M14).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Sweep integration (event names + CSV columns)

**Files:**
- Modify: `crates/anabios-headless/src/sweep.rs`

- [ ] **Step 1: Add the failing test** — add to the sweep `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn event_name_covers_m14_events() {
        use anabios_core::codex::EventType;
        assert_eq!(super::event_name(EventType::DialectFormed), "dialect_formed");
        assert_eq!(super::event_name(EventType::MemeSweep), "meme_sweep");
        assert_eq!(super::event_name(EventType::AlarmCall), "alarm_call");
    }
```

- [ ] **Step 2: Run to verify failure** — FAIL (non-exhaustive `event_name` match; `anabios-headless` won't compile with the 3 new variants).

- [ ] **Step 3: Extend `event_name`** — add `EventType::DialectFormed => "dialect_formed"`, `EventType::MemeSweep => "meme_sweep"`, `EventType::AlarmCall => "alarm_call"`.

- [ ] **Step 4: Extend the CSV** — append `,dialect_formed,meme_sweep,alarm_call` to the header string; add three `{}` placeholders + `g("dialect_formed"), g("meme_sweep"), g("alarm_call")` to the row (matching the existing style — this brings the CSV to 19 columns).

- [ ] **Step 5: Run to verify pass** — `cargo test -p anabios-headless` PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-headless/src/sweep.rs
git commit -m "feat(headless): M14 sweep — dialect_formed/meme_sweep/alarm_call columns

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 8: Determinism lock + snapshot round-trip + workspace gate

Controller verification gate (no new production code).

- [ ] **Step 1:** `cargo test -p anabios-core --test determinism` → PASS (stable). If it fails, GOLDEN wasn't refreshed after the last snapshot-affecting task — regenerate and re-run.
- [ ] **Step 2:** `cargo test -p anabios-core --lib roundtrip_preserves_state` → PASS (new `meme_vector` + CodexState fields survive save/load via generic bincode round-trip).
- [ ] **Step 3:** `cargo test --workspace` → all PASS; `cargo clippy --workspace --all-targets -- -D warnings` → clean (watch for `needless_range_loop` on the `for ch in 0..MEME_CHANNELS` loops — prefer `iter_mut().enumerate()` where a slice is indexed by the loop var); `cargo fmt --check` → clean.
- [ ] **Step 4:** Commit only if anything changed here.

```bash
git add -A
git commit -m "test(core): M14 determinism + snapshot verification; fmt + clippy cleanup

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 9: `communicator` archetype + emergence scenario + test

**Files:**
- Modify: `crates/anabios-core/src/module.rs` (`communicator_kit`)
- Modify: `crates/anabios-core/src/program.rs` (`starter_communicator`; append to `starter_library`; update the library-coverage tests)
- Modify: `crates/anabios-core/src/scenario.rs` (`communicator` archetype)
- Create: `scenarios/dialects.toml`
- Create: `crates/anabios-core/tests/dialect_emergence.rs`

**Interfaces:**
- Produces: `module::communicator_kit()` — Locomotor + Vision Sensor + herbivore Mouth + `Communicator { range: 12.0, channel_id: 0 }`.
- Produces: `program::starter_communicator()` — broadcast own meme on channel 1 (`SenseMeme(1), Broadcast(1)`) and cohere toward same-species (herd), so memes propagate within a cluster.
- Produces: `"communicator"` arm in `archetype_kit`.

- [ ] **Step 1: Add `communicator_kit` + `starter_communicator`** — `communicator_kit` in `module.rs` (mirror `marker_kit`, swap Pheromone→Communicator); `starter_communicator` in `program.rs`:

```rust
/// Communicator: rebroadcast own meme on channel 1 and cohere toward the
/// nearest same-species neighbor, so a meme propagates through the cluster.
pub fn starter_communicator() -> Program {
    Program::from_slice(&[
        Node::SenseMeme(1),
        Node::Broadcast(1),
        Node::SenseSameDirX,
        Node::MoveTowardX,
        Node::SenseSameDirY,
        Node::MoveTowardY,
    ])
}
```

Append `starter_communicator` to `starter_library()` at the END and update `starter_library_has_all_starters` (→ 7) and `social_starters_are_bounded_and_evaluable` to include it. Add the `"communicator" => (communicator_kit(), starter_communicator())` arm to `archetype_kit` (+ imports).

- [ ] **Step 2: Write the scenario** — `scenarios/dialects.toml`: two communicator sub-clusters of the SAME behavior seeded far apart (so memes diverge by geography), plus a seed meme difference via a third variable. Simplest robust design: two `communicator` archetype specs (→ species 1 and species 2) in separate regions; each converges internally and drifts apart. Assert `DialectFormed` OR `MemeSweep` (whichever the controller measures as robust). Example:

```toml
name = "dialects"
seed = 0

[[agents]]
count = 24
archetype = "communicator"
placement = { kind = "cluster", center_x = 260.0, center_y = 512.0, radius = 70.0 }
[agents.traits]
lifespan_bias = 1.0

[[agents]]
count = 24
archetype = "communicator"
placement = { kind = "cluster", center_x = 764.0, center_y = 512.0, radius = 70.0 }
[agents.traits]
lifespan_bias = 1.0
```

- [ ] **Step 3: Write the (initially failing) emergence test** — `crates/anabios-core/tests/dialect_emergence.rs`, mirroring `territory_emergence.rs`: multi-seed, release-gated, checks for `DialectFormed` (and separately measures `MemeSweep`). Placeholder `DIALECT_FLOOR = 8`.

- [ ] **Step 4: Measure + tune (controller):** run `cargo test -p anabios-core --release --test dialect_emergence -- --nocapture` with a temporary `eprintln!` counting both `DialectFormed` and `MemeSweep` seeds. Two `communicator` species are two distinct species — `DialectFormed` splits *within* a species by spatial half, so a single tight cluster may not split; if `DialectFormed` is marginal, either (a) widen each cluster's radius so an internal east/west split diverges, or (b) gate the emergence on `MemeSweep` instead (a seeded rare→dominant meme), or (c) add a single wide communicator species spanning a biome barrier so its two halves isolate. Pick whichever the measurement shows robust, set the floor a few below observed, record the rate in a comment, remove the `eprintln!`.

- [ ] **Step 5: Verify gating** — release run PASS; debug run shows `0 passed / 1 ignored`.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/module.rs crates/anabios-core/src/program.rs \
        crates/anabios-core/src/scenario.rs scenarios/dialects.toml \
        crates/anabios-core/tests/dialect_emergence.rs
git commit -m "test(core): M14 communicator archetype + dialect/meme-sweep emergence

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review (author checklist — completed)

**Spec coverage (spec §M14):**
- `culture.rs` + `meme_vector` buffer → Task 1. `culture_step` transmission with drift → Task 3. ✅
- `Broadcast`/`SenseMeme` wiring (SenseMeme returns the real received value) → Task 2 (Broadcast already wired M11). ✅
- Meme inheritance (child = parent average + jitter) → Task 4. ✅
- `DialectFormed` → Task 5; `MemeSweep` → Task 5; `AlarmCall` (ships M14, emergence deferred M15) → Task 6. ✅
- Mechanism tests (broadcast→neighbor reads next step; no Communicator → nothing; drift; child inherits average; detectors fire on constructed positives) → Tasks 1–6. ✅
- Emergence scenario + multi-seed test → Tasks 9. ✅
- Sweep integration → Task 7. ✅
- Golden-tick refresh (§2.3) → Tasks 1/5/6 (controller) + Task 8 lock. ✅

**Type consistency:** `meme_vector: Vec<[f32; MEME_CHANNELS]>` consistent across Tasks 1–5; `decide`'s new `meme` param (Task 2) consistent with the tick call site; `inherit_meme`/`meme_l2` pure helpers pinned by tests; `EventType` appended 11/12/13; `culture_step`/`inherit_meme` signatures consistent Tasks 3–4. ✅

**Placeholder scan:** substrate tasks (1–4, 7, 9) carry full code; detector Tasks 5–6 give exact signatures/fields/constants + prose bodies that follow the existing `detect_territory_formation`/`detect_niche_partitioning` templates in the same file (the implementer matches those idioms) — no vague "add error handling" placeholders. The one judgment step (Task 9 Step 4) is an explicit measure-then-tune controller action.

## Deviation notes (for reviewers)

- **All meme operations are gated on the `Communicator` module** (transmission in `culture_step`, inheritance jitter in `reproduce`). Agents without a Communicator keep an all-zero `meme_vector` and draw no meme RNG. This keeps `minimal.toml` byte-identical (only the buffer-layout golden refresh in Task 1 is needed) and is ecologically sensible (only communicating lineages have culture).
- **Transmission is a deterministic lerp toward the neighbor broadcast-mean (no per-tick RNG); drift/divergence comes from geographic isolation + inheritance jitter.** A shared per-tick RNG jitter in transmission would work too but complicates the RNG stream; deferred as an M16 tuning knob if dialect emergence proves weak.
- **`SenseMeme(ch)` returns the agent's own `meme_vector[ch]`** (its current cultural state, updated last tick by `culture_step`), satisfying the spec's "returns the real received value" (vs the old hardcoded `0.0`).
- **`AlarmCall` is a same-tick proxy** (alarm broadcast co-occurring with a nearby same-species agent moving opposite its sensed threat), not a strict temporal broadcast→then→flee correlation. Per spec §M14 the AlarmCall *detector* ships in M14 while its standalone *emergence* confirmation folds into M15; true temporal precedence is an M16 refinement.
- **`DialectFormed` splits a species into east/west halves by centroid x** as a cheap spatial-isolation proxy. A species that never spatially separates won't trigger it (false-negative only); the emergence scenario/tuning (Task 9) chooses a layout — or `MemeSweep` — that the measurement shows robust.
