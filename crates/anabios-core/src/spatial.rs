//! Uniform-grid spatial hash for fast neighbor queries.
//!
//! World is a torus of size `WORLD_SIZE`. The hash divides it into `RES × RES`
//! cells. To query a position within `radius`, the caller asks for all agents
//! in the cells that the radius's bounding box touches, then filters by exact
//! distance. Cell size is chosen so that `radius ≤ cell_size`; one ring of
//! neighbour cells is always sufficient.

use crate::biome::WORLD_SIZE;
use crate::prelude::Vec2;

/// Number of cells per axis. 64 gives `cell_size = 16` world units, which
/// safely covers the maximum possible perception radius
/// (`PERCEPTION_MAX_RADIUS`, defined below).
pub const HASH_RES: usize = 64;
pub const HASH_RES_DEFAULT: usize = HASH_RES;
pub const HASH_CELL_SIZE: f32 = WORLD_SIZE / HASH_RES as f32;

/// Hard upper bound on perception radius — must hold for the
/// "one-ring-of-neighbours is sufficient" guarantee.
pub const PERCEPTION_MAX_RADIUS: f32 = HASH_CELL_SIZE;

#[derive(Debug, Clone)]
pub struct UniformSpatialHash {
    /// For each cell index, the slice of `flat` that contains its agent ids.
    bucket_offsets: Vec<u32>,
    bucket_lens: Vec<u32>,
    flat: Vec<u32>,
    /// Reusable per-cell count buffer for `rebuild` (avoids a per-tick alloc).
    counts: Vec<u32>,
}

impl UniformSpatialHash {
    pub fn new() -> Self {
        let total_cells = HASH_RES * HASH_RES;
        Self {
            bucket_offsets: vec![0; total_cells],
            bucket_lens: vec![0; total_cells],
            flat: Vec::new(),
            counts: vec![0; total_cells],
        }
    }

    /// Rebuild from the alive agent positions. Agents whose `alive` bit is
    /// false are skipped. `positions[i]` and `alive_iter` are indexed by
    /// agent id.
    pub fn rebuild(&mut self, positions: &[Vec2], alive: impl Fn(usize) -> bool) {
        self.rebuild_indexed(positions.len(), |i| positions[i], alive);
    }

    /// Rebuild from any indexable position source (e.g. carcasses, where the
    /// position is a field of the element). `pos_of(i)` returns the position
    /// of element `i`; `alive(i)` filters elements out.
    pub fn rebuild_indexed(
        &mut self,
        len: usize,
        pos_of: impl Fn(usize) -> Vec2,
        alive: impl Fn(usize) -> bool,
    ) {
        let total_cells = HASH_RES * HASH_RES;
        // Phase 1: count agents per cell (reused buffer, no per-tick alloc).
        self.counts.clear();
        self.counts.resize(total_cells, 0);
        for i in 0..len {
            if !alive(i) {
                continue;
            }
            let cell = Self::cell_of(pos_of(i));
            self.counts[cell] += 1;
        }

        // Phase 2: prefix-sum to compute offsets.
        let mut total = 0_u32;
        for i in 0..total_cells {
            self.bucket_offsets[i] = total;
            total += self.counts[i];
            self.bucket_lens[i] = 0;
        }
        self.flat.clear();
        self.flat.resize(total as usize, 0);

        // Phase 3: scatter into flat buffer.
        for i in 0..len {
            if !alive(i) {
                continue;
            }
            let cell = Self::cell_of(pos_of(i));
            let off = self.bucket_offsets[cell] + self.bucket_lens[cell];
            self.flat[off as usize] = i as u32;
            self.bucket_lens[cell] += 1;
        }
    }

    /// Visit every agent in the wrap-aware bounding box of a position +
    /// radius. The caller is responsible for the exact distance check.
    ///
    /// `radius` must not exceed `PERCEPTION_MAX_RADIUS`; debug builds assert.
    pub fn query<F: FnMut(u32)>(&self, pos: Vec2, radius: f32, mut f: F) {
        debug_assert!(
            radius <= PERCEPTION_MAX_RADIUS + 1e-3,
            "query radius {radius} exceeds PERCEPTION_MAX_RADIUS={PERCEPTION_MAX_RADIUS}"
        );
        let (cx, cy) = Self::cell_coords(pos);
        // One-cell ring; positions wrap around the torus.
        for dy in [HASH_RES - 1, 0, 1] {
            let row = (cy + dy) % HASH_RES;
            for dx in [HASH_RES - 1, 0, 1] {
                let col = (cx + dx) % HASH_RES;
                let cell = row * HASH_RES + col;
                let off = self.bucket_offsets[cell] as usize;
                let len = self.bucket_lens[cell] as usize;
                for id in &self.flat[off..off + len] {
                    f(*id);
                }
            }
        }
    }

    #[inline]
    fn cell_coords(pos: Vec2) -> (usize, usize) {
        let x = pos.x.rem_euclid(WORLD_SIZE);
        let y = pos.y.rem_euclid(WORLD_SIZE);
        let col = ((x / HASH_CELL_SIZE) as usize).min(HASH_RES - 1);
        let row = ((y / HASH_CELL_SIZE) as usize).min(HASH_RES - 1);
        (col, row)
    }

    #[inline]
    fn cell_of(pos: Vec2) -> usize {
        let (col, row) = Self::cell_coords(pos);
        row * HASH_RES + col
    }
}

impl Default for UniformSpatialHash {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrap-aware distance between two points on the torus.
#[inline]
pub fn torus_distance(a: Vec2, b: Vec2) -> f32 {
    let mut dx = (a.x - b.x).abs();
    let mut dy = (a.y - b.y).abs();
    if dx > WORLD_SIZE * 0.5 {
        dx = WORLD_SIZE - dx;
    }
    if dy > WORLD_SIZE * 0.5 {
        dy = WORLD_SIZE - dy;
    }
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute_force_neighbors(positions: &[Vec2], origin: Vec2, radius: f32) -> Vec<u32> {
        let mut out: Vec<u32> = (0..positions.len() as u32)
            .filter(|i| torus_distance(positions[*i as usize], origin) <= radius)
            .collect();
        out.sort();
        out
    }

    #[test]
    fn empty_hash_returns_no_results() {
        let h = UniformSpatialHash::new();
        let mut found = Vec::new();
        h.query(Vec2::new(100.0, 100.0), 8.0, |id| found.push(id));
        assert!(found.is_empty());
    }

    #[test]
    fn query_matches_brute_force_random_positions() {
        let positions: Vec<Vec2> = (0..500)
            .map(|i| {
                let x = ((i * 17) % 1024) as f32 + 0.5;
                let y = ((i * 31) % 1024) as f32 + 0.5;
                Vec2::new(x, y)
            })
            .collect();
        let mut h = UniformSpatialHash::new();
        h.rebuild(&positions, |_| true);

        let probes = [
            Vec2::new(10.0, 10.0),
            Vec2::new(513.0, 513.0),
            Vec2::new(1023.0, 0.5),
            Vec2::new(0.5, 1023.0),
        ];
        for probe in probes {
            let mut got: Vec<u32> = Vec::new();
            h.query(probe, PERCEPTION_MAX_RADIUS, |id| {
                if torus_distance(positions[id as usize], probe) <= PERCEPTION_MAX_RADIUS {
                    got.push(id);
                }
            });
            got.sort();
            got.dedup();
            let expected = brute_force_neighbors(&positions, probe, PERCEPTION_MAX_RADIUS);
            assert_eq!(got, expected, "probe {:?}", probe);
        }
    }

    #[test]
    fn alive_mask_skips_dead_agents() {
        let positions = vec![Vec2::new(100.0, 100.0); 4];
        let mut h = UniformSpatialHash::new();
        h.rebuild(&positions, |i| i != 2);
        let mut found: Vec<u32> = Vec::new();
        h.query(Vec2::new(100.0, 100.0), 4.0, |id| found.push(id));
        found.sort();
        assert_eq!(found, vec![0, 1, 3]);
    }

    #[test]
    fn torus_distance_wraps_short_way() {
        let a = Vec2::new(2.0, 0.0);
        let b = Vec2::new(WORLD_SIZE - 2.0, 0.0);
        assert!((torus_distance(a, b) - 4.0).abs() < 1e-3);
    }
}
