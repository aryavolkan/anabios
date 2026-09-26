//! Habitat locomotion (territory/habitat/collision layer). Each agent's
//! heritable `GenomeSlot::Locomotion` gene reads as Land / Water / Air, which
//! decides the terrain it may occupy and graze and which bodies it collides
//! with. Pure functions only — every caller gates on `World::territory_enabled`,
//! so a flag-off world never reaches this module.

use serde::{Deserialize, Serialize};

use crate::biome::{BiomeField, TerrainType};
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::{wrap_torus, Vec2};

/// Genes below this read as Water.
pub const WATER_MAX: f32 = 0.25;
/// Genes at or above this read as Air.
pub const AIR_MIN: f32 = 0.75;
/// Scale applied to the Locomotion slot's per-birth mutation delta, so class
/// flips are rare (same RNG draw count — only the magnitude shrinks).
pub const LOCOMOTION_MUTATION_SCALE: f32 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum Locomotion {
    #[default]
    Land = 0,
    Water = 1,
    Air = 2,
}

impl Locomotion {
    /// Every class, in `index()` order (the majority vote in
    /// `territory::territory_step` and the viewer's byte codes rely on it).
    pub const ALL: [Locomotion; 3] = [Locomotion::Land, Locomotion::Water, Locomotion::Air];

    #[inline]
    pub fn from_gene(v: f32) -> Self {
        if v < WATER_MAX {
            Locomotion::Water
        } else if v >= AIR_MIN {
            Locomotion::Air
        } else {
            Locomotion::Land
        }
    }

    #[inline]
    pub fn of(g: &Genome) -> Self {
        Self::from_gene(g.get(GenomeSlot::Locomotion))
    }

    /// Whether this class may stand in a cell of terrain `t`.
    #[inline]
    pub fn can_occupy(self, t: TerrainType) -> bool {
        match self {
            Locomotion::Land => t != TerrainType::Water,
            Locomotion::Water => t == TerrainType::Water,
            Locomotion::Air => true,
        }
    }

    /// Whether this class may graze biomass in a cell of terrain `t`. A class
    /// grazes exactly where it may stand (`can_occupy`): flyers feed over land
    /// AND sea — a seabird niche — while Land and Water are each confined to
    /// their own terrain. Kept as a separate name so the feeding call sites
    /// (`interact::feed_pass`, `sense::best_plant_direction`) read as intent;
    /// `graze_matches_occupy` pins the two tables equal.
    #[inline]
    pub fn can_graze(self, t: TerrainType) -> bool {
        self.can_occupy(t)
    }

    /// Air bodies only collide with Air; Land and Water collide with each other.
    #[inline]
    pub fn collides_with(self, other: Locomotion) -> bool {
        (self == Locomotion::Air) == (other == Locomotion::Air)
    }

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }
}

/// Apply a proposed displacement `v` from `pos` under the habitat rule:
/// the full move if its path is valid, else the x-only move, else the
/// y-only move (a slide along the coastline), else no move. Air is never
/// gated. Returns the displacement actually applied.
///
/// Every sample is taken on the torus-wrapped point — exactly the coordinate
/// `integrate_all` stores — so the cell the gate validates is the cell the
/// agent is later read in. (Sampling the raw `pos + d` is not the same: for
/// an overshoot of less than ~3e-5 past the seam, f32 `rem_euclid` rounds the
/// stored coordinate to exactly `world_size`, which `cell_coords` reads as
/// row/column 0 while the raw value clamps to the last row/column — a
/// habitat violation the gate could not see.) A displacement longer than
/// half a cell is sampled along its segment at ≤ half-cell spacing, so a
/// fast agent (speed multipliers stack to ~9 units/tick against 8-unit
/// cells) cannot tunnel through a one-cell strip of forbidden terrain.
pub fn gate_move(biome: &BiomeField, class: Locomotion, pos: Vec2, v: Vec2) -> Vec2 {
    if class == Locomotion::Air {
        return v;
    }
    let ws = Vec2::splat(biome.world_size);
    let half_cell = 0.5 * biome.cell_size;
    let ok = |d: Vec2| {
        let steps = if d.length_squared() <= half_cell * half_cell {
            1
        } else {
            (d.length() / half_cell).ceil() as u32
        };
        (1..=steps).all(|k| {
            let p = wrap_torus(pos + d * (k as f32 / steps as f32), ws);
            class.can_occupy(biome.sample(p).terrain)
        })
    };
    if ok(v) {
        v
    } else if ok(Vec2::new(v.x, 0.0)) {
        Vec2::new(v.x, 0.0)
    } else if ok(Vec2::new(0.0, v.y)) {
        Vec2::new(0.0, v.y)
    } else {
        Vec2::ZERO
    }
}

/// `pos` itself when its cell is valid for `class`; otherwise the centre of
/// the closest valid cell on the first ring (Chebyshev radius 1, 2, …) that
/// has one. Within a ring the Euclidean-closest cell wins, first-seen on ties
/// (fixed scan order ⇒ deterministic, no RNG). The search covers the whole
/// torus (`res / 2` rings), so `None` means the class has NO habitat on this
/// map at all — not merely none nearby. The cost is paid only on the failure
/// path: a valid `pos` returns immediately and a same-class newborn between
/// two parents stops at ring 1; a class-flipped newborn or a founder placed
/// far from its habitat pays up to `res²` cell reads once. Both callers
/// (`Scenario::instantiate`, `reproduce_all`) keep the original position on
/// `None`, which strands the agent (immobile, cannot graze) — a scenario
/// that seeds a class without habitat is a scenario-authoring error.
pub fn nearest_valid(biome: &BiomeField, pos: Vec2, class: Locomotion) -> Option<Vec2> {
    if class.can_occupy(biome.sample(pos).terrain) {
        return Some(pos);
    }
    let (cx, cy) = biome.cell_coords(pos);
    let res = biome.res as i32;
    let max_ring = (biome.res / 2) as i32;
    for k in 1..=max_ring {
        let mut best: Option<(f32, Vec2)> = None;
        for dy in -k..=k {
            for dx in -k..=k {
                if dx.abs() != k && dy.abs() != k {
                    continue;
                }
                let col = (cx as i32 + dx).rem_euclid(res) as usize;
                let row = (cy as i32 + dy).rem_euclid(res) as usize;
                if !class.can_occupy(biome.at(col, row).terrain) {
                    continue;
                }
                let off = biome.cell_offset_from(col, row, pos);
                let d2 = off.length_squared();
                if best.is_none_or(|(bd, _)| d2 < bd) {
                    best = Some((d2, off));
                }
            }
        }
        if let Some((_, off)) = best {
            let ws = biome.world_size;
            let p = pos + off;
            return Some(Vec2::new(p.x.rem_euclid(ws), p.y.rem_euclid(ws)));
        }
    }
    None
}

/// Whether a habitat-selection pull (`pull`, any length) may apply: the cell
/// one cell-width along it must be valid for `class`. `None` (flag off)
/// always allows, so flag-off arithmetic is untouched.
pub fn pull_allowed(biome: &BiomeField, pos: Vec2, pull: Vec2, class: Option<Locomotion>) -> bool {
    let Some(class) = class else {
        return true;
    };
    let probe = pos + pull.normalize_or_zero() * biome.cell_size;
    class.can_occupy(biome.sample(probe).terrain)
}

/// Shrink this birth's mutation of the Locomotion slot to
/// `LOCOMOTION_MUTATION_SCALE` of its drawn size: `before` is the slot value
/// after crossover and before mutation.
pub fn damp_locomotion_mutation(before: f32, g: &mut Genome) {
    let after = g.get(GenomeSlot::Locomotion);
    g.set(GenomeSlot::Locomotion, before + (after - before) * LOCOMOTION_MUTATION_SCALE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;
    use crate::genome::{Genome, GenomeSlot};
    use crate::prelude::Vec2;
    use crate::world::World;

    /// 256-unit world, 32x32 cells of 8 units: all Grass, with columns
    /// `col >= 16` turned to Water (a straight north-south coastline at x=128).
    fn coast_world() -> World {
        let mut w = World::with_dims(1, 256.0, 32, 16);
        let res = w.biome.res;
        for row in 0..res {
            for col in 0..res {
                w.biome.at_mut(col, row).terrain =
                    if col >= 16 { TerrainType::Water } else { TerrainType::Grass };
            }
        }
        w
    }

    #[test]
    fn bands_split_at_quarter_and_three_quarters() {
        assert_eq!(Locomotion::from_gene(0.0), Locomotion::Water);
        assert_eq!(Locomotion::from_gene(0.249), Locomotion::Water);
        assert_eq!(Locomotion::from_gene(0.25), Locomotion::Land);
        assert_eq!(Locomotion::from_gene(0.5), Locomotion::Land);
        assert_eq!(Locomotion::from_gene(0.749), Locomotion::Land);
        assert_eq!(Locomotion::from_gene(0.75), Locomotion::Air);
        assert_eq!(Locomotion::from_gene(1.0), Locomotion::Air);
        assert_eq!(Locomotion::of(&Genome::neutral()), Locomotion::Land, "neutral genome is Land");
    }

    #[test]
    fn terrain_rules() {
        use Locomotion::*;
        assert!(Land.can_occupy(TerrainType::Grass) && !Land.can_occupy(TerrainType::Water));
        assert!(Water.can_occupy(TerrainType::Water) && !Water.can_occupy(TerrainType::Grass));
        assert!(Air.can_occupy(TerrainType::Water) && Air.can_occupy(TerrainType::Rock));
        assert!(Water.can_graze(TerrainType::Water) && !Water.can_graze(TerrainType::Grass));
        assert!(Land.can_graze(TerrainType::Grass) && !Land.can_graze(TerrainType::Water));
        assert!(Air.can_graze(TerrainType::Grass) && Air.can_graze(TerrainType::Water));
        assert!(Land.collides_with(Water) && Land.collides_with(Land));
        assert!(Air.collides_with(Air));
        assert!(!Air.collides_with(Land) && !Water.collides_with(Air));
    }

    #[test]
    fn gate_blocks_land_into_water_and_slides_along_the_coast() {
        let w = coast_world();
        let pos = Vec2::new(126.0, 100.0); // col 15 (grass), 2 units from the coast
                                           // Straight into the water: blocked entirely.
        assert_eq!(gate_move(&w.biome, Locomotion::Land, pos, Vec2::new(4.0, 0.0)), Vec2::ZERO);
        // Diagonal: the x part would enter water, the y part is fine -> slide.
        let v = gate_move(&w.biome, Locomotion::Land, pos, Vec2::new(2.8, 2.8));
        assert_eq!(v, Vec2::new(0.0, 2.8));
        // Air ignores terrain.
        let a = gate_move(&w.biome, Locomotion::Air, pos, Vec2::new(4.0, 0.0));
        assert_eq!(a, Vec2::new(4.0, 0.0));
        // Water can't climb out onto land.
        let wpos = Vec2::new(130.0, 100.0); // col 16 (water)
        assert_eq!(gate_move(&w.biome, Locomotion::Water, wpos, Vec2::new(-4.0, 0.0)), Vec2::ZERO);
    }

    #[test]
    fn nearest_valid_is_identity_on_valid_ground_and_finds_the_coast() {
        let w = coast_world();
        let land = Vec2::new(40.0, 40.0);
        assert_eq!(nearest_valid(&w.biome, land, Locomotion::Land), Some(land));
        // A water agent on land moves to the nearest water cell centre: col 16.
        let got = nearest_valid(&w.biome, Vec2::new(120.0, 100.0), Locomotion::Water).unwrap();
        assert_eq!(w.biome.sample(got).terrain, TerrainType::Water);
        assert!((got.x - 132.0).abs() < 1e-3, "col 16 centre is x=132, got {}", got.x);
        // Deterministic.
        assert_eq!(nearest_valid(&w.biome, Vec2::new(120.0, 100.0), Locomotion::Water), Some(got));
    }

    #[test]
    fn nearest_valid_is_none_when_the_class_has_no_habitat() {
        let mut w = World::with_dims(1, 256.0, 32, 16);
        for c in w.biome.cells.iter_mut() {
            c.terrain = TerrainType::Grass;
        }
        assert_eq!(nearest_valid(&w.biome, Vec2::new(10.0, 10.0), Locomotion::Water), None);
    }

    #[test]
    fn nearest_valid_searches_the_whole_torus_on_a_large_grid() {
        // 512-cell grid (a continental-scale map): the only Water is column 0,
        // and the agent sits ~200 cells away — far beyond any fixed ring cap.
        let mut w = World::with_dims(1, 4096.0, 512, 16);
        for c in w.biome.cells.iter_mut() {
            c.terrain = TerrainType::Grass;
        }
        let res = w.biome.res;
        for row in 0..res {
            w.biome.at_mut(0, row).terrain = TerrainType::Water;
        }
        let pos = Vec2::new(200.5 * w.biome.cell_size, 100.0);
        let got = nearest_valid(&w.biome, pos, Locomotion::Water).expect("water exists");
        assert_eq!(w.biome.sample(got).terrain, TerrainType::Water);
        assert!((got.x - 0.5 * w.biome.cell_size).abs() < 1e-3, "column 0 centre, got {}", got.x);
    }

    #[test]
    fn graze_matches_occupy() {
        use TerrainType::*;
        for t in [Water, Grass, Forest, Desert, Rock, Savanna, Rainforest, Taiga, Tundra] {
            for c in [Locomotion::Land, Locomotion::Water, Locomotion::Air] {
                assert_eq!(c.can_graze(t), c.can_occupy(t), "{c:?} on {t:?}");
            }
        }
    }

    #[test]
    fn gate_samples_the_wrapped_destination_at_the_seam() {
        // The seed-7 case from the flagship (1024-wide, 8-unit cells): a
        // Water agent in row 0 at y = 0.3134766 steps north by 0.3134973. The
        // raw destination y is -2.07e-5, which f32 `rem_euclid` rounds to
        // exactly 1024.0 — the coordinate integrate stores, and one that
        // every later `cell_coords` maps to row 0. Sampling the RAW value
        // instead clamps to row 127, a different cell.
        let mut w = World::new(1);
        let res = w.biome.res;
        for row in 0..res {
            for col in 0..res {
                w.biome.at_mut(col, row).terrain = TerrainType::Water;
            }
        }
        // Destination column (51) is land in row 0; row 127 is water there.
        w.biome.at_mut(51, 0).terrain = TerrainType::Grass;
        let pos = Vec2::new(416.945, 0.3134766);
        let v = Vec2::new(-2.8718, -0.3134973);
        let raw = pos + v;
        assert!(raw.y < 0.0 && raw.y > -3e-5, "overshoot must be tiny: {}", raw.y);
        assert_eq!(raw.y.rem_euclid(1024.0), 1024.0, "f32 rem_euclid rounds to world_size");
        assert_eq!(w.biome.sample(raw).terrain, TerrainType::Water, "raw sample reads row 127");
        assert_eq!(
            w.biome.sample(wrap_torus(raw, Vec2::splat(1024.0))).terrain,
            TerrainType::Grass,
            "the stored coordinate reads row 0"
        );
        // The gate must judge the stored cell: the full move and the x-only
        // slide both land on (51, 0) = Grass, so only the y-only slide
        // (staying in column 52) is valid.
        let applied = gate_move(&w.biome, Locomotion::Water, pos, v);
        assert_eq!(applied, Vec2::new(0.0, v.y), "y-only slide expected, got {applied:?}");
        let stored = wrap_torus(pos + applied, Vec2::splat(w.world_size));
        assert!(Locomotion::Water.can_occupy(w.biome.sample(stored).terrain));
    }

    #[test]
    fn gate_does_not_tunnel_through_a_one_cell_strip() {
        // Water only in column 16 (x ∈ [128, 136)); Land agent at x = 127 with
        // a 9-unit eastward step: the destination (x = 136, column 17) is
        // valid land, but the path crosses the water strip ⇒ blocked.
        let mut w = World::with_dims(1, 256.0, 32, 16);
        let res = w.biome.res;
        for row in 0..res {
            for col in 0..res {
                w.biome.at_mut(col, row).terrain =
                    if col == 16 { TerrainType::Water } else { TerrainType::Grass };
            }
        }
        let pos = Vec2::new(127.0, 100.0);
        assert_eq!(gate_move(&w.biome, Locomotion::Land, pos, Vec2::new(9.0, 0.0)), Vec2::ZERO);
        // The diagonal slides along the strip instead.
        assert_eq!(
            gate_move(&w.biome, Locomotion::Land, pos, Vec2::new(9.0, 3.0)),
            Vec2::new(0.0, 3.0)
        );
        // A Water agent can't hop the land isthmus between two water columns.
        for row in 0..res {
            w.biome.at_mut(18, row).terrain = TerrainType::Water;
        }
        let wpos = Vec2::new(135.9, 100.0);
        assert_eq!(gate_move(&w.biome, Locomotion::Water, wpos, Vec2::new(9.0, 0.0)), Vec2::ZERO);
        // Short moves are unaffected (single destination sample).
        assert_eq!(
            gate_move(&w.biome, Locomotion::Land, pos, Vec2::new(-3.0, 0.0)),
            Vec2::new(-3.0, 0.0)
        );
    }

    #[test]
    fn pull_allowed_masks_only_under_the_flag() {
        let w = coast_world();
        let pos = Vec2::new(126.0, 100.0);
        let seaward = Vec2::new(1.0, 0.0);
        assert!(pull_allowed(&w.biome, pos, seaward, None), "flag off: never masked");
        assert!(!pull_allowed(&w.biome, pos, seaward, Some(Locomotion::Land)));
        assert!(pull_allowed(&w.biome, pos, Vec2::new(-1.0, 0.0), Some(Locomotion::Land)));
        assert!(pull_allowed(&w.biome, pos, Vec2::ZERO, Some(Locomotion::Land)));
    }

    #[test]
    fn locomotion_mutation_is_damped_tenfold() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Locomotion, 0.6);
        damp_locomotion_mutation(0.5, &mut g);
        assert!((g.get(GenomeSlot::Locomotion) - 0.51).abs() < 1e-6);
    }

    #[test]
    fn instantiate_relocates_founders_onto_their_habitat() {
        let s = crate::scenario::Scenario::parse_toml(include_str!(
            "../../../scenarios/habitat-territories.toml"
        ))
        .expect("parse");
        let w = s.instantiate();
        for id in w.agents.iter_alive() {
            let i = id as usize;
            let class = Locomotion::of(&w.agents.genome[i]);
            let t = w.biome.sample(w.agents.position[i]).terrain;
            assert!(class.can_occupy(t), "founder {id} ({class:?}) spawned on {t:?}");
        }
    }
}
