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

// --- M-B: survival-reflex hijack ---
/// Reactivity raises hijack sensitivity; Boldness lowers it. Both signed [-1,1].
pub const K_HIJACK_REACT: f32 = 0.2;
pub const K_HIJACK_BOLD: f32 = 0.2;
/// Threat farther than this ⇒ Freeze (orient, don't be seen).
pub const FREEZE_DIST: f32 = 140.0;
/// Threat closer than this ⇒ cornered (Fight or Fright/Faint).
pub const CORNER_DIST: f32 = 30.0;
/// Cornered arousal at/above which a too-weak agent tips into Fright/Faint.
pub const FAINT_AROUSAL: f32 = 0.95;
/// Minimum energy to choose Fight (turn-and-attack) when cornered.
pub const FIGHT_ENERGY_MIN: f32 = 0.35 * crate::agent::SPAWN_ENERGY;
/// fire_intent asserted when the hijack chooses Fight.
pub const FIRE_HIJACK: f32 = 1.0;

// --- M-C: RAGE (agonistic) ---
/// Neighbour count at which crowd-pressure (the "blocked from resources" half of
/// frustration) saturates. Higher crowding + hunger ⇒ more RAGE.
pub const RAGE_CROWD_REF: f32 = 6.0;
/// RAGE added to a target's activation each time it takes combat damage
/// (written into the serialized `affect` column by `interact::combat_pass`, so
/// no new column and no serde-skip replay hazard). Clamped to 1.0.
pub const RAGE_ATTACK_IMPULSE: f32 = 0.5;
/// RAGE → `fire_intent` gain (approach-and-attack the combat target).
pub const K_RAGE_FIRE: f32 = 1.0;
/// RAGE → approach-target movement gain.
pub const K_RAGE_APPROACH: f32 = 0.5;
/// FEAR⊣RAGE lateral-inhibition strength (flee before fight): at FEAR = 1 and
/// strength 1, RAGE is fully suppressed.
pub const FEAR_INHIBITS_RAGE: f32 = 1.0;

// --- M-C: LUST (reproductive) ---
/// LUST → reproduction-threshold *reduction* (lowers the mating energy gate).
/// At LUST = 1 the gate drops by this fraction (30%).
pub const K_LUST_REPRO: f32 = 0.3;
/// LUST → approach-mate movement gain.
pub const K_LUST_APPROACH: f32 = 0.4;

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

/// Reproduction-threshold multiplier from LUST. Exactly `1.0` at neutral affect;
/// LUST lowers the mating energy gate by up to `K_LUST_REPRO` (30%). Consumed in
/// `reproduce::is_eligible` alongside `personality_reproduction_factor`.
#[inline]
pub fn affect_reproduction_factor(affect: &AffectState) -> f32 {
    let lust = affect[LUST];
    if lust != 0.0 {
        (1.0 - K_LUST_REPRO * lust).max(0.0)
    } else {
        1.0
    }
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

/// RAGE trigger — *derived frustration*. anabios has no native "frustration"
/// field, so we derive it: an agent is frustrated when it is **hungry while
/// blocked**, i.e. high homeostatic `drive` (energy deficit, a proxy for low
/// recent intake) AND high `crowding` (competitors in the way of the resource).
/// The product means BOTH must hold — a well-fed crowded agent, or a starving
/// solitary one, is not frustrated. Scaled by an `aggressiveness`-derived gain
/// (RAGE gain gene, mapped `[-1,+1] → [0,1]`). Result in `[0,1]`.
///
/// Note: the "having-been-attacked this tick" half of the spec's heuristic is
/// applied as a separate impulse in `interact::combat_pass` (see
/// `RAGE_ATTACK_IMPULSE`), NOT here — combat runs after the affect stage, and
/// its `#[serde(skip)]` `combat_damaged` scratch cannot be read into serialized
/// affect across a tick boundary without reintroducing the serde-skip replay
/// footgun. Writing the impulse into the serialized `affect` column at combat
/// time keeps it determinism-safe.
pub fn trigger_rage(drive: f32, genome: &Genome, sensors: &SensorRegister) -> f32 {
    let crowd_pressure = (sensors.crowding as f32 / RAGE_CROWD_REF).clamp(0.0, 1.0);
    let frustration = drive * crowd_pressure;
    // aggressiveness() ∈ [-1,+1]; map to a [0,1] gain (neutral genome ⇒ 0.5).
    let gain = (0.5 + 0.5 * genome.aggressiveness()).clamp(0.0, 1.0);
    (frustration * gain).clamp(0.0, 1.0)
}

/// LUST trigger — mate-readiness. Rises to 1.0 when the agent's energy is at or
/// above the mating gate (`SPAWN_ENERGY × ReproductionThreshold × REPRO_ENERGY_MULT`,
/// the same reference `reproduce::is_eligible` uses) AND a same-species neighbour
/// is in perception (`nearest_same_id`). Zero otherwise. Deterministic function
/// of serialized/​recomputed state only (energy + genome + this tick's sensors).
pub fn trigger_lust(energy: f32, genome: &Genome, sensors: &SensorRegister) -> f32 {
    if sensors.nearest_same_id == crate::sense::NO_NEIGHBOR_ID {
        return 0.0;
    }
    let repro_energy = crate::agent::SPAWN_ENERGY
        * genome.get(crate::genome::GenomeSlot::ReproductionThreshold)
        * crate::reproduce::REPRO_ENERGY_MULT;
    if repro_energy <= 0.0 || energy < repro_energy {
        return 0.0;
    }
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

            // M-C RAGE: derived frustration (hungry + blocked), gated by the
            // aggressiveness gain gene. Zero RNG.
            let t_rage = trigger_rage(drive, &genome[i], &sensors[i]);
            a[RAGE] = (LAMBDA_DEFAULT * a[RAGE] + (1.0 - LAMBDA_DEFAULT) * t_rage).clamp(0.0, 1.0);
            // M-C LUST: mate-readiness (energy ≥ mating gate + same-species
            // neighbour present).
            let t_lust = trigger_lust(energy[i], &genome[i], &sensors[i]);
            a[LUST] = (LAMBDA_DEFAULT * a[LUST] + (1.0 - LAMBDA_DEFAULT) * t_lust).clamp(0.0, 1.0);

            // M-C lateral inhibition — flee before fight: FEAR gates down RAGE,
            // fully suppressing it at FEAR = 1 (with FEAR_INHIBITS_RAGE = 1).
            a[RAGE] = (a[RAGE] * (1.0 - FEAR_INHIBITS_RAGE * a[FEAR])).clamp(0.0, 1.0);
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

    // M-C RAGE: approach and attack the nearest other-species neighbour (the
    // target combat_pass resolves). Guarded on RAGE ≠ 0 → exact identity at
    // neutral affect.
    let rage = affect[RAGE];
    if rage != 0.0 && sensors.nearest_other_id != crate::sense::NO_NEIGHBOR_ID {
        action.fire_intent += K_RAGE_FIRE * rage;
        action.move_x += K_RAGE_APPROACH * rage * sensors.nearest_other_dir.x;
        action.move_y += K_RAGE_APPROACH * rage * sensors.nearest_other_dir.y;
    }

    // M-C LUST: approach the nearest same-species neighbour (a potential mate).
    // Guarded on LUST ≠ 0 → exact identity at neutral affect. mate_intent stays
    // latent; the reproduction gate is lowered via affect_reproduction_factor.
    let lust = affect[LUST];
    if lust != 0.0 && sensors.nearest_same_id != crate::sense::NO_NEIGHBOR_ID {
        action.move_x += K_LUST_APPROACH * lust * sensors.nearest_same_dir.x;
        action.move_y += K_LUST_APPROACH * lust * sensors.nearest_same_dir.y;
    }
}

/// Survival-reflex override (Bracha: Freeze→Flight→Fight→Fright/Faint). When
/// threat arousal, scaled by Reactivity/Boldness, reaches
/// `HIJACK_AROUSAL_THRESHOLD`, OVERWRITE the LIVE action channels with the
/// reflex chosen by threat proximity/escapability and return `true`. Otherwise
/// leave `action` untouched and return `false`. ZERO RNG. Exact identity at
/// neutral affect (arousal 0 ⇒ returns false before touching `action`).
pub fn apply_hijack(
    action: &mut ActionRegister,
    affect: &AffectState,
    genome: &Genome,
    sensors: &SensorRegister,
    energy: f32,
) -> bool {
    let threat = arousal(affect);
    if threat <= 0.0 {
        return false;
    }
    // "low road" cancel path: bold/steady agents keep cortical control longer.
    let effective =
        threat + K_HIJACK_REACT * genome.reactivity() - K_HIJACK_BOLD * genome.boldness();
    if effective < HIJACK_AROUSAL_THRESHOLD {
        return false;
    }

    // No locatable threat ⇒ Freeze in place.
    if sensors.nearest_other_id == crate::sense::NO_NEIGHBOR_ID {
        action.move_x = 0.0;
        action.move_y = 0.0;
        return true;
    }
    let d = sensors.nearest_other_dist;
    let toward = sensors.nearest_other_dir;
    if d >= FREEZE_DIST {
        // Freeze — distant/ambiguous.
        action.move_x = 0.0;
        action.move_y = 0.0;
    } else if d > CORNER_DIST {
        // Flight — flee directly away; affect_speed_factor (arousal-driven)
        // supplies the speed boost in integrate.rs.
        action.move_x = -toward.x;
        action.move_y = -toward.y;
    } else {
        // Cornered — Fight vs Fright/Faint resolved below.
        return hijack_cornered(action, affect, sensors, energy);
    }
    true
}

/// Cornered branch: Fight when able, else Fright/Faint. Writes only LIVE
/// channels. Always overrides ⇒ returns true.
fn hijack_cornered(
    action: &mut ActionRegister,
    affect: &AffectState,
    sensors: &SensorRegister,
    energy: f32,
) -> bool {
    let extreme = arousal(affect) >= FAINT_AROUSAL;
    let can_fight = energy >= FIGHT_ENERGY_MIN;
    if extreme && !can_fight {
        // Fright/Faint — tonic immobility, suppress LIVE intents.
        action.move_x = 0.0;
        action.move_y = 0.0;
        action.fire_intent = 0.0;
        action.share_intent = 0.0;
        for c in action.emit_intent.iter_mut() {
            *c = 0.0;
        }
        for c in action.broadcast_intent.iter_mut() {
            *c = 0.0;
        }
    } else {
        // Fight — turn to the threat and attack.
        action.move_x = sensors.nearest_other_dir.x;
        action.move_y = sensors.nearest_other_dir.y;
        action.fire_intent = action.fire_intent.max(FIRE_HIJACK);
        action.target_id = sensors.nearest_other_id;
    }
    true
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
    fn affect_reproduction_factor_lowers_gate_with_lust() {
        let neutral: AffectState = [0.0; AFFECT_SYSTEMS];
        assert_eq!(affect_reproduction_factor(&neutral), 1.0, "identity at neutral");
        let mut lusty: AffectState = [0.0; AFFECT_SYSTEMS];
        lusty[LUST] = 1.0;
        let f = affect_reproduction_factor(&lusty);
        assert!((0.0..1.0).contains(&f), "high LUST lowers the reproduction gate: {f}");
        assert!((f - (1.0 - K_LUST_REPRO)).abs() < 1e-6);
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
        let s = SensorRegister {
            nearest_other_id: 4,
            nearest_other_dir: Vec2::new(1.0, 0.0), // threat to the +x
            ..Default::default()
        };

        // Neutral affect ⇒ exact identity (no arithmetic).
        let mut neutral_act =
            ActionRegister { move_x: 0.3, share_intent: 0.5, ..Default::default() };
        let before = neutral_act;
        apply_affect(&mut neutral_act, &[0.0; AFFECT_SYSTEMS], &g, &s, 100.0);
        assert_eq!(neutral_act.move_x, before.move_x);
        assert_eq!(neutral_act.share_intent, before.share_intent);

        // High FEAR ⇒ movement biased AWAY from the threat, share dampened.
        let mut a: AffectState = [0.0; AFFECT_SYSTEMS];
        a[FEAR] = 0.8;
        let mut act = ActionRegister { move_x: 0.0, share_intent: 0.5, ..Default::default() };
        act.broadcast_intent[0] = 0.4;
        apply_affect(&mut act, &a, &g, &s, 100.0);
        assert!(act.move_x < 0.0, "FEAR should push away from +x threat, got {}", act.move_x);
        assert!(act.share_intent < 0.5, "FEAR should dampen sharing");
        assert!(act.broadcast_intent[0] < 0.4, "FEAR should dampen broadcasts");
    }

    #[test]
    fn apply_hijack_gate_and_freeze_flight() {
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::program::ActionRegister;
        use crate::sense::SensorRegister;

        let g = Genome::neutral();

        // Below-threshold arousal ⇒ no override, action untouched, returns false.
        let mut low: AffectState = [0.0; AFFECT_SYSTEMS];
        low[FEAR] = 0.3; // < HIJACK_AROUSAL_THRESHOLD (0.6)
        let s = SensorRegister {
            nearest_other_id: 2,
            nearest_other_dist: 100.0,
            nearest_other_dir: Vec2::new(1.0, 0.0),
            ..Default::default()
        };
        let mut act = ActionRegister { move_x: 0.9, ..Default::default() };
        assert!(!apply_hijack(&mut act, &low, &g, &s, 100.0));
        assert_eq!(act.move_x, 0.9, "no hijack below threshold");

        // High arousal, DISTANT threat ⇒ Freeze (zero movement), returns true.
        let mut hi: AffectState = [0.0; AFFECT_SYSTEMS];
        hi[FEAR] = 0.9;
        let mut freeze_s = s;
        freeze_s.nearest_other_dist = FREEZE_DIST + 10.0;
        let mut freeze_act = ActionRegister { move_x: 0.9, move_y: -0.4, ..Default::default() };
        assert!(apply_hijack(&mut freeze_act, &hi, &g, &freeze_s, 100.0));
        assert_eq!((freeze_act.move_x, freeze_act.move_y), (0.0, 0.0), "distant threat ⇒ Freeze");

        // High arousal, MID-RANGE threat ⇒ Flight (flee away from +x), returns true.
        let mut flight_s = s;
        flight_s.nearest_other_dist = (FREEZE_DIST + CORNER_DIST) * 0.5;
        // was charging toward threat
        let mut flight_act = ActionRegister { move_x: 0.9, ..Default::default() };
        assert!(apply_hijack(&mut flight_act, &hi, &g, &flight_s, 100.0));
        assert!(flight_act.move_x < 0.0, "mid-range threat ⇒ flee -x, got {}", flight_act.move_x);
    }

    #[test]
    fn apply_hijack_cornered_fights_or_faints() {
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::program::{ActionRegister, NO_TARGET};
        use crate::sense::SensorRegister;

        let g = Genome::neutral();
        let mut hi: AffectState = [0.0; AFFECT_SYSTEMS];
        hi[FEAR] = 0.9;
        let s = SensorRegister {
            nearest_other_id: 11,
            nearest_other_dir: Vec2::new(1.0, 0.0),
            nearest_other_dist: CORNER_DIST * 0.5, // cornered
            ..Default::default()
        };

        // Cornered but able (energy high) ⇒ Fight: approach + fire + target set.
        let mut fight = ActionRegister::default();
        assert!(apply_hijack(&mut fight, &hi, &g, &s, 100.0));
        assert!(fight.move_x > 0.0, "Fight approaches the threat (+x)");
        assert!(fight.fire_intent > 0.0, "Fight fires");
        assert_eq!(fight.target_id, 11);
        assert_ne!(fight.target_id, NO_TARGET);

        // Cornered, extreme arousal, and too weak to fight ⇒ Fright/Faint:
        // tonic immobility, intents suppressed.
        let mut faint_aff: AffectState = [0.0; AFFECT_SYSTEMS];
        faint_aff[FEAR] = FAINT_AROUSAL + 0.01;
        let mut faint = ActionRegister {
            move_x: 0.7,
            fire_intent: 0.5,
            share_intent: 0.5,
            ..Default::default()
        };
        faint.broadcast_intent[0] = 0.5;
        assert!(apply_hijack(&mut faint, &faint_aff, &g, &s, FIGHT_ENERGY_MIN - 1.0));
        assert_eq!((faint.move_x, faint.move_y), (0.0, 0.0), "Faint ⇒ tonic immobility");
        assert_eq!(faint.fire_intent, 0.0);
        assert_eq!(faint.share_intent, 0.0);
        assert_eq!(faint.broadcast_intent[0], 0.0);
    }

    #[test]
    fn trigger_rage_scales_with_drive_and_crowding() {
        use crate::genome::Genome;
        use crate::sense::SensorRegister;
        let g = Genome::neutral(); // aggressiveness() == 0.0 → gain 0.5
                                   // No crowding → no frustration regardless of drive.
        let alone = SensorRegister { crowding: 0, ..Default::default() };
        assert_eq!(trigger_rage(1.0, &g, &alone), 0.0);
        // High drive + crowded → positive frustration.
        let crowded = SensorRegister { crowding: RAGE_CROWD_REF as u32, ..Default::default() };
        let r = trigger_rage(1.0, &g, &crowded);
        assert!(r > 0.0 && r <= 1.0, "frustrated agent has positive RAGE trigger: {r}");
        // Well-fed (drive 0) → no frustration even when crowded.
        assert_eq!(trigger_rage(0.0, &g, &crowded), 0.0);
    }

    #[test]
    fn trigger_rage_gain_rises_with_aggressiveness() {
        use crate::genome::{Genome, GenomeSlot};
        use crate::sense::SensorRegister;
        let crowded = SensorRegister { crowding: RAGE_CROWD_REF as u32, ..Default::default() };
        let mut calm = Genome::neutral();
        calm.set(GenomeSlot::Aggressiveness, 0.0); // aggressiveness() == -1.0 → gain 0.0
        let mut fierce = Genome::neutral();
        fierce.set(GenomeSlot::Aggressiveness, 1.0); // aggressiveness() == +1.0 → gain 1.0
        assert!(trigger_rage(1.0, &fierce, &crowded) > trigger_rage(1.0, &calm, &crowded));
    }

    #[test]
    fn trigger_lust_needs_mate_and_energy() {
        use crate::agent::SPAWN_ENERGY;
        use crate::genome::{Genome, GenomeSlot};
        use crate::sense::{SensorRegister, NO_NEIGHBOR_ID};
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.4);
        let repro_energy = SPAWN_ENERGY * 0.4 * crate::reproduce::REPRO_ENERGY_MULT;
        let with_mate = SensorRegister { nearest_same_id: 3, ..Default::default() };
        let no_mate = SensorRegister { nearest_same_id: NO_NEIGHBOR_ID, ..Default::default() };
        // Ready + mate present → lust.
        assert!(trigger_lust(repro_energy + 1.0, &g, &with_mate) > 0.0);
        // Ready but nobody around → no lust.
        assert_eq!(trigger_lust(repro_energy + 1.0, &g, &no_mate), 0.0);
        // Mate present but below the mating energy gate → no lust.
        assert_eq!(trigger_lust(repro_energy - 1.0, &g, &with_mate), 0.0);
    }

    #[test]
    fn develop_all_raises_rage_when_frustrated_and_lust_when_mate_ready() {
        use crate::agent::SPAWN_ENERGY;
        use crate::genome::{Genome, GenomeSlot};
        use crate::prelude::Vec2;
        use crate::world::World;

        let mut w = World::new(1);
        w.affect_enabled = true;
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral()) as usize;
        // Hungry (high drive) and crowded → frustration; energy above the mating
        // gate with a same-species neighbour present → mate-ready.
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.4);
        w.agents.genome[id] = g;
        w.agents.energy[id] = 0.05 * SPAWN_ENERGY; // deep energy deficit → drive ≈ 1
        w.sensors.resize(w.agents.capacity(), Default::default());
        w.sensors[id].crowding = RAGE_CROWD_REF as u32;
        w.sensors[id].nearest_same_id = 999; // a same-species neighbour exists

        // Mate-ready branch needs energy ≥ gate; run once frustrated (low energy)
        // to check RAGE, then again well-fed to check LUST.
        develop_all(&mut w);
        assert!(w.agents.affect[id][RAGE] > 0.0, "frustrated agent accrues RAGE");

        w.agents.energy[id] = SPAWN_ENERGY; // above the mating gate
        for _ in 0..5 {
            develop_all(&mut w);
        } // let the leaky integrator climb
        assert!(w.agents.affect[id][LUST] > 0.0, "mate-ready agent accrues LUST");
    }

    #[test]
    fn fear_suppresses_rage() {
        // Two identical frustrated agents; the one with high FEAR ends with less RAGE.
        use crate::agent::SPAWN_ENERGY;
        use crate::genome::{Genome, GenomeSlot};
        use crate::prelude::Vec2;
        use crate::world::World;

        let setup = |fear: f32| -> f32 {
            let mut w = World::new(1);
            w.affect_enabled = true;
            let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral()) as usize;
            let mut g = Genome::neutral();
            g.set(GenomeSlot::ReproductionThreshold, 0.4);
            w.agents.genome[id] = g;
            w.agents.energy[id] = 0.05 * SPAWN_ENERGY;
            w.sensors.resize(w.agents.capacity(), Default::default());
            w.sensors[id].crowding = RAGE_CROWD_REF as u32;
            // Preload FEAR before this tick's update so inhibition has something to
            // gate against (M-B's FEAR update this tick blends toward its own trigger;
            // with no threat sensed the trigger is ~0, so the preloaded value decays
            // but stays positive for the high-fear case).
            w.agents.affect[id][FEAR] = fear;
            develop_all(&mut w);
            w.agents.affect[id][RAGE]
        };
        let calm = setup(0.0);
        let afraid = setup(1.0);
        assert!(afraid < calm, "FEAR must suppress RAGE: afraid={afraid} calm={calm}");
    }

    #[test]
    fn apply_affect_rage_raises_fire_and_approaches_target() {
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::program::ActionRegister;
        use crate::sense::SensorRegister;

        let mut a = ActionRegister { fire_intent: 0.1, ..Default::default() };
        let mut affect: AffectState = [0.0; AFFECT_SYSTEMS];
        affect[RAGE] = 0.8;
        let s = SensorRegister {
            nearest_other_id: 5,
            nearest_other_dir: Vec2::new(1.0, 0.0),
            ..Default::default()
        };
        apply_affect(&mut a, &affect, &Genome::neutral(), &s, crate::agent::SPAWN_ENERGY);
        assert!(a.fire_intent > 0.1, "RAGE raises fire_intent: {}", a.fire_intent);
        assert!(a.move_x > 0.0, "RAGE approaches the target: {}", a.move_x);
    }

    #[test]
    fn apply_affect_rage_is_identity_at_neutral() {
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::program::ActionRegister;
        use crate::sense::SensorRegister;

        let mut a = ActionRegister { fire_intent: 0.3, move_x: 0.2, ..Default::default() };
        let before = a;
        let affect: AffectState = [0.0; AFFECT_SYSTEMS]; // RAGE == 0
        let s = SensorRegister {
            nearest_other_id: 5,
            nearest_other_dir: Vec2::new(1.0, 0.0),
            ..Default::default()
        };
        apply_affect(&mut a, &affect, &Genome::neutral(), &s, crate::agent::SPAWN_ENERGY);
        assert_eq!(a.fire_intent, before.fire_intent, "neutral RAGE: fire unchanged");
        assert_eq!(a.move_x, before.move_x, "neutral RAGE: move unchanged");
    }

    #[test]
    fn apply_affect_lust_approaches_same_species() {
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::program::ActionRegister;
        use crate::sense::SensorRegister;

        let mut a = ActionRegister::default();
        let before_mate = a.mate_intent;
        let mut affect: AffectState = [0.0; AFFECT_SYSTEMS];
        affect[LUST] = 0.9;
        let s = SensorRegister {
            nearest_same_id: 4,
            nearest_same_dir: Vec2::new(0.0, 1.0),
            ..Default::default()
        };
        apply_affect(&mut a, &affect, &Genome::neutral(), &s, crate::agent::SPAWN_ENERGY);
        assert!(a.move_y > 0.0, "LUST approaches the mate: {}", a.move_y);
        assert_eq!(a.mate_intent, before_mate, "LUST must not touch latent mate_intent");
    }

    #[test]
    fn apply_affect_lust_is_identity_at_neutral() {
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::program::ActionRegister;
        use crate::sense::SensorRegister;

        let mut a = ActionRegister { move_y: 0.2, ..Default::default() };
        let before = a;
        let affect: AffectState = [0.0; AFFECT_SYSTEMS]; // LUST == 0
        let s = SensorRegister {
            nearest_same_id: 4,
            nearest_same_dir: Vec2::new(0.0, 1.0),
            ..Default::default()
        };
        apply_affect(&mut a, &affect, &Genome::neutral(), &s, crate::agent::SPAWN_ENERGY);
        assert_eq!(a.move_y, before.move_y, "neutral LUST: move unchanged");
    }
}
