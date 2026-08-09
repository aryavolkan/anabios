# Varied Nutrient Value + Soil Fertility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two independent, spatially-varying static per-cell fields to `anabios-core` — `nutrient_quality` (scales energy per bite) and `fertility` (scales carrying capacity + regrowth) — as the substrate for a foraging selection experiment.

**Architecture:** Both fields are noise-generated once at world-gen (like the existing `env` climate field), serialized into the golden hash, and consumed only through two opt-in flags (`nutrient_variation`, `soil_fertility`). Zero tick-time RNG. Selection acts through existing sensing/movement — no new genome slot. Design spec: `docs/superpowers/specs/2026-07-25-varied-nutrient-fertility-design.md`.

**Tech Stack:** Rust, `anabios-core` crate, `bincode` snapshots, FNV-1a golden hashing, TOML scenarios, cargo test.

## Global Constraints

- **Determinism contract:** simulation is fully deterministic; the golden `state_hash` (FNV-1a over the bincode payload) must be reproducible. Any change to serialized layout requires bumping `FORMAT_VERSION` and regenerating golden hashes.
- **Zero tick-time RNG:** all new randomness happens at world-gen only, drawn from the biome-generation `Rng`, appended *after* existing draws so terrain/`env` stay byte-identical.
- **Opt-in flags default off** (`#[serde(default)]`), and when off every new multiplier is forced to exactly `1.0` (bit-exact identity for finite floats), so flag-off *dynamics* are unchanged.
- **CI gate (must pass before push):** `cargo fmt --check` (committed tree) and `cargo doc` with `-D warnings`. Run `cargo fmt` before every commit.
- **Field/const naming (verbatim):** `nutrient_quality`, `fertility`, `nutrient_variation`, `soil_fertility`, `NUTRIENT_QUALITY_MIN = 0.6`, `NUTRIENT_QUALITY_MAX = 1.4`, `FERTILITY_MIN = 0.5`, `FERTILITY_MAX = 1.5`.
- **Heavy tests on PR CI:** the full determinism/golden suite runs on PR CI. Locally run the specific tests each task names; the golden regen in Task 1 is the one required local determinism run.

---

### Task 1: Schema + generation + flag plumbing (no behavior yet)

Adds the two `BiomeCell` fields, the four constants, the generation mapping, the two `World` flags, and the `Scenario` fields + instantiate wiring. Nothing consumes the new fields yet, so with flags off the *dynamics* are identical to baseline — but the serialized layout changes, so this is the single golden-rehash point.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (consts ~line 55; `BiomeCell` struct ~line 112; `BiomeField::generate` ~line 148; add unit tests in the `#[cfg(test)]` module ~line 547)
- Modify: `crates/anabios-core/src/world.rs` (add two flags after `pub climate_drift_rate: f32,` ~line 98)
- Modify: `crates/anabios-core/src/scenario.rs` (add two fields after the `climate_drift_rate` scenario field; wire in `instantiate` after `w.climate_drift_rate = self.climate_drift_rate;` ~line 327)
- Modify: `crates/anabios-core/src/snapshot.rs` (`FORMAT_VERSION` ~line 82)
- Modify: `crates/anabios-core/tests/determinism.rs` (`GOLDEN` const ~line 102)
- Test: `crates/anabios-core/tests/nutrient_fertility.rs` (new)

**Interfaces:**
- Produces: `BiomeCell { …, pub nutrient_quality: f32, pub fertility: f32 }`; `World { …, pub nutrient_variation: bool, pub soil_fertility: bool }`; consts `NUTRIENT_QUALITY_MIN/MAX`, `FERTILITY_MIN/MAX` in `biome`.

- [ ] **Step 1: Write the failing range test**

Create `crates/anabios-core/tests/nutrient_fertility.rs`:

```rust
//! Varied nutrient value + soil fertility: field generation, inertness when
//! flagged off, and (later tasks) consumption behavior.

use anabios_core::biome::{
    BiomeField, FERTILITY_MAX, FERTILITY_MIN, NUTRIENT_QUALITY_MAX, NUTRIENT_QUALITY_MIN,
};

#[test]
fn generated_fields_land_in_range() {
    let b = BiomeField::generate(0, 8, 1024.0);
    for cell in &b.cells {
        assert!(
            (NUTRIENT_QUALITY_MIN..=NUTRIENT_QUALITY_MAX).contains(&cell.nutrient_quality),
            "nutrient_quality {} out of range",
            cell.nutrient_quality
        );
        assert!(
            (FERTILITY_MIN..=FERTILITY_MAX).contains(&cell.fertility),
            "fertility {} out of range",
            cell.fertility
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --test nutrient_fertility generated_fields_land_in_range`
Expected: FAIL to compile — `nutrient_quality`/`fertility`/consts do not exist.

- [ ] **Step 3: Add the four constants**

In `crates/anabios-core/src/biome.rs`, after the existing `pub const SEASON_TOLERANCE` block (~line 55):

```rust
/// Per-cell nutrient-quality range (energy-per-bite multiplier). Mean ~1.0 so
/// the global energy economy is roughly conserved vs a flat world; the spatial
/// *variation* is the foraging-selection signal.
pub const NUTRIENT_QUALITY_MIN: f32 = 0.6;
pub const NUTRIENT_QUALITY_MAX: f32 = 1.4;
/// Per-cell soil-fertility range (scales carrying capacity AND regrowth rate).
/// Mean ~1.0 so global productivity is roughly conserved.
pub const FERTILITY_MIN: f32 = 0.5;
pub const FERTILITY_MAX: f32 = 1.5;
```

- [ ] **Step 4: Add the two `BiomeCell` fields**

In the `BiomeCell` struct (~line 112), append after `succession`:

```rust
    /// Static energy-per-bite multiplier for food grazed in this cell, in
    /// `[NUTRIENT_QUALITY_MIN, NUTRIENT_QUALITY_MAX]`. Generated once; consumed
    /// only when `World::nutrient_variation` is on.
    pub nutrient_quality: f32,
    /// Static soil-fertility multiplier scaling this cell's carrying capacity
    /// and regrowth rate, in `[FERTILITY_MIN, FERTILITY_MAX]`. Generated once;
    /// consumed only when `World::soil_fertility` is on.
    pub fertility: f32,
```

- [ ] **Step 5: Generate the fields in `BiomeField::generate`**

In `generate` (~line 148), after the existing `climate_fine` grid (~line 160), append two independent noise grids at distinct frequencies:

```rust
        // Nutrient-quality and fertility fields — drawn AFTER the climate grids
        // so terrain/env generation is byte-identical. Distinct frequencies from
        // env (3/9) and from each other so the three landscapes are uncorrelated.
        let nutrient_coarse = NoiseGrid::new(&mut rng, 5);
        let nutrient_fine = NoiseGrid::new(&mut rng, 13);
        let fertility_coarse = NoiseGrid::new(&mut rng, 4);
        let fertility_fine = NoiseGrid::new(&mut rng, 11);
```

Then inside the per-cell loop, after `env` is computed (~line 169), add the mapped values and set them on the pushed `BiomeCell`:

```rust
                let nq = (0.8 * nutrient_coarse.sample(u, v) + 0.2 * nutrient_fine.sample(u, v))
                    .clamp(0.0, 1.0);
                let nutrient_quality =
                    NUTRIENT_QUALITY_MIN + (NUTRIENT_QUALITY_MAX - NUTRIENT_QUALITY_MIN) * nq;
                let fe = (0.8 * fertility_coarse.sample(u, v) + 0.2 * fertility_fine.sample(u, v))
                    .clamp(0.0, 1.0);
                let fertility = FERTILITY_MIN + (FERTILITY_MAX - FERTILITY_MIN) * fe;
```

and add `nutrient_quality,` and `fertility,` to the `BiomeCell { … }` literal.

- [ ] **Step 6: Run the range test to verify it passes**

Run: `cargo test -p anabios-core --test nutrient_fertility generated_fields_land_in_range`
Expected: PASS.

- [ ] **Step 7: Add the `World` flags**

In `crates/anabios-core/src/world.rs`, after `pub climate_drift_rate: f32,` (~line 98):

```rust
    /// Opt-in: vary energy-per-bite by the local cell's `nutrient_quality`.
    /// `false` (default) forces the multiplier to exactly 1.0, leaving foraging
    /// energy unchanged. Zero RNG. The `nutrient_quality` field is always
    /// generated and serialized regardless of this flag. Same bincode/
    /// `FORMAT_VERSION` caveat as `env_period`.
    #[serde(default)]
    pub nutrient_variation: bool,
    /// Opt-in: scale each cell's carrying capacity AND regrowth rate by its
    /// `fertility`. `false` (default) forces the multiplier to exactly 1.0,
    /// leaving regrowth unchanged. Zero RNG. The `fertility` field is always
    /// generated and serialized regardless of this flag.
    #[serde(default)]
    pub soil_fertility: bool,
```

- [ ] **Step 8: Add the `Scenario` fields + instantiate wiring**

In `crates/anabios-core/src/scenario.rs`, after the `climate_drift_rate` scenario field:

```rust
    /// Opt-in: enable per-cell nutrient-value variation (energy per bite scaled
    /// by `nutrient_quality`). `false` (default) leaves foraging energy unchanged.
    #[serde(default)]
    pub nutrient_variation: bool,
    /// Opt-in: enable per-cell soil fertility (scales carrying capacity and
    /// regrowth). `false` (default) leaves regrowth unchanged.
    #[serde(default)]
    pub soil_fertility: bool,
```

In `instantiate`, after `w.climate_drift_rate = self.climate_drift_rate;` (~line 327):

```rust
        w.nutrient_variation = self.nutrient_variation;
        w.soil_fertility = self.soil_fertility;
```

- [ ] **Step 9: Write the "inert when flagged off" test**

Add to `crates/anabios-core/tests/nutrient_fertility.rs`:

```rust
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const MINIMAL: &str = include_str!("../../../scenarios/minimal.toml");

/// With both flags OFF (default), the nutrient_quality/fertility field VALUES
/// must not influence simulation dynamics: mutating them to extremes leaves the
/// biomass trajectory and agent energies bit-identical.
#[test]
fn fields_are_inert_when_flags_off() {
    let base = Scenario::parse_toml(MINIMAL).expect("parse");
    let mut a = base.clone().instantiate();
    let mut b = base.instantiate();
    assert!(!b.nutrient_variation && !b.soil_fertility);
    // Perturb every cell's new fields in world B only.
    for cell in b.biome.cells.iter_mut() {
        cell.nutrient_quality = 0.1;
        cell.fertility = 0.1;
    }
    for _ in 0..100 {
        step(&mut a);
        step(&mut b);
    }
    let biomass_a: Vec<f32> = a.biome.cells.iter().map(|c| c.plant_biomass).collect();
    let biomass_b: Vec<f32> = b.biome.cells.iter().map(|c| c.plant_biomass).collect();
    assert_eq!(biomass_a, biomass_b, "field values leaked into biomass dynamics");
    let energy_a: Vec<f32> = a.agents.iter_alive().map(|id| a.agents.energy[id as usize]).collect();
    let energy_b: Vec<f32> = b.agents.iter_alive().map(|id| b.agents.energy[id as usize]).collect();
    assert_eq!(energy_a, energy_b, "field values leaked into agent energy");
}
```

> Note: if `Scenario` does not already derive `Clone`, replace `base.clone()` by parsing `MINIMAL` twice. Verify with a quick check before running.

- [ ] **Step 10: Run the inert test**

Run: `cargo test -p anabios-core --test nutrient_fertility fields_are_inert_when_flags_off`
Expected: PASS (nothing consumes the fields yet).

- [ ] **Step 11: Bump `FORMAT_VERSION`**

In `crates/anabios-core/src/snapshot.rs` (~line 82):

```rust
pub const FORMAT_VERSION: u32 = 19;
```

- [ ] **Step 12: Confirm the golden test now fails**

Run: `cargo test -p anabios-core --test determinism minimal_scenario_matches_golden_hashes`
Expected: FAIL — hashes changed because `BiomeCell`/`World` gained serialized fields.

- [ ] **Step 13: Regenerate golden hashes**

Run: `UPDATE_HASHES=1 cargo test -p anabios-core --test determinism minimal_scenario_matches_golden_hashes -- --nocapture`
Copy the printed `(tick, hash)` tuples into the `GOLDEN` const in `crates/anabios-core/tests/determinism.rs` (~line 102), replacing the old values.

- [ ] **Step 14: Verify the golden test passes with new hashes**

Run: `cargo test -p anabios-core --test determinism minimal_scenario_matches_golden_hashes`
Expected: PASS.

- [ ] **Step 15: Format, then run the touched test files**

Run: `cargo fmt && cargo test -p anabios-core --test nutrient_fertility --test determinism`
Expected: PASS.

- [ ] **Step 16: Commit**

```bash
git add crates/anabios-core/src/biome.rs crates/anabios-core/src/world.rs \
  crates/anabios-core/src/scenario.rs crates/anabios-core/src/snapshot.rs \
  crates/anabios-core/tests/determinism.rs crates/anabios-core/tests/nutrient_fertility.rs
git commit -m "feat(biome): add nutrient_quality + fertility fields and flags (schema, no behavior)

Two static per-cell fields generated at world-gen (noise appended after
terrain/climate so existing draws are byte-identical). Opt-in flags
nutrient_variation/soil_fertility default off; fields inert when off.
FORMAT_VERSION 18->19, golden hashes regenerated.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: `soil_fertility` behavior — scale capacity + regrowth

Thread a per-cell fertility multiplier through the regrowth/recolonization paths, gated on `soil_fertility`. Golden scenario has the flag off (`× 1.0` is bit-exact), so no rehash.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (`regrow_climax` ~234, `regrow_succession` ~247, `regrow_step` ~289, `regrow_step_seasonal` ~299, `recolonize_step` ~309)
- Modify: `crates/anabios-core/src/tick.rs` (~lines 105–111 call sites)
- Test: `crates/anabios-core/tests/nutrient_fertility.rs`

**Interfaces:**
- Consumes: `BiomeCell.fertility`, `World.soil_fertility` (Task 1).
- Produces: `regrow_step(&mut self, soil_fertility: bool)`, `regrow_step_seasonal(&mut self, phase: f32, soil_fertility: bool)`, `recolonize_step(&mut self, soil_fertility: bool)` — signatures other callers (tick) rely on.

- [ ] **Step 1: Write the failing fertility-scaling test**

Add to `crates/anabios-core/tests/nutrient_fertility.rs`:

```rust
use anabios_core::biome::TerrainType;

/// With soil_fertility ON, a high-fertility Grass cell reaches a higher standing
/// crop than a low-fertility one; Water stays barren regardless.
#[test]
fn fertility_scales_capacity_and_regrowth() {
    let mut b = BiomeField::generate(0, 8, 1024.0);
    // Cell 0: fertile grass. Cell 1: poor grass. Cell 2: water (barren).
    for (idx, (terr, fert)) in [
        (TerrainType::Grass, 1.5),
        (TerrainType::Grass, 0.5),
        (TerrainType::Water, 1.5),
    ]
    .into_iter()
    .enumerate()
    {
        let c = &mut b.cells[idx];
        c.terrain = terr;
        c.fertility = fert;
        c.plant_biomass = if terr == TerrainType::Water { 0.0 } else { 1.0 };
        c.succession = anabios_core::biome::SUCCESSION_CLIMAX;
        c.pollution = 0.0;
    }
    for _ in 0..2000 {
        b.regrow_step(true);
    }
    assert!(
        b.cells[0].plant_biomass > b.cells[1].plant_biomass,
        "fertile {} should exceed poor {}",
        b.cells[0].plant_biomass,
        b.cells[1].plant_biomass
    );
    // Fertile grass should exceed the flat carrying capacity (10.0) it would cap
    // at with fertility ignored.
    assert!(b.cells[0].plant_biomass > 10.0);
    assert_eq!(b.cells[2].plant_biomass, 0.0, "water stays barren");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p anabios-core --test nutrient_fertility fertility_scales_capacity_and_regrowth`
Expected: FAIL to compile — `regrow_step` takes no argument yet.

- [ ] **Step 3: Thread `fert` through `regrow_climax`**

Replace `regrow_climax` (~line 234). It already takes `capacity` and `rate_mult` from the caller — the caller will now pass a fertility-scaled `capacity` and fold `fert` into `rate_mult`, so `regrow_climax` itself needs no change. (Confirm the caller changes in Steps 4–5 below; leave `regrow_climax` as-is.)

- [ ] **Step 4: Scale capacity + rate in `regrow_succession`**

Change `regrow_succession` (~line 247) to accept a fertility multiplier and apply it to both capacity and every rate:

```rust
    #[inline]
    fn regrow_succession(cell: &mut BiomeCell, rate_mult_fn: impl Fn(&BiomeCell) -> f32, fert: f32) {
        let base_cap = cell.terrain.carrying_capacity();
        if base_cap <= 0.0 {
            return;
        }
        let capacity = base_cap * fert;
        match cell.succession {
            SUCCESSION_BARE => {
                cell.plant_biomass =
                    (cell.plant_biomass + BARE_RESEED_FRAC * capacity).min(capacity);
                if cell.plant_biomass > PIONEER_ENTRY_FRAC * capacity {
                    cell.succession = SUCCESSION_PIONEER;
                }
            }
            SUCCESSION_PIONEER => {
                let pcap = capacity * PIONEER_CAPACITY_MULT;
                if cell.plant_biomass <= 0.0 {
                    cell.succession = SUCCESSION_BARE;
                    return;
                }
                let r = cell.terrain.regrowth_rate()
                    * Self::pollution_mult(cell)
                    * rate_mult_fn(cell)
                    * fert
                    * PIONEER_RATE_MULT;
                let b = cell.plant_biomass;
                let next = b + r * b * (1.0 - b / pcap);
                cell.plant_biomass = next.clamp(0.0, pcap);
                if cell.plant_biomass >= pcap * CLIMAX_ENTRY_FRAC {
                    cell.succession = SUCCESSION_CLIMAX;
                }
            }
            _ => {
                let r = cell.terrain.regrowth_rate() * Self::pollution_mult(cell) * rate_mult_fn(cell)
                    * fert;
                let b = cell.plant_biomass;
                if b > 0.0 {
                    let next = b + r * b * (1.0 - b / capacity);
                    cell.plant_biomass = next.clamp(0.0, capacity);
                }
            }
        }
    }
```

> This inlines the Climax arithmetic (previously delegated to `regrow_climax`) so the fertility-scaled `capacity` and `rate` are used together. `regrow_climax` is now unused — delete it and its doc comment to avoid a dead-code warning (`-D warnings` in CI).

- [ ] **Step 5: Add the `fert` selection to the `regrow_step*` entry points**

```rust
    pub fn regrow_step(&mut self, soil_fertility: bool) {
        for cell in self.cells.iter_mut() {
            Self::decay_pollution(cell);
            let fert = if soil_fertility { cell.fertility } else { 1.0 };
            Self::regrow_succession(cell, |_| 1.0, fert);
        }
    }

    pub fn regrow_step_seasonal(&mut self, phase: f32, soil_fertility: bool) {
        for cell in self.cells.iter_mut() {
            Self::decay_pollution(cell);
            let fert = if soil_fertility { cell.fertility } else { 1.0 };
            Self::regrow_succession(
                cell,
                |c| 1.0 + SEASON_AMPLITUDE * season_match(c.env, phase),
                fert,
            );
        }
    }
```

- [ ] **Step 6: Scale the recolonize ceiling by `fert`**

In `recolonize_step` (~line 309), change the signature to `pub fn recolonize_step(&mut self, soil_fertility: bool)` and scale the two `cap` reads by fertility:

```rust
                let cap = self.cells[idx].terrain.carrying_capacity()
                    * if soil_fertility { self.cells[idx].fertility } else { 1.0 };
```

and in the apply loop:

```rust
        for (cell, a) in self.cells.iter_mut().zip(add.iter()) {
            if *a > 0.0 {
                let cap = cell.terrain.carrying_capacity()
                    * if soil_fertility { cell.fertility } else { 1.0 };
                cell.plant_biomass = (cell.plant_biomass + *a).min(cap);
            }
        }
```

- [ ] **Step 7: Update the tick call sites**

In `crates/anabios-core/src/tick.rs` (~lines 103–111), read the flag into a local first to avoid a borrow conflict, then pass it:

```rust
    if world.tick.is_multiple_of(BIOME_STEP_INTERVAL) {
        let sf = world.soil_fertility;
        if world.living_biome {
            world.biome.recolonize_step(sf);
        }
        if world.season_period > 0 {
            let phase = /* existing phase expression */;
            world.biome.regrow_step_seasonal(phase, sf);
        } else {
            world.biome.regrow_step(sf);
        }
        // ... existing resource_step(world) etc. unchanged ...
    }
```

> Keep the existing `phase` computation exactly as it is; only the method calls gain the `sf` argument.

- [ ] **Step 8: Run the fertility test**

Run: `cargo test -p anabios-core --test nutrient_fertility fertility_scales_capacity_and_regrowth`
Expected: PASS.

- [ ] **Step 9: Verify goldens unchanged (flag off → bit-exact)**

Run: `cargo test -p anabios-core --test determinism minimal_scenario_matches_golden_hashes`
Expected: PASS with the Task 1 hashes (no rehash — `× 1.0` is identity).

- [ ] **Step 10: Format and run the full core test suite**

Run: `cargo fmt && cargo test -p anabios-core`
Expected: PASS (watch `all_scenarios`, `disturbance`, `living_sandbox` — they exercise regrowth).

- [ ] **Step 11: Commit**

```bash
git add crates/anabios-core/src/biome.rs crates/anabios-core/src/tick.rs \
  crates/anabios-core/tests/nutrient_fertility.rs
git commit -m "feat(biome): soil_fertility scales carrying capacity and regrowth

Per-cell fertility multiplier threaded through regrow/recolonize, gated
on World::soil_fertility (x1.0 identity when off, goldens unchanged).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: `nutrient_variation` behavior — scale energy per bite

Multiply the herbivory energy payout by the local cell's `nutrient_quality` when the flag is on. Biomass removed is unchanged.

**Files:**
- Modify: `crates/anabios-core/src/interact.rs` (`feed_pass` energy payout ~lines 129–134)
- Test: `crates/anabios-core/tests/nutrient_fertility.rs`

**Interfaces:**
- Consumes: `BiomeCell.nutrient_quality`, `World.nutrient_variation` (Task 1).

- [ ] **Step 1: Write the failing quality test**

Add to `crates/anabios-core/tests/nutrient_fertility.rs`:

```rust
/// With nutrient_variation ON, uniformly high-quality cells yield more total
/// forage energy than uniformly low-quality cells over the same run.
#[test]
fn nutrient_quality_scales_forage_energy() {
    let make = |q: f32| {
        let mut w = Scenario::parse_toml(MINIMAL).expect("parse").instantiate();
        w.nutrient_variation = true;
        for cell in w.biome.cells.iter_mut() {
            cell.nutrient_quality = q;
        }
        w
    };
    let mut hi = make(1.4);
    let mut lo = make(0.6);
    for _ in 0..20 {
        step(&mut hi);
        step(&mut lo);
    }
    let sum = |w: &anabios_core::world::World| -> f32 {
        w.agents.iter_alive().map(|id| w.agents.energy[id as usize]).sum()
    };
    assert!(
        sum(&hi) > sum(&lo),
        "high-quality total energy {} should exceed low {}",
        sum(&hi),
        sum(&lo)
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p anabios-core --test nutrient_fertility nutrient_quality_scales_forage_energy`
Expected: FAIL — energies equal (quality not yet consumed).

- [ ] **Step 3: Apply the quality multiplier in `feed_pass`**

In `crates/anabios-core/src/interact.rs`, in the `if taken > 0.0` block (~line 130), compute the multiplier and fold it into the energy payout:

```rust
        let taken = world.biome.graze(pos, desired_bite);
        if taken > 0.0 {
            let quality_mult = if world.nutrient_variation {
                world.biome.sample(pos).nutrient_quality
            } else {
                1.0
            };
            // Fire buff: cooked food yields more energy per biomass unit.
            world.agents.energy[i] += taken
                * FOOD_ENERGY_PER_BIOMASS
                * crate::invention::food_energy_multiplier(inv_mask)
                * quality_mult;
            // ... existing C cumulative-skill learning block unchanged ...
        }
```

> `taken` (biomass removed) is unchanged — only the energy conversion is scaled, which is the foraging pressure.

- [ ] **Step 4: Run the quality test**

Run: `cargo test -p anabios-core --test nutrient_fertility nutrient_quality_scales_forage_energy`
Expected: PASS.

- [ ] **Step 5: Verify goldens unchanged**

Run: `cargo test -p anabios-core --test determinism minimal_scenario_matches_golden_hashes`
Expected: PASS (flag off → `× 1.0`).

- [ ] **Step 6: Format and run feeding + core tests**

Run: `cargo fmt && cargo test -p anabios-core --test feeding --test nutrient_fertility --test determinism`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/anabios-core/src/interact.rs crates/anabios-core/tests/nutrient_fertility.rs
git commit -m "feat(interact): nutrient_variation scales energy per bite

feed_pass multiplies the herbivory payout by the local cell's
nutrient_quality when the flag is on; biomass removed unchanged.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Foraging metrics (`forage_quality_gain`, `forage_fertility_gain`)

Two pure, deterministic functions measuring how strongly the live population is concentrated in rich cells relative to the map average — the experiment's observable, since there is no gene to track.

**Files:**
- Create: `crates/anabios-core/src/metrics.rs`
- Modify: `crates/anabios-core/src/lib.rs` (register `pub mod metrics;`, keeping alphabetical order)
- Test: unit tests inside `metrics.rs`

**Interfaces:**
- Consumes: `World`, `BiomeField::sample`, `World::agents.iter_alive()`.
- Produces: `pub fn forage_quality_gain(world: &World) -> f32`; `pub fn forage_fertility_gain(world: &World) -> f32`.

- [ ] **Step 1: Write the module with a failing-to-compile test**

Create `crates/anabios-core/src/metrics.rs`:

```rust
//! Foraging-selection observables. Pure functions of world state, zero RNG.
//! `forage_quality_gain` / `forage_fertility_gain` = (population-weighted mean
//! field value at live-agent positions) − (global mean field value over cells).
//! `> 0` and rising over generations ⇒ foragers concentrate in rich patches.

use crate::world::World;

fn global_mean(vals: impl Iterator<Item = f32>, n: usize) -> f32 {
    if n == 0 {
        return 0.0;
    }
    vals.sum::<f32>() / n as f32
}

/// Mean `nutrient_quality` under live agents minus the global cell mean.
pub fn forage_quality_gain(world: &World) -> f32 {
    let cells = &world.biome.cells;
    if cells.is_empty() {
        return 0.0;
    }
    let map_mean = global_mean(cells.iter().map(|c| c.nutrient_quality), cells.len());
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for id in world.agents.iter_alive() {
        sum += world.biome.sample(world.agents.position[id as usize]).nutrient_quality;
        n += 1;
    }
    if n == 0 {
        return 0.0;
    }
    sum / n as f32 - map_mean
}

/// Mean `fertility` under live agents minus the global cell mean.
pub fn forage_fertility_gain(world: &World) -> f32 {
    let cells = &world.biome.cells;
    if cells.is_empty() {
        return 0.0;
    }
    let map_mean = global_mean(cells.iter().map(|c| c.fertility), cells.len());
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for id in world.agents.iter_alive() {
        sum += world.biome.sample(world.agents.position[id as usize]).fertility;
        n += 1;
    }
    if n == 0 {
        return 0.0;
    }
    sum / n as f32 - map_mean
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Scenario;

    const MINIMAL: &str = include_str!("../../../scenarios/minimal.toml");

    #[test]
    fn uniform_field_gives_zero_gain() {
        let mut w = Scenario::parse_toml(MINIMAL).expect("parse").instantiate();
        for c in w.biome.cells.iter_mut() {
            c.nutrient_quality = 1.0;
        }
        assert_eq!(forage_quality_gain(&w), 0.0);
    }

    #[test]
    fn agents_on_rich_cells_give_positive_gain() {
        let mut w = Scenario::parse_toml(MINIMAL).expect("parse").instantiate();
        // Left half poor, right half rich (split on x).
        let half = w.biome.world_size / 2.0;
        for row in 0..w.biome.res {
            for col in 0..w.biome.res {
                let idx = row * w.biome.res + col;
                let x = col as f32 * w.biome.cell_size;
                w.biome.cells[idx].nutrient_quality = if x >= half { 1.4 } else { 0.6 };
            }
        }
        // Move every live agent onto the rich (right) half.
        for id in w.agents.iter_alive() {
            let i = id as usize;
            w.agents.position[i].x = half + 1.0;
        }
        assert!(forage_quality_gain(&w) > 0.0);
    }
}
```

> Verify the public field/accessor names before running: `world.biome.cells`, `world.biome.res`, `world.biome.cell_size`, `world.biome.world_size`, `world.agents.position`, `world.agents.iter_alive()`. If any differ, adjust to the actual names (all are already used elsewhere in the crate/tests).

- [ ] **Step 2: Register the module**

In `crates/anabios-core/src/lib.rs`, add `pub mod metrics;` in alphabetical position among the existing `pub mod` declarations.

- [ ] **Step 3: Run the metric tests**

Run: `cargo test -p anabios-core metrics::`
Expected: PASS.

- [ ] **Step 4: Format and commit**

```bash
cargo fmt
git add crates/anabios-core/src/metrics.rs crates/anabios-core/src/lib.rs
git commit -m "feat(metrics): forage_quality_gain / forage_fertility_gain observables

Population-vs-map-mean concentration metrics for the foraging experiment
(no gene sweep to track). Pure functions, zero RNG.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Experiment scenario + end-to-end test

Add the `foraging-selection` scenario (both flags on) plus a control-free integration test proving the plumbing runs end to end and the metric is computable. `emergence.sh` and `all_scenarios.rs` auto-discover the TOML — no harness code changes.

**Files:**
- Create: `scenarios/foraging-selection.toml`
- Test: `crates/anabios-core/tests/nutrient_fertility.rs`

**Interfaces:**
- Consumes: flags (Task 1), metric functions (Task 4).

- [ ] **Step 1: Create the scenario**

`scenarios/foraging-selection.toml` (mirrors `biome-adaptation.toml`, both flags on):

```toml
name = "foraging-selection"
seed = 0
nutrient_variation = true
soil_fertility = true

[[agents]]
count = 300
placement = { kind = "uniform" }
[agents.traits]
lifespan_bias = 0.6
reproduction_threshold = 0.5
```

- [ ] **Step 2: Write the end-to-end test**

Add to `crates/anabios-core/tests/nutrient_fertility.rs`:

```rust
use anabios_core::metrics::{forage_fertility_gain, forage_quality_gain};

const FORAGING: &str = include_str!("../../../scenarios/foraging-selection.toml");

#[test]
fn foraging_scenario_runs_and_metrics_are_finite() {
    let mut w = Scenario::parse_toml(FORAGING).expect("parse").instantiate();
    assert!(w.nutrient_variation && w.soil_fertility, "flags must be on");
    w.max_population = 500; // keep the run fast
    for _ in 0..300 {
        step(&mut w);
    }
    assert!(w.agents.iter_alive().count() > 0, "population collapsed");
    let q = forage_quality_gain(&w);
    let f = forage_fertility_gain(&w);
    assert!(q.is_finite() && f.is_finite(), "metrics must be finite: q={q} f={f}");
}
```

> This asserts the mechanism runs end to end and the observable is computable. The *scientific* result (does the gain rise over generations?) is read from a long `scripts/emergence.sh soak foraging-selection` run, not encoded as a brittle unit assertion.

- [ ] **Step 3: Run the new test + the all-scenarios guard**

Run: `cargo test -p anabios-core --test nutrient_fertility foraging_scenario_runs_and_metrics_are_finite && cargo test -p anabios-core --test all_scenarios`
Expected: PASS (all_scenarios auto-picks up the new TOML).

- [ ] **Step 4: Format and run the full suite**

Run: `cargo fmt && cargo test -p anabios-core`
Expected: PASS.

- [ ] **Step 5: Sanity-check the harness lists it**

Run: `scripts/emergence.sh list | grep foraging-selection`
Expected: the scenario appears.

- [ ] **Step 6: Commit**

```bash
git add scenarios/foraging-selection.toml crates/anabios-core/tests/nutrient_fertility.rs
git commit -m "feat(scenario): foraging-selection experiment (nutrient + fertility on)

Both landscapes enabled; end-to-end test asserts the run survives and the
forage-gain metrics compute. Auto-discovered by emergence.sh/all_scenarios.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:**
- §3 new state (two `BiomeCell` fields + consts) → Task 1. ✓
- §4 generation (append noise, distinct freqs, flag-agnostic seeding) → Task 1 Step 5. ✓
- §5.1 nutrient quality in `feed_pass` (energy not biomass) → Task 3. ✓
- §5.2 fertility scales K + r; Water/Rock guard → Task 2. ✓
- §5.3 tick wiring → Task 2 Step 7. ✓
- §6 flags (`#[serde(default)]`, exact-1.0 when off) → Task 1 Steps 7–8. ✓
- §7 determinism (`FORMAT_VERSION` 18→19, golden rehash, zero tick RNG) → Task 1 Steps 11–14; flag-off bit-exactness verified in Tasks 2–3. ✓
- §8 observability (metrics + experiment scenario) → Tasks 4–5. ✓
- §9 tests: determinism/save-load (inert test + golden), ranges, fertility scaling, quality ratio, flag-off equivalence → Tasks 1–3. ✓
- §5.1 scavenging stays flat (explicit non-change) → honored (no `scavenge_pass` edit). ✓
- §10 out-of-scope (no gene, no viewer overlay) → honored. ✓

**Placeholder scan:** No TBD/TODO; every code step has concrete code. The two "verify the accessor/field names" notes point at names already used elsewhere in the crate and are guards, not deferrals.

**Type consistency:** `regrow_step(bool)`, `regrow_step_seasonal(f32, bool)`, `recolonize_step(bool)` defined in Task 2 and called with those exact arities in Task 2 Step 7. `forage_quality_gain(&World)->f32` / `forage_fertility_gain(&World)->f32` defined in Task 4, consumed in Task 5. Field names `nutrient_quality`/`fertility`/`nutrient_variation`/`soil_fertility` and consts consistent across all tasks.

**Known assumption to verify at execution time:** `Scenario` derives `Clone` (used in Task 1 Step 9) — if not, parse the TOML twice instead. Flagged inline.
