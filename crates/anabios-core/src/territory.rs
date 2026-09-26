//! Species territories (territory/habitat/collision layer): a persistent
//! per-species range — centre, radius, locomotion class — maintained every
//! species step, and the soft-edge pull that keeps members roaming inside it.
//! Everything is gated on `World::territory_enabled`.
//!
//! Amended 2026-09-25 after the territory diagnosis
//! (`.superpowers/sdd/2026-09-25-territory-habitat-collision/territory-diagnosis.md`):
//! the original `TERRITORY_PULL` lever alone could not lift `inside_territory`
//! (measured ≤ 1/8 seeds ≥ 80%) because (1) the pull is a fixed-size vector
//! added to an unbounded, evolvable move intent that can reach 10²–10⁶ in
//! magnitude, drowning the pull after normalization, and (2) the old
//! zero-to-`r`-then-ramp-to-`2r` geometry put an outward-steering member's
//! stall point outside `r`, exactly where the metric draws its line. The
//! fix has three parts: `decide_all` unit-caps the intent accumulated so far
//! before adding a non-zero pull (`apply_territory_pull`); `territory_pull`
//! now ramps from `TERRITORY_FREE_FRAC · r` to full strength at `r`; and the
//! per-capita range (`TERRITORY_K`, `TERRITORY_R_MAX`) was enlarged so a
//! binding range can still feed its members. Validated at 6/8 seeds ≥ 80%,
//! zero extinctions (see the diagnosis §6/§9/§10).

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
/// Radius per √member: `r = clamp(K·√n, R_MIN, R_MAX)`. K and R_MAX were
/// raised 12→24 / 256→512 (Task 9b, 2026-09-25 territory diagnosis, §9c): at
/// the old values (≈452 unit² per member) the range's own interior was
/// grazed out, so a pull strong enough to hold members there starved them
/// (measured: 5/8 seeds went fully extinct). Quadrupling per-capita area
/// (≈1810 unit²) let the geometry fix below hold members without
/// extinctions. Caveat: on the flagship's 1024-wide torus, `R_MAX = 512`
/// means a max-size range can cover most of the world — see the design
/// spec's Amended note and the findings doc.
pub const TERRITORY_K: f32 = 24.0;
pub const TERRITORY_R_MIN: f32 = 48.0;
pub const TERRITORY_R_MAX: f32 = 512.0;
/// Homing pull at Territoriality = 1 and the ramp fully engaged (full
/// strength at `r`, see `territory_pull`). Raised 1.0 → 2.5 (Task 9 round 3)
/// to try to lift `inside_territory`; by itself this was swamped by
/// unbounded evolved move intents (Task 9b diagnosis, H6) until
/// `apply_territory_pull` unit-caps the intent accumulated so far in
/// `decide_all` before this pull is added.
pub const TERRITORY_PULL: f32 = 2.5;
/// Free-roam fraction of `r`: a member inside `TERRITORY_FREE_FRAC · r` feels
/// no pull; the pull ramps from there to full strength at `r` (not `2r` as
/// before). Task 9b, 2026-09-25 territory diagnosis (§9b): the old
/// zero-to-`r`-then-ramp-to-`2r` geometry put an outward-steering member's
/// stall point outside `r` by construction — exactly where `inside_territory`
/// draws its line.
pub const TERRITORY_FREE_FRAC: f32 = 0.5;

/// Territory radius for a species of `members` alive agents.
pub fn radius_for(members: u32) -> f32 {
    (TERRITORY_K * (members as f32).sqrt()).clamp(TERRITORY_R_MIN, TERRITORY_R_MAX)
}

/// Soft-edge homing pull toward the territory centre: zero inside
/// `TERRITORY_FREE_FRAC · r` (free roam), then
/// `min((d − inner)/(r − inner), 1) · TERRITORY_PULL · terr` toward the
/// centre, where `inner = TERRITORY_FREE_FRAC · r` — full strength is
/// reached at `r` (previously `2r`), so an outward-steering member's stall
/// point falls inside the range rather than on a shell beyond it. Unset rows
/// pull nothing. Applied in `decide_all` behind the flag, where the intent
/// accumulated so far is unit-capped before this pull is added (see
/// `apply_territory_pull`) so an unbounded evolved intent can't swamp it.
pub fn territory_pull(t: &Territory, pos: Vec2, terr: f32, ws: f32) -> Vec2 {
    if !t.is_set() {
        return Vec2::ZERO;
    }
    let d = crate::spatial::torus_delta(t.centre(), pos, ws);
    let dist = d.length();
    let inner = TERRITORY_FREE_FRAC * t.r;
    if dist <= inner {
        return Vec2::ZERO;
    }
    let ramp = ((dist - inner) / (t.r - inner)).min(1.0);
    (d / dist) * (ramp * TERRITORY_PULL * terr)
}

/// Cap the move intent accumulated in `decide_all` so far to unit length
/// before adding a non-zero territory pull, then add it (Task 9b, 2026-09-25
/// territory diagnosis, §9a). Evolved programs feed sensor values (energy,
/// distances clamped at 1e6) into `MoveToward*`, which by mid-run can reach
/// intents of magnitude 10²–10⁶ — added directly, a `TERRITORY_PULL`-sized
/// bias is invisible once the sum is normalized to a direction (H6). Capping
/// only the intent built so far (not later biases in the stack) keeps the
/// fix local to the territory layer.
///
/// A zero `pull` (inside the free-roam zone) is a pure no-op: `action_xy` is
/// returned unclamped and unchanged, so flag-on agents inside their range are
/// unaffected. A non-finite `action_xy` (`inf`/`NaN` from an overflowing
/// program) is left uncapped too — `decide_all`'s own finite check zeroes the
/// final direction regardless, and clamping `NaN` would not help.
pub fn apply_territory_pull(action_xy: Vec2, pull: Vec2) -> Vec2 {
    let v = if pull != Vec2::ZERO && action_xy.is_finite() {
        action_xy.clamp_length_max(1.0)
    } else {
        action_xy
    };
    v + pull
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
        // Task 9b geometry: free roam out to `TERRITORY_FREE_FRAC · r`, then
        // ramp to full strength at `r` (not `2r`), capped beyond it.
        let t = Territory { cx: 500.0, cy: 500.0, r: 100.0, class: Locomotion::Land };
        let inner = TERRITORY_FREE_FRAC * t.r;
        // inner == 50.0. Free-roam zone: zero strictly inside and exactly at
        // the boundary.
        assert_eq!(
            territory_pull(&t, Vec2::new(500.0 + 0.4 * t.r, 500.0), 1.0, 1024.0),
            Vec2::ZERO,
            "d = 0.4r is inside the free-roam zone"
        );
        assert_eq!(
            territory_pull(&t, Vec2::new(500.0 + inner, 500.0), 1.0, 1024.0),
            Vec2::ZERO,
            "d = 0.5r (the inner boundary) is still free"
        );
        // Half pull at the ramp's midpoint, d = 0.75r.
        let half = territory_pull(&t, Vec2::new(500.0 + 0.75 * t.r, 500.0), 1.0, 1024.0);
        assert!(half.x < 0.0 && half.y == 0.0, "points home: {half:?}");
        assert!(
            (half.length() - 0.5 * TERRITORY_PULL).abs() < 1e-4,
            "half pull at the ramp midpoint: {half:?}"
        );
        // Full pull at r, capped (not stronger) beyond it.
        let at_r = territory_pull(&t, Vec2::new(500.0 + t.r, 500.0), 1.0, 1024.0);
        let beyond = territory_pull(&t, Vec2::new(500.0 + 1.5 * t.r, 500.0), 1.0, 1024.0);
        assert!((at_r.length() - TERRITORY_PULL).abs() < 1e-4, "full pull at r: {at_r:?}");
        assert!(
            (beyond.length() - TERRITORY_PULL).abs() < 1e-4,
            "capped at full pull beyond r: {beyond:?}"
        );
        // Scaled by Territoriality.
        assert!(
            (territory_pull(&t, Vec2::new(500.0 + t.r, 500.0), 0.5, 1024.0).length()
                - 0.5 * TERRITORY_PULL)
                .abs()
                < 1e-4,
            "scaled by Territoriality"
        );
        // Unset territory pulls nothing.
        assert_eq!(territory_pull(&Territory::default(), Vec2::ZERO, 1.0, 1024.0), Vec2::ZERO);
    }

    #[test]
    fn radius_follows_sqrt_members_and_clamps() {
        assert_eq!(radius_for(1), TERRITORY_R_MIN);
        assert!((radius_for(100) - 240.0).abs() < 1e-4, "K=24 * sqrt(100) = 240");
        assert_eq!(radius_for(1_000_000), TERRITORY_R_MAX);
    }

    #[test]
    fn apply_territory_pull_caps_a_huge_outward_intent_before_adding_the_pull() {
        // An evolved intent of magnitude ~1000 pointing away from home (H6 in
        // the Task 9b diagnosis); the pull points home (+x). Without the cap
        // the pull would be invisible after normalization; with it, the
        // capped intent (length <= 1) can't out-weigh the pull.
        let action = Vec2::new(-1000.0, 0.0);
        let pull = Vec2::new(TERRITORY_PULL, 0.0);
        let result = apply_territory_pull(action, pull);
        let home = Vec2::new(1.0, 0.0);
        assert!(
            result.dot(home) > 0.0,
            "clamped intent + pull must point toward home despite a huge outward intent: {result:?}"
        );
        assert!(
            (result - Vec2::new(TERRITORY_PULL - 1.0, 0.0)).length() < 1e-4,
            "intent clamped to unit length, then pull added: {result:?}"
        );
    }

    #[test]
    fn apply_territory_pull_is_a_noop_when_the_pull_is_zero() {
        // Inside the free-roam zone the pull is ZERO; the intent (even huge
        // or unnormalized) must pass through unclamped and unchanged.
        let action = Vec2::new(37.0, -1234.0);
        assert_eq!(apply_territory_pull(action, Vec2::ZERO), action);
    }

    #[test]
    fn apply_territory_pull_leaves_a_non_finite_intent_uncapped() {
        // Mirrors decide_all's own finite guard downstream: don't try to
        // clamp `inf`/`NaN`, just add the pull and let the caller's finite
        // check zero the final direction.
        let action = Vec2::new(f32::INFINITY, 0.0);
        let pull = Vec2::new(1.0, 2.0);
        let result = apply_territory_pull(action, pull);
        assert_eq!(result, Vec2::new(f32::INFINITY, 2.0));
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
