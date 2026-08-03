//! Subcortical affect layer (Panksepp primary-process systems) — Layer 0
//! homeostatic drive + Layer 1 leaky-integrator activations that bias (and, in
//! later milestones, override) the evolved program. Gated on
//! `World::affect_enabled`: with the flag off `develop_all` is a strict no-op,
//! every read-side hook is exact identity at neutral (all-zero) affect, and the
//! stage draws ZERO RNG — so a flag-off world is byte-identical.
//!
//! Biological framing (design spec §1.2): functional/temporal layering
//! (subsumption), NOT evolutionary strata. We model survival/motivational
//! circuits and make no claim that agents *feel* anything.

use crate::{
    genome::Genome, prelude::Vec2, program::ActionRegister, sense::SensorRegister, world::World,
};

/// Number of Panksepp primary-process systems tracked per agent.
pub const AFFECT_SYSTEMS: usize = 7;

// Activation indices into `AffectState`.
pub const SEEK: usize = 0;
pub const FEAR: usize = 1;
pub const RAGE: usize = 2;
pub const LUST: usize = 3;
pub const CARE: usize = 4;
pub const PANIC: usize = 5; // PANIC/GRIEF (separation distress)
pub const PLAY: usize = 6;

/// Hijack fires when threat arousal >= this (before Reactivity modulation). M-B.
pub const HIJACK_AROUSAL_THRESHOLD: f32 = 0.6;

/// Per-system leaky-integrator retention (how long an activation lingers).
pub const LAMBDA_DEFAULT: f32 = 0.8;

/// SEEKING forage-bias gain: how hard a hungry agent steers toward `plant_direction`.
pub const K_SEEK_FORAGE: f32 = 0.6;
/// SEEKING wander gain: intensification of the program's own heading when no
/// food is sensed. A deterministic, RNG-free exploratory proxy — the read-side
/// hook has neither position nor RNG to synthesize a fresh wander vector, so it
/// amplifies whatever direction the evolved program already chose.
pub const K_SEEK_WANDER: f32 = 0.3;
/// Movement-speed gain from SEEKING (+ arousal, which is 0 in M-A).
pub const K_AFFECT_SPEED: f32 = 0.5;

// --- M-B: FEAR / threat circuit ---
/// Perception distance (world units) beyond which a threatening neighbor stops
/// contributing to FEAR. Below it, proximity scales the threat linearly.
pub const FEAR_RANGE: f32 = 200.0;
/// Weight of the size/proximity threat term in the FEAR trigger.
pub const K_FEAR_THREAT: f32 = 1.0;
/// Weight of the war-hostility term in the FEAR trigger.
pub const K_FEAR_HOSTILITY: f32 = 0.8;
/// Boldness gain on the FEAR response: fear is scaled by `(1 - K_FEAR_BOLDNESS *
/// boldness())`, so bold (+1) feels ~0.7× and timid (−1) ~1.3× the raw threat;
/// zero threat stays zero fear for every temperament.
pub const K_FEAR_BOLDNESS: f32 = 0.3;
/// FEAR leaky-integrator retention (how long fear lingers). Reuses the default.
pub const LAMBDA_FEAR: f32 = LAMBDA_DEFAULT;
/// Flee-bias gain: FEAR pushes movement away from the threat direction.
pub const K_FLEE: f32 = 0.6;
/// Non-defensive-intent damping gain under FEAR (share/broadcast/emit).
pub const K_FEAR_DAMP: f32 = 0.5;

/// Per-agent subcortical activations, one per Panksepp system, each in `[0,1]`.
/// Persistent (serialized). Neutral default = all zero.
pub type AffectState = [f32; AFFECT_SYSTEMS];

/// Layer-0 homeostatic drive: normalized energy deficit in `[0,1]`. 0 = sated
/// (energy >= SPAWN_ENERGY), → 1 as energy → 0. Setpoint is `SPAWN_ENERGY`.
#[inline]
pub fn homeostatic_drive(energy: f32) -> f32 {
    let setpoint = crate::agent::SPAWN_ENERGY;
    ((setpoint - energy) / setpoint).clamp(0.0, 1.0)
}

/// Aggregate threat arousal from the defensive activations (FEAR, RAGE, PANIC).
/// `max` of the three — SEEKING and the affiliative systems do not raise threat.
/// Exactly `0.0` at neutral. ZERO RNG.
#[inline]
pub fn arousal(affect: &AffectState) -> f32 {
    affect[FEAR].max(affect[RAGE]).max(affect[PANIC])
}

/// Movement-speed multiplier from SEEKING + arousal. Exactly `1.0` at neutral
/// (all-zero) affect. Consumed in integrate.rs alongside personality_speed_factor.
#[inline]
pub fn affect_speed_factor(affect: &AffectState) -> f32 {
    (1.0 + K_AFFECT_SPEED * affect[SEEK]).max(0.0)
}

/// Reproduction-threshold multiplier from LUST. Exactly `1.0` at neutral.
/// M-A ships this identity stub; M-C implements the LUST effect and wires it
/// into reproduce.rs.
#[inline]
pub fn affect_reproduction_factor(_affect: &AffectState) -> f32 {
    1.0
}

/// Instantaneous FEAR drive from THIS tick's fresh sensors + temperament.
/// Pure function of `world.sensors` (recomputed every tick before `develop_all`
/// reads it) + genome ⇒ replay-safe, ZERO RNG. Returns `[0,1]`. `0.0` when there
/// is no locatable other-species neighbor and no hostility.
pub(crate) fn fear_trigger(sensors: &SensorRegister, genome: &Genome) -> f32 {
    let mut threat = 0.0f32;
    if sensors.nearest_other_id != crate::sense::NO_NEIGHBOR_ID {
        // Closer + bigger + more energetic other-species neighbor ⇒ scarier.
        // rel_size/rel_energy are of the overall-nearest neighbor; when the
        // nearest is the other-species predator (the case that matters) they
        // describe it. Approximation documented in the spec-deviation note.
        let prox = (1.0 - sensors.nearest_other_dist / FEAR_RANGE).clamp(0.0, 1.0);
        let size = sensors.nearest_rel_size.clamp(0.0, 2.0) * 0.5; // rel 2.0 ⇒ 1.0
        let ener = (sensors.nearest_rel_energy - 1.0).clamp(0.0, 1.0); // stronger prey feels safe
        threat += K_FEAR_THREAT * prox * (size + 0.5 * ener).min(1.0);
    }
    threat += K_FEAR_HOSTILITY * sensors.hostility;
    // Boldness modulates the RESPONSE to threat as a GAIN, not a baseline offset:
    // bold (+1) scales fear down (~0.7×), timid (−1) up (~1.3×) — but zero threat
    // ⇒ zero fear for every temperament (no phantom baseline). Signed [-1,+1].
    threat = (threat * (1.0 - K_FEAR_BOLDNESS * genome.boldness())).clamp(0.0, 1.0);
    threat
}

/// Compute stage (Layer 0 → Layer 1). Update each alive agent's affect column
/// from this tick's physiology as a leaky integrator. M-A drives SEEK from the
/// homeostatic energy deficit; the other six systems stay 0.0 (later milestones
/// fill their triggers — FEAR reads sensors in M-B, etc.). STRICT no-op when
/// `!world.affect_enabled`. ZERO RNG. Index-disjoint `par_iter` (iq::develop_all
/// template): each agent writes only its own slot and reads only shared columns
/// by `&`, so the parallel loop is bit-identical to a serial ascending-id loop.
/// Runs post-sense / pre-decide so THIS tick's decision reads fresh affect.
pub fn develop_all(world: &mut World) {
    if !world.affect_enabled {
        return;
    }
    use rayon::prelude::*;
    let cap = world.agents.capacity();
    let sensors = &world.sensors;
    let crate::agent::AgentBuffers { affect, energy, genome, alive, .. } = &mut world.agents;
    let (energy, genome, alive) = (&*energy, &*genome, &*alive);
    affect[..cap].par_iter_mut().enumerate().for_each(|(i, a)| {
        if !alive[i] {
            return;
        }
        // Layer 0: homeostatic drive (energy deficit) powers SEEKING.
        let drive = homeostatic_drive(energy[i]);
        // Layer 1: leaky-integrator update of the SEEK activation.
        let seek = LAMBDA_DEFAULT * a[SEEK] + (1.0 - LAMBDA_DEFAULT) * drive;
        a[SEEK] = seek.clamp(0.0, 1.0);

        // FEAR (M-B): threat/survival drive from fresh sensors + Boldness. The
        // sensors buffer can be shorter than capacity on a growth tick; guard it.
        if i < sensors.len() {
            let fear_in = fear_trigger(&sensors[i], &genome[i]);
            a[FEAR] = (LAMBDA_FEAR * a[FEAR] + (1.0 - LAMBDA_FEAR) * fear_in).clamp(0.0, 1.0);
        }
    });
}

/// Read-side bias hook. Modulate `action` from current affect + percepts +
/// temperament. EXACT IDENTITY at neutral (all-zero) affect — the SEEKING block
/// is guarded `if seek != 0.0` (personality.rs idiom), so a neutral agent's
/// action is left bit-for-bit unchanged. Called in decide_all right AFTER
/// `apply_personality`. Writes only live channels; no RNG. M-A implements
/// SEEKING; later milestones add their systems (they will read `genome`/`energy`).
pub fn apply_affect(
    action: &mut ActionRegister,
    affect: &AffectState,
    _genome: &Genome,
    sensors: &SensorRegister,
    _energy: f32,
) {
    // SEEKING: steer toward food when a plant direction is sensed; otherwise
    // intensify the program's own heading as a deterministic exploratory wander
    // (no RNG/position is available in the read-side hook).
    let seek = affect[SEEK];
    if seek != 0.0 {
        let pd = sensors.plant_direction;
        if pd != Vec2::ZERO {
            action.move_x += K_SEEK_FORAGE * seek * pd.x;
            action.move_y += K_SEEK_FORAGE * seek * pd.y;
        } else {
            let gain = 1.0 + K_SEEK_WANDER * seek;
            action.move_x *= gain;
            action.move_y *= gain;
        }
    }

    // FEAR (M-B): flee the nearest other-species neighbor and dampen non-defensive
    // LIVE intents (share/broadcast/emit). Guarded so neutral affect is identity.
    let fear = affect[FEAR];
    if fear != 0.0 && sensors.nearest_other_id != crate::sense::NO_NEIGHBOR_ID {
        action.move_x -= K_FLEE * fear * sensors.nearest_other_dir.x;
        action.move_y -= K_FLEE * fear * sensors.nearest_other_dir.y;
        let damp = (1.0 - K_FEAR_DAMP * fear).max(0.0);
        action.share_intent *= damp;
        for c in action.broadcast_intent.iter_mut() {
            *c *= damp;
        }
        for c in action.emit_intent.iter_mut() {
            *c *= damp;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::SPAWN_ENERGY;
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::program::ActionRegister;
    use crate::sense::SensorRegister;
    use crate::world::World;

    #[test]
    fn homeostatic_drive_is_zero_when_sated_and_one_when_empty() {
        let e = crate::agent::SPAWN_ENERGY;
        assert_eq!(homeostatic_drive(e), 0.0);
        assert_eq!(homeostatic_drive(0.0), 1.0);
        assert!((homeostatic_drive(e * 0.5) - 0.5).abs() < 1e-6);
        assert_eq!(homeostatic_drive(e * 2.0), 0.0, "surplus clamps to 0");
    }

    #[test]
    fn arousal_is_zero_at_neutral_and_maxes_the_defensive_systems() {
        assert_eq!(arousal(&[0.0; AFFECT_SYSTEMS]), 0.0);
        let mut a = [0.0; AFFECT_SYSTEMS];
        a[FEAR] = 0.4;
        a[RAGE] = 0.7;
        a[PANIC] = 0.1;
        assert!((arousal(&a) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn arousal_is_max_of_defensive_activations() {
        let mut a: AffectState = [0.0; AFFECT_SYSTEMS];
        assert_eq!(arousal(&a), 0.0); // neutral
        a[SEEK] = 0.9; // appetitive, not defensive
        assert_eq!(arousal(&a), 0.0, "SEEKING must not raise threat arousal");
        a[FEAR] = 0.7;
        assert_eq!(arousal(&a), 0.7);
        a[PANIC] = 0.8;
        assert_eq!(arousal(&a), 0.8, "PANIC dominates");
        a[RAGE] = 0.85;
        assert_eq!(arousal(&a), 0.85, "RAGE dominates");
    }

    #[test]
    fn factors_are_identity_at_neutral() {
        let neutral = [0.0; AFFECT_SYSTEMS];
        assert_eq!(affect_speed_factor(&neutral), 1.0);
        assert_eq!(affect_reproduction_factor(&neutral), 1.0);
        let mut a = neutral;
        a[SEEK] = 1.0;
        assert!(affect_speed_factor(&a) > 1.0, "SEEKING speeds foraging up");
    }

    #[test]
    fn develop_is_noop_when_flag_off() {
        let mut w = World::new(2);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.agents.energy[id as usize] = 0.0; // would build SEEK if the layer ran
        develop_all(&mut w);
        assert_eq!(
            w.agents.affect[id as usize], [0.0; AFFECT_SYSTEMS],
            "flag off ⇒ affect untouched, zero work"
        );
    }

    #[test]
    fn seeking_builds_from_energy_deficit_when_on() {
        let mut w = World::new(2);
        w.affect_enabled = true;
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.agents.energy[id as usize] = 0.0; // max drive = 1.0
        develop_all(&mut w);
        // One tick from neutral: seek = λ·0 + (1−λ)·1.0 = (1−λ).
        let seek = w.agents.affect[id as usize][SEEK];
        assert!((seek - (1.0 - LAMBDA_DEFAULT)).abs() < 1e-6);
    }

    #[test]
    fn sated_agent_builds_no_seek() {
        let mut w = World::new(2);
        w.affect_enabled = true;
        // Spawn energy == SPAWN_ENERGY ⇒ drive 0 ⇒ SEEK stays 0.
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        develop_all(&mut w);
        assert_eq!(w.agents.affect[id as usize][SEEK], 0.0, "sated ⇒ no SEEKING");
    }

    #[test]
    fn apply_affect_is_identity_at_neutral() {
        let mut action = ActionRegister { move_x: 0.3, move_y: -0.2, ..Default::default() };
        let before = action; // ActionRegister is Copy
        let affect = [0.0; AFFECT_SYSTEMS];
        let s = SensorRegister { plant_direction: Vec2::new(1.0, 0.0), ..Default::default() };
        apply_affect(&mut action, &affect, &Genome::neutral(), &s, SPAWN_ENERGY);
        assert_eq!(action.move_x, before.move_x);
        assert_eq!(action.move_y, before.move_y);
    }

    #[test]
    fn seeking_biases_toward_sensed_food() {
        let mut action = ActionRegister::default();
        let mut affect = [0.0; AFFECT_SYSTEMS];
        affect[SEEK] = 1.0;
        let s = SensorRegister { plant_direction: Vec2::new(1.0, 0.0), ..Default::default() };
        apply_affect(&mut action, &affect, &Genome::neutral(), &s, 0.0);
        assert!(action.move_x > 0.0, "high SEEKING steers toward food (+x)");
    }

    #[test]
    fn seeking_intensifies_heading_when_no_food() {
        let mut action = ActionRegister { move_x: 0.5, move_y: 0.0, ..Default::default() };
        let mut affect = [0.0; AFFECT_SYSTEMS];
        affect[SEEK] = 1.0;
        let s = SensorRegister::default(); // plant_direction == Vec2::ZERO
        apply_affect(&mut action, &affect, &Genome::neutral(), &s, 0.0);
        assert!(action.move_x > 0.5, "no-food SEEKING intensifies the program's heading");
    }

    #[test]
    fn fear_trigger_rises_with_close_big_hostile_threat_and_falls_with_boldness() {
        use crate::genome::GenomeSlot;

        // No neighbor ⇒ no fear.
        let mut s = SensorRegister::default();
        assert_eq!(fear_trigger(&s, &Genome::neutral()), 0.0);

        // A close, larger other-species neighbor ⇒ moderate (UNSATURATED) fear, so the
        // boldness gain is demonstrable rather than hidden behind the 1.0 clamp.
        s.nearest_other_id = 7;
        s.nearest_neighbor_id = 7;
        s.nearest_other_dist = 60.0; // prox = 1 - 60/200 = 0.7
        s.nearest_other_dir = Vec2::new(1.0, 0.0);
        s.nearest_rel_size = 1.6; // size term = 1.6*0.5 = 0.8
        s.nearest_rel_energy = 1.0; // no energy bonus
        s.hostility = 0.0;
        let neutral = fear_trigger(&s, &Genome::neutral()); // ~0.56, unsaturated
        assert!(
            neutral > 0.4 && neutral < 1.0,
            "expected moderate unsaturated fear, got {neutral}"
        );

        // A bold genome (Boldness = 1.0 ⇒ boldness() = +1.0) feels measurably LESS fear.
        let mut bold = Genome::neutral();
        bold.set(GenomeSlot::Boldness, 1.0);
        let brave = fear_trigger(&s, &bold);
        assert!(brave < neutral - 0.1, "boldness must scale fear down: {brave} vs {neutral}");

        // Invariant: zero threat ⇒ zero fear for EVERY temperament, including timid
        // (Boldness slot 0.0 ⇒ boldness() = -1.0). Guards against phantom baseline fear.
        let mut timid = Genome::neutral();
        timid.set(GenomeSlot::Boldness, 0.0);
        assert_eq!(
            fear_trigger(&SensorRegister::default(), &timid),
            0.0,
            "no threat ⇒ no fear even for timid"
        );

        // A distant neighbor is barely threatening.
        let mut far = s;
        far.nearest_other_dist = FEAR_RANGE * 2.0;
        assert!(fear_trigger(&far, &Genome::neutral()) < neutral);
    }

    #[test]
    fn develop_all_raises_fear_from_a_threatening_sensor() {
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::world::World;

        // Flag ON: a close, large, hostile other-species sensor ⇒ FEAR integrates up.
        let mut w = World::new(3);
        w.affect_enabled = true;
        let prey = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.resize_scratch(); // size world.sensors to capacity (crate-private; ok in lib test)
        {
            let s = &mut w.sensors[prey as usize];
            s.nearest_other_id = 99;
            s.nearest_other_dist = 30.0;
            s.nearest_other_dir = Vec2::new(1.0, 0.0);
            s.nearest_rel_size = 2.0; // predator twice our size
            s.hostility = 0.4;
        }
        develop_all(&mut w); // reads world.sensors[prey], folds FEAR into affect[prey]
        let fear = w.agents.affect[prey as usize][FEAR];
        assert!(fear > 0.0, "a threatening sensor must raise FEAR via develop_all, got {fear}");

        // Flag OFF: identical threatening sensor ⇒ develop_all is a strict no-op.
        let mut w2 = World::new(3);
        let prey2 = w2.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w2.resize_scratch();
        w2.sensors[prey2 as usize] = w.sensors[prey as usize];
        develop_all(&mut w2);
        assert_eq!(w2.agents.affect[prey2 as usize][FEAR], 0.0, "flag off ⇒ no FEAR");
    }

    #[test]
    fn apply_affect_fear_flees_and_dampens_but_is_identity_at_neutral() {
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::program::ActionRegister;
        use crate::sense::SensorRegister;

        let g = Genome::neutral();
        let mut s = SensorRegister::default();
        s.nearest_other_id = 4;
        s.nearest_other_dir = Vec2::new(1.0, 0.0); // threat to the +x

        // Neutral affect ⇒ exact identity (no arithmetic).
        let mut neutral_act = ActionRegister::default();
        neutral_act.move_x = 0.3;
        neutral_act.share_intent = 0.5;
        let before = neutral_act;
        apply_affect(&mut neutral_act, &[0.0; AFFECT_SYSTEMS], &g, &s, 100.0);
        assert_eq!(neutral_act.move_x, before.move_x);
        assert_eq!(neutral_act.share_intent, before.share_intent);

        // High FEAR ⇒ movement biased AWAY from the threat, share dampened.
        let mut a: AffectState = [0.0; AFFECT_SYSTEMS];
        a[FEAR] = 0.8;
        let mut act = ActionRegister::default();
        act.move_x = 0.0;
        act.share_intent = 0.5;
        act.broadcast_intent[0] = 0.4;
        apply_affect(&mut act, &a, &g, &s, 100.0);
        assert!(act.move_x < 0.0, "FEAR should push away from +x threat, got {}", act.move_x);
        assert!(act.share_intent < 0.5, "FEAR should dampen sharing");
        assert!(act.broadcast_intent[0] < 0.4, "FEAR should dampen broadcasts");
    }
}
