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
    /// Cells with agents in the last `rebuild_sparse`, in first-touched order;
    /// the only cells that build has to reset next time.
    touched: Vec<u32>,
    /// Set by the dense `rebuild`, which leaves `counts`/`bucket_lens` dirty
    /// in every occupied cell; `rebuild_sparse` then resets the whole grid
    /// once before trusting `touched` again.
    dense_dirty: bool,
    /// Grid resolution per axis (was the `HASH_RES` const).
    res: usize,
    /// World-units per cell (was the `HASH_CELL_SIZE` const): `world_size / res`.
    cell_size: f32,
    /// World extent per axis (was the `WORLD_SIZE` const).
    world_size: f32,
}

impl UniformSpatialHash {
    pub fn new() -> Self {
        Self::with_dims(WORLD_SIZE, HASH_RES)
    }

    /// Build a hash sized for a `world_size`-extent torus divided into
    /// `hash_res × hash_res` cells.
    pub fn with_dims(world_size: f32, hash_res: usize) -> Self {
        // `query` visits the one-cell ring via offsets `[res-1, 0, 1]`. Those are
        // distinct mod `res` only when `res >= 3`; below that a neighbour would be
        // visited twice (double-counting crowding / feeding / combat), so guard it.
        debug_assert!(
            hash_res >= 3,
            "hash_res must be >= 3 for the query ring to visit each cell once (got {hash_res})"
        );
        let total_cells = hash_res * hash_res;
        Self {
            bucket_offsets: vec![0; total_cells],
            bucket_lens: vec![0; total_cells],
            flat: Vec::new(),
            counts: vec![0; total_cells],
            touched: Vec::new(),
            dense_dirty: false,
            res: hash_res,
            cell_size: world_size / hash_res as f32,
            world_size,
        }
    }

    /// Hard upper bound on perception radius this hash supports — must hold
    /// for the "one-ring-of-neighbours is sufficient" guarantee.
    #[inline]
    pub fn perception_max_radius(&self) -> f32 {
        self.cell_size
    }

    /// Grid resolution per axis.
    #[inline]
    pub fn res(&self) -> usize {
        self.res
    }

    /// World extent per axis (torus size) this hash is sized for.
    #[inline]
    pub fn world_size(&self) -> f32 {
        self.world_size
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
        let total_cells = self.res * self.res;
        self.dense_dirty = true;
        // Phase 1: count agents per cell (reused buffer, no per-tick alloc).
        self.counts.clear();
        self.counts.resize(total_cells, 0);
        for i in 0..len {
            if !alive(i) {
                continue;
            }
            let cell = self.cell_of(pos_of(i));
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
            let cell = self.cell_of(pos_of(i));
            let off = self.bucket_offsets[cell] + self.bucket_lens[cell];
            self.flat[off as usize] = i as u32;
            self.bucket_lens[cell] += 1;
        }
    }

    /// Visit every agent in the wrap-aware bounding box of a position +
    /// radius. The caller is responsible for the exact distance check.
    ///
    /// `radius` must not exceed `self.perception_max_radius()`; debug builds assert.
    pub fn query<F: FnMut(u32)>(&self, pos: Vec2, radius: f32, mut f: F) {
        debug_assert!(
            radius <= self.perception_max_radius() + 1e-3,
            "query radius {radius} exceeds perception_max_radius={}",
            self.perception_max_radius()
        );
        let (cx, cy) = self.cell_coords(pos);
        // One-cell ring; positions wrap around the torus.
        for dy in [self.res - 1, 0, 1] {
            let row = (cy + dy) % self.res;
            for dx in [self.res - 1, 0, 1] {
                let col = (cx + dx) % self.res;
                let cell = row * self.res + col;
                let off = self.bucket_offsets[cell] as usize;
                let len = self.bucket_lens[cell] as usize;
                for id in &self.flat[off..off + len] {
                    f(*id);
                }
            }
        }
    }

    /// `rebuild` for a grid far larger than its population (the collision
    /// hash: 65k cells for ~1k agents on the flagship, 1M cells at the 1024²
    /// cap): O(alive) instead of O(cells). Only the cells the previous sparse
    /// build touched are reset, and bucket offsets are assigned by walking
    /// that touched list (first-touched order over ascending ids ⇒
    /// deterministic) instead of every cell. Each cell's slice holds exactly
    /// the ids `rebuild` would give it, in the same ascending order, so
    /// `query`/`query_bbox`/`query_wide` are bit-identical; only the layout
    /// of `flat` differs, which no reader observes. Safe to mix with the
    /// dense `rebuild` (a full reset follows one).
    pub fn rebuild_sparse(&mut self, positions: &[Vec2], alive: impl Fn(usize) -> bool) {
        let total_cells = self.res * self.res;
        if self.dense_dirty || self.counts.len() != total_cells {
            self.counts.clear();
            self.counts.resize(total_cells, 0);
            self.bucket_offsets.iter_mut().for_each(|o| *o = 0);
            self.bucket_lens.iter_mut().for_each(|l| *l = 0);
            self.touched.clear();
            self.dense_dirty = false;
        }
        // Phase 0: reset only last build's occupied cells. Offsets are zeroed
        // too so an emptied cell slices `flat[0..0]`, never past its end.
        for &cell in &self.touched {
            let c = cell as usize;
            self.counts[c] = 0;
            self.bucket_offsets[c] = 0;
            self.bucket_lens[c] = 0;
        }
        self.touched.clear();
        // Phase 1: count per cell, recording each cell on its first hit.
        for (i, &p) in positions.iter().enumerate() {
            if !alive(i) {
                continue;
            }
            let cell = self.cell_of(p);
            if self.counts[cell] == 0 {
                self.touched.push(cell as u32);
            }
            self.counts[cell] += 1;
        }
        // Phase 2: prefix-sum over the touched cells only.
        let mut total = 0_u32;
        for &cell in &self.touched {
            let c = cell as usize;
            self.bucket_offsets[c] = total;
            total += self.counts[c];
            self.bucket_lens[c] = 0;
        }
        self.flat.clear();
        self.flat.resize(total as usize, 0);
        // Phase 3: scatter (ascending id order within each cell, as `rebuild`).
        for (i, &p) in positions.iter().enumerate() {
            if !alive(i) {
                continue;
            }
            let cell = self.cell_of(p);
            let off = self.bucket_offsets[cell] + self.bucket_lens[cell];
            self.flat[off as usize] = i as u32;
            self.bucket_lens[cell] += 1;
        }
    }

    /// `query` restricted to the ring cells that can hold a point within
    /// `radius` of `pos`, for `radius < cell_size`. Walks the SAME
    /// `[res-1, 0, 1]` ring in the same row-major order as `query` and skips
    /// the ring rows/columns outside the wrapped box `pos ± radius`, so the
    /// ids it yields are a subsequence of `query`'s: a caller whose own
    /// distance check is strictly below `radius` (leave a margin above the
    /// accept reach — a point within the reach is then inside the box by more
    /// than f32 rounding at the seam) sees the same accepted neighbours in the
    /// same order, bit-identical, at roughly half the candidates. The
    /// collision sweeps use it: their reach (≤ 2.75) is well under the 4-unit
    /// cell. Deterministic.
    pub fn query_bbox<F: FnMut(u32)>(&self, pos: Vec2, radius: f32, mut f: F) {
        debug_assert!(
            radius < self.cell_size,
            "query_bbox radius {radius} must be below cell_size={}",
            self.cell_size
        );
        let (cx, cy) = self.cell_coords(pos);
        let (lo_x, lo_y) = self.cell_coords(pos - Vec2::splat(radius));
        let (hi_x, hi_y) = self.cell_coords(pos + Vec2::splat(radius));
        for dy in [self.res - 1, 0, 1] {
            let row = (cy + dy) % self.res;
            if row != cy && row != lo_y && row != hi_y {
                continue;
            }
            for dx in [self.res - 1, 0, 1] {
                let col = (cx + dx) % self.res;
                if col != cx && col != lo_x && col != hi_x {
                    continue;
                }
                let cell = row * self.res + col;
                let off = self.bucket_offsets[cell] as usize;
                let len = self.bucket_lens[cell] as usize;
                for id in &self.flat[off..off + len] {
                    f(*id);
                }
            }
        }
    }

    /// Visit every agent in the wrap-aware bounding box of a position +
    /// radius, for radii BEYOND the one-ring guarantee: walks
    /// `ceil(radius / cell_size)` rings. Deterministic (row-major, rings
    /// centred on the query cell). The caller does the exact distance check.
    /// Cost grows with the ring count, so this is for occasional wide scans
    /// (mate seeking), not the per-tick perception path.
    pub fn query_wide<F: FnMut(u32)>(&self, pos: Vec2, radius: f32, mut f: F) {
        let rings = ((radius / self.cell_size).ceil() as usize).max(1);
        let (cx, cy) = self.cell_coords(pos);
        // A wide enough radius covers the whole torus; never visit a cell twice.
        let span = (2 * rings + 1).min(self.res);
        for dy in 0..span {
            let row = (cy + self.res + dy - rings.min(self.res / 2)) % self.res;
            for dx in 0..span {
                let col = (cx + self.res + dx - rings.min(self.res / 2)) % self.res;
                let cell = row * self.res + col;
                let off = self.bucket_offsets[cell] as usize;
                let len = self.bucket_lens[cell] as usize;
                for id in &self.flat[off..off + len] {
                    f(*id);
                }
            }
        }
    }

    #[inline]
    fn cell_coords(&self, pos: Vec2) -> (usize, usize) {
        let x = pos.x.rem_euclid(self.world_size);
        let y = pos.y.rem_euclid(self.world_size);
        let col = ((x / self.cell_size) as usize).min(self.res - 1);
        let row = ((y / self.cell_size) as usize).min(self.res - 1);
        (col, row)
    }

    #[inline]
    fn cell_of(&self, pos: Vec2) -> usize {
        let (col, row) = self.cell_coords(pos);
        row * self.res + col
    }
}

impl Default for UniformSpatialHash {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrap-aware signed shortest-path delta `a − b` on a torus of `world_size`.
///
/// Each component is folded into `[-world_size/2, world_size/2]` — the
/// displacement you'd travel going the short way around the torus. This is the
/// single source for the signed-wrap arithmetic that direction, midpoint, and
/// homing calculations all build on; keep it in one place so the determinism-
/// sensitive fold is identical everywhere.
#[inline]
pub fn torus_delta(a: Vec2, b: Vec2, world_size: f32) -> Vec2 {
    let mut dx = a.x - b.x;
    let mut dy = a.y - b.y;
    if dx > world_size * 0.5 {
        dx -= world_size;
    } else if dx < -world_size * 0.5 {
        dx += world_size;
    }
    if dy > world_size * 0.5 {
        dy -= world_size;
    } else if dy < -world_size * 0.5 {
        dy += world_size;
    }
    Vec2::new(dx, dy)
}

/// Wrap-aware *squared* distance between two points on a torus of the given
/// `world_size`. This is the body of `torus_distance` without the final
/// `sqrt`: `torus_distance(a, b, ws) == torus_distance_sq(a, b, ws).sqrt()`
/// bit-for-bit. Callers that only need to compare or reject by distance should
/// use this (squares are monotonic on `[0, ∞)`) and take the `sqrt` once, at
/// the end, on the few values they actually keep.
#[inline]
pub fn torus_distance_sq(a: Vec2, b: Vec2, world_size: f32) -> f32 {
    let mut dx = (a.x - b.x).abs();
    let mut dy = (a.y - b.y).abs();
    if dx > world_size * 0.5 {
        dx = world_size - dx;
    }
    if dy > world_size * 0.5 {
        dy = world_size - dy;
    }
    dx * dx + dy * dy
}

/// Wrap-aware distance between two points on a torus of the given `world_size`.
#[inline]
pub fn torus_distance(a: Vec2, b: Vec2, world_size: f32) -> f32 {
    torus_distance_sq(a, b, world_size).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random positions (LCG; no RNG crate needed).
    fn scatter(n: usize, ws: f32, seed: u64) -> Vec<Vec2> {
        let mut x = seed;
        let mut next = || {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((x >> 33) as f32 / (1u64 << 31) as f32) * ws
        };
        (0..n).map(|_| Vec2::new(next(), next())).collect()
    }

    fn collect(h: &UniformSpatialHash, pos: Vec2, r: f32, bbox: bool) -> Vec<u32> {
        let mut out = Vec::new();
        if bbox {
            h.query_bbox(pos, r, |id| out.push(id));
        } else {
            h.query(pos, r, |id| out.push(id));
        }
        out
    }

    #[test]
    fn sparse_rebuild_matches_dense_per_cell_contents() {
        let ws = 256.0;
        let pos = scatter(3000, ws, 7);
        let alive = |i: usize| i % 7 != 3;
        let mut dense = UniformSpatialHash::with_dims(ws, 64);
        let mut sparse = UniformSpatialHash::with_dims(ws, 64);
        dense.rebuild(&pos, alive);
        // Two sparse builds in a row (the second resets via `touched`), then
        // one after a dense build on the same hash (reset via `dense_dirty`).
        sparse.rebuild_sparse(&pos, alive);
        sparse.rebuild_sparse(&pos, alive);
        let mut mixed = UniformSpatialHash::with_dims(ws, 64);
        mixed.rebuild(&scatter(500, ws, 3), |_| true);
        mixed.rebuild_sparse(&pos, alive);
        for probe in scatter(400, ws, 11) {
            let want = collect(&dense, probe, 4.0, false);
            assert_eq!(collect(&sparse, probe, 4.0, false), want, "sparse vs dense at {probe:?}");
            assert_eq!(collect(&mixed, probe, 4.0, false), want, "mixed vs dense at {probe:?}");
        }
        // A cell emptied between builds must slice `flat[0..0]`, not panic.
        let moved: Vec<Vec2> = pos.iter().map(|p| *p + Vec2::splat(100.0)).collect();
        sparse.rebuild_sparse(&moved, alive);
        dense.rebuild(&moved, alive);
        for probe in scatter(200, ws, 13) {
            assert_eq!(collect(&sparse, probe, 4.0, false), collect(&dense, probe, 4.0, false));
        }
    }

    #[test]
    fn query_bbox_is_an_order_preserving_subsequence_that_keeps_every_near_agent() {
        let ws = 256.0;
        let pos = scatter(4000, ws, 5);
        let mut h = UniformSpatialHash::with_dims(ws, 64); // 4-unit cells
        h.rebuild(&pos, |_| true);
        let r = 2.75;
        for probe in scatter(600, ws, 17).into_iter().chain([
            Vec2::new(0.0, 0.0),
            Vec2::new(255.999, 128.0),
            Vec2::new(1.0, 255.99),
            Vec2::new(4.0, 4.0),
            Vec2::new(3.999, 7.999),
        ]) {
            let full = collect(&h, probe, 4.0, false);
            let sub = collect(&h, probe, r, true);
            // Subsequence of the full ring visit, in the same order.
            let mut k = 0;
            for id in &full {
                if k < sub.len() && sub[k] == *id {
                    k += 1;
                }
            }
            assert_eq!(k, sub.len(), "bbox ids must be an in-order subsequence at {probe:?}");
            // Every agent within the (strictly smaller) accept reach is kept.
            for (id, p) in pos.iter().enumerate() {
                if torus_distance(probe, *p, ws) < r - 0.25 {
                    assert!(
                        sub.contains(&(id as u32)),
                        "agent {id} within reach missing at {probe:?}"
                    );
                }
            }
        }
    }

    fn brute_force_neighbors(positions: &[Vec2], origin: Vec2, radius: f32) -> Vec<u32> {
        let mut out: Vec<u32> = (0..positions.len() as u32)
            .filter(|i| torus_distance(positions[*i as usize], origin, WORLD_SIZE) <= radius)
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
                if torus_distance(positions[id as usize], probe, WORLD_SIZE)
                    <= PERCEPTION_MAX_RADIUS
                {
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
        assert!((torus_distance(a, b, WORLD_SIZE) - 4.0).abs() < 1e-3);
    }
}

#[cfg(test)]
mod wide_query_tests {
    use super::*;

    #[test]
    fn query_wide_reaches_beyond_one_ring_and_wraps() {
        let h_size = 1024.0;
        let mut hash = UniformSpatialHash::with_dims(h_size, 64); // cell 16
                                                                  // Agent 0 at the origin corner, agent 1 five cells away, agent 2 on
                                                                  // the far side of the wrap (one cell "behind" the origin).
        let positions = vec![Vec2::new(8.0, 8.0), Vec2::new(88.0, 8.0), Vec2::new(1016.0, 8.0)];
        hash.rebuild(&positions, |_| true);
        let mut seen = Vec::new();
        hash.query(positions[0], 16.0, |id| seen.push(id));
        assert!(!seen.contains(&1), "one-ring query must not reach five cells out");
        let mut wide = Vec::new();
        hash.query_wide(positions[0], 96.0, |id| wide.push(id));
        wide.sort_unstable();
        assert_eq!(wide, vec![0, 1, 2], "wide query reaches five cells out and across the wrap");
    }

    #[test]
    fn query_wide_covering_the_whole_torus_visits_each_agent_once() {
        let mut hash = UniformSpatialHash::with_dims(64.0, 4); // cell 16, 4x4 grid
        let positions: Vec<Vec2> = (0..16)
            .map(|i| Vec2::new((i % 4) as f32 * 16.0 + 1.0, (i / 4) as f32 * 16.0 + 1.0))
            .collect();
        hash.rebuild(&positions, |_| true);
        let mut seen = Vec::new();
        hash.query_wide(Vec2::new(1.0, 1.0), 1000.0, |id| seen.push(id));
        seen.sort_unstable();
        assert_eq!(seen, (0..16).collect::<Vec<u32>>());
    }
}
