# Climate-driven Procedural World Generation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the elevation-only terrain generator with a climate-driven Whittaker pipeline (gradient/fBm noise + domain warping + latitude-band temperature & moisture) so every world contains subtropical deserts and an equatorial rainforest belt by construction.

**Architecture:** A new dependency-free `noise` module provides tileable gradient noise, fBm, and domain warping. `BiomeField::generate` computes per-cell elevation, temperature, and moisture, then classifies terrain via a hard-threshold Whittaker matrix. `TerrainType` gains Savanna/Rainforest/Taiga/Tundra; `BiomeCell` gains `moisture` and repurposes `env` to mean temperature.

**Tech Stack:** Rust (`anabios-core`), existing `Rng` (no new deps), bincode snapshots, `cargo test`/`fmt`/`clippy`.

## Global Constraints

- Determinism is a hard contract. `state_hash` = FNV1a over the full bincode-serialized world; RNG draw order in `generate` is part of the contract — fix it and never reorder casually.
- No new crate dependencies. All noise is hand-rolled on the existing `crate::rng::Rng`.
- New `BiomeCell`/enum data that feeds hashed state must NOT be `#[serde(skip)]` (breaks replay — the serde-skip determinism footgun).
- Adding enum variants: append only, preserve existing discriminants (Water=0, Grass=1, Forest=2, Desert=3, Rock=4).
- CI gate to match locally before pushing: `cargo fmt --check`, `cargo clippy`, rustdoc `-D warnings`, `cargo test`.
- Terrain productivity ordering (carrying_capacity): Rainforest 28 > Forest 20 > Taiga 12 > Grass 10 > Savanna 8 > Tundra 4 > Desert 3 > Water/Rock 0. Existing Water/Grass/Forest/Desert/Rock values unchanged.
- Constants (initial, tunable): SEA_LEVEL 0.40, ROCK_LINE 0.82, LAPSE 0.55, TEMP_NOISE 0.10, WARP_AMP 0.35, OCTAVES 5, lacunarity 2, persistence 0.5, band cutoffs 0.33/0.66.
- Extends PR #74 on branch `worktree-desert-tropical-world`. Commit after each task.

---

### Task 1: Tileable gradient noise + fBm + domain warp (`noise` module)

**Files:**
- Create: `crates/anabios-core/src/noise.rs`
- Modify: `crates/anabios-core/src/lib.rs` (add `mod noise;` near the other `mod` lines, ~line 10)
- Test: inline `#[cfg(test)]` in `noise.rs`

**Interfaces:**
- Consumes: `crate::rng::Rng` (`Rng::from_seed(u64)`, `rng.f32_range(a,b)`, `rng.f32_unit()`).
- Produces:
  - `pub struct GradientNoise` with `pub fn new(rng: &mut Rng, period: usize) -> Self` and `pub fn sample(&self, u: f32, v: f32) -> f32` (returns `[0,1]`, tileable with period 1.0 in u and v).
  - `pub struct Fbm { }` with `pub fn new(rng: &mut Rng, base_period: usize, octaves: usize, lacunarity: usize, persistence: f32) -> Self` and `pub fn sample(&self, u: f32, v: f32) -> f32` (returns `[0,1]`).
  - `pub fn warp(fx: &Fbm, fy: &Fbm, u: f32, v: f32, amp: f32) -> (f32, f32)`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/anabios-core/src/noise.rs  (append at bottom, in #[cfg(test)] mod tests)
#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn gradient_noise_is_bounded_and_varies() {
        let mut rng = Rng::from_seed(1);
        let n = GradientNoise::new(&mut rng, 8);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..64 {
            for j in 0..64 {
                let s = n.sample(i as f32 / 64.0, j as f32 / 64.0);
                assert!((0.0..=1.0).contains(&s), "out of range: {s}");
                lo = lo.min(s);
                hi = hi.max(s);
            }
        }
        assert!(hi - lo > 0.3, "field too flat: {lo}..{hi}");
    }

    #[test]
    fn gradient_noise_is_seamless_on_torus() {
        let mut rng = Rng::from_seed(2);
        let n = GradientNoise::new(&mut rng, 8);
        for k in 0..16 {
            let t = k as f32 / 16.0;
            assert!((n.sample(0.0, t) - n.sample(1.0, t)).abs() < 1e-5, "u seam at {t}");
            assert!((n.sample(t, 0.0) - n.sample(t, 1.0)).abs() < 1e-5, "v seam at {t}");
        }
    }

    #[test]
    fn noise_is_deterministic() {
        let a = { let mut r = Rng::from_seed(7); GradientNoise::new(&mut r, 8).sample(0.3, 0.6) };
        let b = { let mut r = Rng::from_seed(7); GradientNoise::new(&mut r, 8).sample(0.3, 0.6) };
        assert_eq!(a, b);
    }

    #[test]
    fn fbm_is_bounded_and_varies() {
        let mut rng = Rng::from_seed(3);
        let f = Fbm::new(&mut rng, 4, 5, 2, 0.5);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..64 {
            let s = f.sample(i as f32 / 64.0, 0.5);
            assert!((0.0..=1.0).contains(&s));
            lo = lo.min(s);
            hi = hi.max(s);
        }
        assert!(hi - lo > 0.2, "fbm too flat: {lo}..{hi}");
    }

    #[test]
    fn warp_offsets_coordinates_deterministically() {
        let mut rng = Rng::from_seed(9);
        let fx = Fbm::new(&mut rng, 4, 3, 2, 0.5);
        let fy = Fbm::new(&mut rng, 4, 3, 2, 0.5);
        let (u, v) = warp(&fx, &fy, 0.5, 0.5, 0.3);
        assert!((u - 0.5).abs() > 1e-6 || (v - 0.5).abs() > 1e-6, "warp did nothing");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p anabios-core --lib noise 2>&1 | tail -20`
Expected: FAIL — `noise` module / `GradientNoise` not found.

- [ ] **Step 3: Write the implementation**

```rust
// crates/anabios-core/src/noise.rs  (top of file)
//! Dependency-free, deterministic, torus-tileable gradient noise (Perlin
//! construction) with fBm and domain warping, for procedural world generation.
//! All randomness comes from `crate::rng::Rng`; RNG draw order is part of the
//! determinism contract.

use crate::rng::Rng;

#[inline]
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Perlin-style gradient noise on a `period x period` corner grid. Corners wrap
/// modulo `period`, so the field is seamless across the unit torus.
pub struct GradientNoise {
    period: usize,
    grad: Vec<(f32, f32)>,
}

impl GradientNoise {
    /// Draw one unit gradient per corner. Draw order (row-major) is part of the
    /// determinism contract.
    pub fn new(rng: &mut Rng, period: usize) -> Self {
        let mut grad = Vec::with_capacity(period * period);
        for _ in 0..period * period {
            let angle = rng.f32_range(0.0, std::f32::consts::TAU);
            grad.push((angle.cos(), angle.sin()));
        }
        Self { period, grad }
    }

    #[inline]
    fn grad_at(&self, cx: usize, cy: usize) -> (f32, f32) {
        self.grad[cy * self.period + cx]
    }

    /// Sample at `(u, v)`; inputs are wrapped into `[0,1)`, output remapped to
    /// `[0,1]`. Seamless: `sample(0,v) == sample(1,v)`.
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let p = self.period as f32;
        let x = u.rem_euclid(1.0) * p;
        let y = v.rem_euclid(1.0) * p;
        let x0 = (x.floor() as usize) % self.period;
        let y0 = (y.floor() as usize) % self.period;
        let x1 = (x0 + 1) % self.period;
        let y1 = (y0 + 1) % self.period;
        let fx = x - x.floor();
        let fy = y - y.floor();
        let dot = |cx: usize, cy: usize, dx: f32, dy: f32| {
            let (gx, gy) = self.grad_at(cx, cy);
            gx * dx + gy * dy
        };
        let n00 = dot(x0, y0, fx, fy);
        let n10 = dot(x1, y0, fx - 1.0, fy);
        let n01 = dot(x0, y1, fx, fy - 1.0);
        let n11 = dot(x1, y1, fx - 1.0, fy - 1.0);
        let sx = smoothstep(fx);
        let sy = smoothstep(fy);
        let nx0 = lerp(n00, n10, sx);
        let nx1 = lerp(n01, n11, sx);
        let n = lerp(nx0, nx1, sy); // ~[-0.7, 0.7]
        (n * 0.7 + 0.5).clamp(0.0, 1.0)
    }
}

/// Fractal Brownian motion: sum of gradient-noise octaves at increasing period
/// (frequency) and decreasing amplitude. Each octave is individually tileable,
/// so the sum is too.
pub struct Fbm {
    layers: Vec<(GradientNoise, f32)>, // (octave, amplitude)
    norm: f32,
}

impl Fbm {
    pub fn new(
        rng: &mut Rng,
        base_period: usize,
        octaves: usize,
        lacunarity: usize,
        persistence: f32,
    ) -> Self {
        let mut layers = Vec::with_capacity(octaves);
        let mut period = base_period;
        let mut amp = 1.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            layers.push((GradientNoise::new(rng, period), amp));
            norm += amp;
            period *= lacunarity;
            amp *= persistence;
        }
        Self { layers, norm }
    }

    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let mut acc = 0.0;
        for (noise, amp) in &self.layers {
            acc += noise.sample(u, v) * amp;
        }
        (acc / self.norm).clamp(0.0, 1.0)
    }
}

/// Inigo-Quilez domain warp: offset `(u,v)` by two fBm fields centered on 0.
pub fn warp(fx: &Fbm, fy: &Fbm, u: f32, v: f32, amp: f32) -> (f32, f32) {
    (u + amp * (fx.sample(u, v) - 0.5), v + amp * (fy.sample(u, v) - 0.5))
}
```

Add to `crates/anabios-core/src/lib.rs` alongside the other `mod` declarations:

```rust
mod noise;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p anabios-core --lib noise 2>&1 | tail -20`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/noise.rs crates/anabios-core/src/lib.rs
git commit -m "feat(worldgen): tileable gradient noise, fBm, domain warp"
```

---

### Task 2: Expand `TerrainType` + productivity + all exhaustive match sites

This task is golden-neutral: it only appends enum variants and match arms. `generate` is unchanged, so no new variant is produced yet and `state_hash` is unchanged.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (enum ~88-94, `carrying_capacity` ~100-107, `regrowth_rate` ~110-117)
- Modify: `crates/anabios-core/src/resource.rs` (`from_terrain` ~86-94)
- Modify: `crates/anabios-godot/src/lib.rs` (`biome_colors` match ~636-642)
- Modify: `crates/anabios-core/examples/biome_scout.rs` (ASCII `match` ~107-119; scoring)
- Test: inline in `biome.rs`, plus existing `resource.rs::home_terrain_inverts_from_terrain`

**Interfaces:**
- Produces: `TerrainType::{Savanna=5, Rainforest=6, Taiga=7, Tundra=8}`; `carrying_capacity`/`regrowth_rate` total over 9 variants; `Good::from_terrain` total over 9 variants.

- [ ] **Step 1: Write the failing test**

```rust
// crates/anabios-core/src/biome.rs  (in #[cfg(test)] mod tests)
#[test]
fn new_terrains_have_productivity_ordering() {
    use TerrainType::*;
    assert!(Rainforest.carrying_capacity() > Forest.carrying_capacity());
    assert!(Forest.carrying_capacity() > Taiga.carrying_capacity());
    assert!(Taiga.carrying_capacity() > Grass.carrying_capacity());
    assert!(Grass.carrying_capacity() > Savanna.carrying_capacity());
    assert!(Savanna.carrying_capacity() > Tundra.carrying_capacity());
    assert!(Tundra.carrying_capacity() > Desert.carrying_capacity());
    assert_eq!(Water.carrying_capacity(), 0.0);
    assert_eq!(Rock.carrying_capacity(), 0.0);
    // Existing values unchanged.
    assert_eq!(Forest.carrying_capacity(), 20.0);
    assert_eq!(Grass.carrying_capacity(), 10.0);
    assert_eq!(Desert.carrying_capacity(), 3.0);
    for t in [Savanna, Rainforest, Taiga, Tundra] {
        assert!(t.regrowth_rate() > 0.0, "{t:?} must regrow");
    }
}
```

```rust
// crates/anabios-core/src/resource.rs  (in #[cfg(test)] mod tests)
#[test]
fn every_land_terrain_yields_a_good() {
    use crate::biome::TerrainType::*;
    for t in [Desert, Rock, Forest, Grass, Savanna, Rainforest, Taiga, Tundra] {
        assert!(Good::from_terrain(t).is_some(), "{t:?} must map to a good");
    }
    assert!(Good::from_terrain(Water).is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p anabios-core --lib new_terrains_have_productivity_ordering every_land_terrain_yields_a_good 2>&1 | tail -20`
Expected: FAIL — variants `Savanna`/`Rainforest`/`Taiga`/`Tundra` do not exist (compile error).

- [ ] **Step 3: Write the implementation**

Enum (`biome.rs`):

```rust
pub enum TerrainType {
    Water = 0,
    Grass = 1,
    Forest = 2,
    Desert = 3,
    Rock = 4,
    Savanna = 5,
    Rainforest = 6,
    Taiga = 7,
    Tundra = 8,
}
```

`carrying_capacity`:

```rust
        match self {
            TerrainType::Water => 0.0,
            TerrainType::Grass => 10.0,
            TerrainType::Forest => 20.0,
            TerrainType::Desert => 3.0,
            TerrainType::Rock => 0.0,
            TerrainType::Savanna => 8.0,
            TerrainType::Rainforest => 28.0,
            TerrainType::Taiga => 12.0,
            TerrainType::Tundra => 4.0,
        }
```

`regrowth_rate`:

```rust
        match self {
            TerrainType::Water => 0.0,
            TerrainType::Grass => 0.01,
            TerrainType::Forest => 0.003,
            TerrainType::Desert => 0.002,
            TerrainType::Rock => 0.0,
            TerrainType::Savanna => 0.006,
            TerrainType::Rainforest => 0.004,
            TerrainType::Taiga => 0.004,
            TerrainType::Tundra => 0.002,
        }
```

`Good::from_terrain` (`resource.rs`):

```rust
        match t {
            TerrainType::Desert => Some(Good::Salt),
            TerrainType::Rock => Some(Good::Obsidian),
            TerrainType::Forest => Some(Good::Amber),
            TerrainType::Grass => Some(Good::Spice),
            TerrainType::Rainforest | TerrainType::Taiga => Some(Good::Amber),
            TerrainType::Savanna | TerrainType::Tundra => Some(Good::Spice),
            TerrainType::Water => None,
        }
```

(`home_terrain` is a match on `Good` — unchanged; the invariant `from_terrain(g.home_terrain()) == Some(g)` still holds because the four representatives are unchanged.)

`biome_colors` (`crates/anabios-godot/src/lib.rs`), add four arms inside the existing match:

```rust
                TerrainType::Savanna => Color::from_rgb(0.72, 0.66, 0.36),
                TerrainType::Rainforest => Color::from_rgb(0.06, 0.34, 0.16),
                TerrainType::Taiga => Color::from_rgb(0.16, 0.34, 0.26),
                TerrainType::Tundra => Color::from_rgb(0.62, 0.66, 0.62),
```

`biome_scout.rs` ASCII map match — add arms and update legend (`'S'` savanna, `'T'` rainforest/tropical, `'t'` taiga, `'u'` tundra); update the `score_seed` "tropical" definition to count `TerrainType::Rainforest` directly (see Task 6 for the full scout rewrite; here just make it compile):

```rust
                TerrainType::Savanna => 'S',
                TerrainType::Rainforest => 'T',
                TerrainType::Taiga => 't',
                TerrainType::Tundra => 'u',
```

- [ ] **Step 4: Run tests + full workspace build**

Run: `cargo test -p anabios-core --lib 2>&1 | tail -15 && cargo build --workspace 2>&1 | tail -5`
Expected: new tests PASS; workspace compiles (all exhaustive matches covered); existing golden tests still pass (generate unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/biome.rs crates/anabios-core/src/resource.rs crates/anabios-godot/src/lib.rs crates/anabios-core/examples/biome_scout.rs
git commit -m "feat(worldgen): expand TerrainType with savanna/rainforest/taiga/tundra"
```

---

### Task 3: Climate field + Whittaker classification (pure functions)

Pure, unit-testable functions. `generate` does not call them yet, so goldens stay green.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (add constants + functions near the noise usage; place above `generate`)
- Test: inline in `biome.rs`

**Interfaces:**
- Produces:
  - consts `SEA_LEVEL`, `ROCK_LINE`, `TEMP_LAPSE`, band cutoffs `BAND_LO=0.33`, `BAND_HI=0.66`.
  - `fn latitude_temp(v: f32) -> f32` — `1 - |2v-1|`.
  - `fn latitude_moisture(v: f32) -> f32` — `0.5 + 0.5*cos(3π(2v-1))`.
  - `fn classify(elevation: f32, temperature: f32, moisture: f32) -> TerrainType`.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/anabios-core/src/biome.rs  (in #[cfg(test)] mod tests)
#[test]
fn latitude_temp_peaks_at_equator_and_wraps() {
    assert!(latitude_temp(0.5) > latitude_temp(0.25));
    assert!(latitude_temp(0.5) > latitude_temp(0.75));
    assert!((latitude_temp(0.0) - latitude_temp(1.0)).abs() < 1e-6); // toroidal
    assert!(latitude_temp(0.0) < 0.05); // poles cold
}

#[test]
fn latitude_moisture_is_dry_in_subtropics_wet_at_equator() {
    let equator = latitude_moisture(0.5);
    let subtropics = latitude_moisture(0.5 - 1.0 / 6.0); // |2v-1| = 1/3
    assert!(equator > subtropics, "equator {equator} should be wetter than subtropics {subtropics}");
}

#[test]
fn classify_matches_whittaker_corners() {
    use TerrainType::*;
    let mid = 0.6; // land, below rock line
    assert_eq!(classify(0.1, 0.9, 0.9), Water); // below sea level
    assert_eq!(classify(0.95, 0.9, 0.9), Rock); // above rock line
    assert_eq!(classify(mid, 0.9, 0.1), Desert); // hot + arid
    assert_eq!(classify(mid, 0.9, 0.9), Rainforest); // hot + wet
    assert_eq!(classify(mid, 0.9, 0.5), Savanna); // hot + moderate
    assert_eq!(classify(mid, 0.5, 0.1), Grass); // temperate + arid
    assert_eq!(classify(mid, 0.5, 0.9), Forest); // temperate + wet
    assert_eq!(classify(mid, 0.1, 0.1), Tundra); // cold + arid
    assert_eq!(classify(mid, 0.1, 0.9), Taiga); // cold + wet
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p anabios-core --lib latitude_temp latitude_moisture classify_matches 2>&1 | tail -20`
Expected: FAIL — `latitude_temp` / `classify` not defined.

- [ ] **Step 3: Write the implementation**

```rust
// crates/anabios-core/src/biome.rs  (above `impl BiomeField`)
/// Fraction of world extent below which a cell is open water.
pub const SEA_LEVEL: f32 = 0.40;
/// Elevation above which a cell is barren rock/peak.
pub const ROCK_LINE: f32 = 0.82;
/// Temperature drop per unit elevation above sea level.
pub const TEMP_LAPSE: f32 = 0.55;
/// Whittaker band cutoffs for temperature and moisture.
pub const BAND_LO: f32 = 0.33;
pub const BAND_HI: f32 = 0.66;

/// Latitude temperature: 1 at the equator (v=0.5), 0 at the poles (v=0 or 1).
/// Continuous across the torus wrap (both y-edges are poles).
#[inline]
pub fn latitude_temp(v: f32) -> f32 {
    1.0 - (2.0 * v - 1.0).abs()
}

/// Latitude moisture band profile: wet equator, dry subtropics (~30 deg), wet
/// temperate, dry poles — the driver of subtropical deserts and the equatorial
/// rainforest belt.
#[inline]
pub fn latitude_moisture(v: f32) -> f32 {
    0.5 + 0.5 * (3.0 * std::f32::consts::PI * (2.0 * v - 1.0)).cos()
}

/// Whittaker classification: elevation gates water/rock, then (temperature,
/// moisture) select the land biome. Hard thresholds, no border dithering.
pub fn classify(elevation: f32, temperature: f32, moisture: f32) -> TerrainType {
    if elevation < SEA_LEVEL {
        return TerrainType::Water;
    }
    if elevation > ROCK_LINE {
        return TerrainType::Rock;
    }
    let hot = temperature > BAND_HI;
    let cold = temperature < BAND_LO;
    let arid = moisture < BAND_LO;
    let wet = moisture > BAND_HI;
    if hot {
        if arid {
            TerrainType::Desert
        } else if wet {
            TerrainType::Rainforest
        } else {
            TerrainType::Savanna
        }
    } else if cold {
        if arid {
            TerrainType::Tundra
        } else {
            TerrainType::Taiga
        }
    } else if arid {
        TerrainType::Grass
    } else {
        TerrainType::Forest
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p anabios-core --lib latitude_temp latitude_moisture classify_matches 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/biome.rs
git commit -m "feat(worldgen): latitude climate fields + Whittaker classification"
```

---

### Task 4: Rewrite `BiomeField::generate` + add `moisture` to `BiomeCell`

This is the golden-breaking task. `generate` now produces climate-based terrain and `BiomeCell` gains a field, changing `state_hash`.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs` (`BiomeCell` struct add `moisture`; rewrite `generate` ~168-217; delete `elevation_to_terrain` ~537-549; update `env` doc comment; fix existing tests that relied on old generation)
- Modify: `crates/anabios-core/src/snapshot.rs` (`FORMAT_VERSION` 19→20 + changelog line ~82)
- Test: inline in `biome.rs`

**Interfaces:**
- Consumes: `crate::noise::{Fbm, warp}`, `classify`, `latitude_temp`, `latitude_moisture`, consts from Tasks 1 & 3.
- Produces: `BiomeCell.moisture: f32`; `BiomeField::generate(seed, res, world_size)` (same signature) producing climate terrain; `env` now holds temperature.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/anabios-core/src/biome.rs  (in #[cfg(test)] mod tests)
#[test]
fn every_world_has_deserts_and_rainforest() {
    for seed in 0..12 {
        let b = BiomeField::generate(seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        let has = |t: TerrainType| b.cells.iter().any(|c| c.terrain == t);
        assert!(has(TerrainType::Desert), "seed {seed} has no desert");
        assert!(has(TerrainType::Rainforest), "seed {seed} has no rainforest");
    }
}

#[test]
fn all_land_terrains_appear_across_seeds() {
    use TerrainType::*;
    let mut seen = std::collections::HashSet::new();
    for seed in 0..24 {
        let b = BiomeField::generate(seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        for c in &b.cells {
            seen.insert(c.terrain as u8);
        }
    }
    for t in [Water, Desert, Savanna, Grass, Forest, Rainforest, Taiga, Tundra] {
        assert!(seen.contains(&(t as u8)), "{t:?} never generated");
    }
}

#[test]
fn generate_is_deterministic_including_moisture() {
    let a = BiomeField::generate(42, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
    let b = BiomeField::generate(42, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
    for i in 0..a.cells.len() {
        assert_eq!(a.cells[i].terrain, b.cells[i].terrain);
        assert_eq!(a.cells[i].moisture, b.cells[i].moisture);
        assert_eq!(a.cells[i].env, b.cells[i].env);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p anabios-core --lib every_world_has_deserts all_land_terrains generate_is_deterministic_including_moisture 2>&1 | tail -20`
Expected: FAIL — `moisture` field missing / rainforest absent (old generator).

- [ ] **Step 3: Write the implementation**

Add to `BiomeCell` (after `env`), and update `env`'s doc comment to say it holds temperature:

```rust
    /// Per-cell temperature climate value in `[0,1]` (latitude gradient minus
    /// elevation lapse plus noise). Static after generation. Read by the
    /// biome-adaptation feeding bonus and the seasonal-regrowth match.
    pub env: f32,
    /// Per-cell moisture in `[0,1]` (latitude band profile + fBm + coastal).
    /// Static after generation; drives Whittaker classification at gen time.
    #[serde(default)]
    pub moisture: f32,
```

Rewrite `generate` (fixed RNG draw order: elevation warp x, warp y, elevation, temp-noise, moisture, then the existing nutrient/fertility fields to minimize churn — but note the whole hash is regenerated regardless):

```rust
    pub fn generate(seed: u64, res: usize, world_size: f32) -> Self {
        let mut rng = Rng::from_seed(seed);
        // Domain-warp fields, then elevation / temperature-noise / moisture.
        // Draw order is part of the determinism contract.
        let warp_x = crate::noise::Fbm::new(&mut rng, 3, 4, 2, 0.5);
        let warp_y = crate::noise::Fbm::new(&mut rng, 3, 4, 2, 0.5);
        let elevation = crate::noise::Fbm::new(&mut rng, 4, 5, 2, 0.5);
        let temp_noise = crate::noise::Fbm::new(&mut rng, 5, 3, 2, 0.5);
        let moisture_noise = crate::noise::Fbm::new(&mut rng, 6, 4, 2, 0.5);
        // Nutrient-quality and fertility fields (unchanged semantics).
        let nutrient = crate::noise::Fbm::new(&mut rng, 5, 3, 2, 0.5);
        let fertility_noise = crate::noise::Fbm::new(&mut rng, 4, 3, 2, 0.5);

        const WARP_AMP: f32 = 0.35;
        const TEMP_NOISE: f32 = 0.10;
        let mut cells = Vec::with_capacity(res * res);
        for row in 0..res {
            for col in 0..res {
                let u = col as f32 / res as f32;
                let v = row as f32 / res as f32;
                let (wu, wv) = crate::noise::warp(&warp_x, &warp_y, u, v, WARP_AMP);
                let elev = elevation.sample(wu, wv);
                let temperature = (latitude_temp(v)
                    - TEMP_LAPSE * (elev - SEA_LEVEL).max(0.0)
                    + TEMP_NOISE * (temp_noise.sample(u, v) - 0.5))
                    .clamp(0.0, 1.0);
                let moisture = (0.5 * latitude_moisture(v)
                    + 0.5 * moisture_noise.sample(wu, wv))
                    .clamp(0.0, 1.0);
                let terrain = classify(elev, temperature, moisture);
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
                });
            }
        }
        Self { cells, res, world_size, cell_size: world_size / res as f32 }
    }
```

Delete `elevation_to_terrain` (now unused) and the old `NoiseGrid` struct/impl if nothing else references them (grep first: `grep -n NoiseGrid crates/anabios-core/src/biome.rs`). Bump `FORMAT_VERSION`:

```rust
// snapshot.rs
pub const FORMAT_VERSION: u32 = 20;
```
Add a changelog line in the version-history doc comment: `/// v20: climate-driven worldgen — BiomeCell.moisture; env now holds temperature; expanded TerrainType.`

Fix existing `biome.rs` tests that assumed old generation:
- `climate_field_is_bounded_and_varies`, `climate_not_a_function_of_terrain_alone` — should still pass (temperature varies within a terrain); run and confirm.
- `biome_contains_multiple_terrain_types` — still holds.
- Any test asserting specific `Grass` cells exist at a seed (e.g. recolonize/regrow tests at seed 13/31) — Grass still generated; if a specific seed lacks Grass, pick a seed/cell that has it or search for a Grass cell dynamically. Run the suite and fix per failure.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p anabios-core --lib 2>&1 | tail -25`
Expected: new tests PASS; fix any old `biome.rs` unit test that encoded elevation-era assumptions (update to search for a cell of the needed terrain rather than a hard-coded index).

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/src/biome.rs crates/anabios-core/src/snapshot.rs
git commit -m "feat(worldgen): climate-driven generate + moisture field, FORMAT_VERSION 20"
```

---

### Task 5: Regenerate golden hashes + fix behavioral tests

**Files:**
- Modify (regenerate values): `crates/anabios-core/tests/determinism.rs`, `crates/anabios-core/tests/inventions.rs`, `crates/anabios-core/tests/cognition.rs`, `crates/anabios-core/tests/trade.rs`
- Inspect/possibly fix: `crates/anabios-core/tests/nutrient_fertility.rs`, `tests/dims.rs`, `tests/combat_predation.rs`, `tests/module_gating.rs`, `tests/cognition_evolution.rs`
- Possibly modify: `scenarios/biome-trade.toml` (only if a trade behavioral assert regresses)

**Interfaces:**
- Consumes: everything from Task 4.

- [ ] **Step 1: Run the full suite to see what broke**

Run: `cargo test --workspace 2>&1 | tail -40`
Expected: golden-hash tests FAIL with mismatched hashes; note any behavioral failures separately.

- [ ] **Step 2: Confirm how goldens are regenerated**

Run: `grep -rn "UPDATE_HASHES\|env::var\|expect_hash\|GOLDEN" crates/anabios-core/tests/determinism.rs | head`
Expected: reveals the update mechanism (e.g. `UPDATE_HASHES=1`). Use it:

Run: `UPDATE_HASHES=1 cargo test --workspace 2>&1 | tail -20` (or, if no such flag exists, copy each printed `actual` hash into the test's expected constant).

- [ ] **Step 3: Investigate each non-golden behavioral failure**

For any failure that is NOT a golden-hash mismatch (e.g. a test asserting a cell is `Grass` at a fixed index, or `trade.rs` asserting `ResourceTraded` fires), read the test and update the assumption:
- Terrain-at-fixed-index assertions → search for the terrain instead of hard-coding.
- If `trade.rs`/`biome-trade.toml` no longer yields trades because the hand-picked cluster lost its 4-good terrain mix, re-pick the cluster center by scanning the new field (same method as `biome_scout`) and update `center_x`/`center_y` in `scenarios/biome-trade.toml`, then re-regen that scenario's golden.

- [ ] **Step 4: Run the full suite to verify green**

Run: `cargo test --workspace 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/anabios-core/tests scenarios/biome-trade.toml
git commit -m "test(worldgen): regenerate goldens + fix terrain-dependent tests for climate worldgen"
```

---

### Task 6: Update `desert-tropical.toml` and `biome_scout` for the climate model

**Files:**
- Modify: `scenarios/desert-tropical.toml`
- Modify: `crates/anabios-core/examples/biome_scout.rs`

**Interfaces:**
- Consumes: the climate generator (biomes guaranteed by latitude).

- [ ] **Step 1: Rewrite `desert-tropical.toml` for latitude placement**

Biomes are now guaranteed, so drop seed-scouting. Place the desert cohort in a subtropical band and the tropical cohort at the equator. World spans 0..1024; equator `v=0.5 → y≈512`, subtropics `v≈0.33 → y≈341`.

```toml
# A world with subtropical deserts and an equatorial rainforest belt.
#
# Terrain is now climate-driven (Whittaker classification of elevation x
# temperature x moisture; see docs/superpowers/specs/2026-07-27-procgen-worldgen-design.md).
# Deserts form in the dry subtropical band (~y=341) and tropical rainforest in
# the wet equatorial band (~y=512) in EVERY seed — no seed-scouting needed.
name = "desert-tropical"
seed = 14
biome_adaptation = true
terrain_habitat = true
living_biome = true
season_period = 400
max_population = 2000

# Desert cohort — subtropical dry band.
[[agents]]
count = 120
placement = { kind = "cluster", center_x = 512.0, center_y = 341.0, radius = 80.0 }
[agents.traits]
terrain_affinity = 0.1
lifespan_bias = 0.6
reproduction_threshold = 0.5

# Tropical cohort — equatorial rainforest band.
[[agents]]
count = 120
placement = { kind = "cluster", center_x = 512.0, center_y = 512.0, radius = 80.0 }
[agents.traits]
terrain_affinity = 0.6
lifespan_bias = 0.6
reproduction_threshold = 0.5
```

- [ ] **Step 2: Update `biome_scout` scoring/legend for the new terrains**

"Tropical" now means `TerrainType::Rainforest` directly (drop the `Forest`+`env` heuristic). Update `score_seed` to count `Desert` and `Rainforest`, update the densest-patch predicates, and update the ASCII legend line to include savanna/taiga/tundra. (The match arms were added in Task 2.) Concretely, in `score_seed` replace the tropical branch:

```rust
    for c in &field.cells {
        match c.terrain {
            TerrainType::Desert => desert += 1.0,
            TerrainType::Rainforest => tropical += 1.0,
            TerrainType::Forest => forest += 1.0,
            _ => {}
        }
    }
```

and the densest-patch tropical predicate to `|c| c.terrain == TerrainType::Rainforest`. Update the legend `println!` to: `~ water  . desert  S savanna  " grass  f forest  T rainforest  t taiga  u tundra  ^ rock`.

- [ ] **Step 3: Verify scout + scenario run**

Run:
```
cargo run -q -p anabios-core --example biome_scout 2>&1 | sed -n '1,24p'
cargo run -q --release -p anabios-headless -- run --scenario scenarios/desert-tropical.toml --ticks 1000 2>&1 | tail -3
```
Expected: scout renders a map with deserts + rainforest; scenario runs 1000 ticks, population capped ~2000, deterministic `state_hash`.

- [ ] **Step 4: Commit**

```bash
git add scenarios/desert-tropical.toml crates/anabios-core/examples/biome_scout.rs
git commit -m "feat(worldgen): latitude-based desert-tropical scenario + scout update"
```

---

### Task 7: Final CI gate

**Files:** none (verification only).

- [ ] **Step 1: fmt**

Run: `cargo fmt && cargo fmt --check && echo FMT_OK`
Expected: `FMT_OK`.

- [ ] **Step 2: clippy**

Run: `cargo clippy --workspace --all-targets 2>&1 | tail -15`
Expected: no warnings.

- [ ] **Step 3: rustdoc (CI gate)**

Run: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps 2>&1 | tail -10`
Expected: builds clean.

- [ ] **Step 4: full test**

Run: `cargo test --workspace 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 5: Commit any fmt-only changes and push**

```bash
git add -u && git commit -m "chore(worldgen): fmt" || echo "nothing to commit"
git push
```

---

## Self-Review

**Spec coverage:**
- Gradient noise + fBm + domain warp → Task 1. ✅
- Seamless torus → Task 1 (`gradient_noise_is_seamless_on_torus`). ✅
- Elevation redistribution → folded into fBm sampling (Task 4); note: the spec's power-curve redistribution is approximated by fBm normalization + SEA_LEVEL/ROCK_LINE gating. If land ratio is off after Task 4, tune SEA_LEVEL (documented as tunable). ✅ (acceptable simplification)
- Temperature (latitude + lapse + noise) → Task 3 (`latitude_temp`) + Task 4 (assembly). ✅
- Moisture (latitude bands + fBm + coastal) → Task 3 (`latitude_moisture`) + Task 4. Note: coastal term dropped from the initial assembly to avoid a distance pass; latitude+fBm already yields the desert/tropical bands the goal requires. Coastal is an optional follow-up. ✅ (documented deviation)
- Whittaker classification, hard thresholds → Task 3 (`classify`). ✅
- Expanded TerrainType + productivity + all matches → Task 2. ✅
- Trade remap → Task 2 (`from_terrain`), invariant preserved. ✅
- BiomeCell.moisture + env=temperature + FORMAT_VERSION bump → Task 4. ✅
- Golden regen + behavioral test fixes → Task 5. ✅
- Scenario + scout updates → Task 6. ✅
- CI gate (fmt/clippy/rustdoc/test) → Task 7. ✅

**Placeholder scan:** No TBD/TODO. Task 4 delete-`elevation_to_terrain` and old-test-fix steps say "grep first / fix per failure" — these are genuine investigate-then-act steps with the exact commands given, not hand-waves.

**Type consistency:** `GradientNoise::new`/`sample`, `Fbm::new`/`sample`, `warp` signatures match between Task 1 (definition) and Task 4 (use). `classify`/`latitude_temp`/`latitude_moisture` signatures match between Task 3 and Task 4. `TerrainType` variant names identical across Tasks 2/3/4/6. `from_terrain` returns `Option<Good>` consistently.

**Deviations from spec (intentional, low-risk):** coastal moisture term and explicit elevation power-curve are deferred (the latitude bands alone deliver guaranteed deserts+tropics); both are tunable/optional and noted above.
