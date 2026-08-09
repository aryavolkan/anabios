# Procedural Climate Biome + Genetic Local Adaptation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the biome a per-cell climate gradient and an `EnvAffinity` gene rewarded (when a flag is on) for matching local climate, so populations locally adapt and diverge into biome-adapted species.

**Architecture:** A dedicated climate `NoiseGrid` fills a new `BiomeCell.env`; an opt-in `World.biome_adaptation` flag gates a feeding-match bonus in `feed_pass` that rewards `EnvAffinity` (repurposed inert genome slot 41) matching the local cell's `env`, reusing the DIT triangular-kernel pattern. `EnvAffinity` counts toward speciation distance, driving biome-driven species divergence.

**Tech Stack:** Rust (`anabios-core`), deterministic seeded RNG.

## Global Constraints

- **Determinism:** the serialized layout changes ONCE (Task 1 adds `BiomeCell.env` + `World.biome_adaptation`). Task 1 does the single deliberate **golden refresh** + `snapshot::FORMAT_VERSION` bump (1 → 2). **Tasks 2–5 MUST hold the golden byte-identical** to Task 1's refreshed values (they add no serialized state and, with the flag OFF, no behavior change) — that byte-identity is the flag-off-invariance proof.
- **Terrain/biomass must stay byte-identical** to pre-feature: the climate `NoiseGrid` is drawn AFTER the existing `coarse`/`fine` terrain grids from the biome-gen `Rng`, so terrain generation is unchanged; only the new `env` field is added.
- **Flag off (default) ⇒ behavior identical:** the `feed_pass` affinity bonus is gated on `world.biome_adaptation`; `EnvAffinity`'s slot 41 already existed as `_SensoryReserved41` and already mutated, so no genome-layout or RNG change.
- **`EnvAffinity` counts toward `genome.distance`** — do NOT add it to the personality-exclusion list (this drives biome speciation, by design).
- All work in `anabios-core`. CI gate — stable toolchain: `rustup run stable cargo fmt --all --check` / `clippy --workspace --all-targets -- -D warnings` / `RUSTDOCFLAGS="-D warnings" ... doc --workspace --no-deps --document-private-items` / `test --workspace --lib --tests`. Commit fmt output. Escape `` `[0,1]` `` in doc comments.
- Constants: `ENV_AFFINITY_BONUS = 0.5`, `ENV_AFFINITY_TOLERANCE = 0.25`.

---

### Task 1: Climate field + adaptation-flag plumbing (+ golden refresh)

Add the new serialized state — `BiomeCell.env` (from a dedicated climate noise) and the opt-in `World.biome_adaptation` flag with scenario wiring. No behavior yet. One golden refresh.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (`BiomeCell`, `generate`)
- Modify: `crates/anabios-core/src/world.rs` (`World` struct + `new`)
- Modify: `crates/anabios-core/src/scenario.rs` (`Scenario` struct + `instantiate`)
- Modify: `crates/anabios-core/src/snapshot.rs` (`FORMAT_VERSION`)
- Modify: `crates/anabios-core/tests/determinism.rs` (refreshed hashes)

**Interfaces:**
- Produces: `BiomeCell.env: f32` (`[0,1]`); `World.biome_adaptation: bool`; `Scenario.biome_adaptation: bool`.

- [ ] **Step 1: Add `env` to `BiomeCell`**

In `crates/anabios-core/src/biome.rs`:
```rust
pub struct BiomeCell {
    pub terrain: TerrainType,
    pub plant_biomass: f32,
    /// Per-cell climate value in `[0,1]` from a dedicated noise field, semi-
    /// independent of terrain. Static after generation. Read by the biome-
    /// adaptation feeding bonus when `World.biome_adaptation` is on.
    pub env: f32,
}
```

- [ ] **Step 2: Fill `env` from a dedicated climate noise in `generate`**

Replace the `generate` body's grid setup + cell push (keep `coarse`/`fine` FIRST and unchanged so terrain is byte-identical):
```rust
    pub fn generate(seed: u64) -> Self {
        let mut rng = Rng::from_seed(seed);
        // Hash-based value-noise corner grid, sampled at two octaves (terrain).
        let coarse = NoiseGrid::new(&mut rng, 8);
        let fine = NoiseGrid::new(&mut rng, 24);
        // Dedicated climate field — drawn AFTER the terrain grids so terrain
        // generation is byte-identical to before. Different frequencies keep
        // climate semi-independent of terrain.
        let climate_coarse = NoiseGrid::new(&mut rng, 6);
        let climate_fine = NoiseGrid::new(&mut rng, 18);

        let mut cells = Vec::with_capacity(BIOME_RES * BIOME_RES);
        for row in 0..BIOME_RES {
            for col in 0..BIOME_RES {
                let u = col as f32 / BIOME_RES as f32;
                let v = row as f32 / BIOME_RES as f32;
                let n = 0.65 * coarse.sample(u, v) + 0.35 * fine.sample(u, v);
                let terrain = elevation_to_terrain(n);
                let env =
                    (0.7 * climate_coarse.sample(u, v) + 0.3 * climate_fine.sample(u, v)).clamp(0.0, 1.0);
                cells.push(BiomeCell { terrain, plant_biomass: terrain.carrying_capacity(), env });
            }
        }
        Self { cells }
    }
```

Search for other `BiomeCell { ... }` construction sites (tests, `regrow`) and add `env: <value>` — e.g. `grep -rn "BiomeCell {" crates/anabios-core/src`; a test cell can use `env: 0.5`.

- [ ] **Step 3: Add the `World` flag**

In `crates/anabios-core/src/world.rs`, after the `env_period` field:
```rust
    /// When true, the biome-adaptation feeding bonus (EnvAffinity vs local
    /// climate) is active. Off by default; opt-in per scenario.
    #[serde(default)]
    pub biome_adaptation: bool,
```
and in `World::new`, after `env_period: 0,`:
```rust
            biome_adaptation: false,
```

- [ ] **Step 4: Add the `Scenario` field + apply it**

In `crates/anabios-core/src/scenario.rs`, add to `struct Scenario` (near `env_period`):
```rust
    #[serde(default)]
    pub biome_adaptation: bool,
```
and in `instantiate`, next to `w.env_period = self.env_period;`:
```rust
        w.biome_adaptation = self.biome_adaptation;
```

- [ ] **Step 5: Bump the snapshot format version**

In `crates/anabios-core/src/snapshot.rs`: `pub const FORMAT_VERSION: u32 = 2;` (was 1 — the `BiomeCell`/`World` layout changed).

- [ ] **Step 6: Write the failing climate-field unit test**

In `biome.rs` `#[cfg(test)] mod tests`:
```rust
    #[test]
    fn climate_field_is_bounded_and_varies() {
        let b = BiomeField::generate(12345);
        let mut min = 1.0f32;
        let mut max = 0.0f32;
        for cell in b.cells.iter() {
            assert!((0.0..=1.0).contains(&cell.env), "env out of range: {}", cell.env);
            min = min.min(cell.env);
            max = max.max(cell.env);
        }
        assert!(max - min > 0.3, "climate field too flat: {min}..{max}");
    }

    #[test]
    fn climate_not_a_function_of_terrain_alone() {
        // Two cells of the SAME terrain should be able to differ in env.
        let b = BiomeField::generate(7);
        use std::collections::BTreeMap;
        let mut by_terrain: BTreeMap<u8, Vec<f32>> = BTreeMap::new();
        for cell in b.cells.iter() {
            by_terrain.entry(cell.terrain as u8).or_default().push(cell.env);
        }
        let varied = by_terrain
            .values()
            .any(|v| v.len() > 1 && v.iter().cloned().fold(0.0f32, f32::max) - v.iter().cloned().fold(1.0f32, f32::min) > 0.1);
        assert!(varied, "env should vary within at least one terrain type");
    }
```

- [ ] **Step 7: Build; confirm golden FAILS (expected); refresh**

Run: `rustup run stable cargo build -p anabios-core` — clean.
Run: `rustup run stable cargo test -p anabios-core --lib biome::` — the two new tests PASS.
Run: `rustup run stable cargo test -p anabios-core --test determinism` — FAILS (biome/world layout changed). Expected.
Run: `UPDATE_HASHES=1 rustup run stable cargo test -p anabios-core --test determinism -- --nocapture` — copy the printed triple into `crates/anabios-core/tests/determinism.rs` `GOLDEN`. Update the comment: "Refreshed 2026-07-17: added BiomeCell.env climate field + World.biome_adaptation flag (behavior unchanged with flag off; serialized layout changed)."

- [ ] **Step 8: Verify refreshed golden + full core suite**

Run: `rustup run stable cargo test -p anabios-core --test determinism` (twice — PASS both).
Run: `rustup run stable cargo test -p anabios-core --lib --tests` — PASS. (Behavior is unchanged — flag off, env unread — so all behavioral tests hold; only the hash moved.)

- [ ] **Step 9: Lint, format, commit — RECORD the refreshed hashes**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/biome.rs crates/anabios-core/src/world.rs crates/anabios-core/src/scenario.rs crates/anabios-core/src/snapshot.rs crates/anabios-core/tests/determinism.rs
git commit -m "feat(core): per-cell climate field + biome_adaptation flag (golden refresh)

BiomeCell.env from a dedicated climate noise (terrain byte-identical); opt-in
World.biome_adaptation flag. Behavior unchanged (flag off); one golden refresh
+ FORMAT_VERSION 1->2 for the layout change.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```
Note the refreshed `GOLDEN` triple in the commit body — Tasks 2–5 must hold it byte-identical.

---

### Task 2: `EnvAffinity` gene + `env_affinity_match` kernel

**Files:**
- Modify: `crates/anabios-core/src/genome.rs` (rename slot 41; accessor optional)
- Modify: `crates/anabios-core/src/culture.rs` (constants + kernel) — or a new small module; culture.rs already holds the sibling DIT kernel.

**Interfaces:**
- Produces: `GenomeSlot::EnvAffinity = 41`; `pub const ENV_AFFINITY_BONUS`/`ENV_AFFINITY_TOLERANCE`; `pub fn env_affinity_match(affinity: f32, env: f32) -> f32`.

- [ ] **Step 1: Rename the inert slot**

In `crates/anabios-core/src/genome.rs`, change (index unchanged):
```rust
    /// Genetic affinity in `[0,1]` for the local biome climate (`BiomeCell.env`).
    /// Read by the biome-adaptation feeding bonus when `World.biome_adaptation`
    /// is on. Counts toward speciation distance (drives biome-driven divergence).
    EnvAffinity = 41,
```
(replacing `_SensoryReserved41 = 41,`). Do NOT add it to `PERSONALITY_SLOTS` — it must count in `distance`. Grep for any `_SensoryReserved41` reference and update (there should be none outside the enum).

- [ ] **Step 2: Write the failing kernel test**

In `culture.rs` `#[cfg(test)] mod tests`:
```rust
    #[test]
    fn env_affinity_match_peaks_and_falls_off() {
        assert!((super::env_affinity_match(0.5, 0.5) - 1.0).abs() < 1e-6);
        assert_eq!(super::env_affinity_match(0.0, 1.0), 0.0); // > tolerance apart
        let m = super::env_affinity_match(0.5, 0.6);
        assert!(m > 0.0 && m < 1.0);
        for (a, e) in [(0.2, 0.9), (1.0, 0.0), (0.5, 0.5)] {
            let v = super::env_affinity_match(a, e);
            assert!((0.0..=1.0).contains(&v));
        }
    }
```

- [ ] **Step 3: Implement the constants + kernel**

In `crates/anabios-core/src/culture.rs` (near the DIT `ENV_BONUS`/`ENV_TOLERANCE`):
```rust
/// Feeding bonus multiplier for a perfect biome-climate affinity match.
pub const ENV_AFFINITY_BONUS: f32 = 0.5;
/// Affinity distance beyond which the biome-adaptation bonus is zero.
pub const ENV_AFFINITY_TOLERANCE: f32 = 0.25;

/// Triangular match kernel for genetic biome-climate adaptation: 1.0 at a
/// perfect match, linearly to 0.0 at `ENV_AFFINITY_TOLERANCE` apart. Both args
/// in `[0,1]`.
pub fn env_affinity_match(affinity: f32, env: f32) -> f32 {
    (1.0 - (affinity - env).abs() / ENV_AFFINITY_TOLERANCE).clamp(0.0, 1.0)
}
```

- [ ] **Step 4: Verify (byte-identical golden)**

Run: `rustup run stable cargo test -p anabios-core --lib culture:: genome::` — PASS incl. the new kernel test.
Run: `rustup run stable cargo test -p anabios-core --test determinism` — PASS, **byte-identical** to Task 1 (renaming a slot + adding an unused fn/consts changes no serialized bytes and no behavior).

- [ ] **Step 5: Lint, format, commit**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/genome.rs crates/anabios-core/src/culture.rs
git commit -m "feat(core): EnvAffinity gene (slot 41) + env_affinity_match kernel

Golden byte-identical (gene not yet read).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Wire the adaptation feeding bonus into `feed_pass`

**Files:**
- Modify: `crates/anabios-core/src/interact.rs` (`feed_pass`)

**Interfaces:**
- Consumes: `World.biome_adaptation` (Task 1), `env_affinity_match`/`ENV_AFFINITY_BONUS` (Task 2), `BiomeCell.env` (Task 1).

- [ ] **Step 1: Apply the bonus after the DIT/skill block, before graze**

In `crates/anabios-core/src/interact.rs` `feed_pass`, immediately AFTER the `if world.env_period > 0 { … } else if is_comm { … }` block and BEFORE the `// Individual technique learning (env mode)` block, insert:
```rust
        // Biome adaptation (opt-in): reward EnvAffinity matching the local
        // climate. Composes multiplicatively with the DIT/skill bonuses above.
        if world.biome_adaptation {
            let env = world.biome.sample(pos).env;
            let affinity = world.agents.genome[i].get(GenomeSlot::EnvAffinity);
            let m = crate::culture::env_affinity_match(affinity, env);
            desired_bite *= 1.0 + crate::culture::ENV_AFFINITY_BONUS * m;
        }
```
(`pos` is already `let pos = world.agents.position[i];` earlier in the loop. Reads no RNG.)

- [ ] **Step 2: Verify golden byte-identical (flag off = invariant)**

Run: `rustup run stable cargo build -p anabios-core` — clean.
Run: `rustup run stable cargo test -p anabios-core --test determinism` — PASS, byte-identical (all shipped scenarios have `biome_adaptation == false`, so the new block never runs).
Run: `rustup run stable cargo test -p anabios-core --lib --tests` — PASS.

- [ ] **Step 3: Lint, format, commit**

```bash
rustup run stable cargo fmt --all
rustup run stable cargo clippy -p anabios-core --all-targets -- -D warnings
git add crates/anabios-core/src/interact.rs
git commit -m "feat(core): biome-adaptation feeding bonus in feed_pass (flag-gated)

EnvAffinity vs local climate match multiplies bite when biome_adaptation is on.
Golden byte-identical (off in all shipped scenarios).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Scenario + local-adaptation behavioral proof

**Files:**
- Create: `scenarios/biome-adaptation.toml`
- Create: `crates/anabios-core/tests/biome_adaptation.rs`

- [ ] **Step 1: Add the scenario**

Create `scenarios/biome-adaptation.toml`:
```toml
name = "biome-adaptation"
seed = 0
biome_adaptation = true

[[agents]]
count = 300
placement = { kind = "uniform" }
[agents.traits]
lifespan_bias = 0.6
reproduction_threshold = 0.5
```

- [ ] **Step 2: Write the cline-formation integration test**

Create `crates/anabios-core/tests/biome_adaptation.rs`:
```rust
//! With biome_adaptation on, populations evolve a spatial EnvAffinity cline
//! matched to the local climate — agents in high-climate cells carry higher
//! affinity than those in low-climate cells (in-place local adaptation).

use anabios_core::genome::GenomeSlot;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/biome-adaptation.toml");

#[test]
fn affinity_cline_tracks_local_climate() {
    let mut w = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    assert!(w.biome_adaptation);
    for _ in 0..2500 {
        step(&mut w);
    }
    // Bucket alive agents by their local cell env (low half < 0.5 <= high half).
    let (mut lo_sum, mut lo_n, mut hi_sum, mut hi_n) = (0.0f32, 0u32, 0.0f32, 0u32);
    for id in w.agents.iter_alive() {
        let i = id as usize;
        let env = w.biome.sample(w.agents.position[i]).env;
        let aff = w.agents.genome[i].get(GenomeSlot::EnvAffinity);
        if env < 0.5 {
            lo_sum += aff;
            lo_n += 1;
        } else {
            hi_sum += aff;
            hi_n += 1;
        }
    }
    assert!(lo_n > 0 && hi_n > 0, "need agents in both climate halves ({lo_n}/{hi_n})");
    let lo_mean = lo_sum / lo_n as f32;
    let hi_mean = hi_sum / hi_n as f32;
    assert!(
        hi_mean > lo_mean,
        "high-climate agents should carry higher EnvAffinity than low-climate: hi={hi_mean} lo={lo_mean}"
    );
}
```

- [ ] **Step 3: Run the behavioral test**

Run: `rustup run stable cargo test -p anabios-core --test biome_adaptation`
Expected: PASS. If it fails (no cline), the selection is too weak or the run too short — first lengthen ticks; only if a real gain issue, raise `ENV_AFFINITY_BONUS` (and re-run Task 1's golden refresh only if a shipped scenario's behavior changed — it won't, since the constant is read only under the flag). Do NOT weaken the assertion. If a run goes extinct, note it and adjust population/lifespan so the metric is meaningful.

- [ ] **Step 4: Confirm the scenario runs via the sweep/all-scenarios path**

Run: `rustup run stable cargo test -p anabios-core --test all_scenarios`
Expected: PASS — `biome-adaptation.toml` parses, instantiates, and runs without NaN (all_scenarios iterates `scenarios/*.toml`).

- [ ] **Step 5: Commit**

```bash
rustup run stable cargo fmt --all
git add scenarios/biome-adaptation.toml crates/anabios-core/tests/biome_adaptation.rs
git commit -m "test(core): biome-adaptation scenario + affinity-cline proof

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Full verification

- [ ] **Step 1: Full CI gate**
```bash
rustup run stable cargo fmt --all --check
rustup run stable cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" rustup run stable cargo doc --workspace --no-deps --document-private-items
rustup run stable cargo test --workspace --lib --tests
```
Expected: all PASS (incl. the godot crate, which reads `BiomeCell` via `biome_colors` — confirm it still compiles; add `env` to any exhaustive `BiomeCell` match there if the compiler flags it, though field access shouldn't).

- [ ] **Step 2: Determinism reproducibility**
Run: `rustup run stable cargo test -p anabios-core --test determinism` (twice) — PASS both against Task 1's refreshed hashes (Tasks 2–4 held them byte-identical: flag-off invariance confirmed).

- [ ] **Step 3: Sanity — the feature actually does something**
Run: `rustup run stable cargo run -p anabios-headless --release -- run --scenario scenarios/biome-adaptation.toml --ticks 1500 --seed 0`
Expected: clean run, non-zero alive. (The affinity cline is asserted by Task 4's test; this just confirms the headless path.)

---

## Self-Review

**Spec coverage:**
- Component 1 climate field (BiomeCell.env, dedicated noise, terrain byte-identical) → Task 1. ✅
- Component 2 EnvAffinity gene + env_affinity_match kernel + counts-toward-distance → Task 2 (+ Task 3 wiring). ✅
- Component 3 opt-in flag + flag-off invariance → Task 1 (flag) + Task 3 (gated bonus) + byte-identical golden through Tasks 2–5. ✅
- Component 4 scenario + cline behavioral proof → Task 4. ✅
- Determinism: one golden refresh (Task 1), FORMAT_VERSION bump, byte-identical after → Task 1 + Global Constraints. ✅
- Unit tests (climate bounds/variance, kernel, accessor) → Tasks 1–2. ✅ CI gate → Task 5. ✅
- Out-of-scope (frontend viz, habitat-selection movement pull, more terrain, domestication) → absent. ✅

**Placeholder scan:** No TBD/TODO. Task 4 Step 3's "lengthen ticks / raise bonus if needed" is explicit tuning guidance with the guardrail "don't weaken the assertion", not a placeholder.

**Type consistency:** `BiomeCell.env: f32`, `World.biome_adaptation: bool`, `Scenario.biome_adaptation: bool`, `GenomeSlot::EnvAffinity = 41`, `env_affinity_match(f32,f32)->f32`, `ENV_AFFINITY_BONUS`/`ENV_AFFINITY_TOLERANCE` are used identically across Tasks 1–4. The golden-refresh workflow (`UPDATE_HASHES=1`) matches `determinism.rs`. `feed_pass` insertion point (after DIT/skill block, before learning block, using existing `pos`) is precise.
