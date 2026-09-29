//! Predation as a chase (opt-in, `World::chase_enabled`): a hunt is a
//! pursuit that costs stamina, and a contact strike lands with a
//! probability set by the predator's speed advantage and the size ratio.
//!
//! **Stamina.** `World::stamina` (one `[0, 1]` bar per agent slot, full for
//! a fresh slot; kept on `World` like `still_ticks`, so the `agents` layout
//! and the flag-off trajectory pins that hash it are untouched) drains
//! while an agent's *applied* move (`velocity[i]`, after the habitat gate
//! and the swept-contact cut) is faster than a walk — `WALK_FRACTION` of
//! its own top speed (`integrate::top_speed` under the growth speed scale),
//! gait's `GAIT_WALK` — and recovers while it walks or rests. When it
//! reaches `STAMINA_EXHAUST_AT` the agent is *exhausted* (`World::exhausted`):
//! `integrate_all` holds it to the walk until stamina has climbed back to
//! `STAMINA_RECOVER_AT` (a hysteresis band, the sleep bit's shape). The cap
//! is a multiplier on the speed (`stamina_speed_multiplier`), so it
//! composes with the gait fraction `decide_all` folds into the heading's
//! length (`gait.rs`) and with the growth speed scale: a fleeing prey
//! sprints at its full top speed while its stamina lasts, then at the walk.
//!
//! **Catch roll.** In `interact::combat_pass` a contact-range strike
//! (`Weapon`/`Jaws`; a `Spines` volley flies regardless) lands with
//! `catch_probability`: a speed term — the predator's attainable speed this
//! tick (its top speed under its own exhaustion cap) over the prey's
//! *escape* speed (the part of its applied move directed away from the
//! predator) — times a size term (predator Size over prey Size). Prey that
//! are faster and larger are rarely caught; an exhausted, cornered or
//! oblivious prey is caught easily. A miss still costs the attacker its
//! weapon energy (the lunge). The roll (`catch_roll`) is a stateless hash of
//! serialized state — the world seed, the tick and the pair of ids: the
//! combat pass never drew from `world.rng` (a strike was certain), and the
//! chase keeps that draw count at zero, so a flag-on world diverges from
//! the flag-off one only through the chase itself, never through a shifted
//! RNG stream — the `personality_rng` substream's rationale.
//!
//! **Wound bank.** A strike takes energy from the prey (its HP *is* its
//! energy); with the flag on that energy is not destroyed but banked on the
//! prey (`World::wound_bank`, less any spoils the attacker already
//! recovered) and returned as carcass flesh when the prey dies
//! (`age::age_and_starve`, at `carcass::FLESH_ENERGY_PER_UNIT`), so a fat
//! prey is a big meal and a kill conserves energy. Without it a grazer that
//! ambled itself to 400 energy took 25 strikes to bring down and yielded
//! the same 32-energy carcass as a lean one — the predator paid more for
//! the kill than the carcass returned, and no pursuer lineage fed itself.
//!
//! **Eating the kill.** With the flag on a hungry carnivore (carnivory at
//! or above `carcass::CARCASS_SEEK_CARNIVORY`, energy below
//! `carcass::CARCASS_SEEK_SATIETY`) walks to the nearest carcass with flesh
//! within `carcass::CARCASS_SEEK_REACH` and stands to eat once within
//! `carcass::SCAVENGE_RANGE` (`tick::decide_all`; the pull replaces the
//! program's movement, the mood still sets the pace and only the survival
//! hijack overrides it). Without it a predator left its kill the tick it
//! made it — on `predator-prey` the pursuers sat at a carcass on 5–10% of
//! their ticks and let 86% of all flesh rot, with every realism knob on or
//! off alike — and no pursuer lineage fed itself; with it (2026-09-29,
//! 1500 ticks, seeds 0–1) they hold 50–94 energy through the juvenile
//! window, breed from maturity on (30 founders → 60 by tick 1500 on seed
//! 0) and waste a quarter of the flesh instead.
//!
//! Flag off ⇒ `stamina_step` early-returns (the vectors stay 1.0 / false /
//! 0 and are never read), `integrate_all` skips the cap, `combat_pass`
//! lands every strike as before and banks nothing, and a carcass carries
//! the Size term alone: zero RNG, byte-identical (the flag-off trajectory
//! pins in `tests/determinism.rs` are unchanged, and
//! `tests::flag_off_columns_never_influence_behaviour` proves the vectors
//! are unread).

use crate::prelude::Vec2;
use crate::world::World;

/// The walk, as a fraction of an agent's top speed (`integrate::top_speed`):
/// moving at or below it costs no stamina, and an exhausted agent is held
/// to it. It is gait's walk (`gait::GAIT_WALK`), so the two features agree
/// on what a walk is: with `gait_enabled` the seeking moods travel at it
/// without tiring, an amble is well below it, and only a sprint — FLEE,
/// FIGHT, a hunter closing on prey — drains stamina. With gait off every
/// move is a flat-out sprint (the decide stage emits a unit heading), so a
/// wanderer cycles between sprint and walk; jostling in a crowd (moves cut
/// short by the swept contact) reads as a walk either way.
pub const WALK_FRACTION: f32 = crate::gait::GAIT_WALK;
/// Slack above `WALK_FRACTION` (as a fraction of top speed) still counted as
/// walking, so a move at exactly the walk never drains through rounding.
pub const WALK_TOLERANCE: f32 = 0.01;
/// Stamina drained per tick at a flat-out sprint (speed fraction 1.0): a
/// full bar lasts 80 ticks. The drain scales linearly with the speed above
/// the walk, so a jog costs less.
pub const STAMINA_DRAIN: f32 = 0.0125;
/// Stamina recovered per tick while walking (moving, at or below the walk).
pub const STAMINA_RECOVER_WALK: f32 = 0.01;
/// Stamina recovered per tick while resting (standing still or asleep) —
/// twice the walking rate, so an ambusher lying in wait refills fastest.
pub const STAMINA_RECOVER_REST: f32 = 0.02;
/// Applied speed (world units per tick) at or below which an agent rests.
pub const REST_SPEED: f32 = 1e-3;
/// Stamina at or below which the agent becomes exhausted (the cap engages).
pub const STAMINA_EXHAUST_AT: f32 = 0.0;
/// Stamina at or above which an exhausted agent recovers (the cap lifts).
/// The band `(STAMINA_EXHAUST_AT, STAMINA_RECOVER_AT)` is the hysteresis: a
/// runner at the edge does not flicker between sprint and walk each tick.
/// From empty, a walking agent recovers in 50 ticks, a resting one in 25.
pub const STAMINA_RECOVER_AT: f32 = 0.5;

/// Speed advantage (predator's attainable speed over the prey's escape
/// speed) at or below which a contact strike never lands: a prey running
/// away at least 1/0.6 ≈ 1.7× faster cannot be caught.
pub const SPEED_ADV_MIN: f32 = 0.6;
/// Speed advantage at or above which the speed term saturates at 1 — a
/// predator 1.6× faster than its prey's escape closes every gap.
pub const SPEED_ADV_MAX: f32 = 1.6;
/// Size ratio (predator Size over prey Size) at or below which a strike
/// never lands: nothing catches a prey four times its size.
pub const SIZE_RATIO_MIN: f32 = 0.25;
/// Size ratio at or above which the size term saturates at 1.
pub const SIZE_RATIO_MAX: f32 = 1.5;
/// Escape speed (world units per tick) below which the prey counts as
/// standing still, so the speed term saturates instead of dividing by zero.
pub const ESCAPE_SPEED_MIN: f32 = 1e-3;

/// Multiplier on an agent's applied speed in `integrate_all`: the walk for
/// an exhausted agent, exactly 1.0 otherwise. Composes with any other speed
/// fraction (a gait) by multiplication.
#[inline]
pub fn stamina_speed_multiplier(exhausted: bool) -> f32 {
    if exhausted {
        WALK_FRACTION
    } else {
        1.0
    }
}

/// Linear ramp: 0 at or below `lo`, 1 at or above `hi`.
#[inline]
fn ramp(x: f32, lo: f32, hi: f32) -> f32 {
    ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// The prey's escape speed: the component of its applied move directed
/// along `away` (the unit vector from the predator to the prey), floored at
/// zero. A prey that turns to fight, stands still or blunders toward the
/// predator escapes at 0 — caught as if standing.
#[inline]
pub fn escape_speed(prey_velocity: Vec2, away: Vec2) -> f32 {
    prey_velocity.dot(away).max(0.0)
}

/// Probability that a contact strike lands this tick: the product of a speed
/// term (`pred_speed / prey_escape`, ramped over
/// `[SPEED_ADV_MIN, SPEED_ADV_MAX]`) and a size term (`pred_size /
/// prey_size`, ramped over `[SIZE_RATIO_MIN, SIZE_RATIO_MAX]`). Equal speed
/// and size give 0.4 × 0.6 = 0.24 per tick in range. Pure; no RNG.
pub fn catch_probability(pred_speed: f32, prey_escape: f32, pred_size: f32, prey_size: f32) -> f32 {
    let speed_adv =
        if prey_escape <= ESCAPE_SPEED_MIN { SPEED_ADV_MAX } else { pred_speed / prey_escape };
    let speed_term = ramp(speed_adv, SPEED_ADV_MIN, SPEED_ADV_MAX);
    let size_term = ramp(pred_size / prey_size.max(0.1), SIZE_RATIO_MIN, SIZE_RATIO_MAX);
    speed_term * size_term
}

/// A uniform `[0, 1)` value for one encounter, as a stateless hash of the
/// world seed, the tick and the (attacker, target) pair — splitmix64's
/// finalizer over the three, at the same 24-bit float resolution as
/// `Rng::f32_unit`. Nothing is drawn from `world.rng` (see the module doc);
/// every input is serialized, so a restored world rolls what the continuous
/// run rolled, and the value is independent of evaluation order and thread
/// count.
pub fn catch_roll(seed: u64, tick: u64, attacker: u32, target: u32) -> f32 {
    #[inline]
    fn mix(mut z: u64) -> u64 {
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    let pair = ((attacker as u64) << 32) | target as u64;
    let z = mix(mix(mix(seed.wrapping_add(0x9E37_79B9_7F4A_7C15)) ^ tick) ^ pair);
    (z >> 40) as f32 / 16_777_216.0
}

/// Tick stage (after integrate and the collision stages, so `velocity` is
/// this tick's applied move): drain or recover each alive agent's stamina
/// and run the exhaustion hysteresis. Serial ascending-id loop over each
/// agent's own columns — deterministic, RNG-free. Strict no-op when the
/// flag is off.
pub fn stamina_step(world: &mut World) {
    if !world.chase_enabled {
        return;
    }
    // Sized here as well as in `resize_scratch`, so a direct call on a
    // freshly built world (tests) never indexes past the vectors.
    let cap = world.agents.capacity();
    if world.stamina.len() < cap {
        world.stamina.resize(cap, 1.0);
    }
    if world.exhausted.len() < cap {
        world.exhausted.resize(cap, false);
    }
    if world.wound_bank.len() < cap {
        world.wound_bank.resize(cap, 0.0);
    }
    let coupling = world.gene_tech_coupling;
    let growth_enabled = world.growth_enabled;
    let agents = &world.agents;
    let stamina = &mut world.stamina;
    let exhausted = &mut world.exhausted;
    let wound_bank = &mut world.wound_bank;
    for i in 0..cap {
        if !agents.is_alive(i as u32) {
            // A dead slot reads as fresh, so the newborn that next reuses it
            // (reproduce, stage 6, after this stage) starts with a full bar
            // and no wounds.
            stamina[i] = 1.0;
            exhausted[i] = false;
            wound_bank[i] = 0.0;
            continue;
        }
        let speed = agents.velocity[i].length();
        // The agent's own top speed this tick, growth speed scale included
        // (exactly ×1.0 with growth off or once mature).
        let top = crate::integrate::top_speed(
            &agents.modules[i],
            &agents.genome[i],
            crate::invention::held_mask(&agents.meme_vector[i]),
            &agents.affect[i],
            coupling,
        ) * crate::growth::speed_scale(crate::growth::body_scale_of(
            growth_enabled,
            agents.age[i],
            &agents.genome[i],
        ));
        let frac = if top > 0.0 { speed / top } else { 0.0 };
        let s = &mut stamina[i];
        // An exhausted agent is held to the walk by `integrate_all`, so it
        // always recovers here — the branch never depends on rounding at
        // the cap.
        if !exhausted[i] && frac > WALK_FRACTION + WALK_TOLERANCE {
            let excess = ((frac - WALK_FRACTION) / (1.0 - WALK_FRACTION)).min(1.0);
            *s = (*s - STAMINA_DRAIN * excess).max(0.0);
            if *s <= STAMINA_EXHAUST_AT {
                exhausted[i] = true;
            }
        } else {
            let rate =
                if speed <= REST_SPEED { STAMINA_RECOVER_REST } else { STAMINA_RECOVER_WALK };
            *s = (*s + rate).min(1.0);
            if exhausted[i] && *s >= STAMINA_RECOVER_AT {
                exhausted[i] = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::module::Module;
    use crate::program::{Node, Program};
    use crate::snapshot::state_hash;
    use crate::tick::step;

    /// Pin the agent's Locomotor at `max_speed` (top speed `SPEED_MAX_CAP ×
    /// max_speed` at a neutral genome).
    fn set_locomotor(w: &mut World, id: u32, max_speed: f32) {
        for m in w.agents.modules[id as usize].iter_mut() {
            if let Module::Locomotor { max_speed: s, .. } = m {
                *s = max_speed;
            }
        }
    }

    #[test]
    fn catch_probability_favours_speed_and_size() {
        // Faster and bigger than the prey's escape: certain.
        assert_eq!(catch_probability(4.0, 2.0, 1.0, 0.5), 1.0);
        // Prey running away 1.7× faster: never.
        assert_eq!(catch_probability(2.0, 4.0, 0.5, 0.5), 0.0);
        // Prey four times the size: never, however slow.
        assert_eq!(catch_probability(4.0, 0.0, 0.2, 0.8), 0.0);
        // Standing prey: the speed term saturates, only size remains.
        let standing = catch_probability(4.0, 0.0, 0.5, 0.5);
        assert!((standing - 0.6).abs() < 1e-6, "size term alone: {standing}");
        // Equal speed and size: the documented 0.24 baseline.
        let even = catch_probability(2.0, 2.0, 0.5, 0.5);
        assert!((even - 0.24).abs() < 1e-6, "even chase: {even}");
        // Monotonic in the predator's speed and size, bounded in [0, 1].
        let slower = catch_probability(2.5, 2.0, 0.5, 0.5);
        let faster = catch_probability(3.0, 2.0, 0.5, 0.5);
        assert!(even < slower && slower < faster && faster <= 1.0);
        let bigger = catch_probability(2.0, 2.0, 0.7, 0.5);
        assert!(bigger > even);
    }

    #[test]
    fn escape_speed_counts_only_motion_away() {
        let away = Vec2::new(1.0, 0.0);
        assert_eq!(escape_speed(Vec2::new(3.0, 0.0), away), 3.0, "fleeing");
        assert_eq!(escape_speed(Vec2::new(-3.0, 0.0), away), 0.0, "charging");
        assert_eq!(escape_speed(Vec2::new(0.0, 3.0), away), 0.0, "sidestepping");
        assert_eq!(escape_speed(Vec2::new(3.0, 0.0), Vec2::ZERO), 0.0, "coincident");
    }

    #[test]
    fn catch_roll_is_stateless_uniform_and_input_sensitive() {
        assert_eq!(catch_roll(7, 100, 3, 9), catch_roll(7, 100, 3, 9), "pure");
        assert_ne!(catch_roll(7, 100, 3, 9), catch_roll(7, 101, 3, 9), "tick matters");
        assert_ne!(catch_roll(7, 100, 3, 9), catch_roll(7, 100, 9, 3), "order matters");
        assert_ne!(catch_roll(7, 100, 3, 9), catch_roll(8, 100, 3, 9), "seed matters");
        let n = 4096;
        let mut sum = 0.0f64;
        let mut low = 0usize;
        for tick in 0..n {
            let r = catch_roll(12345, tick, 1, 2);
            assert!((0.0..1.0).contains(&r), "in [0, 1): {r}");
            sum += r as f64;
            low += (r < 0.25) as usize;
        }
        let mean = sum / n as f64;
        assert!((mean - 0.5).abs() < 0.03, "mean over ticks ≈ 0.5: {mean}");
        let quarter = low as f64 / n as f64;
        assert!((quarter - 0.25).abs() < 0.03, "a quarter below 0.25: {quarter}");
    }

    #[test]
    fn stamina_step_is_a_noop_with_the_flag_off() {
        let mut w = World::new(3);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.resize_scratch();
        w.agents.velocity[id as usize] = Vec2::new(4.0, 0.0);
        let before = state_hash(&w);
        stamina_step(&mut w);
        assert_eq!(state_hash(&w), before, "flag off: zero state change");
        assert_eq!(w.stamina[id as usize], 1.0);
        assert!(!w.exhausted[id as usize]);
    }

    #[test]
    fn stamina_drains_above_a_walk_and_recovers_below_it() {
        let mut w = World::new(3);
        w.chase_enabled = true;
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        set_locomotor(&mut w, id, 1.0); // top speed 4.0
        w.resize_scratch();
        let i = id as usize;
        // Flat-out sprint: the full drain.
        w.agents.velocity[i] = Vec2::new(4.0, 0.0);
        stamina_step(&mut w);
        assert!((w.stamina[i] - (1.0 - STAMINA_DRAIN)).abs() < 1e-6);
        // A jog at 80% of top speed drains half as much (0.2 of the 0.4 band).
        w.agents.velocity[i] = Vec2::new(3.2, 0.0);
        let before = w.stamina[i];
        stamina_step(&mut w);
        assert!((before - w.stamina[i] - STAMINA_DRAIN * 0.5).abs() < 1e-5);
        // At the walk: recovery at the walking rate (from half a bar, so the
        // 1.0 cap does not truncate the step), capped at 1.0 in the end.
        w.stamina[i] = 0.5;
        w.agents.velocity[i] = Vec2::new(4.0 * WALK_FRACTION, 0.0);
        let before = w.stamina[i];
        stamina_step(&mut w);
        assert!((w.stamina[i] - before - STAMINA_RECOVER_WALK).abs() < 1e-6);
        // Standing still: the resting rate.
        w.agents.velocity[i] = Vec2::ZERO;
        let before = w.stamina[i];
        stamina_step(&mut w);
        assert!((w.stamina[i] - before - STAMINA_RECOVER_REST).abs() < 1e-6);
        for _ in 0..200 {
            stamina_step(&mut w);
        }
        assert_eq!(w.stamina[i], 1.0, "capped at full");
        assert!(!w.exhausted[i]);
    }

    #[test]
    fn exhaustion_hysteresis_engages_at_empty_and_lifts_at_recover_at() {
        let mut w = World::new(3);
        w.chase_enabled = true;
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        set_locomotor(&mut w, id, 1.0);
        w.resize_scratch();
        let i = id as usize;
        w.stamina[i] = STAMINA_DRAIN * 0.5;
        w.agents.velocity[i] = Vec2::new(4.0, 0.0);
        stamina_step(&mut w);
        assert_eq!(w.stamina[i], 0.0);
        assert!(w.exhausted[i], "empty ⇒ exhausted");
        // Exhausted: recovers even while moving (the cap holds it to a walk),
        // and stays exhausted below STAMINA_RECOVER_AT.
        w.agents.velocity[i] = Vec2::new(4.0 * WALK_FRACTION, 0.0);
        stamina_step(&mut w);
        assert!(w.stamina[i] > 0.0 && w.exhausted[i]);
        w.stamina[i] = STAMINA_RECOVER_AT - STAMINA_RECOVER_WALK * 0.5;
        stamina_step(&mut w);
        assert!(!w.exhausted[i], "recovered past STAMINA_RECOVER_AT ⇒ cap lifts");
    }

    #[test]
    fn sprinter_exhausts_is_held_to_a_walk_then_recovers() {
        let mut w = World::new(5);
        w.chase_enabled = true;
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        set_locomotor(&mut w, id, 1.0);
        let i = id as usize;
        // Always run +x flat out.
        w.agents.program[i] = Program::from_slice(&[Node::Const(1.0), Node::MoveTowardX]);
        let step_len = |w: &mut World| -> f32 {
            step(w);
            w.agents.velocity[i].length()
        };
        // Fresh: full-speed steps and a draining bar.
        for _ in 0..40 {
            assert!((step_len(&mut w) - 4.0).abs() < 1e-3, "fresh sprinter moves at top speed");
        }
        assert!(!w.exhausted[i]);
        assert!((w.stamina[i] - (1.0 - 40.0 * STAMINA_DRAIN)).abs() < 1e-3);
        // Empties after 80 ticks in all; the next step is the walk.
        let mut exhausted_at = None;
        for t in 40..120 {
            step(&mut w);
            if w.exhausted[i] {
                exhausted_at = Some(t);
                break;
            }
        }
        let t0 = exhausted_at.expect("a flat-out sprinter exhausts");
        assert!((78..=82).contains(&t0), "exhausted at tick {t0}");
        for _ in 0..10 {
            let l = step_len(&mut w);
            assert!((l - 4.0 * WALK_FRACTION).abs() < 1e-3, "exhausted ⇒ held to a walk, got {l}");
        }
        // Walking refills the bar to STAMINA_RECOVER_AT in ~50 ticks, then the
        // sprint resumes.
        let mut recovered_at = None;
        for t in 0..100 {
            step(&mut w);
            if !w.exhausted[i] {
                recovered_at = Some(t);
                break;
            }
        }
        let t1 = recovered_at.expect("a walking agent recovers");
        assert!((35..=45).contains(&t1), "recovered after {t1} more ticks");
        assert!(w.stamina[i] >= STAMINA_RECOVER_AT);
        assert!((step_len(&mut w) - 4.0).abs() < 1e-3, "recovered ⇒ sprints again");
    }

    /// A lethal predator (one catch kills) at `pred_speed` beside a fleeing
    /// prey at `prey_speed`, both size 0.5, on a bare flag-on world.
    fn chase_fixture(seed: u64, pred_speed: f32, prey_speed: f32) -> (World, u32, u32) {
        let mut w = World::new(seed);
        w.chase_enabled = true;
        let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let prey = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
        crate::prelude_test::reassign_to_new_species(&mut w, prey);
        w.agents.modules[pred as usize] = crate::module::predator_kit();
        for m in w.agents.modules[pred as usize].iter_mut() {
            if let Module::Weapon { damage, .. } = m {
                *damage = 1000.0;
            }
        }
        set_locomotor(&mut w, pred, pred_speed);
        set_locomotor(&mut w, prey, prey_speed);
        w.agents.program[pred as usize] = crate::program::starter_stalker();
        w.agents.program[prey as usize] = crate::program::starter_sentinel();
        (w, pred, prey)
    }

    #[test]
    fn faster_predator_catches_fleeing_prey_within_the_window() {
        // Predator at 4.0 units/tick, prey fleeing at 2.0: speed term 1.0,
        // size term 0.6 ⇒ 0.6 per tick in range.
        let (mut w, _pred, prey) = chase_fixture(11, 1.0, 0.5);
        let mut caught_at = None;
        for t in 0..60 {
            step(&mut w);
            if !w.agents.is_alive(prey) {
                caught_at = Some(t);
                break;
            }
        }
        assert!(caught_at.is_some(), "a faster predator runs its prey down within 60 ticks");
        assert_eq!(w.carcasses.len(), 1, "the kill leaves a carcass");
    }

    #[test]
    fn much_faster_prey_outruns_the_predator() {
        // Prey fleeing at 4.0, predator at 2.0: speed advantage 0.5 ≤
        // SPEED_ADV_MIN ⇒ the one strike in range cannot land, and the gap
        // only grows from there.
        let (mut w, pred, prey) = chase_fixture(11, 0.5, 1.0);
        let pred_e0 = w.agents.energy[pred as usize];
        for _ in 0..60 {
            step(&mut w);
        }
        assert!(w.agents.is_alive(prey), "a much faster prey survives the same window");
        assert!(w.carcasses.is_empty());
        assert!(w.agents.energy[pred as usize] < pred_e0, "the lunge still cost the predator");
    }

    #[test]
    fn flag_off_columns_never_influence_behaviour() {
        // Two flag-off worlds, identical but for poisoned stamina/exhausted
        // columns in one: after the same ticks, everything else must match
        // bit for bit, so no flag-off path reads the columns.
        let build = |poison: bool| -> World {
            let mut w = World::new(9);
            let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
            let prey = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
            crate::prelude_test::reassign_to_new_species(&mut w, prey);
            w.agents.modules[pred as usize] = crate::module::predator_kit();
            w.agents.program[pred as usize] = crate::program::starter_stalker();
            w.agents.program[prey as usize] = crate::program::starter_sentinel();
            for k in 0..6 {
                w.spawn_agent(Vec2::new(300.0 + 5.0 * k as f32, 300.0), Genome::neutral());
            }
            w.resize_scratch();
            if poison {
                for i in 0..w.agents.capacity() {
                    w.stamina[i] = 0.0;
                    w.exhausted[i] = true;
                    w.wound_bank[i] = 123.0;
                }
            }
            w
        };
        let mut clean = build(false);
        let mut poisoned = build(true);
        for _ in 0..40 {
            step(&mut clean);
            step(&mut poisoned);
        }
        assert!(clean.stamina.iter().all(|&s| s == 1.0), "flag off: stamina untouched");
        assert!(clean.exhausted.iter().all(|&e| !e), "flag off: nobody exhausted");
        assert!(clean.wound_bank.iter().all(|&b| b == 0.0), "flag off: no wounds banked");
        // The poison survives on every founder still alive (only `kill`
        // resets a dead slot's columns and `spawn` fills a newborn's — the
        // dead-slot / birth convention every column follows; a newborn may
        // even reuse a dead founder's slot, so founders are told apart by
        // their lineage ids, 1..=8).
        let founders = 8u64;
        for id in poisoned.agents.iter_alive() {
            if poisoned.agents.lineage_id[id as usize] > founders {
                continue;
            }
            assert_eq!(poisoned.stamina[id as usize], 0.0, "flag off: poison untouched");
            assert!(poisoned.exhausted[id as usize], "flag off: poison untouched");
            assert_eq!(poisoned.wound_bank[id as usize], 123.0, "flag off: poison untouched");
        }
        for i in 0..poisoned.agents.capacity() {
            poisoned.stamina[i] = 1.0;
            poisoned.exhausted[i] = false;
            poisoned.wound_bank[i] = 0.0;
        }
        assert_eq!(
            state_hash(&clean),
            state_hash(&poisoned),
            "flag off: the columns must not influence any other state"
        );
    }
}
