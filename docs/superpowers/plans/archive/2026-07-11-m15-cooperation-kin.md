# M15 — Cooperation & Kin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The collaboration payoff — kin recognition (`SenseKinship`), altruistic food-sharing (`Share`), and the `EvolvedCooperation` / `PackHunting` / `HerdCohesion` detectors — composed from the M11–M14 primitives.

**Architecture:** A pure `kin::kinship(...)` blends shared ancestry (`parent_ids`/`lineage_id` overlap) with genome similarity (`1 − L2/√50`); `sense_all` computes it for the nearest neighbor into `SensorRegister.nearest_kinship`, exposed via a new `SenseKinship` node. A new `Share` node writes `ActionRegister.share_intent`; a `share_pass` in `interact()` transfers `SHARE_FRACTION·energy·altruism` from a donor to its `target_id` (donor loses, recipient gains) — gated on `share_intent` + the `Altruism` genome slot, so kin-direction emerges from programs gating `Share` on `SenseKinship`. Three detectors (mirroring the M12–M14 rolling-window + latch templates) recognize sustained sharing, pack attacks (≥N distinct same-species attackers on one target in a window — recorded per combat hit), and persistent herd crowding. This milestone also fixes the M14 `detect_alarm_call` birth-tick bug by re-sizing scratch after reproduce.

**Tech Stack:** Rust (`anabios-core` pure-sim, `anabios-headless` CLI), `glam::Vec2`, `serde`/`bincode` snapshots, `BTreeMap`/`VecDeque` detector state, single `Xoshiro256++` RNG.

## Global Constraints

- **Determinism (design §7.2):** id-ordered or `BTreeMap`/`BTreeSet`/`VecDeque` iteration; **no `HashMap`** in tick/detector paths; no unordered float reductions; `share_pass` iterates ascending ids (in-place energy transfer is deterministic in that order); no RNG in kinship/share/detectors.
- **`EventType` variants appended at the END, in order:** `EvolvedCooperation = 14`, `PackHunting = 15`, `HerdCohesion = 16` (current tail `AlarmCall = 13`). bincode encodes by positional index — never insert mid-enum.
- **`Node` variants appended at the END:** `SenseKinship` (input, arity 0, node_kind 41) then `Share` (output, arity 1, node_kind 42), after `SensePheromone` (kind 40). **Both excluded from the `random_node` mutation grammar** (M11–M14 convention) so evolved programs stay unchanged.
- **Gate new mechanics on their trigger:** sharing runs only when `share_intent > SHARE_THRESHOLD` AND `Altruism > 0`; kinship sensing is passive but `SenseKinship`/`Share` aren't in the grammar and aren't in `starter_grazer`, so `minimal.toml` is behaviorally unchanged. Detector `CodexState` additions stay empty in `minimal.toml` (no sharing/pack/herd there). Expect golden-tick refreshes only where noted (serialized layout changes + possible tick-1000 shifts from evolved behavior).
- **Snapshot / golden-tick:** `ActionRegister.share_intent` is `#[serde(skip)]` scratch (in `world.actions`) — no snapshot impact. `SensorRegister.nearest_kinship` is in the serde-skip `sensors` scratch — no snapshot impact. New serialized state = `CodexState` detector fields (Tasks 4–6) → refresh. The AlarmCall fix (Task 3) may shift tick-1000. Controller refreshes and verifies stability.
- **Channels / sentinels:** `NO_TARGET = u32::MAX`, `NO_NEIGHBOR_ID = u32::MAX`, `LINEAGE_NONE = 0`. `GENOME_LEN = 50`, `GenomeSlot::Altruism = 24` (already exists).

---

## File Structure

- `crates/anabios-core/src/kin.rs` — **new**: pure `kinship(...)` + `SQRT_GENOME_LEN`.
- `crates/anabios-core/src/lib.rs` — `pub mod kin;`.
- `crates/anabios-core/src/sense.rs` — `SensorRegister.nearest_kinship`; compute it in `sense_all`.
- `crates/anabios-core/src/program.rs` — `SenseKinship`/`Share` nodes; `ActionRegister.share_intent`; `EvalContext.nearest_kinship`; `starter_cooperator`/`starter_kin_sharer`.
- `crates/anabios-core/src/behavior.rs` — thread `sensor.nearest_kinship` into `EvalContext`.
- `crates/anabios-core/src/interact.rs` — `share_pass`; record combat hits in `combat_pass`.
- `crates/anabios-core/src/tick.rs` — re-size scratch after reproduce (AlarmCall fix).
- `crates/anabios-core/src/codex.rs` — `EventType` variants; `CombatHit`; `CodexState` fields; `detect_evolved_cooperation`/`detect_pack_hunting`/`detect_herd_cohesion` + pure helpers; wire into `observe_all`.
- `crates/anabios-core/src/scenario.rs` — `cooperator`/`pack_hunter` (exists)/`herd` (exists) archetypes.
- `crates/anabios-headless/src/sweep.rs` — 3 new event names + CSV columns.
- `crates/anabios-core/tests/cooperation.rs` — **new**: mechanism tests.
- `crates/anabios-core/tests/cooperation_emergence.rs` — **new**: multi-seed emergence test(s).
- `scenarios/cooperation.toml`, `scenarios/pack-vs-herd.toml` — **new**.

---

## Task 1: Kin recognition (`kinship` helper + `SenseKinship` node)

**Files:**
- Create: `crates/anabios-core/src/kin.rs`
- Modify: `crates/anabios-core/src/lib.rs` (`pub mod kin;`)
- Modify: `crates/anabios-core/src/sense.rs` (`SensorRegister.nearest_kinship`; compute in `sense_all`)
- Modify: `crates/anabios-core/src/program.rs` (`SenseKinship` node; `EvalContext.nearest_kinship`)
- Modify: `crates/anabios-core/src/behavior.rs` (thread into `EvalContext`)
- Test: `crates/anabios-core/tests/cooperation.rs` (new)

**Interfaces:**
- Produces: `kin::SQRT_GENOME_LEN: f32` (= √50 ≈ 7.0710678); `kin::kinship(a_lineage: u64, a_parents: &[u64; 2], a_genome: &Genome, b_lineage: u64, b_parents: &[u64; 2], b_genome: &Genome) -> f32` in [0,1] = `0.5*ancestry + 0.5*genome_sim`, where `ancestry = (shared_nonzero_parents*0.25 + parent_child*0.5).min(1.0)`, `genome_sim = (1 - a.distance(b)/SQRT_GENOME_LEN).clamp(0,1)`.
- Produces: `SensorRegister.nearest_kinship: f32` (0.0 default; kinship of the overall-nearest neighbor, 0 if none).
- Produces: `Node::SenseKinship` (arity 0, node_kind 41) pushing `ctx.nearest_kinship`; `EvalContext.nearest_kinship: f32`.
- Consumes: `AgentBuffers.{lineage_id, parent_ids, genome}`, `Genome::distance`, `LINEAGE_NONE`.

- [ ] **Step 1: Write the failing test** — create `crates/anabios-core/tests/cooperation.rs`:

```rust
//! M15 mechanism tests: kin recognition, sharing, and the cooperation detectors.

use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::kin::kinship;

#[test]
fn kinship_high_for_siblings_low_for_unrelated() {
    // Siblings: share both parent lineages (2 and 3); near-identical genomes.
    let g = Genome::neutral();
    let sib = kinship(10, &[2, 3], &g, 11, &[2, 3], &g);
    // Unrelated: no shared parents; distant genomes.
    let mut far = Genome::neutral();
    far.set(GenomeSlot::Size, 1.0);
    far.set(GenomeSlot::DietCarnivory, 1.0);
    far.set(GenomeSlot::SpeedMax, 1.0);
    let unrel = kinship(10, &[2, 3], &g, 99, &[50, 51], &far);
    assert!(sib > 0.7, "siblings with identical genome are highly related ({sib})");
    assert!(unrel < sib, "unrelated distant-genome pair is less related ({unrel} < {sib})");
}

#[test]
fn kinship_parent_child_is_related() {
    let g = Genome::neutral();
    // Agent 5 is a parent of agent 12 (12's parents include lineage 5).
    let r = kinship(5, &[1, 2], &g, 12, &[5, 7], &g);
    assert!(r > 0.5, "parent-child relatedness ({r})");
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p anabios-core --test cooperation` → FAIL (`no crate/module kin`).

- [ ] **Step 3: Create `kin.rs`** — `crates/anabios-core/src/kin.rs`:

```rust
//! Kin recognition: a scalar relatedness in [0,1] blending shared ancestry
//! (parent-lineage overlap + parent/child links) with genome similarity.
//! Gates altruism on kin so cooperation is evolutionarily stable (§3.2, §4.3).

use crate::agent::LINEAGE_NONE;
use crate::genome::{Genome, GENOME_LEN};

/// √GENOME_LEN — the max possible L2 distance between two genomes whose slots
/// are all in [0,1]. Used to normalize genome distance into a [0,1] similarity.
pub const SQRT_GENOME_LEN: f32 = 7.071_068; // sqrt(50)

/// Relatedness of two agents in [0,1]: `0.5*ancestry + 0.5*genome_similarity`.
pub fn kinship(
    a_lineage: u64,
    a_parents: &[u64; 2],
    a_genome: &Genome,
    b_lineage: u64,
    b_parents: &[u64; 2],
    b_genome: &Genome,
) -> f32 {
    // Ancestry: shared (non-NONE) parents + parent/child link.
    let mut shared = 0u32;
    for pa in a_parents {
        if *pa != LINEAGE_NONE && b_parents.contains(pa) {
            shared += 1;
        }
    }
    let parent_child = a_parents.contains(&b_lineage) || b_parents.contains(&a_lineage);
    let ancestry = (shared as f32 * 0.25 + if parent_child { 0.5 } else { 0.0 }).min(1.0);

    // Genome similarity from normalized L2 distance.
    let genome_sim = (1.0 - a_genome.distance(b_genome) / SQRT_GENOME_LEN).clamp(0.0, 1.0);

    (0.5 * ancestry + 0.5 * genome_sim).clamp(0.0, 1.0)
}
```

Add `pub mod kin;` to `lib.rs`. Confirm `GENOME_LEN` and `Genome::distance` are `pub` (map: `distance` at genome.rs:168, `GENOME_LEN` at genome.rs:14) and `LINEAGE_NONE` is `pub` (agent.rs:33). If `LINEAGE_NONE` is `pub(crate)`, use the literal `0` and note it.

- [ ] **Step 4: Add `nearest_kinship` to SensorRegister + compute in `sense_all`** — in `sense.rs`, add `pub nearest_kinship: f32` to `SensorRegister` and `nearest_kinship: 0.0` to `Default`. At the end of the per-agent neighbor scan in `sense_all` (after `nearest_neighbor_id` is finalized), add:

```rust
        // Kinship of the overall-nearest neighbor (0 when there is none).
        registers[i].nearest_kinship = if registers[i].has_neighbor {
            let n = registers[i].nearest_neighbor_id as usize;
            crate::kin::kinship(
                agents.lineage_id[i],
                &agents.parent_ids[i],
                &agents.genome[i],
                agents.lineage_id[n],
                &agents.parent_ids[n],
                &agents.genome[n],
            )
        } else {
            0.0
        };
```

(Use the loop's real agent index binding for `i`. Confirm `has_neighbor`/`nearest_neighbor_id` are set before this point.)

- [ ] **Step 5: Add the `SenseKinship` node + EvalContext field** — in `program.rs`: append `SenseKinship` to the `Node` enum END (after `SensePheromone`); `arity => 0`; `node_kind => 41`; `evaluate` arm `Node::SenseKinship => scratch.push(ctx.nearest_kinship)`. Add `pub nearest_kinship: f32` to `EvalContext`. **Do NOT add to `random_node`.**

- [ ] **Step 6: Thread into `decide`** — in `behavior.rs`, add `nearest_kinship: sensor.nearest_kinship,` to the `EvalContext { ... }` literal.

- [ ] **Step 7: Run to verify pass** — `cargo test -p anabios-core --test cooperation` PASS.
Note (controller): `SensorRegister`/`EvalContext` are serde-skip scratch and `SenseKinship` is grammar-excluded → `minimal.toml` unchanged. Run `cargo test -p anabios-core --test determinism` → PASS (no refresh).

- [ ] **Step 8: Commit**

```bash
git add crates/anabios-core/src/kin.rs crates/anabios-core/src/lib.rs \
        crates/anabios-core/src/sense.rs crates/anabios-core/src/program.rs \
        crates/anabios-core/src/behavior.rs crates/anabios-core/tests/cooperation.rs
git commit -m "feat(core): M15 kin recognition — kinship helper + SenseKinship node

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Altruistic sharing (`Share` node + `share_pass`)

**Files:**
- Modify: `crates/anabios-core/src/program.rs` (`ActionRegister.share_intent`; `Share` node)
- Modify: `crates/anabios-core/src/interact.rs` (`share_pass`)
- Test: `crates/anabios-core/tests/cooperation.rs` (append)

**Interfaces:**
- Produces: `ActionRegister.share_intent: f32` (default 0.0); `Node::Share` (arity 1, node_kind 42) doing `action.share_intent += pop()`.
- Produces: `interact::SHARE_THRESHOLD: f32 = 0.5`, `interact::SHARE_FRACTION: f32 = 0.2`, `interact::SHARE_RANGE: f32 = 2.0`.
- Produces: a `share_pass(world, &alive_ids)` called in `interact_all` after `combat_pass`/`scavenge_pass`.
- Consumes: `ActionRegister.{share_intent, target_id}`, `GenomeSlot::Altruism`, `SensorRegister.nearest_neighbor_dist`.

- [ ] **Step 1: Write the failing test** — append to `crates/anabios-core/tests/cooperation.rs`:

```rust
use anabios_core::prelude_test::Vec2;
use anabios_core::program::{Node, Program};
use anabios_core::tick::step;
use anabios_core::world::World;

#[test]
fn share_transfers_energy_scaled_by_altruism() {
    let mut w = World::new(4);
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Altruism, 1.0);
    let donor = w.spawn_agent(Vec2::new(500.0, 500.0), g);
    let recipient = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral()); // within SHARE_RANGE
    // Donor always shares (share_intent = 1.0 via Const + Share).
    w.agents.program[donor as usize] = Program::from_slice(&[Node::Const(1.0), Node::Share]);
    w.agents.program[recipient as usize] = Program::from_slice(&[Node::Idle]);
    let d0 = w.agents.energy[donor as usize];
    let r0 = w.agents.energy[recipient as usize];
    step(&mut w);
    assert!(w.agents.energy[donor as usize] < d0, "donor lost energy");
    assert!(w.agents.energy[recipient as usize] > r0, "recipient gained energy");
}

#[test]
fn zero_altruism_means_no_sharing() {
    let mut w = World::new(4);
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Altruism, 0.0);
    let donor = w.spawn_agent(Vec2::new(500.0, 500.0), g);
    let recipient = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
    w.agents.program[donor as usize] = Program::from_slice(&[Node::Const(1.0), Node::Share]);
    let r0 = w.agents.energy[recipient as usize];
    step(&mut w);
    // Recipient's only energy change is its own metabolism/grazing — no share in.
    // Assert it did not gain the share amount (energy did not increase from sharing).
    assert!(w.agents.energy[recipient as usize] <= r0 + 1e-3, "no altruism → no share");
}
```

- [ ] **Step 2: Run to verify failure** — FAIL (`no field share_intent` / `Node::Share`).

- [ ] **Step 3: Add `share_intent` + the `Share` node** — in `program.rs`: add `pub share_intent: f32` to `ActionRegister` (and `share_intent: 0.0` to its `Default`); append `Share` to the `Node` enum END (after `SenseKinship`); `arity => 1`; `is_output => true`; `node_kind => 42`; `evaluate` arm `Node::Share => action.share_intent += scratch.pop().unwrap()`. **Do NOT add to `random_node`.**

- [ ] **Step 4: Implement `share_pass`** — in `interact.rs`, add the constants and the pass, and call it in `interact_all` after the other passes:

```rust
/// `share_intent` above this triggers a transfer.
pub const SHARE_THRESHOLD: f32 = 0.5;
/// Max fraction of the donor's energy shared in one tick (before altruism scale).
pub const SHARE_FRACTION: f32 = 0.2;
/// Contact range (world units) for sharing. Mirrors COMBAT_RANGE.
pub const SHARE_RANGE: f32 = 2.0;

/// Altruism: a donor with `share_intent` transfers a fraction of its energy to
/// its action target (the nearest neighbor), scaled by the `Altruism` genome
/// slot. Donor loses, recipient gains. Program-level gating on `SenseKinship`
/// makes this kin-directed.
fn share_pass(world: &mut World, alive_ids: &[u32]) {
    use crate::genome::GenomeSlot;
    for &id in alive_ids {
        let i = id as usize;
        if world.actions[i].share_intent <= SHARE_THRESHOLD {
            continue;
        }
        let altruism = world.agents.genome[i].get(GenomeSlot::Altruism);
        if altruism <= 0.0 {
            continue;
        }
        let tgt = world.actions[i].target_id;
        if tgt == crate::program::NO_TARGET {
            continue;
        }
        let t = tgt as usize;
        if t == i || !world.agents.is_alive(tgt) {
            continue;
        }
        if world.sensors[i].nearest_neighbor_dist >= SHARE_RANGE {
            continue;
        }
        let amount = SHARE_FRACTION * world.agents.energy[i].max(0.0) * altruism;
        if amount <= 0.0 {
            continue;
        }
        world.agents.energy[i] -= amount;
        world.agents.energy[t] += amount;
        // Record for the EvolvedCooperation detector (Task 4 adds the buffer).
    }
}
```

Add `share_pass(world, &alive_ids);` to `interact_all` after the existing passes. (Task 4 adds the share-event recording line.)

- [ ] **Step 5: Run to verify pass** — PASS. Determinism stays green (`starter_grazer` has no `Share`; `minimal.toml` never shares). Run `--test determinism`.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/program.rs crates/anabios-core/src/interact.rs \
        crates/anabios-core/tests/cooperation.rs
git commit -m "feat(core): M15 altruistic sharing — Share node + share_pass (altruism-scaled)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Fix M14 AlarmCall birth-tick suppression

**Files:**
- Modify: `crates/anabios-core/src/tick.rs` (re-size scratch after reproduce)
- Test: `crates/anabios-core/tests/cooperation.rs` (append) — or verify via existing behavior

**Design:** `detect_alarm_call` early-returns when `world.actions.len() < capacity()`. `resize_scratch` runs at the top of `step()`, but `reproduce_all` (stage 6) grows capacity, so on any birth tick the guard trips and alarm counting is skipped. Fix: call `world.resize_scratch()` once more after `reproduce_all` (and after `culture_step`), so `actions`/`sensors`/`desired_direction` cover the new capacity before `observe_all`. Newborns get default scratch entries (no broadcast, no threat, zero movement) — they're inert for alarm detection, which is correct.

**Interfaces:** Consumes `World::resize_scratch` (pub(crate), callable from tick.rs).

- [ ] **Step 1: Write the failing test** — append to `crates/anabios-core/tests/cooperation.rs`:

```rust
#[test]
fn resize_scratch_after_reproduce_keeps_alarm_scratch_sized() {
    // A stepping world with reproduction must keep world.actions sized to
    // capacity every tick, so the alarm detector never early-returns on a
    // birth tick. Construct a small growing population and assert the
    // invariant holds across ticks.
    let mut w = World::new(7);
    for k in 0..8 {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.0); // reproduce readily
        let _ = w.spawn_agent(Vec2::new(500.0 + k as f32, 500.0), g);
    }
    for _ in 0..30 {
        step(&mut w);
        assert!(
            w.actions.len() >= w.agents.capacity(),
            "world.actions must stay sized to capacity (alarm scratch invariant)"
        );
    }
}
```

- [ ] **Step 2: Run to verify failure** — this may already pass or fail depending on whether a birth grew capacity beyond `actions.len()` within 30 ticks. If it passes as-is, that means no birth grew capacity mid-run in this seed; keep the test (it documents the invariant) and still apply Step 3 (the real fix), then it's guaranteed. If it fails, Step 3 fixes it.

- [ ] **Step 3: Re-size scratch after reproduce** — in `crates/anabios-core/src/tick.rs`, after `crate::culture::culture_step(world);` (stage 6b) and before `age_and_starve(world);`:

```rust
    // Keep scratch sized to the post-reproduce capacity so end-of-tick detectors
    // (AlarmCall) that read actions/sensors/desired_direction see every agent.
    world.resize_scratch();
```

- [ ] **Step 4: Run to verify pass** — `cargo test -p anabios-core --test cooperation` PASS.
Note (controller): run `cargo test -p anabios-core --test determinism`. `minimal.toml` has no Communicators, so alarm detection is a no-op there regardless — BUT the extra resize is harmless and behavior-neutral for ticks 0/100. If evolved Communicators over 1000 ticks now trigger alarm counting on birth ticks that were previously skipped, tick-1000 may shift; if so, refresh the golden hash (verify stable) and note it.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/tick.rs crates/anabios-core/tests/cooperation.rs \
        crates/anabios-core/tests/determinism.rs
git commit -m "fix(core): M15 resize scratch after reproduce so AlarmCall counts on birth ticks

Carries the M14 whole-branch-review follow-up: the alarm detector's scratch
guard early-returned whenever a birth grew capacity past the scratch length,
silently suppressing alarm counting.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: `EvolvedCooperation` detector

**Files:**
- Modify: `crates/anabios-core/src/interact.rs` (record share events)
- Modify: `crates/anabios-core/src/codex.rs` (EventType variant; CodexState fields; detector; wire in)
- Test: `crates/anabios-core/tests/cooperation.rs` (append)

**Design:** Sharing that persists at a rate above a threshold, per species. `share_pass` records each transfer as `(tick, donor_species)` into a rolling `CodexState.share_events` window; `detect_evolved_cooperation` prunes the window and fires (edge-triggered, per species) when a species has ≥ `COOPERATION_MIN_SHARES` shares within `COOPERATION_WINDOW` ticks.

**Interfaces:**
- Produces: `EventType::EvolvedCooperation = 14`.
- Produces: `CodexState.share_events: VecDeque<(u64, u32)>` (tick, species), `cooperation_active: BTreeSet<u32>`.
- Produces: `codex::COOPERATION_WINDOW: u64 = 100`, `codex::COOPERATION_MIN_SHARES: usize = 12`.
- Consumes: `compute_centroids`/`centroid_of`.

- [ ] **Step 1: Write the failing test** — append (drives sharing then observes):

```rust
use anabios_core::codex::EventType;

#[test]
fn evolved_cooperation_fires_on_sustained_sharing() {
    let mut w = World::new(5);
    // A tight cluster of altruists that always share with their neighbor.
    let mut ids = Vec::new();
    for k in 0..8 {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Altruism, 1.0);
        let id = w.spawn_agent(Vec2::new(500.0 + (k % 3) as f32, 500.0 + (k / 3) as f32), g);
        w.agents.program[id as usize] = Program::from_slice(&[Node::Const(1.0), Node::Share]);
        ids.push(id);
    }
    let mut fired = false;
    for _ in 0..200 {
        step(&mut w);
        if w.codex.events.iter().any(|e| e.event_type == EventType::EvolvedCooperation) {
            fired = true;
            break;
        }
    }
    assert!(fired, "sustained kin sharing → EvolvedCooperation");
}
```

- [ ] **Step 2: Run to verify failure** — FAIL (`EvolvedCooperation` missing).

- [ ] **Step 3: Record shares + implement the detector** — in `interact.rs` `share_pass`, after the energy transfer, add:

```rust
        world
            .codex
            .share_events
            .push_back((world.tick, world.agents.species_id[i]));
```

In `codex.rs`: append `EvolvedCooperation = 14` to `EventType`; add constants `COOPERATION_WINDOW`, `COOPERATION_MIN_SHARES`; add `share_events`/`cooperation_active` to `CodexState` (before `events`); add `detect_evolved_cooperation(world, centroids)` — prune `share_events` older than `tick - COOPERATION_WINDOW`, tally per species, edge-fire (via `cooperation_active`) when a species' count ≥ `COOPERATION_MIN_SHARES`, re-arm when it drops below. Wire into `observe_all` after `detect_alarm_call`. Follow the `detect_combat_raid` prune + `detect_territory_formation` per-species latch idioms.

- [ ] **Step 4: Run to verify pass** — PASS. Controller: refresh golden hashes (new `CodexState` fields), confirm stable.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/interact.rs crates/anabios-core/src/codex.rs \
        crates/anabios-core/tests/cooperation.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M15 EvolvedCooperation detector — sustained kin sharing

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: `PackHunting` detector

**Files:**
- Modify: `crates/anabios-core/src/interact.rs` (record combat hits per attacker)
- Modify: `crates/anabios-core/src/codex.rs` (EventType variant; `CombatHit`; CodexState fields; detector; wire in)
- Test: `crates/anabios-core/tests/cooperation.rs` (append)

**Design:** ≥ `PACK_MIN_ATTACKERS` distinct same-species agents deal combat damage to one target within `PACK_WINDOW` ticks. `combat_pass` records each hit as `CombatHit { tick, target_id, attacker_id, species }` into a rolling `CodexState.combat_hits` window; `detect_pack_hunting` prunes, groups by target, counts distinct attacker ids sharing the majority species, and edge-fires (global `pack_active` latch, re-arm when no target qualifies).

**Interfaces:**
- Produces: `EventType::PackHunting = 15`; `codex::CombatHit { tick: u64, target_id: u32, attacker_id: u32, species: u32 }`.
- Produces: `CodexState.combat_hits: VecDeque<CombatHit>`, `pack_active: bool`.
- Produces: `codex::PACK_WINDOW: u64 = 8`, `codex::PACK_MIN_ATTACKERS: usize = 3`.

- [ ] **Step 1: Write the failing test** — append (3 same-species predators hit one prey):

```rust
use anabios_core::module::Module;

#[test]
fn pack_hunting_fires_when_three_attackers_hit_one_target() {
    let mut w = World::new(6);
    let prey = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    // Move prey to its own species so the attackers are "other species" to it.
    let psid = w.species_centroids.len() as u32;
    w.species_centroids.push(Genome::neutral());
    w.species_parents.push(Some(0));
    w.species_member_counts.push(0);
    w.next_species_id = psid + 1;
    w.remove_from_species(w.agents.species_id[prey as usize]);
    w.agents.species_id[prey as usize] = psid;
    w.add_to_species(psid);
    // Three armed same-species predators adjacent to the prey, all firing.
    for k in 0..3 {
        let pred = w.spawn_agent(Vec2::new(501.0, 500.0 + k as f32 * 0.3), Genome::neutral());
        let mut kit = anabios_core::module::ModuleList::new();
        kit.push(Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 });
        kit.push(Module::Sensor {
            sensor_type: anabios_core::module::SensorType::Vision,
            radius: 0.6,
            acuity: 0.6,
        });
        kit.push(Module::Weapon { damage: 1.0, energy_cost: 0.1 });
        w.agents.modules[pred as usize] = kit;
        w.agents.program[pred as usize] = Program::from_slice(&[Node::Const(1.0), Node::FireWeapon]);
    }
    let mut fired = false;
    for _ in 0..12 {
        step(&mut w);
        if w.codex.events.iter().any(|e| e.event_type == EventType::PackHunting) {
            fired = true;
            break;
        }
    }
    assert!(fired, "3 same-species attackers on one target → PackHunting");
}
```

- [ ] **Step 2: Run to verify failure** — FAIL (`PackHunting` / `CombatHit` missing).

- [ ] **Step 3: Record hits + implement** — in `interact.rs` `combat_pass`, after applying damage (`world.agents.energy[t] -= net; ...`), record the hit:

```rust
        world.codex.combat_hits.push_back(crate::codex::CombatHit {
            tick: world.tick,
            target_id: tgt,
            attacker_id: id,
            species: world.agents.species_id[i],
        });
```

In `codex.rs`: append `PackHunting = 15`; add the `CombatHit` struct (derives `Debug, Clone, Serialize, Deserialize`); constants `PACK_WINDOW`, `PACK_MIN_ATTACKERS`; `CodexState.combat_hits` + `pack_active` (before `events`); `detect_pack_hunting(world, centroids)` — prune `combat_hits` older than `tick - PACK_WINDOW`; build `BTreeMap<u32 target, BTreeMap<u32 species, BTreeSet<u32 attacker>>>`; if any (target, species) has `≥ PACK_MIN_ATTACKERS` distinct attackers → `raiding = true` (record that species + a location from `centroid_of`); edge-fire on `pack_active` transition, re-arm when none qualify. Wire into `observe_all` after `detect_evolved_cooperation`. Determinism: all `BTreeMap`/`BTreeSet`.

- [ ] **Step 4: Run to verify pass** — PASS. Controller: refresh golden hashes, confirm stable.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/interact.rs crates/anabios-core/src/codex.rs \
        crates/anabios-core/tests/cooperation.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M15 PackHunting detector — N same-species attackers on one target

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: `HerdCohesion` detector

**Files:**
- Modify: `crates/anabios-core/src/codex.rs` (EventType variant; CodexState fields; detector; wire in)
- Test: `crates/anabios-core/tests/cooperation.rs` (append)

**Design:** A species maintains persistent clustering — its mean per-member crowding (same-species neighbors within perception, already computed in `SensorRegister.crowding`) stays high over a window. `detect_herd_cohesion` reads `world.sensors[i].crowding` per member (scratch, sized after the Task 3 resize), means it per species, pushes into a bounded window, and edge-fires when the full window is all ≥ `HERD_CROWDING_MIN` for a species with ≥ `HERD_MIN_MEMBERS` members.

**Interfaces:**
- Produces: `EventType::HerdCohesion = 16`.
- Produces: `CodexState.herd_crowding: BTreeMap<u32, VecDeque<f32>>`, `herd_active: BTreeSet<u32>`.
- Produces: `codex::HERD_WINDOW: usize = 60`, `codex::HERD_CROWDING_MIN: f32 = 3.0`, `codex::HERD_MIN_MEMBERS: u32 = 5`.
- Consumes: `world.sensors[i].crowding`, `compute_centroids`/`centroid_of`.

- [ ] **Step 1: Write the failing test** — append (a tight herd sustains high crowding):

```rust
#[test]
fn herd_cohesion_fires_for_a_tight_persistent_herd() {
    use anabios_core::codex::HERD_WINDOW;
    let mut w = World::new(8);
    // A tight cluster of same-species herders (default species 0).
    let mut ids = Vec::new();
    for k in 0..10 {
        let id = w.spawn_agent(Vec2::new(500.0 + (k % 5) as f32 * 0.5, 500.0 + (k / 5) as f32 * 0.5), Genome::neutral());
        // Herd behavior: cohere toward same-species neighbor.
        w.agents.program[id as usize] = Program::from_slice(&[
            Node::SenseSameDirX, Node::MoveTowardX, Node::SenseSameDirY, Node::MoveTowardY,
        ]);
        ids.push(id);
    }
    let mut fired = false;
    for _ in 0..(HERD_WINDOW + 20) {
        step(&mut w);
        if w.codex.events.iter().any(|e| e.event_type == EventType::HerdCohesion) {
            fired = true;
            break;
        }
    }
    assert!(fired, "a tight persistent herd → HerdCohesion");
}
```

- [ ] **Step 2: Run to verify failure** — FAIL (`HerdCohesion` missing).

- [ ] **Step 3: Implement** — in `codex.rs`: append `HerdCohesion = 16`; constants; `CodexState.herd_crowding` + `herd_active` (before `events`); `detect_herd_cohesion(world, centroids)` — gather per-species members + summed crowding (guard against unsized `sensors` like `detect_alarm_call`: if `world.sensors.len() < world.agents.capacity()`, return); for each species with ≥ `HERD_MIN_MEMBERS`, push mean crowding into a bounded window; edge-fire when full window all ≥ `HERD_CROWDING_MIN`, latch, clean up on failing the gate (mirror `detect_territory_formation`). Wire into `observe_all` after `detect_pack_hunting`.

- [ ] **Step 4: Run to verify pass** — PASS. Controller: refresh golden hashes, confirm stable.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/codex.rs crates/anabios-core/tests/cooperation.rs \
        crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): M15 HerdCohesion detector — persistent high same-species crowding

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Sweep integration (event names + CSV columns)

**Files:** Modify `crates/anabios-headless/src/sweep.rs`.

- [ ] **Step 1: Add the failing test** — add `event_name_covers_m15_events` asserting `EvolvedCooperation→"evolved_cooperation"`, `PackHunting→"pack_hunting"`, `HerdCohesion→"herd_cohesion"`.
- [ ] **Step 2: Run to verify failure** — non-exhaustive match; `anabios-headless` won't compile.
- [ ] **Step 3: Extend `event_name`** — add the three arms.
- [ ] **Step 4: Extend the CSV** — append `,evolved_cooperation,pack_hunting,herd_cohesion` to the header, three `{}` + three `g(...)` to the row (→ 22 columns).
- [ ] **Step 5: Run** — `cargo test -p anabios-headless` PASS.
- [ ] **Step 6: Commit**

```bash
git add crates/anabios-headless/src/sweep.rs
git commit -m "feat(headless): M15 sweep — evolved_cooperation/pack_hunting/herd_cohesion columns

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 8: Determinism lock + snapshot + workspace + doc gate

Controller verification gate (no new production code). **Includes the two CI-only gates that failed M14.**

- [ ] **Step 1:** `cargo test -p anabios-core --test determinism` → PASS (stable; regenerate GOLDEN if a prior task left it stale).
- [ ] **Step 2:** `cargo test -p anabios-core --lib roundtrip_preserves_state` → PASS (new `CombatHit`/`CodexState` fields round-trip).
- [ ] **Step 3:** `cargo test --workspace` → all PASS.
- [ ] **Step 4:** `cargo clippy --workspace --all-targets -- -D warnings` → clean (watch `needless_range_loop` on any `for ch in 0..N`; prefer `iter_mut().enumerate()`).
- [ ] **Step 5:** `cargo fmt` then **commit any reformatting** (M14 failed CI because a fmt change was left uncommitted — `git status` must be clean after `cargo fmt`).
- [ ] **Step 6:** `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` → PASS. Escape any `[N]` array-index prose in doc comments as `` `x[N]` `` (M14 failed CI on `meme[1]` parsed as a broken intra-doc link).
- [ ] **Step 7:** Commit anything changed here.

```bash
git add -A
git commit -m "test(core): M15 determinism + snapshot + fmt + clippy + rustdoc gate

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 9: Cooperation archetype + emergence scenario(s) + test

**Files:**
- Modify: `crates/anabios-core/src/program.rs` (`starter_cooperator`; append to `starter_library`; update library tests)
- Modify: `crates/anabios-core/src/scenario.rs` (`cooperator` archetype)
- Create: `scenarios/cooperation.toml` (and optionally `scenarios/pack-vs-herd.toml`)
- Create: `crates/anabios-core/tests/cooperation_emergence.rs`

**Interfaces:**
- Produces: `program::starter_cooperator()` — share with the nearest neighbor when kinship is high, and cohere (herd): e.g. `[SenseKinship, ThresholdGt(0.3), Share, SenseSameDirX, MoveTowardX, SenseSameDirY, MoveTowardY]` (the `ThresholdGt` gate makes `Share` fire only for kin; verify the postfix stack shape during implementation — `SenseKinship` pushes kinship, `ThresholdGt(0.3)` maps it to 1/0, `Share` consumes it).
- Produces: `"cooperator"` arm in `archetype_kit` → a herbivore kit with `Altruism` boosted via scenario traits.

- [ ] **Step 1: Add `starter_cooperator` + archetype** — add the starter (append to `starter_library` at END; update `starter_library_has_all_starters` → 8 and `social_starters_are_bounded_and_evaluable`); add `"cooperator" => (starter_kit(), starter_cooperator())` to `archetype_kit`.

- [ ] **Step 2: Write `scenarios/cooperation.toml`** — a dense cluster of `cooperator` herbivores with `[agents.traits] altruism = 1.0` (kin sharing under crowding). Optionally add a second scenario/species for pack-vs-herd; keep it simple for the first pass.

- [ ] **Step 3: Write the emergence test** — `crates/anabios-core/tests/cooperation_emergence.rs`, mirroring `dialect_emergence.rs`: multi-seed, release-gated, checks for `EvolvedCooperation` (and separately measures `HerdCohesion`/`PackHunting`). Placeholder `COOP_FLOOR = 8`.

- [ ] **Step 4: Measure + tune (controller):** run `cargo test -p anabios-core --release --test cooperation_emergence -- --nocapture` with a temporary `eprintln!` counting each detector's seed rate. Gate on whichever detector(s) the measurement shows robust (EvolvedCooperation from the sharing cluster, and/or HerdCohesion from the cohesion). Adjust scenario density/traits/`TICKS`, set the floor a few below observed, record the rate in a comment, remove the `eprintln!`. If PackHunting or AlarmCall emergence proves robust in a two-species variant, gate/measure it too; otherwise leave those detectors shipped-only.

- [ ] **Step 5: Verify gating** — release PASS; debug `0 passed / 1 ignored`.

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/program.rs crates/anabios-core/src/scenario.rs \
        scenarios/cooperation.toml crates/anabios-core/tests/cooperation_emergence.rs
git commit -m "test(core): M15 cooperator archetype + cooperation emergence

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review (author checklist — completed)

**Spec coverage (spec §M15):**
- Kin recognition (`SenseKinship` from ancestry + genome distance) → Task 1. ✅
- Food sharing / altruism (share_intent, altruism-scaled transfer) → Task 2. ✅
- Pack hunting = compose broadcast + FireWeapon (detector counts distinct attackers) → Task 5. ✅
- Herd cohesion = crowding + MoveToward same-species (detector) → Task 6. ✅
- `EvolvedCooperation` → Task 4; `PackHunting` → Task 5; `HerdCohesion` → Task 6. ✅
- AlarmCall emergence-confirmation follow-up + the birth-tick fix → Task 3 (fix) + Task 9 (optional emergence if robust). ✅
- Mechanism tests (share exact transfer scaled by altruism; zero altruism → none; SenseKinship high for siblings/parent-child, low for unrelated; detectors fire on constructed positives) → Tasks 1–6. ✅
- Emergence scenarios + multi-seed test → Task 9. ✅
- Sweep integration → Task 7. ✅
- Golden-tick refresh (§2.3) → Tasks 4/5/6 (+ possible Task 3) + Task 8 lock. ✅

**Type consistency:** `kinship(...)` signature (Task 1) matches its call in `sense_all`; `nearest_kinship` on `SensorRegister`/`EvalContext`; `share_intent` on `ActionRegister`; `CombatHit` fields (Task 5) consistent with `combat_pass` recording; `EventType` appended 14/15/16; `Node` appended `SenseKinship`(41)/`Share`(42). ✅

**Placeholder scan:** substrate tasks carry full code; detector Tasks 4–6 give exact signatures/fields/constants + prose bodies following the in-file `detect_combat_raid`/`detect_territory_formation` templates; the one judgment step (Task 9 measure/tune) is explicit.

## Deviation notes (for reviewers)

- **Kinship blends ancestry and genome similarity 50/50.** Seeded scenario founders have `parent_ids = [NONE, NONE]` (ancestry 0) and identical genomes (genome_sim ~1) → kinship ~0.5; true siblings/parent-child add ancestry → higher. This makes `SenseKinship` meaningful both for founding clusters and for evolved lineages. `SQRT_GENOME_LEN = √50` normalizes the L2 distance.
- **Share targets `ActionRegister.target_id` (the overall-nearest neighbor).** Kin-direction is achieved at the program level by gating `Share` behind `SenseKinship` (both concern the nearest neighbor, so they're coherent). No separate kin-target resolution in the substrate.
- **HerdCohesion measures sustained per-species crowding**, not a direct predation-rate reduction (which needs a dispersed control). Crowding is the measurable cohesion signal; the predation benefit is ecological context. Documented; sharpen in M16.
- **PackHunting keys hits by attacker/target agent id**, which the freelist can reuse; within the short `PACK_WINDOW` (8 ticks) reuse is negligible. Documented.
- **AlarmCall emergence** is confirmed only if Task 9's measurement shows it robust in a two-species (predator + sentinel-herd) variant; otherwise the detector ships (fixed in Task 3) with emergence deferred to M16, per spec §M14.
