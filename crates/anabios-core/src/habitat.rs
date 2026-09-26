//! Habitat locomotion (territory/habitat/collision layer). Each agent's
//! heritable `GenomeSlot::Locomotion` gene reads as Land / Water / Air, which
//! decides the terrain it may occupy and graze and which bodies it collides
//! with. Pure functions only — every caller gates on `World::territory_enabled`,
//! so a flag-off world never reaches this module.

use serde::{Deserialize, Serialize};

use crate::biome::{BiomeField, TerrainType};
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;

/// Genes below this read as Water.
pub const WATER_MAX: f32 = 0.25;
/// Genes at or above this read as Air.
pub const AIR_MIN: f32 = 0.75;
/// Scale applied to the Locomotion slot's per-birth mutation delta, so class
/// flips are rare (same RNG draw count — only the magnitude shrinks).
pub const LOCOMOTION_MUTATION_SCALE: f32 = 0.1;
/// Farthest ring (in biome cells) `nearest_valid` searches.
pub const RELOCATE_MAX_RING: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum Locomotion {
    #[default]
    Land = 0,
    Water = 1,
    Air = 2,
}

impl Locomotion {
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

    /// Whether this class may graze biomass in a cell of terrain `t`
    /// (flyers feed on land only).
    #[inline]
    pub fn can_graze(self, t: TerrainType) -> bool {
        match self {
            Locomotion::Water => t == TerrainType::Water,
            Locomotion::Land | Locomotion::Air => t != TerrainType::Water,
        }
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
/// the full move if its destination is valid, else the x-only move, else the
/// y-only move (a slide along the coastline), else no move. Air is never
/// gated. Returns the displacement actually applied.
pub fn gate_move(biome: &BiomeField, class: Locomotion, pos: Vec2, v: Vec2) -> Vec2 {
    if class == Locomotion::Air {
        return v;
    }
    let ok = |d: Vec2| class.can_occupy(biome.sample(pos + d).terrain);
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
/// the closest valid cell on the first ring (Chebyshev radius 1, 2, …, up to
/// `RELOCATE_MAX_RING`) that has one. Within a ring the Euclidean-closest
/// cell wins, first-seen on ties (fixed scan order ⇒ deterministic, no RNG).
/// `None` if no valid cell is within reach.
pub fn nearest_valid(biome: &BiomeField, pos: Vec2, class: Locomotion) -> Option<Vec2> {
    if class.can_occupy(biome.sample(pos).terrain) {
        return Some(pos);
    }
    let (cx, cy) = biome.cell_coords(pos);
    let res = biome.res as i32;
    let max_ring = RELOCATE_MAX_RING.min(biome.res / 2) as i32;
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
        assert!(Air.can_graze(TerrainType::Grass) && !Air.can_graze(TerrainType::Water));
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
