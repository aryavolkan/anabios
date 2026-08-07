# Continental Worldgen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Layer continent masses, mountain ranges, rain-shadow, and rivers onto the existing climate-driven worldgen so a world reads as a recognizable planet, and ship a large-scale (4096/512) flagship scenario for a small population.

**Architecture:** Four new geography passes inside `BiomeField::generate_with`, all controlled by new `ClimateParams` knobs that default to `0.0` (off) so every existing scenario stays behavior-identical. `BiomeCell` gains two stored fields (`elevation`, `river_flow`). Rivers are a passable moisture field, not a terrain, so no `TerrainType`-match churn. A new `continental.toml` opts everything on at large scale.

**Tech Stack:** Rust (`anabios-core`), deterministic hand-rolled gradient noise (`crate::noise`), `crate::rng::Rng`, bincode/serde snapshots, criterion (not touched here). No new dependencies.

## Global Constraints

- **Determinism is a hard contract.** RNG draw order in `generate_with` is part of it: new `Fbm::new` calls must be **appended after** the existing seven, and **skipped entirely when their knob is `0.0`** so a knobs-off world draws the identical stream.
- **`state_hash` = FNV1a over the full bincode-serialized world.** Adding `BiomeCell` fields rehashes every golden suite; regenerate with `UPDATE_HASHES=1` and paste the printed values. Golden suites known to pin hashes: `tests/determinism.rs`, `tests/inventions.rs`, `tests/cognition.rs`, `tests/trade.rs`, `tests/affect.rs`, `tests/affect_play.rs` — **run each; regen any that fail with a hash mismatch.**
- **No `#[serde(skip)]` on the new cell fields** — they vary by world and feed hashed state; skipping breaks `save→load→step` replay (the serde-skip determinism footgun).
- **Rivers stay passable** — no `carrying_capacity` change, no new `TerrainType` variant.
- **Torus everywhere** — all neighbor/offset math wraps with `rem_euclid`.
- **`BiomeCell` is not `Default`** — every struct-literal site must set the new fields. There are exactly three: `biome.rs:358` (generator), `biome.rs:1054` (test helper `grass_cell`), `crates/anabios-headless/src/record.rs:537` (test helper `cell`).
- Gate: `cargo fmt --check`, `cargo clippy`, `cargo test -p anabios-core` all green before each commit. CI runs `cargo fmt --check` on the committed tree and rustdoc `-D warnings`.
- Never `git add -A`/`.` — stage explicit paths.

---

## File Structure

- **Modify** `crates/anabios-core/src/biome.rs` — `ClimateParams` (4 new knobs), `BiomeCell` (2 new fields), `generate_with` (two-pass refactor + continent/mountain/rain-shadow), new `carve_rivers` method, new geometry constants, unit tests.
- **Modify** `crates/anabios-core/src/snapshot.rs` — bump `FORMAT_VERSION` 29 → 30, changelog line.
- **Modify** `crates/anabios-core/src/scenario.rs` — `ScenarioClimate` (4 new `Option` fields) + `resolve()`.
- **Modify** `crates/anabios-headless/src/record.rs:537` — add the 2 fields to the test helper.
- **Modify** golden suites (`tests/determinism.rs`, `tests/inventions.rs`, `tests/cognition.rs`, `tests/trade.rs`, `tests/affect.rs`, `tests/affect_play.rs`) — regenerated hash constants only.
- **Modify** `crates/anabios-core/examples/biome_scout.rs` — add a PPM (P6) dump of terrain + rivers + relief.
- **Create** `scenarios/continental.toml` — flagship 4096/512 world, small clustered cohort.
- **Modify** `docs/CHANGELOG` or the snapshot changelog comment (wherever FORMAT_VERSION history lives) — one line.

---

## Task 1: Store `elevation` + `river_flow` on `BiomeCell` (schema + FORMAT_VERSION + golden regen)

Behavior-neutral: `elevation` is already computed, now persisted; `river_flow` is `0.0` everywhere until Task 6. Existing scenarios keep identical terrain/moisture/biomass **values**; only the added bytes move the hashes.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (struct `BiomeCell` ~137, generator push ~358, test helper `grass_cell` ~1054)
- Modify: `crates/anabios-core/src/snapshot.rs:132`
- Modify: `crates/anabios-headless/src/record.rs:537`
- Modify: `crates/anabios-core/tests/{determinism,inventions,cognition,trade,affect,affect_play}.rs` (hash constants)

**Interfaces:**
- Produces: `BiomeCell.elevation: f32` (normalized `[0,1]`), `BiomeCell.river_flow: f32` (normalized `[0,1]`, `0.0` = not a river). Both `#[serde(default)]`, both hashed.

- [ ] **Step 1: Write the failing test** — add to `biome.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn elevation_is_stored_and_bounded() {
    let f = BiomeField::generate(42, 64, 1024.0);
    // Every cell has a populated elevation in range; water sits below sea level.
    for c in &f.cells {
        assert!((0.0..=1.0).contains(&c.elevation), "elev out of range: {}", c.elevation);
        assert_eq!(c.river_flow, 0.0, "river_flow is 0 until hydrology is enabled");
        if c.terrain == TerrainType::Water {
            assert!(c.elevation < SEA_LEVEL + 1e-3, "water above sea level");
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib biome::tests::elevation_is_stored_and_bounded`
Expected: FAIL — no field `elevation` on `BiomeCell`.

- [ ] **Step 3: Add the fields and populate them.**

In `BiomeCell` (after `fertility`):
```rust
    /// Normalized terrain elevation in `[0,1]` from generation (water/rock
    /// gating, temperature lapse, hydrology). Stored so the viewer can render
    /// shaded relief without recomputation. Static after generation.
    #[serde(default)]
    pub elevation: f32,
    /// Normalized river flow-accumulation in `[0,1]`; `0.0` for non-river cells.
    /// Set by the hydrology post-pass (`carve_rivers`) only when a scenario's
    /// `river_threshold > 0`. Static after generation.
    #[serde(default)]
    pub river_flow: f32,
```

In the generator push (`biome.rs:358`), add to the struct literal:
```rust
                    elevation: elev,
                    river_flow: 0.0,
```

In the `grass_cell` test helper (`biome.rs:1054`) and `record.rs:537` `cell` helper, add:
```rust
            elevation: 0.5,
            river_flow: 0.0,
```

Bump `snapshot.rs:132`: `pub const FORMAT_VERSION: u32 = 30;` and add a one-line changelog comment above it (e.g. `// 30: BiomeCell.elevation + river_flow (continental worldgen)`).

- [ ] **Step 4: Run the new test + full lib tests**

Run: `cargo test -p anabios-core --lib biome::`
Expected: PASS.

- [ ] **Step 5: Regenerate goldens.** For each golden suite, run with `UPDATE_HASHES=1`, copy the printed constants into the test, and confirm the suite then passes without the env var:

```bash
UPDATE_HASHES=1 cargo test -p anabios-core --test determinism -- --nocapture
UPDATE_HASHES=1 cargo test -p anabios-core --test inventions -- --nocapture
UPDATE_HASHES=1 cargo test -p anabios-core --test cognition -- --nocapture
UPDATE_HASHES=1 cargo test -p anabios-core --test trade -- --nocapture
UPDATE_HASHES=1 cargo test -p anabios-core --test affect -- --nocapture
UPDATE_HASHES=1 cargo test -p anabios-core --test affect_play -- --nocapture
```
Paste the printed `(tick, 0x...)` / hash values into each test file. Then:
```bash
cargo test -p anabios-core --tests
```
Expected: all PASS. (If a suite printed nothing, it doesn't pin a state hash — leave it.)

- [ ] **Step 6: Commit**

```bash
git add crates/anabios-core/src/biome.rs crates/anabios-core/src/snapshot.rs \
  crates/anabios-headless/src/record.rs \
  crates/anabios-core/tests/determinism.rs crates/anabios-core/tests/inventions.rs \
  crates/anabios-core/tests/cognition.rs crates/anabios-core/tests/trade.rs \
  crates/anabios-core/tests/affect.rs crates/anabios-core/tests/affect_play.rs
git commit -m "feat(biome): store elevation + river_flow on BiomeCell (FORMAT_VERSION 30)"
```

---

## Task 2: Add the four geography knobs to `ClimateParams` + scenario plumbing

Off by default (`0.0`), so a scenario that sets none is bit-identical. No generator behavior yet — just the knobs and their scenario wiring.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (`ClimateParams` struct ~213, `Default` impl ~227)
- Modify: `crates/anabios-core/src/scenario.rs` (`ScenarioClimate` ~178, `resolve()` ~191)

**Interfaces:**
- Produces: `ClimateParams { continentality: f32, mountain_uplift: f32, rain_shadow: f32, river_threshold: f32 }` (all `0.0` default = off). `ScenarioClimate` gains matching `Option<f32>` fields resolved in `resolve()`.

- [ ] **Step 1: Write the failing test** — in `biome.rs` tests:

```rust
#[test]
fn geography_knobs_default_off() {
    let d = ClimateParams::default();
    assert_eq!(d.continentality, 0.0);
    assert_eq!(d.mountain_uplift, 0.0);
    assert_eq!(d.rain_shadow, 0.0);
    assert_eq!(d.river_threshold, 0.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib biome::tests::geography_knobs_default_off`
Expected: FAIL — unknown fields.

- [ ] **Step 3: Add the fields.**

In `ClimateParams`:
```rust
    /// Continent shaping in `[0,1]`: 0 = today's fBm speckle; >0 pulls land into
    /// a few large masses separated by ocean.
    pub continentality: f32,
    /// Ridged mountain uplift added to elevation on land: 0 = scattered peaks;
    /// >0 raises connected linear ranges.
    pub mountain_uplift: f32,
    /// Orographic rain-shadow strength: 0 = no drying; >0 dries cells downwind
    /// of higher terrain.
    pub rain_shadow: f32,
    /// Minimum flow-accumulation (in upstream-cell units) for a cell to become a
    /// river. 0 = hydrology off (no rivers, `river_flow` stays 0).
    pub river_threshold: f32,
```
In `Default`:
```rust
            continentality: 0.0,
            mountain_uplift: 0.0,
            rain_shadow: 0.0,
            river_threshold: 0.0,
```
In `scenario.rs` `ScenarioClimate`, add four `#[serde(default)] pub <name>: Option<f32>,` fields; in `resolve()` add `<name>: self.<name>.unwrap_or(d.<name>),` for each.

- [ ] **Step 4: Run tests**

Run: `cargo test -p anabios-core --lib biome::tests::geography_knobs_default_off && cargo test -p anabios-core --lib scenario::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/biome.rs crates/anabios-core/src/scenario.rs
git commit -m "feat(biome): add continentality/mountain_uplift/rain_shadow/river_threshold knobs (off by default)"
```

---

## Task 3: Continent mask (two-pass refactor of `generate_with`)

Refactor `generate_with` into **pass 1 (elevation grid)** and **pass 2 (climate + classify + build cells)**. Splitting the sampling loops is RNG-neutral (RNG is consumed only by the `Fbm::new` constructions at the top), so with `continentality = 0` the world is byte-identical to Task 1's. When `> 0`, a low-frequency continent mask pulls land into masses.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (`generate_with` ~311, new constant `DEEP_OCEAN_ELEV`)

**Interfaces:**
- Consumes: `ClimateParams.continentality` (Task 2), `crate::noise::Fbm::new(&mut rng, base_period, octaves, lacunarity, persistence)`, `Fbm::sample(u, v)`.
- Produces: an `elev_grid: Vec<f32>` computed in pass 1 (used by later tasks for mountain uplift and rain-shadow neighbor lookups).

- [ ] **Step 1: Write the failing test** — in `biome.rs` tests:

```rust
/// Count connected land components (4-neighbour, torus) as a speckle metric.
fn land_component_count(f: &BiomeField) -> usize {
    let res = f.res;
    let mut seen = vec![false; f.cells.len()];
    let is_land = |i: usize| f.cells[i].terrain != TerrainType::Water;
    let mut comps = 0;
    let mut stack = Vec::new();
    for start in 0..f.cells.len() {
        if seen[start] || !is_land(start) { continue; }
        comps += 1;
        stack.push(start);
        seen[start] = true;
        while let Some(i) = stack.pop() {
            let (col, row) = (i % res, i / res);
            for (dc, dr) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let nc = (col as i32 + dc).rem_euclid(res as i32) as usize;
                let nr = (row as i32 + dr).rem_euclid(res as i32) as usize;
                let ni = nr * res + nc;
                if !seen[ni] && is_land(ni) { seen[ni] = true; stack.push(ni); }
            }
        }
    }
    comps
}

#[test]
fn continentality_reduces_land_fragmentation() {
    let plain = BiomeField::generate(7, 128, 1024.0);
    let cont = {
        let mut c = ClimateParams::default();
        c.continentality = 0.85;
        BiomeField::generate_with(7, 128, 1024.0, &c)
    };
    assert!(
        land_component_count(&cont) < land_component_count(&plain),
        "continentality should consolidate land: plain={} cont={}",
        land_component_count(&plain), land_component_count(&cont)
    );
}

#[test]
fn continentality_zero_is_identity() {
    // Two-pass refactor must not change values when the knob is off.
    let a = BiomeField::generate(7, 96, 1024.0);
    let b = BiomeField::generate_with(7, 96, 1024.0, &ClimateParams::default());
    for (x, y) in a.cells.iter().zip(b.cells.iter()) {
        assert_eq!(x.terrain, y.terrain);
        assert_eq!(x.elevation, y.elevation);
        assert_eq!(x.moisture, y.moisture);
        assert_eq!(x.env, y.env);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib biome::tests::continentality_reduces_land_fragmentation`
Expected: FAIL — continentality has no effect yet.

- [ ] **Step 3: Refactor `generate_with` to two passes with the continent mask.**

Add the constant near the other geometry consts:
```rust
/// Elevation that ocean basins are pulled down toward under continent shaping.
pub const DEEP_OCEAN_ELEV: f32 = 0.15;
```
Append the gated continent noise after the existing seven `Fbm` constructions:
```rust
        // Geography knobs draw AFTER the base seven and only when active, so a
        // knobs-off world keeps the exact pre-change RNG stream (goldens hold).
        let continent_noise =
            (climate.continentality > 0.0).then(|| crate::noise::Fbm::new(&mut rng, 2, 3, 2, 0.5));
```
Replace the single cell loop with two passes. Pass 1 builds `elev_grid`:
```rust
        let mut elev_grid = vec![0.0f32; res * res];
        for row in 0..res {
            for col in 0..res {
                let u = col as f32 / res as f32;
                let v = row as f32 / res as f32;
                let (wu, wv) = crate::noise::warp(&warp_x, &warp_y, u, v, WARP_AMP);
                let mut elev = ((elevation.sample(wu, wv) - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
                if let Some(cn) = &continent_noise {
                    let mask = cn.sample(wu, wv);
                    let blend = 1.0 - climate.continentality + climate.continentality * mask;
                    elev = (DEEP_OCEAN_ELEV + (elev - DEEP_OCEAN_ELEV) * blend).clamp(0.0, 1.0);
                }
                elev_grid[row * res + col] = elev;
            }
        }
```
Pass 2 builds the cells, reading `elev` from `elev_grid` (temperature/moisture math and the `cells.push` are the SAME as before, only `elev` now comes from the grid and `elevation: elev` is set):
```rust
        let mut cells = Vec::with_capacity(res * res);
        for row in 0..res {
            for col in 0..res {
                let u = col as f32 / res as f32;
                let v = row as f32 / res as f32;
                let (wu, wv) = crate::noise::warp(&warp_x, &warp_y, u, v, WARP_AMP);
                let elev = elev_grid[row * res + col];
                let temperature = (latitude_temp(v)
                    - TEMP_LAPSE * (elev - climate.sea_level).max(0.0)
                    + TEMP_NOISE * (temp_noise.sample(u, v) - 0.5)
                    + climate.temp_bias)
                    .clamp(0.0, 1.0);
                let moisture = (0.5 * latitude_moisture(v)
                    + 0.5 * moisture_noise.sample(wu, wv)
                    + climate.moisture_bias)
                    .clamp(0.0, 1.0);
                let terrain = classify_with(elev, temperature, moisture, climate.sea_level);
                let nq = nutrient.sample(u, v);
                let nutrient_quality =
                    NUTRIENT_QUALITY_MIN + (NUTRIENT_QUALITY_MAX - NUTRIENT_QUALITY_MIN) * nq;
                let fe = fertility_noise.sample(u, v);
                let fertility = FERTILITY_MIN + (FERTILITY_MAX - FERTILITY_MIN) * fe;
                cells.push(BiomeCell {
                    terrain,
                    plant_biomass: terrain.carrying_capacity(),
                    env: temperature,
                    moisture,
                    pollution: 0.0,
                    succession: SUCCESSION_CLIMAX,
                    nutrient_quality,
                    fertility,
                    elevation: elev,
                    river_flow: 0.0,
                });
            }
        }
```
Keep the `Self { cells, res, world_size, cell_size: world_size / res as f32, recolonize_scratch: Vec::new() }` return.

- [ ] **Step 4: Run tests**

Run: `cargo test -p anabios-core --lib biome:: && cargo test -p anabios-core --test determinism`
Expected: PASS — including `continentality_zero_is_identity` (proves the refactor is byte-neutral) and the determinism golden (unchanged, since default ClimateParams still generates identical values).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/biome.rs
git commit -m "feat(biome): continent mask pulls land into masses (two-pass generate)"
```

---

## Task 4: Mountain ranges (ridged uplift in pass 1)

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (`generate_with` pass 1)

**Interfaces:**
- Consumes: `ClimateParams.mountain_uplift`, `elev_grid` + `continent_noise` from Task 3.
- Produces: elevated ridge lines that cross `ROCK_LINE` on land.

- [ ] **Step 1: Write the failing test:**

```rust
#[test]
fn mountain_uplift_raises_connected_ranges() {
    let flat = {
        let mut c = ClimateParams::default();
        c.continentality = 0.85;
        BiomeField::generate_with(11, 128, 1024.0, &c)
    };
    let ranged = {
        let mut c = ClimateParams::default();
        c.continentality = 0.85;
        c.mountain_uplift = 0.6;
        BiomeField::generate_with(11, 128, 1024.0, &c)
    };
    let rock = |f: &BiomeField| f.cells.iter().filter(|c| c.terrain == TerrainType::Rock).count();
    assert!(rock(&ranged) > rock(&flat), "uplift should create more rock: {} vs {}", rock(&ranged), rock(&flat));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib biome::tests::mountain_uplift_raises_connected_ranges`
Expected: FAIL — mountain_uplift has no effect.

- [ ] **Step 3: Add the gated mountain noise + uplift.**

After the `continent_noise` line:
```rust
        let mountain_noise =
            (climate.mountain_uplift > 0.0).then(|| crate::noise::Fbm::new(&mut rng, 3, 4, 2, 0.5));
```
In pass 1, after the continent-mask block and before storing `elev_grid`:
```rust
                if let Some(mn) = &mountain_noise {
                    let ridge = 1.0 - (2.0 * mn.sample(wu, wv) - 1.0).abs();
                    // Weight uplift to land interiors so ranges sit on continents.
                    let land_weight = continent_noise.as_ref().map_or(1.0, |cn| cn.sample(wu, wv));
                    elev = (elev + climate.mountain_uplift * ridge * land_weight).clamp(0.0, 1.0);
                }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p anabios-core --lib biome:: && cargo test -p anabios-core --test determinism`
Expected: PASS (determinism golden unchanged — default has `mountain_uplift = 0`).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/biome.rs
git commit -m "feat(biome): ridged mountain uplift builds linear ranges on land"
```

---

## Task 5: Rain-shadow (orographic drying in pass 2)

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (`generate_with` pass 2, new wind constants)

**Interfaces:**
- Consumes: `ClimateParams.rain_shadow`, `elev_grid` from Task 3.
- Produces: reduced moisture on cells downwind of higher terrain.

- [ ] **Step 1: Write the failing test:**

```rust
#[test]
fn rain_shadow_dries_lee_of_ranges() {
    // Compare mean moisture with and without rain-shadow on a mountainous world;
    // the shadowed world must be drier on average over land.
    let mk = |rs: f32| {
        let mut c = ClimateParams::default();
        c.continentality = 0.85;
        c.mountain_uplift = 0.6;
        c.rain_shadow = rs;
        BiomeField::generate_with(23, 128, 1024.0, &c)
    };
    let mean_land_moisture = |f: &BiomeField| {
        let land: Vec<f32> = f.cells.iter()
            .filter(|c| c.terrain != TerrainType::Water)
            .map(|c| c.moisture).collect();
        land.iter().sum::<f32>() / land.len() as f32
    };
    assert!(
        mean_land_moisture(&mk(0.5)) < mean_land_moisture(&mk(0.0)),
        "rain-shadow should lower mean land moisture"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib biome::tests::rain_shadow_dries_lee_of_ranges`
Expected: FAIL — rain_shadow has no effect.

- [ ] **Step 3: Add wind constants + the moisture reduction.**

Near the other geometry consts:
```rust
/// Prevailing wind (westerly): upwind is `-WIND_DX/-WIND_DY` cells away.
pub const WIND_DX: isize = 1;
pub const WIND_DY: isize = 0;
/// Upwind sample distance (cells) for the rain-shadow term.
pub const SHADOW_DIST: isize = 4;
```
In pass 2, replace the `let moisture = ...` binding with a `let mut moisture = ...` and, immediately after it, before `classify_with`:
```rust
                if climate.rain_shadow > 0.0 {
                    let uc = (col as isize - WIND_DX * SHADOW_DIST).rem_euclid(res as isize) as usize;
                    let ur = (row as isize - WIND_DY * SHADOW_DIST).rem_euclid(res as isize) as usize;
                    let upwind_elev = elev_grid[ur * res + uc];
                    moisture = (moisture - climate.rain_shadow * (upwind_elev - elev).max(0.0))
                        .clamp(0.0, 1.0);
                }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p anabios-core --lib biome:: && cargo test -p anabios-core --test determinism`
Expected: PASS (default has `rain_shadow = 0`, so `continentality_zero_is_identity` and goldens hold).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/biome.rs
git commit -m "feat(biome): orographic rain-shadow dries lee slopes"
```

---

## Task 6: Rivers (hydrology post-pass)

Deterministic flow accumulation over the finished elevation field, no RNG. Marks river cells (`river_flow`), boosts riparian moisture, and reclassifies affected cells.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (new `carve_rivers` method + call site at the end of `generate_with`, new `RIPARIAN_MOISTURE` const)

**Interfaces:**
- Consumes: `ClimateParams.river_threshold`, `BiomeCell.elevation`/`env`/`moisture`, `classify_with`.
- Produces: `BiomeCell.river_flow > 0` on river cells; wetter, reclassified banks.

- [ ] **Step 1: Write the failing test:**

```rust
#[test]
fn rivers_flow_downhill_to_water_and_wet_banks() {
    let mut c = ClimateParams::default();
    c.continentality = 0.85;
    c.mountain_uplift = 0.6;
    c.river_threshold = 150.0;
    let f = BiomeField::generate_with(31, 128, 1024.0, &c);
    let river_cells: Vec<usize> =
        (0..f.cells.len()).filter(|&i| f.cells[i].river_flow > 0.0).collect();
    assert!(!river_cells.is_empty(), "expected some river cells");
    // Every river cell can descend (8-neighbour) to a strictly-lower cell or is
    // adjacent to water (a mouth / sink terminus).
    let res = f.res;
    for &i in &river_cells {
        let (col, row) = (i % res, i / res);
        let e = f.cells[i].elevation;
        let mut ok = false;
        for (dc, dr) in [(-1i32,-1i32),(0,-1),(1,-1),(-1,0),(1,0),(-1,1),(0,1),(1,1)] {
            let nc = (col as i32 + dc).rem_euclid(res as i32) as usize;
            let nr = (row as i32 + dr).rem_euclid(res as i32) as usize;
            let n = &f.cells[nr * res + nc];
            if n.elevation < e || n.terrain == TerrainType::Water { ok = true; break; }
        }
        assert!(ok, "river cell {i} has no downhill exit and no water neighbour");
    }
}

#[test]
fn river_threshold_zero_leaves_flow_empty() {
    let mut c = ClimateParams::default();
    c.continentality = 0.85;
    let f = BiomeField::generate_with(31, 96, 1024.0, &c);
    assert!(f.cells.iter().all(|c| c.river_flow == 0.0));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p anabios-core --lib biome::tests::rivers_flow_downhill_to_water_and_wet_banks`
Expected: FAIL — no `carve_rivers`, `river_flow` all 0.

- [ ] **Step 3: Implement `carve_rivers` and call it.**

Add the constant:
```rust
/// Moisture added to river cells and their 4-neighbours (riparian greening).
pub const RIPARIAN_MOISTURE: f32 = 0.25;
```
Add the method inside `impl BiomeField`:
```rust
    /// Flow-accumulation hydrology over the finished elevation field (no RNG).
    /// Downhill routing (8-neighbour steepest descent, torus), accumulation in
    /// descending-elevation order (stable index tie-break), river cells above
    /// `threshold`, then a riparian moisture bump + reclassify on banks.
    fn carve_rivers(&mut self, threshold: f32, sea_level: f32) {
        let res = self.res;
        let n = self.cells.len();
        // 1. steepest-descent downhill neighbour (usize::MAX = sink or water).
        let mut downhill = vec![usize::MAX; n];
        for row in 0..res {
            for col in 0..res {
                let i = row * res + col;
                if self.cells[i].terrain == TerrainType::Water {
                    continue;
                }
                let e = self.cells[i].elevation;
                let (mut best, mut best_idx) = (e, usize::MAX);
                for (dc, dr) in [(-1i32,-1i32),(0,-1),(1,-1),(-1,0),(1,0),(-1,1),(0,1),(1,1)] {
                    let nc = (col as i32 + dc).rem_euclid(res as i32) as usize;
                    let nr = (row as i32 + dr).rem_euclid(res as i32) as usize;
                    let ni = nr * res + nc;
                    let ne = self.cells[ni].elevation;
                    if ne < best {
                        best = ne;
                        best_idx = ni;
                    }
                }
                downhill[i] = best_idx;
            }
        }
        // 2. accumulate in descending elevation order (index tie-break).
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| {
            self.cells[b].elevation
                .partial_cmp(&self.cells[a].elevation)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.cmp(&b))
        });
        let mut accum = vec![1.0f32; n];
        for &i in &order {
            let d = downhill[i];
            if d != usize::MAX {
                accum[d] += accum[i];
            }
        }
        // 3. mark river cells + normalized flow.
        let max_accum = accum.iter().cloned().fold(1.0f32, f32::max);
        let mut river = vec![false; n];
        for i in 0..n {
            if self.cells[i].terrain != TerrainType::Water && accum[i] >= threshold {
                river[i] = true;
                self.cells[i].river_flow = (accum[i] / max_accum).clamp(0.0, 1.0);
            }
        }
        // 4. riparian moisture bump + reclassify on river cells & 4-neighbours.
        for row in 0..res {
            for col in 0..res {
                let i = row * res + col;
                if self.cells[i].terrain == TerrainType::Water {
                    continue;
                }
                let near = river[i] || [(-1i32,0i32),(1,0),(0,-1),(0,1)].iter().any(|&(dc, dr)| {
                    let nc = (col as i32 + dc).rem_euclid(res as i32) as usize;
                    let nr = (row as i32 + dr).rem_euclid(res as i32) as usize;
                    river[nr * res + nc]
                });
                if near {
                    let c = &mut self.cells[i];
                    c.moisture = (c.moisture + RIPARIAN_MOISTURE).clamp(0.0, 1.0);
                    c.terrain = classify_with(c.elevation, c.env, c.moisture, sea_level);
                    c.plant_biomass = c.terrain.carrying_capacity();
                }
            }
        }
    }
```
At the very end of `generate_with`, replace the bare `Self { ... }` return with:
```rust
        let mut field = Self {
            cells,
            res,
            world_size,
            cell_size: world_size / res as f32,
            recolonize_scratch: Vec::new(),
        };
        if climate.river_threshold > 0.0 {
            field.carve_rivers(climate.river_threshold, climate.sea_level);
        }
        field
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p anabios-core --lib biome:: && cargo test -p anabios-core --test determinism`
Expected: PASS (default `river_threshold = 0` → `carve_rivers` skipped → goldens hold).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/biome.rs
git commit -m "feat(biome): river flow-accumulation carves passable moisture corridors"
```

---

## Task 7: Cross-seed guarantee test

One integration test asserting a continental config produces all three features across a sample of seeds — the regression net for the whole feature.

**Files:**
- Create: `crates/anabios-core/tests/continental_worldgen.rs`

**Interfaces:**
- Consumes: `BiomeField::generate_with`, `ClimateParams`, `TerrainType`.

- [ ] **Step 1: Write the test:**

```rust
use anabios_core::biome::{BiomeField, ClimateParams, TerrainType};

fn continental() -> ClimateParams {
    let mut c = ClimateParams::default();
    c.continentality = 0.85;
    c.mountain_uplift = 0.6;
    c.rain_shadow = 0.4;
    c.river_threshold = 150.0;
    c
}

#[test]
fn every_continental_seed_has_land_mountains_and_rivers() {
    let cfg = continental();
    for seed in 0..8u64 {
        let f = BiomeField::generate_with(seed, 256, 4096.0, &cfg);
        let land = f.cells.iter().filter(|c| c.terrain != TerrainType::Water).count();
        let rock = f.cells.iter().filter(|c| c.terrain == TerrainType::Rock).count();
        let rivers = f.cells.iter().filter(|c| c.river_flow > 0.0).count();
        assert!(land > f.cells.len() / 10, "seed {seed}: too little land ({land})");
        assert!(rock > 0, "seed {seed}: no mountains");
        assert!(rivers > 0, "seed {seed}: no rivers");
    }
}

#[test]
fn continental_generation_is_deterministic() {
    let cfg = continental();
    let a = BiomeField::generate_with(3, 256, 4096.0, &cfg);
    let b = BiomeField::generate_with(3, 256, 4096.0, &cfg);
    for (x, y) in a.cells.iter().zip(b.cells.iter()) {
        assert_eq!(x.terrain, y.terrain);
        assert_eq!(x.elevation, y.elevation);
        assert_eq!(x.river_flow, y.river_flow);
        assert_eq!(x.moisture, y.moisture);
    }
}
```

- [ ] **Step 2: Run to verify it passes**

Run: `cargo test -p anabios-core --test continental_worldgen`
Expected: PASS. (If a seed lacks rivers, lower `river_threshold` in `continental()` and note it — the threshold is scale-dependent.)

- [ ] **Step 3: Commit**

```bash
git add crates/anabios-core/tests/continental_worldgen.rs
git commit -m "test(biome): cross-seed guarantee — continents, mountains, rivers"
```

---

## Task 8: PPM scout dump for eyeballing worlds

Extend `biome_scout` so it writes a viewable `P6` PPM (dependency-free) coloring terrain, tinting river cells blue, and shading by elevation — the visual probe for tuning knobs.

**Files:**
- Modify: `crates/anabios-core/examples/biome_scout.rs`

**Interfaces:**
- Consumes: `BiomeField::generate_with`, `BiomeCell.{terrain,elevation,river_flow}`.

- [ ] **Step 1: Add a `write_ppm` helper + a `--png`/`--ppm` path.** Add near the top of `biome_scout.rs`:

```rust
use std::io::Write;

fn terrain_rgb(t: anabios_core::biome::TerrainType) -> (u8, u8, u8) {
    use anabios_core::biome::TerrainType::*;
    match t {
        Water => (23, 49, 112), Grass => (54, 112, 48), Forest => (18, 66, 28),
        Desert => (173, 148, 84), Rock => (107, 102, 115), Savanna => (184, 168, 92),
        Rainforest => (15, 87, 41), Taiga => (41, 87, 66), Tundra => (158, 168, 158),
    }
}

/// Write a P6 PPM: terrain color, dimmed/brightened by elevation, rivers blue.
fn write_ppm(path: &str, f: &anabios_core::biome::BiomeField) -> std::io::Result<()> {
    let res = f.res;
    let mut buf = Vec::with_capacity(res * res * 3 + 32);
    write!(buf, "P6\n{res} {res}\n255\n")?;
    for c in &f.cells {
        let (mut r, mut g, mut b) = terrain_rgb(c.terrain);
        // Relief shading: scale by 0.6..1.15 with elevation.
        let s = 0.6 + 0.55 * c.elevation;
        r = (r as f32 * s).min(255.0) as u8;
        g = (g as f32 * s).min(255.0) as u8;
        b = (b as f32 * s).min(255.0) as u8;
        if c.river_flow > 0.0 { r = 40; g = 90; b = 200; }
        buf.extend_from_slice(&[r, g, b]);
    }
    std::fs::write(path, buf)
}
```

- [ ] **Step 2: Wire it into `main`.** When invoked with a `--ppm <path>` arg (and optional `--seed N`), generate a continental world and dump it:

```rust
    // In main(), after arg parsing:
    if let Some(pos) = std::env::args().position(|a| a == "--ppm") {
        let path = std::env::args().nth(pos + 1).unwrap_or_else(|| "world.ppm".into());
        let seed = /* reuse the example's existing seed arg, else */ 7u64;
        let mut c = anabios_core::biome::ClimateParams::default();
        c.continentality = 0.85; c.mountain_uplift = 0.6; c.rain_shadow = 0.4; c.river_threshold = 150.0;
        let f = anabios_core::biome::BiomeField::generate_with(seed, 512, 4096.0, &c);
        write_ppm(&path, &f).expect("write ppm");
        eprintln!("wrote {path}");
        return;
    }
```
(Adapt to the example's actual `main` shape — keep its existing scoring path intact; this is an added branch.)

- [ ] **Step 3: Run it**

Run: `cargo run -p anabios-core --example biome_scout -- --ppm /tmp/world.ppm --seed 7`
Expected: prints `wrote /tmp/world.ppm`; open the file (Preview/any image viewer) and confirm continents, ranges, and a river network are visible. Tune the `continental()` knob values here until the world looks right; carry the chosen values into Task 9.

- [ ] **Step 4: Commit**

```bash
git add crates/anabios-core/examples/biome_scout.rs
git commit -m "feat(scout): PPM dump of terrain + rivers + relief for worldgen tuning"
```

---

## Task 9: Flagship `continental.toml` scenario + smoke test

**Files:**
- Create: `scenarios/continental.toml`
- Modify: `crates/anabios-core/tests/continental_worldgen.rs` (add a scenario smoke test)

**Interfaces:**
- Consumes: the scenario loader (`world_size`/`biome_res`/`hash_res` + `[climate]` knobs from Task 2), `Placement::Cluster`.

- [ ] **Step 1: Scout a land start.** Run the Task 8 scout (`--ppm`) on the chosen seed and read off a river-adjacent, high-carrying-capacity cluster center (pixel `(col,row)` × `cell_size 8` → world `(x,y)`), exactly as `desert-tropical.toml`'s header documents its centers.

- [ ] **Step 2: Write `scenarios/continental.toml`** (fill `center_x`/`center_y` and the tuned knob values from scouting; keep `world_size / hash_res ≈ 16`):

```toml
# A continent-scale world: a small founder cohort seeded on one landmass beside
# a river, dwarfed by oceans, mountain ranges, and a river network.
#
# Geography knobs (see docs/superpowers/specs/2026-08-07-continental-worldgen-design.md):
#   continentality  - pulls land into a few large masses
#   mountain_uplift - ridged linear ranges on land
#   rain_shadow     - dry lee slopes downwind of ranges
#   river_threshold - min flow-accumulation for a river cell (scale-dependent)
name = "continental"
seed = 7
world_size = 4096.0
biome_res = 512
hash_res = 256
# Small population, large world.
max_population = 800
living_biome = true
terrain_habitat = true

[climate]
continentality = 0.85
mountain_uplift = 0.6
rain_shadow = 0.4
river_threshold = 150.0

# Founder cohort clustered on a river-adjacent grassland (center from the scout).
[[agents]]
count = 120
placement = { kind = "cluster", center_x = 0.0, center_y = 0.0, radius = 120.0 }
[agents.traits]
lifespan_bias = 0.6
reproduction_threshold = 0.5
```

- [ ] **Step 3: Add a scenario smoke test** to `tests/continental_worldgen.rs`:

Add these imports at the top of the file (alongside the existing `use anabios_core::...` lines):
```rust
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;
```
Then the test (verified API: `Scenario::parse_toml(&str)` → `.instantiate()` → free-fn `step(&mut World)` / `state_hash(&World)`):
```rust
#[test]
fn continental_scenario_loads_and_runs_deterministically() {
    let toml = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/continental.toml"),
    ).expect("read continental.toml");
    let scenario = Scenario::parse_toml(&toml).expect("parse");
    let mut a = scenario.instantiate();
    let mut b = scenario.instantiate();
    for _ in 0..200 {
        step(&mut a);
        step(&mut b);
    }
    assert_eq!(state_hash(&a), state_hash(&b), "scenario must be deterministic");
    assert!(a.agents.iter_alive().count() > 0, "population should survive 200 ticks");
}
```
(The `world_size`/`biome_res`/`hash_res` keys are already parsed as `Option`s per `scenario.rs:153-157`; the `[climate]` table maps to `ScenarioClimate` from Task 2.)

- [ ] **Step 4: Run it**

Run: `cargo test -p anabios-core --test continental_worldgen`
Expected: PASS. If the cohort dies out, move the cluster center to a denser grassland (re-scout) or raise `count`.

- [ ] **Step 5: Full gate**

Run:
```bash
cargo fmt --check && cargo clippy -p anabios-core --tests -- -D warnings && cargo test -p anabios-core --tests
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add scenarios/continental.toml crates/anabios-core/tests/continental_worldgen.rs
git commit -m "feat(scenario): continental.toml — large world, small founder cohort"
```

---

## Self-Review Notes (for the executor)

- **Spec coverage:** continent mask (T3), mountains (T4), rain-shadow (T5), rivers (T6), `elevation`+`river_flow` schema + FORMAT_VERSION + golden regen (T1), knobs + scenario plumbing (T2), cross-seed guarantee + determinism (T7), PPM scout (T8), flagship 4096/512 scenario (T9). All spec §sections map to a task.
- **Determinism invariant:** every feature is gated `if knob > 0.0` and its `Fbm::new` draw is skipped when off, so Tasks 3–6 leave the default-config goldens byte-identical — only Task 1 (schema) regenerates them. `continentality_zero_is_identity` (T3) is the tripwire if any refactor drifts a float.
- **Verified APIs** (already confirmed against the tree): scenario load is `Scenario::parse_toml(&str) -> Result<Scenario, _>` then `.instantiate() -> World` (`scenario.rs:461,492`); stepping and hashing are **free functions** `anabios_core::tick::step(&mut World)` and `anabios_core::snapshot::state_hash(&World) -> u64`, imported as `use anabios_core::tick::step;` / `use anabios_core::snapshot::state_hash;` (see `tests/affect.rs`, `tests/determinism.rs`). The one thing still to adapt by reading the file: the `biome_scout.rs` `main` shape (its arg parsing is `std::env::args().nth(1)` for the scan count; add the `--ppm` branch without disturbing that).
- **Scale-dependent constant:** `river_threshold = 150.0` is a starting guess at 512-res; tune via the T8 scout and reflect the final value in both `continental()` (T7) and `continental.toml` (T9).
