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
        Self { cells: vec![[0.0; PHEROMONE_CHANNELS]; BIOME_RES * BIOME_RES], nonzero: false }
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
        self.nonzero = true;
    }

    /// Read the concentration at `pos` on `channel` (index clamped).
    pub fn sample(&self, pos: Vec2, channel: usize) -> f32 {
        let ch = channel.min(PHEROMONE_CHANNELS - 1);
        self.cells[Self::idx(pos)][ch]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_sets_nonzero_and_decay_applies() {
        let mut f = PheromoneField::new();
        assert!(!f.nonzero, "fresh field has the early-out flag clear");
        f.decay_step(); // no-op on the empty field
        f.deposit(Vec2::new(10.0, 10.0), 0, 1.0);
        assert!(f.nonzero, "deposit latches the flag");
        f.decay_step();
        assert!(
            (f.sample(Vec2::new(10.0, 10.0), 0) - (1.0 - PHEROMONE_DECAY)).abs() < 1e-6,
            "decay multiplies once the flag is set"
        );
    }

    #[test]
    fn deposit_and_sample_clamp_out_of_range_channels() {
        let mut f = PheromoneField::new();
        f.deposit(Vec2::new(5.0, 5.0), usize::MAX, 2.0);
        assert_eq!(f.sample(Vec2::new(5.0, 5.0), PHEROMONE_CHANNELS - 1), 2.0);
        assert_eq!(f.sample(Vec2::new(5.0, 5.0), usize::MAX), 2.0);
    }

    #[test]
    fn refresh_nonzero_recovers_flag_from_cells() {
        let mut f = PheromoneField::new();
        f.deposit(Vec2::new(7.0, 7.0), 1, 0.5);
        // Simulate a snapshot roundtrip: serde skips the flag, so it comes
        // back false while the cells keep their values.
        f.nonzero = false;
        f.decay_step();
        assert_eq!(
            f.sample(Vec2::new(7.0, 7.0), 1),
            0.5,
            "with the flag lost, decay is (incorrectly) skipped — hence refresh on load"
        );
        f.refresh_nonzero();
        assert!(f.nonzero);
        f.decay_step();
        assert!((f.sample(Vec2::new(7.0, 7.0), 1) - 0.5 * (1.0 - PHEROMONE_DECAY)).abs() < 1e-6);
    }
}
