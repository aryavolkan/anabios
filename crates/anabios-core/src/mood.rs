//! Mood — a discrete per-agent winner-take-all state derived from the basic
//! needs columns (energy deficit, thirst, the `asleep` bit) and the affect
//! layer's FEAR / RAGE / LUST activations. Written each tick by
//! `affect::develop_all` into `AgentBuffers::mood`, read by `apply_mood` in
//! `decide_all`, and exported to the Godot front-end (inspector + body-color
//! mode).
//!
//! The mood is the sub-acute behavior ARBITER: it labels what the agent is
//! currently doing (seek food / seek water / sleep / flee / fight / seek
//! mate / mate / content) and sharpens the action register for the dominant
//! drive — suppressing competing appetitive intents and amplifying the
//! matching steer. The acute survival reflex (`affect::apply_hijack`) still
//! runs after it in `decide_all` and wins outright at high arousal, and
//! reproduction itself stays proximity+energy gated in `reproduce_all` —
//! moods steer and label, they do not change who mates.
//!
//! Priority ladder (winner-take-all, highest first): Sleep > Flee > Fight >
//! SeekWater > SeekFood > Mate (in range) > SeekMate > Content. Flee before
//! Fight mirrors the affect layer's FEAR⊣RAGE lateral inhibition. Mate ranks
//! below every survival need; LUST is already 0 below the mating energy
//! gate, so hunger only competes with courtship in low-threshold genomes —
//! and wins, which is the correct ecology.
//!
//! Gating: the column is written only by `affect::develop_all`, a strict
//! no-op when `World::affect_enabled` is off — so a flag-off world keeps
//! every slot at `CONTENT` and `apply_mood` is exact identity on `CONTENT`.
//! `SeekWater`/`Sleep` additionally require non-zero thirst / a set asleep
//! bit, which only `needs::needs_step` (basic-needs flag) produces.
//! Serialized as `Vec<u8>` — persistent state, never `#[serde(skip)]`.

use crate::affect::{self, AffectState, FEAR, LUST, RAGE};
use crate::prelude::Vec2;
use crate::program::ActionRegister;
use crate::sense::SensorRegister;

/// Number of mood discriminants (the `AgentBuffers::mood` column is `Vec<u8>`).
pub const MOOD_COUNT: usize = 8;

pub const CONTENT: u8 = 0;
pub const SEEK_FOOD: u8 = 1;
pub const SEEK_WATER: u8 = 2;
pub const SLEEP: u8 = 3;
pub const FLEE: u8 = 4;
pub const FIGHT: u8 = 5;
pub const SEEK_MATE: u8 = 6;
pub const MATE: u8 = 7;

/// FEAR at/above which the agent counts as fleeing (deliberately below the
/// acute hijack's `HIJACK_AROUSAL_THRESHOLD` = 0.6, so the mood covers the
/// sub-acute regime the reflex leaves alone).
pub const MOOD_FEAR_MIN: f32 = 0.45;
/// RAGE at/above which the agent counts as fighting (same sub-acute band).
pub const MOOD_RAGE_MIN: f32 = 0.45;
/// Homeostatic energy-deficit drive at/above which foraging dominates.
pub const MOOD_HUNGER_MIN: f32 = 0.4;
/// LUST at/above which mate-seeking dominates. LUST is leaky-integrated
/// (λ = 0.8 toward a 1.0 target), so this is reached ~4 ticks after
/// `trigger_lust` first fires — brief gate flicker never flips the mood.
pub const MOOD_LUST_MIN: f32 = 0.5;

/// Extra forage steer gain in SeekFood (on top of affect's `K_SEEK_FORAGE`).
pub const K_MOOD_FORAGE: f32 = 0.4;
/// Extra flee push gain in Flee (on top of affect's `K_FLEE`).
pub const K_MOOD_FLEE: f32 = 0.4;
/// `fire_intent` lift in Fight (on top of affect's `K_RAGE_FIRE`).
pub const K_MOOD_FIGHT_FIRE: f32 = 0.5;
/// Extra approach gain in Fight (on top of affect's `K_RAGE_APPROACH`).
pub const K_MOOD_FIGHT_APPROACH: f32 = 0.3;
/// Extra approach gain in SeekMate (on top of affect's `K_LUST_APPROACH`).
pub const K_MOOD_LUST: f32 = 0.4;
/// Fraction of movement intent retained in Mate: hold position inside
/// `MATING_RANGE` so `reproduce_all`'s proximity gate keeps seeing the pair.
pub const MOOD_MATE_HOLD: f32 = 0.2;

/// Display name for the Godot inspector / legend.
pub fn name(mood: u8) -> &'static str {
    match mood {
        SEEK_FOOD => "seek food",
        SEEK_WATER => "seek water",
        SLEEP => "sleep",
        FLEE => "flee",
        FIGHT => "fight",
        SEEK_MATE => "seek mate",
        MATE => "mate",
        _ => "content",
    }
}

/// The winner-take-all arbiter. Pure function of the agent's own columns
/// (one tick stale on thirst — `needs_step` runs after `develop_all` in the
/// tick, the same staleness every needs read has) plus this tick's freshly
/// updated affect and sensors. ZERO RNG.
pub fn compute_mood(
    asleep: bool,
    thirst: f32,
    energy: f32,
    affect: &AffectState,
    nearest_same_dist: f32,
) -> u8 {
    if asleep {
        return SLEEP;
    }
    let fear = affect[FEAR];
    let rage = affect[RAGE];
    // Flee before fight — mirrors the FEAR⊣RAGE lateral inhibition.
    if fear >= MOOD_FEAR_MIN && fear >= rage {
        return FLEE;
    }
    // A cornered-but-starving agent can't afford to turn and attack (same
    // energy floor the hijack's Fight branch uses).
    if rage >= MOOD_RAGE_MIN && energy >= affect::FIGHT_ENERGY_MIN {
        return FIGHT;
    }
    // Strict `>`: must match decide_all's water-pull gate exactly, so the
    // mood never suppresses feeding without the pull being active.
    if thirst > crate::needs::WATER_SEEK_MIN {
        return SEEK_WATER;
    }
    if affect::homeostatic_drive(energy) >= MOOD_HUNGER_MIN {
        return SEEK_FOOD;
    }
    if affect[LUST] >= MOOD_LUST_MIN {
        // In mating reach ⇒ the act itself (consummated by `reproduce_all`
        // later in the tick); otherwise still courting at a distance.
        if nearest_same_dist <= crate::reproduce::MATING_RANGE {
            return MATE;
        }
        return SEEK_MATE;
    }
    CONTENT
}

/// Read-side arbiter hook, called in `decide_all` after the full movement-
/// bias stack (affect bias + habitat/terrain/hub/anchor/water pulls), so
/// MATE's hold-position damping covers the additive pulls too; only the
/// livestock pen override and the survival hijack run later and win.
/// Sharpens the action register for the dominant drive: amplifies the
/// matching steer, suppresses competing appetitive intents. Exact identity
/// on `CONTENT` (the flag-off state of the whole column). ZERO RNG.
pub fn apply_mood(
    action: &mut ActionRegister,
    mood: u8,
    sensors: &SensorRegister,
    affect: &AffectState,
) {
    match mood {
        SEEK_FOOD => {
            // Forage harder: extra steer toward the sensed plant direction.
            let pd = sensors.plant_direction;
            if pd != Vec2::ZERO {
                action.move_x += K_MOOD_FORAGE * pd.x;
                action.move_y += K_MOOD_FORAGE * pd.y;
            }
            action.feed_intent = action.feed_intent.max(1.0);
        }
        SEEK_WATER => {
            // Single-minded: drinking outranks grazing and courtship. The
            // movement pull itself is the thirst-scaled water-seek bias in
            // `decide_all` (needs.rs); here we just clear competing intents.
            action.feed_intent *= 0.25;
            action.mate_intent *= 0.25;
        }
        FLEE => {
            // Extra push away from the threat; fighting and feeding are
            // suppressed proportionally to the fear.
            let fear = affect[FEAR];
            if sensors.nearest_other_id != crate::sense::NO_NEIGHBOR_ID {
                action.move_x -= K_MOOD_FLEE * fear * sensors.nearest_other_dir.x;
                action.move_y -= K_MOOD_FLEE * fear * sensors.nearest_other_dir.y;
            }
            let damp = (1.0 - fear).max(0.0);
            action.feed_intent *= damp;
            action.mate_intent *= damp;
            action.fire_intent *= damp;
        }
        FIGHT => {
            // Committed: lift the attack and close the distance (stacked on
            // the RAGE bias `apply_affect` already applied).
            let rage = affect[RAGE];
            action.fire_intent += K_MOOD_FIGHT_FIRE * rage;
            if sensors.nearest_other_id != crate::sense::NO_NEIGHBOR_ID {
                action.move_x += K_MOOD_FIGHT_APPROACH * rage * sensors.nearest_other_dir.x;
                action.move_y += K_MOOD_FIGHT_APPROACH * rage * sensors.nearest_other_dir.y;
            }
        }
        SEEK_MATE => {
            // Court: extra approach toward the nearest same-species neighbour,
            // and no attacking while courting.
            let lust = affect[LUST];
            if sensors.nearest_same_id != crate::sense::NO_NEIGHBOR_ID {
                action.move_x += K_MOOD_LUST * lust * sensors.nearest_same_dir.x;
                action.move_y += K_MOOD_LUST * lust * sensors.nearest_same_dir.y;
            }
            action.fire_intent *= 1.0 - lust;
        }
        MATE => {
            // In reach: hold position so the pair stays inside MATING_RANGE
            // for `reproduce_all` (the damp covers the additive pulls too —
            // safe, since MATE outranks nothing thirsty: SEEK_WATER wins the
            // ladder first), and assert the (otherwise latent) mate_intent
            // for observability. The reproduction gate itself is untouched —
            // proximity + energy still decide.
            action.move_x *= MOOD_MATE_HOLD;
            action.move_y *= MOOD_MATE_HOLD;
            action.fire_intent = 0.0;
            action.mate_intent = 1.0;
        }
        // SLEEP (integrate/feed already gate on the asleep bit) and CONTENT
        // (flag-off state) are exact identity.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::SPAWN_ENERGY;

    fn neutral() -> AffectState {
        [0.0; crate::affect::AFFECT_SYSTEMS]
    }

    #[test]
    fn content_when_nothing_dominates() {
        let a = neutral();
        assert_eq!(compute_mood(false, 0.0, SPAWN_ENERGY, &a, f32::INFINITY), CONTENT);
    }

    #[test]
    fn sleep_beats_everything() {
        let mut a = neutral();
        a[FEAR] = 1.0; // terrified, but unconscious
        assert_eq!(compute_mood(true, 1.0, 1.0, &a, 0.0), SLEEP);
    }

    #[test]
    fn fear_and_rage_map_to_flee_and_fight() {
        let mut a = neutral();
        a[FEAR] = 0.6;
        assert_eq!(compute_mood(false, 0.0, SPAWN_ENERGY, &a, f32::INFINITY), FLEE);
        // Flee before fight on a tie.
        a[RAGE] = 0.6;
        assert_eq!(compute_mood(false, 0.0, SPAWN_ENERGY, &a, f32::INFINITY), FLEE);
        // Rage alone, well-fed ⇒ fight.
        a[FEAR] = 0.0;
        assert_eq!(compute_mood(false, 0.0, SPAWN_ENERGY, &a, f32::INFINITY), FIGHT);
        // ...but a starving agent can't afford to turn and attack.
        assert_eq!(
            compute_mood(false, 0.0, affect::FIGHT_ENERGY_MIN - 1.0, &a, f32::INFINITY),
            SEEK_FOOD
        );
    }

    #[test]
    fn thirst_beats_hunger_and_hunger_beats_lust() {
        let mut a = neutral();
        a[LUST] = 1.0;
        // Hungry + lustful ⇒ food first.
        assert_eq!(compute_mood(false, 0.0, SPAWN_ENERGY * 0.3, &a, 50.0), SEEK_FOOD);
        // Thirsty + hungry ⇒ water first.
        assert_eq!(compute_mood(false, 0.8, SPAWN_ENERGY * 0.3, &a, 50.0), SEEK_WATER);
        // Sated + hydrated + lustful ⇒ mate-seeking.
        assert_eq!(compute_mood(false, 0.0, SPAWN_ENERGY, &a, 50.0), SEEK_MATE);
        // ...and in reach ⇒ mating.
        assert_eq!(
            compute_mood(false, 0.0, SPAWN_ENERGY, &a, crate::reproduce::MATING_RANGE),
            MATE
        );
    }

    #[test]
    fn apply_mood_is_identity_on_content() {
        let mut action = ActionRegister {
            move_x: 0.3,
            move_y: -0.7,
            fire_intent: 0.9,
            feed_intent: 0.4,
            ..Default::default()
        };
        let before = action;
        let a = neutral();
        let s = SensorRegister::default();
        apply_mood(&mut action, CONTENT, &s, &a);
        assert_eq!(action.move_x, before.move_x);
        assert_eq!(action.move_y, before.move_y);
        assert_eq!(action.fire_intent, before.fire_intent);
        assert_eq!(action.feed_intent, before.feed_intent);
    }

    #[test]
    fn mate_holds_position_and_asserts_intent() {
        let mut action =
            ActionRegister { move_x: 3.0, move_y: 4.0, fire_intent: 0.8, ..Default::default() };
        let mut a = neutral();
        a[LUST] = 1.0;
        let s = SensorRegister::default();
        apply_mood(&mut action, MATE, &s, &a);
        assert_eq!(action.move_x, 3.0 * MOOD_MATE_HOLD);
        assert_eq!(action.move_y, 4.0 * MOOD_MATE_HOLD);
        assert_eq!(action.fire_intent, 0.0);
        assert_eq!(action.mate_intent, 1.0);
    }

    #[test]
    fn seek_water_suppresses_appetitive_intents() {
        let mut action =
            ActionRegister { feed_intent: 1.0, mate_intent: 1.0, ..Default::default() };
        let a = neutral();
        let s = SensorRegister::default();
        apply_mood(&mut action, SEEK_WATER, &s, &a);
        assert_eq!(action.feed_intent, 0.25);
        assert_eq!(action.mate_intent, 0.25);
    }

    #[test]
    fn flee_damps_fire_and_pushes_away() {
        let mut action = ActionRegister { fire_intent: 1.0, ..Default::default() };
        let mut a = neutral();
        a[FEAR] = 0.8;
        let s = SensorRegister {
            nearest_other_id: 7,
            nearest_other_dir: Vec2::new(1.0, 0.0),
            ..Default::default()
        };
        apply_mood(&mut action, FLEE, &s, &a);
        assert!(action.move_x < 0.0, "flee pushes away from the threat: {}", action.move_x);
        assert!((action.fire_intent - 0.2).abs() < 1e-6, "fear damps fire: {}", action.fire_intent);
    }

    #[test]
    fn names_cover_every_mood() {
        for m in 0..MOOD_COUNT as u8 {
            assert!(!name(m).is_empty());
        }
        assert_eq!(name(CONTENT), "content");
        assert_eq!(name(u8::MAX), "content", "unknown discriminants read as content");
    }

    #[test]
    fn develop_all_writes_mood_and_flag_off_stays_content() {
        use crate::genome::Genome;
        use crate::world::World;

        // Flag off: develop_all no-ops, the column stays CONTENT forever.
        let mut w = World::new(3);
        let a = w.spawn_agent(Vec2::new(100.0, 100.0), Genome::neutral());
        crate::tick::step(&mut w);
        assert_eq!(w.agents.mood[a as usize], CONTENT);

        // Flag on: a sleeping agent's mood is SLEEP from the next develop_all.
        let mut w = World::new(3);
        w.affect_enabled = true;
        w.basic_needs_enabled = true;
        let a = w.spawn_agent(Vec2::new(100.0, 100.0), Genome::neutral());
        w.agents.asleep.set(a as usize, true);
        w.agents.fatigue[a as usize] = 0.5; // above WAKE_AT ⇒ stays asleep
        crate::tick::step(&mut w);
        assert!(w.agents.asleep[a as usize], "still asleep (fatigue 0.49)");
        assert_eq!(w.agents.mood[a as usize], SLEEP);
    }
}
