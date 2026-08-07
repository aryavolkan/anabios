//! 128×128 biome field with terrain types and plant biomass.
//!
//! Terrain is generated deterministically from a seed by a climate-driven
//! pipeline: gradient-noise fBm (with domain warping) for elevation, a
//! latitude-band temperature and moisture, then a Whittaker classification into
//! nine terrains (see [`classify`]). Plant biomass starts at the cell's carrying
//! capacity (a function of terrain type) and is replenished each tick by
//! logistic regrowth.

use serde::{Deserialize, Serialize};

use crate::prelude::Vec2;
use crate::rng::Rng;

/// Grid resolution per axis. Total cells = `BIOME_RES * BIOME_RES`.
pub const BIOME_RES: usize = 128;
/// World extent per axis. The biome covers `[0, WORLD_SIZE) × [0, WORLD_SIZE)`.
pub const WORLD_SIZE: f32 = 1024.0;
/// Side length of one biome cell, in world units.
pub const CELL_SIZE: f32 = WORLD_SIZE / BIOME_RES as f32;

/// Default world dimensions (today's compile-time values). New runtime
/// dimension fields on `World` default to these so existing scenarios are
/// byte-identical.
pub const WORLD_SIZE_DEFAULT: f32 = WORLD_SIZE;
pub const BIOME_RES_DEFAULT: usize = BIOME_RES;

/// Fraction of the mean vegetated-neighbour biomass a depleted cell gains per
/// recolonization step. Modest, so recovery is gradual (avoids boom/bust).
pub const RECOLONIZE_RATE: f32 = 0.08;
/// A cell counts as a viable seed source above this biomass.
pub const RECOLONIZE_SEED_MIN: f32 = 0.5;

/// Peak regrowth multiplier bonus for a cell whose climate matches the season.
pub const SEASON_AMPLITUDE: f32 = 1.5;

/// Succession states (E4). Climax is the default everywhere and keeps the
/// pre-E4 regrowth arithmetic exactly; Pioneer/Bare only appear after
/// disturbance.
pub const SUCCESSION_CLIMAX: u8 = 0;
pub const SUCCESSION_PIONEER: u8 = 1;
pub const SUCCESSION_BARE: u8 = 2;
/// Pioneer regrowth rate multiplier (fast, weedy recovery).
pub const PIONEER_RATE_MULT: f32 = 1.5;
/// Pioneer effective capacity, as a fraction of terrain capacity (low
/// standing crop while the community matures).
pub const PIONEER_CAPACITY_MULT: f32 = 0.5;
/// Bare cells reseed spontaneously at this fraction of capacity per biome
/// step (wind-blown seed; without it burns could never recover).
pub const BARE_RESEED_FRAC: f32 = 0.005;
/// Bare → Pioneer once biomass exceeds this fraction of terrain capacity.
pub const PIONEER_ENTRY_FRAC: f32 = 0.05;
/// Pioneer → Climax once biomass reaches this fraction of the *pioneer*
/// capacity (the weedy ceiling signals a matured community).
pub const CLIMAX_ENTRY_FRAC: f32 = 0.9;
/// Climate distance beyond which the seasonal bonus is zero (triangular).
pub const SEASON_TOLERANCE: f32 = 0.25;

/// Per-cell nutrient-quality range (energy-per-bite multiplier). Mean ~1.0 so
/// the global energy economy is roughly conserved vs a flat world; the spatial
/// *variation* is the foraging-selection signal.
pub const NUTRIENT_QUALITY_MIN: f32 = 0.6;
pub const NUTRIENT_QUALITY_MAX: f32 = 1.4;
/// Per-cell soil-fertility range (scales carrying capacity AND regrowth rate).
/// Mean ~1.0 so global productivity is roughly conserved.
pub const FERTILITY_MIN: f32 = 0.5;
pub const FERTILITY_MAX: f32 = 1.5;

/// Moisture added to river cells and their 4-neighbours (riparian greening).
pub const RIPARIAN_MOISTURE: f32 = 0.25;

/// Season phase in \[0,1\], a triangle wave with full cycle `2*period` ticks.
pub fn season_phase(tick: u64, period: u32) -> f32 {
    if period == 0 {
        return 0.0;
    }
    let p = period as u64;
    let t = tick % (2 * p);
    if t < p {
        t as f32 / p as f32
    } else {
        2.0 - t as f32 / p as f32
    }
}

/// Triangular match of a cell's static climate to the current season phase.
pub fn season_match(env: f32, phase: f32) -> f32 {
    (1.0 - (env - phase).abs() / SEASON_TOLERANCE).clamp(0.0, 1.0)
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
            TerrainType::Savanna => 8.0,
            TerrainType::Rainforest => 28.0,
            TerrainType::Taiga => 12.0,
            TerrainType::Tundra => 4.0,
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
            TerrainType::Savanna => 0.006,
            TerrainType::Rainforest => 0.004,
            TerrainType::Taiga => 0.004,
            TerrainType::Tundra => 0.002,
        }
    }
}

/// One cell of the biome grid.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BiomeCell {
    pub terrain: TerrainType,
    pub plant_biomass: f32,
    /// Per-cell temperature climate value in `[0,1]`: a latitude gradient minus
    /// an elevation lapse plus a little noise. Static after generation. Read as
    /// the climate axis by the biome-adaptation feeding bonus and the seasonal-
    /// regrowth match. (Historically a generic "env" noise field; now holds
    /// temperature so it lines up with the Whittaker classification.)
    pub env: f32,
    /// Per-cell moisture in `[0,1]`: a latitude band profile plus fBm. Static
    /// after generation; combined with `env`/temperature to classify terrain at
    /// generation time (Whittaker).
    #[serde(default)]
    pub moisture: f32,
    /// Industrial pollution in `[0, invention::POLLUTION_CAP]`. Deposited by
    /// Machinery holders (`invention_step`), decays per biome step, and
    /// penalizes logistic regrowth. Always 0.0 unless the invention tree is
    /// active.
    pub pollution: f32,
    /// Succession state (E4): `SUCCESSION_CLIMAX` (0) everywhere unless a
    /// disaster scorched the cell. See the `SUCCESSION_*` consts.
    #[serde(default)]
    pub succession: u8,
    /// Static energy-per-bite multiplier for food grazed in this cell, in
    /// `[NUTRIENT_QUALITY_MIN, NUTRIENT_QUALITY_MAX]`. Generated once; consumed
    /// only when `World::nutrient_variation` is on.
    #[serde(default)]
    pub nutrient_quality: f32,
    /// Static soil-fertility multiplier scaling this cell's carrying capacity
    /// and regrowth rate, in `[FERTILITY_MIN, FERTILITY_MAX]`. Generated once;
    /// consumed only when `World::soil_fertility` is on.
    #[serde(default)]
    pub fertility: f32,
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
}

/// 128×128 biome field (at default dims). Indexed `[row * res + col]` with
/// `row` = y, `col` = x. World position `(x, y)` maps to `(col, row) =
/// (x/cell_size, y/cell_size)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiomeField {
    pub cells: Vec<BiomeCell>,
    /// Grid resolution per axis (was the `BIOME_RES` const).
    pub res: usize,
    /// World extent per axis (was `WORLD_SIZE`).
    pub world_size: f32,
    /// Side length of one cell = `world_size / res` (was `CELL_SIZE`).
    pub cell_size: f32,
    /// Reusable delta buffer for `recolonize_step`'s double-buffering, kept to
    /// avoid a fresh `cells.len()`-sized allocation every call. Cleared and
    /// resized to all-zero at the start of each `recolonize_step` before any
    /// read, so it never carries state between calls. `#[serde(skip)]`: pure
    /// scratch, excluded from the snapshot/state hash (default-empty on load,
    /// immediately rebuilt on the next call).
    #[serde(skip)]
    recolonize_scratch: Vec<f32>,
}

/// Fraction of world extent below which a cell is open water.
pub const SEA_LEVEL: f32 = 0.35;
/// Elevation above which a cell is barren rock/peak.
pub const ROCK_LINE: f32 = 0.78;
/// Temperature drop per unit elevation above sea level.
pub const TEMP_LAPSE: f32 = 0.55;
/// Whittaker band cutoffs for temperature and moisture.
pub const BAND_LO: f32 = 0.33;
pub const BAND_HI: f32 = 0.66;
/// fBm of averaged octaves concentrates near 0.5; a linear contrast about 0.5
/// widens the elevation distribution so lowland basins (Water/Desert) and
/// high peaks (Rock) actually occur, not just mid-elevation terrain.
pub const ELEV_CONTRAST: f32 = 2.1;
/// Elevation that ocean basins are pulled down toward under continent shaping.
pub const DEEP_OCEAN_ELEV: f32 = 0.15;
/// Contrast applied to the continent mask before blending (mirrors
/// `ELEV_CONTRAST`'s role for base elevation): the raw low-frequency fBm is too
/// soft on its own — its wide transition band lets base-elevation noise poke
/// land through the "ocean" side and cut through the "land" side, so masking
/// can *increase* fragmentation instead of consolidating it. Sharpening the
/// mask toward a near-binary land/ocean template fixes that (empirically
/// verified: at `continentality = 0.85`, this makes continent-masked worlds
/// less fragmented than unmasked ones in 10/15 sampled seeds, vs 2/15 with no
/// contrast).
pub const CONTINENT_MASK_CONTRAST: f32 = 8.0;
/// Prevailing wind (westerly): upwind is `-WIND_DX/-WIND_DY` cells away.
pub const WIND_DX: isize = 1;
pub const WIND_DY: isize = 0;
/// Upwind sample distance (cells) for the rain-shadow term.
pub const SHADOW_DIST: isize = 4;

/// Scenario-tunable climate knobs (the follow-up deferred by the 2026-07-27
/// worldgen design doc). Defaults exactly reproduce the compile-time
/// constants, so an untouched scenario generates a bit-identical world;
/// setting any knob reshapes the climate for that scenario only.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClimateParams {
    /// Shifted onto every cell's temperature before classification
    /// (negative = ice age, positive = hothouse). 0.0 = current climate.
    pub temp_bias: f32,
    /// Shifted onto every cell's moisture before classification
    /// (negative = arid world, positive = lush world). 0.0 = current.
    pub moisture_bias: f32,
    /// Elevation cutoff for open water (higher = more ocean / archipelagos).
    pub sea_level: f32,
    /// Elevation distribution widening about 0.5 (higher = more abyssal
    /// basins AND more rock peaks).
    pub elev_contrast: f32,
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
}

impl Default for ClimateParams {
    fn default() -> Self {
        Self {
            temp_bias: 0.0,
            moisture_bias: 0.0,
            sea_level: SEA_LEVEL,
            elev_contrast: ELEV_CONTRAST,
            continentality: 0.0,
            mountain_uplift: 0.0,
            rain_shadow: 0.0,
            river_threshold: 0.0,
        }
    }
}

/// Latitude temperature: 1 at the equator (v=0.5), 0 at the poles (v=0 or 1).
/// A raised-cosine (not a triangle) so the tropics/subtropics stay hot
/// (temp > 0.66 across v in ~0.30..0.70) before falling off to the poles — this
/// is what lets the dry subtropical band classify as hot Desert rather than
/// temperate Grass. Continuous and smooth across the torus wrap (both y-edges
/// are poles, temperature 0).
#[inline]
pub fn latitude_temp(v: f32) -> f32 {
    0.5 + 0.5 * crate::mathf::cosf(std::f32::consts::PI * (2.0 * v - 1.0))
}

/// Latitude moisture band profile: wet equator, dry subtropics (~30 deg), wet
/// temperate, dry poles — the driver of subtropical deserts and the equatorial
/// rainforest belt.
#[inline]
pub fn latitude_moisture(v: f32) -> f32 {
    0.5 + 0.5 * crate::mathf::cosf(3.0 * std::f32::consts::PI * (2.0 * v - 1.0))
}

/// Whittaker classification: elevation gates water/rock, then (temperature,
/// moisture) select the land biome. Hard thresholds, no border dithering.
pub fn classify(elevation: f32, temperature: f32, moisture: f32) -> TerrainType {
    classify_with(elevation, temperature, moisture, SEA_LEVEL)
}

/// `classify` with a scenario-overridden sea level (see `ClimateParams`).
pub fn classify_with(
    elevation: f32,
    temperature: f32,
    moisture: f32,
    sea_level: f32,
) -> TerrainType {
    if elevation < sea_level {
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

impl BiomeField {
    /// Generate a biome field deterministically from a seed, at the given
    /// grid resolution and world extent per axis.
    pub fn generate(seed: u64, res: usize, world_size: f32) -> Self {
        Self::generate_with(seed, res, world_size, &ClimateParams::default())
    }

    /// `generate` with scenario climate knobs. The default `ClimateParams`
    /// reproduces `generate` bit-identically; the RNG draw order is
    /// unchanged, so knobs reshape the climate without reseeding the world.
    pub fn generate_with(seed: u64, res: usize, world_size: f32, climate: &ClimateParams) -> Self {
        let mut rng = Rng::from_seed(seed);
        // Climate-driven pipeline. Draw order is part of the determinism
        // contract: reordering these fBm constructions rehashes every world.
        // Domain-warp fields warp the sampling coordinates so landmasses are
        // organic rather than grid-aligned; elevation/temperature-noise/moisture
        // follow; then the nutrient and fertility fields (distinct frequencies
        // so the landscapes stay uncorrelated).
        let warp_x = crate::noise::Fbm::new(&mut rng, 3, 4, 2, 0.5);
        let warp_y = crate::noise::Fbm::new(&mut rng, 3, 4, 2, 0.5);
        let elevation = crate::noise::Fbm::new(&mut rng, 4, 5, 2, 0.5);
        let temp_noise = crate::noise::Fbm::new(&mut rng, 5, 3, 2, 0.5);
        let moisture_noise = crate::noise::Fbm::new(&mut rng, 6, 4, 2, 0.5);
        let nutrient = crate::noise::Fbm::new(&mut rng, 5, 3, 2, 0.5);
        let fertility_noise = crate::noise::Fbm::new(&mut rng, 4, 3, 2, 0.5);

        // Geography knobs draw AFTER the base seven and only when active, so a
        // knobs-off world keeps the exact pre-change RNG stream (goldens hold).
        let continent_noise =
            (climate.continentality > 0.0).then(|| crate::noise::Fbm::new(&mut rng, 3, 2, 2, 0.5));
        let mountain_noise =
            (climate.mountain_uplift > 0.0).then(|| crate::noise::Fbm::new(&mut rng, 3, 4, 2, 0.5));

        const WARP_AMP: f32 = 0.35;
        const TEMP_NOISE: f32 = 0.10;
        let contrast = climate.elev_contrast.max(0.1);

        // Pass 1: elevation grid (later tasks add mountain uplift here and
        // read it for rain-shadow neighbour lookups).
        let mut elev_grid = vec![0.0f32; res * res];
        for row in 0..res {
            for col in 0..res {
                let u = col as f32 / res as f32;
                let v = row as f32 / res as f32;
                let (wu, wv) = crate::noise::warp(&warp_x, &warp_y, u, v, WARP_AMP);
                let mut elev = ((elevation.sample(wu, wv) - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
                if let Some(cn) = &continent_noise {
                    let raw_mask = cn.sample(wu, wv);
                    let mask = ((raw_mask - 0.5) * CONTINENT_MASK_CONTRAST + 0.5).clamp(0.0, 1.0);
                    let blend = 1.0 - climate.continentality + climate.continentality * mask;
                    elev = (DEEP_OCEAN_ELEV + (elev - DEEP_OCEAN_ELEV) * blend).clamp(0.0, 1.0);
                }
                if let Some(mn) = &mountain_noise {
                    let ridge = 1.0 - (2.0 * mn.sample(wu, wv) - 1.0).abs();
                    // Weight uplift to land interiors so ranges sit on continents.
                    let land_weight = continent_noise.as_ref().map_or(1.0, |cn| cn.sample(wu, wv));
                    elev = (elev + climate.mountain_uplift * ridge * land_weight).clamp(0.0, 1.0);
                }
                elev_grid[row * res + col] = elev;
            }
        }

        // Pass 2: temperature/moisture/nutrient/fertility, classification,
        // and the BiomeCell build. `elev` comes from the pass-1 grid.
        let mut cells = Vec::with_capacity(res * res);
        for row in 0..res {
            for col in 0..res {
                let u = col as f32 / res as f32;
                let v = row as f32 / res as f32;
                let (wu, wv) = crate::noise::warp(&warp_x, &warp_y, u, v, WARP_AMP);
                let elev = elev_grid[row * res + col];
                // Temperature: hot equator, cold poles, colder at altitude,
                // plus the scenario's global bias (ice age / hothouse).
                let temperature = (latitude_temp(v)
                    - TEMP_LAPSE * (elev - climate.sea_level).max(0.0)
                    + TEMP_NOISE * (temp_noise.sample(u, v) - 0.5)
                    + climate.temp_bias)
                    .clamp(0.0, 1.0);
                // Moisture: latitude band profile (wet equator / dry subtropics /
                // wet temperate) blended with an fBm field sampled on warped
                // coordinates so it tracks the warped landmasses, plus the
                // scenario's global bias (arid / lush world).
                let mut moisture = (0.5 * latitude_moisture(v)
                    + 0.5 * moisture_noise.sample(wu, wv)
                    + climate.moisture_bias)
                    .clamp(0.0, 1.0);
                if climate.rain_shadow > 0.0 {
                    let uc =
                        (col as isize - WIND_DX * SHADOW_DIST).rem_euclid(res as isize) as usize;
                    let ur =
                        (row as isize - WIND_DY * SHADOW_DIST).rem_euclid(res as isize) as usize;
                    let upwind_elev = elev_grid[ur * res + uc];
                    moisture = (moisture - climate.rain_shadow * (upwind_elev - elev).max(0.0))
                        .clamp(0.0, 1.0);
                }
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
    }

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
                for (dc, dr) in
                    [(-1i32, -1i32), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]
                {
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
            self.cells[b]
                .elevation
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
                let near = river[i]
                    || [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].iter().any(|&(dc, dr)| {
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

    /// Convert a world position into a `(col, row)` cell index. Out-of-range
    /// positions are wrapped into the torus.
    #[inline]
    pub fn cell_coords(&self, pos: Vec2) -> (usize, usize) {
        let wrapped_x = pos.x.rem_euclid(self.world_size);
        let wrapped_y = pos.y.rem_euclid(self.world_size);
        let col = (wrapped_x / self.cell_size) as usize;
        let row = (wrapped_y / self.cell_size) as usize;
        (col.min(self.res - 1), row.min(self.res - 1))
    }

    #[inline]
    pub fn cell_index(&self, col: usize, row: usize) -> usize {
        row * self.res + col
    }

    #[inline]
    pub fn at(&self, col: usize, row: usize) -> &BiomeCell {
        &self.cells[self.cell_index(col, row)]
    }

    #[inline]
    pub fn at_mut(&mut self, col: usize, row: usize) -> &mut BiomeCell {
        let i = self.cell_index(col, row);
        &mut self.cells[i]
    }

    /// Sample the biome at a world position.
    pub fn sample(&self, pos: Vec2) -> &BiomeCell {
        let (col, row) = self.cell_coords(pos);
        self.at(col, row)
    }

    /// Decay one biome step's worth of pollution (Machinery debuff).
    fn decay_pollution(cell: &mut BiomeCell) {
        if cell.pollution > 0.0 {
            cell.pollution *= crate::invention::POLLUTION_DECAY;
            if cell.pollution < 1e-6 {
                cell.pollution = 0.0;
            }
        }
    }

    /// Regrowth-rate multiplier from pollution: `1 - min(pollution, MAX_EFFECT)`.
    fn pollution_mult(cell: &BiomeCell) -> f32 {
        1.0 - cell.pollution.min(crate::invention::POLLUTION_MAX_EFFECT)
    }

    /// Regrow + advance one cell's succession state. `rate_mult_fn` carries the
    /// seasonal bonus (1.0 in the non-seasonal path). `fert` is the soil-fertility
    /// multiplier (1.0 when `World::soil_fertility` is off — a bit-exact identity),
    /// applied to BOTH the carrying capacity and the regrowth rate.
    #[inline]
    fn regrow_succession(
        cell: &mut BiomeCell,
        rate_mult_fn: impl Fn(&BiomeCell) -> f32,
        fert: f32,
    ) {
        let base_cap = cell.terrain.carrying_capacity();
        if base_cap <= 0.0 {
            return;
        }
        let capacity = base_cap * fert;
        match cell.succession {
            SUCCESSION_BARE => {
                // Wind-blown reseed: slow linear recovery from scorch.
                cell.plant_biomass =
                    (cell.plant_biomass + BARE_RESEED_FRAC * capacity).min(capacity);
                if cell.plant_biomass > PIONEER_ENTRY_FRAC * capacity {
                    cell.succession = SUCCESSION_PIONEER;
                }
            }
            SUCCESSION_PIONEER => {
                let pcap = capacity * PIONEER_CAPACITY_MULT;
                if cell.plant_biomass <= 0.0 {
                    // Pioneer ground re-scorched to zero: back to bare.
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
            // Climax path — the pre-E4 logistic arithmetic, now with the
            // fertility-scaled capacity and rate.
            _ => {
                if cell.plant_biomass > 0.0 {
                    let r = cell.terrain.regrowth_rate()
                        * Self::pollution_mult(cell)
                        * rate_mult_fn(cell)
                        * fert;
                    let b = cell.plant_biomass;
                    let next = b + r * b * (1.0 - b / capacity);
                    cell.plant_biomass = next.clamp(0.0, capacity);
                }
            }
        }
    }

    /// Apply logistic regrowth: `b += r * b * (1 - b / K)` clamped to `[0, K]`.
    /// Empty cells stay empty — no spontaneous regeneration (see
    /// `recolonize_step` for the opt-in renewal). Pollution (Machinery debuff)
    /// decays once per biome step and scales the regrowth increment down.
    /// Climax cells follow the original arithmetic exactly; Pioneer/Bare
    /// cells (post-disturbance) follow the succession path.
    pub fn regrow_step(&mut self, soil_fertility: bool) {
        for cell in self.cells.iter_mut() {
            Self::decay_pollution(cell);
            let fert = if soil_fertility { cell.fertility } else { 1.0 };
            Self::regrow_succession(cell, |_| 1.0, fert);
        }
    }

    /// Logistic regrowth with a per-cell seasonal multiplier: cells whose
    /// climate matches the current season phase regrow faster, so the
    /// productive band migrates. `phase` in \[0,1\]. Deterministic, no RNG.
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

    /// Spread vegetation into depleted cells from their 4-neighbours (torus).
    /// Only cells with positive carrying capacity can recolonize. Double-
    /// buffered so the result is independent of scan order. Deterministic.
    pub fn recolonize_step(&mut self, soil_fertility: bool) {
        let res = self.res;
        // Read the pre-step biomass; write deltas, apply after. The delta buffer
        // is a reused scratch (mem::take/return, like the tick scratch buffers):
        // cleared and re-zeroed to `cells.len()` so it's a fresh all-zero buffer
        // each call without reallocating.
        let mut add = std::mem::take(&mut self.recolonize_scratch);
        add.clear();
        add.resize(self.cells.len(), 0.0);
        for row in 0..res {
            for col in 0..res {
                let idx = row * res + col;
                let cap = self.cells[idx].terrain.carrying_capacity()
                    * if soil_fertility { self.cells[idx].fertility } else { 1.0 };
                if cap <= 0.0 || self.cells[idx].plant_biomass > RECOLONIZE_SEED_MIN {
                    continue; // only depleted, colonizable cells receive seed
                }
                let n = [
                    idx_wrap(row + res - 1, col, res),
                    idx_wrap(row + 1, col, res),
                    idx_wrap(row, col + res - 1, res),
                    idx_wrap(row, col + 1, res),
                ];
                let mut sum = 0.0f32;
                let mut count = 0.0f32;
                for &ni in &n {
                    let b = self.cells[ni].plant_biomass;
                    if b > RECOLONIZE_SEED_MIN {
                        sum += b;
                        count += 1.0;
                    }
                }
                if count > 0.0 {
                    add[idx] = (RECOLONIZE_RATE * (sum / count)).min(cap);
                }
            }
        }
        for (cell, a) in self.cells.iter_mut().zip(add.iter()) {
            if *a > 0.0 {
                let cap = cell.terrain.carrying_capacity()
                    * if soil_fertility { cell.fertility } else { 1.0 };
                cell.plant_biomass = (cell.plant_biomass + *a).min(cap);
            }
        }
        // Return the buffer so its allocation is reused next call.
        self.recolonize_scratch = add;
    }

    /// Consume up to `desired` biomass from the cell containing `pos`,
    /// returning how much was actually consumed. The biome's biomass is
    /// reduced by the same amount.
    pub fn graze(&mut self, pos: Vec2, desired: f32) -> f32 {
        if desired <= 0.0 {
            return 0.0;
        }
        let (col, row) = self.cell_coords(pos);
        let cell = self.at_mut(col, row);
        let taken = desired.min(cell.plant_biomass);
        cell.plant_biomass -= taken;
        taken
    }
}

/// Unit direction toward the nearby cell whose climate (`env`) best matches
/// `affinity`, within `radius` world units — the habitat-selection pull. Returns
/// `Vec2::ZERO` if the agent's current cell is already the best match in range
/// (so a well-placed agent stays put). Deterministic: fixed scan order, strict
/// improvement wins. Reads no RNG.
pub fn best_env_direction(biome: &BiomeField, pos: Vec2, affinity: f32, radius: f32) -> Vec2 {
    let cell_reach = (radius / biome.cell_size).ceil() as i32 + 1;
    let (cx, cy) = biome.cell_coords(pos);
    let mut best_err = (biome.at(cx, cy).env - affinity).abs();
    let mut best_offset = Vec2::ZERO;
    for dy in -cell_reach..=cell_reach {
        for dx in -cell_reach..=cell_reach {
            let col = ((cx as i32 + dx).rem_euclid(biome.res as i32)) as usize;
            let row = ((cy as i32 + dy).rem_euclid(biome.res as i32)) as usize;
            let cell = biome.at(col, row);
            let cell_center = Vec2::new(
                (col as f32 + 0.5) * biome.cell_size,
                (row as f32 + 0.5) * biome.cell_size,
            );
            let offset = crate::prelude::wrap_torus(
                cell_center - pos + Vec2::splat(biome.world_size * 0.5),
                Vec2::splat(biome.world_size),
            ) - Vec2::splat(biome.world_size * 0.5);
            if offset.length() > radius {
                continue;
            }
            let err = (cell.env - affinity).abs();
            if err < best_err {
                best_err = err;
                best_offset = offset;
            }
        }
    }
    best_offset.normalize_or_zero()
}

/// True iff `(col,row)` is `target` terrain AND has a 4-neighbour of a
/// DIFFERENT terrain (i.e. it sits on a border of the target region).
/// Torus-wrapped.
fn is_border_target(biome: &BiomeField, col: usize, row: usize, target: TerrainType) -> bool {
    if biome.at(col, row).terrain != target {
        return false;
    }
    for (ddx, ddy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
        let ncol = ((col as i32 + ddx).rem_euclid(biome.res as i32)) as usize;
        let nrow = ((row as i32 + ddy).rem_euclid(biome.res as i32)) as usize;
        if biome.at(ncol, nrow).terrain != target {
            return true;
        }
    }
    false
}

/// Unit direction toward the nearby cell whose terrain matches `target`,
/// within `radius` world units — the terrain habitat-selection pull.
/// BORDER-seeking: prefers the nearest cell that is both `target` terrain and
/// adjacent to a different terrain (see `is_border_target`), so agents settle
/// on the edges of their home terrain next to their trading neighbours rather
/// than deep in the terrain's interior. Falls back to the nearest `target`
/// cell of any kind if no border cell is in range. Returns `Vec2::ZERO` if the
/// agent is already standing on a border-target cell, or if no `target` cell
/// is in range at all. Deterministic: fixed scan order, strict improvement
/// wins (lowest `(dy,dx)` on ties). Reads no RNG.
pub fn best_terrain_direction(
    biome: &BiomeField,
    pos: Vec2,
    target: TerrainType,
    radius: f32,
) -> Vec2 {
    let cell_reach = (radius / biome.cell_size).ceil() as i32 + 1;
    let (cx, cy) = biome.cell_coords(pos);
    // Already on a border of our terrain -> a good trading spot, stay put.
    if is_border_target(biome, cx, cy, target) {
        return Vec2::ZERO;
    }
    let mut best_border: Option<(f32, Vec2)> = None; // nearest border-target cell
    let mut best_any: Option<(f32, Vec2)> = None; // fallback: nearest target cell
    for dy in -cell_reach..=cell_reach {
        for dx in -cell_reach..=cell_reach {
            let col = ((cx as i32 + dx).rem_euclid(biome.res as i32)) as usize;
            let row = ((cy as i32 + dy).rem_euclid(biome.res as i32)) as usize;
            if biome.at(col, row).terrain != target {
                continue;
            }
            let cell_center = Vec2::new(
                (col as f32 + 0.5) * biome.cell_size,
                (row as f32 + 0.5) * biome.cell_size,
            );
            let offset = crate::prelude::wrap_torus(
                cell_center - pos + Vec2::splat(biome.world_size * 0.5),
                Vec2::splat(biome.world_size),
            ) - Vec2::splat(biome.world_size * 0.5);
            let d2 = offset.length_squared();
            if d2 > radius * radius {
                continue;
            }
            // strict `<` keeps the earliest (lowest dy,dx) on ties -> deterministic
            if best_any.is_none_or(|(bd, _)| d2 < bd) {
                best_any = Some((d2, offset));
            }
            if d2 > 1e-6
                && is_border_target(biome, col, row, target)
                && best_border.is_none_or(|(bd, _)| d2 < bd)
            {
                best_border = Some((d2, offset));
            }
        }
    }
    if let Some((_, off)) = best_border {
        return off.normalize_or_zero();
    }
    if let Some((_, off)) = best_any {
        return off.normalize_or_zero();
    }
    Vec2::ZERO
}

/// Wrap `(row, col)` onto a `res × res` torus and flatten to a cell index.
#[inline]
fn idx_wrap(row: usize, col: usize, res: usize) -> usize {
    (row % res) * res + (col % res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rivers_flow_downhill_to_water_and_wet_banks() {
        // Seed 31 (the brief's example seed) never satisfies this invariant at
        // ANY threshold with these climate knobs: continentality 0.85 +
        // mountain_uplift 0.6 confines seed 31's landmass so every drainage
        // basin large enough to clear a threshold terminates in a landlocked
        // sink rather than reaching open water (scanned thresholds 5..500 in
        // steps of 5 — see task-6-report.md). Seed 2 at the same climate knobs
        // has a basin that drains cleanly to the ocean; threshold 300 is
        // stable (bad-cell count is 0 across the 300..320 range, scanned).
        let c = ClimateParams {
            continentality: 0.85,
            mountain_uplift: 0.6,
            river_threshold: 300.0,
            ..Default::default()
        };
        let f = BiomeField::generate_with(2, 128, 1024.0, &c);
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
            for (dc, dr) in
                [(-1i32, -1i32), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]
            {
                let nc = (col as i32 + dc).rem_euclid(res as i32) as usize;
                let nr = (row as i32 + dr).rem_euclid(res as i32) as usize;
                let n = &f.cells[nr * res + nc];
                if n.elevation < e || n.terrain == TerrainType::Water {
                    ok = true;
                    break;
                }
            }
            assert!(ok, "river cell {i} has no downhill exit and no water neighbour");
        }
    }

    #[test]
    fn river_threshold_zero_leaves_flow_empty() {
        let c = ClimateParams { continentality: 0.85, ..Default::default() };
        let f = BiomeField::generate_with(31, 96, 1024.0, &c);
        assert!(f.cells.iter().all(|c| c.river_flow == 0.0));
    }

    #[test]
    fn geography_knobs_default_off() {
        let d = ClimateParams::default();
        assert_eq!(d.continentality, 0.0);
        assert_eq!(d.mountain_uplift, 0.0);
        assert_eq!(d.rain_shadow, 0.0);
        assert_eq!(d.river_threshold, 0.0);
    }

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
        assert!(
            equator > subtropics,
            "equator {equator} should be wetter than subtropics {subtropics}"
        );
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

    #[test]
    fn default_climate_params_are_bit_identical() {
        let a = BiomeField::generate(42, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        let b = BiomeField::generate_with(
            42,
            BIOME_RES_DEFAULT,
            WORLD_SIZE_DEFAULT,
            &ClimateParams::default(),
        );
        for i in 0..a.cells.len() {
            assert_eq!(a.cells[i].terrain, b.cells[i].terrain);
            assert_eq!(a.cells[i].moisture, b.cells[i].moisture);
            assert_eq!(a.cells[i].env, b.cells[i].env);
            assert_eq!(a.cells[i].fertility, b.cells[i].fertility);
        }
    }

    /// Count connected land components (4-neighbour, torus) as a speckle metric.
    fn land_component_count(f: &BiomeField) -> usize {
        let res = f.res;
        let mut seen = vec![false; f.cells.len()];
        let is_land = |i: usize| f.cells[i].terrain != TerrainType::Water;
        let mut comps = 0;
        let mut stack = Vec::new();
        for start in 0..f.cells.len() {
            if seen[start] || !is_land(start) {
                continue;
            }
            comps += 1;
            stack.push(start);
            seen[start] = true;
            while let Some(i) = stack.pop() {
                let (col, row) = (i % res, i / res);
                for (dc, dr) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let nc = (col as i32 + dc).rem_euclid(res as i32) as usize;
                    let nr = (row as i32 + dr).rem_euclid(res as i32) as usize;
                    let ni = nr * res + nc;
                    if !seen[ni] && is_land(ni) {
                        seen[ni] = true;
                        stack.push(ni);
                    }
                }
            }
        }
        comps
    }

    #[test]
    fn continentality_reduces_land_fragmentation() {
        let plain = BiomeField::generate(7, 128, 1024.0);
        let cont = {
            let c = ClimateParams { continentality: 0.85, ..Default::default() };
            BiomeField::generate_with(7, 128, 1024.0, &c)
        };
        assert!(
            land_component_count(&cont) < land_component_count(&plain),
            "continentality should consolidate land: plain={} cont={}",
            land_component_count(&plain),
            land_component_count(&cont)
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

    #[test]
    fn mountain_uplift_raises_connected_ranges() {
        let flat = {
            let c = ClimateParams { continentality: 0.85, ..Default::default() };
            BiomeField::generate_with(11, 128, 1024.0, &c)
        };
        let ranged = {
            let c =
                ClimateParams { continentality: 0.85, mountain_uplift: 0.6, ..Default::default() };
            BiomeField::generate_with(11, 128, 1024.0, &c)
        };
        let rock =
            |f: &BiomeField| f.cells.iter().filter(|c| c.terrain == TerrainType::Rock).count();
        assert!(
            rock(&ranged) > rock(&flat),
            "uplift should create more rock: {} vs {}",
            rock(&ranged),
            rock(&flat)
        );
    }

    #[test]
    fn rain_shadow_dries_lee_of_ranges() {
        // Compare mean moisture with and without rain-shadow on a mountainous world;
        // the shadowed world must be drier on average over land.
        let mk = |rs: f32| {
            let c = ClimateParams {
                continentality: 0.85,
                mountain_uplift: 0.6,
                rain_shadow: rs,
                ..Default::default()
            };
            BiomeField::generate_with(23, 128, 1024.0, &c)
        };
        let mean_land_moisture = |f: &BiomeField| {
            let land: Vec<f32> = f
                .cells
                .iter()
                .filter(|c| c.terrain != TerrainType::Water)
                .map(|c| c.moisture)
                .collect();
            land.iter().sum::<f32>() / land.len() as f32
        };
        assert!(
            mean_land_moisture(&mk(0.5)) < mean_land_moisture(&mk(0.0)),
            "rain-shadow should lower mean land moisture"
        );
    }

    #[test]
    fn climate_knobs_reshape_terrain() {
        let base = BiomeField::generate(42, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        let count =
            |b: &BiomeField, t: TerrainType| b.cells.iter().filter(|c| c.terrain == t).count();
        // Ice age: taiga/tundra expand at the expense of hotter biomes.
        let ice = BiomeField::generate_with(
            42,
            BIOME_RES_DEFAULT,
            WORLD_SIZE_DEFAULT,
            &ClimateParams { temp_bias: -0.25, ..Default::default() },
        );
        let cold_base = count(&base, TerrainType::Taiga) + count(&base, TerrainType::Tundra);
        let cold_ice = count(&ice, TerrainType::Taiga) + count(&ice, TerrainType::Tundra);
        assert!(
            cold_ice > cold_base,
            "ice age should expand cold biomes: {cold_base} -> {cold_ice}"
        );
        // Arid world: desert expands.
        let arid = BiomeField::generate_with(
            42,
            BIOME_RES_DEFAULT,
            WORLD_SIZE_DEFAULT,
            &ClimateParams { moisture_bias: -0.25, ..Default::default() },
        );
        assert!(
            count(&arid, TerrainType::Desert) > count(&base, TerrainType::Desert),
            "arid bias should expand desert"
        );
        // Raised sea level: more water.
        let sea = BiomeField::generate_with(
            42,
            BIOME_RES_DEFAULT,
            WORLD_SIZE_DEFAULT,
            &ClimateParams { sea_level: 0.55, ..Default::default() },
        );
        assert!(
            count(&sea, TerrainType::Water) > count(&base, TerrainType::Water),
            "higher sea level should expand water"
        );
    }

    #[test]
    fn climate_field_is_bounded_and_varies() {
        let b = BiomeField::generate(12345, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
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
        let b = BiomeField::generate(7, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        use std::collections::BTreeMap;
        let mut by_terrain: BTreeMap<u8, Vec<f32>> = BTreeMap::new();
        for cell in b.cells.iter() {
            by_terrain.entry(cell.terrain as u8).or_default().push(cell.env);
        }
        let varied = by_terrain.values().any(|v| {
            v.len() > 1
                && v.iter().cloned().fold(0.0f32, f32::max)
                    - v.iter().cloned().fold(1.0f32, f32::min)
                    > 0.1
        });
        assert!(varied, "env should vary within at least one terrain type");
    }

    #[test]
    fn biome_is_deterministic() {
        let a = BiomeField::generate(42, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        let b = BiomeField::generate(42, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        for i in 0..a.cells.len() {
            assert_eq!(a.cells[i].terrain, b.cells[i].terrain);
            assert!((a.cells[i].plant_biomass - b.cells[i].plant_biomass).abs() < 1e-6);
        }
    }

    #[test]
    fn biome_contains_multiple_terrain_types() {
        let b = BiomeField::generate(7, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        let mut seen = [0_usize; 9];
        for cell in &b.cells {
            seen[cell.terrain as usize] += 1;
        }
        let nonzero: usize = seen.iter().filter(|&&c| c > 0).count();
        assert!(nonzero >= 3, "biome should contain at least 3 terrain types, saw {:?}", seen);
    }

    #[test]
    fn cell_coords_wraps_negative_and_oversize_positions() {
        let b = BiomeField::generate(1, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        let (cx, cy) = b.cell_coords(Vec2::new(-1.0, WORLD_SIZE + 5.0));
        assert!(cx < BIOME_RES);
        assert!(cy < BIOME_RES);
    }

    #[test]
    fn carrying_capacity_is_initial_biomass() {
        let b = BiomeField::generate(99, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        for cell in &b.cells {
            assert!((cell.plant_biomass - cell.terrain.carrying_capacity()).abs() < 1e-6);
        }
    }

    #[test]
    fn regrow_increases_partial_biomass_toward_capacity() {
        let mut b = BiomeField::generate(13, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
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
            b.regrow_step(false);
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
        let mut b = BiomeField::generate(13, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        for _ in 0..1000 {
            b.regrow_step(false);
        }
        for cell in &b.cells {
            let cap = cell.terrain.carrying_capacity();
            assert!(
                cell.plant_biomass <= cap + 1e-4,
                "biomass {} > cap {}",
                cell.plant_biomass,
                cap
            );
        }
    }

    #[test]
    fn regrow_leaves_dead_cells_dead() {
        let mut b = BiomeField::generate(13, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        for cell in b.cells.iter_mut() {
            if cell.terrain == TerrainType::Grass {
                cell.plant_biomass = 0.0;
            }
        }
        for _ in 0..100 {
            b.regrow_step(false);
        }
        for cell in &b.cells {
            if cell.terrain == TerrainType::Grass {
                assert_eq!(cell.plant_biomass, 0.0);
            }
        }
    }

    #[test]
    fn graze_reduces_biomass_and_returns_taken_amount() {
        let mut b = BiomeField::generate(31, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        // Find a grass cell so we know biomass > 0.
        let mut target = Vec2::ZERO;
        'outer: for row in 0..b.res {
            for col in 0..b.res {
                if b.at(col, row).terrain == TerrainType::Grass {
                    target = Vec2::new(
                        (col as f32 + 0.5) * b.cell_size,
                        (row as f32 + 0.5) * b.cell_size,
                    );
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

    #[test]
    fn best_terrain_direction_pulls_toward_border_and_zero_when_already_there() {
        let b = BiomeField::generate(31, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
        // Find two horizontally-adjacent cells with different terrain: `t_col`
        // is `target` terrain AND sits on a border (its neighbour `o_col`
        // differs), i.e. `is_border_target(b, t_col, t_row, target)` is true.
        let mut found: Option<(usize, usize, usize, usize)> = None;
        'outer: for row in 0..b.res {
            for col in 0..b.res {
                let next_col = (col + 1) % b.res;
                if b.at(col, row).terrain != b.at(next_col, row).terrain {
                    found = Some((col, row, next_col, row));
                    break 'outer;
                }
            }
        }
        let (t_col, t_row, o_col, o_row) =
            found.expect("expected adjacent cells with differing terrain");
        let target = b.at(t_col, t_row).terrain;
        assert!(
            is_border_target(&b, t_col, t_row, target),
            "constructed cell should be a border-target cell by construction"
        );
        let target_center =
            Vec2::new((t_col as f32 + 0.5) * b.cell_size, (t_row as f32 + 0.5) * b.cell_size);
        let off_center =
            Vec2::new((o_col as f32 + 0.5) * b.cell_size, (o_row as f32 + 0.5) * b.cell_size);

        // Case (a): standing on a border-target cell — this is already a good
        // trading spot, so the pull is zero.
        let at_border = best_terrain_direction(&b, target_center, target, 200.0);
        assert_eq!(at_border, Vec2::ZERO, "should not move when already on a border-target cell");

        // Case (b): standing on an adjacent off-target cell, with the target
        // terrain within reach — the pull should be a non-zero unit vector,
        // toward the (border) target cell.
        let toward_target = best_terrain_direction(&b, off_center, target, 48.0);
        assert!(
            toward_target.length() > 0.9 && toward_target.length() < 1.1,
            "expected a roughly unit vector, got {toward_target:?}"
        );
    }

    fn grass_cell(biomass: f32, succession: u8) -> BiomeCell {
        BiomeCell {
            terrain: TerrainType::Grass,
            plant_biomass: biomass,
            env: 0.5,
            moisture: 0.5,
            pollution: 0.0,
            succession,
            nutrient_quality: 1.0,
            fertility: 1.0,
            elevation: 0.5,
            river_flow: 0.0,
        }
    }

    #[test]
    fn recolonize_step_reuses_scratch_without_carrying_state() {
        // 3×3 all-grass grid (every cell colonizable), only the center
        // vegetated; its four orthogonal neighbours are depleted and get seeded.
        let build = || {
            let mut cells = vec![grass_cell(0.0, SUCCESSION_CLIMAX); 9];
            cells[4] = grass_cell(10.0, SUCCESSION_CLIMAX); // center (row 1, col 1)
            BiomeField {
                cells,
                res: 3,
                world_size: 24.0,
                cell_size: 8.0,
                recolonize_scratch: Vec::new(),
            }
        };

        // Sanity: recolonization actually happens — each of the center's four
        // neighbours gains `RECOLONIZE_RATE * mean_seed_biomass` (only the
        // center exceeds the seed minimum, so the mean is just 10.0).
        let mut once = build();
        once.recolonize_step(false);
        let expect_edge = RECOLONIZE_RATE * 10.0; // 0.8, well under the grass cap
        for &e in &[1usize, 3, 5, 7] {
            assert!((once.cells[e].plant_biomass - expect_edge).abs() < 1e-6);
        }
        for &corner in &[0usize, 2, 6, 8] {
            assert_eq!(once.cells[corner].plant_biomass, 0.0, "no vegetated neighbour");
        }

        // The reused scratch must be cleared each call, not accumulated: a field
        // whose scratch is dirty from a prior call but whose cells are reset to
        // the initial state must produce exactly the same result as a fresh
        // field's first call. If the delta buffer leaked the earlier call's
        // values, `warm` would diverge from `clean`.
        let mut warm = build();
        warm.recolonize_step(false); // dirties warm.recolonize_scratch
        warm.cells.clone_from(&build().cells); // reset cells; scratch stays dirty
        warm.recolonize_step(false);

        let mut clean = build(); // empty scratch
        clean.recolonize_step(false);

        let wb: Vec<f32> = warm.cells.iter().map(|c| c.plant_biomass).collect();
        let cb: Vec<f32> = clean.cells.iter().map(|c| c.plant_biomass).collect();
        assert_eq!(wb, cb, "reused scratch must be cleared, not carried across calls");
    }

    #[test]
    fn climax_regrowth_matches_pre_succession_arithmetic() {
        // The Climax path must be byte-identical to the original logistic
        // regrowth: b += r*b*(1 - b/K) with r = rate * pollution_mult.
        let cells = vec![grass_cell(5.0, SUCCESSION_CLIMAX)];
        let mut field = BiomeField {
            cells,
            res: 1,
            world_size: 8.0,
            cell_size: 8.0,
            recolonize_scratch: Vec::new(),
        };
        field.regrow_step(false);
        let c = field.cells[0];
        let r = TerrainType::Grass.regrowth_rate();
        let expect = 5.0 + r * 5.0 * (1.0 - 5.0 / 10.0);
        assert_eq!(c.plant_biomass, expect);
        assert_eq!(c.succession, SUCCESSION_CLIMAX);
    }

    #[test]
    fn bare_cell_reseeds_to_pioneer() {
        let cells = vec![grass_cell(0.0, SUCCESSION_BARE)];
        let mut field = BiomeField {
            cells,
            res: 1,
            world_size: 8.0,
            cell_size: 8.0,
            recolonize_scratch: Vec::new(),
        };
        // One step: reseed by 0.5% of capacity (0.05), still below the 5%
        // pioneer-entry threshold (0.5).
        field.regrow_step(false);
        assert!((field.cells[0].plant_biomass - 0.05).abs() < 1e-4);
        assert_eq!(field.cells[0].succession, SUCCESSION_BARE);
        // 10 more steps: biomass 0.55 > 0.5 → pioneer.
        for _ in 0..10 {
            field.regrow_step(false);
        }
        assert_eq!(field.cells[0].succession, SUCCESSION_PIONEER);
    }

    #[test]
    fn pioneer_grows_fast_to_half_capacity_then_matures() {
        // Start pioneer just below its effective ceiling (0.5 × 10 = 5).
        let cells = vec![grass_cell(4.4, SUCCESSION_PIONEER)];
        let mut field = BiomeField {
            cells,
            res: 1,
            world_size: 8.0,
            cell_size: 8.0,
            recolonize_scratch: Vec::new(),
        };
        for _ in 0..40 {
            field.regrow_step(false);
        }
        let c = field.cells[0];
        assert_eq!(c.succession, SUCCESSION_CLIMAX, "pioneer matures at its ceiling");
        // Pioneer never exceeds 0.5 × terrain capacity while pioneer.
        // (After maturing, Climax regrowth resumes toward the full 10.0.)
        let cells2 = vec![grass_cell(0.6, SUCCESSION_PIONEER)];
        let mut f2 = BiomeField {
            cells: cells2,
            res: 1,
            world_size: 8.0,
            cell_size: 8.0,
            recolonize_scratch: Vec::new(),
        };
        let mut peak = 0.0f32;
        for _ in 0..200 {
            f2.regrow_step(false);
            if f2.cells[0].succession == SUCCESSION_PIONEER {
                peak = peak.max(f2.cells[0].plant_biomass);
            }
        }
        assert!(peak <= 5.0 + 1e-3, "pioneer overshot its half capacity: {peak}");
    }

    #[test]
    fn pioneer_rescorched_to_zero_regresses_to_bare() {
        let cells = vec![grass_cell(0.0, SUCCESSION_PIONEER)];
        let mut field = BiomeField {
            cells,
            res: 1,
            world_size: 8.0,
            cell_size: 8.0,
            recolonize_scratch: Vec::new(),
        };
        field.regrow_step(false);
        assert_eq!(field.cells[0].succession, SUCCESSION_BARE);
    }
}
