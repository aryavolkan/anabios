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

use crate::world::World;

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

/// Per-agent subcortical activations, one per Panksepp system, each in [0,1].
/// Persistent (serialized). Neutral default = all zero.
pub type AffectState = [f32; AFFECT_SYSTEMS];

/// Layer-0 homeostatic drive: normalized energy deficit in [0,1]. 0 = sated
/// (energy >= SPAWN_ENERGY), → 1 as energy → 0. Setpoint is `SPAWN_ENERGY`.
#[inline]
pub fn homeostatic_drive(energy: f32) -> f32 {
    let setpoint = crate::agent::SPAWN_ENERGY;
    ((setpoint - energy) / setpoint).clamp(0.0, 1.0)
}

/// Aggregate threat arousal from the defensive activations (FEAR, RAGE, PANIC).
/// M-A: those stay 0.0, so this is a 0.0 baseline; M-B finalizes it with the
/// hijack.
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
    let crate::agent::AgentBuffers { affect, energy, alive, .. } = &mut world.agents;
    let (energy, alive) = (&*energy, &*alive);
    affect[..cap].par_iter_mut().enumerate().for_each(|(i, a)| {
        if !alive[i] {
            return;
        }
        // Layer 0: homeostatic drive (energy deficit) powers SEEKING.
        let drive = homeostatic_drive(energy[i]);
        // Layer 1: leaky-integrator update of the SEEK activation.
        let seek = LAMBDA_DEFAULT * a[SEEK] + (1.0 - LAMBDA_DEFAULT) * drive;
        a[SEEK] = seek.clamp(0.0, 1.0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;
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
}
