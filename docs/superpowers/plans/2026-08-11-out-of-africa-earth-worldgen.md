# Out of Africa on a Real Earth Map — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a new grand-scale scenario `out-of-africa-earth` that runs the full out-of-africa subsystem stack on a real Earth map (real elevation + temperature + precipitation) at 256×256 / 4096 units, with founding lineages placed on real coordinates.

**Architecture:** An offline builder resamples public-domain Earth rasters into three checked-in `u8` assets (normalized to `[0,1]`). A new `BiomeField::from_earth` dequantizes those assets into the *existing* `elevation`/`env`/`moisture` arrays and runs the *existing* `classify()`. A scenario opt-in (`world_map = "earth"`) routes biome generation through `from_earth`, and a new `Placement::Geo { lat, lon, radius }` maps real coordinates to sim positions. No new `World`/`BiomeCell` fields → no golden regen, no `FORMAT_VERSION` bump.

**Tech Stack:** Rust (anabios-core), TOML scenarios, Python (offline asset builder only — not in the deterministic core), `include_bytes!` for the embedded assets.

## Global Constraints

- **No new `BiomeCell` or `World` field.** Real data flows through the existing `elevation` / `env` (temperature) / `moisture` cell fields. This is what keeps the three golden tests (`minimal_scenario_matches_golden_hashes`, inventions, cognition) untouched. (Ref spec §"Determinism".)
- **Do not modify** `scenarios/out-of-africa.toml`, `scenarios/out-of-africa-saga.toml`, or any existing scenario. Additive only.
- **Scale is fixed:** `world_size = 4096`, `biome_res = 256`, `hash_res = 256` (keeps `world_size / hash_res ≈ 16`, the perception cap).
- **`EARTH_RES = 256`.** Assets are exactly 256×256 = 65536 bytes per channel, row-major, `row=0` = north (lat +90), `col=0` = west (lon −180). `classify` uses default `SEA_LEVEL = 0.35`; the builder normalizes real 0 m sea level to exactly 0.35.
- **Dequantization is `byte as f32 / 255.0`** for all three channels — every real-world unit conversion lives in the offline builder, never in the core.
- **Public-domain data only** (NOAA/NASA). Record provenance in `crates/anabios-core/assets/earth/README.md`. No CC-BY sources.
- **Determinism:** `from_earth` uses no RNG. `Placement::Geo` draws from `w.rng` in the exact same order as `Placement::Cluster` (`theta` then `r`).
- Run `cargo fmt` and `cargo clippy` clean before each commit (CI runs `cargo fmt --check` + rustdoc `-D warnings`).

## File Structure

- **Create** `scripts/build_earth_map.py` — offline asset builder (real-raster resampler + synthetic fallback). Not run by the sim; not tested by cargo.
- **Create** `crates/anabios-core/assets/earth/{elevation,temperature,precip}.u8` — checked-in normalized `u8` rasters (65536 bytes each).
- **Create** `crates/anabios-core/assets/earth/README.md` — data provenance + quantization ranges.
- **Modify** `crates/anabios-core/src/biome.rs` — add `EARTH_RES` const + `BiomeField::from_earth`.
- **Modify** `crates/anabios-core/src/scenario.rs` — add `WorldMapSource` enum + `world_map` field; add `Placement::Geo`; wire `from_earth` and `Geo` into `instantiate`.
- **Create** `crates/anabios-core/tests/earth_worldgen.rs` — `from_earth` + `Geo` unit/integration tests.
- **Create** `scenarios/out-of-africa-earth.toml` — the new scenario.
- **Modify** `crates/anabios-core/tests/determinism.rs` — add the save/load round-trip test for the new scenario.

---

## Task 0: Data-acquisition spike (decide the source)

**Purpose:** De-risk the one thing the whole feature depends on — obtaining real Earth rasters in this environment — before writing any core code. Outcome is a decision recorded in the README, not production code.

**Files:**
- Create: `crates/anabios-core/assets/earth/README.md`

- [ ] **Step 1: Probe for real public-domain rasters.** Try to obtain, at any resolution (they will be resampled to 256×256 in Task 1):
  - Elevation: NOAA ETOPO 2022 (public domain), or Natural Earth 1:50m/1:110m raster (public domain), or the GEBCO/ETOPO derivative bundled with common libraries.
  - Temperature + precipitation: a public-domain mean-annual climatology (NASA NEO, NOAA). Avoid WorldClim (CC-BY).

  Attempt fetch via `curl`/`wget` in a scratch dir (`/private/tmp/.../scratchpad`). Do **not** commit raw downloads — only the resampled 256×256 assets get checked in (Task 1).

- [ ] **Step 2: Decide the path and record it.** Write `crates/anabios-core/assets/earth/README.md` stating which source path is in use:
  - **Path A (real rasters):** list each source URL, license (must be public domain), original resolution, and the planned normalization (elevation: 0 m → 0.35; temp: −40..40 °C → 0..1; precip: 0..~4000 mm, log-scaled → 0..1).
  - **Path B (synthetic fallback):** if no public-domain raster is obtainable here, record that the assets are generated by `scripts/build_earth_map.py --source synthetic` from a coarse hand-traced Earth land mask, with latitude-derived temperature and a rough precipitation model. Note the upgrade path: swapping in real rasters later is a drop-in (identical `.u8` format), no core change.

- [ ] **Step 3: Commit the decision.**

```bash
git add crates/anabios-core/assets/earth/README.md
git commit -m "docs(worldgen): record Earth-map data source decision (Task 0 spike)"
```

---

## Task 1: Offline asset builder + checked-in Earth rasters

**Purpose:** Produce the three normalized `u8` assets the core will embed. All real-world math lives here; the output is a pure `[0,1]`-per-channel raster.

**Files:**
- Create: `scripts/build_earth_map.py`
- Create: `crates/anabios-core/assets/earth/{elevation,temperature,precip}.u8`

**Interfaces:**
- Produces: three files, each exactly `256*256 = 65536` bytes, row-major (`row=0` north, `col=0` west), one `u8` per cell = `round(value_normalized_to_[0,1] * 255)`.

- [ ] **Step 1: Write the builder skeleton with both source paths.**

```python
#!/usr/bin/env python3
"""Offline builder for the checked-in Earth biome rasters.

Outputs three 256x256 u8 files (elevation, temperature, precip), each a
value in [0,1] quantized to a byte. NOT run by the simulation — the sim
embeds the .u8 outputs via include_bytes!. Determinism lives in the bytes,
not this script.

Usage:
  build_earth_map.py --source real --elev ETOPO.tif --temp T.tif --precip P.tif
  build_earth_map.py --source synthetic   # no external data; coarse Earth
"""
import argparse, struct, sys
from pathlib import Path

RES = 256
OUT = Path(__file__).resolve().parents[1] / "crates/anabios-core/assets/earth"
SEA_LEVEL_NORM = 0.35  # must match biome::SEA_LEVEL

def write_u8(name, grid):
    assert len(grid) == RES * RES, f"{name}: {len(grid)} != {RES*RES}"
    b = bytes(max(0, min(255, round(v * 255))) for v in grid)
    (OUT / f"{name}.u8").write_bytes(b)
    print(f"wrote {name}.u8 ({len(b)} bytes)")
```

- [ ] **Step 2: Implement `--source real` (resample + normalize).** Reads input rasters (any resolution) and resamples to 256×256. Requires `numpy` + `rasterio`/`Pillow`; guard the import with a clear error.

```python
def build_real(elev_path, temp_path, precip_path):
    import numpy as np
    from PIL import Image
    def load_resampled(path):
        img = Image.open(path).convert("F").resize((RES, RES), Image.BILINEAR)
        return np.asarray(img, dtype="float32")  # row 0 = north
    elev_m = load_resampled(elev_path)
    temp_c = load_resampled(temp_path)
    precip_mm = load_resampled(precip_path)
    # Elevation: 0 m -> SEA_LEVEL_NORM; deep ocean -> ~0.05; peaks -> ~1.0.
    en = np.where(
        elev_m >= 0,
        SEA_LEVEL_NORM + (elev_m / 8850.0) * (1.0 - SEA_LEVEL_NORM),
        SEA_LEVEL_NORM * (1.0 + np.clip(elev_m, -11000, 0) / 11000.0),
    ).clip(0, 1)
    tn = ((temp_c + 40.0) / 80.0).clip(0, 1)                 # -40..40 C -> 0..1
    pn = (np.log1p(precip_mm.clip(0, None)) / np.log1p(4000.0)).clip(0, 1)
    for name, arr in (("elevation", en), ("temperature", tn), ("precip", pn)):
        write_u8(name, arr.flatten().tolist())
```

- [ ] **Step 3: Implement `--source synthetic` (self-contained fallback, stdlib only).** Rasterizes a coarse Earth land mask (traced into `LAND_MASK`, a list of `RES`-wide rows or an upscaled low-res mask acquired at implementation time), then derives temperature from latitude and precipitation from a wet-equator/dry-subtropics band. This path guarantees a checked-in, deterministic asset even with no network.

```python
def build_synthetic():
    import math
    # LAND_MASK: RES*RES of 0/1 (1 = land), acquired at implementation time by
    # upscaling a coarse public-domain Earth outline. See README Path B.
    land = load_land_mask()  # -> list[float] length RES*RES, 0.0 or 1.0
    elev, temp, precip = [], [], []
    for row in range(RES):
        v = row / RES                      # 0=north .. 1=south
        lat = 90.0 - v * 180.0
        t = max(0.0, min(1.0, 1.0 - abs(lat) / 90.0))   # hot equator, cold poles
        # wet equator, dry ~25 deg, wet temperate, dry poles
        p = max(0.0, min(1.0, 0.5 + 0.5 * math.cos(math.radians(lat) * 3.0)))
        for col in range(RES):
            i = row * RES + col
            is_land = land[i] > 0.5
            elev.append(SEA_LEVEL_NORM + 0.25 if is_land else 0.15)
            temp.append(t)
            precip.append(p if is_land else min(1.0, p + 0.2))
    write_u8("elevation", elev); write_u8("temperature", temp); write_u8("precip", precip)
```

- [ ] **Step 4: Wire argparse + run.**

```python
if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", choices=["real", "synthetic"], required=True)
    ap.add_argument("--elev"); ap.add_argument("--temp"); ap.add_argument("--precip")
    a = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    if a.source == "real":
        build_real(a.elev, a.temp, a.precip)
    else:
        build_synthetic()
```

- [ ] **Step 5: Generate + validate the assets.** Run the path chosen in Task 0. Then sanity-check with a throwaway snippet: global land fraction (elevation ≥ 0.35) is plausible (~0.25–0.35), central Africa cell `(lat=0, lon=20)` reads land, mid-Pacific `(lat=0, lon=-150)` reads water.

```bash
python3 scripts/build_earth_map.py --source <chosen>   # + --elev/--temp/--precip if real
ls -l crates/anabios-core/assets/earth/*.u8            # each must be 65536 bytes
```

Expected: three 65536-byte files; land fraction ~0.29; Africa land; mid-Pacific water.

- [ ] **Step 6: Commit.**

```bash
git add scripts/build_earth_map.py crates/anabios-core/assets/earth/*.u8 crates/anabios-core/assets/earth/README.md
git commit -m "feat(worldgen): offline Earth-map builder + checked-in 256x256 rasters"
```

---

## Task 2: `BiomeField::from_earth`

**Purpose:** The deterministic core path that turns the embedded rasters into a `BiomeField`, reusing the existing `classify_with` and `BiomeCell` shape.

**Files:**
- Modify: `crates/anabios-core/src/biome.rs`
- Create: `crates/anabios-core/tests/earth_worldgen.rs`

**Interfaces:**
- Consumes: assets from Task 1 (`crates/anabios-core/assets/earth/*.u8`).
- Produces: `pub const EARTH_RES: usize = 256;` and `pub fn from_earth(res: usize, world_size: f32) -> BiomeField`.

- [ ] **Step 1: Write the failing test** in `crates/anabios-core/tests/earth_worldgen.rs`:

```rust
use anabios_core::biome::{BiomeField, TerrainType, EARTH_RES};

/// from_earth builds a full 256x256 field with a plausible land fraction and
/// real coastlines: central Africa is land, the mid-Pacific is open water.
#[test]
fn from_earth_has_real_coastlines() {
    let f = BiomeField::from_earth(EARTH_RES, 4096.0);
    assert_eq!(f.cells.len(), EARTH_RES * EARTH_RES);
    let land = f.cells.iter().filter(|c| c.terrain != TerrainType::Water).count();
    let frac = land as f32 / f.cells.len() as f32;
    assert!((0.15..0.45).contains(&frac), "implausible land fraction {frac}");

    // Equirectangular: x = (lon+180)/360*W, y = (90-lat)/180*W ; cell = pos/cell_size.
    let cell = |lat: f32, lon: f32| {
        let x = (lon + 180.0) / 360.0 * f.world_size;
        let y = (90.0 - lat) / 180.0 * f.world_size;
        let col = (x / f.cell_size) as usize;
        let row = (y / f.cell_size) as usize;
        f.cells[row * f.res + col].terrain
    };
    assert_ne!(cell(0.0, 20.0), TerrainType::Water, "central Africa should be land");
    assert_eq!(cell(0.0, -150.0), TerrainType::Water, "mid-Pacific should be water");
}

/// from_earth is a pure function of the embedded assets: identical every call.
#[test]
fn from_earth_is_deterministic() {
    let a = BiomeField::from_earth(EARTH_RES, 4096.0);
    let b = BiomeField::from_earth(EARTH_RES, 4096.0);
    assert!(a.cells.iter().zip(&b.cells).all(|(x, y)| x.terrain == y.terrain
        && x.elevation == y.elevation && x.env == y.env && x.moisture == y.moisture));
}
```

- [ ] **Step 2: Run test to verify it fails.**

Run: `cargo test -p anabios-core --test earth_worldgen`
Expected: FAIL to compile — `from_earth` / `EARTH_RES` not found.

- [ ] **Step 3: Implement `from_earth`** in `crates/anabios-core/src/biome.rs` (add near `generate_with`):

```rust
/// Resolution of the embedded real-Earth rasters (see `assets/earth/`).
pub const EARTH_RES: usize = 256;

impl BiomeField {
    /// Build a biome field from the embedded real-Earth rasters instead of the
    /// procedural fBm pipeline. No RNG: a pure dequantize + `classify`. The
    /// three `u8` assets hold elevation/temperature/moisture already normalized
    /// to `[0,1]` (all real-world unit conversion lives in the offline builder,
    /// `scripts/build_earth_map.py`). `res` must equal `EARTH_RES`.
    pub fn from_earth(res: usize, world_size: f32) -> Self {
        assert_eq!(res, EARTH_RES, "from_earth requires biome_res == EARTH_RES");
        const ELEV: &[u8] = include_bytes!("../assets/earth/elevation.u8");
        const TEMP: &[u8] = include_bytes!("../assets/earth/temperature.u8");
        const MOIST: &[u8] = include_bytes!("../assets/earth/precip.u8");
        assert!(
            ELEV.len() == res * res && TEMP.len() == res * res && MOIST.len() == res * res,
            "earth asset length must be res*res",
        );
        // Neutral midpoints for the fields the procedural path fills from noise;
        // real nutrient/fertility variation is out of scope for v1.
        let nutrient_quality = (NUTRIENT_QUALITY_MIN + NUTRIENT_QUALITY_MAX) / 2.0;
        let fertility = (FERTILITY_MIN + FERTILITY_MAX) / 2.0;
        let mut cells = Vec::with_capacity(res * res);
        for i in 0..res * res {
            let elevation = ELEV[i] as f32 / 255.0;
            let temperature = TEMP[i] as f32 / 255.0;
            let moisture = MOIST[i] as f32 / 255.0;
            let terrain = classify_with(elevation, temperature, moisture, SEA_LEVEL);
            cells.push(BiomeCell {
                terrain,
                plant_biomass: terrain.carrying_capacity(),
                env: temperature,
                moisture,
                pollution: 0.0,
                succession: SUCCESSION_CLIMAX,
                nutrient_quality,
                fertility,
                elevation,
                river_flow: 0.0,
            });
        }
        Self { cells, res, world_size, cell_size: world_size / res as f32, recolonize_scratch: Vec::new() }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass.**

Run: `cargo test -p anabios-core --test earth_worldgen`
Expected: PASS (both tests). If `from_earth_has_real_coastlines` fails on the Africa/Pacific assertion, the asset's row/col orientation is flipped — fix the builder's orientation in Task 1, regenerate, and re-run (do not flip it in the core).

- [ ] **Step 5: Commit.**

```bash
cargo fmt && cargo clippy -p anabios-core --tests
git add crates/anabios-core/src/biome.rs crates/anabios-core/tests/earth_worldgen.rs
git commit -m "feat(worldgen): BiomeField::from_earth — real Earth rasters via existing classify"
```

---

## Task 3: Scenario opt-in `world_map = "earth"`

**Purpose:** Let a scenario request the real map. When set, `instantiate` builds the biome via `from_earth`.

**Files:**
- Modify: `crates/anabios-core/src/scenario.rs`
- Modify: `crates/anabios-core/tests/earth_worldgen.rs`

**Interfaces:**
- Consumes: `BiomeField::from_earth` (Task 2).
- Produces: `pub enum WorldMapSource { Earth }` and `Scenario { world_map: Option<WorldMapSource>, .. }`, serde tag `world_map = "earth"`.

- [ ] **Step 1: Write the failing test** (append to `earth_worldgen.rs`):

```rust
use anabios_core::scenario::Scenario;

#[test]
fn scenario_world_map_earth_uses_from_earth() {
    let toml = r#"
name = "t"
seed = 1
world_size = 4096.0
biome_res = 256
hash_res = 256
world_map = "earth"
[[agents]]
count = 1
placement = { kind = "uniform" }
"#;
    let w = Scenario::parse_toml(toml).expect("parse").instantiate();
    assert_eq!(w.biome.res, 256);
    // Matches from_earth's real map: central Africa is land.
    let x = (20.0 + 180.0) / 360.0 * w.world_size;
    let y = (90.0 - 0.0) / 180.0 * w.world_size;
    let (col, row) = ((x / w.biome.cell_size) as usize, (y / w.biome.cell_size) as usize);
    assert_ne!(
        w.biome.cells[row * w.biome.res + col].terrain,
        anabios_core::biome::TerrainType::Water,
    );
}
```

- [ ] **Step 2: Run test to verify it fails.**

Run: `cargo test -p anabios-core --test earth_worldgen scenario_world_map_earth_uses_from_earth`
Expected: FAIL — unknown field `world_map` (serde `deny_unknown_fields`).

- [ ] **Step 3: Add the enum + field** in `crates/anabios-core/src/scenario.rs`. Near the other opt-in fields on the scenario struct:

```rust
/// Opt-in real-world biome source. Absent = the procedural climate pipeline.
/// `Earth` builds the field from the embedded 256x256 rasters via
/// `BiomeField::from_earth` (requires `biome_res == biome::EARTH_RES`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldMapSource {
    Earth,
}
```

Add to the scenario struct (with the other `#[serde(default)]` opt-ins):

```rust
    #[serde(default)]
    pub world_map: Option<WorldMapSource>,
```

- [ ] **Step 4: Wire it into `instantiate`.** In `crates/anabios-core/src/scenario.rs`, replace the existing climate-regen block so the real map takes precedence (climate knobs are meaningless against real data):

```rust
        match &self.world_map {
            Some(WorldMapSource::Earth) => {
                w.biome = crate::biome::BiomeField::from_earth(w.biome_res, w.world_size);
            }
            None => {
                if let Some(climate) = &self.climate {
                    w.biome = crate::biome::BiomeField::generate_with(
                        self.seed,
                        w.biome_res,
                        w.world_size,
                        &climate.resolve(),
                    );
                }
            }
        }
```

Note: `market_field` is sized from `w.biome.cells.len()` earlier; `from_earth` preserves `res*res`, so that stays correct.

- [ ] **Step 5: Run tests to verify they pass.**

Run: `cargo test -p anabios-core --test earth_worldgen`
Expected: PASS (all three tests).

- [ ] **Step 6: Guard the existing goldens (no regen expected).**

Run: `cargo test -p anabios-core --test determinism minimal_scenario_matches_golden_hashes`
Expected: PASS unchanged — adding an unused `Option` field and an enum must not move any existing world hash. If it fails, a shared path was touched; revert and re-scope.

- [ ] **Step 7: Commit.**

```bash
cargo fmt && cargo clippy -p anabios-core --tests
git add crates/anabios-core/src/scenario.rs crates/anabios-core/tests/earth_worldgen.rs
git commit -m "feat(worldgen): scenario world_map = \"earth\" routes biome gen through from_earth"
```

---

## Task 4: `Placement::Geo { lat, lon, radius }`

**Purpose:** Place founding lineages on real coordinates. Maps lat/lon to a sim cluster center via the equirectangular transform, then distributes exactly like `Cluster` (same RNG draws → determinism preserved).

**Files:**
- Modify: `crates/anabios-core/src/scenario.rs`
- Modify: `crates/anabios-core/tests/earth_worldgen.rs`

**Interfaces:**
- Produces: `Placement::Geo { lat: f32, lon: f32, radius: f32 }`, serde `kind = "geo"`.

- [ ] **Step 1: Write the failing test** (append to `earth_worldgen.rs`):

```rust
/// A geo placement lands agents near the mapped lat/lon; radius spread only.
#[test]
fn geo_placement_maps_latlon_to_position() {
    let toml = r#"
name = "t"
seed = 7
world_size = 4096.0
biome_res = 256
hash_res = 256
world_map = "earth"
[[agents]]
count = 200
placement = { kind = "geo", lat = 0.0, lon = 20.0, radius = 30.0 }
"#;
    let w = Scenario::parse_toml(toml).expect("parse").instantiate();
    let cx = (20.0 + 180.0) / 360.0 * w.world_size;
    let cy = (90.0 - 0.0) / 180.0 * w.world_size;
    // Single agent spec → ids 0..200 are the geo-placed founders.
    for id in 0..200usize {
        let p = w.agents.position[id];
        let d = ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt();
        assert!(d <= 30.0 + 1e-3, "agent {id} at distance {d} beyond geo radius");
    }
}
```

Accessor confirmed against existing tests (`crates/anabios-core/tests/dims.rs`): agent positions are `w.agents.position[id]` (a `Vec<Vec2>` of `x`/`y`); `Vec2` is `anabios_core::prelude::Vec2` if a direct import is needed elsewhere.

- [ ] **Step 2: Run test to verify it fails.**

Run: `cargo test -p anabios-core --test earth_worldgen geo_placement_maps_latlon_to_position`
Expected: FAIL — unknown placement variant `geo`.

- [ ] **Step 3: Add the variant** to the `Placement` enum in `crates/anabios-core/src/scenario.rs`:

```rust
    /// Cluster around a real-world lat/lon (equirectangular → sim coords),
    /// spread within `radius`. For real-map (`world_map`) scenarios.
    Geo { lat: f32, lon: f32, radius: f32 },
```

- [ ] **Step 4: Handle it in the spawn loop.** In `instantiate`'s `match spec.placement`, add an arm that mirrors `Cluster`'s draw order exactly (`theta` then `r`):

```rust
                    Placement::Geo { lat, lon, radius } => {
                        let center_x = (lon + 180.0) / 360.0 * w.world_size;
                        let center_y = (90.0 - lat) / 180.0 * w.world_size;
                        let theta = w.rng.f32_range(0.0, std::f32::consts::TAU);
                        let r = w.rng.f32_range(0.0, radius);
                        Vec2::new(
                            center_x + r * crate::mathf::cosf(theta),
                            center_y + r * crate::mathf::sinf(theta),
                        )
                    }
```

- [ ] **Step 5: Run tests to verify they pass.**

Run: `cargo test -p anabios-core --test earth_worldgen`
Expected: PASS (all four tests).

- [ ] **Step 6: Commit.**

```bash
cargo fmt && cargo clippy -p anabios-core --tests
git add crates/anabios-core/src/scenario.rs crates/anabios-core/tests/earth_worldgen.rs
git commit -m "feat(worldgen): Placement::Geo — real lat/lon placement on the Earth map"
```

---

## Task 5: `out-of-africa-earth.toml` + determinism round-trip

**Purpose:** The deliverable scenario, plus a save/load/step test proving it survives serialization (the every-opt-in-subsystem discipline).

**Files:**
- Create: `scenarios/out-of-africa-earth.toml`
- Modify: `crates/anabios-core/tests/determinism.rs`

- [ ] **Step 1: Write `scenarios/out-of-africa-earth.toml`.** Header comment (like the existing OoA files) describing the real-Earth premise. Full subsystem stack copied from `out-of-africa.toml`'s flags, plus:

```toml
name = "out-of-africa-earth"
seed = 318

# OUT OF AFRICA — REAL EARTH. The full out-of-africa subsystem stack on a real
# Earth map (world_map = "earth"): real elevation + temperature + precipitation
# rasters (assets/earth/, built by scripts/build_earth_map.py) drive the biome
# field through the existing Whittaker classify(). Founding lineages are placed
# on real coordinates with `geo`: the cognitive cradle in equatorial East
# Africa, archaics across Eurasia. The dispersal funnels through the real
# African exits (Sinai / Bab-el-Mandeb, Gibraltar). Equirectangular; E-W wraps
# (dateline), poles are ice at the y-edges.

max_population = 3000
world_size = 4096.0
biome_res = 256
hash_res = 256
world_map = "earth"

env_period = 400
climate_drift_rate = 0.00005
season_period = 2000
biome_adaptation = true
living_biome = true
nutrient_variation = true
soil_fertility = true
disasters_enabled = true
terrain_habitat = true
resources_enabled = true
settlement_enabled = true
inventions_enabled = true
gene_tech_coupling = true
cognition_enabled = true
war_enabled = true
sexual_dimorphism_enabled = true
domestication_enabled = true

# The Cradle — equatorial East Africa (lat 0, lon ~37). Foragers + innovators.
[[agents]]
count = 220
archetype = "asocial_forager"
placement = { kind = "geo", lat = 0.5, lon = 37.0, radius = 120.0 }
[agents.traits]
reproduction_threshold = 0.2
terrain_affinity = 0.12

[[agents]]
count = 40
archetype = "innovator"
placement = { kind = "geo", lat = 0.5, lon = 37.0, radius = 90.0 }
[agents.traits]
altruism = 0.3
basal_metabolism = 0.6
lifespan_bias = 1.0
openness = 1.0
reproduction_threshold = 0.3
sexual_dimorphism = 0.8
mate_choosiness = 0.6

# ... (port the remaining lineages from out-of-africa.toml, converting each
# cluster center to a `geo { lat, lon }` on real coordinates: Sahel/relay
# populations across North Africa, herds on the savanna, archaics across
# Eurasia — NW Europe, Central/East Asia. Keep counts/traits identical to the
# procedural scenario so only the geography differs.)
```

Convert every `out-of-africa.toml` lineage to a `geo` placement on a real coordinate. Use the inverse of the transform to sanity-check each: `lon = x/W*360 - 180`, `lat = 90 - y/W*180`. Keep all counts and traits identical to `out-of-africa.toml` so this scenario differs from it *only* in geography.

- [ ] **Step 2: Verify it parses + instantiates + steps.**

Run:
```bash
cargo run -p anabios-headless --release -- demo --scenario scenarios/out-of-africa-earth.toml --seed 318 --ticks 200 --report-every 100
```
Expected: no panic; a populated world stepping (agent counts, events). If `from_earth`'s `assert_eq!(res, EARTH_RES)` fires, the scenario's `biome_res` isn't 256 — fix the TOML.

- [ ] **Step 3: Write the failing determinism test** in `crates/anabios-core/tests/determinism.rs` (mirror `gene_tech_coupling_survives_save_load_step`):

```rust
/// The real-Earth out-of-africa scenario must survive a save→load→step
/// round-trip: from_earth's field + every opt-in subsystem, no hidden state.
#[test]
fn out_of_africa_earth_survives_save_load_step() {
    const OOA_EARTH: &str = include_str!("../../../scenarios/out-of-africa-earth.toml");
    let mut world = Scenario::parse_toml(OOA_EARTH).expect("parse ooa-earth").instantiate();
    assert_eq!(world.biome.res, 256, "scenario must use the earth map res");
    for _ in 0..300 {
        step(&mut world);
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&world), state_hash(&reloaded), "load must restore identical state");
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "ooa-earth diverged after save→load→step (hidden non-serialized state?)",
    );
}
```

- [ ] **Step 4: Run the round-trip test.**

Run: `cargo test -p anabios-core --test determinism out_of_africa_earth_survives_save_load_step`
Expected: PASS. (First run may be slow — grand-scale world × 300 ticks.)

- [ ] **Step 5: Commit.**

```bash
git add scenarios/out-of-africa-earth.toml crates/anabios-core/tests/determinism.rs
git commit -m "feat(scenario): out-of-africa-earth — full stack on the real Earth map"
```

---

## Task 6: Viewer verification + benchmark + finalize

**Purpose:** Confirm the real map renders and runs at watchable cost; tune founding counts if needed; finalize the scenario header.

**Files:**
- Modify (only if a rendering gap is found): `crates/anabios-godot/src/lib.rs` and/or the viewer field-atlas/minimap code.
- Modify (if tuning needed): `scenarios/out-of-africa-earth.toml`

- [ ] **Step 1: Headless smoke + rough benchmark.**

Run:
```bash
cargo run -p anabios-headless --release -- demo --scenario scenarios/out-of-africa-earth.toml --seed 318 --ticks 2000 --report-every 500
```
Expected: completes; note wall-clock/1000 ticks. If materially slower than `continental` at the same size, the cost is agent count, not the map — reduce founding counts / `max_population` and note it in the header (per the isolated-stage-bench discipline, don't trust dev-laptop tick timing for micro-claims; this is a coarse watchability check only).

- [ ] **Step 2: Visual check in the Godot viewer.** Launch the viewer on the scenario (per the project's run path) and confirm: real coastlines appear (recognizable Africa/Eurasia), the field atlas isn't torn (the square-atlas Metal fix holds at 256), and the world minimap shows the real landmasses with agents dotted on the cradle. Screenshot for the record.

- [ ] **Step 3: Fix only a confirmed rendering gap.** If (and only if) the atlas/minimap misrenders the real field, fix it in the viewer following the existing atlas/minimap patterns; otherwise no viewer change. Do not refactor working viewer code.

- [ ] **Step 4: Finalize the scenario header** with the measured honest expectations (population trajectory, where the exodus lands, which techs emerge vs. stall) — mirroring the "Honest expectations (measured on this build)" paragraph style of `out-of-africa.toml`. Explicitly note that emergent era-3 is the open question sub-project B probes; this scenario does not seed inventions.

- [ ] **Step 5: Full test sweep before wrap-up.**

Run: `cargo test -p anabios-core`
Expected: PASS (new tests green; goldens unchanged). Then `cargo fmt --check` and `cargo clippy --tests`.

- [ ] **Step 6: Commit.**

```bash
git add scenarios/out-of-africa-earth.toml   # + any viewer files actually changed
git commit -m "feat(scenario): finalize out-of-africa-earth header + verify render/perf"
```

---

## Sub-project B (follow-on cycle — NOT in this plan)

After this plan lands, start B as a **probe-first** cycle (its own brainstorm → probe → maybe spec), per the design doc §"Sub-project B". First measurement: does the real African cradle's geographic isolation (sea / Sahara / rift separating the cognitive lineages from the r-selected `asocial_forager`) let culture survive past era-1 and climb — sweeping 16 seeds × 20k ticks and deriving era-reached from `InventionDiscovered` events, compared against the documented 0/16 baseline. Spec a mechanism only if a probe shows signal; otherwise extend the findings doc with the negative result.

---

## Self-Review

**Spec coverage:**
- Data pipeline (spec §Architecture/Data flow) → Tasks 0, 1. ✓
- `from_earth` through existing arrays/classify, no new fields (spec §Components 1–2, §Determinism) → Task 2 + Global Constraints. ✓
- Scenario opt-in `world_map` (spec §Components 3) → Task 3. ✓
- `geo` placement (spec §Components 4) → Task 4. ✓
- New `out-of-africa-earth.toml` (spec §Components 5) → Task 5. ✓
- Projection/topology (spec §Projection) → encoded in the transform (Tasks 2–4) + scenario header (Task 5). ✓
- Determinism/goldens/FORMAT_VERSION (spec §Determinism) → Global Constraints + Task 3 Step 6 + Task 5 round-trip. ✓
- Viewer (spec §Viewer) → Task 6. ✓
- Risks: data acquisition (Task 0 + Task 1 synthetic fallback), licensing (Task 0/README), perf (Task 6), distortion (Task 5 header). ✓
- Sub-project B recorded, not built (spec §Sub-project B) → follow-on section. ✓

**Placeholder scan:** Core Rust tasks (2–4) carry full code + real tests. Task 1's synthetic `load_land_mask()` and Task 5's "port remaining lineages" are the two intentional data-acquisition/porting steps (raw map data + coordinate conversion done at implementation time, like fetched rasters) — both have explicit method + validation, not hand-waved logic. Task 4's test notes the two accessor names (`iter_positions`, `Vec2` path) to confirm against existing tests before writing.

**Type consistency:** `from_earth(res, world_size)`, `EARTH_RES`, `WorldMapSource::Earth`, `world_map: Option<WorldMapSource>`, `Placement::Geo { lat, lon, radius }`, the equirectangular transform (`x=(lon+180)/360·W`, `y=(90−lat)/180·W`), and `SEA_LEVEL`/`classify_with` are used identically across Tasks 2–5. Dequant `byte/255.0` and neutral nutrient/fertility `1.0` consistent with the real `BiomeCell` fields read from `biome.rs`.
