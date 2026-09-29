//! Turning inertia: a persistent per-agent facing (`AgentBuffers::heading`)
//! that `tick::decide_all` turns toward each tick's wanted movement direction
//! at a bounded rate, so an agent cannot reverse in one tick, herds stop
//! jittering and paths come out as arcs rather than zigzags. Real animals
//! turn at a bounded rate and turn tighter when slow, so the allowance grows
//! as the speed fraction shrinks (`max_turn`).
//!
//! Pure functions only — the single caller gates on `World::turning_enabled`,
//! so a flag-off world never reaches this module and never reads or writes
//! the column (it stays at `SPAWN_HEADING` on every slot). The heading
//! follows the *intended* direction, not the resolved displacement: the
//! habitat gate (`habitat::gate_move`) and the collision resolve
//! (`collision::resolve_overlaps`) may shorten or shift the applied move
//! afterwards, and a body pressed sideways by a crowd keeps facing where it
//! wants to go. Each agent reads and writes only its own slot, with no RNG,
//! so the parallel `decide_all` stays independent of rayon thread count;
//! trig goes through `mathf` (libm) so the turn is bit-identical across hosts.

use crate::mathf::{cosf, sinf};
use crate::prelude::Vec2;

/// Facing every agent is born with, founders and newborns alike: +x. A fixed
/// constant (rather than a parent's heading) keeps the column a pure function
/// of the spawn sequence — no RNG draw, no cross-row read at birth — and makes
/// the flag-off column trivially inert: nothing else writes it.
pub const SPAWN_HEADING: Vec2 = Vec2::new(1.0, 0.0);

/// Largest turn per tick at full speed, in radians (0.6 rad ≈ 34°): a
/// reversal takes six ticks at top speed, and at `integrate::SPEED_MAX_CAP`
/// (4 units per tick) the tightest circle has a radius of ~6.7 units, about
/// nine body diameters (`collision::BODY_R_BASE`).
pub const MAX_TURN_RAD: f32 = 0.6;

/// Speed fraction below which the turn allowance stops growing: the allowance
/// is `MAX_TURN_RAD / max(frac, MIN_TURN_FRAC)`, so a crawling agent turns up
/// to four times as sharply per tick as a sprinting one (2.4 rad — a reversal
/// in two ticks).
pub const MIN_TURN_FRAC: f32 = 0.25;

/// A wanted direction shorter than this is "stand still": the heading is left
/// alone and nothing moves.
pub const MOVE_EPS: f32 = 1e-6;

/// Turn allowance (radians) for one tick at speed fraction `frac`:
/// `MAX_TURN_RAD` at full speed, growing as the agent slows, capped at π (a
/// reversal in one tick) — the cap is a safety net; with the constants above
/// it never binds.
#[inline]
pub fn max_turn(frac: f32) -> f32 {
    (MAX_TURN_RAD / frac.clamp(MIN_TURN_FRAC, 1.0)).min(std::f32::consts::PI)
}

/// Rotate the unit `heading` toward the unit `wanted` direction by at most
/// `max_turn` radians. Snaps to `wanted` when it is within the allowance, so a
/// finished turn faces exactly where it wants (no accumulated rounding);
/// otherwise rotates by exactly the allowance the shorter way round, with an
/// exact reversal (no shorter side) turning counter-clockwise. The result is
/// re-normalised so a long turn (an agent circling a target for hundreds of
/// ticks) cannot drift off the unit circle. A degenerate heading (zero or
/// non-finite — nothing in the engine produces one) snaps to `wanted`.
pub fn turn_toward(heading: Vec2, wanted: Vec2, max_turn: f32) -> Vec2 {
    if !heading.is_finite() || heading.length_squared() < 0.25 || max_turn >= std::f32::consts::PI {
        return wanted;
    }
    // Unit vectors: dot = cos φ, perp_dot = sin φ, with φ the signed angle
    // from `heading` to `wanted` (positive = counter-clockwise).
    let cos_phi = heading.dot(wanted);
    let sin_phi = heading.perp_dot(wanted);
    if cos_phi >= cosf(max_turn) {
        return wanted;
    }
    let theta = if sin_phi >= 0.0 { max_turn } else { -max_turn };
    let (s, c) = (sinf(theta), cosf(theta));
    let turned = Vec2::new(heading.x * c - heading.y * s, heading.x * s + heading.y * c);
    turned / turned.length()
}

/// The direction `decide_all` applies this tick for an agent facing
/// `*heading` that wants `desired`, whose length is its speed fraction
/// (`ZERO` = stand still). Turns the heading toward `desired`'s direction by
/// at most `max_turn(frac)`, stores it, and returns the new heading scaled
/// back by the fraction — the applied move keeps the wanted speed but points
/// where the body can actually face this tick. A zero (or non-finite)
/// `desired` leaves the heading unchanged and returns `ZERO`.
#[inline]
pub fn apply_turn(heading: &mut Vec2, desired: Vec2) -> Vec2 {
    let frac = desired.length();
    if frac < MOVE_EPS || !desired.is_finite() {
        return Vec2::ZERO;
    }
    let turned = turn_toward(*heading, desired / frac, max_turn(frac));
    *heading = turned;
    turned * frac
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn angle_between(a: Vec2, b: Vec2) -> f32 {
        a.dot(b).clamp(-1.0, 1.0).acos()
    }

    /// Ticks of `apply_turn` at speed fraction `frac` until the heading faces
    /// due west from the spawn facing (+x), i.e. a full reversal.
    fn reversal_ticks(frac: f32) -> u32 {
        let mut h = SPAWN_HEADING;
        let wanted = Vec2::new(-frac, 0.0);
        let mut ticks = 0;
        while h != Vec2::new(-1.0, 0.0) {
            apply_turn(&mut h, wanted);
            ticks += 1;
            assert!(ticks < 100, "never finished reversing at frac {frac}: {h:?}");
        }
        ticks
    }

    #[test]
    fn allowance_is_max_turn_at_full_speed_and_grows_when_slow() {
        assert_eq!(max_turn(1.0), MAX_TURN_RAD);
        assert!((max_turn(0.5) - 2.0 * MAX_TURN_RAD).abs() < 1e-6);
        assert!((max_turn(MIN_TURN_FRAC) - MAX_TURN_RAD / MIN_TURN_FRAC).abs() < 1e-6);
        assert_eq!(max_turn(0.0), max_turn(MIN_TURN_FRAC), "floors at MIN_TURN_FRAC");
        assert!(max_turn(0.0) < PI, "the π cap never binds with these constants");
        assert_eq!(max_turn(3.0), MAX_TURN_RAD, "a fraction above 1 turns no slower than 1");
    }

    #[test]
    fn a_reversal_at_full_speed_takes_several_ticks_each_bounded_by_max_turn() {
        let mut h = SPAWN_HEADING;
        let wanted = Vec2::new(-1.0, 0.0);
        let mut ticks = 0;
        while h != wanted {
            let before = h;
            let applied = apply_turn(&mut h, wanted);
            ticks += 1;
            let turned = angle_between(before, h);
            assert!(turned <= MAX_TURN_RAD + 1e-5, "tick {ticks} turned {turned} rad");
            assert!((h.length() - 1.0).abs() < 1e-5, "heading stays unit: {h:?}");
            assert_eq!(applied, h, "at frac 1 the applied direction is the new heading");
            assert!(ticks < 100);
        }
        assert!(ticks > 1, "a reversal must take more than one tick");
        assert_eq!(ticks, (PI / MAX_TURN_RAD).ceil() as u32);
    }

    #[test]
    fn an_exact_reversal_turns_counter_clockwise_from_either_facing() {
        let mut h = SPAWN_HEADING;
        apply_turn(&mut h, Vec2::new(-1.0, 0.0));
        assert!(h.y > 0.0 && h.x < 1.0, "from +x, counter-clockwise goes through +y: {h:?}");
        // From -x the cross product is -0.0: a tie, not "clockwise".
        let mut h = Vec2::new(-1.0, 0.0);
        apply_turn(&mut h, Vec2::new(1.0, 0.0));
        assert!(h.y < 0.0 && h.x > -1.0, "from -x, counter-clockwise goes through -y: {h:?}");
    }

    #[test]
    fn a_slow_agent_turns_more_sharply_than_a_fast_one() {
        let fast = reversal_ticks(1.0);
        let slow = reversal_ticks(MIN_TURN_FRAC);
        assert!(slow < fast, "crawling reversal {slow} ticks vs sprinting {fast}");
        assert_eq!(fast, 6);
        assert_eq!(slow, 2);
        // A right-angle turn: unfinished after one tick at full speed, done in
        // one when crawling — and the applied move keeps the crawling speed.
        let mut h = SPAWN_HEADING;
        apply_turn(&mut h, Vec2::new(0.0, 1.0));
        assert!(angle_between(h, Vec2::new(0.0, 1.0)) > 1e-3, "full speed: not there yet");
        let mut h = SPAWN_HEADING;
        let applied = apply_turn(&mut h, Vec2::new(0.0, MIN_TURN_FRAC));
        assert_eq!(h, Vec2::new(0.0, 1.0), "crawling: snapped in one tick");
        assert!((applied.length() - MIN_TURN_FRAC).abs() < 1e-6, "speed fraction kept");
        assert!((applied - Vec2::new(0.0, MIN_TURN_FRAC)).length() < 1e-6);
    }

    #[test]
    fn standing_still_leaves_the_heading_alone_and_moves_nothing() {
        let north = Vec2::new(0.0, 1.0);
        let mut h = north;
        assert_eq!(apply_turn(&mut h, Vec2::ZERO), Vec2::ZERO);
        assert_eq!(h, north);
        assert_eq!(apply_turn(&mut h, Vec2::new(f32::NAN, 0.0)), Vec2::ZERO);
        assert_eq!(h, north);
    }

    #[test]
    fn a_wanted_direction_within_the_allowance_is_taken_exactly() {
        let wanted = Vec2::new(cosf(0.3), sinf(0.3));
        let mut h = SPAWN_HEADING;
        let applied = apply_turn(&mut h, wanted);
        assert_eq!(h, wanted, "snaps to the exact wanted direction");
        assert_eq!(applied, wanted);
    }

    #[test]
    fn a_long_turn_stays_on_the_unit_circle() {
        // Chase a target that is always a right angle to the left: the heading
        // rotates by the full allowance every tick and never snaps.
        let mut h = SPAWN_HEADING;
        for _ in 0..10_000 {
            let left = Vec2::new(-h.y, h.x);
            apply_turn(&mut h, left);
            assert!((h.length() - 1.0).abs() < 1e-5, "drifted off the unit circle: {h:?}");
        }
    }

    #[test]
    fn a_degenerate_heading_snaps_to_the_wanted_direction() {
        let south = Vec2::new(0.0, -1.0);
        let mut h = Vec2::ZERO;
        apply_turn(&mut h, south);
        assert_eq!(h, south);
        let mut h = Vec2::new(f32::NAN, 0.0);
        apply_turn(&mut h, south);
        assert_eq!(h, south);
    }
}
