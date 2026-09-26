//! Species territories (territory/habitat/collision layer): a persistent
//! per-species range — centre, radius, locomotion class — maintained every
//! species step, and the soft-edge pull that keeps members roaming inside it.
//! Everything is gated on `World::territory_enabled`.

use serde::{Deserialize, Serialize};

use crate::habitat::Locomotion;
use crate::prelude::Vec2;

/// One species' territory. `r == 0.0` marks a row not yet initialized (no
/// member seen by `territory_step`), which applies no pull.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Territory {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
    pub class: Locomotion,
}

impl Territory {
    #[inline]
    pub fn is_set(&self) -> bool {
        self.r > 0.0
    }

    #[inline]
    pub fn centre(&self) -> Vec2 {
        Vec2::new(self.cx, self.cy)
    }
}

/// EMA rate at which a territory centre follows its members' mean position,
/// per species step (`species::SPECIES_STEP_INTERVAL` ticks).
pub const TERRITORY_CENTRE_RATE: f32 = 0.1;
/// Radius per √member: `r = clamp(K·√n, R_MIN, R_MAX)`.
pub const TERRITORY_K: f32 = 12.0;
pub const TERRITORY_R_MIN: f32 = 48.0;
pub const TERRITORY_R_MAX: f32 = 256.0;
/// Homing pull at Territoriality = 1 once a member is ≥ 2r from the centre.
/// Raised 1.0 → 2.5 (Task 9 round 3, the plan's prescribed lever) to try to
/// lift `inside_territory`, which stayed mostly below 80% through rounds 1–2.
pub const TERRITORY_PULL: f32 = 2.5;

/// Territory radius for a species of `members` alive agents.
pub fn radius_for(members: u32) -> f32 {
    (TERRITORY_K * (members as f32).sqrt()).clamp(TERRITORY_R_MIN, TERRITORY_R_MAX)
}

/// Soft-edge homing pull toward the territory centre: zero inside radius `r`
/// (free roam), then `min((d − r)/r, 1) · TERRITORY_PULL · terr` toward the
/// centre. Unset rows pull nothing. Applied in `decide_all` behind the flag.
pub fn territory_pull(t: &Territory, pos: Vec2, terr: f32, ws: f32) -> Vec2 {
    if !t.is_set() {
        return Vec2::ZERO;
    }
    let d = crate::spatial::torus_delta(t.centre(), pos, ws);
    let dist = d.length();
    if dist <= t.r {
        return Vec2::ZERO;
    }
    let ramp = ((dist - t.r) / t.r).min(1.0);
    (d / dist) * (ramp * TERRITORY_PULL * terr)
}

/// Per-species territory update, run right after `species::species_step`.
/// Grows `species_territories` to `next_species_id` (a new species inherits
/// its parent's record), then for every species with members: centre ← EMA
/// toward the members' torus-safe mean position (mean offset from a reference
/// point — the current centre, or the first member for an unset row — so the
/// seam never splits a species), radius ← `radius_for(n)`, class ← majority
/// member class (lowest index wins ties). Ascending id order, f64 sums, no
/// RNG, no trig ⇒ deterministic. No-op with the flag off.
pub fn territory_step(world: &mut crate::world::World) {
    if !world.territory_enabled {
        return;
    }
    let ws = world.world_size;
    let n = world.next_species_id as usize;
    while world.species_territories.len() < n {
        let sid = world.species_territories.len();
        let inherited = world
            .species_parents
            .get(sid)
            .copied()
            .flatten()
            .and_then(|p| world.species_territories.get(p as usize).copied())
            .unwrap_or_default();
        world.species_territories.push(inherited);
    }

    #[derive(Clone, Copy, Default)]
    struct Acc {
        reference: Option<Vec2>,
        dx: f64,
        dy: f64,
        count: u32,
        votes: [u32; 3],
    }
    let mut acc = vec![Acc::default(); n];
    let territories = &world.species_territories;
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i] as usize;
        let pos = world.agents.position[i];
        let a = &mut acc[sid];
        let reference = *a.reference.get_or_insert_with(|| {
            let t = territories[sid];
            if t.is_set() {
                t.centre()
            } else {
                pos
            }
        });
        let d = crate::spatial::torus_delta(pos, reference, ws);
        a.dx += d.x as f64;
        a.dy += d.y as f64;
        a.count += 1;
        a.votes[Locomotion::of(&world.agents.genome[i]).index()] += 1;
    }

    for (sid, a) in acc.iter().enumerate() {
        let Some(reference) = a.reference else {
            continue;
        };
        let inv = 1.0 / a.count as f64;
        let centroid = Vec2::new(
            (reference.x + (a.dx * inv) as f32).rem_euclid(ws),
            (reference.y + (a.dy * inv) as f32).rem_euclid(ws),
        );
        let t = &mut world.species_territories[sid];
        if t.is_set() {
            let d = crate::spatial::torus_delta(centroid, t.centre(), ws);
            t.cx = (t.cx + d.x * TERRITORY_CENTRE_RATE).rem_euclid(ws);
            t.cy = (t.cy + d.y * TERRITORY_CENTRE_RATE).rem_euclid(ws);
        } else {
            t.cx = centroid.x;
            t.cy = centroid.y;
        }
        t.r = radius_for(a.count);
        let mut best = 0;
        for k in 1..3 {
            if a.votes[k] > a.votes[best] {
                best = k;
            }
        }
        t.class = [Locomotion::Land, Locomotion::Water, Locomotion::Air][best];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::{Genome, GenomeSlot};
    use crate::world::World;

    #[test]
    fn pull_is_zero_inside_and_ramps_outside() {
        let t = Territory { cx: 500.0, cy: 500.0, r: 100.0, class: Locomotion::Land };
        assert_eq!(territory_pull(&t, Vec2::new(550.0, 500.0), 1.0, 1024.0), Vec2::ZERO);
        assert_eq!(territory_pull(&t, Vec2::new(600.0, 500.0), 1.0, 1024.0), Vec2::ZERO);
        let near = territory_pull(&t, Vec2::new(650.0, 500.0), 1.0, 1024.0);
        let far = territory_pull(&t, Vec2::new(750.0, 500.0), 1.0, 1024.0);
        assert!(near.x < 0.0 && near.y == 0.0, "points home: {near:?}");
        assert!((near.length() - 0.5 * TERRITORY_PULL).abs() < 1e-5);
        assert!((far.length() - TERRITORY_PULL).abs() < 1e-5, "capped at full pull");
        assert!(
            (territory_pull(&t, Vec2::new(750.0, 500.0), 0.5, 1024.0).length()
                - 0.5 * TERRITORY_PULL)
                .abs()
                < 1e-5,
            "scaled by Territoriality"
        );
        assert_eq!(territory_pull(&Territory::default(), Vec2::ZERO, 1.0, 1024.0), Vec2::ZERO);
    }

    #[test]
    fn radius_follows_sqrt_members_and_clamps() {
        assert_eq!(radius_for(1), TERRITORY_R_MIN);
        assert!((radius_for(100) - 120.0).abs() < 1e-4);
        assert_eq!(radius_for(1_000_000), TERRITORY_R_MAX);
    }

    fn world_with(positions: &[(f32, f32)], loco: f32) -> World {
        let mut w = World::new(3);
        w.territory_enabled = true;
        for &(x, y) in positions {
            let mut g = Genome::neutral();
            g.set(GenomeSlot::Locomotion, loco);
            w.spawn_agent(Vec2::new(x, y), g);
        }
        w
    }

    #[test]
    fn step_initializes_centre_across_the_torus_seam() {
        // Members straddle x = 0: the torus mean is ~x = 0, not x = 512.
        let mut w =
            world_with(&[(1020.0, 300.0), (4.0, 300.0), (1016.0, 300.0), (8.0, 300.0)], 0.5);
        territory_step(&mut w);
        let t = w.species_territories[0];
        assert!(t.is_set());
        let dx = crate::spatial::torus_delta(Vec2::new(t.cx, 0.0), Vec2::new(0.0, 0.0), 1024.0).x;
        assert!(dx.abs() < 1e-3, "centre at the seam, got cx={}", t.cx);
        assert!((t.cy - 300.0).abs() < 1e-3);
        assert_eq!(t.class, Locomotion::Land);
        assert_eq!(t.r, radius_for(4));
    }

    #[test]
    fn step_moves_an_existing_centre_by_the_ema_rate() {
        let mut w = world_with(&[(600.0, 500.0)], 0.1);
        w.species_territories.push(Territory {
            cx: 500.0,
            cy: 500.0,
            r: 50.0,
            class: Locomotion::Land,
        });
        territory_step(&mut w);
        let t = w.species_territories[0];
        assert!((t.cx - (500.0 + 100.0 * TERRITORY_CENTRE_RATE)).abs() < 1e-3);
        assert_eq!(t.class, Locomotion::Water, "majority class of members");
    }

    #[test]
    fn new_species_inherit_the_parent_record() {
        let mut w = world_with(&[(100.0, 100.0)], 0.5);
        territory_step(&mut w);
        let parent = w.species_territories[0];
        let child = w.push_species(Genome::neutral(), Some(0));
        territory_step(&mut w); // child has no members yet: row inherited, not updated
        assert_eq!(w.species_territories[child as usize], parent);
    }

    #[test]
    fn step_is_a_noop_with_the_flag_off() {
        let mut w = world_with(&[(100.0, 100.0)], 0.5);
        w.territory_enabled = false;
        territory_step(&mut w);
        assert!(w.species_territories.is_empty());
    }
}
