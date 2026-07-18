//! Per-channel pheromone fields: 128×128 grids (one value per channel per cell)
//! that agents with a `Pheromone` module deposit into and `Smell`-sensored
//! agents read. Fields decay exponentially each tick (design §3.6, §3.7 step 9).

use serde::{Deserialize, Serialize};

use crate::biome::{BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT};
use crate::prelude::Vec2;
use crate::program::PHEROMONE_CHANNELS;

/// Fraction of every cell's pheromone that dissipates each tick.
pub const PHEROMONE_DECAY: f32 = 0.05;
/// `emit_intent[ch]` above this triggers a deposit that tick.
pub const PHEROMONE_EMIT_THRESHOLD: f32 = 0.5;
/// Scales `emit_intent * strength` into deposited concentration.
pub const PHEROMONE_DEPOSIT_SCALE: f32 = 1.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PheromoneField {
    /// Row-major `res*res` cells; each holds one value per channel.
    pub cells: Vec<[f32; PHEROMONE_CHANNELS]>,
    /// Grid resolution per axis (was the `BIOME_RES` const). Kept in step
    /// with the world's `biome_res` by `World::with_dims`.
    pub res: usize,
    /// World extent per axis (was the `WORLD_SIZE` const). Kept in step with
    /// the world's `world_size` by `World::with_dims` so cell math scales
    /// with the torus instead of always assuming the default 1024.
    pub world_size: f32,
    /// Cached "any cell is nonzero" flag — lets `decay_step` skip the full
    /// 65k-multiply pass when no pheromone has ever been deposited. Once true
    /// it stays true (decay only shrinks values toward zero, never reaching
    /// it). Skipped by serde; recomputed on snapshot load.
    #[serde(skip)]
    nonzero: bool,
}

impl Default for PheromoneField {
    fn default() -> Self {
        Self::new()
    }
}

impl PheromoneField {
    pub fn new() -> Self {
        Self::with_dims(BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT)
    }

    /// Build an empty pheromone grid at the given resolution per axis.
    /// Assumes the default world size (`WORLD_SIZE_DEFAULT`); use
    /// `with_dims` when the torus extent differs.
    pub fn with_res(res: usize) -> Self {
        Self::with_dims(res, WORLD_SIZE_DEFAULT)
    }

    /// Build an empty pheromone grid at the given resolution per axis and
    /// world extent per axis.
    pub fn with_dims(res: usize, world_size: f32) -> Self {
        Self { cells: vec![[0.0; PHEROMONE_CHANNELS]; res * res], res, world_size, nonzero: false }
    }

    /// Convert a world position into a flat cell index. Self-contained (no
    /// `BiomeField` dependency): wraps into `[0, world_size)` on each axis and
    /// scales by this field's own resolution, clamping so the stride
    /// (`row * self.res + col`) always stays in bounds regardless of `res`.
    #[inline]
    fn idx(&self, pos: Vec2) -> usize {
        let cell_size = self.world_size / self.res as f32;
        let wrapped_x = pos.x.rem_euclid(self.world_size);
        let wrapped_y = pos.y.rem_euclid(self.world_size);
        let col = ((wrapped_x / cell_size) as usize).min(self.res - 1);
        let row = ((wrapped_y / cell_size) as usize).min(self.res - 1);
        row * self.res + col
    }

    /// Add `amount` to the cell at `pos` on `channel` (index clamped).
    pub fn deposit(&mut self, pos: Vec2, channel: usize, amount: f32) {
        let ch = channel.min(PHEROMONE_CHANNELS - 1);
        let i = self.idx(pos);
        self.cells[i][ch] += amount;
        self.nonzero = true;
    }

    /// Read the concentration at `pos` on `channel` (index clamped).
    pub fn sample(&self, pos: Vec2, channel: usize) -> f32 {
        let ch = channel.min(PHEROMONE_CHANNELS - 1);
        self.cells[self.idx(pos)][ch]
    }

    /// Recompute the `nonzero` cache. Called after snapshot load, where the
    /// serde-skipped flag defaults to false but cells may hold values.
    pub(crate) fn refresh_nonzero(&mut self) {
        self.nonzero = self.cells.iter().any(|c| c.iter().any(|&v| v != 0.0));
    }

    /// Exponential per-tick decay across every cell and channel.
    pub fn decay_step(&mut self) {
        if !self.nonzero {
            return;
        }
        let keep = 1.0 - PHEROMONE_DECAY;
        for cell in self.cells.iter_mut() {
            for v in cell.iter_mut() {
                *v *= keep;
            }
        }
    }
}
