//! Big Five (OCEAN) personality: hard-coded modulation of an agent's action
//! intents from its signed `[-1,+1]` personality traits. At neutral traits
//! (value 0.0) every function here is an exact identity, so wiring it into the
//! pipeline is determinism-neutral until genomes are given non-neutral traits.

use crate::agent::SPAWN_ENERGY;
use crate::genome::Genome;
use crate::program::{ActionRegister, NO_TARGET};
use crate::sense::{SensorRegister, NO_NEIGHBOR_ID};

/// Openness → movement-speed gain (applied in `integrate`).
pub const K_O: f32 = 0.5;
/// Conscientiousness → reproduction-threshold gain (applied in `reproduce`).
pub const K_C: f32 = 0.5;
/// Extraversion → same-species approach bias + broadcast gain.
pub const K_E: f32 = 0.6;
/// Neuroticism → flee bias from other-species neighbors.
pub const K_N: f32 = 0.8;
/// Agreeableness → same-species attack suppression gain.
pub const K_A: f32 = 1.0;
/// Conscientiousness → feed-intent boost when below comfort energy.
pub const K_C_FEED: f32 = 0.5;
/// Comfort energy fraction (of `SPAWN_ENERGY`) below which C boosts feeding.
pub const COMFORT_FRAC: f32 = 0.5;
/// Neuroticism → feed/mate dampening under threat.
pub const N_DAMPEN: f32 = 0.5;

/// Movement-speed multiplier from Openness. `1.0` at neutral.
pub fn personality_speed_factor(genome: &Genome) -> f32 {
    (1.0 + K_O * genome.openness()).max(0.0)
}

/// Reproduction energy-threshold multiplier from Conscientiousness. `1.0` at neutral.
pub fn personality_reproduction_factor(genome: &Genome) -> f32 {
    (1.0 + K_C * genome.conscientiousness()).max(0.0)
}

/// Modulate an action from personality + current percepts (E, N, A, C-feed).
/// Openness (speed) and Conscientiousness (repro threshold) are applied at
/// their own sites via the factor helpers above.
pub fn apply_personality(
    action: &mut ActionRegister,
    genome: &Genome,
    sensors: &SensorRegister,
    energy: f32,
) {
    let c = genome.conscientiousness();
    let e = genome.extraversion();
    let a = genome.agreeableness();
    let n = genome.neuroticism();

    // Extraversion: bias movement toward the nearest same-species neighbor and
    // scale broadcasts. (Introverts, e < 0, bias away.)
    if sensors.nearest_same_id != NO_NEIGHBOR_ID {
        action.move_x += K_E * e * sensors.nearest_same_dir.x;
        action.move_y += K_E * e * sensors.nearest_same_dir.y;
    }
    let bcast = (1.0 + K_E * e.max(0.0)).max(0.0);
    for ch in action.broadcast_intent.iter_mut() {
        *ch *= bcast;
    }

    // Neuroticism: flee nearby other-species neighbors; dampen feed/mate under threat.
    if sensors.nearest_other_id != NO_NEIGHBOR_ID {
        let flee = K_N * n.max(0.0);
        action.move_x -= flee * sensors.nearest_other_dir.x;
        action.move_y -= flee * sensors.nearest_other_dir.y;
        let damp = (1.0 - N_DAMPEN * n.max(0.0)).max(0.0);
        action.feed_intent *= damp;
        action.mate_intent *= damp;
    }

    // Agreeableness: scale sharing (+A shares more, −A none); suppress attacks on
    // kin (+A peaceful → ×0; −A antagonistic → up to ×2).
    action.share_intent *= (1.0 + a).clamp(0.0, 2.0);
    if sensors.nearest_same_id != NO_NEIGHBOR_ID
        && action.target_id != NO_TARGET
        && action.target_id == sensors.nearest_same_id
    {
        action.fire_intent *= (1.0 - K_A * a).clamp(0.0, 2.0);
    }

    // Conscientiousness: boost feeding when below comfort energy (provisioning).
    if energy < COMFORT_FRAC * SPAWN_ENERGY {
        action.feed_intent *= 1.0 + K_C_FEED * c.max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::GenomeSlot;
    use crate::prelude::Vec2;

    fn neutral() -> Genome {
        Genome::neutral()
    }

    fn same_neighbor(dir: Vec2, id: u32) -> SensorRegister {
        SensorRegister { nearest_same_id: id, nearest_same_dir: dir, ..Default::default() }
    }

    fn other_neighbor(dir: Vec2, id: u32) -> SensorRegister {
        SensorRegister { nearest_other_id: id, nearest_other_dir: dir, ..Default::default() }
    }

    #[test]
    fn identity_at_neutral_traits() {
        let g = neutral();
        assert!((personality_speed_factor(&g) - 1.0).abs() < 1e-6);
        assert!((personality_reproduction_factor(&g) - 1.0).abs() < 1e-6);
        let mut a = ActionRegister {
            feed_intent: 1.0,
            fire_intent: 1.0,
            share_intent: 1.0,
            move_x: 0.3,
            target_id: 7,
            ..Default::default()
        };
        let before = a;
        let s = SensorRegister {
            nearest_same_id: 7,
            nearest_other_id: 9,
            nearest_same_dir: Vec2::new(1.0, 0.0),
            nearest_other_dir: Vec2::new(0.0, 1.0),
            ..Default::default()
        };
        apply_personality(&mut a, &g, &s, SPAWN_ENERGY); // energy above comfort
        assert!((a.feed_intent - before.feed_intent).abs() < 1e-6);
        assert!((a.fire_intent - before.fire_intent).abs() < 1e-6);
        assert!((a.share_intent - before.share_intent).abs() < 1e-6);
        assert!((a.move_x - before.move_x).abs() < 1e-6);
    }

    #[test]
    fn openness_and_conscientiousness_factors_scale_with_trait() {
        let mut g = neutral();
        g.set(GenomeSlot::Openness, 1.0);
        assert!(personality_speed_factor(&g) > 1.0);
        g.set(GenomeSlot::Openness, 0.0);
        assert!(personality_speed_factor(&g) < 1.0);
        let mut g2 = neutral();
        g2.set(GenomeSlot::Conscientiousness, 1.0);
        assert!(personality_reproduction_factor(&g2) > 1.0);
    }

    #[test]
    fn extraversion_biases_toward_same_neighbor() {
        let mut g = neutral();
        g.set(GenomeSlot::Extraversion, 1.0);
        let mut a = ActionRegister::default();
        let s = same_neighbor(Vec2::new(1.0, 0.0), 3);
        apply_personality(&mut a, &g, &s, SPAWN_ENERGY);
        assert!(a.move_x > 0.0, "extravert should bias toward same neighbor");
    }

    #[test]
    fn agreeableness_raises_share_and_suppresses_kin_fire() {
        let mut hi = neutral();
        hi.set(GenomeSlot::Agreeableness, 1.0);
        let s = same_neighbor(Vec2::ZERO, 5);
        let mut a = ActionRegister {
            share_intent: 1.0,
            fire_intent: 1.0,
            target_id: 5,
            ..Default::default()
        };
        apply_personality(&mut a, &hi, &s, SPAWN_ENERGY);
        assert!(a.share_intent > 1.0, "agreeable shares more");
        assert!(a.fire_intent < 1.0, "agreeable suppresses kin attack");

        let mut lo = neutral();
        lo.set(GenomeSlot::Agreeableness, 0.0); // antagonistic
        let mut a2 = ActionRegister { fire_intent: 1.0, target_id: 5, ..Default::default() };
        apply_personality(&mut a2, &lo, &s, SPAWN_ENERGY);
        assert!(a2.fire_intent > 1.0, "antagonist attacks kin more");
    }

    #[test]
    fn neuroticism_flees_other_species_and_dampens_feed() {
        let mut g = neutral();
        g.set(GenomeSlot::Neuroticism, 1.0);
        let mut a = ActionRegister { feed_intent: 1.0, ..Default::default() };
        let s = other_neighbor(Vec2::new(1.0, 0.0), 8);
        apply_personality(&mut a, &g, &s, SPAWN_ENERGY);
        assert!(a.move_x < 0.0, "neurotic flees away from other-species");
        assert!(a.feed_intent < 1.0, "neurotic dampens feeding under threat");
    }

    #[test]
    fn conscientiousness_boosts_feed_when_hungry() {
        let mut g = neutral();
        g.set(GenomeSlot::Conscientiousness, 1.0);
        let mut a = ActionRegister { feed_intent: 1.0, ..Default::default() };
        let s = SensorRegister::default();
        apply_personality(&mut a, &g, &s, 0.1 * SPAWN_ENERGY); // below comfort
        assert!(a.feed_intent > 1.0, "conscientious boosts feeding when hungry");
    }
}
