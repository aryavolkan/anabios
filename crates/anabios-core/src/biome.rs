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
        let coarse = NoiseGrid::new(&mut rng, 8);
        let fine = NoiseGrid::new(&mut rng, 24);

        let mut cells = Vec::with_capacity(BIOME_RES * BIOME_RES);
        for row in 0..BIOME_RES {
            for col in 0..BIOME_RES {
                let u = col as f32 / BIOME_RES as f32;
                let v = row as f32 / BIOME_RES as f32;
                let n = 0.65 * coarse.sample(u, v) + 0.35 * fine.sample(u, v);
                let terrain = elevation_to_terrain(n);
                cells.push(BiomeCell { terrain, plant_biomass: terrain.carrying_capacity() });
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
    // NOTE: constructor renamed from `sample` to `new` because Rust disallows
    // two items with the same name in one `impl` block (E0592). The method
    // `sample(u, v)` keeps its planned name.
    fn new(rng: &mut Rng, cells_per_axis: usize) -> Self {
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
