//! Growth and juveniles: an agent is born small and grows to adult size over
//! the first `MATURITY_FRAC` of its lifespan, then holds there.
//!
//! The growth fraction is a pure function of the `age` column and the
//! genome's lifespan (`age::lifespan_of`) — no state, no RNG, nothing to
//! serialize. Where the engine reads an adult value today it multiplies by
//! the body scale instead: the collision body radius
//! (`collision::body_radius_at`), the grazing bite (`interact::feed_pass`),
//! basal metabolism and the move cost (`integrate::integrate_all`), plus a
//! milder speed penalty (`speed_scale`); `reproduce::is_eligible` rejects
//! anyone below maturity. All of it is gated on `World::growth_enabled`:
//! off ⇒ every multiplier is exactly `1.0` and the maturity gate is inert,
//! so flag-off worlds are byte-identical (pinned by the
//! `*_trajectory_is_pinned` guards in `tests/determinism.rs`). The viewers
//! (`anabios-wasm`'s `view::fill_agents`, the Godot bridge's `alive_sizes`)
//! export the grown size, so juveniles draw small.
//!
//! Founders spawn at age 0 like every other juvenile window in the engine
//! (`iq::IQ_MATURATION_AGE`, `domestication::TAME_MAX_AGE`): with the flag
//! on, a founding population grows up over its first maturity window and
//! breeds only after it. Maturity reads the genome lifespan alone — the
//! invention lifespan buff (Medicine) lengthens old age, not childhood.

use crate::genome::Genome;

/// Fraction of an individual's lifespan spent growing: `growth_fraction`
/// rises from 0 to 1 over `MATURITY_FRAC · lifespan_of(genome)` ticks (75 at
/// `LifespanBias` 0, 750 at 1, ~413 for the neutral genome) and holds at 1
/// afterwards. Breeding starts at that age.
pub const MATURITY_FRAC: f32 = 0.15;
/// Body scale at birth: a newborn is about a third of its adult size. Body
/// radius, bite, basal metabolism and the move cost scale with the body, so
/// a juvenile eats less, costs less to run and packs closer than an adult.
pub const JUVENILE_BODY: f32 = 0.35;
/// Speed multiplier at body scale 0 (`speed_scale`), so juveniles are only
/// slightly slower than adults: a newborn (body `JUVENILE_BODY`) moves at
/// `0.6 + 0.4 · 0.35 = 0.74×` adult speed, an adult at exactly `1.0×`.
pub const JUVENILE_SPEED: f32 = 0.6;

/// Ticks to maturity for a lifespan, as the `f32` the fraction divides by.
#[inline]
fn maturity(lifespan_ticks: u32) -> f32 {
    lifespan_ticks as f32 * MATURITY_FRAC
}

/// Age (ticks) at which an agent with this lifespan is first mature —
/// `growth_fraction(maturity_ticks(l), l) == 1.0`, and `is_mature` flips
/// there. Test and fixture helper.
#[inline]
pub fn maturity_ticks(lifespan_ticks: u32) -> u32 {
    maturity(lifespan_ticks).ceil() as u32
}

/// Growth fraction of an agent `age` ticks old with lifespan
/// `lifespan_ticks`: 0 at birth, rising along a smoothstep (zero slope at
/// both ends, 0.5 halfway) to exactly 1 at `MATURITY_FRAC · lifespan`, and 1
/// for the rest of its life. A zero lifespan (no juvenile window) is mature.
#[inline]
pub fn growth_fraction(age: u32, lifespan_ticks: u32) -> f32 {
    let m = maturity(lifespan_ticks);
    let a = age as f32;
    if a >= m {
        return 1.0;
    }
    let t = a / m;
    t * t * (3.0 - 2.0 * t)
}

/// Body scale for a growth fraction: `JUVENILE_BODY + (1 − JUVENILE_BODY) ·
/// fraction`, written as a lerp from the adult end so a fraction of 1 gives
/// exactly `1.0` (an adult with the flag on is arithmetically an adult).
#[inline]
pub fn body_scale(fraction: f32) -> f32 {
    1.0 - (1.0 - JUVENILE_BODY) * (1.0 - fraction)
}

/// Speed multiplier for a body scale: `JUVENILE_SPEED + (1 − JUVENILE_SPEED)
/// · body`, in the same exact-at-one form as `body_scale`.
#[inline]
pub fn speed_scale(body: f32) -> f32 {
    1.0 - (1.0 - JUVENILE_SPEED) * (1.0 - body)
}

/// Body scale of an agent `age` ticks old with genome `g`, or exactly `1.0`
/// when growth is off — the flag-off identity every consumer relies on.
#[inline]
pub fn body_scale_of(growth_enabled: bool, age: u32, g: &Genome) -> f32 {
    if !growth_enabled {
        return 1.0;
    }
    body_scale(growth_fraction(age, crate::age::lifespan_of(g)))
}

/// Whether an agent may breed: at or past maturity (`growth_fraction == 1`),
/// or always when growth is off.
#[inline]
pub fn is_mature(growth_enabled: bool, age: u32, g: &Genome) -> bool {
    !growth_enabled || age as f32 >= maturity(crate::age::lifespan_of(g))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::age::{lifespan_of, LIFESPAN_MAX_TICKS, LIFESPAN_MIN_TICKS};
    use crate::genome::GenomeSlot;

    #[test]
    fn growth_fraction_runs_from_zero_at_birth_to_one_at_maturity() {
        for l in [LIFESPAN_MIN_TICKS, 2750, LIFESPAN_MAX_TICKS] {
            assert_eq!(growth_fraction(0, l), 0.0, "lifespan {l}: newborn");
            let m = maturity_ticks(l);
            assert!(growth_fraction(m - 1, l) < 1.0, "lifespan {l}: still growing at {}", m - 1);
            assert_eq!(growth_fraction(m, l), 1.0, "lifespan {l}: mature at {m}");
            assert_eq!(growth_fraction(l, l), 1.0, "lifespan {l}: adult for life");
            assert_eq!(growth_fraction(u32::MAX, l), 1.0);
        }
        assert_eq!(maturity_ticks(LIFESPAN_MIN_TICKS), 75);
        assert_eq!(maturity_ticks(LIFESPAN_MAX_TICKS), 750);
        assert_eq!(growth_fraction(0, 0), 1.0, "no juvenile window: mature at birth");
    }

    #[test]
    fn growth_fraction_is_monotonic_and_smooth() {
        let l = LIFESPAN_MAX_TICKS;
        let m = maturity_ticks(l);
        let mut prev = growth_fraction(0, l);
        let mut max_step = 0.0f32;
        for age in 1..=m {
            let f = growth_fraction(age, l);
            assert!(f >= prev, "age {age}: {f} < {prev}");
            if age < m {
                assert!(f > prev, "age {age}: strictly rising while juvenile");
            }
            max_step = max_step.max(f - prev);
            prev = f;
        }
        // Smoothstep: flat at both ends (a linear ramp would already be 1/m
        // ≈ 1.3e-3 after one tick), steepest (1.5/m) halfway, 0.5 halfway.
        assert!(growth_fraction(1, l) < 1e-4, "{}", growth_fraction(1, l));
        assert!(1.0 - growth_fraction(m - 1, l) < 1e-4);
        assert!(max_step < 2.0 / m as f32, "max per-tick step {max_step}");
        assert!((growth_fraction(m / 2, l) - 0.5).abs() < 1e-2);
    }

    #[test]
    fn body_and_speed_scales_span_their_juvenile_floor_to_exactly_one() {
        assert!((body_scale(0.0) - JUVENILE_BODY).abs() < 1e-6);
        assert_eq!(body_scale(1.0), 1.0);
        let half = JUVENILE_BODY + 0.5 * (1.0 - JUVENILE_BODY);
        assert!((body_scale(0.5) - half).abs() < 1e-6);
        let newborn_speed = JUVENILE_SPEED + (1.0 - JUVENILE_SPEED) * JUVENILE_BODY;
        assert!((speed_scale(JUVENILE_BODY) - newborn_speed).abs() < 1e-6);
        assert_eq!(speed_scale(1.0), 1.0);
        // Juveniles are slower, but far less so than they are small.
        assert!(speed_scale(JUVENILE_BODY) > JUVENILE_BODY);
    }

    #[test]
    fn maturity_scales_with_the_individual_lifespan() {
        let mut short = Genome::neutral();
        short.set(GenomeSlot::LifespanBias, 0.0);
        let mut long = Genome::neutral();
        long.set(GenomeSlot::LifespanBias, 1.0);
        let (ms, ml) = (maturity_ticks(lifespan_of(&short)), maturity_ticks(lifespan_of(&long)));
        assert!(ms < ml, "{ms} vs {ml}");
        // At the short-lived genome's maturity the long-lived one is still growing.
        assert!(is_mature(true, ms, &short));
        assert!(!is_mature(true, ms, &long));
        assert!(body_scale_of(true, ms, &long) < 1.0);
        assert_eq!(body_scale_of(true, ms, &short), 1.0);
    }

    #[test]
    fn flag_off_is_exactly_the_adult_identity() {
        let g = Genome::neutral();
        let m = maturity_ticks(lifespan_of(&g));
        for age in [0, 1, 100, m, u32::MAX] {
            assert_eq!(body_scale_of(false, age, &g), 1.0, "age {age}");
            assert!(is_mature(false, age, &g), "age {age}");
        }
        // Flag on: a newborn is JUVENILE_BODY of an adult and not mature; at
        // maturity it is exactly an adult.
        assert!((body_scale_of(true, 0, &g) - JUVENILE_BODY).abs() < 1e-6);
        assert!(!is_mature(true, 0, &g));
        assert!(!is_mature(true, m - 1, &g));
        assert!(is_mature(true, m, &g));
        assert_eq!(body_scale_of(true, m, &g), 1.0);
    }
}
