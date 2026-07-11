//! Per-channel pheromone fields: 128×128 grids (one value per channel per cell)
//! that agents with a `Pheromone` module deposit into and `Smell`-sensored
//! agents read. Fields decay exponentially each tick (design §3.6, §3.7 step 9).

use serde::{Deserialize, Serialize};

use crate::biome::{BiomeField, BIOME_RES};
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
    /// Row-major `BIOME_RES*BIOME_RES` cells; each holds one value per channel.
    pub cells: Vec<[f32; PHEROMONE_CHANNELS]>,
}

impl Default for PheromoneField {
    fn default() -> Self {
        Self::new()
    }
}

impl PheromoneField {
    pub fn new() -> Self {
        Self { cells: vec![[0.0; PHEROMONE_CHANNELS]; BIOME_RES * BIOME_RES] }
    }

    #[inline]
    fn idx(pos: Vec2) -> usize {
        let (col, row) = BiomeField::cell_coords(pos);
        BiomeField::cell_index(col, row)
    }

    /// Add `amount` to the cell at `pos` on `channel` (index clamped).
    pub fn deposit(&mut self, pos: Vec2, channel: usize, amount: f32) {
        let ch = channel.min(PHEROMONE_CHANNELS - 1);
        self.cells[Self::idx(pos)][ch] += amount;
    }

    /// Read the concentration at `pos` on `channel` (index clamped).
    pub fn sample(&self, pos: Vec2, channel: usize) -> f32 {
        let ch = channel.min(PHEROMONE_CHANNELS - 1);
        self.cells[Self::idx(pos)][ch]
    }

    /// Exponential per-tick decay across every cell and channel.
    pub fn decay_step(&mut self) {
        let keep = 1.0 - PHEROMONE_DECAY;
        for cell in self.cells.iter_mut() {
            for v in cell.iter_mut() {
                *v *= keep;
            }
        }
    }
}
