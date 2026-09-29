//! Gait — movement speed chosen from urgency (opt-in, `World::gait_enabled`).
//!
//! Without it every agent with any movement intent covers its full Locomotor
//! speed each tick: a grazing herd crosses two body lengths a tick exactly
//! like a fleeing one (median step ~2.3 world units on `predator-prey`).
//! Real animals graze and idle slowly, walk to what they want, and run only
//! to flee, fight or hunt. With the flag on, `decide_all` folds a speed
//! fraction in `[0, 1]` into the LENGTH of `desired_direction` (a unit vector
//! with the flag off), chosen from
//!
//! 1. how hard the program pushes — `|intent| / GAIT_FULL_INTENT`, the move
//!    intent accumulated over the whole bias stack before normalisation,
//!    clamped to `[0, 1]` (evolved programs emit intents of any magnitude),
//!    and
//! 2. the mood arbiter's winner (`mood.rs`): FLEE, FIGHT and hunting force a
//!    sprint whatever the intent's size; the seeking moods cap the fraction
//!    at a walk; CONTENT (grazing, idling) at an amble.
//!
//! `integrate_all` then applies `desired_direction × top_speed` exactly as
//! before — the shorter vector is the slower move — and multiplies the move
//! cost by the superlinear `1 + GAIT_SPRINT_COST · frac²`, so sprinting is
//! expensive per unit distance and ambling nearly free. Every function here
//! is a pure function of one agent's own row (no RNG, no cross-agent read),
//! so `decide_all` / `integrate_all` stay index-disjoint under rayon. Flag
//! off ⇒ none of this runs: the direction stays `v / |v|` and the cost
//! factor is never applied, byte-identical to a build without the module.

use crate::mood::{FIGHT, FLEE, SEEK_FOOD, SEEK_MATE, SEEK_WATER};
use crate::prelude::Vec2;

/// Move-intent magnitude (`|(move_x, move_y)|` before normalisation) that
/// reads as full effort: `|intent| / GAIT_FULL_INTENT` is the intent
/// fraction, clamped to `[0, 1]`. A starter program's steer is a unit sensor
/// direction (`SensePlantDirX/Y`, `SenseOtherDirX/Y`), so 1.0 means "the
/// program asks with a whole unit vector"; the additive pulls (affect, mood,
/// separation, habitat, territory) push past it and are clamped, while an
/// evolved program that scales its steer down asks for a slower move.
pub const GAIT_FULL_INTENT: f32 = 1.0;
/// Amble: the fraction of top speed a CONTENT agent (grazing, idling) may
/// use at most. Also the cap in MATE (the intent is held to
/// `mood::MOOD_MATE_HOLD` anyway) and SLEEP (`integrate_all` suppresses the
/// move outright).
pub const GAIT_AMBLE: f32 = 0.3;
/// Walk: the cap for the seeking moods (SEEK_FOOD, SEEK_WATER, SEEK_MATE) —
/// purposeful travel toward something wanted, still short of a run.
pub const GAIT_WALK: f32 = 0.6;
/// Sprint: FLEE, FIGHT and a hunter closing on prey run flat out, whatever
/// the intent's magnitude — the reflex, not the program, sets the pace.
pub const GAIT_SPRINT: f32 = 1.0;
/// Superlinear move-cost gain: the per-unit-distance move cost is multiplied
/// by `1 + GAIT_SPRINT_COST · frac²`, so a sprint costs twice per unit
/// distance what a crawl does, a walk 1.36×, an amble 1.09×. Per tick (the
/// distance itself scales with `frac`) the cost grows as `frac · (1 +
/// GAIT_SPRINT_COST · frac²)`: cubic at the top, so a fleeing or hunting
/// agent burns energy far faster than a grazing one, as it should.
pub const GAIT_SPRINT_COST: f32 = 1.0;
/// Mouth carnivory (`module::effective_diet_carnivory`) at or above which an
/// armed agent closing on another species counts as hunting: the ape band's
/// omnivore midpoint (`scenario::make_omnivore`), so an armed ape hunts,
/// while a grazer (0.0) that evolves a weapon fights but never hunts.
pub const GAIT_HUNT_CARNIVORY: f32 = 0.5;

/// The speed cap the mood imposes: sprint for FLEE / FIGHT, walk for the
/// seeking moods, amble for everything else (CONTENT, MATE, SLEEP).
#[inline]
pub fn mood_cap(mood: u8) -> f32 {
    match mood {
        FLEE | FIGHT => GAIT_SPRINT,
        SEEK_FOOD | SEEK_WATER | SEEK_MATE => GAIT_WALK,
        _ => GAIT_AMBLE,
    }
}

/// A hunter closing on prey. Predation in this engine is an attack — a
/// weapon strike at contact on the nearest other-species agent
/// (`interact::combat_pass`, `fire_intent > FIRE_THRESHOLD`), whose carcass
/// the carnivore then eats (`scavenge_pass`) — and the starter hunters only
/// raise `fire_intent` inside strike range, so the chase itself is read from
/// the movement: a meat-eater (`carnivory >= GAIT_HUNT_CARNIVORY`) carrying
/// a weapon, with an other-species agent in perception (`has_other`) and a
/// move intent with a component toward it (`intent · other_dir > 0`), is
/// hunting. A carnivore backing away from a bigger predator is not.
#[inline]
pub fn is_hunting(
    carnivory: f32,
    armed: bool,
    intent: Vec2,
    has_other: bool,
    other_dir: Vec2,
) -> bool {
    carnivory >= GAIT_HUNT_CARNIVORY && armed && has_other && intent.dot(other_dir) > 0.0
}

/// The speed fraction in `[0, 1]` for one agent this tick: `GAIT_SPRINT` when
/// hunting or in FLEE / FIGHT, otherwise the clamped intent fraction
/// `|intent| / GAIT_FULL_INTENT` capped by the mood (`mood_cap`).
/// `intent_len` must be finite (`decide_all` zeroes a non-finite intent
/// before it gets here).
#[inline]
pub fn speed_fraction(intent_len: f32, mood: u8, hunting: bool) -> f32 {
    if hunting || mood == FLEE || mood == FIGHT {
        return GAIT_SPRINT;
    }
    (intent_len / GAIT_FULL_INTENT).clamp(0.0, 1.0).min(mood_cap(mood))
}

/// Move-cost multiplier at speed fraction `frac`: `1 + GAIT_SPRINT_COST ·
/// frac²`. Applied by `integrate_all` on the flag-on path only.
#[inline]
pub fn move_cost_factor(frac: f32) -> f32 {
    1.0 + GAIT_SPRINT_COST * frac * frac
}

/// The unit heading of a `desired_direction` that gait may have shortened
/// (`Vec2::ZERO` stays zero). Readers that compare directions by dot product
/// against a fixed threshold — the codex's structured-signaling detector,
/// any viewer deriving a rotation — use this rather than the raw vector
/// when `gait_enabled`, so a slow agent still counts as facing where it
/// faces.
#[inline]
pub fn heading(dir: Vec2) -> Vec2 {
    dir.normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mood::{CONTENT, MATE, SLEEP};

    #[test]
    fn mood_caps_follow_the_ladder() {
        assert_eq!(mood_cap(FLEE), GAIT_SPRINT);
        assert_eq!(mood_cap(FIGHT), GAIT_SPRINT);
        assert_eq!(mood_cap(SEEK_FOOD), GAIT_WALK);
        assert_eq!(mood_cap(SEEK_WATER), GAIT_WALK);
        assert_eq!(mood_cap(SEEK_MATE), GAIT_WALK);
        assert_eq!(mood_cap(CONTENT), GAIT_AMBLE);
        assert_eq!(mood_cap(MATE), GAIT_AMBLE);
        assert_eq!(mood_cap(SLEEP), GAIT_AMBLE);
        // Amble < walk < sprint (= the full step) — compared on the caps the
        // moods actually yield, so a constant edit that broke the ladder
        // fails here.
        let (amble, walk, sprint) = (mood_cap(CONTENT), mood_cap(SEEK_FOOD), mood_cap(FLEE));
        assert!(amble < walk && walk < sprint && sprint == 1.0, "{amble} {walk} {sprint}");
    }

    #[test]
    fn content_is_capped_at_an_amble_and_scales_with_a_weak_intent() {
        // A huge intent (an evolved program, or the separation steer in a
        // crowd) still only ambles when content.
        assert_eq!(speed_fraction(1000.0, CONTENT, false), GAIT_AMBLE);
        assert_eq!(speed_fraction(GAIT_FULL_INTENT, CONTENT, false), GAIT_AMBLE);
        // Below the cap the program's own effort sets the pace.
        assert_eq!(speed_fraction(0.1, CONTENT, false), 0.1);
        assert_eq!(speed_fraction(0.0, CONTENT, false), 0.0);
    }

    #[test]
    fn seeking_walks_and_fleeing_or_fighting_sprints_whatever_the_intent() {
        assert_eq!(speed_fraction(1000.0, SEEK_FOOD, false), GAIT_WALK);
        assert_eq!(speed_fraction(0.5, SEEK_WATER, false), 0.5);
        assert_eq!(speed_fraction(1000.0, SEEK_MATE, false), GAIT_WALK);
        // The reflex sets the pace, not the program: a tiny flee intent is
        // still a sprint.
        assert_eq!(speed_fraction(0.01, FLEE, false), GAIT_SPRINT);
        assert_eq!(speed_fraction(0.01, FIGHT, false), GAIT_SPRINT);
        // Hunting overrides any non-reflex mood.
        assert_eq!(speed_fraction(0.01, CONTENT, true), GAIT_SPRINT);
        assert_eq!(speed_fraction(0.01, SEEK_FOOD, true), GAIT_SPRINT);
    }

    #[test]
    fn hunting_needs_meat_a_weapon_prey_in_sight_and_a_closing_heading() {
        let toward = Vec2::new(1.0, 0.0);
        let intent_toward = Vec2::new(0.7, 0.2);
        let intent_away = Vec2::new(-0.7, 0.2);
        assert!(is_hunting(1.0, true, intent_toward, true, toward));
        // An armed ape (omnivore midpoint) hunts too.
        assert!(is_hunting(GAIT_HUNT_CARNIVORY, true, intent_toward, true, toward));
        // A grazer with a weapon fights, it does not hunt.
        assert!(!is_hunting(0.0, true, intent_toward, true, toward));
        // Unarmed carnivores cannot kill, so they are not hunting.
        assert!(!is_hunting(1.0, false, intent_toward, true, toward));
        // No other-species agent perceived ⇒ nothing to hunt.
        assert!(!is_hunting(1.0, true, intent_toward, false, toward));
        // Backing off from the other species is not a chase.
        assert!(!is_hunting(1.0, true, intent_away, true, toward));
        // Standing still is not a chase either (the ambusher waiting).
        assert!(!is_hunting(1.0, true, Vec2::ZERO, true, toward));
    }

    #[test]
    fn move_cost_factor_is_one_at_rest_and_superlinear() {
        assert_eq!(move_cost_factor(0.0), 1.0);
        assert_eq!(move_cost_factor(1.0), 1.0 + GAIT_SPRINT_COST);
        let amble = move_cost_factor(GAIT_AMBLE);
        let walk = move_cost_factor(GAIT_WALK);
        let sprint = move_cost_factor(GAIT_SPRINT);
        assert!(1.0 < amble && amble < walk && walk < sprint);
        // Superlinear per unit distance: doubling the fraction more than
        // doubles the surcharge.
        assert!(walk - 1.0 > 2.0 * (amble - 1.0));
    }

    #[test]
    fn heading_recovers_the_unit_vector_and_keeps_zero() {
        let dir = Vec2::new(0.6, 0.8) * GAIT_AMBLE;
        let h = heading(dir);
        assert!((h.length() - 1.0).abs() < 1e-6, "{h:?}");
        assert!((h.x - 0.6).abs() < 1e-6 && (h.y - 0.8).abs() < 1e-6, "{h:?}");
        assert_eq!(heading(Vec2::ZERO), Vec2::ZERO);
    }
}
