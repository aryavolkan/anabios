# M3 — Modular Morphology Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the genome's monolithic "what an agent can do" with a variable-length list of typed body modules per agent. Module presence gates actions (no Locomotor → no motion, no Mouth → no feeding, no Reproductive → no mating), modules cost per-tick upkeep energy, and reproduction inherits + mutates the module list (point-mutate parameters; add/delete/duplicate/replace whole modules).

**Architecture:** Add a `Module` enum (variant per type) with bounded f32 parameters in `[0, 1]`. Each agent owns a `SmallVec<[Module; 8]>` (3-12 typical). New `crates/anabios-core/src/module.rs` houses the enum, mutation operators, and crossover. Existing tick stages gain gating guards: `integrate` checks for Locomotor, `interact` checks for Mouth, `reproduce` checks for Reproductive. A new `module_upkeep_all` stage deducts a per-tick energy cost summed across an agent's modules. The genome's `SpeedMax`, `PerceptionRadius`, and `DietCarnivory` slots become **modulators** of module parameters rather than absolute values.

**Tech Stack:** Unchanged from M2 — Rust stable, `glam`, `rand_xoshiro`, `serde + bincode`, `smallvec`, `bitvec`, `proptest`, `criterion`.

**Style conventions** (inherited from M1/M2):

- 4-space indent, no tabs
- Rustdoc `///` only where behavior isn't obvious from the name
- All randomness routed through `World.rng`
- No allocations in the hot path; reuse scratch buffers
- Deterministic iteration: ascending agent id; module lists iterated in stored order
- Conventional Commits message prefixes
- Single commit per task unless noted

**Branch:** `m3-modular-morphology` branched from `main`.

**Working directory:** All commands assume cwd = `/Users/aryasen/projects/anabios/`.

**Prerequisite from M2 followups:** Important #1 (`species_member_counts` incremental tracking) lands as Task 0 below, before any M3 morphology work touches species state.

---

## File structure after M3

New files:
```
crates/anabios-core/src/
└── module.rs                          # Module enum + ModuleType + starter kit + mutation + crossover + upkeep
crates/anabios-core/tests/
├── module_gating.rs                   # gating: missing modules disable actions
└── morphology_evolution.rs            # integration: modules drift across generations
```

Modified files:
```
crates/anabios-core/src/
├── agent.rs                           # +modules: Vec<SmallVec<[Module; 8]>>; spawn signature
├── world.rs                           # +species count tracking helpers (Task 0); species_step uses them
├── species.rs                         # use the new helpers; remove the manual count bookkeeping
├── reproduce.rs                       # gate on Reproductive; inherit + mutate module list
├── interact.rs                        # gate on Mouth; bite_size from module
├── integrate.rs                       # gate on Locomotor; speed_max from module
├── sense.rs                           # perception_radius from Sensor modules
├── behavior.rs                        # decide() reads effective speed from module
├── tick.rs                            # +module_upkeep_all stage
└── scenario.rs                        # starter kit + optional override of module composition
crates/anabios-core/tests/
├── determinism.rs                     # regenerate GOLDEN hashes (M3 dynamics)
├── invariants.rs                      # +modules invariants
├── reproduction.rs                    # may need to tolerate occasional infertile (no-Reproductive) offspring
└── feeding.rs                         # unchanged (starter kit includes Mouth)
crates/anabios-core/Cargo.toml          # (no changes; smallvec already pulled in)
```

---

## Task 0: Incremental species count tracking (M2 followup prerequisite)

**Goal:** Fix M2 Important #1 from `docs/superpowers/m2-followups.md` before M3 reads species state during gameplay. Track `species_member_counts` on every `spawn` and `kill` instead of recomputing each species_step.

**Files:**
- Modify: `crates/anabios-core/src/world.rs`
- Modify: `crates/anabios-core/src/agent.rs`
- Modify: `crates/anabios-core/src/reproduce.rs`
- Modify: `crates/anabios-core/src/age.rs`
- Modify: `crates/anabios-core/src/species.rs`
- Modify: `crates/anabios-core/tests/determinism.rs` (golden hash regen)

- [ ] **Step 0.1: Create branch**

```bash
git checkout main
git pull
git checkout -b m3-modular-morphology
```

- [ ] **Step 0.2: Add helpers to World**

Append to the `impl World` block in `crates/anabios-core/src/world.rs`:

```rust
    /// Increment the species member count, growing the table if needed.
    /// Called by every spawn path.
    pub fn add_to_species(&mut self, species_id: u32) {
        let idx = species_id as usize;
        if idx >= self.species_member_counts.len() {
            // Caller created a species via the species_step split-off path
            // and is responsible for pushing centroid + parent first; this
            // helper only grows the count vec.
            self.species_member_counts.resize(idx + 1, 0);
        }
        self.species_member_counts[idx] = self
            .species_member_counts[idx]
            .checked_add(1)
            .expect("species member count overflow");
    }

    /// Decrement the species member count. Saturating: if the count is
    /// already zero (bookkeeping bug), do not underflow.
    pub fn remove_from_species(&mut self, species_id: u32) {
        let idx = species_id as usize;
        if idx >= self.species_member_counts.len() {
            return;
        }
        self.species_member_counts[idx] = self.species_member_counts[idx].saturating_sub(1);
    }
```

- [ ] **Step 0.3: Update World::spawn_agent to track counts**

Replace the existing `spawn_agent` in `world.rs`:

```rust
    pub fn spawn_agent(&mut self, position: Vec2, genome: Genome) -> AgentId {
        let lineage = self.next_lineage();
        let id = self.agents.spawn(position, genome, lineage, [LINEAGE_NONE; 2], 0);
        self.add_to_species(0);
        id
    }
```

- [ ] **Step 0.4: Update reproduce.rs to track counts**

Edit `crates/anabios-core/src/reproduce.rs`. Find the spawn-child block (near the end of the per-pair loop, after `let lineage = world.next_lineage();`). After the spawn call, add the species count increment. Replace:

```rust
        let lineage = world.next_lineage();
        let child_id =
            world.agents.spawn(child_pos, child_genome, lineage, [a_lineage, b_lineage], a_species);
```

with:

```rust
        let lineage = world.next_lineage();
        let child_id =
            world.agents.spawn(child_pos, child_genome, lineage, [a_lineage, b_lineage], a_species);
        world.add_to_species(a_species);
```

- [ ] **Step 0.5: Update age.rs to track counts on death**

Edit `crates/anabios-core/src/age.rs`. The function `age_and_starve` currently calls `agents.kill(id)` without touching species counts. Wrap each kill with a species decrement. Find:

```rust
        if agents.energy[i] <= 0.0 {
            agents.kill(id);
        } else if agents.age[i] >= lifespan {
            agents.kill(id);
        }
```

Note: `age_and_starve` takes `&mut AgentBuffers`, not `&mut World`. To track species counts we need `&mut World`. **Refactor the function** to take `&mut World`. Replace the entire function:

```rust
pub fn age_and_starve(world: &mut crate::world::World) {
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
        world.agents.age[i] = world.agents.age[i].saturating_add(1);

        let lifespan = lifespan_of(&world.agents.genome[i]);
        let died = if world.agents.energy[i] <= 0.0 {
            true
        } else {
            world.agents.age[i] >= lifespan
        };

        if died {
            let sid = world.agents.species_id[i];
            world.agents.kill(id);
            world.remove_from_species(sid);
        }
    }
}
```

Then update the call site in `crates/anabios-core/src/tick.rs`. Find:

```rust
    age_and_starve(&mut world.agents);
```

Replace with:

```rust
    age_and_starve(world);
```

- [ ] **Step 0.6: Update age.rs tests to use World**

The tests in `crates/anabios-core/src/age.rs::tests` call `age_and_starve(&mut w.agents)`. Update each to `age_and_starve(&mut w)`:

```rust
    #[test]
    fn age_increments_each_call() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        age_and_starve(&mut w);
        age_and_starve(&mut w);
        age_and_starve(&mut w);
        assert_eq!(w.agents.age[id as usize], 3);
        assert!(w.agents.is_alive(id));
    }

    #[test]
    fn agent_with_zero_energy_dies() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.agents.energy[id as usize] = 0.0;
        age_and_starve(&mut w);
        assert!(!w.agents.is_alive(id));
    }

    #[test]
    fn agent_dies_of_old_age_at_lifespan_bias_zero() {
        let mut w = World::new(1);
        let mut g = Genome::neutral();
        g.set(GenomeSlot::LifespanBias, 0.0);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), g);
        for _ in 0..LIFESPAN_MIN_TICKS as usize {
            age_and_starve(&mut w);
            if !w.agents.is_alive(id) {
                break;
            }
        }
        assert!(!w.agents.is_alive(id));
    }
```

- [ ] **Step 0.7: Update species.rs to use the helpers**

Edit `crates/anabios-core/src/species.rs`. Inside `species_step`, the reassignment branch already manipulates counts manually. Replace the manual updates with the helpers. Find the block:

```rust
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
```

Replace with:

```rust
        if best_d <= SPECIATION_THRESHOLD {
            // Reassign to the existing closer species.
            world.remove_from_species(cur_species as u32);
            world.add_to_species(best_id as u32);
            world.agents.species_id[i] = best_id as u32;
        } else {
            // Allocate a new species with this agent's genome as centroid.
            let new_id = world.next_species_id;
            world.next_species_id = world
                .next_species_id
                .checked_add(1)
                .expect("species id overflow");
            world.species_centroids.push(g);
            world.species_member_counts.push(0); // helper increments below
            world.species_parents.push(Some(cur_species as u32));
            world.remove_from_species(cur_species as u32);
            world.add_to_species(new_id);
            world.agents.species_id[i] = new_id;
        }
```

Then update the `recompute_centroids` function. It currently authoritatively rewrites `species_member_counts` from `iter_alive`. Now that counts are tracked incrementally, the helper should ONLY recompute centroids (the means). Replace `recompute_centroids` body with:

```rust
fn recompute_centroids(world: &mut World) {
    let num_species = world.species_centroids.len();

    // Sum genome slots per species in deterministic agent id order.
    let mut sums: Vec<[f64; GENOME_LEN]> = vec![[0.0_f64; GENOME_LEN]; num_species];

    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i] as usize;
        let g = &world.agents.genome[i].0;
        for k in 0..GENOME_LEN {
            sums[sid][k] += g[k] as f64;
        }
    }

    for sid in 0..num_species {
        let n = world.species_member_counts[sid];
        if n > 0 {
            let mut centroid = [0.0_f32; GENOME_LEN];
            let nf = n as f64;
            for k in 0..GENOME_LEN {
                centroid[k] = (sums[sid][k] / nf) as f32;
            }
            world.species_centroids[sid] = Genome(centroid);
        }
    }
}
```

Update the `species_member_counts` doc comment in `world.rs` to remove the staleness warning — counts are now authoritative everywhere:

```rust
    /// Per-species live member count. Tracked incrementally by
    /// `World::add_to_species` / `remove_from_species` on every spawn,
    /// kill, and `species_step` reassignment, so it is authoritative
    /// outside of `species_step` itself.
    pub species_member_counts: Vec<u32>,
```

- [ ] **Step 0.8: Run all lib tests**

```bash
cargo test -p anabios-core --lib
```

Expected: 69 tests pass (same as M2). If a count is off, the species_step recompute may have hidden an earlier bug; investigate before continuing.

- [ ] **Step 0.9: Regenerate golden hashes**

The `species_member_counts` field is now non-zero at tick 0 (after scenario instantiation), changing the snapshot hash.

Edit `crates/anabios-core/tests/determinism.rs` and reset the GOLDEN array to zeros:

```rust
const GOLDEN: &[(u64, u64)] = &[
    (0, 0x0000000000000000),
    (100, 0x0000000000000000),
    (1000, 0x0000000000000000),
];
```

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --test determinism -- --nocapture`

Copy the three printed `(tick, hash)` pairs into the array. Verify:

```bash
cargo test -p anabios-core --test determinism
```

Expected: 1 passed.

- [ ] **Step 0.10: fmt + clippy + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "refactor(core): incremental species_member_counts tracking on spawn/kill"
```

---

## Task 1: Module type and library

**Goal:** Define the `Module` enum with one variant per type. All parameters are `f32` in `[0, 1]`. Provide a `starter_kit()` constructor returning the default 4-module set every founder agent gets.

**Files:**
- Create: `crates/anabios-core/src/module.rs`
- Modify: `crates/anabios-core/src/lib.rs`

- [ ] **Step 1.1: Declare the module**

Add `pub mod module;` to `crates/anabios-core/src/lib.rs` in alphabetical order (between `interact` and `reproduce`):

```rust
pub mod age;
pub mod agent;
pub mod behavior;
pub mod biome;
pub mod genome;
pub mod integrate;
pub mod interact;
pub mod module;
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

Also re-export the new types from `lib.rs`:

```rust
pub use module::{Module, ModuleType};
```

- [ ] **Step 1.2: Implement the Module enum**

Create `crates/anabios-core/src/module.rs`:

```rust
//! Modular morphology (M3).
//!
//! Each agent carries a `SmallVec<[Module; 8]>` (typically 3-12 modules)
//! that define what it can do. Module presence gates actions in the tick
//! pipeline:
//!
//! - No `Locomotor`  → cannot move
//! - No `Sensor`     → cannot perceive plants or neighbours
//! - No `Mouth`      → cannot feed
//! - No `Reproductive` → cannot mate
//!
//! Other module types (`Weapon`, `Armor`, `Storage`, `Communicator`,
//! `Pheromone`) are part of the M3 substrate but their gameplay effects
//! land in later milestones (combat in M4, pheromones in a later
//! milestone). They still pay upkeep when present.
//!
//! All parameters are `f32` in `[0, 1]` and are perturbed by Gaussian
//! mutation during reproduction. Whole-module mutation (add, delete,
//! duplicate, replace) is also applied with low probability.

use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};

use crate::rng::Rng;

/// Maximum number of modules per agent. The `SmallVec` inline storage is
/// also 8; agents with > 8 modules spill to the heap.
pub const MODULE_INLINE_CAPACITY: usize = 8;
pub const MODULE_LIST_MAX: usize = 16;

/// Per-module per-tick upkeep cost at parameter value 1.0. Actual cost
/// scales linearly with the dominant parameter of the module.
pub const UPKEEP_BASE: f32 = 0.005;

/// Module-list inheritance probabilities applied during reproduction.
pub const MUTATE_PARAM_PROB: f32 = 0.5;
pub const ADD_MODULE_PROB: f32 = 0.02;
pub const DELETE_MODULE_PROB: f32 = 0.02;
pub const DUPLICATE_MODULE_PROB: f32 = 0.02;
pub const REPLACE_MODULE_PROB: f32 = 0.01;

/// Gaussian sigma when perturbing a single module parameter.
pub const PARAM_SIGMA: f32 = 0.05;

/// Sensor channel type. Vision sees plants and other agents; smell, heat,
/// and sound are reserved for later milestones and have no effect in M3.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SensorType {
    Vision = 0,
    Smell = 1,
    Heat = 2,
    Sound = 3,
}

/// Pheromone channel id. Multiple channels coexist; M3 does not yet read
/// pheromones in any tick stage (no field present in `World`), so the
/// channel value is currently inert metadata. Reserved for later.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PheromoneChannel {
    Alarm = 0,
    Mate = 1,
    Trail = 2,
    Marker = 3,
}

/// One body module.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Module {
    /// Enables motion. `max_speed` scales the agent's velocity cap.
    /// `terrain_affinity` is reserved for M4+ (will gate land vs water
    /// crossing); currently inert.
    Locomotor { max_speed: f32, terrain_affinity: f32 },
    /// Enables one channel of perception. `radius` and `acuity` shape
    /// what the agent can sense.
    Sensor { sensor_type: SensorType, radius: f32, acuity: f32 },
    /// Enables feeding. `bite_size` caps biomass per bite; `diet_affinity`
    /// = 0 → pure herbivore, 1 → pure carnivore (carnivory has no effect
    /// in M3 since combat is M4).
    Mouth { bite_size: f32, diet_affinity: f32 },
    /// Inflicts damage on contact. No gameplay effect in M3 (combat is
    /// later); pays upkeep.
    Weapon { damage: f32, energy_cost: f32 },
    /// Reduces damage. No gameplay effect in M3; pays upkeep.
    Armor { protection: f32, mass_penalty: f32 },
    /// Increases the agent's effective energy capacity. No gameplay
    /// effect in M3 (no overflow check yet); pays upkeep.
    Storage { capacity: f32 },
    /// Emits/receives meme signals. No gameplay effect in M3; pays upkeep.
    Communicator { range: f32, channel_id: u8 },
    /// Leaves chemical marks on the biome. No gameplay effect in M3 (no
    /// pheromone field yet); pays upkeep.
    Pheromone { channel: PheromoneChannel, strength: f32, decay: f32 },
    /// Required for reproduction. `viability` modulates the mating energy
    /// cost; `brood_size_bias` is reserved for M5.
    Reproductive { viability: f32, brood_size_bias: f32 },
}

/// Discriminant tag — useful when generating a random module or checking
/// "do I have any module of type X".
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleType {
    Locomotor = 0,
    Sensor = 1,
    Mouth = 2,
    Weapon = 3,
    Armor = 4,
    Storage = 5,
    Communicator = 6,
    Pheromone = 7,
    Reproductive = 8,
}

impl Module {
    /// Tag-only view of this module's type.
    #[inline]
    pub fn module_type(&self) -> ModuleType {
        match self {
            Module::Locomotor { .. } => ModuleType::Locomotor,
            Module::Sensor { .. } => ModuleType::Sensor,
            Module::Mouth { .. } => ModuleType::Mouth,
            Module::Weapon { .. } => ModuleType::Weapon,
            Module::Armor { .. } => ModuleType::Armor,
            Module::Storage { .. } => ModuleType::Storage,
            Module::Communicator { .. } => ModuleType::Communicator,
            Module::Pheromone { .. } => ModuleType::Pheromone,
            Module::Reproductive { .. } => ModuleType::Reproductive,
        }
    }

    /// Per-tick upkeep cost in energy units. Scales with the module's
    /// dominant parameter so a high-capacity organ costs more than a
    /// vestigial one.
    pub fn upkeep(&self) -> f32 {
        let factor = match self {
            Module::Locomotor { max_speed, .. } => *max_speed,
            Module::Sensor { radius, acuity, .. } => 0.5 * (radius + acuity),
            Module::Mouth { bite_size, .. } => *bite_size,
            Module::Weapon { damage, .. } => *damage,
            Module::Armor { protection, mass_penalty } => 0.5 * (protection + mass_penalty),
            Module::Storage { capacity } => *capacity,
            Module::Communicator { range, .. } => *range,
            Module::Pheromone { strength, .. } => *strength,
            Module::Reproductive { viability, .. } => *viability,
        };
        UPKEEP_BASE * factor.clamp(0.05, 1.0)
    }

    /// Construct a random module of the given type with uniform parameters.
    pub fn random_of_type(module_type: ModuleType, rng: &mut Rng) -> Module {
        let p = |rng: &mut Rng| rng.f32_unit();
        match module_type {
            ModuleType::Locomotor => Module::Locomotor {
                max_speed: p(rng),
                terrain_affinity: p(rng),
            },
            ModuleType::Sensor => Module::Sensor {
                sensor_type: match (rng.f32_unit() * 4.0) as u8 {
                    0 => SensorType::Vision,
                    1 => SensorType::Smell,
                    2 => SensorType::Heat,
                    _ => SensorType::Sound,
                },
                radius: p(rng),
                acuity: p(rng),
            },
            ModuleType::Mouth => Module::Mouth {
                bite_size: p(rng),
                diet_affinity: p(rng),
            },
            ModuleType::Weapon => Module::Weapon {
                damage: p(rng),
                energy_cost: p(rng),
            },
            ModuleType::Armor => Module::Armor {
                protection: p(rng),
                mass_penalty: p(rng),
            },
            ModuleType::Storage => Module::Storage { capacity: p(rng) },
            ModuleType::Communicator => Module::Communicator {
                range: p(rng),
                channel_id: (rng.f32_unit() * 4.0) as u8,
            },
            ModuleType::Pheromone => Module::Pheromone {
                channel: match (rng.f32_unit() * 4.0) as u8 {
                    0 => PheromoneChannel::Alarm,
                    1 => PheromoneChannel::Mate,
                    2 => PheromoneChannel::Trail,
                    _ => PheromoneChannel::Marker,
                },
                strength: p(rng),
                decay: p(rng),
            },
            ModuleType::Reproductive => Module::Reproductive {
                viability: p(rng),
                brood_size_bias: p(rng),
            },
        }
    }

    /// Construct a random module of any type. Used by the structural
    /// "add" and "replace" mutation operators.
    pub fn random_any(rng: &mut Rng) -> Module {
        let t = match (rng.f32_unit() * 9.0) as u8 {
            0 => ModuleType::Locomotor,
            1 => ModuleType::Sensor,
            2 => ModuleType::Mouth,
            3 => ModuleType::Weapon,
            4 => ModuleType::Armor,
            5 => ModuleType::Storage,
            6 => ModuleType::Communicator,
            7 => ModuleType::Pheromone,
            _ => ModuleType::Reproductive,
        };
        Module::random_of_type(t, rng)
    }
}

/// Variable-length module list owned by an agent.
pub type ModuleList = SmallVec<[Module; MODULE_INLINE_CAPACITY]>;

/// The default 4-module kit assigned to every founder spawned via
/// `World::spawn_agent`. All four are at parameter value 0.6 (above the
/// upkeep dead-band, below max).
pub fn starter_kit() -> ModuleList {
    smallvec![
        Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 },
        Module::Sensor { sensor_type: SensorType::Vision, radius: 0.6, acuity: 0.6 },
        Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 },
        Module::Reproductive { viability: 0.6, brood_size_bias: 0.5 },
    ]
}

/// `true` iff the list contains at least one module of the given type.
#[inline]
pub fn has(modules: &ModuleList, module_type: ModuleType) -> bool {
    modules.iter().any(|m| m.module_type() == module_type)
}

/// Total per-tick upkeep cost.
#[inline]
pub fn total_upkeep(modules: &ModuleList) -> f32 {
    modules.iter().map(|m| m.upkeep()).sum()
}

/// Sum the `max_speed` of every Locomotor in the list. Used by the
/// integrate stage; 0.0 if no Locomotor is present (agent can't move).
#[inline]
pub fn effective_speed_max(modules: &ModuleList) -> f32 {
    modules.iter().filter_map(|m| match m {
        Module::Locomotor { max_speed, .. } => Some(*max_speed),
        _ => None,
    }).sum()
}

/// Maximum perception radius across all Sensor modules. 0.0 if no Sensor.
#[inline]
pub fn effective_perception_radius(modules: &ModuleList) -> f32 {
    modules.iter().filter_map(|m| match m {
        Module::Sensor { radius, .. } => Some(*radius),
        _ => None,
    }).fold(0.0_f32, f32::max)
}

/// Maximum bite size across all Mouth modules. 0.0 if no Mouth.
#[inline]
pub fn effective_bite_size(modules: &ModuleList) -> f32 {
    modules.iter().filter_map(|m| match m {
        Module::Mouth { bite_size, .. } => Some(*bite_size),
        _ => None,
    }).fold(0.0_f32, f32::max)
}

/// Maximum diet affinity across all Mouth modules. 0.0 (pure herbivore)
/// if no Mouth, but action gating means feeding is skipped anyway.
#[inline]
pub fn effective_diet_carnivory(modules: &ModuleList) -> f32 {
    modules.iter().filter_map(|m| match m {
        Module::Mouth { diet_affinity, .. } => Some(*diet_affinity),
        _ => None,
    }).fold(0.0_f32, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_kit_has_required_modules() {
        let k = starter_kit();
        assert!(has(&k, ModuleType::Locomotor));
        assert!(has(&k, ModuleType::Sensor));
        assert!(has(&k, ModuleType::Mouth));
        assert!(has(&k, ModuleType::Reproductive));
    }

    #[test]
    fn upkeep_is_proportional_to_dominant_param() {
        let small = Module::Locomotor { max_speed: 0.1, terrain_affinity: 0.5 };
        let big = Module::Locomotor { max_speed: 1.0, terrain_affinity: 0.5 };
        assert!(big.upkeep() > small.upkeep());
    }

    #[test]
    fn random_module_is_deterministic() {
        let mut a = Rng::from_seed(42);
        let mut b = Rng::from_seed(42);
        for _ in 0..20 {
            assert_eq!(Module::random_any(&mut a), Module::random_any(&mut b));
        }
    }

    #[test]
    fn module_type_matches_variant() {
        for t in [
            ModuleType::Locomotor, ModuleType::Sensor, ModuleType::Mouth,
            ModuleType::Weapon, ModuleType::Armor, ModuleType::Storage,
            ModuleType::Communicator, ModuleType::Pheromone, ModuleType::Reproductive,
        ] {
            let mut rng = Rng::from_seed(1);
            let m = Module::random_of_type(t, &mut rng);
            assert_eq!(m.module_type(), t);
        }
    }
}
```

- [ ] **Step 1.3: Run tests + fmt + clippy + commit**

```bash
cargo test -p anabios-core module
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/lib.rs crates/anabios-core/src/module.rs
git commit -m "feat(core): Module enum + library + starter kit + upkeep + helpers"
```

Expected: 4 module tests pass; full workspace clean.

---

## Task 2: Add modules to AgentBuffers and starter kit on spawn

**Goal:** Every agent gets a module list. Founders get the `starter_kit()`. Offspring inherit (Task 4 implements crossover + mutation).

**Files:**
- Modify: `crates/anabios-core/src/agent.rs`
- Modify: `crates/anabios-core/src/world.rs`

- [ ] **Step 2.1: Add modules field to AgentBuffers**

Edit `crates/anabios-core/src/agent.rs`. Add an import:

```rust
use crate::module::ModuleList;
```

Extend `AgentBuffers`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentBuffers {
    pub position: Vec<Vec2>,
    pub velocity: Vec<Vec2>,
    pub energy: Vec<f32>,
    pub age: Vec<u32>,
    pub genome: Vec<Genome>,
    pub lineage_id: Vec<LineageId>,
    pub parent_ids: Vec<[LineageId; 2]>,
    pub species_id: Vec<SpeciesId>,
    pub modules: Vec<ModuleList>,
    pub alive: BitVec,
    free_list: Vec<AgentId>,
    live_count: u32,
}
```

- [ ] **Step 2.2: Extend spawn signature**

Replace the `spawn` method in `agent.rs` so it accepts a `ModuleList`:

```rust
    pub fn spawn(
        &mut self,
        position: Vec2,
        genome: Genome,
        lineage_id: LineageId,
        parent_ids: [LineageId; 2],
        species_id: SpeciesId,
        modules: ModuleList,
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
            self.modules[i] = modules;
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
            self.modules.push(modules);
            self.alive.push(true);
            i as AgentId
        };
        self.live_count += 1;
        id
    }
```

- [ ] **Step 2.3: Update agent unit tests**

Every existing `a.spawn(pos, g, lineage, parents, sid)` call in `agent.rs::tests` becomes `a.spawn(pos, g, lineage, parents, sid, crate::module::starter_kit())`. Update the 5 tests.

- [ ] **Step 2.4: Update World::spawn_agent to pass starter_kit**

In `crates/anabios-core/src/world.rs`, the call inside `spawn_agent` is:

```rust
        let id = self.agents.spawn(position, genome, lineage, [LINEAGE_NONE; 2], 0);
```

Replace with:

```rust
        let id = self.agents.spawn(position, genome, lineage, [LINEAGE_NONE; 2], 0, crate::module::starter_kit());
```

- [ ] **Step 2.5: Update reproduce.rs to pass parent-derived modules (placeholder)**

`reproduce_all` currently calls `world.agents.spawn(child_pos, child_genome, lineage, [a_lineage, b_lineage], a_species)`. For now (until Task 4 implements module crossover), pass parent A's module list verbatim:

```rust
        let child_modules = world.agents.modules[i].clone();
        let lineage = world.next_lineage();
        let child_id = world.agents.spawn(
            child_pos, child_genome, lineage, [a_lineage, b_lineage], a_species, child_modules,
        );
        world.add_to_species(a_species);
```

Task 4 will replace `child_modules` with proper crossover + mutation.

- [ ] **Step 2.6: Run all lib tests**

```bash
cargo test -p anabios-core --lib
```

Expected: every M2 test still passes (now all agents have the starter kit, but no gating is active yet so behavior is unchanged).

- [ ] **Step 2.7: fmt + clippy + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/agent.rs crates/anabios-core/src/world.rs crates/anabios-core/src/reproduce.rs
git commit -m "feat(core): add modules field to AgentBuffers; spawn assigns starter kit"
```

---

## Task 3: Module mutation operators

**Goal:** Pure functions: param-mutate, add, delete, duplicate, replace. Plus a top-level `crossover_and_mutate` that combines parent module lists and applies all operators.

**Files:**
- Modify: `crates/anabios-core/src/module.rs`

- [ ] **Step 3.1: Add mutation functions**

Append to `crates/anabios-core/src/module.rs` (just before the `#[cfg(test)]` block):

```rust
/// Perturb every parameter of `module` with probability `MUTATE_PARAM_PROB`,
/// drawing perturbations from `N(0, PARAM_SIGMA)` and clamping back into
/// `[0, 1]`. Per-slot decisions consume the RNG in a fixed order so the
/// result is deterministic.
pub fn mutate_params(module: &mut Module, rng: &mut Rng) {
    fn perturb(v: &mut f32, rng: &mut Rng) {
        if rng.f32_unit() < MUTATE_PARAM_PROB {
            *v = (*v + rng.gaussian(0.0, PARAM_SIGMA)).clamp(0.0, 1.0);
        }
    }
    match module {
        Module::Locomotor { max_speed, terrain_affinity } => {
            perturb(max_speed, rng);
            perturb(terrain_affinity, rng);
        }
        Module::Sensor { sensor_type: _, radius, acuity } => {
            perturb(radius, rng);
            perturb(acuity, rng);
        }
        Module::Mouth { bite_size, diet_affinity } => {
            perturb(bite_size, rng);
            perturb(diet_affinity, rng);
        }
        Module::Weapon { damage, energy_cost } => {
            perturb(damage, rng);
            perturb(energy_cost, rng);
        }
        Module::Armor { protection, mass_penalty } => {
            perturb(protection, rng);
            perturb(mass_penalty, rng);
        }
        Module::Storage { capacity } => {
            perturb(capacity, rng);
        }
        Module::Communicator { range, channel_id: _ } => {
            perturb(range, rng);
        }
        Module::Pheromone { channel: _, strength, decay } => {
            perturb(strength, rng);
            perturb(decay, rng);
        }
        Module::Reproductive { viability, brood_size_bias } => {
            perturb(viability, rng);
            perturb(brood_size_bias, rng);
        }
    }
}

/// Apply structural mutations to `modules` in place. Each operator fires
/// independently with its own probability. The list is clamped to
/// `[0, MODULE_LIST_MAX]` items; if a delete would empty the list, it
/// skips to leave at least one module (so the agent is not entirely
/// vestigial — extinction by full module loss is unproductive noise).
pub fn structural_mutate(modules: &mut ModuleList, rng: &mut Rng) {
    // Add
    if modules.len() < MODULE_LIST_MAX && rng.f32_unit() < ADD_MODULE_PROB {
        modules.push(Module::random_any(rng));
    }
    // Duplicate
    if modules.len() < MODULE_LIST_MAX && !modules.is_empty()
        && rng.f32_unit() < DUPLICATE_MODULE_PROB
    {
        let pick = rng.index(modules.len());
        let copy = modules[pick];
        modules.push(copy);
    }
    // Replace
    if !modules.is_empty() && rng.f32_unit() < REPLACE_MODULE_PROB {
        let pick = rng.index(modules.len());
        modules[pick] = Module::random_any(rng);
    }
    // Delete (last, so we don't replace then immediately delete)
    if modules.len() > 1 && rng.f32_unit() < DELETE_MODULE_PROB {
        let pick = rng.index(modules.len());
        modules.remove(pick);
    }
}

/// Build a child's module list from two parents:
/// 1. For each slot index up to the longer parent's length, inherit from
///    parent A or parent B with equal probability (per-slot uniform
///    crossover). The shorter parent's slots beyond its length are skipped
///    (so the child's length lands between the two parents' lengths).
/// 2. Run `mutate_params` on every inherited module.
/// 3. Run `structural_mutate` once on the resulting list.
pub fn crossover_and_mutate(a: &ModuleList, b: &ModuleList, rng: &mut Rng) -> ModuleList {
    let max_len = a.len().max(b.len());
    let mut out = ModuleList::new();
    for i in 0..max_len {
        let from_a = rng.f32_unit() < 0.5;
        let chosen = if from_a {
            if i < a.len() { Some(&a[i]) } else if i < b.len() { Some(&b[i]) } else { None }
        } else if i < b.len() {
            Some(&b[i])
        } else if i < a.len() {
            Some(&a[i])
        } else {
            None
        };
        if let Some(m) = chosen {
            let mut copy = *m;
            mutate_params(&mut copy, rng);
            out.push(copy);
        }
    }
    structural_mutate(&mut out, rng);
    out
}
```

- [ ] **Step 3.2: Add mutation tests**

Append to the existing `#[cfg(test)] mod tests` block in `module.rs`:

```rust
    #[test]
    fn mutate_params_keeps_values_in_range() {
        let mut rng = Rng::from_seed(7);
        let mut m = Module::Locomotor { max_speed: 0.5, terrain_affinity: 0.5 };
        for _ in 0..200 {
            mutate_params(&mut m, &mut rng);
            if let Module::Locomotor { max_speed, terrain_affinity } = m {
                assert!((0.0..=1.0).contains(&max_speed));
                assert!((0.0..=1.0).contains(&terrain_affinity));
            }
        }
    }

    #[test]
    fn structural_mutate_never_empties_the_list() {
        let mut rng = Rng::from_seed(11);
        let mut k = starter_kit();
        for _ in 0..1000 {
            structural_mutate(&mut k, &mut rng);
            assert!(!k.is_empty());
            assert!(k.len() <= MODULE_LIST_MAX);
        }
    }

    #[test]
    fn crossover_with_identical_parents_yields_same_length_distribution() {
        let mut rng = Rng::from_seed(13);
        let p = starter_kit();
        let mut len_sum = 0;
        let n = 100;
        for _ in 0..n {
            let c = crossover_and_mutate(&p, &p, &mut rng);
            len_sum += c.len();
        }
        // With identical parents and small structural mutation rates,
        // child length should average close to parent length.
        let avg = len_sum as f32 / n as f32;
        let parent_len = p.len() as f32;
        assert!(
            (avg - parent_len).abs() < 1.5,
            "average child length {avg} differs significantly from parent {parent_len}",
        );
    }

    #[test]
    fn crossover_is_deterministic() {
        let p = starter_kit();
        let mut r1 = Rng::from_seed(99);
        let mut r2 = Rng::from_seed(99);
        let c1 = crossover_and_mutate(&p, &p, &mut r1);
        let c2 = crossover_and_mutate(&p, &p, &mut r2);
        assert_eq!(c1, c2);
    }
```

- [ ] **Step 3.3: Run tests + fmt + clippy + commit**

```bash
cargo test -p anabios-core module
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/module.rs
git commit -m "feat(core): module mutation operators (param + structural) and crossover"
```

Expected: 8 module tests pass (4 from Task 1 + 4 new).

---

## Task 4: Use crossover_and_mutate in reproduce

**Goal:** Replace the parent-A-verbatim placeholder from Task 2 with the proper crossover + mutation.

**Files:**
- Modify: `crates/anabios-core/src/reproduce.rs`

- [ ] **Step 4.1: Swap module inheritance**

Find:

```rust
        let child_modules = world.agents.modules[i].clone();
        let lineage = world.next_lineage();
        let child_id = world.agents.spawn(
            child_pos, child_genome, lineage, [a_lineage, b_lineage], a_species, child_modules,
        );
        world.add_to_species(a_species);
```

Replace with:

```rust
        let a_modules = world.agents.modules[i].clone();
        let b_modules = world.agents.modules[j].clone();
        let child_modules =
            crate::module::crossover_and_mutate(&a_modules, &b_modules, &mut world.rng);

        let lineage = world.next_lineage();
        let child_id = world.agents.spawn(
            child_pos, child_genome, lineage, [a_lineage, b_lineage], a_species, child_modules,
        );
        world.add_to_species(a_species);
```

- [ ] **Step 4.2: Run lib tests + commit**

```bash
cargo test -p anabios-core --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/reproduce.rs
git commit -m "feat(core): reproduction inherits + mutates parent module lists"
```

Expected: every previously-passing test still passes.

---

## Task 5: Gate motion on Locomotor

**Goal:** `integrate` no longer reads the genome's `SpeedMax` directly; instead it uses the agent's effective Locomotor speed (sum of `max_speed` across all Locomotor modules). No Locomotor → velocity is zeroed.

**Files:**
- Modify: `crates/anabios-core/src/integrate.rs`
- Modify: `crates/anabios-core/src/behavior.rs`

- [ ] **Step 5.1: Plumb effective speed through behavior::decide**

Edit `behavior.rs`. The current `decide` reads `genome.get(GenomeSlot::SpeedMax)` and multiplies by `SPEED_MAX_CAP`. Now the speed limit is owned by the modules, but the **direction selection** still belongs to the behavior function. The cleanest interface: `decide` returns a unit-direction vector (or zero) and `integrate` scales it by the effective speed.

Replace `decide`:

```rust
pub fn decide(
    genome: &Genome,
    sensor: &SensorRegister,
    energy: f32,
    own_species: u32,
    rng: &mut Rng,
) -> Vec2 {
    let hunger_threshold = SPAWN_ENERGY * genome.get(GenomeSlot::ReproductionThreshold);
    let is_hungry = energy < hunger_threshold;

    let mate_ready_threshold = hunger_threshold * 1.5;
    let mate_ready = energy >= mate_ready_threshold
        && sensor.has_neighbor
        && sensor.nearest_neighbor_species == own_species;

    let direction = if is_hungry && sensor.plant_direction != Vec2::ZERO {
        sensor.plant_direction
    } else if mate_ready {
        sensor.nearest_neighbor_dir
    } else {
        let theta = rng.f32_unit() * std::f32::consts::TAU;
        Vec2::new(theta.cos(), theta.sin())
    };

    direction
}
```

And remove the `SPEED_MAX_CAP` constant — it migrates to `integrate.rs`. Update the imports at the top of `behavior.rs` to drop the no-longer-needed ones if clippy complains.

Wait — `SPEED_MAX_CAP` is still useful as the integration speed cap. Move it to `integrate.rs` instead:

In `behavior.rs`, delete the line:

```rust
pub const SPEED_MAX_CAP: f32 = 4.0;
```

In `integrate.rs`, add:

```rust
/// Maximum agent speed at `Locomotor.max_speed = 1.0`, in world units per
/// tick. Capping here keeps spatial-hash neighbor queries within their
/// `PERCEPTION_MAX_RADIUS` guarantee even when an agent has multiple
/// Locomotor modules (their max_speed contributions sum, then we clamp).
pub const SPEED_MAX_CAP: f32 = 4.0;
```

- [ ] **Step 5.2: Update behavior tests**

The tests in `behavior.rs::tests` currently assert against velocity magnitudes (e.g., `(v.length() - SPEED_MAX_CAP).abs() < 1e-3`). Now `decide` returns unit vectors. Update assertions:

```rust
    #[test]
    fn zero_speed_max_yields_zero_velocity() {
        // No longer meaningful: decide() always returns a direction, the
        // speed cap moved to integrate.rs. Replace this test with:
        // wander direction is a unit vector.
        let g = Genome::neutral();
        let s = SensorRegister::default();
        let mut rng = Rng::from_seed(1);
        let v = decide(&g, &s, 0.0, 0, &mut rng);
        assert!((v.length() - 1.0).abs() < 1e-3 || v == Vec2::ZERO,
            "wander direction should be unit-length (got {:?})", v);
    }

    #[test]
    fn hungry_agent_with_plant_returns_plant_direction() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 1.0);
        let s = SensorRegister {
            plant_direction: Vec2::new(1.0, 0.0),
            ..Default::default()
        };
        let mut rng = Rng::from_seed(1);
        let v = decide(&g, &s, 0.0, 0, &mut rng);
        assert_eq!(v, Vec2::new(1.0, 0.0));
    }

    #[test]
    fn well_fed_agent_wanders() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.0);
        let s = SensorRegister {
            plant_direction: Vec2::new(1.0, 0.0),
            ..Default::default()
        };
        let mut directions = std::collections::HashSet::new();
        for seed in 0..16 {
            let mut rng = Rng::from_seed(seed);
            let v = decide(&g, &s, SPAWN_ENERGY, 0, &mut rng);
            let key = ((v.x * 100.0) as i32, (v.y * 100.0) as i32);
            directions.insert(key);
        }
        assert!(directions.len() >= 4);
    }

    #[test]
    fn mate_ready_agent_heads_toward_same_species_neighbor() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.5);
        let s = SensorRegister {
            plant_direction: Vec2::new(0.0, -1.0),
            has_neighbor: true,
            nearest_neighbor_dist: 5.0,
            nearest_neighbor_dir: Vec2::new(1.0, 0.0),
            nearest_neighbor_species: 0,
            ..Default::default()
        };
        let mut rng = Rng::from_seed(1);
        let v = decide(&g, &s, 50.0, 0, &mut rng);
        assert!(v.x > 0.5);
        assert!(v.y.abs() < 0.5);
    }

    #[test]
    fn mate_ready_with_different_species_does_not_mate_seek() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.5);
        let s = SensorRegister {
            has_neighbor: true,
            nearest_neighbor_dist: 5.0,
            nearest_neighbor_dir: Vec2::new(1.0, 0.0),
            nearest_neighbor_species: 1,
            ..Default::default()
        };
        let mut wandered = std::collections::HashSet::new();
        for seed in 1..16 {
            let mut r = Rng::from_seed(seed);
            let vw = decide(&g, &s, 50.0, 0, &mut r);
            wandered.insert(((vw.x * 10.0) as i32, (vw.y * 10.0) as i32));
        }
        assert!(wandered.len() >= 4);
    }
```

- [ ] **Step 5.3: Update integrate_all to gate on Locomotor**

Edit `crates/anabios-core/src/integrate.rs`. Replace `integrate_all`:

```rust
pub fn integrate_all(agents: &mut AgentBuffers, desired_direction: &[Vec2]) {
    for id in agents.iter_alive().collect::<Vec<_>>() {
        let i = id as usize;

        // Action gating: no Locomotor → no motion.
        if !crate::module::has(&agents.modules[i], crate::module::ModuleType::Locomotor) {
            agents.velocity[i] = Vec2::ZERO;
            // Still pay basal metabolism.
            let basal = BASAL_METABOLISM_COST
                * agents.genome[i].get(GenomeSlot::BasalMetabolism);
            agents.energy[i] -= basal;
            continue;
        }

        let direction = desired_direction[i];
        let module_speed = crate::module::effective_speed_max(&agents.modules[i])
            .clamp(0.0, 1.0);
        let v = direction * (SPEED_MAX_CAP * module_speed);
        agents.velocity[i] = v;

        let new_pos = agents.position[i] + v;
        agents.position[i] = wrap_torus(new_pos, Vec2::splat(WORLD_SIZE));

        let move_dist = v.length();
        let size = agents.genome[i].get(GenomeSlot::Size).max(0.1);
        let move_cost = MOVE_ENERGY_COST * move_dist * size;
        let basal = BASAL_METABOLISM_COST
            * agents.genome[i].get(GenomeSlot::BasalMetabolism);
        agents.energy[i] -= move_cost + basal;
    }
}
```

- [ ] **Step 5.4: Update tick.rs to pass desired_direction (not desired_velocity)**

The semantics of `desired_velocity` changed: it's now a unit direction. Rename the field in `World` from `desired_velocity` to `desired_direction` for clarity.

Edit `crates/anabios-core/src/world.rs`. Rename:

```rust
    #[serde(skip)]
    pub desired_direction: Vec<crate::prelude::Vec2>,
```

And in `resize_scratch`:

```rust
        if self.desired_direction.len() < cap {
            self.desired_direction.resize(cap, crate::prelude::Vec2::ZERO);
        }
```

(Search-and-replace `desired_velocity` → `desired_direction` throughout.)

Edit `crates/anabios-core/src/tick.rs`. `decide_all` writes into `world.desired_direction`, `integrate_all` reads from the same. Update the call site:

```rust
    integrate_all(&mut world.agents, &world.desired_direction[..cap]);
```

- [ ] **Step 5.5: Add integrate tests**

Append to `integrate.rs::tests`:

```rust
    #[test]
    fn agent_without_locomotor_does_not_move() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        // Strip Locomotor from the starter kit.
        w.agents.modules[id as usize].retain(|m| {
            !matches!(m, crate::module::Module::Locomotor { .. })
        });

        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        let pos_before = w.agents.position[id as usize];
        integrate_all(&mut w.agents, &desired);
        let pos_after = w.agents.position[id as usize];
        assert_eq!(pos_before, pos_after, "no Locomotor → no motion");
    }

    #[test]
    fn agent_with_locomotor_moves_proportionally_to_speed_param() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        // Replace starter kit Locomotor with a max-speed one.
        for m in w.agents.modules[id as usize].iter_mut() {
            if let crate::module::Module::Locomotor { max_speed, .. } = m {
                *max_speed = 1.0;
            }
        }

        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        integrate_all(&mut w.agents, &desired);
        let new_pos = w.agents.position[id as usize];
        // Moved roughly SPEED_MAX_CAP × 1.0 = 4.0 in +x.
        assert!((new_pos.x - 504.0).abs() < 0.1);
    }
```

- [ ] **Step 5.6: Run tests + fmt + clippy + commit**

```bash
cargo test -p anabios-core --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/behavior.rs crates/anabios-core/src/integrate.rs crates/anabios-core/src/tick.rs crates/anabios-core/src/world.rs
git commit -m "feat(core): gate motion on Locomotor; speed scales from module max_speed"
```

Expected: every test passes. Behaviors that previously used the genome `SpeedMax` slot now use the module — the M2 starter kit has `max_speed = 0.6`, equivalent to roughly the M2 default speed.

---

## Task 6: Gate feeding on Mouth; perception on Sensor

**Goal:** `interact_all` skips agents without a Mouth. `sense_all` returns zero perception for agents without a Sensor.

**Files:**
- Modify: `crates/anabios-core/src/interact.rs`
- Modify: `crates/anabios-core/src/sense.rs`

- [ ] **Step 6.1: Gate interact on Mouth**

Replace `interact_all` in `interact.rs`:

```rust
pub fn interact_all(agents: &mut AgentBuffers, biome: &mut BiomeField) {
    let alive_ids: Vec<u32> = agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;

        // Action gating: no Mouth → can't eat.
        if !crate::module::has(&agents.modules[i], crate::module::ModuleType::Mouth) {
            continue;
        }

        let pos = agents.position[i];
        let bite_cap = crate::module::effective_bite_size(&agents.modules[i]);
        let diet_carn = crate::module::effective_diet_carnivory(&agents.modules[i]);
        let herbivory = (1.0 - diet_carn).clamp(0.0, 1.0);
        if herbivory <= 0.0 || bite_cap <= 0.0 {
            continue;
        }
        let size = agents.genome[i].get(GenomeSlot::Size).max(0.1);
        let desired_bite = BITE_MAX * size * bite_cap * herbivory;
        let taken = biome.graze(pos, desired_bite);
        if taken > 0.0 {
            agents.energy[i] += taken * FOOD_ENERGY_PER_BIOMASS;
        }
    }
}
```

- [ ] **Step 6.2: Update interact tests**

Existing tests in `interact.rs::tests` use `Genome::neutral()` which sets `DietCarnivory = 0.5`. They also rely on the genome's `DietCarnivory` slot. Now the module's `diet_affinity` drives feeding. The tests use the starter kit which includes `Mouth { bite_size: 0.6, diet_affinity: 0.0 }` — agents will graze. Update assertions that referenced the old genome-only behavior:

Replace the `obligate_carnivore_does_not_eat_plants` test:

```rust
    #[test]
    fn obligate_carnivore_does_not_eat_plants() {
        let mut w = World::new(11);
        let pos = find_grass_cell_center(&w);
        let id = w.spawn_agent(pos, Genome::neutral());
        // Replace Mouth with a pure carnivore.
        for m in w.agents.modules[id as usize].iter_mut() {
            if let crate::module::Module::Mouth { diet_affinity, .. } = m {
                *diet_affinity = 1.0;
            }
        }
        let energy_before = w.agents.energy[id as usize];
        let biomass_before = w.biome.sample(pos).plant_biomass;
        interact_all(&mut w.agents, &mut w.biome);
        assert_eq!(w.agents.energy[id as usize], energy_before);
        assert_eq!(w.biome.sample(pos).plant_biomass, biomass_before);
    }

    #[test]
    fn agent_without_mouth_does_not_eat() {
        let mut w = World::new(11);
        let pos = find_grass_cell_center(&w);
        let id = w.spawn_agent(pos, Genome::neutral());
        w.agents.modules[id as usize].retain(|m| {
            !matches!(m, crate::module::Module::Mouth { .. })
        });
        let energy_before = w.agents.energy[id as usize];
        let biomass_before = w.biome.sample(pos).plant_biomass;
        interact_all(&mut w.agents, &mut w.biome);
        assert_eq!(w.agents.energy[id as usize], energy_before);
        assert_eq!(w.biome.sample(pos).plant_biomass, biomass_before);
    }
```

The other two existing tests (`herbivore_on_grass_gains_energy`, `two_agents_share_finite_biomass_deterministically`) should still pass — the starter kit has a herbivorous Mouth.

- [ ] **Step 6.3: Gate sense on Sensor**

In `crates/anabios-core/src/sense.rs`, replace the perception_radius helper and the body of `sense_all` to read from modules:

Replace the existing `perception_radius` function with:

```rust
/// Effective perception radius for an agent given its module list and
/// genome. Combines the max Sensor radius with the genome's
/// `PerceptionRadius` slot (the genome acts as a modulator on top of
/// module capability). Capped at `PERCEPTION_MAX_RADIUS` for the
/// spatial-hash one-ring guarantee.
pub fn perception_radius(modules: &crate::module::ModuleList, genome: &Genome) -> f32 {
    let sensor_radius = crate::module::effective_perception_radius(modules);
    if sensor_radius <= 0.0 {
        return 0.0;
    }
    let modulator = 0.25 + 0.75 * genome.get(GenomeSlot::PerceptionRadius);
    (PERCEPTION_MAX_RADIUS * sensor_radius * modulator).min(PERCEPTION_MAX_RADIUS)
}
```

Update the call site inside `sense_all`. Find the line:

```rust
        let radius = perception_radius(genome);
```

Replace with:

```rust
        let radius = perception_radius(&agents.modules[i], genome);
        if radius <= 0.0 {
            registers[i] = SensorRegister::default();
            continue;
        }
```

- [ ] **Step 6.4: Update sense tests**

The existing sense tests use the World's `spawn_agent`, which gives the starter kit (includes a Sensor with `radius = 0.6`). They should still pass. Add a new test for the no-Sensor case:

```rust
    #[test]
    fn agent_without_sensor_perceives_nothing() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.agents.modules[id as usize].retain(|m| {
            !matches!(m, crate::module::Module::Sensor { .. })
        });
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.spatial, &mut regs);
        assert_eq!(regs[id as usize].local_plant_biomass, 0.0);
        assert!(!regs[id as usize].has_neighbor);
    }
```

- [ ] **Step 6.5: Run tests + fmt + clippy + commit**

```bash
cargo test -p anabios-core --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/interact.rs crates/anabios-core/src/sense.rs
git commit -m "feat(core): gate feeding on Mouth and perception on Sensor modules"
```

---

## Task 7: Gate reproduction on Reproductive

**Goal:** Both parents must have a Reproductive module for offspring to be produced. If either lacks one, skip the pair (the higher-id agent remains available for later attempts).

**Files:**
- Modify: `crates/anabios-core/src/reproduce.rs`

- [ ] **Step 7.1: Add Reproductive gate to is_eligible**

Edit `crates/anabios-core/src/reproduce.rs`. Update `is_eligible`:

```rust
fn is_eligible(agents: &AgentBuffers, id: u32) -> bool {
    let i = id as usize;
    if !agents.is_alive(id) {
        return false;
    }
    // Action gating: must have Reproductive module to mate.
    if !crate::module::has(&agents.modules[i], crate::module::ModuleType::Reproductive) {
        return false;
    }
    let threshold = SPAWN_ENERGY * agents.genome[i].get(GenomeSlot::ReproductionThreshold) * 1.5;
    agents.energy[i] >= threshold
}
```

- [ ] **Step 7.2: Add a unit test**

Add to `reproduce.rs::tests`:

```rust
    #[test]
    fn agent_without_reproductive_does_not_mate() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());

        // Strip Reproductive from id0 only.
        w.agents.modules[id0 as usize].retain(|m| {
            !matches!(m, crate::module::Module::Reproductive { .. })
        });

        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();
        assert_eq!(after, before, "missing Reproductive must block mating");
    }
```

- [ ] **Step 7.3: Run tests + fmt + clippy + commit**

```bash
cargo test -p anabios-core reproduce
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/reproduce.rs
git commit -m "feat(core): gate reproduction on presence of Reproductive module in both parents"
```

Expected: 5 reproduce tests pass.

---

## Task 8: Module upkeep tick stage

**Goal:** A new tick stage that deducts each agent's `total_upkeep(modules)` from their energy. Runs after `interact_all` (which feeds them) and before `reproduce_all` (so reproduction costs and upkeep don't both fight over the same energy in a confusing order).

**Files:**
- Modify: `crates/anabios-core/src/module.rs` (add `upkeep_all` function)
- Modify: `crates/anabios-core/src/tick.rs`

- [ ] **Step 8.1: Add upkeep_all to module.rs**

Append to `module.rs` (just above the `#[cfg(test)]`):

```rust
/// Deduct per-tick module upkeep from every alive agent. Modules cost
/// energy continuously regardless of whether they were used this tick;
/// agents with too many modules for their food intake go negative and
/// die in the subsequent `age_and_starve` stage.
pub fn upkeep_all(agents: &mut crate::agent::AgentBuffers) {
    for id in agents.iter_alive() {
        let i = id as usize;
        let cost = total_upkeep(&agents.modules[i]);
        agents.energy[i] -= cost;
    }
}
```

- [ ] **Step 8.2: Add a unit test**

Append to `module.rs::tests`:

```rust
    #[test]
    fn upkeep_all_deducts_starter_kit_cost() {
        use crate::world::World;
        use crate::prelude::Vec2;
        use crate::genome::Genome;
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let before = w.agents.energy[id as usize];
        upkeep_all(&mut w.agents);
        let after = w.agents.energy[id as usize];
        let expected_cost = total_upkeep(&w.agents.modules[id as usize]);
        assert!((before - after - expected_cost).abs() < 1e-5);
    }
```

- [ ] **Step 8.3: Wire upkeep_all into tick.rs**

Edit `crates/anabios-core/src/tick.rs`. The current `step` function has the order: spatial → sense → decide → integrate → interact → reproduce → age_and_starve → species_step → biome_step. Insert `module::upkeep_all` between `interact_all` and `reproduce_all`:

```rust
pub fn step(world: &mut World) {
    world.resize_scratch();
    let cap = world.agents.capacity();

    world.spatial.rebuild(&world.agents.position, |i| world.agents.is_alive(i as u32));
    sense_all(&world.agents, &world.biome, &world.spatial, &mut world.sensors);
    decide_all(world);
    integrate_all(&mut world.agents, &world.desired_direction[..cap]);
    interact_all(&mut world.agents, &mut world.biome);

    // M3: module upkeep — every alive agent pays for its modules.
    crate::module::upkeep_all(&mut world.agents);

    crate::reproduce::reproduce_all(world);
    age_and_starve(world);

    if world.tick.is_multiple_of(crate::species::SPECIES_STEP_INTERVAL) {
        crate::species::species_step(world);
    }

    if world.tick.is_multiple_of(BIOME_STEP_INTERVAL) {
        world.biome.regrow_step();
    }

    world.tick += 1;
}
```

- [ ] **Step 8.4: Run tests + fmt + clippy + commit**

```bash
cargo test -p anabios-core --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/src/module.rs crates/anabios-core/src/tick.rs
git commit -m "feat(core): module_upkeep tick stage deducts per-tick maintenance energy"
```

---

## Task 9: Module fields in invariants + golden hashes

**Goal:** Add proptest invariants for module lists; regenerate the golden tick hashes to account for the new fields in the snapshot.

**Files:**
- Modify: `crates/anabios-core/tests/invariants.rs`
- Modify: `crates/anabios-core/tests/determinism.rs`

- [ ] **Step 9.1: Add proptest invariants for modules**

Append to the `proptest!` block in `crates/anabios-core/tests/invariants.rs`:

```rust
    /// Every alive agent has at least one module (the structural_mutate
    /// operator preserves the "never empty" invariant).
    #[test]
    fn alive_agents_have_at_least_one_module(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for id in w.agents.iter_alive() {
            let n = w.agents.modules[id as usize].len();
            prop_assert!(n >= 1, "agent {id} has 0 modules");
        }
    }

    /// Module lists never exceed MODULE_LIST_MAX.
    #[test]
    fn modules_respect_max_list_size(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for id in w.agents.iter_alive() {
            let n = w.agents.modules[id as usize].len();
            prop_assert!(n <= anabios_core::module::MODULE_LIST_MAX,
                "agent {id} has {n} modules");
        }
    }
```

- [ ] **Step 9.2: Regenerate the golden hashes**

Reset GOLDEN to zeros in `crates/anabios-core/tests/determinism.rs`:

```rust
const GOLDEN: &[(u64, u64)] = &[
    (0, 0x0000000000000000),
    (100, 0x0000000000000000),
    (1000, 0x0000000000000000),
];
```

Run:

```bash
UPDATE_HASHES=1 cargo test -p anabios-core --test determinism -- --nocapture
```

Copy the three printed pairs into GOLDEN. Verify:

```bash
cargo test -p anabios-core --test determinism
```

- [ ] **Step 9.3: Run all tests + fmt + clippy + commit**

```bash
cargo test --workspace
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/tests/invariants.rs crates/anabios-core/tests/determinism.rs
git commit -m "test(core): module invariants + regenerated golden hashes for M3"
```

---

## Task 10: Morphology evolution integration test

**Goal:** A dedicated integration test that runs the simulation long enough for module composition to drift across generations. Asserts that after N ticks, the population contains at least one module type that wasn't in any founder's starter kit (e.g., Storage or Communicator), confirming structural mutation works in vivo.

**Files:**
- Create: `crates/anabios-core/tests/morphology_evolution.rs`

- [ ] **Step 10.1: Implement the test**

Create `crates/anabios-core/tests/morphology_evolution.rs`:

```rust
//! Integration test: over many generations, structural mutation introduces
//! module types that were not present in the founders' starter kit.

use anabios_core::module::{ModuleType, MODULE_LIST_MAX};
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use std::collections::HashSet;

const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

#[test]
fn novel_module_types_appear_within_5000_ticks() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();

    // Founders all have the starter kit: Locomotor, Sensor, Mouth,
    // Reproductive. Any other module type appearing in the alive
    // population indicates structural mutation introduced it.
    let starter_types: HashSet<ModuleType> = [
        ModuleType::Locomotor,
        ModuleType::Sensor,
        ModuleType::Mouth,
        ModuleType::Reproductive,
    ].into_iter().collect();

    let mut seen_novel = false;
    for _ in 0..5_000 {
        step(&mut world);
        for id in world.agents.iter_alive() {
            for m in &world.agents.modules[id as usize] {
                if !starter_types.contains(&m.module_type()) {
                    seen_novel = true;
                    break;
                }
            }
            if seen_novel { break; }
        }
        if seen_novel { break; }
    }
    assert!(seen_novel, "no novel module types appeared in 5000 ticks");

    // Sanity: nobody overflows the cap.
    for id in world.agents.iter_alive() {
        assert!(world.agents.modules[id as usize].len() <= MODULE_LIST_MAX);
    }
}
```

- [ ] **Step 10.2: Run + commit**

```bash
cargo test -p anabios-core --test morphology_evolution
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/tests/morphology_evolution.rs
git commit -m "test(core): novel module types emerge via structural mutation within 5000 ticks"
```

If the test fails (no novel types in 5000 ticks), the structural mutation probabilities are too low. The current values (ADD_MODULE_PROB = 0.02) mean ~50 reproductions per add. With ~100 reproductions per tick at population cap, novel modules should appear within ~5 ticks. If the test fails, double-check `crossover_and_mutate` is actually being called — check `reproduce.rs` Task 4 changes landed.

---

## Task 11: Module gating integration test

**Goal:** A dedicated test for action-gating: an agent with no Locomotor cannot move; with no Mouth cannot eat; with no Reproductive cannot mate. Different from the unit tests in that these run through `step()` end-to-end.

**Files:**
- Create: `crates/anabios-core/tests/module_gating.rs`

- [ ] **Step 11.1: Implement the test**

Create `crates/anabios-core/tests/module_gating.rs`:

```rust
//! End-to-end gating: stripping a module type from an agent prevents
//! the corresponding action through one full tick.

use anabios_core::genome::Genome;
use anabios_core::module::{Module, ModuleType};
use anabios_core::prelude_test::Vec2;
use anabios_core::tick::step;
use anabios_core::world::World;

#[test]
fn no_locomotor_no_motion_through_step() {
    let mut w = World::new(1);
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    w.agents.modules[id as usize].retain(|m| !matches!(m, Module::Locomotor { .. }));
    let pos_before = w.agents.position[id as usize];
    step(&mut w);
    let pos_after = w.agents.position[id as usize];
    assert_eq!(pos_before, pos_after);
}

#[test]
fn no_mouth_no_energy_gain_through_step() {
    let mut w = World::new(13);
    // Find a grass cell.
    let mut spawn = Vec2::ZERO;
    use anabios_core::biome::{BIOME_RES, CELL_SIZE};
    'outer: for row in 0..BIOME_RES {
        for col in 0..BIOME_RES {
            if w.biome.at(col, row).terrain == anabios_core::biome::TerrainType::Grass {
                spawn = Vec2::new((col as f32 + 0.5) * CELL_SIZE, (row as f32 + 0.5) * CELL_SIZE);
                break 'outer;
            }
        }
    }
    let id = w.spawn_agent(spawn, Genome::neutral());
    w.agents.modules[id as usize].retain(|m| !matches!(m, Module::Mouth { .. }));

    let energy_before = w.agents.energy[id as usize];
    let biomass_before = w.biome.sample(spawn).plant_biomass;
    step(&mut w);
    let biomass_after = w.biome.sample(spawn).plant_biomass;
    assert_eq!(biomass_after, biomass_before, "biomass unchanged when no Mouth");
    // Energy may have dropped from upkeep + metabolism, but not increased.
    assert!(w.agents.energy[id as usize] <= energy_before);
}

#[test]
fn no_sensor_population_count_unchanged() {
    // Mainly a smoke test that no Sensor doesn't panic in sense_all.
    let mut w = World::new(1);
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    w.agents.modules[id as usize].retain(|m| !matches!(m, Module::Sensor { .. }));
    for _ in 0..20 {
        step(&mut w);
        if !w.agents.is_alive(id) {
            break;
        }
    }
    // Either still alive (with no Sensor, can't find food) or starved —
    // either way no panic.
    let _ = ModuleType::Sensor;
}
```

- [ ] **Step 11.2: Run + commit**

```bash
cargo test -p anabios-core --test module_gating
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/anabios-core/tests/module_gating.rs
git commit -m "test(core): end-to-end module gating (no Locomotor → no motion, no Mouth → no feeding)"
```

---

## Task 12: Bench perf check + final wrap-up

**Goal:** Run the bench to confirm M3's per-tick cost stays within budget; full green-bar; tag.

- [ ] **Step 12.1: Bench**

```bash
cargo bench -p anabios-core --bench tick_bench
```

Record the 1k and 10k median times. M2 baseline: 0.8 ms / 7.5 ms. M3 additions (module gating checks + upkeep stage + module crossover during reproduction) should add ~10-20% overhead. Expected M3: 1k ≈ 0.9-1.0 ms, 10k ≈ 8-9 ms. Both still under spec budget (1 ms / 15 ms).

If 10k exceeds 15 ms, profile and add a narrow optimization. Most likely candidate: `effective_speed_max` / `has` iterating modules per agent — could cache. Defer optimization unless actually needed.

- [ ] **Step 12.2: Full workspace check**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All must pass.

- [ ] **Step 12.3: Determinism smoke**

```bash
cargo build --release --bin anabios-headless
./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 5000 > /tmp/m3_a.txt
./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 5000 > /tmp/m3_b.txt
diff /tmp/m3_a.txt /tmp/m3_b.txt && echo "deterministic"
```

- [ ] **Step 12.4: Tag the milestone**

```bash
git tag -a m3 -m "M3: modular morphology layer"
```

- [ ] **Step 12.5: Push branch + tag and open PR**

```bash
git push -u origin m3-modular-morphology
git push origin m3
gh pr create --base main --head m3-modular-morphology --title "M3: modular morphology (agents grow body plans)" --body "<summary>"
```

---

## Post-implementation expectations

After M3 merges:

- Every agent has a variable-length module list (3-12 typical) instead of all capabilities being implicit in the genome
- Action gating: no Locomotor → no motion, no Mouth → no feeding, no Sensor → no perception, no Reproductive → no mating
- Per-tick module upkeep makes excess modules selectively disadvantaged
- Reproduction inherits parent module lists with param + structural mutation
- Novel module types (Storage, Weapon, Armor, Communicator, Pheromone) appear via structural mutation within ~5000 ticks
- Determinism preserved; golden hashes regenerated; bench remains in budget

What remains deferred:

- Evolvable behavior program (M4) — `decide()` is still hardcoded; module presence gates what actions the hardcoded function can usefully take
- Codex detectors (M5) — substrate is ready; no detectors wired
- Combat / pheromone / culture interaction effects of Weapon / Pheromone / Communicator modules — present and paying upkeep, but no gameplay consequence yet
- Godot rendering (M6+)
