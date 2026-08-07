# M1 — Headless Core Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up `anabios-core` as a pure, deterministic Rust simulation crate where 50-float-genome agents roam a 2D toroidal biome and eat plants, plus a thin `anabios-headless` CLI that runs scenarios from the command line.

**Architecture:** Cargo workspace with two crates initially (`anabios-core` library, `anabios-headless` binary). Struct-of-Arrays agent buffers inside a `World`. Deterministic tick pipeline driven by a single `Xoshiro256++` RNG. Tests are the primary deliverable alongside the code — every system has unit tests, property tests covering invariants, and golden-tick hashes pinned in CI.

**Tech Stack:**
- Rust stable (matches the rest of the workspace)
- `glam` (Vec2 math — same library underneath rapier2d, which `evolve-physics` already uses)
- `rand` + `rand_xoshiro` (deterministic RNG)
- `serde` + `bincode` (snapshot serialization)
- `smallvec` (already needed for later milestones — pull in early)
- `bitvec` (dense liveness mask)
- `clap` (CLI for `anabios-headless`)
- `proptest` (property tests, dev-dep)
- `criterion` (benches, dev-dep)
- `rayon` (parallelism — added in M1 even though early stages are mostly serial, to lock in the threading discipline early)

**Scope explicitly excluded from M1** (deferred to later milestones, do NOT add them):

- Reproduction, mutation, speciation (M2)
- Modular morphology (M3)
- Behavior program / expression tree (M4) — M1 uses a single hardcoded behavior function
- Codex detectors (M5)
- Godot / gdext (M6+)
- Pheromone fields, culture/meme vectors (later milestones — biome layer only stores plants/terrain in M1)
- Cross-platform `sin`/`cos` wrappers (M1 documents the discipline; the wrapping is not required until M4 when the behavior program needs nonlinear ops)

**Style conventions** (apply to every code block in this plan):

- 4-space indentation, no tabs (matches Rust default `rustfmt`)
- Public types/functions get rustdoc `///` comments only when their behavior is non-obvious from the name — no doc-for-doc's-sake
- All randomness goes through `World.rng`, never `rand::thread_rng()` or `SmallRng::from_entropy()`
- No `unwrap()` on operations that depend on runtime data; `expect("invariant: …")` with an invariant message is acceptable when truly unreachable
- Commit messages use Conventional Commits prefixes (`feat:`, `test:`, `chore:`, `refactor:`, `bench:`, `docs:`)

**Working directory:** All commands assume cwd = `/Users/aryasen/projects/anabios/`. Every `git` command in this plan runs from that root.

---

## File structure (locked at start of plan)

The M1 deliverable produces this tree:

```
anabios/
├── Cargo.toml                              # workspace manifest
├── rust-toolchain.toml                     # pin stable
├── rustfmt.toml                            # formatting config
├── crates/
│   ├── anabios-core/
│   │   ├── Cargo.toml
│   │   ├── benches/
│   │   │   └── tick_bench.rs               # criterion benches
│   │   ├── src/
│   │   │   ├── lib.rs                      # crate root, module declarations, pub re-exports
│   │   │   ├── prelude.rs                  # internal use prelude
│   │   │   ├── rng.rs                      # Rng wrapper around Xoshiro256PlusPlus
│   │   │   ├── genome.rs                   # Genome type + trait slot constants
│   │   │   ├── biome.rs                    # BiomeField + plant regrowth
│   │   │   ├── spatial.rs                  # UniformSpatialHash
│   │   │   ├── agent.rs                    # AgentBuffers (SoA) + spawn/kill
│   │   │   ├── world.rs                    # World struct (owns everything)
│   │   │   ├── behavior.rs                 # M1 hardcoded behavior function
│   │   │   ├── tick.rs                     # tick() orchestrator (sense, decide, act, etc.)
│   │   │   ├── snapshot.rs                 # bincode save/load + state hash
│   │   │   └── scenario.rs                 # Scenario struct (initial conditions) + load_toml
│   │   └── tests/
│   │       ├── determinism.rs              # golden-tick hash test
│   │       ├── invariants.rs               # proptest properties
│   │       └── feeding.rs                  # integration: agent eats and survives
│   └── anabios-headless/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs                     # clap CLI: run/info
├── scenarios/
│   └── minimal.toml                        # 200 agents, simple biome, used in tests
├── .github/
│   └── workflows/
│       └── ci.yml                          # rustfmt, clippy, test, bench-compare, determinism
└── docs/superpowers/
    ├── specs/2026-05-23-anabios-design.md  # (already exists from brainstorming)
    └── plans/2026-05-23-m1-headless-core-skeleton.md  # this file
```

Boundary rules in force from day one:

- `anabios-core` has no Godot, no I/O dependencies, no `std::time` reads inside the tick path.
- `anabios-headless` is the only place that touches `std::fs` for scenario loading and snapshot writing.
- Tests in `crates/anabios-core/tests/` are integration tests (separate binary per file). Inline unit tests live in `#[cfg(test)] mod tests` blocks inside each `.rs` file in `src/`.

---

## Task 1: Workspace scaffolding & tooling

**Goal:** Initialize the Cargo workspace, lock the toolchain, and verify the empty crates build.

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `rustfmt.toml`
- Create: `crates/anabios-core/Cargo.toml`
- Create: `crates/anabios-core/src/lib.rs`
- Create: `crates/anabios-headless/Cargo.toml`
- Create: `crates/anabios-headless/src/main.rs`

- [ ] **Step 1.1: Write the workspace manifest**

Write `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/anabios-core", "crates/anabios-headless"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/aryasen/anabios"

[workspace.dependencies]
glam = { version = "0.27", features = ["serde"] }
rand = { version = "0.8", default-features = false, features = ["std", "std_rng"] }
rand_xoshiro = { version = "0.6", features = ["serde1"] }
serde = { version = "1", features = ["derive"] }
bincode = "1.3"
smallvec = { version = "1.13", features = ["serde", "const_generics"] }
bitvec = { version = "1", features = ["serde"] }
rayon = "1.10"
clap = { version = "4", features = ["derive"] }
toml = "0.8"
anyhow = "1"
thiserror = "1"
tracing = "0.1"

[workspace.dependencies.proptest]
version = "1.4"

[workspace.dependencies.criterion]
version = "0.5"
features = ["html_reports"]

[profile.release]
lto = "thin"
codegen-units = 1
opt-level = 3

[profile.bench]
inherits = "release"
debug = true
```

- [ ] **Step 1.2: Pin the Rust toolchain**

Write `rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

- [ ] **Step 1.3: Configure rustfmt**

Write `rustfmt.toml`:

```toml
edition = "2021"
max_width = 100
use_small_heuristics = "Max"
imports_granularity = "Module"
group_imports = "StdExternalCrate"
reorder_imports = true
```

- [ ] **Step 1.4: Create the anabios-core crate manifest**

Write `crates/anabios-core/Cargo.toml`:

```toml
[package]
name = "anabios-core"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Deterministic agent-based ecology simulation core for anabios."

[dependencies]
glam = { workspace = true }
rand = { workspace = true }
rand_xoshiro = { workspace = true }
serde = { workspace = true }
bincode = { workspace = true }
smallvec = { workspace = true }
bitvec = { workspace = true }
rayon = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
toml = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
criterion = { workspace = true }
anyhow = { workspace = true }
```

The `[[bench]]` target is declared later (Task 22), once the bench source file exists — adding it earlier would break `cargo clippy --all-targets`.

- [ ] **Step 1.5: Create the anabios-core skeleton lib**

Write `crates/anabios-core/src/lib.rs`:

```rust
//! anabios-core — deterministic agent-based ecology simulation.
//!
//! This crate has no Godot, no file I/O, no wall-clock reads. Pure functions
//! over state buffers. Given the same seed and scenario, every run is
//! bit-identical.

pub mod agent;
pub mod behavior;
pub mod biome;
pub mod genome;
pub mod rng;
pub mod scenario;
pub mod snapshot;
pub mod spatial;
pub mod tick;
pub mod world;

mod prelude;

pub use agent::AgentId;
pub use genome::{Genome, GenomeSlot};
pub use scenario::Scenario;
pub use world::World;
```

- [ ] **Step 1.6: Create the empty source modules**

Create each of the following files with a single-line placeholder so the crate compiles. Each will be filled out in later tasks.

`crates/anabios-core/src/prelude.rs`:

```rust
//! Internal prelude used across the crate.

pub(crate) use glam::Vec2;
```

`crates/anabios-core/src/rng.rs`:

```rust
//! Deterministic RNG wrapper. Filled out in Task 3.
```

`crates/anabios-core/src/genome.rs`:

```rust
//! 50-float genome. Filled out in Task 4.
```

`crates/anabios-core/src/biome.rs`:

```rust
//! 128×128 biome field. Filled out in Tasks 5-6.
```

`crates/anabios-core/src/spatial.rs`:

```rust
//! Uniform-grid spatial hash. Filled out in Task 7.
```

`crates/anabios-core/src/agent.rs`:

```rust
//! Struct-of-Arrays agent buffers. Filled out in Task 8.

/// Stable agent identifier. `u32::MAX` is reserved as a null sentinel.
pub type AgentId = u32;
```

`crates/anabios-core/src/world.rs`:

```rust
//! World root struct. Filled out in Task 9.
```

`crates/anabios-core/src/behavior.rs`:

```rust
//! M1 hardcoded behavior function. Filled out in Task 11.
```

`crates/anabios-core/src/tick.rs`:

```rust
//! Tick orchestration. Filled out in Task 14.
```

`crates/anabios-core/src/snapshot.rs`:

```rust
//! Snapshot save/load + state hash. Filled out in Task 15.
```

`crates/anabios-core/src/scenario.rs`:

```rust
//! Scenario initial conditions + TOML loading. Filled out in Task 17.
```

- [ ] **Step 1.7: Create the anabios-headless crate manifest**

Write `crates/anabios-headless/Cargo.toml`:

```toml
[package]
name = "anabios-headless"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Headless CLI runner for anabios scenarios."

[dependencies]
anabios-core = { path = "../anabios-core" }
clap = { workspace = true }
anyhow = { workspace = true }
serde = { workspace = true }
tracing = { workspace = true }

[[bin]]
name = "anabios-headless"
path = "src/main.rs"
```

- [ ] **Step 1.8: Create the headless main stub**

Write `crates/anabios-headless/src/main.rs`:

```rust
//! Headless runner for anabios scenarios. Fleshed out in Task 18.

fn main() {
    println!("anabios-headless — stub. See Task 18.");
}
```

- [ ] **Step 1.9: Verify the workspace compiles**

Run: `cargo check --workspace`

Expected: completes with zero errors. Warnings about unused modules are acceptable at this stage (they go away as later tasks fill the modules in).

- [ ] **Step 1.10: Verify formatting and lint pass**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`

Expected: zero output, zero diffs, zero warnings.

- [ ] **Step 1.11: Commit**

```bash
git add Cargo.toml rust-toolchain.toml rustfmt.toml crates/
git commit -m "chore: scaffold cargo workspace with anabios-core and anabios-headless crates"
```

---

## Task 2: Add a Vec2 math sanity test

**Goal:** Confirm `glam::Vec2` is wired in and demonstrate the inline-unit-test pattern other tasks will follow.

**Files:**
- Modify: `crates/anabios-core/src/prelude.rs`

- [ ] **Step 2.1: Add a failing test inside the prelude module**

Replace `crates/anabios-core/src/prelude.rs` with:

```rust
//! Internal prelude used across the crate.

pub(crate) use glam::Vec2;

/// Wrap a position into the bounded toroidal world `[0, size)` along each axis.
/// Inputs outside the range, including negative values, are normalized.
#[inline]
pub(crate) fn wrap_torus(pos: Vec2, size: Vec2) -> Vec2 {
    Vec2::new(pos.x.rem_euclid(size.x), pos.y.rem_euclid(size.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_torus_keeps_positive_in_range() {
        let wrapped = wrap_torus(Vec2::new(1024.5, -0.1), Vec2::splat(1024.0));
        assert!(wrapped.x >= 0.0 && wrapped.x < 1024.0);
        assert!(wrapped.y >= 0.0 && wrapped.y < 1024.0);
        assert!((wrapped.x - 0.5).abs() < 1e-5);
        assert!((wrapped.y - 1023.9).abs() < 1e-3);
    }
}
```

- [ ] **Step 2.2: Run the test to verify it passes**

Run: `cargo test -p anabios-core prelude`

Expected: 1 passed.

- [ ] **Step 2.3: Commit**

```bash
git add crates/anabios-core/src/prelude.rs
git commit -m "feat(core): add wrap_torus helper and prelude unit test pattern"
```

---

## Task 3: Deterministic RNG wrapper

**Goal:** Define a single `Rng` type the whole simulation routes through. No code outside `world.rs` is allowed to construct one — enforcing the single-RNG discipline at the API level.

**Files:**
- Modify: `crates/anabios-core/src/rng.rs`

- [ ] **Step 3.1: Write the failing tests for the RNG wrapper**

Replace `crates/anabios-core/src/rng.rs` with:

```rust
//! Deterministic RNG wrapper.
//!
//! The simulation uses a single Xoshiro256++ stream owned by `World`. Every
//! stochastic operation pulls from this stream in a fixed order. No code
//! reads `rand::thread_rng()` or `std::time` for randomness.

use rand::distributions::Standard;
use rand::prelude::Distribution;
use rand::{Rng as _, RngCore as _, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};

/// Deterministic RNG used throughout the simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rng {
    inner: Xoshiro256PlusPlus,
}

impl Rng {
    /// Construct from a 64-bit seed. Same seed → bit-identical stream.
    #[inline]
    pub fn from_seed(seed: u64) -> Self {
        Self { inner: Xoshiro256PlusPlus::seed_from_u64(seed) }
    }

    /// Uniform `f32` in `[0, 1)`.
    #[inline]
    pub fn f32_unit(&mut self) -> f32 {
        Standard.sample(&mut self.inner)
    }

    /// Uniform `f32` in `[low, high)`.
    #[inline]
    pub fn f32_range(&mut self, low: f32, high: f32) -> f32 {
        debug_assert!(low < high, "f32_range: low must be < high");
        self.inner.gen_range(low..high)
    }

    /// Gaussian sample with given mean and standard deviation, generated via
    /// the Box–Muller transform so it stays deterministic across platforms
    /// (the standard library has no fixed-output normal distribution).
    pub fn gaussian(&mut self, mean: f32, std_dev: f32) -> f32 {
        // Two uniforms in (0, 1] for Box–Muller.
        let u1 = (1.0 - self.f32_unit()).max(f32::MIN_POSITIVE);
        let u2 = self.f32_unit();
        let mag = (-2.0_f32 * u1.ln()).sqrt();
        let z0 = mag * (std::f32::consts::TAU * u2).cos();
        mean + std_dev * z0
    }

    /// Uniform `u32`.
    #[inline]
    pub fn u32(&mut self) -> u32 {
        self.inner.next_u32()
    }

    /// Uniform index `< n`.
    #[inline]
    pub fn index(&mut self, n: usize) -> usize {
        debug_assert!(n > 0, "index: n must be > 0");
        self.inner.gen_range(0..n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_yields_same_stream() {
        let mut a = Rng::from_seed(42);
        let mut b = Rng::from_seed(42);
        for _ in 0..1024 {
            assert_eq!(a.u32(), b.u32());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::from_seed(1);
        let mut b = Rng::from_seed(2);
        let first_a = a.u32();
        let first_b = b.u32();
        assert_ne!(first_a, first_b);
    }

    #[test]
    fn f32_unit_in_range() {
        let mut r = Rng::from_seed(7);
        for _ in 0..10_000 {
            let x = r.f32_unit();
            assert!(x >= 0.0 && x < 1.0);
        }
    }

    #[test]
    fn gaussian_has_reasonable_moments() {
        let mut r = Rng::from_seed(11);
        let n = 50_000;
        let samples: Vec<f32> = (0..n).map(|_| r.gaussian(0.0, 1.0)).collect();
        let mean = samples.iter().sum::<f32>() / n as f32;
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n as f32;
        assert!(mean.abs() < 0.05, "mean drifted: {mean}");
        assert!((var - 1.0).abs() < 0.05, "variance drifted: {var}");
    }

    #[test]
    fn snapshot_roundtrip_preserves_stream() {
        let mut a = Rng::from_seed(99);
        for _ in 0..17 {
            a.u32();
        }
        let bytes = bincode::serialize(&a).expect("serialize");
        let mut b: Rng = bincode::deserialize(&bytes).expect("deserialize");
        for _ in 0..1024 {
            assert_eq!(a.u32(), b.u32());
        }
    }
}
```

- [ ] **Step 3.2: Run the RNG tests to verify they pass**

Run: `cargo test -p anabios-core rng`

Expected: 5 passed.

- [ ] **Step 3.3: Run fmt and clippy**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`

Expected: no diffs, no warnings.

- [ ] **Step 3.4: Commit**

```bash
git add crates/anabios-core/src/rng.rs
git commit -m "feat(core): deterministic Rng wrapper around Xoshiro256++"
```

---

## Task 4: Genome type and trait slots

**Goal:** A 50-float genome with named trait slots, value clamping, deterministic random construction, and Gaussian mutation. M1 only uses a handful of slots actively; the rest are present and inert.

**Files:**
- Modify: `crates/anabios-core/src/genome.rs`

- [ ] **Step 4.1: Implement the Genome type and slot enum**

Replace `crates/anabios-core/src/genome.rs` with:

```rust
//! 50-float genome with named trait slots.
//!
//! Every value is clamped to `[0, 1]`. Slot meanings are hardcoded; values
//! mutate. Only a handful of slots drive behavior in M1 (see `behavior.rs`);
//! the rest are present and inert, awaiting later milestones.

use serde::{Deserialize, Serialize};

use crate::rng::Rng;

/// Number of trait slots in the genome.
pub const GENOME_LEN: usize = 50;

/// Per-trait Gaussian mutation sigma when `mutation_rate` is at maximum.
///
/// Effective sigma per mutation = `MUTATION_SIGMA_MAX * genome[mutation_rate]`.
pub const MUTATION_SIGMA_MAX: f32 = 0.08;

/// Named slot indices into the 50-float genome.
///
/// Slot meanings are stable. New slots are appended; existing indices never
/// shift (so saved genomes stay readable across versions).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenomeSlot {
    // Body modifiers (0..10)
    Size = 0,
    ColorHue = 1,
    ColorSat = 2,
    ColorVal = 3,
    LifespanBias = 4,
    BasalMetabolism = 5,
    MutationRate = 6,
    ImmuneStrength = 7,
    _BodyReserved8 = 8,
    _BodyReserved9 = 9,

    // Drive levels (10..20)
    Aggression = 10,
    Fearfulness = 11,
    Curiosity = 12,
    SocialAffinity = 13,
    KinPreference = 14,
    Territoriality = 15,
    _DriveReserved16 = 16,
    _DriveReserved17 = 17,
    _DriveReserved18 = 18,
    _DriveReserved19 = 19,

    // Behavioral biases (20..30)
    ExploreVsExploit = 20,
    RiskTolerance = 21,
    AmbushPreference = 22,
    CommunicationStrength = 23,
    Altruism = 24,
    SpeedMax = 25,
    PerceptionRadius = 26,
    DietCarnivory = 27,
    _BehaviorReserved28 = 28,
    _BehaviorReserved29 = 29,

    // Reproductive (30..40)
    ReproductionThreshold = 30,
    OffspringInvestment = 31,
    MateChoosiness = 32,
    SexualDimorphism = 33,
    _ReproReserved34 = 34,
    _ReproReserved35 = 35,
    _ReproReserved36 = 36,
    _ReproReserved37 = 37,
    _ReproReserved38 = 38,
    _ReproReserved39 = 39,

    // Sensory weighting (40..50)
    _SensoryReserved40 = 40,
    _SensoryReserved41 = 41,
    _SensoryReserved42 = 42,
    _SensoryReserved43 = 43,
    _SensoryReserved44 = 44,
    _SensoryReserved45 = 45,
    _SensoryReserved46 = 46,
    _SensoryReserved47 = 47,
    _SensoryReserved48 = 48,
    _SensoryReserved49 = 49,
}

impl GenomeSlot {
    #[inline]
    pub const fn idx(self) -> usize {
        self as usize
    }
}

/// Fixed-size 50-float genome.
///
/// All values are kept in `[0, 1]`; constructors and mutation respect this.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Genome(pub [f32; GENOME_LEN]);

impl Genome {
    /// Construct a genome filled with `0.5` (a neutral baseline used by
    /// scenario seed templates).
    #[inline]
    pub fn neutral() -> Self {
        Self([0.5; GENOME_LEN])
    }

    /// Construct a uniformly random genome.
    pub fn random(rng: &mut Rng) -> Self {
        let mut g = [0.0_f32; GENOME_LEN];
        for slot in g.iter_mut() {
            *slot = rng.f32_unit();
        }
        Self(g)
    }

    /// Read a slot by name.
    #[inline]
    pub fn get(&self, slot: GenomeSlot) -> f32 {
        self.0[slot.idx()]
    }

    /// Write a slot by name. The value is clamped into `[0, 1]`.
    #[inline]
    pub fn set(&mut self, slot: GenomeSlot, value: f32) {
        self.0[slot.idx()] = value.clamp(0.0, 1.0);
    }

    /// L2 distance between two genomes. Used by speciation in M2; kept here
    /// because it is conceptually part of the genome's contract.
    pub fn distance(&self, other: &Genome) -> f32 {
        let mut acc = 0.0_f32;
        for i in 0..GENOME_LEN {
            let d = self.0[i] - other.0[i];
            acc += d * d;
        }
        acc.sqrt()
    }

    /// Apply per-slot Gaussian mutation in place. Sigma scales with the
    /// genome's own `MutationRate` slot. Values are clamped back into
    /// `[0, 1]` after perturbation.
    pub fn mutate_in_place(&mut self, rng: &mut Rng) {
        let sigma = MUTATION_SIGMA_MAX * self.get(GenomeSlot::MutationRate);
        if sigma <= 0.0 {
            return;
        }
        for i in 0..GENOME_LEN {
            let delta = rng.gaussian(0.0, sigma);
            self.0[i] = (self.0[i] + delta).clamp(0.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_genome_is_all_half() {
        let g = Genome::neutral();
        for v in g.0.iter() {
            assert_eq!(*v, 0.5);
        }
    }

    #[test]
    fn random_genome_is_in_unit_range() {
        let mut rng = Rng::from_seed(1);
        let g = Genome::random(&mut rng);
        for v in g.0.iter() {
            assert!(*v >= 0.0 && *v < 1.0);
        }
    }

    #[test]
    fn random_genome_is_deterministic() {
        let mut a = Rng::from_seed(123);
        let mut b = Rng::from_seed(123);
        let ga = Genome::random(&mut a);
        let gb = Genome::random(&mut b);
        assert_eq!(ga, gb);
    }

    #[test]
    fn get_and_set_use_named_slots() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 0.9);
        g.set(GenomeSlot::PerceptionRadius, 0.3);
        assert!((g.get(GenomeSlot::SpeedMax) - 0.9).abs() < 1e-6);
        assert!((g.get(GenomeSlot::PerceptionRadius) - 0.3).abs() < 1e-6);
        assert_eq!(g.get(GenomeSlot::Size), 0.5);
    }

    #[test]
    fn set_clamps_out_of_range_values() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Aggression, -1.0);
        g.set(GenomeSlot::Curiosity, 2.0);
        assert_eq!(g.get(GenomeSlot::Aggression), 0.0);
        assert_eq!(g.get(GenomeSlot::Curiosity), 1.0);
    }

    #[test]
    fn distance_is_zero_for_identical_genomes() {
        let g = Genome::neutral();
        assert_eq!(g.distance(&g), 0.0);
    }

    #[test]
    fn distance_is_symmetric() {
        let mut a = Genome::neutral();
        let mut b = Genome::neutral();
        a.set(GenomeSlot::SpeedMax, 0.9);
        b.set(GenomeSlot::SpeedMax, 0.1);
        assert!((a.distance(&b) - b.distance(&a)).abs() < 1e-6);
    }

    #[test]
    fn mutate_keeps_values_in_range_and_respects_zero_rate() {
        let mut rng = Rng::from_seed(7);
        let mut g = Genome::neutral();
        g.set(GenomeSlot::MutationRate, 0.0);
        let before = g.0;
        g.mutate_in_place(&mut rng);
        assert_eq!(before, g.0, "mutation with rate 0 must be a no-op");

        g.set(GenomeSlot::MutationRate, 1.0);
        for _ in 0..1000 {
            g.mutate_in_place(&mut rng);
            for v in g.0.iter() {
                assert!(*v >= 0.0 && *v <= 1.0);
            }
        }
    }
}
```

- [ ] **Step 4.2: Run the genome tests**

Run: `cargo test -p anabios-core genome`

Expected: 8 passed.

- [ ] **Step 4.3: Run fmt and clippy**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`

Expected: no diffs, no warnings.

- [ ] **Step 4.4: Commit**

```bash
git add crates/anabios-core/src/genome.rs
git commit -m "feat(core): 50-float genome with named slots, distance, mutation"
```

---

## Task 5: Biome field data structures

**Goal:** Create the 128×128 biome grid with terrain types, plant biomass, and a deterministic generator that produces continents and lakes.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs`

- [ ] **Step 5.1: Implement BiomeField with terrain generation**

Replace `crates/anabios-core/src/biome.rs` with:

```rust
//! 128×128 biome field with terrain types and plant biomass.
//!
//! The terrain is generated deterministically from a seed using a simple
//! value-noise field with two octaves. Plant biomass starts at the cell's
//! carrying capacity (a function of terrain type) and is replenished each
//! tick by logistic regrowth (see Task 6).

use serde::{Deserialize, Serialize};

use crate::prelude::Vec2;
use crate::rng::Rng;

/// Grid resolution per axis. Total cells = `BIOME_RES * BIOME_RES`.
pub const BIOME_RES: usize = 128;
/// World extent per axis. The biome covers `[0, WORLD_SIZE) × [0, WORLD_SIZE)`.
pub const WORLD_SIZE: f32 = 1024.0;
/// Side length of one biome cell, in world units.
pub const CELL_SIZE: f32 = WORLD_SIZE / BIOME_RES as f32;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainType {
    Water = 0,
    Grass = 1,
    Forest = 2,
    Desert = 3,
    Rock = 4,
}

impl TerrainType {
    /// Maximum plant biomass (per cell, in arbitrary energy units) a cell of
    /// this terrain type can support. Water and Rock support no plants.
    pub const fn carrying_capacity(self) -> f32 {
        match self {
            TerrainType::Water => 0.0,
            TerrainType::Grass => 10.0,
            TerrainType::Forest => 20.0,
            TerrainType::Desert => 3.0,
            TerrainType::Rock => 0.0,
        }
    }

    /// Logistic regrowth rate (fraction of carrying capacity per tick).
    pub const fn regrowth_rate(self) -> f32 {
        match self {
            TerrainType::Water => 0.0,
            TerrainType::Grass => 0.01,
            TerrainType::Forest => 0.003,
            TerrainType::Desert => 0.002,
            TerrainType::Rock => 0.0,
        }
    }
}

/// One cell of the biome grid.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BiomeCell {
    pub terrain: TerrainType,
    pub plant_biomass: f32,
}

/// 128×128 biome field. Indexed `[row * BIOME_RES + col]` with `row` = y,
/// `col` = x. World position `(x, y)` maps to `(col, row) = (x/CELL_SIZE,
/// y/CELL_SIZE)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiomeField {
    pub cells: Vec<BiomeCell>,
}

impl BiomeField {
    /// Generate a biome field deterministically from a seed.
    pub fn generate(seed: u64) -> Self {
        let mut rng = Rng::from_seed(seed);
        // Hash-based value-noise corner grid, sampled at two octaves.
        let coarse = NoiseGrid::sample(&mut rng, 8);
        let fine = NoiseGrid::sample(&mut rng, 24);

        let mut cells = Vec::with_capacity(BIOME_RES * BIOME_RES);
        for row in 0..BIOME_RES {
            for col in 0..BIOME_RES {
                let u = col as f32 / BIOME_RES as f32;
                let v = row as f32 / BIOME_RES as f32;
                let n = 0.65 * coarse.sample(u, v) + 0.35 * fine.sample(u, v);
                let terrain = elevation_to_terrain(n);
                cells.push(BiomeCell {
                    terrain,
                    plant_biomass: terrain.carrying_capacity(),
                });
            }
        }
        Self { cells }
    }

    /// Convert a world position into a `(col, row)` cell index. Out-of-range
    /// positions are wrapped into the torus.
    #[inline]
    pub fn cell_coords(pos: Vec2) -> (usize, usize) {
        let wrapped_x = pos.x.rem_euclid(WORLD_SIZE);
        let wrapped_y = pos.y.rem_euclid(WORLD_SIZE);
        let col = (wrapped_x / CELL_SIZE) as usize;
        let row = (wrapped_y / CELL_SIZE) as usize;
        (col.min(BIOME_RES - 1), row.min(BIOME_RES - 1))
    }

    #[inline]
    pub fn cell_index(col: usize, row: usize) -> usize {
        row * BIOME_RES + col
    }

    #[inline]
    pub fn at(&self, col: usize, row: usize) -> &BiomeCell {
        &self.cells[Self::cell_index(col, row)]
    }

    #[inline]
    pub fn at_mut(&mut self, col: usize, row: usize) -> &mut BiomeCell {
        &mut self.cells[Self::cell_index(col, row)]
    }

    /// Sample the biome at a world position.
    pub fn sample(&self, pos: Vec2) -> &BiomeCell {
        let (col, row) = Self::cell_coords(pos);
        self.at(col, row)
    }
}

fn elevation_to_terrain(n: f32) -> TerrainType {
    if n < 0.30 {
        TerrainType::Water
    } else if n < 0.45 {
        TerrainType::Desert
    } else if n < 0.65 {
        TerrainType::Grass
    } else if n < 0.85 {
        TerrainType::Forest
    } else {
        TerrainType::Rock
    }
}

/// A grid of corner samples used for value noise. `cells_per_axis` controls
/// the frequency; higher = finer detail.
struct NoiseGrid {
    cells_per_axis: usize,
    samples: Vec<f32>,
}

impl NoiseGrid {
    fn sample(rng: &mut Rng, cells_per_axis: usize) -> Self {
        let n = (cells_per_axis + 1) * (cells_per_axis + 1);
        let mut samples = Vec::with_capacity(n);
        for _ in 0..n {
            samples.push(rng.f32_unit());
        }
        Self { cells_per_axis, samples }
    }

    fn corner(&self, cx: usize, cy: usize) -> f32 {
        let stride = self.cells_per_axis + 1;
        self.samples[cy * stride + cx]
    }

    /// Sample at `(u, v)` in `[0, 1)²` using bilinear interpolation.
    fn sample(&self, u: f32, v: f32) -> f32 {
        let scaled_x = u * self.cells_per_axis as f32;
        let scaled_y = v * self.cells_per_axis as f32;
        let cx = scaled_x.floor() as usize;
        let cy = scaled_y.floor() as usize;
        let fx = scaled_x - cx as f32;
        let fy = scaled_y - cy as f32;
        let cx2 = (cx + 1).min(self.cells_per_axis);
        let cy2 = (cy + 1).min(self.cells_per_axis);
        let a = self.corner(cx, cy);
        let b = self.corner(cx2, cy);
        let c = self.corner(cx, cy2);
        let d = self.corner(cx2, cy2);
        let ab = a + (b - a) * fx;
        let cd = c + (d - c) * fx;
        ab + (cd - ab) * fy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biome_is_deterministic() {
        let a = BiomeField::generate(42);
        let b = BiomeField::generate(42);
        for i in 0..a.cells.len() {
            assert_eq!(a.cells[i].terrain, b.cells[i].terrain);
            assert!((a.cells[i].plant_biomass - b.cells[i].plant_biomass).abs() < 1e-6);
        }
    }

    #[test]
    fn biome_contains_multiple_terrain_types() {
        let b = BiomeField::generate(7);
        let mut seen = [0_usize; 5];
        for cell in &b.cells {
            seen[cell.terrain as usize] += 1;
        }
        let nonzero: usize = seen.iter().filter(|&&c| c > 0).count();
        assert!(nonzero >= 3, "biome should contain at least 3 terrain types, saw {:?}", seen);
    }

    #[test]
    fn cell_coords_wraps_negative_and_oversize_positions() {
        let (cx, cy) = BiomeField::cell_coords(Vec2::new(-1.0, WORLD_SIZE + 5.0));
        assert!(cx < BIOME_RES);
        assert!(cy < BIOME_RES);
    }

    #[test]
    fn carrying_capacity_is_initial_biomass() {
        let b = BiomeField::generate(99);
        for cell in &b.cells {
            assert!((cell.plant_biomass - cell.terrain.carrying_capacity()).abs() < 1e-6);
        }
    }
}
```

- [ ] **Step 5.2: Run the biome tests**

Run: `cargo test -p anabios-core biome`

Expected: 4 passed.

- [ ] **Step 5.3: Commit**

```bash
git add crates/anabios-core/src/biome.rs
git commit -m "feat(core): BiomeField with deterministic terrain generation"
```

---

## Task 6: Plant regrowth step

**Goal:** A logistic regrowth function the tick orchestrator can call once per N ticks.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs`

- [ ] **Step 6.1: Add the regrowth method with tests**

Append to `crates/anabios-core/src/biome.rs` (replace the existing `#[cfg(test)]` block at the bottom — the new tests merge with the existing ones):

Find this section near the bottom:

```rust
impl BiomeField {
```

and add the following method inside the impl block, just before its closing `}`:

```rust
    /// Apply logistic regrowth: `b += r * b * (1 - b / K)` clamped to `[0, K]`.
    /// Empty cells stay empty (no spontaneous regeneration) — recolonization
    /// requires neighbour cells with biomass and is added in M3.
    pub fn regrow_step(&mut self) {
        for cell in self.cells.iter_mut() {
            let capacity = cell.terrain.carrying_capacity();
            if capacity <= 0.0 || cell.plant_biomass <= 0.0 {
                continue;
            }
            let r = cell.terrain.regrowth_rate();
            let b = cell.plant_biomass;
            let next = b + r * b * (1.0 - b / capacity);
            cell.plant_biomass = next.clamp(0.0, capacity);
        }
    }

    /// Consume up to `desired` biomass from the cell containing `pos`,
    /// returning how much was actually consumed. The biome's biomass is
    /// reduced by the same amount.
    pub fn graze(&mut self, pos: Vec2, desired: f32) -> f32 {
        if desired <= 0.0 {
            return 0.0;
        }
        let (col, row) = Self::cell_coords(pos);
        let cell = self.at_mut(col, row);
        let taken = desired.min(cell.plant_biomass);
        cell.plant_biomass -= taken;
        taken
    }
```

And add these tests inside the existing `mod tests` block (just before its closing `}`):

```rust
    #[test]
    fn regrow_increases_partial_biomass_toward_capacity() {
        let mut b = BiomeField::generate(13);
        // Drain every grass cell to 1.0 biomass.
        for cell in b.cells.iter_mut() {
            if cell.terrain == TerrainType::Grass {
                cell.plant_biomass = 1.0;
            }
        }
        let before_total: f32 = b
            .cells
            .iter()
            .filter(|c| c.terrain == TerrainType::Grass)
            .map(|c| c.plant_biomass)
            .sum();
        for _ in 0..50 {
            b.regrow_step();
        }
        let after_total: f32 = b
            .cells
            .iter()
            .filter(|c| c.terrain == TerrainType::Grass)
            .map(|c| c.plant_biomass)
            .sum();
        assert!(after_total > before_total, "biomass should grow: {before_total} -> {after_total}");
    }

    #[test]
    fn regrow_does_not_exceed_carrying_capacity() {
        let mut b = BiomeField::generate(13);
        for _ in 0..1000 {
            b.regrow_step();
        }
        for cell in &b.cells {
            let cap = cell.terrain.carrying_capacity();
            assert!(cell.plant_biomass <= cap + 1e-4, "biomass {} > cap {}", cell.plant_biomass, cap);
        }
    }

    #[test]
    fn regrow_leaves_dead_cells_dead() {
        let mut b = BiomeField::generate(13);
        for cell in b.cells.iter_mut() {
            if cell.terrain == TerrainType::Grass {
                cell.plant_biomass = 0.0;
            }
        }
        for _ in 0..100 {
            b.regrow_step();
        }
        for cell in &b.cells {
            if cell.terrain == TerrainType::Grass {
                assert_eq!(cell.plant_biomass, 0.0);
            }
        }
    }

    #[test]
    fn graze_reduces_biomass_and_returns_taken_amount() {
        let mut b = BiomeField::generate(31);
        // Find a grass cell so we know biomass > 0.
        let mut target = Vec2::ZERO;
        'outer: for row in 0..BIOME_RES {
            for col in 0..BIOME_RES {
                if b.at(col, row).terrain == TerrainType::Grass {
                    target = Vec2::new((col as f32 + 0.5) * CELL_SIZE, (row as f32 + 0.5) * CELL_SIZE);
                    break 'outer;
                }
            }
        }
        let before = b.sample(target).plant_biomass;
        assert!(before > 0.0, "expected biomass at grass cell");
        let taken = b.graze(target, 2.0);
        assert!(taken > 0.0 && taken <= 2.0);
        let after = b.sample(target).plant_biomass;
        assert!((before - after - taken).abs() < 1e-5);
    }
```

- [ ] **Step 6.2: Run the biome tests**

Run: `cargo test -p anabios-core biome`

Expected: 8 passed.

- [ ] **Step 6.3: Commit**

```bash
git add crates/anabios-core/src/biome.rs
git commit -m "feat(core): logistic plant regrowth and grazing on biome field"
```

---

## Task 7: Uniform spatial hash

**Goal:** An `O(1)`-query neighbor index over agent positions. Rebuilt every tick before sensing.

**Files:**
- Modify: `crates/anabios-core/src/spatial.rs`

- [ ] **Step 7.1: Implement UniformSpatialHash with tests**

Replace `crates/anabios-core/src/spatial.rs` with:

```rust
//! Uniform-grid spatial hash for fast neighbor queries.
//!
//! World is a torus of size `WORLD_SIZE`. The hash divides it into `RES × RES`
//! cells. To query a position within `radius`, the caller asks for all agents
//! in the cells that the radius's bounding box touches, then filters by exact
//! distance. Cell size is chosen so that `radius ≤ cell_size`; one ring of
//! neighbour cells is always sufficient.

use crate::biome::WORLD_SIZE;
use crate::prelude::Vec2;

/// Number of cells per axis. 64 gives `cell_size = 16` world units, which
/// safely covers the maximum possible perception radius
/// (`PERCEPTION_MAX_RADIUS = 12` in `behavior.rs`).
pub const HASH_RES: usize = 64;
pub const HASH_CELL_SIZE: f32 = WORLD_SIZE / HASH_RES as f32;

/// Hard upper bound on perception radius — must hold for the
/// "one-ring-of-neighbours is sufficient" guarantee.
pub const PERCEPTION_MAX_RADIUS: f32 = HASH_CELL_SIZE;

#[derive(Debug, Clone)]
pub struct UniformSpatialHash {
    /// For each cell index, the slice of `flat` that contains its agent ids.
    bucket_offsets: Vec<u32>,
    bucket_lens: Vec<u32>,
    flat: Vec<u32>,
}

impl UniformSpatialHash {
    pub fn new() -> Self {
        let total_cells = HASH_RES * HASH_RES;
        Self {
            bucket_offsets: vec![0; total_cells],
            bucket_lens: vec![0; total_cells],
            flat: Vec::new(),
        }
    }

    /// Rebuild from the alive agent positions. Agents whose `alive` bit is
    /// false are skipped. `positions[i]` and `alive_iter` are indexed by
    /// agent id.
    pub fn rebuild<'a>(
        &mut self,
        positions: &[Vec2],
        alive: impl Fn(usize) -> bool,
    ) {
        let total_cells = HASH_RES * HASH_RES;
        // Phase 1: count agents per cell.
        let mut counts = vec![0_u32; total_cells];
        for (i, pos) in positions.iter().enumerate() {
            if !alive(i) {
                continue;
            }
            let cell = Self::cell_of(*pos);
            counts[cell] += 1;
        }

        // Phase 2: prefix-sum to compute offsets.
        let mut total = 0_u32;
        for i in 0..total_cells {
            self.bucket_offsets[i] = total;
            total += counts[i];
            self.bucket_lens[i] = 0;
        }
        self.flat.clear();
        self.flat.resize(total as usize, 0);

        // Phase 3: scatter into flat buffer.
        for (i, pos) in positions.iter().enumerate() {
            if !alive(i) {
                continue;
            }
            let cell = Self::cell_of(*pos);
            let off = self.bucket_offsets[cell] + self.bucket_lens[cell];
            self.flat[off as usize] = i as u32;
            self.bucket_lens[cell] += 1;
        }
    }

    /// Visit every agent in the wrap-aware bounding box of a position +
    /// radius. The caller is responsible for the exact distance check.
    ///
    /// `radius` must not exceed `PERCEPTION_MAX_RADIUS`; debug builds assert.
    pub fn query<F: FnMut(u32)>(&self, pos: Vec2, radius: f32, mut f: F) {
        debug_assert!(
            radius <= PERCEPTION_MAX_RADIUS + 1e-3,
            "query radius {radius} exceeds PERCEPTION_MAX_RADIUS={PERCEPTION_MAX_RADIUS}"
        );
        let (cx, cy) = Self::cell_coords(pos);
        // One-cell ring; positions wrap around the torus.
        for dy in [HASH_RES - 1, 0, 1] {
            let row = (cy + dy) % HASH_RES;
            for dx in [HASH_RES - 1, 0, 1] {
                let col = (cx + dx) % HASH_RES;
                let cell = row * HASH_RES + col;
                let off = self.bucket_offsets[cell] as usize;
                let len = self.bucket_lens[cell] as usize;
                for id in &self.flat[off..off + len] {
                    f(*id);
                }
            }
        }
    }

    #[inline]
    fn cell_coords(pos: Vec2) -> (usize, usize) {
        let x = pos.x.rem_euclid(WORLD_SIZE);
        let y = pos.y.rem_euclid(WORLD_SIZE);
        let col = ((x / HASH_CELL_SIZE) as usize).min(HASH_RES - 1);
        let row = ((y / HASH_CELL_SIZE) as usize).min(HASH_RES - 1);
        (col, row)
    }

    #[inline]
    fn cell_of(pos: Vec2) -> usize {
        let (col, row) = Self::cell_coords(pos);
        row * HASH_RES + col
    }
}

impl Default for UniformSpatialHash {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrap-aware distance between two points on the torus.
#[inline]
pub fn torus_distance(a: Vec2, b: Vec2) -> f32 {
    let mut dx = (a.x - b.x).abs();
    let mut dy = (a.y - b.y).abs();
    if dx > WORLD_SIZE * 0.5 {
        dx = WORLD_SIZE - dx;
    }
    if dy > WORLD_SIZE * 0.5 {
        dy = WORLD_SIZE - dy;
    }
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute_force_neighbors(positions: &[Vec2], origin: Vec2, radius: f32) -> Vec<u32> {
        let mut out: Vec<u32> = (0..positions.len() as u32)
            .filter(|i| torus_distance(positions[*i as usize], origin) <= radius)
            .collect();
        out.sort();
        out
    }

    #[test]
    fn empty_hash_returns_no_results() {
        let h = UniformSpatialHash::new();
        let mut found = Vec::new();
        h.query(Vec2::new(100.0, 100.0), 8.0, |id| found.push(id));
        assert!(found.is_empty());
    }

    #[test]
    fn query_matches_brute_force_random_positions() {
        let positions: Vec<Vec2> = (0..500)
            .map(|i| {
                let x = ((i * 17) % 1024) as f32 + 0.5;
                let y = ((i * 31) % 1024) as f32 + 0.5;
                Vec2::new(x, y)
            })
            .collect();
        let mut h = UniformSpatialHash::new();
        h.rebuild(&positions, |_| true);

        let probes = [
            Vec2::new(10.0, 10.0),
            Vec2::new(513.0, 513.0),
            Vec2::new(1023.0, 0.5),
            Vec2::new(0.5, 1023.0),
        ];
        for probe in probes {
            let mut got: Vec<u32> = Vec::new();
            h.query(probe, PERCEPTION_MAX_RADIUS, |id| {
                if torus_distance(positions[id as usize], probe) <= PERCEPTION_MAX_RADIUS {
                    got.push(id);
                }
            });
            got.sort();
            got.dedup();
            let expected = brute_force_neighbors(&positions, probe, PERCEPTION_MAX_RADIUS);
            assert_eq!(got, expected, "probe {:?}", probe);
        }
    }

    #[test]
    fn alive_mask_skips_dead_agents() {
        let positions = vec![Vec2::new(100.0, 100.0); 4];
        let mut h = UniformSpatialHash::new();
        h.rebuild(&positions, |i| i != 2);
        let mut found: Vec<u32> = Vec::new();
        h.query(Vec2::new(100.0, 100.0), 4.0, |id| found.push(id));
        found.sort();
        assert_eq!(found, vec![0, 1, 3]);
    }

    #[test]
    fn torus_distance_wraps_short_way() {
        let a = Vec2::new(2.0, 0.0);
        let b = Vec2::new(WORLD_SIZE - 2.0, 0.0);
        assert!((torus_distance(a, b) - 4.0).abs() < 1e-3);
    }
}
```

- [ ] **Step 7.2: Run the spatial tests**

Run: `cargo test -p anabios-core spatial`

Expected: 4 passed.

- [ ] **Step 7.3: Commit**

```bash
git add crates/anabios-core/src/spatial.rs
git commit -m "feat(core): uniform spatial hash with torus-wrapped queries"
```

---

## Task 8: Agent Struct-of-Arrays buffers

**Goal:** The `AgentBuffers` type that owns all per-agent parallel `Vec`s plus a free-list for id recycling.

**Files:**
- Modify: `crates/anabios-core/src/agent.rs`

- [ ] **Step 8.1: Implement AgentBuffers**

Replace `crates/anabios-core/src/agent.rs` with:

```rust
//! Struct-of-Arrays agent buffers.
//!
//! Each per-agent field is its own `Vec<T>` indexed by `AgentId`. Dead agent
//! slots stay allocated for index stability; `alive` is a bitvec used to mask
//! reads. Newly spawned agents reuse dead slots via a free list, so live
//! agent counts stay dense.

use bitvec::vec::BitVec;
use serde::{Deserialize, Serialize};

use crate::genome::Genome;
use crate::prelude::Vec2;

/// Stable agent identifier. `u32::MAX` is reserved as a null sentinel.
pub type AgentId = u32;
pub const AGENT_NULL: AgentId = u32::MAX;

/// Maximum starting energy for newly-spawned agents.
pub const SPAWN_ENERGY: f32 = 50.0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentBuffers {
    pub position: Vec<Vec2>,
    pub velocity: Vec<Vec2>,
    pub energy: Vec<f32>,
    pub age: Vec<u32>,
    pub genome: Vec<Genome>,
    pub alive: BitVec,
    free_list: Vec<AgentId>,
    live_count: u32,
}

impl AgentBuffers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of currently alive agents.
    #[inline]
    pub fn live_count(&self) -> u32 {
        self.live_count
    }

    /// Total slot capacity (alive + dead). Use only for sizing scratch
    /// buffers — iterate via `iter_alive()` instead of raw indices.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.position.len()
    }

    /// `true` iff the slot is currently alive.
    #[inline]
    pub fn is_alive(&self, id: AgentId) -> bool {
        let i = id as usize;
        i < self.alive.len() && self.alive[i]
    }

    /// Spawn an agent at the given position with the given genome. Reuses a
    /// dead slot if available, otherwise extends every buffer by one.
    pub fn spawn(&mut self, position: Vec2, genome: Genome) -> AgentId {
        let id = if let Some(id) = self.free_list.pop() {
            let i = id as usize;
            self.position[i] = position;
            self.velocity[i] = Vec2::ZERO;
            self.energy[i] = SPAWN_ENERGY;
            self.age[i] = 0;
            self.genome[i] = genome;
            self.alive.set(i, true);
            id
        } else {
            let i = self.position.len();
            self.position.push(position);
            self.velocity.push(Vec2::ZERO);
            self.energy.push(SPAWN_ENERGY);
            self.age.push(0);
            self.genome.push(genome);
            self.alive.push(true);
            i as AgentId
        };
        self.live_count += 1;
        id
    }

    /// Kill the agent. Energy is zeroed and the slot is added to the free list.
    pub fn kill(&mut self, id: AgentId) {
        let i = id as usize;
        if i >= self.alive.len() || !self.alive[i] {
            return;
        }
        self.alive.set(i, false);
        self.energy[i] = 0.0;
        self.free_list.push(id);
        self.live_count -= 1;
    }

    /// Iterate live agent ids. Order is by raw index (ascending), which is
    /// deterministic.
    pub fn iter_alive(&self) -> impl Iterator<Item = AgentId> + '_ {
        self.alive.iter_ones().map(|i| i as AgentId)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neutral() -> Genome {
        Genome::neutral()
    }

    #[test]
    fn spawn_increases_capacity_and_live_count() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(Vec2::new(1.0, 2.0), neutral());
        let id1 = a.spawn(Vec2::new(3.0, 4.0), neutral());
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
        let id = a.spawn(Vec2::ZERO, neutral());
        a.kill(id);
        assert!(!a.is_alive(id));
        assert_eq!(a.live_count(), 0);
    }

    #[test]
    fn spawn_after_kill_reuses_slot() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(Vec2::ZERO, neutral());
        let id1 = a.spawn(Vec2::ZERO, neutral());
        a.kill(id0);
        let id2 = a.spawn(Vec2::new(5.0, 6.0), neutral());
        assert_eq!(id2, id0, "slot 0 should have been reused");
        assert_eq!(a.live_count(), 2);
        assert!(a.is_alive(id1));
        assert!(a.is_alive(id2));
    }

    #[test]
    fn iter_alive_skips_dead_slots() {
        let mut a = AgentBuffers::new();
        let id0 = a.spawn(Vec2::ZERO, neutral());
        let _id1 = a.spawn(Vec2::ZERO, neutral());
        let id2 = a.spawn(Vec2::ZERO, neutral());
        a.kill(id0);
        let alive: Vec<AgentId> = a.iter_alive().collect();
        assert_eq!(alive, vec![1, id2]);
    }

    #[test]
    fn double_kill_is_a_noop() {
        let mut a = AgentBuffers::new();
        let id = a.spawn(Vec2::ZERO, neutral());
        a.kill(id);
        a.kill(id);
        assert_eq!(a.live_count(), 0);
        assert_eq!(a.iter_alive().count(), 0);
    }
}
```

- [ ] **Step 8.2: Run the agent tests**

Run: `cargo test -p anabios-core agent`

Expected: 5 passed.

- [ ] **Step 8.3: Commit**

```bash
git add crates/anabios-core/src/agent.rs
git commit -m "feat(core): Struct-of-Arrays agent buffers with spawn/kill and free list"
```

---

## Task 9: World root struct

**Goal:** Tie agents, biome, spatial hash, RNG, and tick counter into one owning struct. Construction takes a seed; all randomness goes through `world.rng`.

**Files:**
- Modify: `crates/anabios-core/src/world.rs`

- [ ] **Step 9.1: Implement the World struct**

Replace `crates/anabios-core/src/world.rs` with:

```rust
//! `World` is the root state object owned by every simulation. It carries
//! the RNG, biome field, agent buffers, spatial hash, and tick counter.
//! Nothing outside this struct holds simulation state.

use serde::{Deserialize, Serialize};

use crate::agent::{AgentBuffers, AgentId, SPAWN_ENERGY};
use crate::biome::{BiomeField, WORLD_SIZE};
use crate::genome::Genome;
use crate::prelude::Vec2;
use crate::rng::Rng;
use crate::spatial::UniformSpatialHash;

/// World root struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub tick: u64,
    pub seed: u64,
    pub rng: Rng,
    pub biome: BiomeField,
    pub agents: AgentBuffers,
    #[serde(skip)]
    pub spatial: UniformSpatialHash,
}

impl World {
    /// Build a world from a seed: deterministic biome + empty agent
    /// population + fresh spatial hash + tick 0.
    pub fn new(seed: u64) -> Self {
        Self {
            tick: 0,
            seed,
            rng: Rng::from_seed(seed),
            biome: BiomeField::generate(seed),
            agents: AgentBuffers::new(),
            spatial: UniformSpatialHash::new(),
        }
    }

    /// Convenience: spawn an agent with starting energy at the given position.
    pub fn spawn_agent(&mut self, position: Vec2, genome: Genome) -> AgentId {
        self.agents.spawn(position, genome)
    }

    /// World dimensions (for callers that want the constant without
    /// importing the biome module directly).
    #[inline]
    pub fn size(&self) -> f32 {
        WORLD_SIZE
    }

    /// Sanity helper used by tests and the headless CLI.
    pub fn alive_energy_total(&self) -> f32 {
        let mut total = 0.0;
        for id in self.agents.iter_alive() {
            total += self.agents.energy[id as usize];
        }
        total
    }

    /// Sum of plant biomass across the biome.
    pub fn plant_biomass_total(&self) -> f32 {
        self.biome.cells.iter().map(|c| c.plant_biomass).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_construction_is_deterministic() {
        let a = World::new(42);
        let b = World::new(42);
        assert_eq!(a.tick, b.tick);
        assert_eq!(a.seed, b.seed);
        for i in 0..a.biome.cells.len() {
            assert_eq!(a.biome.cells[i].terrain, b.biome.cells[i].terrain);
            assert!((a.biome.cells[i].plant_biomass - b.biome.cells[i].plant_biomass).abs() < 1e-6);
        }
    }

    #[test]
    fn spawn_agent_sets_initial_energy() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(10.0, 10.0), Genome::neutral());
        assert!(w.agents.is_alive(id));
        assert_eq!(w.agents.energy[id as usize], SPAWN_ENERGY);
    }
}
```

- [ ] **Step 9.2: Run the world tests**

Run: `cargo test -p anabios-core world`

Expected: 2 passed.

- [ ] **Step 9.3: Commit**

```bash
git add crates/anabios-core/src/world.rs
git commit -m "feat(core): World struct ties biome, agents, RNG, and spatial hash"
```

---

## Task 10: Sense stage

**Goal:** For each alive agent, sample biome + nearest plant cell + nearest neighbour from the spatial hash. Write into a per-agent `SensorRegister` slice. Pure functional over inputs.

**Files:**
- Create: `crates/anabios-core/src/sense.rs`
- Modify: `crates/anabios-core/src/lib.rs` (add the new module)

- [ ] **Step 10.1: Add the new module declaration**

Edit `crates/anabios-core/src/lib.rs`. Find the existing module declarations and add `pub mod sense;` so the list reads:

```rust
pub mod agent;
pub mod behavior;
pub mod biome;
pub mod genome;
pub mod rng;
pub mod scenario;
pub mod sense;
pub mod snapshot;
pub mod spatial;
pub mod tick;
pub mod world;
```

- [ ] **Step 10.2: Implement the sense module**

Create `crates/anabios-core/src/sense.rs`:

```rust
//! Per-agent sensor sampling.
//!
//! `sense()` reads world state and writes each alive agent's `SensorRegister`.
//! All values are deterministic functions of the world buffers and the
//! agent's position.

use serde::{Deserialize, Serialize};

use crate::agent::AgentBuffers;
use crate::biome::{BiomeCell, BiomeField, CELL_SIZE, WORLD_SIZE};
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::{wrap_torus, Vec2};
use crate::spatial::{torus_distance, UniformSpatialHash, PERCEPTION_MAX_RADIUS};

/// Per-agent sensor outputs computed each tick.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SensorRegister {
    /// Plant biomass in the agent's own cell.
    pub local_plant_biomass: f32,
    /// Direction (unit) to the highest-biomass cell within perception, or
    /// zero if no edible cell exists in range.
    pub plant_direction: Vec2,
    /// Distance to the nearest other alive agent on the torus, or `f32::INFINITY`.
    pub nearest_neighbor_dist: f32,
    /// Direction (unit) to that nearest neighbor.
    pub nearest_neighbor_dir: Vec2,
    /// Whether the agent currently has any alive neighbor in perception.
    pub has_neighbor: bool,
}

/// Effective perception radius for an agent given its genome.
#[inline]
pub fn perception_radius(genome: &Genome) -> f32 {
    // Perception radius scales between 25% and 100% of the engine cap.
    let frac = 0.25 + 0.75 * genome.get(GenomeSlot::PerceptionRadius);
    PERCEPTION_MAX_RADIUS * frac
}

/// Run the sense stage. `registers[i]` is populated for every alive agent;
/// dead slots are left unchanged. Caller owns `registers` and reuses it
/// across ticks to avoid per-tick allocation.
pub fn sense_all(
    agents: &AgentBuffers,
    biome: &BiomeField,
    spatial: &UniformSpatialHash,
    registers: &mut [SensorRegister],
) {
    debug_assert!(registers.len() >= agents.capacity());

    for id in agents.iter_alive() {
        let i = id as usize;
        let pos = agents.position[i];
        let genome = &agents.genome[i];
        let radius = perception_radius(genome);

        let local_cell = biome.sample(pos);
        let plant_direction = best_plant_direction(biome, pos, radius);

        let mut nearest_dist = f32::INFINITY;
        let mut nearest_dir = Vec2::ZERO;
        let mut has_neighbor = false;
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

        registers[i] = SensorRegister {
            local_plant_biomass: local_cell.plant_biomass,
            plant_direction,
            nearest_neighbor_dist: nearest_dist,
            nearest_neighbor_dir: nearest_dir,
            has_neighbor,
        };
    }
}

/// Find the direction toward the best-biomass biome cell within `radius`.
/// Returns `Vec2::ZERO` if no cell in range has positive biomass.
fn best_plant_direction(biome: &BiomeField, pos: Vec2, radius: f32) -> Vec2 {
    let mut best_biomass = 0.0_f32;
    let mut best_offset = Vec2::ZERO;
    let cell_reach = (radius / CELL_SIZE).ceil() as i32 + 1;
    let (cx, cy) = BiomeField::cell_coords(pos);

    for dy in -cell_reach..=cell_reach {
        for dx in -cell_reach..=cell_reach {
            let col = ((cx as i32 + dx).rem_euclid(crate::biome::BIOME_RES as i32)) as usize;
            let row = ((cy as i32 + dy).rem_euclid(crate::biome::BIOME_RES as i32)) as usize;
            let cell: &BiomeCell = biome.at(col, row);
            if cell.plant_biomass <= 0.0 {
                continue;
            }
            let cell_center = Vec2::new(
                (col as f32 + 0.5) * CELL_SIZE,
                (row as f32 + 0.5) * CELL_SIZE,
            );
            let offset = wrap_torus(cell_center - pos + Vec2::splat(WORLD_SIZE * 0.5), Vec2::splat(WORLD_SIZE))
                - Vec2::splat(WORLD_SIZE * 0.5);
            let dist = offset.length();
            if dist > radius {
                continue;
            }
            if cell.plant_biomass > best_biomass {
                best_biomass = cell.plant_biomass;
                best_offset = offset;
            }
        }
    }

    if best_biomass <= 0.0 {
        Vec2::ZERO
    } else {
        best_offset.normalize_or_zero()
    }
}

/// Wrap-aware direction unit vector from `from` toward `to`.
fn torus_direction(from: Vec2, to: Vec2) -> Vec2 {
    let mut dx = to.x - from.x;
    let mut dy = to.y - from.y;
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
    Vec2::new(dx, dy).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;
    use crate::world::World;

    #[test]
    fn agent_on_grass_sees_local_biomass() {
        let mut w = World::new(7);
        // Find any grass cell and spawn an agent at its center.
        let mut spawn = Vec2::ZERO;
        'outer: for row in 0..crate::biome::BIOME_RES {
            for col in 0..crate::biome::BIOME_RES {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    spawn = Vec2::new((col as f32 + 0.5) * CELL_SIZE, (row as f32 + 0.5) * CELL_SIZE);
                    break 'outer;
                }
            }
        }
        let _ = w.spawn_agent(spawn, Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.spatial, &mut regs);
        assert!(regs[0].local_plant_biomass > 0.0);
    }

    #[test]
    fn agent_finds_neighbor_within_perception() {
        let mut w = World::new(1);
        let pos_a = Vec2::new(100.0, 100.0);
        let pos_b = Vec2::new(104.0, 100.0);
        let _ = w.spawn_agent(pos_a, Genome::neutral());
        let _ = w.spawn_agent(pos_b, Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.spatial, &mut regs);
        assert!(regs[0].has_neighbor);
        assert!((regs[0].nearest_neighbor_dist - 4.0).abs() < 1e-3);
        assert!(regs[0].nearest_neighbor_dir.x > 0.9);
    }

    #[test]
    fn isolated_agent_has_no_neighbor() {
        let mut w = World::new(1);
        let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let mut regs = vec![SensorRegister::default(); w.agents.capacity()];
        sense_all(&w.agents, &w.biome, &w.spatial, &mut regs);
        assert!(!regs[0].has_neighbor);
        assert_eq!(regs[0].nearest_neighbor_dist, f32::INFINITY);
    }
}
```

- [ ] **Step 10.3: Run the sense tests**

Run: `cargo test -p anabios-core sense`

Expected: 3 passed.

- [ ] **Step 10.4: Commit**

```bash
git add crates/anabios-core/src/lib.rs crates/anabios-core/src/sense.rs
git commit -m "feat(core): sense stage populates per-agent sensor registers"
```

---

## Task 11: Decide stage (M1 hardcoded behavior)

**Goal:** A single, hardcoded behavior function turns sensor registers + genome into a desired velocity. In later milestones the evolvable behavior program replaces this; M1 keeps it simple.

**Files:**
- Modify: `crates/anabios-core/src/behavior.rs`

- [ ] **Step 11.1: Implement the M1 behavior**

Replace `crates/anabios-core/src/behavior.rs` with:

```rust
//! M1 hardcoded behavior function.
//!
//! Replaced in M4 by the evolvable behavior program. The function returns a
//! desired-velocity vector given the agent's genome and current sensor
//! register. Two drives:
//!
//! - **Forage** — when energy is below `reproduction_threshold * SPAWN_ENERGY`,
//!   move toward the best plant in perception.
//! - **Wander** — otherwise drift with low-amplitude correlated noise sampled
//!   from a per-tick uniform draw.

use crate::agent::SPAWN_ENERGY;
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;
use crate::rng::Rng;
use crate::sense::SensorRegister;

/// Maximum agent speed at `SpeedMax = 1.0`. In world units per tick.
pub const SPEED_MAX_CAP: f32 = 4.0;

/// Choose a desired velocity for one agent. Pure function of inputs.
///
/// `rng` is used for the wander noise. It is the *world's* RNG passed in by
/// the tick orchestrator; deterministic ordering is preserved by iterating
/// agents in ascending id order in `decide_all`.
pub fn decide(genome: &Genome, sensor: &SensorRegister, energy: f32, rng: &mut Rng) -> Vec2 {
    let speed_max = SPEED_MAX_CAP * genome.get(GenomeSlot::SpeedMax);
    if speed_max <= 0.0 {
        return Vec2::ZERO;
    }

    let hunger_threshold = SPAWN_ENERGY * genome.get(GenomeSlot::ReproductionThreshold);
    let is_hungry = energy < hunger_threshold;

    let direction = if is_hungry && sensor.plant_direction != Vec2::ZERO {
        sensor.plant_direction
    } else {
        // Wander: random unit vector blended with previous direction. We
        // don't have access to previous direction here without making the
        // sensor register stateful, so use a fresh random unit each tick;
        // the tick rate makes this look correlated enough at small dt.
        let theta = rng.f32_unit() * std::f32::consts::TAU;
        Vec2::new(theta.cos(), theta.sin())
    };

    direction * speed_max
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neutral_with(slot: GenomeSlot, v: f32) -> Genome {
        let mut g = Genome::neutral();
        g.set(slot, v);
        g
    }

    #[test]
    fn zero_speed_max_yields_zero_velocity() {
        let g = neutral_with(GenomeSlot::SpeedMax, 0.0);
        let s = SensorRegister::default();
        let mut rng = Rng::from_seed(1);
        let v = decide(&g, &s, 0.0, &mut rng);
        assert_eq!(v, Vec2::ZERO);
    }

    #[test]
    fn hungry_agent_with_plant_moves_toward_plant() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 1.0);
        g.set(GenomeSlot::ReproductionThreshold, 1.0); // always "hungry"
        let s = SensorRegister {
            plant_direction: Vec2::new(1.0, 0.0),
            ..Default::default()
        };
        let mut rng = Rng::from_seed(1);
        let v = decide(&g, &s, 0.0, &mut rng);
        assert!(v.x > 0.0);
        assert!((v.length() - SPEED_MAX_CAP).abs() < 1e-3);
    }

    #[test]
    fn well_fed_agent_wanders() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 1.0);
        g.set(GenomeSlot::ReproductionThreshold, 0.0); // never hungry
        let s = SensorRegister {
            plant_direction: Vec2::new(1.0, 0.0),
            ..Default::default()
        };
        // Even when a plant is in the sensor, a fed agent shouldn't be locked
        // onto +x; multiple draws should produce varying directions.
        let mut directions = std::collections::HashSet::new();
        for seed in 0..16 {
            let mut rng = Rng::from_seed(seed);
            let v = decide(&g, &s, SPAWN_ENERGY, &mut rng);
            let key = ((v.x * 100.0) as i32, (v.y * 100.0) as i32);
            directions.insert(key);
        }
        assert!(directions.len() >= 4, "wander should produce varied directions: {:?}", directions);
    }
}
```

- [ ] **Step 11.2: Run the behavior tests**

Run: `cargo test -p anabios-core behavior`

Expected: 3 passed.

- [ ] **Step 11.3: Commit**

```bash
git add crates/anabios-core/src/behavior.rs
git commit -m "feat(core): M1 hardcoded forage/wander behavior function"
```

---

## Task 12: Integrate stage (motion + energy expenditure)

**Goal:** Apply velocities to positions with torus wrap. Drain energy proportional to the movement performed.

**Files:**
- Create: `crates/anabios-core/src/integrate.rs`
- Modify: `crates/anabios-core/src/lib.rs`

- [ ] **Step 12.1: Add module declaration**

Edit `crates/anabios-core/src/lib.rs`. Add `pub mod integrate;` alongside the other modules so the list reads:

```rust
pub mod agent;
pub mod behavior;
pub mod biome;
pub mod genome;
pub mod integrate;
pub mod rng;
pub mod scenario;
pub mod sense;
pub mod snapshot;
pub mod spatial;
pub mod tick;
pub mod world;
```

- [ ] **Step 12.2: Implement integrate**

Create `crates/anabios-core/src/integrate.rs`:

```rust
//! Integration step: applies desired velocities to positions, wraps to the
//! torus, and drains energy proportional to movement plus a per-tick basal
//! metabolism cost.

use crate::agent::AgentBuffers;
use crate::biome::WORLD_SIZE;
use crate::genome::GenomeSlot;
use crate::prelude::{wrap_torus, Vec2};

/// Cost per world-unit of movement at `Size = 1.0`. Smaller agents pay less.
pub const MOVE_ENERGY_COST: f32 = 0.005;
/// Per-tick basal metabolism cost at `BasalMetabolism = 1.0`.
pub const BASAL_METABOLISM_COST: f32 = 0.05;

/// Apply `desired_velocity[i]` to each alive agent.
pub fn integrate_all(agents: &mut AgentBuffers, desired_velocity: &[Vec2]) {
    for id in agents.iter_alive().collect::<Vec<_>>() {
        let i = id as usize;
        let v = desired_velocity[i];
        agents.velocity[i] = v;

        let new_pos = agents.position[i] + v;
        agents.position[i] = wrap_torus(new_pos, Vec2::splat(WORLD_SIZE));

        let move_dist = v.length();
        let size = agents.genome[i].get(GenomeSlot::Size).max(0.1);
        let move_cost = MOVE_ENERGY_COST * move_dist * size;
        let basal_cost = BASAL_METABOLISM_COST * agents.genome[i].get(GenomeSlot::BasalMetabolism);
        agents.energy[i] -= move_cost + basal_cost;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::SPAWN_ENERGY;
    use crate::genome::Genome;
    use crate::world::World;

    #[test]
    fn position_wraps_on_torus() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(WORLD_SIZE - 1.0, 0.5), Genome::neutral());
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(3.0, 0.0);
        integrate_all(&mut w.agents, &desired);
        let p = w.agents.position[id as usize];
        assert!(p.x >= 0.0 && p.x < WORLD_SIZE);
        assert!((p.x - 2.0).abs() < 1e-3, "expected wrap-around to ~2.0, got {}", p.x);
    }

    #[test]
    fn motion_drains_energy_proportionally() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(4.0, 0.0);
        let before = w.agents.energy[id as usize];
        integrate_all(&mut w.agents, &desired);
        let after = w.agents.energy[id as usize];
        assert!(after < before);
        // Spawn energy should not have been touched outside the cost.
        let expected_move_cost = MOVE_ENERGY_COST * 4.0 * 0.5; // size = 0.5 in neutral genome
        let expected_basal = BASAL_METABOLISM_COST * 0.5;
        let drained = before - after;
        assert!((drained - (expected_move_cost + expected_basal)).abs() < 1e-3,
            "drained={drained}, expected~{}", expected_move_cost + expected_basal);
        // Sanity: still alive with non-zero energy.
        assert!(after < SPAWN_ENERGY);
    }
}
```

- [ ] **Step 12.3: Run the integrate tests**

Run: `cargo test -p anabios-core integrate`

Expected: 2 passed.

- [ ] **Step 12.4: Commit**

```bash
git add crates/anabios-core/src/lib.rs crates/anabios-core/src/integrate.rs
git commit -m "feat(core): integrate stage moves agents with torus wrap and energy drain"
```

---

## Task 13: Interact stage (feeding only in M1)

**Goal:** When an agent overlaps a plant-bearing cell, attempt to graze. The amount eaten is bounded by mouth capacity (M1 uses the `Size` trait as a proxy until M3 Modules ship).

**Files:**
- Create: `crates/anabios-core/src/interact.rs`
- Modify: `crates/anabios-core/src/lib.rs`

- [ ] **Step 13.1: Add module declaration**

Edit `crates/anabios-core/src/lib.rs`. Insert `pub mod interact;` in alphabetical order:

```rust
pub mod agent;
pub mod behavior;
pub mod biome;
pub mod genome;
pub mod integrate;
pub mod interact;
pub mod rng;
pub mod scenario;
pub mod sense;
pub mod snapshot;
pub mod spatial;
pub mod tick;
pub mod world;
```

- [ ] **Step 13.2: Implement interact**

Create `crates/anabios-core/src/interact.rs`:

```rust
//! Interaction step. In M1 the only interaction is **feeding**: agents in a
//! cell with plant biomass and a herbivorous diet (low `DietCarnivory`)
//! graze. Combat and mating land in later milestones.

use crate::agent::AgentBuffers;
use crate::biome::BiomeField;
use crate::genome::GenomeSlot;

/// Maximum plant biomass an agent can eat per tick at `Size = 1.0`.
pub const BITE_MAX: f32 = 0.5;
/// Energy gained per biomass unit consumed.
pub const FOOD_ENERGY_PER_BIOMASS: f32 = 4.0;

pub fn interact_all(agents: &mut AgentBuffers, biome: &mut BiomeField) {
    // Iterate in ascending id order for determinism. Two agents in the same
    // cell graze in id order, sharing the available biomass.
    let alive_ids: Vec<u32> = agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
        let pos = agents.position[i];
        let genome = &agents.genome[i];
        let herbivory = 1.0 - genome.get(GenomeSlot::DietCarnivory);
        if herbivory <= 0.0 {
            continue;
        }
        let size = genome.get(GenomeSlot::Size).max(0.1);
        let desired_bite = BITE_MAX * size * herbivory;
        let taken = biome.graze(pos, desired_bite);
        if taken > 0.0 {
            agents.energy[i] += taken * FOOD_ENERGY_PER_BIOMASS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::world::World;

    fn find_grass_cell_center(w: &World) -> Vec2 {
        for row in 0..crate::biome::BIOME_RES {
            for col in 0..crate::biome::BIOME_RES {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    return Vec2::new(
                        (col as f32 + 0.5) * crate::biome::CELL_SIZE,
                        (row as f32 + 0.5) * crate::biome::CELL_SIZE,
                    );
                }
            }
        }
        panic!("no grass cell in biome");
    }

    #[test]
    fn herbivore_on_grass_gains_energy() {
        let mut w = World::new(11);
        let pos = find_grass_cell_center(&w);
        let mut genome = Genome::neutral();
        genome.set(GenomeSlot::DietCarnivory, 0.0);
        let id = w.spawn_agent(pos, genome);
        let energy_before = w.agents.energy[id as usize];
        let biomass_before = w.biome.sample(pos).plant_biomass;
        interact_all(&mut w.agents, &mut w.biome);
        let energy_after = w.agents.energy[id as usize];
        let biomass_after = w.biome.sample(pos).plant_biomass;
        assert!(energy_after > energy_before);
        assert!(biomass_after < biomass_before);
    }

    #[test]
    fn obligate_carnivore_does_not_eat_plants() {
        let mut w = World::new(11);
        let pos = find_grass_cell_center(&w);
        let mut genome = Genome::neutral();
        genome.set(GenomeSlot::DietCarnivory, 1.0);
        let id = w.spawn_agent(pos, genome);
        let energy_before = w.agents.energy[id as usize];
        let biomass_before = w.biome.sample(pos).plant_biomass;
        interact_all(&mut w.agents, &mut w.biome);
        assert_eq!(w.agents.energy[id as usize], energy_before);
        assert_eq!(w.biome.sample(pos).plant_biomass, biomass_before);
    }

    #[test]
    fn two_agents_share_finite_biomass_deterministically() {
        let mut w = World::new(11);
        let pos = find_grass_cell_center(&w);
        // Drain to a small amount.
        let (col, row) = BiomeField::cell_coords(pos);
        w.biome.at_mut(col, row).plant_biomass = 0.3;
        let g = {
            let mut g = Genome::neutral();
            g.set(GenomeSlot::DietCarnivory, 0.0);
            g.set(GenomeSlot::Size, 1.0);
            g
        };
        let id0 = w.spawn_agent(pos, g);
        let id1 = w.spawn_agent(pos, g);
        interact_all(&mut w.agents, &mut w.biome);
        // First-in-id wins the larger share.
        assert!(w.agents.energy[id0 as usize] > w.agents.energy[id1 as usize]);
        assert!(w.biome.sample(pos).plant_biomass < 1e-5);
    }
}
```

- [ ] **Step 13.3: Run the interact tests**

Run: `cargo test -p anabios-core interact`

Expected: 3 passed.

- [ ] **Step 13.4: Commit**

```bash
git add crates/anabios-core/src/lib.rs crates/anabios-core/src/interact.rs
git commit -m "feat(core): interact stage with deterministic grazing"
```

---

## Task 14: Age and starve

**Goal:** Increment age, kill agents whose energy hit zero or who exceeded their lifespan.

**Files:**
- Create: `crates/anabios-core/src/age.rs`
- Modify: `crates/anabios-core/src/lib.rs`

- [ ] **Step 14.1: Add module declaration**

Edit `crates/anabios-core/src/lib.rs` to add `pub mod age;`:

```rust
pub mod age;
pub mod agent;
pub mod behavior;
pub mod biome;
pub mod genome;
pub mod integrate;
pub mod interact;
pub mod rng;
pub mod scenario;
pub mod sense;
pub mod snapshot;
pub mod spatial;
pub mod tick;
pub mod world;
```

- [ ] **Step 14.2: Implement age_and_starve**

Create `crates/anabios-core/src/age.rs`:

```rust
//! Ageing and death of agents at the end of each tick.

use crate::agent::AgentBuffers;
use crate::genome::GenomeSlot;

/// Maximum lifespan in ticks at `LifespanBias = 1.0`.
pub const LIFESPAN_MAX_TICKS: u32 = 5_000;
/// Minimum lifespan in ticks at `LifespanBias = 0.0` (so newborns aren't
/// instantly senescent).
pub const LIFESPAN_MIN_TICKS: u32 = 500;

pub fn age_and_starve(agents: &mut AgentBuffers) {
    let alive_ids: Vec<u32> = agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
        agents.age[i] = agents.age[i].saturating_add(1);

        let lifespan = lifespan_of(&agents.genome[i]);
        if agents.energy[i] <= 0.0 {
            agents.kill(id);
        } else if agents.age[i] >= lifespan {
            agents.kill(id);
        }
    }
}

/// Maximum tick age an agent of this genome can reach before dying of old age.
pub fn lifespan_of(genome: &crate::genome::Genome) -> u32 {
    let bias = genome.get(GenomeSlot::LifespanBias);
    let span = LIFESPAN_MIN_TICKS as f32
        + (LIFESPAN_MAX_TICKS - LIFESPAN_MIN_TICKS) as f32 * bias;
    span as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::world::World;

    #[test]
    fn age_increments_each_call() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        age_and_starve(&mut w.agents);
        age_and_starve(&mut w.agents);
        age_and_starve(&mut w.agents);
        assert_eq!(w.agents.age[id as usize], 3);
        assert!(w.agents.is_alive(id));
    }

    #[test]
    fn agent_with_zero_energy_dies() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.agents.energy[id as usize] = 0.0;
        age_and_starve(&mut w.agents);
        assert!(!w.agents.is_alive(id));
    }

    #[test]
    fn agent_dies_of_old_age_at_lifespan_bias_zero() {
        let mut w = World::new(1);
        let mut g = Genome::neutral();
        g.set(GenomeSlot::LifespanBias, 0.0);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), g);
        for _ in 0..LIFESPAN_MIN_TICKS as usize {
            age_and_starve(&mut w.agents);
            if !w.agents.is_alive(id) {
                break;
            }
        }
        assert!(!w.agents.is_alive(id));
    }
}
```

- [ ] **Step 14.3: Run the age tests**

Run: `cargo test -p anabios-core age`

Expected: 3 passed.

- [ ] **Step 14.4: Commit**

```bash
git add crates/anabios-core/src/lib.rs crates/anabios-core/src/age.rs
git commit -m "feat(core): age increment and death by starvation or old age"
```

---

## Task 15: Tick orchestrator

**Goal:** Wire up `sense → decide → integrate → interact → age` plus the periodic biome regrowth. Owns the scratch buffers (sensor registers, desired velocities) inside `World`.

**Files:**
- Modify: `crates/anabios-core/src/tick.rs`
- Modify: `crates/anabios-core/src/world.rs` (add scratch fields)

- [ ] **Step 15.1: Add scratch buffers to World**

Edit `crates/anabios-core/src/world.rs`. Find the `World` struct definition:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub tick: u64,
    pub seed: u64,
    pub rng: Rng,
    pub biome: BiomeField,
    pub agents: AgentBuffers,
    #[serde(skip)]
    pub spatial: UniformSpatialHash,
}
```

Replace it with the version below (adds two new scratch fields):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub tick: u64,
    pub seed: u64,
    pub rng: Rng,
    pub biome: BiomeField,
    pub agents: AgentBuffers,
    #[serde(skip)]
    pub spatial: UniformSpatialHash,
    #[serde(skip)]
    pub sensors: Vec<crate::sense::SensorRegister>,
    #[serde(skip)]
    pub desired_velocity: Vec<crate::prelude::Vec2>,
}
```

Then in `impl World`, update `new` to initialize the new fields. Replace:

```rust
    pub fn new(seed: u64) -> Self {
        Self {
            tick: 0,
            seed,
            rng: Rng::from_seed(seed),
            biome: BiomeField::generate(seed),
            agents: AgentBuffers::new(),
            spatial: UniformSpatialHash::new(),
        }
    }
```

with:

```rust
    pub fn new(seed: u64) -> Self {
        Self {
            tick: 0,
            seed,
            rng: Rng::from_seed(seed),
            biome: BiomeField::generate(seed),
            agents: AgentBuffers::new(),
            spatial: UniformSpatialHash::new(),
            sensors: Vec::new(),
            desired_velocity: Vec::new(),
        }
    }
```

And add a helper that grows the scratch buffers when the agent capacity grows. Add this method just before the closing `}` of `impl World`:

```rust
    /// Resize scratch buffers to match agent capacity. Called by the tick.
    pub(crate) fn resize_scratch(&mut self) {
        let cap = self.agents.capacity();
        if self.sensors.len() < cap {
            self.sensors.resize(cap, crate::sense::SensorRegister::default());
        }
        if self.desired_velocity.len() < cap {
            self.desired_velocity.resize(cap, crate::prelude::Vec2::ZERO);
        }
    }
```

- [ ] **Step 15.2: Implement the tick orchestrator**

Replace `crates/anabios-core/src/tick.rs` with:

```rust
//! Tick orchestration: the master `step()` function for M1.

use crate::age::age_and_starve;
use crate::behavior::decide;
use crate::integrate::integrate_all;
use crate::interact::interact_all;
use crate::sense::sense_all;
use crate::world::World;

/// How often (in ticks) the biome plant regrowth step runs.
pub const BIOME_STEP_INTERVAL: u64 = 10;

/// Advance the world by one tick.
pub fn step(world: &mut World) {
    world.resize_scratch();
    let cap = world.agents.capacity();

    // Stage 1: rebuild the spatial hash from current positions.
    world.spatial.rebuild(&world.agents.position, |i| world.agents.is_alive(i as u32));

    // Stage 2: sense.
    sense_all(&world.agents, &world.biome, &world.spatial, &mut world.sensors);

    // Stage 3: decide.
    decide_all(world);

    // Stage 4: integrate (motion + per-tick metabolism).
    integrate_all(&mut world.agents, &world.desired_velocity[..cap]);

    // Stage 5: interact (feeding).
    interact_all(&mut world.agents, &mut world.biome);

    // Stage 6: age + starve.
    age_and_starve(&mut world.agents);

    // Stage 7: periodic biome regrowth.
    if world.tick % BIOME_STEP_INTERVAL == 0 {
        world.biome.regrow_step();
    }

    world.tick += 1;
}

fn decide_all(world: &mut World) {
    // Deterministic order: ascending id.
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
        let genome = world.agents.genome[i];
        let sensor = world.sensors[i];
        let energy = world.agents.energy[i];
        world.desired_velocity[i] = decide(&genome, &sensor, energy, &mut world.rng);
    }
    // Dead slots keep their old velocities; they're never read because
    // `integrate_all` only iterates alive ids.
}

#[cfg(test)]
mod tests {
    use crate::biome::TerrainType;
    use crate::genome::{Genome, GenomeSlot};
    use crate::prelude::Vec2;
    use crate::world::World;

    use super::step;

    #[test]
    fn empty_world_can_tick() {
        let mut w = World::new(1);
        for _ in 0..100 {
            step(&mut w);
        }
        assert_eq!(w.tick, 100);
    }

    #[test]
    fn agent_in_food_rich_world_survives_initial_ticks() {
        let mut w = World::new(13);
        // Find a grass cell to spawn near.
        let mut spawn = Vec2::ZERO;
        'outer: for row in 0..crate::biome::BIOME_RES {
            for col in 0..crate::biome::BIOME_RES {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    spawn = Vec2::new(
                        (col as f32 + 0.5) * crate::biome::CELL_SIZE,
                        (row as f32 + 0.5) * crate::biome::CELL_SIZE,
                    );
                    break 'outer;
                }
            }
        }
        let mut g = Genome::neutral();
        g.set(GenomeSlot::DietCarnivory, 0.0);
        g.set(GenomeSlot::LifespanBias, 1.0);
        let id = w.spawn_agent(spawn, g);
        for _ in 0..200 {
            step(&mut w);
            if !w.agents.is_alive(id) {
                break;
            }
        }
        assert!(w.agents.is_alive(id), "well-fed agent on grass should survive 200 ticks");
    }

    #[test]
    fn starving_agent_dies() {
        let mut w = World::new(1);
        // Spawn far from any food and drain energy.
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.agents.energy[id as usize] = 0.5;
        for _ in 0..200 {
            step(&mut w);
            if !w.agents.is_alive(id) {
                break;
            }
        }
        assert!(!w.agents.is_alive(id));
    }
}
```

- [ ] **Step 15.3: Run the tick tests**

Run: `cargo test -p anabios-core tick`

Expected: 3 passed.

- [ ] **Step 15.4: Run all unit tests**

Run: `cargo test -p anabios-core --lib`

Expected: all pass.

- [ ] **Step 15.5: Run fmt and clippy**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`

Expected: no diffs, no warnings.

- [ ] **Step 15.6: Commit**

```bash
git add crates/anabios-core/src/world.rs crates/anabios-core/src/tick.rs
git commit -m "feat(core): tick orchestrator wires sense/decide/integrate/interact/age"
```

---

## Task 16: Snapshot serialization + state hash

**Goal:** Save/load `World` via bincode with a versioned wrapper. Compute a fast deterministic state hash for golden-tick replay tests.

**Files:**
- Modify: `crates/anabios-core/src/snapshot.rs`

- [ ] **Step 16.1: Implement Snapshot, save_to_bytes, load_from_bytes, state_hash**

Replace `crates/anabios-core/src/snapshot.rs` with:

```rust
//! World snapshot save/load + deterministic state hash.
//!
//! The serialized format is a versioned envelope around bincode-encoded
//! `World` bytes. `format_version` exists so future code can refuse or
//! migrate old snapshots cleanly.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::world::World;

/// Current snapshot format version. Bump on any breaking change to the
/// serialized layout.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct Envelope {
    format_version: u32,
    payload: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("bincode error: {0}")]
    Bincode(#[from] bincode::Error),
    #[error("unsupported snapshot format version {found}, expected {expected}")]
    Version { found: u32, expected: u32 },
}

pub fn save_to_bytes(world: &World) -> Result<Vec<u8>, SnapshotError> {
    let payload = bincode::serialize(world)?;
    let env = Envelope { format_version: FORMAT_VERSION, payload };
    Ok(bincode::serialize(&env)?)
}

pub fn load_from_bytes(bytes: &[u8]) -> Result<World, SnapshotError> {
    let env: Envelope = bincode::deserialize(bytes)?;
    if env.format_version != FORMAT_VERSION {
        return Err(SnapshotError::Version {
            found: env.format_version,
            expected: FORMAT_VERSION,
        });
    }
    let world: World = bincode::deserialize(&env.payload)?;
    Ok(world)
}

/// A 64-bit fingerprint of the world's persistent state. Uses FNV-1a over
/// the bincode-serialized payload. Suitable for golden-tick replay tests.
pub fn state_hash(world: &World) -> u64 {
    // Don't include scratch buffers; only persistent fields are serialized.
    let payload = bincode::serialize(world).expect("world is always serializable");
    fnv1a_64(&payload)
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut h = FNV_OFFSET;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::tick::step;

    #[test]
    fn roundtrip_preserves_state() {
        let mut w = World::new(123);
        for _ in 0..5 {
            let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        }
        for _ in 0..20 {
            step(&mut w);
        }
        let bytes = save_to_bytes(&w).expect("save");
        let w2 = load_from_bytes(&bytes).expect("load");
        assert_eq!(w.tick, w2.tick);
        assert_eq!(w.agents.live_count(), w2.agents.live_count());
        assert_eq!(state_hash(&w), state_hash(&w2));
    }

    #[test]
    fn state_hash_differs_after_a_tick() {
        let mut w = World::new(7);
        let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let h0 = state_hash(&w);
        step(&mut w);
        let h1 = state_hash(&w);
        assert_ne!(h0, h1);
    }

    #[test]
    fn version_mismatch_is_rejected() {
        let mut w = World::new(1);
        let _ = w.spawn_agent(Vec2::ZERO, Genome::neutral());
        let bytes = save_to_bytes(&w).expect("save");
        // Mutate the version byte. The Envelope is `{format_version: u32,
        // payload: Vec<u8>}`; bincode encodes the u32 LE first.
        let mut tampered = bytes.clone();
        tampered[0] = 99;
        let err = load_from_bytes(&tampered).err().expect("should error");
        assert!(matches!(err, SnapshotError::Version { .. }));
    }
}
```

- [ ] **Step 16.2: Run the snapshot tests**

Run: `cargo test -p anabios-core snapshot`

Expected: 3 passed.

- [ ] **Step 16.3: Commit**

```bash
git add crates/anabios-core/src/snapshot.rs
git commit -m "feat(core): bincode snapshot envelope and FNV state hash"
```

---

## Task 17: Scenario file (TOML)

**Goal:** A `Scenario` struct (initial conditions) loadable from TOML. Used by tests and the headless CLI.

**Files:**
- Modify: `crates/anabios-core/src/scenario.rs`
- Create: `scenarios/minimal.toml`

- [ ] **Step 17.1: Implement Scenario**

Replace `crates/anabios-core/src/scenario.rs` with:

```rust
//! Scenario initial conditions, loadable from TOML.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::biome::WORLD_SIZE;
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;
use crate::world::World;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub seed: u64,
    #[serde(default)]
    pub agents: Vec<AgentSpec>,
}

/// A request for `count` agents distributed via the given placement, each
/// initialized from the given trait overrides on top of a neutral genome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub count: u32,
    #[serde(default)]
    pub placement: Placement,
    #[serde(default)]
    pub traits: TraitOverrides,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TraitOverrides {
    pub speed_max: Option<f32>,
    pub perception_radius: Option<f32>,
    pub size: Option<f32>,
    pub diet_carnivory: Option<f32>,
    pub basal_metabolism: Option<f32>,
    pub lifespan_bias: Option<f32>,
    pub reproduction_threshold: Option<f32>,
}

impl TraitOverrides {
    pub fn apply(&self, g: &mut Genome) {
        if let Some(v) = self.speed_max {
            g.set(GenomeSlot::SpeedMax, v);
        }
        if let Some(v) = self.perception_radius {
            g.set(GenomeSlot::PerceptionRadius, v);
        }
        if let Some(v) = self.size {
            g.set(GenomeSlot::Size, v);
        }
        if let Some(v) = self.diet_carnivory {
            g.set(GenomeSlot::DietCarnivory, v);
        }
        if let Some(v) = self.basal_metabolism {
            g.set(GenomeSlot::BasalMetabolism, v);
        }
        if let Some(v) = self.lifespan_bias {
            g.set(GenomeSlot::LifespanBias, v);
        }
        if let Some(v) = self.reproduction_threshold {
            g.set(GenomeSlot::ReproductionThreshold, v);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Placement {
    /// Uniform random across the world bounds.
    Uniform,
    /// Cluster around `center` within `radius`.
    Cluster { center_x: f32, center_y: f32, radius: f32 },
}

impl Default for Placement {
    fn default() -> Self {
        Placement::Uniform
    }
}

#[derive(Debug, Error)]
pub enum ScenarioError {
    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

impl Scenario {
    pub fn parse_toml(text: &str) -> Result<Self, ScenarioError> {
        Ok(toml::from_str(text)?)
    }

    /// Build a `World` from this scenario. Determinism: world.rng is seeded
    /// from `seed`; agent positions for `Placement::Uniform` come from this
    /// RNG in agent-id order.
    pub fn instantiate(&self) -> World {
        let mut w = World::new(self.seed);
        for spec in &self.agents {
            for _ in 0..spec.count {
                let position = match spec.placement {
                    Placement::Uniform => {
                        let x = w.rng.f32_range(0.0, WORLD_SIZE);
                        let y = w.rng.f32_range(0.0, WORLD_SIZE);
                        Vec2::new(x, y)
                    }
                    Placement::Cluster { center_x, center_y, radius } => {
                        let theta = w.rng.f32_range(0.0, std::f32::consts::TAU);
                        let r = w.rng.f32_range(0.0, radius);
                        Vec2::new(center_x + r * theta.cos(), center_y + r * theta.sin())
                    }
                };
                let mut g = Genome::neutral();
                spec.traits.apply(&mut g);
                w.spawn_agent(position, g);
            }
        }
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_toml() {
        let text = r#"
name = "test"
seed = 42

[[agents]]
count = 10
placement = { kind = "uniform" }
[agents.traits]
speed_max = 0.5
size = 0.5
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        assert_eq!(s.name, "test");
        assert_eq!(s.seed, 42);
        assert_eq!(s.agents.len(), 1);
        assert_eq!(s.agents[0].count, 10);
        assert!(matches!(s.agents[0].placement, Placement::Uniform));
        assert_eq!(s.agents[0].traits.speed_max, Some(0.5));
    }

    #[test]
    fn instantiate_creates_requested_agents() {
        let text = r#"
name = "test"
seed = 1

[[agents]]
count = 25
[agents.traits]
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        let w = s.instantiate();
        assert_eq!(w.agents.live_count(), 25);
    }

    #[test]
    fn instantiation_is_deterministic() {
        let text = r#"
name = "test"
seed = 999

[[agents]]
count = 50
[agents.traits]
"#;
        let s = Scenario::parse_toml(text).expect("parse");
        let a = s.instantiate();
        let b = s.instantiate();
        for id in a.agents.iter_alive() {
            assert_eq!(a.agents.position[id as usize], b.agents.position[id as usize]);
        }
    }
}
```

- [ ] **Step 17.2: Create the minimal scenario file**

Create `scenarios/minimal.toml`:

```toml
name = "minimal"
seed = 12345

[[agents]]
count = 200
placement = { kind = "uniform" }

[agents.traits]
speed_max = 0.4
perception_radius = 0.5
size = 0.4
diet_carnivory = 0.0
basal_metabolism = 0.4
lifespan_bias = 0.6
reproduction_threshold = 0.5
```

- [ ] **Step 17.3: Run the scenario tests**

Run: `cargo test -p anabios-core scenario`

Expected: 3 passed.

- [ ] **Step 17.4: Commit**

```bash
git add crates/anabios-core/src/scenario.rs scenarios/minimal.toml
git commit -m "feat(core): Scenario struct with TOML loading and deterministic instantiation"
```

---

## Task 18: anabios-headless CLI

**Goal:** A `clap`-driven CLI that loads a scenario, runs N ticks, and prints summary metrics. Smoke test that ties everything together end-to-end.

**Files:**
- Modify: `crates/anabios-headless/src/main.rs`

- [ ] **Step 18.1: Implement the CLI**

Replace `crates/anabios-headless/src/main.rs` with:

```rust
//! Headless runner for anabios scenarios.

use std::path::PathBuf;

use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "anabios-headless", version, about = "Headless runner for anabios.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a scenario for N ticks and report summary metrics.
    Run {
        /// Path to a `.toml` scenario file.
        #[arg(long)]
        scenario: PathBuf,
        /// Number of ticks to run. Default 1000.
        #[arg(long, default_value_t = 1000)]
        ticks: u64,
        /// Optional explicit seed; overrides the scenario seed.
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Print summary of a scenario without running it.
    Info {
        #[arg(long)]
        scenario: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run { scenario, ticks, seed } => run(scenario, ticks, seed),
        Command::Info { scenario } => info(scenario),
    }
}

fn run(scenario_path: PathBuf, ticks: u64, seed: Option<u64>) -> Result<()> {
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario file {}", scenario_path.display()))?;
    let mut scenario = Scenario::parse_toml(&text)?;
    if let Some(s) = seed {
        scenario.seed = s;
    }

    let mut world = scenario.instantiate();
    println!(
        "scenario={} seed={} initial_agents={} initial_biomass={:.1}",
        scenario.name,
        world.seed,
        world.agents.live_count(),
        world.plant_biomass_total()
    );

    for _ in 0..ticks {
        step(&mut world);
    }

    let hash = state_hash(&world);
    println!(
        "ticks={} alive={} biomass={:.1} energy_total={:.1} state_hash=0x{:016x}",
        world.tick,
        world.agents.live_count(),
        world.plant_biomass_total(),
        world.alive_energy_total(),
        hash
    );

    Ok(())
}

fn info(scenario_path: PathBuf) -> Result<()> {
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario file {}", scenario_path.display()))?;
    let scenario = Scenario::parse_toml(&text)?;
    println!("{:#?}", scenario);
    Ok(())
}
```

- [ ] **Step 18.2: Smoke-run the CLI against the minimal scenario**

Run: `cargo run -q --release --bin anabios-headless -- run --scenario scenarios/minimal.toml --ticks 200`

Expected output: two lines of summary. `alive` should be > 0. Exact `state_hash` will be captured in the next task — for this step, just verify it runs without panicking and that `alive` is in `[1, 200]`.

- [ ] **Step 18.3: Smoke-run again with the same seed and confirm identical hash**

Run: `cargo run -q --release --bin anabios-headless -- run --scenario scenarios/minimal.toml --ticks 200 --seed 12345`

Then run the same command again. Both runs should print identical `state_hash` values.

- [ ] **Step 18.4: Commit**

```bash
git add crates/anabios-headless/src/main.rs
git commit -m "feat(headless): clap CLI runs scenarios and prints state hash"
```

---

## Task 19: Determinism integration test (golden tick hashes)

**Goal:** Pin state hashes at fixed ticks for the minimal scenario. Any future change that alters the simulation's deterministic output forces a deliberate hash regeneration.

**Files:**
- Create: `crates/anabios-core/tests/determinism.rs`

- [ ] **Step 19.1: Write the golden-tick test (initial hashes are placeholders that the test discovers and prints on first run)**

Create `crates/anabios-core/tests/determinism.rs`:

```rust
//! Pin deterministic state hashes at fixed ticks of the minimal scenario.
//!
//! When this test fails because hashes mismatch, the dev must either:
//! (a) confirm the change is intentional and update the constants below, or
//! (b) fix the regression. Hash changes must be deliberate.

use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

/// Hashes at (tick, hash). Generated by running this test once with the
/// `UPDATE_HASHES=1` env var (see below); copy the printed values into here.
const GOLDEN: &[(u64, u64)] = &[
    (0, 0x__REPLACE_AT_TICK_0__),
    (100, 0x__REPLACE_AT_TICK_100__),
    (1000, 0x__REPLACE_AT_TICK_1000__),
];

#[test]
fn minimal_scenario_matches_golden_hashes() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse minimal scenario");
    let mut world = scenario.instantiate();

    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    let max_tick = GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);

    while world.tick <= max_tick {
        while idx < GOLDEN.len() && GOLDEN[idx].0 == world.tick {
            let h = state_hash(&world);
            observed.push((world.tick, h));
            idx += 1;
        }
        if world.tick == max_tick {
            break;
        }
        step(&mut world);
    }

    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// regenerated hashes:");
        for (t, h) in &observed {
            println!("    ({}, 0x{:016x}),", t, h);
        }
        return;
    }

    for ((expected_tick, expected_hash), (got_tick, got_hash)) in GOLDEN.iter().zip(&observed) {
        assert_eq!(expected_tick, got_tick, "tick mismatch");
        assert_eq!(
            *expected_hash, *got_hash,
            "hash drift at tick {expected_tick}: expected 0x{expected_hash:016x}, got 0x{got_hash:016x}.\n\
             If this change is intentional, rerun with UPDATE_HASHES=1 and copy the printed values.",
        );
    }
}
```

- [ ] **Step 19.2: Generate the initial golden hashes**

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --test determinism -- --nocapture`

Expected: prints three lines like `(0, 0xabc…),`, `(100, 0xdef…),`, `(1000, 0x123…),`. Copy those three values into the `GOLDEN` array, replacing the placeholders.

- [ ] **Step 19.3: Confirm the pinned hashes pass without the env var**

Run: `cargo test -p anabios-core --test determinism`

Expected: 1 passed.

- [ ] **Step 19.4: Confirm two consecutive runs produce identical hashes**

Run: `cargo test -p anabios-core --test determinism` again. It should still pass — same hashes.

- [ ] **Step 19.5: Commit**

```bash
git add crates/anabios-core/tests/determinism.rs
git commit -m "test(core): golden-tick hash determinism test for minimal scenario"
```

---

## Task 20: Property tests for global invariants

**Goal:** `proptest` cases that hold for any seed/scenario perturbation.

**Files:**
- Create: `crates/anabios-core/tests/invariants.rs`

- [ ] **Step 20.1: Implement the invariant property tests**

Create `crates/anabios-core/tests/invariants.rs`:

```rust
//! Global invariants over any scenario × any seed.

use anabios_core::biome::WORLD_SIZE;
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude_test::Vec2;
use anabios_core::tick::step;
use anabios_core::world::World;
use proptest::prelude::*;

mod prelude_helper {
    // proptest needs a visible Vec2 module path that doesn't go through
    // anabios_core::prelude (which is crate-private). Re-export via the
    // glam type directly.
    pub use glam::Vec2;
}

fn build_world(seed: u64, agent_count: usize) -> World {
    let mut w = World::new(seed);
    for i in 0..agent_count {
        let x = ((i * 17) % 1024) as f32 + 0.5;
        let y = ((i * 31) % 1024) as f32 + 0.5;
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 0.4);
        g.set(GenomeSlot::DietCarnivory, 0.0);
        g.set(GenomeSlot::Size, 0.4);
        g.set(GenomeSlot::LifespanBias, 0.5);
        w.spawn_agent(prelude_helper::Vec2::new(x, y), g);
    }
    w
}

proptest! {
    /// All agent positions are inside the world bounds after any number of ticks.
    #[test]
    fn positions_stay_in_world(seed in 0u64..1_000, ticks in 0u64..500, count in 0usize..50) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for id in w.agents.iter_alive() {
            let p = w.agents.position[id as usize];
            prop_assert!(p.x >= 0.0 && p.x < WORLD_SIZE,
                "x out of range: {} (seed={seed} ticks={ticks})", p.x);
            prop_assert!(p.y >= 0.0 && p.y < WORLD_SIZE,
                "y out of range: {} (seed={seed} ticks={ticks})", p.y);
        }
    }

    /// Total plant biomass + agent energy can only grow due to regrowth, never
    /// from feeding alone. So between two adjacent non-regrowth ticks, total
    /// (biomass*FOOD_ENERGY_PER_BIOMASS + energy) should be non-increasing.
    #[test]
    fn energy_plus_biomass_does_not_grow_between_regrowth_ticks(
        seed in 0u64..1_000,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        // Drive the tick forward to a non-regrowth boundary first.
        step(&mut w);
        let before = combined_energy(&w);
        // Take 9 more steps to land just before the next regrowth tick
        // (BIOME_STEP_INTERVAL = 10).
        for _ in 0..8 {
            step(&mut w);
            let now = combined_energy(&w);
            prop_assert!(now <= before + 1e-2,
                "energy grew without regrowth: before={before} now={now}");
        }
    }

    /// Agent ids are never re-used while the original slot is still alive.
    #[test]
    fn ids_unique_among_alive(seed in 0u64..1_000, ticks in 0u64..200, count in 0usize..40) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        let mut sorted = alive.clone();
        sorted.sort();
        sorted.dedup();
        prop_assert_eq!(alive.len(), sorted.len());
    }
}

fn combined_energy(w: &World) -> f32 {
    use anabios_core::interact::FOOD_ENERGY_PER_BIOMASS;
    w.alive_energy_total() + w.plant_biomass_total() * FOOD_ENERGY_PER_BIOMASS
}
```

- [ ] **Step 20.2: Expose a test-only re-export for Vec2 from prelude**

The test above uses `anabios_core::prelude_test::Vec2`. Expose that name. Edit `crates/anabios-core/src/lib.rs`. After the `mod prelude;` line, add:

```rust
#[doc(hidden)]
pub mod prelude_test {
    pub use glam::Vec2;
}
```

- [ ] **Step 20.3: Run the property tests**

Run: `cargo test -p anabios-core --test invariants`

Expected: 3 properties pass with 256 cases each (proptest default).

- [ ] **Step 20.4: Commit**

```bash
git add crates/anabios-core/src/lib.rs crates/anabios-core/tests/invariants.rs
git commit -m "test(core): proptest invariants for bounds, energy conservation, id uniqueness"
```

---

## Task 21: Feeding integration test

**Goal:** A direct end-to-end test that a small population on a grass biome survives 500 ticks with stable biomass and energy.

**Files:**
- Create: `crates/anabios-core/tests/feeding.rs`

- [ ] **Step 21.1: Implement the integration test**

Create `crates/anabios-core/tests/feeding.rs`:

```rust
//! Integration test: a herbivore population on grass survives 500 ticks
//! without total collapse or runaway plant blow-up.

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

#[test]
fn population_persists_for_500_ticks() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    let initial_alive = world.agents.live_count();
    assert!(initial_alive > 0);

    let initial_biomass = world.plant_biomass_total();
    assert!(initial_biomass > 0.0);

    for _ in 0..500 {
        step(&mut world);
    }

    let final_alive = world.agents.live_count();
    let final_biomass = world.plant_biomass_total();
    // We expect attrition, but not extinction.
    assert!(final_alive > 0, "population went extinct in 500 ticks: {} -> {}", initial_alive, final_alive);
    // Biomass should remain in a reasonable band — not zero, not multiples
    // of carrying capacity.
    assert!(final_biomass > 0.0);
    assert!(final_biomass < initial_biomass * 1.5);
}
```

- [ ] **Step 21.2: Run the feeding integration test**

Run: `cargo test -p anabios-core --test feeding -- --nocapture`

Expected: 1 passed. If the population goes extinct, that's a sign the scenario tuning needs adjustment — but for the minimal scenario above (200 herbivores on a torus with grass), persistence at 500 ticks is realistic.

- [ ] **Step 21.3: Commit**

```bash
git add crates/anabios-core/tests/feeding.rs
git commit -m "test(core): integration test for population persistence at 500 ticks"
```

---

## Task 22: Criterion benchmarks

**Goal:** Pin per-tick cost numbers so future changes can be measured. Two scales: 1k agents and 10k agents.

**Files:**
- Create: `crates/anabios-core/benches/tick_bench.rs`
- Modify: `crates/anabios-core/Cargo.toml` (add `[[bench]]` target)

- [ ] **Step 22.1: Declare the bench target in Cargo.toml**

Append to `crates/anabios-core/Cargo.toml`:

```toml

[[bench]]
name = "tick_bench"
harness = false
```

- [ ] **Step 22.2: Implement the benches**

Create `crates/anabios-core/benches/tick_bench.rs`:

```rust
//! Per-tick benchmarks at 1k and 10k agents.

use anabios_core::biome::WORLD_SIZE;
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude_test::Vec2;
use anabios_core::tick::step;
use anabios_core::world::World;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn build_population(count: usize, seed: u64) -> World {
    let mut w = World::new(seed);
    for i in 0..count {
        let x = ((i.wrapping_mul(2_654_435_761)) as u32 as f32) / u32::MAX as f32 * WORLD_SIZE;
        let y = ((i.wrapping_mul(40_503)) as u32 as f32) / u32::MAX as f32 * WORLD_SIZE;
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 0.4);
        g.set(GenomeSlot::DietCarnivory, 0.0);
        g.set(GenomeSlot::Size, 0.4);
        w.spawn_agent(Vec2::new(x, y), g);
    }
    w
}

fn bench_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick");
    group.sample_size(20);
    for &count in &[1_000_usize, 10_000_usize] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            // Build once outside the timed loop.
            let world_template = build_population(count, 1);
            b.iter_batched(
                || world_template.clone(),
                |mut w| {
                    step(&mut w);
                    w
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_tick);
criterion_main!(benches);
```

- [ ] **Step 22.3: Run the benches**

Run: `cargo bench -p anabios-core --bench tick_bench`

Expected: criterion reports per-iteration times for the `1000` and `10000` cases. There's no pass/fail at this stage — capture the numbers so future regressions are visible. A reasonable M1 baseline on an M-series MacBook is **≤ 1 ms per 1k-agent tick** and **≤ 15 ms per 10k-agent tick**. If either is dramatically worse, profile before proceeding.

- [ ] **Step 22.4: Commit**

```bash
git add crates/anabios-core/Cargo.toml crates/anabios-core/benches/tick_bench.rs
git commit -m "bench(core): criterion tick benchmarks at 1k and 10k agents"
```

---

## Task 23: CI pipeline

**Goal:** GitHub Actions workflow that runs fmt, clippy, tests, and the headless determinism smoke on every push and PR.

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 23.1: Write the workflow file**

Create `.github/workflows/ci.yml`:

```yaml
name: ci

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"

jobs:
  rust:
    name: rust (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: fmt
        run: cargo fmt --all --check
      - name: clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: test (lib + unit)
        run: cargo test --workspace --lib
      - name: test (integration)
        run: cargo test --workspace --tests

  determinism:
    name: determinism smoke
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: build release
        run: cargo build --release -p anabios-headless
      - name: run twice and diff
        run: |
          set -euo pipefail
          a=$(./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 1000 | tail -n 1)
          b=$(./target/release/anabios-headless run --scenario scenarios/minimal.toml --ticks 1000 | tail -n 1)
          echo "a=$a"
          echo "b=$b"
          [ "$a" = "$b" ] || (echo "non-deterministic output" && exit 1)
```

- [ ] **Step 23.2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: rust fmt/clippy/test on linux+mac plus headless determinism smoke"
```

---

## Task 24: Final cleanup & green-bar pass

**Goal:** Run the entire test + lint surface one more time, fix any remaining clippy nits or warnings, tag the milestone.

- [ ] **Step 24.1: Full workspace check**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`

Expected: zero diffs, zero warnings, all tests pass.

- [ ] **Step 24.2: Verify the determinism smoke runs locally**

Run:

```bash
cargo run -q --release --bin anabios-headless -- run --scenario scenarios/minimal.toml --ticks 1000 > /tmp/a.txt
cargo run -q --release --bin anabios-headless -- run --scenario scenarios/minimal.toml --ticks 1000 > /tmp/b.txt
diff /tmp/a.txt /tmp/b.txt && echo "deterministic"
```

Expected: the two outputs match byte-for-byte. The `echo "deterministic"` line prints.

- [ ] **Step 24.3: Tag the milestone**

```bash
git tag -a m1 -m "M1: headless core skeleton (deterministic sim, agents eat plants)"
```

This tag is a local checkpoint. No need to push it unless the user wants to.

---

## Post-implementation expectations

After M1 is merged, the project has:

- A deterministic Rust simulation core with full test coverage of unit invariants, properties, and golden-tick hashes
- A CLI that runs scenarios from TOML and emits reproducible state hashes
- Per-tick benchmarks pinning baseline performance
- A CI pipeline that catches non-determinism and lint regressions
- File and module boundaries that match the design spec exactly, leaving clean seams for M2 (reproduction), M3 (modules), M4 (behavior program), and beyond

What it does **not** have yet:

- Reproduction or mutation in the simulation (genome `mutate_in_place` exists but isn't called by the tick — that's wired up in M2)
- Modules, behavior programs, codex, Godot rendering
- A scenario richer than "200 herbivores on grass"

Those are deliberately deferred to subsequent plans.
