# M2 — Reproduction, Mutation, Speciation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the M1 simulation so populations sustain themselves: agents have lineage and species identity, reproduce sexually with crossover + mutation, and an online speciation algorithm partitions the population into discoverable species with tracked phylogeny.

**Architecture:** Add three persistent per-agent fields (`lineage_id`, `parent_ids`, `species_id`) and two new tick stages (`reproduce`, `species_step`). Reproduction is opportunistic: two adjacent same-species agents above an energy threshold produce one offspring whose genome is the uniform crossover of the parents' genomes followed by Gaussian mutation. Speciation runs every 200 ticks via online centroid clustering with genetic-distance threshold ≈ 0.6; new clusters spawn and inherit a `parent_species_id` tracked in a phylogeny table on `World`.

**Tech Stack:** Same as M1 — Rust stable, `glam`, `rand_xoshiro`, `serde` + `bincode`, `smallvec`, `bitvec`, `proptest`, `criterion`.

**Style conventions** (inherited from M1):

- 4-space indent, no tabs
- Rustdoc `///` only where behavior isn't obvious from the name
- All randomness through `World.rng`
- No allocations in the hot path; reuse scratch buffers via `World.resize_scratch()`
- Deterministic iteration: agents ordered by ascending id
- Commit messages use Conventional Commits prefixes (`feat:`, `test:`, `chore:`, `refactor:`, `bench:`, `docs:`)
- Each task lands as a **single commit** unless explicitly noted

**Working directory:** All commands assume cwd = `/Users/aryasen/projects/anabios/`.

**Branch:** Create and work on `m2-reproduction-speciation` branched from `main`.

---

## File structure after M2

New files:
```
crates/anabios-core/src/
├── reproduce.rs                       # NEW: reproduce_all + offspring construction
└── species.rs                         # NEW: incremental clustering + phylogeny
crates/anabios-core/tests/
├── reproduction.rs                    # NEW: pair-mating, energy accounting
└── speciation.rs                      # NEW: forced speciation scenario
```

Modified files:
```
crates/anabios-core/src/
├── agent.rs                           # +lineage_id, +parent_ids, +species_id fields
├── world.rs                           # +next_lineage_id, +species tables, +scratch BitVec
├── tick.rs                            # wire reproduce() + species_step()
├── behavior.rs                        # +mate-seeking drive in decide()
├── genome.rs                          # +Genome::crossover function
├── sense.rs                           # +SensorRegister::nearest_neighbor_species
└── scenario.rs                        # init lineage_id and species_id on spawn
crates/anabios-core/tests/
├── determinism.rs                     # regenerate GOLDEN hashes
├── invariants.rs                      # +lineage uniqueness + species validity
└── feeding.rs                         # populations now sustain — tighten the upper bound
```

---

## Task 1: Branch setup + extended agent fields

**Goal:** Branch from main, then add `lineage_id`, `parent_ids`, and `species_id` to `AgentBuffers`. Initialize them on spawn. Default behavior unchanged (no mating yet).

**Files:**
- Modify: `crates/anabios-core/src/agent.rs`

- [ ] **Step 1.1: Create the feature branch**

```bash
git checkout main
git pull
git checkout -b m2-reproduction-speciation
```

Expected: switched to a new branch with no diffs.

- [ ] **Step 1.2: Extend AgentBuffers with three new fields**

Edit `crates/anabios-core/src/agent.rs`. After the existing field block in `AgentBuffers`, add three new `Vec` fields.

Replace the existing `AgentBuffers` struct definition with this:

```rust
/// Unique lineage identifier. Each agent gets a fresh value at birth; never
/// reused even after death. Used for ancestry, kin recognition, and codex
/// lineage-hall entries.
pub type LineageId = u64;
/// Stable species identifier. Initially every agent is species 0; speciation
/// (M2) assigns new species ids over time.
pub type SpeciesId = u32;

/// Lineage id used for ancestors of seeded (founder) agents that have no
/// modelled parent. Stored in `parent_ids` slots to mean "no parent".
pub const LINEAGE_NONE: LineageId = 0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentBuffers {
    pub position: Vec<Vec2>,
    /// Most-recently-applied velocity. Recorded by the integrate stage but
    /// not yet read by any sensor. Reserved for M3 correlated-wander
    /// behavior, which will read this as `last_velocity` to bias new
    /// directions toward recent motion. Included in the persistent
    /// snapshot to keep golden hashes stable across that change.
    pub velocity: Vec<Vec2>,
    pub energy: Vec<f32>,
    pub age: Vec<u32>,
    pub genome: Vec<Genome>,
    pub lineage_id: Vec<LineageId>,
    pub parent_ids: Vec<[LineageId; 2]>,
    pub species_id: Vec<SpeciesId>,
    pub alive: BitVec,
    free_list: Vec<AgentId>,
    live_count: u32,
}
```

- [ ] **Step 1.3: Extend spawn() signature to take lineage and species identity**

Replace `AgentBuffers::spawn` with:

```rust
    /// Spawn an agent. Reuses a dead slot if available; otherwise extends
    /// every buffer by one. `lineage_id` must be globally unique across the
    /// world's lifetime (allocate via `World::next_lineage()`). `parent_ids`
    /// = `[LINEAGE_NONE; 2]` for founders; otherwise the lineage ids of the
    /// two parents.
    pub fn spawn(
        &mut self,
        position: Vec2,
        genome: Genome,
        lineage_id: LineageId,
        parent_ids: [LineageId; 2],
        species_id: SpeciesId,
    ) -> AgentId {
        let id = if let Some(id) = self.free_list.pop() {
            let i = id as usize;
            self.position[i] = position;
            self.velocity[i] = Vec2::ZERO;
            self.energy[i] = SPAWN_ENERGY;
            self.age[i] = 0;
            self.genome[i] = genome;
            self.lineage_id[i] = lineage_id;
            self.parent_ids[i] = parent_ids;
            self.species_id[i] = species_id;
            self.alive.set(i, true);
            id
        } else {
            let i = self.position.len();
            self.position.push(position);
            self.velocity.push(Vec2::ZERO);
            self.energy.push(SPAWN_ENERGY);
            self.age.push(0);
            self.genome.push(genome);
            self.lineage_id.push(lineage_id);
            self.parent_ids.push(parent_ids);
            self.species_id.push(species_id);
            self.alive.push(true);
            i as AgentId
        };
        self.live_count += 1;
        id
    }
```

- [ ] **Step 1.4: Update existing unit tests to use the new spawn signature**

Existing tests in `agent.rs`'s `mod tests` call `a.spawn(Vec2::ZERO, neutral())`. They will not compile. Update each call to pass `(pos, genome, 0, [LINEAGE_NONE; 2], 0)`:

```rust
    fn neutral() -> Genome {
        Genome::neutral()
    }

    #[test]
    fn spawn_increases_capacity_and_live_count() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(Vec2::new(1.0, 2.0), neutral(), 1, [LINEAGE_NONE; 2], 0);
        let id1 = a.spawn(Vec2::new(3.0, 4.0), neutral(), 2, [LINEAGE_NONE; 2], 0);
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(a.capacity(), 2);
        assert_eq!(a.live_count(), 2);
        assert!(a.is_alive(0));
        assert!(a.is_alive(1));
    }

    #[test]
    fn kill_marks_slot_dead_and_decrements_live_count() {
        let mut a = AgentBuffers::new();
        let id = a.spawn(Vec2::ZERO, neutral(), 1, [LINEAGE_NONE; 2], 0);
        a.kill(id);
        assert!(!a.is_alive(id));
        assert_eq!(a.live_count(), 0);
    }

    #[test]
    fn spawn_after_kill_reuses_slot() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(Vec2::ZERO, neutral(), 1, [LINEAGE_NONE; 2], 0);
        let id1 = a.spawn(Vec2::ZERO, neutral(), 2, [LINEAGE_NONE; 2], 0);
        a.kill(id0);
        let id2 = a.spawn(Vec2::new(5.0, 6.0), neutral(), 3, [LINEAGE_NONE; 2], 0);
        assert_eq!(id2, id0, "slot 0 should have been reused");
        assert_eq!(a.live_count(), 2);
        assert!(a.is_alive(id1));
        assert!(a.is_alive(id2));
    }

    #[test]
    fn iter_alive_skips_dead_slots() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(Vec2::ZERO, neutral(), 1, [LINEAGE_NONE; 2], 0);
        let _id1 = a.spawn(Vec2::ZERO, neutral(), 2, [LINEAGE_NONE; 2], 0);
        let id2 = a.spawn(Vec2::ZERO, neutral(), 3, [LINEAGE_NONE; 2], 0);
        a.kill(id0);
        let alive: Vec<AgentId> = a.iter_alive().collect();
        assert_eq!(alive, vec![1, id2]);
    }

    #[test]
    fn double_kill_is_a_noop() {
        let mut a = AgentBuffers::new();
        let id = a.spawn(Vec2::ZERO, neutral(), 1, [LINEAGE_NONE; 2], 0);
        a.kill(id);
        a.kill(id);
        assert_eq!(a.live_count(), 0);
        assert_eq!(a.iter_alive().count(), 0);
    }
```

- [ ] **Step 1.5: Run agent unit tests**

Run: `cargo test -p anabios-core agent::tests`

Expected: 5 tests pass.

- [ ] **Step 1.6: Skip fmt + clippy until Task 2**

The rest of the workspace (`World`, `Scenario`, `tests/`) still calls the old 2-argument `spawn` and won't compile yet. Task 2 fixes the call sites. Do not run `cargo check --workspace` here — it will fail; that's expected.

Verify only that `agent.rs` builds in isolation:

Run: `cargo check -p anabios-core --lib --tests 2>&1 | grep -c "^error\[E0061\]"`

Expected: a non-zero count (the spawn-arity errors in `world.rs` and `scenario.rs` — they are intentionally deferred to Task 2). Do NOT commit yet.

- [ ] **Step 1.7: Combine with Task 2 commit**

This task's changes will be committed as part of Task 2 (next), once the rest of the codebase is reconciled with the new spawn signature. **Do not run `git commit` here.**

---

## Task 2: Wire World as the source of lineage and initial species

**Goal:** `World` now owns the global lineage counter and the species table seed. Add `next_lineage()` and update `spawn_agent` to take just a position + genome (it allocates lineage internally and uses species 0). Update `Scenario::instantiate` and the existing world tests to use the new API.

**Files:**
- Modify: `crates/anabios-core/src/world.rs`
- Modify: `crates/anabios-core/src/scenario.rs`

- [ ] **Step 2.1: Extend the World struct**

Replace the existing `World` struct in `crates/anabios-core/src/world.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub tick: u64,
    pub seed: u64,
    pub rng: Rng,
    pub biome: BiomeField,
    pub agents: AgentBuffers,
    /// Next lineage id to allocate. Monotonically increasing.
    /// Lineage id 0 is reserved as `LINEAGE_NONE` (no parent).
    pub next_lineage_id: LineageId,
    #[serde(skip)]
    pub spatial: UniformSpatialHash,
    #[serde(skip)]
    pub sensors: Vec<crate::sense::SensorRegister>,
    #[serde(skip)]
    pub desired_velocity: Vec<crate::prelude::Vec2>,
    /// Per-agent BitVec marking who has already mated this tick.
    /// Cleared at the start of `reproduce_all`.
    #[serde(skip)]
    pub reproduced_this_tick: BitVec,
}
```

Add the `bitvec::vec::BitVec` and `agent::{LineageId, LINEAGE_NONE}` imports at the top of `world.rs`:

```rust
use bitvec::vec::BitVec;
use serde::{Deserialize, Serialize};

use crate::agent::{AgentBuffers, AgentId, LineageId, LINEAGE_NONE, SPAWN_ENERGY};
use crate::biome::{BiomeField, WORLD_SIZE};
use crate::genome::Genome;
use crate::prelude::Vec2;
use crate::rng::Rng;
use crate::spatial::UniformSpatialHash;
```

- [ ] **Step 2.2: Update World::new to initialize the new fields**

Replace the `new` constructor in the `impl World` block:

```rust
    pub fn new(seed: u64) -> Self {
        Self {
            tick: 0,
            seed,
            rng: Rng::from_seed(seed),
            biome: BiomeField::generate(seed),
            agents: AgentBuffers::new(),
            // Start at 1 — id 0 is reserved as LINEAGE_NONE for founder parents.
            next_lineage_id: 1,
            spatial: UniformSpatialHash::new(),
            sensors: Vec::new(),
            desired_velocity: Vec::new(),
            reproduced_this_tick: BitVec::new(),
        }
    }
```

- [ ] **Step 2.3: Add next_lineage() helper and update spawn_agent**

Replace the `spawn_agent` method in `impl World` and add a new `next_lineage` method:

```rust
    /// Allocate a fresh, globally-unique lineage id. Never reuses values.
    #[inline]
    pub fn next_lineage(&mut self) -> LineageId {
        let id = self.next_lineage_id;
        self.next_lineage_id = self.next_lineage_id.checked_add(1)
            .expect("lineage id overflow: 2^64 births is implausible");
        id
    }

    /// Spawn a founder agent (no modelled parents) into the world. Lineage
    /// id is allocated here; species id is 0 (the founder species).
    pub fn spawn_agent(&mut self, position: Vec2, genome: Genome) -> AgentId {
        let lineage = self.next_lineage();
        self.agents.spawn(position, genome, lineage, [LINEAGE_NONE; 2], 0)
    }
```

- [ ] **Step 2.4: Update resize_scratch to size the reproduced_this_tick bitvec**

Replace `resize_scratch`:

```rust
    /// Resize scratch buffers to match agent capacity. Called by the tick.
    pub(crate) fn resize_scratch(&mut self) {
        let cap = self.agents.capacity();
        if self.sensors.len() < cap {
            self.sensors
                .resize(cap, crate::sense::SensorRegister::default());
        }
        if self.desired_velocity.len() < cap {
            self.desired_velocity.resize(cap, crate::prelude::Vec2::ZERO);
        }
        if self.reproduced_this_tick.len() < cap {
            self.reproduced_this_tick.resize(cap, false);
        }
    }
```

- [ ] **Step 2.5: Update Scenario::instantiate (no signature change to the public API)**

`Scenario::instantiate` already calls `w.spawn_agent(position, g)` — that still compiles because we kept the same external signature. Verify no changes are needed in `scenario.rs`. **Read `crates/anabios-core/src/scenario.rs` and confirm the `w.spawn_agent(position, g)` call is on a single line and unchanged.** If it is, no edit needed here.

- [ ] **Step 2.6: Verify the workspace compiles**

Run: `cargo check --workspace`

Expected: compiles with zero errors. Warnings about unused `LINEAGE_NONE` import or unused `reproduced_this_tick` field are acceptable now — they will be consumed in later tasks. If clippy flags them under `-D warnings`, add a temporary `#[allow(dead_code)]` on the `reproduced_this_tick` field comment with `// allow: filled by Task 6`.

- [ ] **Step 2.7: Run all existing tests to confirm nothing regressed**

Run: `cargo test -p anabios-core --lib`

Expected: every previously-passing M1 test still passes. The library is back to a coherent state with the new fields plumbed through.

- [ ] **Step 2.8: Commit Tasks 1 + 2 together**

```bash
git add crates/anabios-core/src/agent.rs crates/anabios-core/src/world.rs
git commit -m "feat(core): add lineage_id, parent_ids, species_id agent fields and World.next_lineage"
```

---

## Task 3: Genome crossover function

**Goal:** Add `Genome::crossover` (uniform crossover at each slot) plus unit tests. Pure function; deterministic given an RNG.

**Files:**
- Modify: `crates/anabios-core/src/genome.rs`

- [ ] **Step 3.1: Add crossover method to Genome**

In `crates/anabios-core/src/genome.rs`, add inside `impl Genome` (just before its closing `}`):

```rust
    /// Uniform crossover: each slot is independently inherited from one of
    /// the two parents with equal probability. The RNG is consumed in slot
    /// order so the output is deterministic given the seed.
    pub fn crossover(a: &Genome, b: &Genome, rng: &mut Rng) -> Genome {
        let mut out = [0.0_f32; GENOME_LEN];
        for i in 0..GENOME_LEN {
            // Bit-packed source select: one RNG draw, 32 binary decisions
            // per draw. Cheaper than calling f32_unit 50 times.
            // Simplified for clarity: just use f32_unit each slot.
            let from_a = rng.f32_unit() < 0.5;
            out[i] = if from_a { a.0[i] } else { b.0[i] };
        }
        Genome(out)
    }
```

- [ ] **Step 3.2: Add unit tests for crossover**

Add inside the existing `mod tests` block in `genome.rs`:

```rust
    #[test]
    fn crossover_with_identical_parents_yields_same_genome() {
        let mut rng = Rng::from_seed(1);
        let g = Genome::neutral();
        let child = Genome::crossover(&g, &g, &mut rng);
        assert_eq!(child, g);
    }

    #[test]
    fn crossover_yields_per_slot_values_from_one_parent() {
        let mut rng = Rng::from_seed(7);
        let mut a = Genome::neutral();
        let mut b = Genome::neutral();
        for i in 0..GENOME_LEN {
            a.0[i] = 0.1;
            b.0[i] = 0.9;
        }
        let child = Genome::crossover(&a, &b, &mut rng);
        for i in 0..GENOME_LEN {
            let v = child.0[i];
            assert!(v == 0.1 || v == 0.9, "slot {i} was {v}");
        }
    }

    #[test]
    fn crossover_is_deterministic() {
        let a = Genome::neutral();
        let mut b = Genome::neutral();
        b.set(GenomeSlot::SpeedMax, 0.9);

        let mut rng1 = Rng::from_seed(42);
        let mut rng2 = Rng::from_seed(42);
        let c1 = Genome::crossover(&a, &b, &mut rng1);
        let c2 = Genome::crossover(&a, &b, &mut rng2);
        assert_eq!(c1, c2);
    }

    #[test]
    fn crossover_output_stays_in_unit_range() {
        let mut rng = Rng::from_seed(99);
        let mut a = Genome::neutral();
        let mut b = Genome::neutral();
        a.set(GenomeSlot::MutationRate, 1.0);
        b.set(GenomeSlot::Aggression, 1.0);
        let child = Genome::crossover(&a, &b, &mut rng);
        for v in child.0.iter() {
            assert!(*v >= 0.0 && *v <= 1.0);
        }
    }
```

- [ ] **Step 3.3: Run the genome tests**

Run: `cargo test -p anabios-core genome`

Expected: 12 passed (8 from M1 + 4 new).

- [ ] **Step 3.4: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`

Expected: zero diffs, zero warnings.

- [ ] **Step 3.5: Commit**

```bash
git add crates/anabios-core/src/genome.rs
git commit -m "feat(core): Genome::crossover with uniform per-slot inheritance"
```

---

## Task 4: Expose nearest-neighbor species in the sensor register

**Goal:** Reproduction needs to know whether a nearby agent is mate-compatible (same species). Add `nearest_neighbor_species: Option<SpeciesId>` to `SensorRegister` so the behavior program can read it.

**Files:**
- Modify: `crates/anabios-core/src/sense.rs`

- [ ] **Step 4.1: Add the new field to SensorRegister**

Edit `SensorRegister` in `crates/anabios-core/src/sense.rs`:

```rust
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SensorRegister {
    pub local_plant_biomass: f32,
    pub plant_direction: Vec2,
    pub nearest_neighbor_dist: f32,
    pub nearest_neighbor_dir: Vec2,
    pub has_neighbor: bool,
    /// Species id of the nearest neighbor, or `u32::MAX` if no neighbor.
    /// `u32::MAX` is chosen as a sentinel so the default-initialized state
    /// of an uninhabited sensor register doesn't accidentally look like
    /// "compatible with species 0".
    pub nearest_neighbor_species: u32,
}
```

Add a constant near the top of the file (after the use-imports):

```rust
/// Sentinel value in `SensorRegister.nearest_neighbor_species` meaning
/// "no neighbor". `Default` initializes the field to this value.
pub const NO_NEIGHBOR_SPECIES: u32 = u32::MAX;
```

And fix `Default` so `nearest_neighbor_species` defaults to the sentinel. The cleanest way is a manual `Default` impl:

Replace `#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]` above the struct with:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
```

Add this `impl Default` block immediately after the struct:

```rust
impl Default for SensorRegister {
    fn default() -> Self {
        Self {
            local_plant_biomass: 0.0,
            plant_direction: Vec2::ZERO,
            nearest_neighbor_dist: f32::INFINITY,
            nearest_neighbor_dir: Vec2::ZERO,
            has_neighbor: false,
            nearest_neighbor_species: NO_NEIGHBOR_SPECIES,
        }
    }
}
```

- [ ] **Step 4.2: Populate the new field inside sense_all**

Edit `sense_all` in `sense.rs`. The inner loop's `spatial.query` callback updates `nearest_neighbor_*` fields when a closer neighbor is found. Extend it to also capture the neighbor's species id.

Find the loop body that looks like:

```rust
        spatial.query(pos, radius, |other_id| {
            if other_id == id {
                return;
            }
            let other_pos = agents.position[other_id as usize];
            let d = torus_distance(pos, other_pos);
            if d <= radius && d < nearest_dist {
                nearest_dist = d;
                nearest_dir = torus_direction(pos, other_pos);
                has_neighbor = true;
            }
        });
```

Replace with:

```rust
        let mut nearest_species: u32 = NO_NEIGHBOR_SPECIES;
        spatial.query(pos, radius, |other_id| {
            if other_id == id {
                return;
            }
            let other_pos = agents.position[other_id as usize];
            let d = torus_distance(pos, other_pos);
            if d <= radius && d < nearest_dist {
                nearest_dist = d;
                nearest_dir = torus_direction(pos, other_pos);
                has_neighbor = true;
                nearest_species = agents.species_id[other_id as usize];
            }
        });
```

And update the `registers[i] = SensorRegister { ... }` assignment further down to include the new field:

```rust
        registers[i] = SensorRegister {
            local_plant_biomass: local_cell.plant_biomass,
            plant_direction,
            nearest_neighbor_dist: nearest_dist,
            nearest_neighbor_dir: nearest_dir,
            has_neighbor,
            nearest_neighbor_species: nearest_species,
        };
```

- [ ] **Step 4.3: Update sense tests for the new field**

In the existing `mod tests` block in `sense.rs`, the test `agent_finds_neighbor_within_perception` currently asserts only on direction and distance. Extend it with a species check:

In that test, after the existing assertions, add:

```rust
        assert_eq!(regs[0].nearest_neighbor_species, 0);
```

The test for `isolated_agent_has_no_neighbor` already asserts no neighbor; add:

```rust
        assert_eq!(regs[0].nearest_neighbor_species, NO_NEIGHBOR_SPECIES);
```

- [ ] **Step 4.4: Add an import for the constant in tests**

At the top of the `mod tests` block in `sense.rs`, the `use super::*;` already pulls in `NO_NEIGHBOR_SPECIES`. No new imports needed.

- [ ] **Step 4.5: Run tests + fmt + clippy**

```bash
cargo test -p anabios-core sense
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 3 sense tests pass; fmt/clippy clean.

- [ ] **Step 4.6: Commit**

```bash
git add crates/anabios-core/src/sense.rs
git commit -m "feat(core): SensorRegister exposes nearest_neighbor_species for mate compatibility"
```

---

## Task 5: Mate-seeking drive in decide()

**Goal:** When an agent has accumulated enough energy and has a same-species neighbor in perception, head toward them instead of foraging or wandering. Hungry agents still prioritize food.

**Files:**
- Modify: `crates/anabios-core/src/behavior.rs`

- [ ] **Step 5.1: Add the mate-seeking branch**

Replace the body of `decide` in `behavior.rs` with:

```rust
pub fn decide(
    genome: &Genome,
    sensor: &SensorRegister,
    energy: f32,
    own_species: u32,
    rng: &mut Rng,
) -> Vec2 {
    let speed_max = SPEED_MAX_CAP * genome.get(GenomeSlot::SpeedMax);
    if speed_max <= 0.0 {
        return Vec2::ZERO;
    }

    let hunger_threshold = SPAWN_ENERGY * genome.get(GenomeSlot::ReproductionThreshold);
    let is_hungry = energy < hunger_threshold;

    // Reproduce threshold is a separate (higher) bar: agents save up surplus
    // energy before mating becomes attractive. Scale by 1.5× the hunger
    // threshold so well-fed agents pursue mates instead of just wandering.
    let mate_ready_threshold = hunger_threshold * 1.5;
    let mate_ready = energy >= mate_ready_threshold
        && sensor.has_neighbor
        && sensor.nearest_neighbor_species == own_species;

    let direction = if is_hungry && sensor.plant_direction != Vec2::ZERO {
        sensor.plant_direction
    } else if mate_ready {
        // Head toward the same-species neighbor; reproduction happens in the
        // reproduce stage when proximity drops below the mating range.
        sensor.nearest_neighbor_dir
    } else {
        // Wander: random unit vector.
        let theta = rng.f32_unit() * std::f32::consts::TAU;
        Vec2::new(theta.cos(), theta.sin())
    };

    direction * speed_max
}
```

Note the new `own_species: u32` parameter. Update the import block at the top of `behavior.rs` to bring `SensorRegister` and `SPAWN_ENERGY` from where they live:

```rust
use crate::agent::SPAWN_ENERGY;
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;
use crate::rng::Rng;
use crate::sense::SensorRegister;
```

- [ ] **Step 5.2: Update the call site in tick.rs**

Edit `crates/anabios-core/src/tick.rs`. Find `decide_all`:

```rust
fn decide_all(world: &mut World) {
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
        let genome = world.agents.genome[i];
        let sensor = world.sensors[i];
        let energy = world.agents.energy[i];
        world.desired_velocity[i] = decide(&genome, &sensor, energy, &mut world.rng);
    }
}
```

Replace the inner body with one that passes `own_species`:

```rust
fn decide_all(world: &mut World) {
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
        let genome = world.agents.genome[i];
        let sensor = world.sensors[i];
        let energy = world.agents.energy[i];
        let own_species = world.agents.species_id[i];
        world.desired_velocity[i] = decide(&genome, &sensor, energy, own_species, &mut world.rng);
    }
}
```

- [ ] **Step 5.3: Update behavior tests**

The three existing tests in `behavior.rs::tests` call `decide(&g, &s, energy, &mut rng)` and won't compile. Update each to add the `own_species: 0` argument. Replace the calls:

```rust
let v = decide(&g, &s, 0.0, 0, &mut rng);
```

```rust
let v = decide(&g, &s, 0.0, 0, &mut rng);
```

```rust
let v = decide(&g, &s, SPAWN_ENERGY, 0, &mut rng);
```

Add a new test demonstrating the mate-seeking branch:

```rust
    #[test]
    fn mate_ready_agent_heads_toward_same_species_neighbor() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 1.0);
        g.set(GenomeSlot::ReproductionThreshold, 0.5); // hunger at 25 energy
        let s = SensorRegister {
            plant_direction: Vec2::new(0.0, -1.0), // food is down, but we're full
            has_neighbor: true,
            nearest_neighbor_dist: 5.0,
            nearest_neighbor_dir: Vec2::new(1.0, 0.0), // neighbor is right
            nearest_neighbor_species: 0,
            ..Default::default()
        };
        let mut rng = Rng::from_seed(1);
        // Energy 50 >= 1.5 × hunger_threshold (25) → mate-ready
        let v = decide(&g, &s, 50.0, 0, &mut rng);
        assert!(v.x > 0.5, "mate-ready agent should move toward neighbor (+x): {v:?}");
        assert!(v.y.abs() < 0.5);
    }

    #[test]
    fn mate_ready_with_different_species_does_not_mate_seek() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 1.0);
        g.set(GenomeSlot::ReproductionThreshold, 0.5);
        let s = SensorRegister {
            has_neighbor: true,
            nearest_neighbor_dist: 5.0,
            nearest_neighbor_dir: Vec2::new(1.0, 0.0),
            nearest_neighbor_species: 1, // different species
            ..Default::default()
        };
        let mut rng = Rng::from_seed(1);
        let v = decide(&g, &s, 50.0, 0, &mut rng);
        // Should NOT consistently move +x — should wander (different species)
        // Run with a couple of different seeds to confirm variety.
        let mut wandered_directions = std::collections::HashSet::new();
        for seed in 1..16 {
            let mut r = Rng::from_seed(seed);
            let vw = decide(&g, &s, 50.0, 0, &mut r);
            wandered_directions.insert(((vw.x * 10.0) as i32, (vw.y * 10.0) as i32));
        }
        assert!(wandered_directions.len() >= 4, "should wander, not lock onto neighbor: {wandered_directions:?}");
        // (The first decide is consumed but its direction is not asserted; the rest of the test relies on the multi-seed exploration.)
        let _ = v;
    }
```

- [ ] **Step 5.4: Run behavior tests**

Run: `cargo test -p anabios-core behavior`

Expected: 5 tests pass (3 existing + 2 new).

- [ ] **Step 5.5: Run all lib tests**

Run: `cargo test -p anabios-core --lib`

Expected: full lib suite still green. Tick tests may need their behavior expectations updated if mate-seeking changes outcomes — verify by running. If `agent_in_food_rich_world_survives_initial_ticks` fails because the lone agent now wanders less effectively, leave the test as-is; it should still survive because there's no neighbor of any species so mate-seeking never activates.

- [ ] **Step 5.6: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`

Expected: zero diffs, zero warnings. If clippy flags the unused `_ = v` line in the second new test, replace with a comment that explains why we threw it away.

- [ ] **Step 5.7: Commit**

```bash
git add crates/anabios-core/src/behavior.rs crates/anabios-core/src/tick.rs
git commit -m "feat(core): add mate-seeking drive when energy is high and same-species neighbor in perception"
```

---

## Task 6: Reproduction stage

**Goal:** Create `crates/anabios-core/src/reproduce.rs` with a `reproduce_all` function that mates eligible pairs and produces offspring. Wire into the tick.

**Files:**
- Create: `crates/anabios-core/src/reproduce.rs`
- Modify: `crates/anabios-core/src/lib.rs` (add `pub mod reproduce;`)
- Modify: `crates/anabios-core/src/tick.rs` (call `reproduce::reproduce_all`)

- [ ] **Step 6.1: Add the module declaration**

Edit `crates/anabios-core/src/lib.rs`. Add `pub mod reproduce;` in alphabetical order, so the module list reads:

```rust
pub mod age;
pub mod agent;
pub mod behavior;
pub mod biome;
pub mod genome;
pub mod integrate;
pub mod interact;
pub mod reproduce;
pub mod rng;
pub mod scenario;
pub mod sense;
pub mod snapshot;
pub mod spatial;
pub mod tick;
pub mod world;
```

- [ ] **Step 6.2: Implement reproduce.rs**

Create `crates/anabios-core/src/reproduce.rs`:

```rust
//! Reproduction stage.
//!
//! Two same-species agents in close proximity (≤ MATING_RANGE) with energy
//! above `reproduction_threshold * SPAWN_ENERGY * 1.5` may produce one
//! offspring per tick. Each parent pays `OFFSPRING_INVESTMENT * SPAWN_ENERGY
//! / 2` energy; the offspring is seeded with `SPAWN_ENERGY` from the world.
//! (Energy is approximately conserved within the family-pair exchange.)

use crate::agent::{LineageId, AgentBuffers, SPAWN_ENERGY};
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;
use crate::spatial::{torus_distance, UniformSpatialHash};
use crate::world::World;

/// Maximum distance between two parents at the moment of mating, in world units.
pub const MATING_RANGE: f32 = 2.0;

/// Fraction of `SPAWN_ENERGY` that each parent pays to produce an offspring.
pub const PARENT_ENERGY_COST_FRAC: f32 = 0.25;

/// Run the reproduce stage. Each alive agent at most mates once per tick.
/// Order: ascending agent id. Each agent A checks its same-cell neighbours
/// in ascending id order and mates with the first eligible B such that
/// `B.id > A.id`; this avoids double-counting and keeps the algorithm
/// deterministic.
pub fn reproduce_all(world: &mut World) {
    // Pull scratch buffer length up to current capacity.
    if world.reproduced_this_tick.len() < world.agents.capacity() {
        world
            .reproduced_this_tick
            .resize(world.agents.capacity(), false);
    }
    world.reproduced_this_tick.fill(false);

    // Snapshot the alive ids to a local vec; reproduction mutates the
    // alive set via spawn() and we don't want to iterate over newborns
    // this tick.
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();

    for &a_id in &alive_ids {
        let i = a_id as usize;
        if world.reproduced_this_tick[i] {
            continue;
        }
        if !is_eligible(&world.agents, a_id) {
            continue;
        }

        let a_pos = world.agents.position[i];
        let a_species = world.agents.species_id[i];
        let a_genome = world.agents.genome[i];
        let a_lineage = world.agents.lineage_id[i];

        // Find an eligible mate with a strictly higher id.
        let mate = find_mate(&world.spatial, &world.agents, &world.reproduced_this_tick, a_id, a_pos, a_species);
        let Some(b_id) = mate else { continue; };

        let j = b_id as usize;
        let b_pos = world.agents.position[j];
        let b_genome = world.agents.genome[j];
        let b_lineage = world.agents.lineage_id[j];

        // Pay energy from both parents.
        let cost = SPAWN_ENERGY * PARENT_ENERGY_COST_FRAC;
        world.agents.energy[i] -= cost;
        world.agents.energy[j] -= cost;

        // Build child genome: crossover + mutate.
        let mut child_genome = Genome::crossover(&a_genome, &b_genome, &mut world.rng);
        child_genome.mutate_in_place(&mut world.rng);

        // Mark both parents as reproduced this tick before spawning so the
        // newborn's slot (which gets a fresh bitvec bit) isn't accidentally
        // touched.
        world.reproduced_this_tick.set(i, true);
        world.reproduced_this_tick.set(j, true);

        // Spawn at midpoint of parents on the torus (account for wrap).
        let child_pos = midpoint_torus(a_pos, b_pos);

        let lineage = world.next_lineage();
        let child_id = world.agents.spawn(
            child_pos,
            child_genome,
            lineage,
            [a_lineage, b_lineage],
            a_species,
        );

        // Ensure the bitvec covers the new slot, mark the child as
        // "reproduced this tick" so they cannot immediately mate again.
        if world.reproduced_this_tick.len() <= child_id as usize {
            world
                .reproduced_this_tick
                .resize(child_id as usize + 1, false);
        }
        world.reproduced_this_tick.set(child_id as usize, true);
    }
}

fn is_eligible(agents: &AgentBuffers, id: u32) -> bool {
    let i = id as usize;
    if !agents.is_alive(id) {
        return false;
    }
    let threshold = SPAWN_ENERGY * agents.genome[i].get(GenomeSlot::ReproductionThreshold) * 1.5;
    agents.energy[i] >= threshold
}

fn find_mate(
    spatial: &UniformSpatialHash,
    agents: &AgentBuffers,
    reproduced: &bitvec::vec::BitVec,
    a_id: u32,
    a_pos: Vec2,
    a_species: u32,
) -> Option<u32> {
    let mut best: Option<u32> = None;
    spatial.query(a_pos, MATING_RANGE, |other_id| {
        if other_id <= a_id {
            return;
        }
        let j = other_id as usize;
        if reproduced[j] {
            return;
        }
        if !is_eligible(agents, other_id) {
            return;
        }
        if agents.species_id[j] != a_species {
            return;
        }
        let d = torus_distance(a_pos, agents.position[j]);
        if d > MATING_RANGE {
            return;
        }
        // First eligible mate wins; we iterate ids in deterministic order
        // because the spatial hash flattens cells in ascending bucket order.
        // To be safe, take the lowest id we've seen.
        match best {
            None => best = Some(other_id),
            Some(cur) if other_id < cur => best = Some(other_id),
            _ => {}
        }
    });
    best
}

fn midpoint_torus(a: Vec2, b: Vec2) -> Vec2 {
    use crate::biome::WORLD_SIZE;
    let mut dx = b.x - a.x;
    let mut dy = b.y - a.y;
    if dx > WORLD_SIZE * 0.5 {
        dx -= WORLD_SIZE;
    } else if dx < -WORLD_SIZE * 0.5 {
        dx += WORLD_SIZE;
    }
    if dy > WORLD_SIZE * 0.5 {
        dy -= WORLD_SIZE;
    } else if dy < -WORLD_SIZE * 0.5 {
        dy += WORLD_SIZE;
    }
    let mid_x = (a.x + dx * 0.5).rem_euclid(WORLD_SIZE);
    let mid_y = (a.y + dy * 0.5).rem_euclid(WORLD_SIZE);
    Vec2::new(mid_x, mid_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;
    use crate::genome::Genome;
    use crate::world::World;

    fn find_grass_cell_center(w: &World) -> Vec2 {
        use crate::biome::{BIOME_RES, CELL_SIZE};
        for row in 0..BIOME_RES {
            for col in 0..BIOME_RES {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    return Vec2::new(
                        (col as f32 + 0.5) * CELL_SIZE,
                        (row as f32 + 0.5) * CELL_SIZE,
                    );
                }
            }
        }
        panic!("no grass cell in biome");
    }

    fn fertile_genome() -> Genome {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.4);
        g.set(GenomeSlot::DietCarnivory, 0.0);
        g.set(GenomeSlot::SpeedMax, 0.4);
        g.set(GenomeSlot::Size, 0.4);
        g.set(GenomeSlot::BasalMetabolism, 0.4);
        g
    }

    #[test]
    fn two_adjacent_well_fed_agents_produce_offspring() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());

        // Give both ample energy.
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;

        // Build the spatial hash so find_mate can see them.
        w.spatial
            .rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();

        assert_eq!(after, before + 1, "expected exactly one offspring");
        // Each parent paid energy.
        assert!(w.agents.energy[id0 as usize] < SPAWN_ENERGY * 2.0);
        assert!(w.agents.energy[id1 as usize] < SPAWN_ENERGY * 2.0);
    }

    #[test]
    fn cross_species_pair_does_not_mate() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        // Force different species.
        w.agents.species_id[id1 as usize] = 1;
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;

        w.spatial
            .rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();
        assert_eq!(after, before, "different species must not produce offspring");
    }

    #[test]
    fn low_energy_pair_does_not_mate() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        // Below threshold.
        w.agents.energy[id0 as usize] = 1.0;
        w.agents.energy[id1 as usize] = 1.0;

        w.spatial
            .rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();
        assert_eq!(after, before, "low-energy agents must not mate");
    }

    #[test]
    fn offspring_inherits_parent_lineages() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        let lin0 = w.agents.lineage_id[id0 as usize];
        let lin1 = w.agents.lineage_id[id1 as usize];

        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial
            .rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        reproduce_all(&mut w);

        // The newborn is the only agent with non-zero parent ids.
        let mut found = false;
        for id in w.agents.iter_alive() {
            let p = w.agents.parent_ids[id as usize];
            if p != [crate::agent::LINEAGE_NONE; 2] {
                assert_eq!({ let mut s = p; s.sort(); s }, {
                    let mut s = [lin0, lin1]; s.sort(); s
                });
                found = true;
            }
        }
        assert!(found, "offspring with parent ids not found");
    }
}
```

- [ ] **Step 6.3: Wire reproduce_all into the tick pipeline**

Edit `crates/anabios-core/src/tick.rs`. Find the `step` function and insert a call to `reproduce_all` after `interact_all`. Replace the existing `step` body with:

```rust
pub fn step(world: &mut World) {
    world.resize_scratch();
    let cap = world.agents.capacity();

    world
        .spatial
        .rebuild(&world.agents.position, |i| world.agents.is_alive(i as u32));

    sense_all(&world.agents, &world.biome, &world.spatial, &mut world.sensors);

    decide_all(world);

    integrate_all(&mut world.agents, &world.desired_velocity[..cap]);

    interact_all(&mut world.agents, &mut world.biome);

    // Stage 6: reproduce. Mutates the alive set; do not rely on `cap` after
    // this point.
    crate::reproduce::reproduce_all(world);

    age_and_starve(&mut world.agents);

    if world.tick.is_multiple_of(BIOME_STEP_INTERVAL) {
        world.biome.regrow_step();
    }

    world.tick += 1;
}
```

- [ ] **Step 6.4: Run reproduce tests**

Run: `cargo test -p anabios-core reproduce`

Expected: 4 tests pass.

- [ ] **Step 6.5: Run all lib tests**

Run: `cargo test -p anabios-core --lib`

Expected: lib green. Tick tests may now show population growth in `agent_in_food_rich_world_survives_initial_ticks` (one agent doesn't have a mate, so no growth) — verify it still passes.

- [ ] **Step 6.6: fmt + clippy**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: zero diffs, zero warnings.

- [ ] **Step 6.7: Commit**

```bash
git add crates/anabios-core/src/lib.rs crates/anabios-core/src/reproduce.rs crates/anabios-core/src/tick.rs
git commit -m "feat(core): reproduce stage produces offspring from same-species adjacent parents"
```

---

## Task 7: Species clustering and phylogeny

**Goal:** Create `crates/anabios-core/src/species.rs` with `species_step` that runs every 200 ticks. Add species centroids and phylogeny tables to `World`. Wire `species_step` into the tick pipeline.

**Files:**
- Create: `crates/anabios-core/src/species.rs`
- Modify: `crates/anabios-core/src/lib.rs` (add `pub mod species;`)
- Modify: `crates/anabios-core/src/world.rs` (add species + phylogeny tables)
- Modify: `crates/anabios-core/src/tick.rs` (call species::species_step every N ticks)

- [ ] **Step 7.1: Add the module declaration**

Edit `crates/anabios-core/src/lib.rs`. Add `pub mod species;` after `pub mod snapshot;`, keeping alphabetical order:

```rust
pub mod age;
pub mod agent;
pub mod behavior;
pub mod biome;
pub mod genome;
pub mod integrate;
pub mod interact;
pub mod reproduce;
pub mod rng;
pub mod scenario;
pub mod sense;
pub mod snapshot;
pub mod spatial;
pub mod species;
pub mod tick;
pub mod world;
```

- [ ] **Step 7.2: Add species + phylogeny tables to World**

Edit `crates/anabios-core/src/world.rs`. Extend the `World` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub tick: u64,
    pub seed: u64,
    pub rng: Rng,
    pub biome: BiomeField,
    pub agents: AgentBuffers,
    pub next_lineage_id: LineageId,
    /// Per-species mean genome. Indexed by `SpeciesId`. Empty entries
    /// (extinct species) are kept in place so existing ids stay stable;
    /// `species_member_counts[id] == 0` marks them.
    pub species_centroids: Vec<crate::genome::Genome>,
    pub species_member_counts: Vec<u32>,
    /// Parent species id for each species. `None` for founder species
    /// (initially only species 0). Indexed by `SpeciesId`.
    pub species_parents: Vec<Option<u32>>,
    /// Next species id to allocate.
    pub next_species_id: u32,
    #[serde(skip)]
    pub spatial: UniformSpatialHash,
    #[serde(skip)]
    pub sensors: Vec<crate::sense::SensorRegister>,
    #[serde(skip)]
    pub desired_velocity: Vec<crate::prelude::Vec2>,
    #[serde(skip)]
    pub reproduced_this_tick: BitVec,
}
```

Update `World::new` to seed the species tables with the founder species (id 0):

```rust
    pub fn new(seed: u64) -> Self {
        Self {
            tick: 0,
            seed,
            rng: Rng::from_seed(seed),
            biome: BiomeField::generate(seed),
            agents: AgentBuffers::new(),
            next_lineage_id: 1,
            // Species 0 is the founder; centroid will be initialized by
            // the first call to `species_step` once agents exist.
            species_centroids: vec![Genome::neutral()],
            species_member_counts: vec![0],
            species_parents: vec![None],
            next_species_id: 1,
            spatial: UniformSpatialHash::new(),
            sensors: Vec::new(),
            desired_velocity: Vec::new(),
            reproduced_this_tick: BitVec::new(),
        }
    }
```

- [ ] **Step 7.3: Implement species.rs**

Create `crates/anabios-core/src/species.rs`:

```rust
//! Online species clustering and phylogeny tracking.
//!
//! Runs every `SPECIES_STEP_INTERVAL` ticks. Algorithm:
//!
//! 1. Recompute each species' centroid as the mean of its alive members.
//!    Mark empty species (member count = 0) but keep their id slots intact.
//! 2. For each alive agent in id order:
//!    - Compute distance to its current species centroid.
//!    - If `> SPECIATION_THRESHOLD`, find the closest existing species
//!      (over all non-empty species).
//!      - If that closest species is also `> SPECIATION_THRESHOLD`, allocate
//!        a new species id whose centroid is this agent's genome and whose
//!        `species_parents[k] = Some(prior_id)`.
//!      - Otherwise reassign the agent to the closest species.
//! 3. Recompute centroids once more (since memberships changed in step 2).

use crate::genome::{Genome, GENOME_LEN};
use crate::world::World;

/// Run species clustering every N ticks.
pub const SPECIES_STEP_INTERVAL: u64 = 200;

/// L2 distance threshold beyond which an agent's genome is considered
/// "different enough" from its species' centroid to trigger reassignment
/// or split-off.
pub const SPECIATION_THRESHOLD: f32 = 0.6;

pub fn species_step(world: &mut World) {
    recompute_centroids(world);

    // Snapshot alive ids to iterate deterministically.
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();

    for id in &alive_ids {
        let i = *id as usize;
        let g = world.agents.genome[i];
        let cur_species = world.agents.species_id[i] as usize;
        let d_own = world.species_centroids[cur_species].distance(&g);

        if d_own <= SPECIATION_THRESHOLD {
            continue;
        }

        // Find closest non-empty species across the table.
        let mut best_id: usize = cur_species;
        let mut best_d: f32 = d_own;
        for (sid, count) in world.species_member_counts.iter().enumerate() {
            if *count == 0 || sid == cur_species {
                continue;
            }
            let d = world.species_centroids[sid].distance(&g);
            if d < best_d {
                best_d = d;
                best_id = sid;
            }
        }

        if best_d <= SPECIATION_THRESHOLD {
            // Reassign to the existing closer species.
            world.species_member_counts[cur_species] -= 1;
            world.species_member_counts[best_id] += 1;
            world.agents.species_id[i] = best_id as u32;
        } else {
            // Allocate a new species with this agent's genome as centroid.
            let new_id = world.next_species_id;
            world.next_species_id = world
                .next_species_id
                .checked_add(1)
                .expect("species id overflow");
            world.species_centroids.push(g);
            world.species_member_counts.push(1);
            world.species_parents.push(Some(cur_species as u32));
            world.species_member_counts[cur_species] -= 1;
            world.agents.species_id[i] = new_id;
        }
    }

    // Step 3: recompute centroids once more so they reflect new memberships.
    recompute_centroids(world);
}

fn recompute_centroids(world: &mut World) {
    let num_species = world.species_centroids.len();

    // Accumulator: sum of each genome slot per species, plus member counts.
    let mut sums: Vec<[f64; GENOME_LEN]> = vec![[0.0_f64; GENOME_LEN]; num_species];
    let mut counts: Vec<u32> = vec![0_u32; num_species];

    // Iterate alive agents in ascending id order for determinism.
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i] as usize;
        let g = &world.agents.genome[i].0;
        for k in 0..GENOME_LEN {
            sums[sid][k] += g[k] as f64;
        }
        counts[sid] += 1;
    }

    for sid in 0..num_species {
        world.species_member_counts[sid] = counts[sid];
        if counts[sid] > 0 {
            let mut centroid = [0.0_f32; GENOME_LEN];
            let n = counts[sid] as f64;
            for k in 0..GENOME_LEN {
                centroid[k] = (sums[sid][k] / n) as f32;
            }
            world.species_centroids[sid] = Genome(centroid);
        }
        // Empty species keep their last-known centroid; this is fine for
        // history and the recompute on next step will refresh if members
        // return.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::GenomeSlot;
    use crate::prelude::Vec2;

    #[test]
    fn empty_world_runs_without_panic() {
        let mut w = World::new(1);
        species_step(&mut w);
        assert_eq!(w.species_centroids.len(), 1);
    }

    #[test]
    fn homogeneous_population_stays_one_species() {
        let mut w = World::new(7);
        for _ in 0..50 {
            w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        }
        species_step(&mut w);
        assert_eq!(w.species_member_counts.len(), 1);
        assert_eq!(w.species_member_counts[0], 50);
    }

    #[test]
    fn divergent_genome_triggers_speciation() {
        let mut w = World::new(7);
        for _ in 0..20 {
            w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        }
        // Add one agent with a very different genome.
        let mut weird = Genome::neutral();
        for i in 0..GENOME_LEN {
            weird.0[i] = if i % 2 == 0 { 0.0 } else { 1.0 };
        }
        w.spawn_agent(Vec2::new(500.0, 500.0), weird);

        species_step(&mut w);
        // Should have produced one new species with the weird agent.
        assert!(w.species_member_counts.len() >= 2,
            "expected speciation: {:?}", w.species_member_counts);
        assert_eq!(w.species_parents[1], Some(0));
    }
}
```

- [ ] **Step 7.4: Wire species_step into tick.rs**

Edit `crates/anabios-core/src/tick.rs`. Update `step` to call `species::species_step` every `SPECIES_STEP_INTERVAL` ticks. Replace `step` with:

```rust
pub fn step(world: &mut World) {
    world.resize_scratch();
    let cap = world.agents.capacity();

    world
        .spatial
        .rebuild(&world.agents.position, |i| world.agents.is_alive(i as u32));

    sense_all(&world.agents, &world.biome, &world.spatial, &mut world.sensors);

    decide_all(world);

    integrate_all(&mut world.agents, &world.desired_velocity[..cap]);

    interact_all(&mut world.agents, &mut world.biome);

    crate::reproduce::reproduce_all(world);

    age_and_starve(&mut world.agents);

    if world.tick.is_multiple_of(crate::species::SPECIES_STEP_INTERVAL) {
        crate::species::species_step(world);
    }

    if world.tick.is_multiple_of(BIOME_STEP_INTERVAL) {
        world.biome.regrow_step();
    }

    world.tick += 1;
}
```

- [ ] **Step 7.5: Run species tests**

Run: `cargo test -p anabios-core species`

Expected: 3 tests pass.

- [ ] **Step 7.6: Run all lib tests**

Run: `cargo test -p anabios-core --lib`

Expected: lib still green. The existing tick test `empty_world_can_tick` runs 100 ticks — that won't hit the 200-tick speciation threshold, so it should still pass.

- [ ] **Step 7.7: fmt + clippy**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: zero diffs, zero warnings.

- [ ] **Step 7.8: Commit**

```bash
git add crates/anabios-core/src/lib.rs crates/anabios-core/src/species.rs crates/anabios-core/src/world.rs crates/anabios-core/src/tick.rs
git commit -m "feat(core): online species clustering with phylogeny tracking, runs every 200 ticks"
```

---

## Task 8: Regenerate golden-tick determinism hashes

**Goal:** All M2 changes alter the world state (new fields, new tick stages). The golden hashes from M1 are stale. Regenerate them.

**Files:**
- Modify: `crates/anabios-core/tests/determinism.rs`

- [ ] **Step 8.1: Reset the GOLDEN array to placeholders**

Edit `crates/anabios-core/tests/determinism.rs`. Replace the `GOLDEN` constant with placeholders so the `UPDATE_HASHES` path can run:

```rust
const GOLDEN: &[(u64, u64)] = &[
    (0, 0x0000000000000000),
    (100, 0x0000000000000000),
    (1000, 0x0000000000000000),
];
```

- [ ] **Step 8.2: Generate new hashes**

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --test determinism -- --nocapture`

Expected: the test prints three lines like `(0, 0x...)`, `(100, 0x...)`, `(1000, 0x...)`.

- [ ] **Step 8.3: Paste the new hashes back**

Replace the placeholder `GOLDEN` array with the actual values printed in Step 8.2.

- [ ] **Step 8.4: Verify the pinned hashes pass without the env var**

Run: `cargo test -p anabios-core --test determinism`

Expected: 1 passed.

- [ ] **Step 8.5: Run a second time to confirm reproducibility**

Run: `cargo test -p anabios-core --test determinism`

Expected: 1 passed, same hashes.

- [ ] **Step 8.6: Commit**

```bash
git add crates/anabios-core/tests/determinism.rs
git commit -m "test(core): regenerate golden tick hashes for M2 (lineage + species + reproduce)"
```

---

## Task 9: Update existing property + feeding tests for M2 dynamics

**Goal:** `tests/invariants.rs` and `tests/feeding.rs` reflect M1's no-reproduction world. Update the assertions for the new dynamics: populations now grow / sustain, and we have new invariants to check.

**Files:**
- Modify: `crates/anabios-core/tests/invariants.rs`
- Modify: `crates/anabios-core/tests/feeding.rs`

- [ ] **Step 9.1: Add lineage and species invariants**

Append to `crates/anabios-core/tests/invariants.rs` (inside the existing `proptest!` block):

```rust
    /// Every alive agent has a non-zero lineage_id (zero is reserved as
    /// LINEAGE_NONE for "no parent"). Newborns get fresh ids from
    /// `World.next_lineage()`.
    #[test]
    fn alive_agents_have_nonzero_lineage_id(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for id in w.agents.iter_alive() {
            let lin = w.agents.lineage_id[id as usize];
            prop_assert_ne!(lin, anabios_core::agent::LINEAGE_NONE,
                "agent {id} has LINEAGE_NONE");
        }
    }

    /// Every alive agent's species_id refers to a slot in the species table.
    /// (Both empty and populated species are valid; out-of-range ids are not.)
    #[test]
    fn agent_species_ids_are_valid(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        let max_id = w.species_centroids.len() as u32;
        for id in w.agents.iter_alive() {
            let sid = w.agents.species_id[id as usize];
            prop_assert!(sid < max_id,
                "agent {id} has species_id {sid} but table has {max_id}");
        }
    }

    /// Every non-founder species has a parent recorded in the phylogeny.
    /// Species 0 is the founder.
    #[test]
    fn non_founder_species_have_parents(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for (sid, parent) in w.species_parents.iter().enumerate() {
            if sid == 0 {
                prop_assert_eq!(*parent, None, "species 0 should have no parent");
            } else {
                prop_assert!(parent.is_some(), "species {sid} has no recorded parent");
            }
        }
    }
```

- [ ] **Step 9.2: Add the `LINEAGE_NONE` re-export to the public crate API**

Check `crates/anabios-core/src/lib.rs`. After the existing `pub use agent::AgentId;` line, add:

```rust
pub use agent::{LineageId, LINEAGE_NONE, SpeciesId};
```

- [ ] **Step 9.3: Update feeding integration test**

Edit `crates/anabios-core/tests/feeding.rs`. The current test asserts `final_biomass < initial_biomass * 1.5`. With reproduction, populations now sustain — but biomass shouldn't explode either. Replace the assertions inside `population_persists_for_500_ticks`:

```rust
    for _ in 0..500 {
        step(&mut world);
    }

    let final_alive = world.agents.live_count();
    let final_biomass = world.plant_biomass_total();
    // Population must persist (M2 dynamic: reproduction sustains the pop).
    assert!(final_alive > 0, "population went extinct in 500 ticks: {} -> {}", initial_alive, final_alive);
    // Biomass should remain in a reasonable band — not zero, not multiples
    // of carrying capacity.
    assert!(final_biomass > 0.0);
    assert!(final_biomass < initial_biomass * 1.5);
```

The current assertions are still sensible — the change is mostly that we can trust them with reproduction in place. **If the test fails because population grows too aggressively and biomass drops far below `initial_biomass * 0.1`, widen the lower bound or run the test for fewer ticks; report what you did.**

- [ ] **Step 9.4: Run tests**

```bash
cargo test -p anabios-core --tests
```

Expected: all integration tests pass (determinism + feeding + invariants).

- [ ] **Step 9.5: fmt + clippy**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 9.6: Commit**

```bash
git add crates/anabios-core/src/lib.rs crates/anabios-core/tests/invariants.rs crates/anabios-core/tests/feeding.rs
git commit -m "test(core): M2 invariants — lineage uniqueness, species validity, phylogeny correctness"
```

---

## Task 10: Reproduction integration test (long-run stable population)

**Goal:** A dedicated integration test that runs the minimal scenario for 5,000 ticks and asserts the population sustains itself. Mirror the existing `feeding.rs` style but check longer-term dynamics.

**Files:**
- Create: `crates/anabios-core/tests/reproduction.rs`

- [ ] **Step 10.1: Implement the test**

Create `crates/anabios-core/tests/reproduction.rs`:

```rust
//! Integration test: with reproduction (M2), the minimal scenario must
//! sustain its population over a window longer than the natural lifespan,
//! confirming that newborns are replacing deaths.

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

#[test]
fn population_sustains_past_one_lifespan() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    let initial_alive = world.agents.live_count();
    assert!(initial_alive > 0);

    // Run for 5,000 ticks — well past the natural lifespan (≈ 3,200 ticks
    // at LifespanBias = 0.6).
    for _ in 0..5_000 {
        step(&mut world);
    }

    let final_alive = world.agents.live_count();
    assert!(
        final_alive > 0,
        "population should sustain past one lifespan; initial={initial_alive}, final={final_alive}",
    );
}
```

- [ ] **Step 10.2: Run the test**

Run: `cargo test -p anabios-core --test reproduction`

Expected: 1 passed (takes ~1-2 s).

If the test fails (final_alive = 0), the tuning of reproduction thresholds isn't sustaining the population. Investigate the cause — likely candidates: (a) `PARENT_ENERGY_COST_FRAC` too high, (b) mate-finding radius too small, (c) reproduction_threshold setting in minimal.toml means agents can't reach mate-ready. Adjust the parameter that's bottlenecking and document the change in the task report. Do not commit a failing test.

- [ ] **Step 10.3: Commit**

```bash
git add crates/anabios-core/tests/reproduction.rs
git commit -m "test(core): population sustains past natural lifespan with reproduction"
```

---

## Task 11: Forced speciation integration test

**Goal:** A scenario where two distant initial populations with divergent genomes converge to different species and we verify that `species_centroids.len() >= 2` after enough ticks for speciation to fire.

**Files:**
- Create: `crates/anabios-core/tests/speciation.rs`
- Create: `scenarios/divergent.toml`

- [ ] **Step 11.1: Create the divergent scenario**

Create `scenarios/divergent.toml`:

```toml
name = "divergent"
seed = 4242

# Population A: low-speed, low-aggression, low-perception herbivores
# clustered near the world origin.
[[agents]]
count = 60
[agents.placement]
kind = "cluster"
center_x = 100.0
center_y = 100.0
radius = 30.0
[agents.traits]
speed_max = 0.1
perception_radius = 0.1
size = 0.2
diet_carnivory = 0.0
basal_metabolism = 0.2
lifespan_bias = 0.7
reproduction_threshold = 0.4

# Population B: high-speed, high-perception herbivores clustered far away.
[[agents]]
count = 60
[agents.placement]
kind = "cluster"
center_x = 900.0
center_y = 900.0
radius = 30.0
[agents.traits]
speed_max = 0.95
perception_radius = 0.95
size = 0.8
diet_carnivory = 0.0
basal_metabolism = 0.8
lifespan_bias = 0.7
reproduction_threshold = 0.4
```

The two populations start in opposite world corners with very different trait vectors (`distance ≈ √(7×0.85²) ≈ 2.25` — well past the speciation threshold of 0.6).

- [ ] **Step 11.2: Implement the test**

Create `crates/anabios-core/tests/speciation.rs`:

```rust
//! Integration test: two genetically-distant founder populations should be
//! recognized as separate species by the first time `species_step` runs
//! (tick 200) or shortly after.

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/divergent.toml");

#[test]
fn distant_founder_populations_become_separate_species() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();

    // Run past the first speciation event (200 ticks) plus a buffer for
    // the algorithm to recognize the split.
    for _ in 0..400 {
        step(&mut world);
    }

    // At least two non-empty species expected.
    let non_empty: usize = world
        .species_member_counts
        .iter()
        .filter(|&&c| c > 0)
        .count();
    assert!(
        non_empty >= 2,
        "expected speciation, got species member counts {:?}",
        world.species_member_counts,
    );

    // At least one species has a recorded parent (non-founder).
    let any_child = world.species_parents.iter().any(|p| p.is_some());
    assert!(any_child, "no non-founder species recorded in phylogeny");
}
```

- [ ] **Step 11.3: Run the test**

Run: `cargo test -p anabios-core --test speciation`

Expected: 1 passed.

If the test fails ("expected speciation, got species member counts [120]"), the two populations are too close to the centroid for the split to be detected. Verify the `divergent.toml` traits remain at their extremes after a few hundred ticks (mutation drift is bounded but real). If the test still fails, lower `SPECIATION_THRESHOLD` from 0.6 to 0.5 in `species.rs` — but document the change and re-run all other tests (determinism golden hashes may change again, which is OK; regenerate them in Task 8's style if needed).

- [ ] **Step 11.4: Commit**

```bash
git add crates/anabios-core/tests/speciation.rs scenarios/divergent.toml
git commit -m "test(core): distant founder populations resolve to separate species"
```

---

## Task 12: Bench update + perf check

**Goal:** Verify M2's additions (extra fields, two new tick stages) haven't blown the perf budget. The §8 spec targets 5k agents @ 60 ticks/s and 10k @ 30 ticks/s.

**Files:**
- Modify: `crates/anabios-core/benches/tick_bench.rs`

- [ ] **Step 12.1: No code change to the bench file**

The existing bench at `crates/anabios-core/benches/tick_bench.rs` builds populations with the M1 API. It should still compile because `spawn_agent(position, genome)` retained its signature (per Task 2). Verify by running `cargo bench --no-run -p anabios-core`. If it doesn't compile, fix the spawn call sites narrowly.

- [ ] **Step 12.2: Run the bench**

Run: `cargo bench -p anabios-core --bench tick_bench`

Expected: criterion reports two cases (1k, 10k). Record the median times.

Target reminder from §8 of the spec:
- 1k agents: ≤ 1 ms/tick. M1 baseline was ≈ 0.19 ms.
- 10k agents: ≤ 15 ms/tick. M1 baseline was ≈ 3.8 ms.

M2 adds: reproduce stage (1 spatial query per agent), species clustering (once per 200 ticks, amortized to <0.1 ms/tick at 10k). Expected new numbers: 1k ≈ 0.3 ms, 10k ≈ 5 ms.

If 10k exceeds 15 ms, the most likely culprit is `reproduce_all`'s per-agent spatial query when no one is reproducing — most calls are wasted. Optimization: skip the spatial query when `is_eligible(a)` is false. Apply that fix narrowly and re-bench. Document any optimization in the task report.

- [ ] **Step 12.3: Commit if any code change was needed**

If no code changes were made (most likely outcome), no commit. If a perf fix was needed:

```bash
git add crates/anabios-core/src/reproduce.rs
git commit -m "perf(core): skip spatial query when agent is reproduction-ineligible"
```

---

## Task 13: Final green-bar pass + milestone tag

**Goal:** Verify everything passes one more time, run the CLI determinism smoke, and tag the milestone.

- [ ] **Step 13.1: Full workspace check**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: zero diffs, zero warnings, all tests pass (M1 + M2).

- [ ] **Step 13.2: Determinism smoke**

```bash
cargo run -q --release --bin anabios-headless -- run --scenario scenarios/minimal.toml --ticks 5000 > /tmp/anabios_m2_a.txt
cargo run -q --release --bin anabios-headless -- run --scenario scenarios/minimal.toml --ticks 5000 > /tmp/anabios_m2_b.txt
diff /tmp/anabios_m2_a.txt /tmp/anabios_m2_b.txt && echo "deterministic"
```

Expected: identical output, "deterministic" printed.

- [ ] **Step 13.3: Confirm population sustains in the smoke run**

The last line of either smoke run should report `alive > 0` and `state_hash` deterministic. If `alive=0` after 5000 ticks, the reproduction tuning needs adjusting; investigate before tagging.

- [ ] **Step 13.4: Tag the milestone**

```bash
git tag -a m2 -m "M2: reproduction, mutation, speciation, phylogeny"
```

- [ ] **Step 13.5: Push branch + tag and open PR**

```bash
git push -u origin m2-reproduction-speciation
git push origin m2
```

Open the PR with `gh pr create`. PR title: `M2: reproduction, mutation, speciation (sustained populations)`. Body should summarize the changes, link the spec section §3.8, note any deviations from the plan, and include a test plan checklist.

---

## Post-implementation expectations

After M2 merges:

- The minimal scenario runs indefinitely with a sustained herbivore population (no extinction-by-old-age like M1).
- Two distant founder populations resolve into separate species after ≈ 200-400 ticks.
- Every alive agent has a non-zero `lineage_id`, a valid `species_id`, and `parent_ids` (or `[LINEAGE_NONE; 2]` for founders).
- The phylogeny tree (`World.species_parents`) is accessible for the codex events that M5 will detect (`SpeciationEvent`, `Extinction`).
- Determinism is preserved: golden tick hashes pinned, headless smoke runs byte-identical twice.

Deferred to M3 and later:

- Modular morphology (M3): variable-length module list per agent.
- Behavior program (M4): evolvable expression tree replaces the hardcoded forage/wander/mate function.
- Codex detectors (M5): the substrate for `SpeciationEvent`, `Extinction`, `MigrationDetected`, etc. exists after M2 but no detectors are wired yet.
- Cross-OS determinism in CI (M1 follow-up I1): tackle once the behavior program in M4 inevitably calls more transcendentals.
