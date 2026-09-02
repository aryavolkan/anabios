//! Integration step: applies desired directions to positions, wraps to the
//! torus, and drains energy proportional to movement plus a per-tick basal
//! metabolism cost.

use crate::agent::AgentBuffers;
#[cfg(test)]
use crate::biome::WORLD_SIZE;
use crate::genome::GenomeSlot;
use crate::prelude::{wrap_torus, Vec2};

/// Cost per world-unit of movement at `Size = 1.0`. Smaller agents pay less.
pub const MOVE_ENERGY_COST: f32 = 0.005;
/// Per-tick basal metabolism cost at `BasalMetabolism = 1.0`.
pub const BASAL_METABOLISM_COST: f32 = 0.05;

/// Maximum agent speed at `Locomotor.max_speed = 1.0`, in world units per
/// tick. Capping here keeps spatial-hash neighbor queries within their
/// `PERCEPTION_MAX_RADIUS` guarantee even when an agent has multiple
/// Locomotor modules (their max_speed contributions sum, then we clamp).
pub const SPEED_MAX_CAP: f32 = 4.0;

/// Apply `desired_direction[i]` to each alive agent, scaled by the agent's
/// effective Locomotor speed. Agents without a Locomotor still pay basal
/// metabolism but do not move. `dimorphism_enabled` (E12) switches on the
/// sex-linked basal-metabolism factor; pass `false` for exact identity.
/// `gene_tech_coupling` (TG1) scales the Machinery speed buff by the holder's
/// affinity gene; pass `false` for exact identity.
pub fn integrate_all(
    agents: &mut AgentBuffers,
    desired_direction: &[Vec2],
    world_size: f32,
    dimorphism_enabled: bool,
    gene_tech_coupling: bool,
) {
    use rayon::prelude::*;
    let cap = agents.capacity();
    // Each agent's motion + metabolism is a pure function of its OWN
    // modules/genome/meme/desired_direction, writing only its own
    // position/velocity/energy — index-disjoint, no RNG, no cross-agent read.
    // So the parallel loop is bit-identical to the old serial ascending-id
    // loop (same argument as `sense_all`/`decide_all`). Disjoint field borrows
    // give the mutated columns `&mut` while the read columns stay shared `&`.
    let AgentBuffers {
        position,
        velocity,
        energy,
        modules,
        genome,
        meme_vector,
        iq,
        affect,
        thirst,
        asleep,
        sex,
        alive,
        ..
    } = agents;
    let (modules, genome, meme_vector, iq, affect, thirst, asleep, sex, alive) =
        (&*modules, &*genome, &*meme_vector, &*iq, &*affect, &*thirst, &*asleep, &*sex, &*alive);
    position[..cap]
        .par_iter_mut()
        .zip(velocity[..cap].par_iter_mut())
        .zip(energy[..cap].par_iter_mut())
        .enumerate()
        .for_each(|(i, ((pos, vel), en))| {
            if !alive[i] {
                return;
            }
            let dimorph_basal =
                crate::dimorphism::metabolism_factor(&genome[i], sex[i], dimorphism_enabled);
            // Held-invention bitmask: a pure function of this agent's meme
            // vector, read by both the speed buff and the metabolism debuff
            // below (and on both the moving and non-moving paths). Compute it
            // once — it was previously recomputed per call site.
            let inv_mask = crate::invention::held_mask(&meme_vector[i]);
            // Basic-needs dehydration factor on basal drain. Exactly 1.0 at
            // thirst 0.0 (the flag-off state), so disabled worlds stay
            // byte-identical — the same identity contract as `affect_speed`.
            let needs_basal = crate::needs::dehydration_metabolism_multiplier(thirst[i]);

            // Asleep (basic needs): movement fully suppressed (no move cost),
            // basal metabolism discounted. `asleep` is all-false when the flag
            // is off, so this branch never runs there.
            if asleep[i] {
                *vel = Vec2::ZERO;
                let basal = BASAL_METABOLISM_COST
                    * genome[i].get(GenomeSlot::BasalMetabolism)
                    * crate::invention::metabolism_multiplier(inv_mask)
                    * crate::iq::metabolism_multiplier(iq[i])
                    * dimorph_basal
                    * needs_basal
                    * crate::needs::SLEEP_METABOLISM_FACTOR;
                *en -= basal;
                return;
            }

            // Action gating: no Locomotor → no motion.
            if !crate::module::has(&modules[i], crate::module::ModuleType::Locomotor) {
                *vel = Vec2::ZERO;
                // Still pay basal metabolism (invention debuffs + IQ scale it).
                let basal = BASAL_METABOLISM_COST
                    * genome[i].get(GenomeSlot::BasalMetabolism)
                    * crate::invention::metabolism_multiplier(inv_mask)
                    * crate::iq::metabolism_multiplier(iq[i])
                    * dimorph_basal
                    * needs_basal;
                *en -= basal;
                return;
            }

            let direction = desired_direction[i];
            let module_speed = crate::module::effective_speed_max(&modules[i]).clamp(0.0, 1.0);
            // Openness scales effective speed (identity at neutral personality).
            let speed_factor = crate::personality::personality_speed_factor(&genome[i]);
            // Affect movement-speed factor (SEEKING + arousal). Exactly 1.0 at
            // neutral affect, so a flag-off world stays byte-identical.
            let affect_speed = crate::affect::affect_speed_factor(&affect[i]);
            // Machinery buff: powered locomotion.
            let inv_speed = crate::invention::speed_multiplier_coupled(
                inv_mask,
                &genome[i],
                gene_tech_coupling,
            );
            let v = direction
                * (SPEED_MAX_CAP * module_speed * speed_factor * inv_speed * affect_speed);
            *vel = v;

            let new_pos = *pos + v;
            *pos = wrap_torus(new_pos, Vec2::splat(world_size));

            let move_dist = v.length();
            let size = genome[i].get(GenomeSlot::Size).max(0.1);
            let move_cost = MOVE_ENERGY_COST * move_dist * size;
            let basal = BASAL_METABOLISM_COST
                * genome[i].get(GenomeSlot::BasalMetabolism)
                * crate::invention::metabolism_multiplier(inv_mask)
                * crate::iq::metabolism_multiplier(iq[i])
                * dimorph_basal
                * needs_basal;
            *en -= move_cost + basal;
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::SPAWN_ENERGY;
    use crate::genome::Genome;
    use crate::world::World;

    #[test]
    fn position_wraps_on_torus() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(WORLD_SIZE - 1.0, 0.5), Genome::neutral());
        // Force max-speed Locomotor so the unit direction produces a 4-unit step.
        for m in w.agents.modules[id as usize].iter_mut() {
            if let crate::module::Module::Locomotor { max_speed, .. } = m {
                *max_speed = 1.0;
            }
        }
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        // Move 3 ticks worth in one call by scaling the direction? No — direction
        // must be unit. Instead place agent close enough that one 4-unit step wraps.
        // WORLD_SIZE - 1.0 + 4.0 = WORLD_SIZE + 3.0 → wraps to 3.0.
        integrate_all(&mut w.agents, &desired, w.world_size, false, false);
        let p = w.agents.position[id as usize];
        assert!(p.x >= 0.0 && p.x < WORLD_SIZE);
        assert!((p.x - 3.0).abs() < 1e-3, "expected wrap-around to ~3.0, got {}", p.x);
    }

    #[test]
    fn motion_drains_energy_proportionally() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        for m in w.agents.modules[id as usize].iter_mut() {
            if let crate::module::Module::Locomotor { max_speed, .. } = m {
                *max_speed = 1.0;
            }
        }
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        let before = w.agents.energy[id as usize];
        integrate_all(&mut w.agents, &desired, w.world_size, false, false);
        let after = w.agents.energy[id as usize];
        assert!(after < before);
        // Speed is now SPEED_MAX_CAP * 1.0 = 4.0 units per tick.
        let expected_move_cost = MOVE_ENERGY_COST * 4.0 * 0.5; // size = 0.5 in neutral genome
        let expected_basal = BASAL_METABOLISM_COST * 0.5;
        let drained = before - after;
        assert!(
            (drained - (expected_move_cost + expected_basal)).abs() < 1e-3,
            "drained={drained}, expected~{}",
            expected_move_cost + expected_basal
        );
        // Sanity: still alive with non-zero energy.
        assert!(after < SPAWN_ENERGY);
    }

    #[test]
    fn agent_without_locomotor_does_not_move() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        // Strip Locomotor from the starter kit.
        w.agents.modules[id as usize]
            .retain(|m| !matches!(m, crate::module::Module::Locomotor { .. }));

        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        let pos_before = w.agents.position[id as usize];
        integrate_all(&mut w.agents, &desired, w.world_size, false, false);
        let pos_after = w.agents.position[id as usize];
        assert_eq!(pos_before, pos_after, "no Locomotor → no motion");
    }

    #[test]
    fn dimorphism_shifts_basal_metabolism_by_sex() {
        let drain = |male: bool, enabled: bool| -> f32 {
            let mut w = World::new(1);
            let mut g = Genome::neutral();
            g.set(GenomeSlot::SexualDimorphism, 1.0);
            let id = w.spawn_agent(Vec2::new(500.0, 500.0), g);
            w.agents.sex.set(id as usize, male);
            let desired = vec![Vec2::ZERO; w.agents.capacity()];
            let before = w.agents.energy[id as usize];
            integrate_all(&mut w.agents, &desired, w.world_size, enabled, false);
            before - w.agents.energy[id as usize]
        };
        let plain = drain(false, false);
        let female = drain(false, true);
        let male = drain(true, true);
        // d = 1: females pay ×(1 − 0.20), males ×(1 + 0.30) of neutral basal.
        assert!((female - plain * 0.8).abs() < 1e-5, "female discount: {plain} -> {female}");
        assert!((male - plain * 1.3).abs() < 1e-5, "male surcharge: {plain} -> {male}");
    }

    #[test]
    fn iq_raises_basal_metabolism() {
        // Realized IQ costs basal metabolism (brains are expensive) — the tradeoff
        // that keeps IQ from freely maxing out. A high-IQ agent must pay strictly
        // more basal drain than an otherwise-identical low-IQ one, scaled by
        // metabolism_multiplier(iq). No movement → basal is the only drain.
        let drain = |iq: f32| -> f32 {
            let mut w = World::new(1);
            let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
            w.agents.iq[id as usize] = iq;
            let desired = vec![Vec2::ZERO; w.agents.capacity()];
            let before = w.agents.energy[id as usize];
            integrate_all(&mut w.agents, &desired, w.world_size, false, false);
            before - w.agents.energy[id as usize]
        };
        let low = drain(0.0);
        let high = drain(1.0);
        assert!(high > low, "high IQ must cost more basal metabolism: low={low} high={high}");
        // iq == 0 is the identity multiplier; iq == 1 scales basal by
        // (1 + IQ_METABOLIC_COST) — integrate applies iq::metabolism_multiplier,
        // so the drain ratio matches it (tolerance for chained-float rounding).
        let ratio = high / low;
        assert!(
            (ratio - (1.0 + crate::iq::IQ_METABOLIC_COST)).abs() < 1e-3,
            "iq=1 basal ≈ neutral × (1 + IQ_METABOLIC_COST): ratio={ratio} (low={low} high={high})"
        );
    }

    #[test]
    fn agent_with_locomotor_moves_proportionally_to_speed_param() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        // Replace starter kit Locomotor with a max-speed one.
        for m in w.agents.modules[id as usize].iter_mut() {
            if let crate::module::Module::Locomotor { max_speed, .. } = m {
                *max_speed = 1.0;
            }
        }

        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        integrate_all(&mut w.agents, &desired, w.world_size, false, false);
        let new_pos = w.agents.position[id as usize];
        // Moved roughly SPEED_MAX_CAP × 1.0 = 4.0 in +x.
        assert!((new_pos.x - 504.0).abs() < 0.1);
    }

    #[test]
    fn dehydration_raises_basal_drain() {
        // Basic-needs hook: thirst multiplies basal metabolism (never adds
        // energy), so dehydration kills via the existing starvation path.
        let drain = |thirst: f32| -> f32 {
            let mut w = World::new(1);
            let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
            w.agents.thirst[id as usize] = thirst;
            let desired = vec![Vec2::ZERO; w.agents.capacity()];
            let before = w.agents.energy[id as usize];
            integrate_all(&mut w.agents, &desired, w.world_size, false, false);
            before - w.agents.energy[id as usize]
        };
        let hydrated = drain(0.0);
        let parched = drain(1.0);
        let ratio = parched / hydrated;
        assert!(
            (ratio - crate::needs::dehydration_metabolism_multiplier(1.0)).abs() < 1e-3,
            "thirst=1 basal ≈ neutral × (1 + DEHYDRATION_DRAIN): ratio={ratio}"
        );
    }

    #[test]
    fn asleep_suppresses_movement_and_discounts_basal() {
        let run = |asleep: bool| -> (f32, f32) {
            let mut w = World::new(1);
            let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
            for m in w.agents.modules[id as usize].iter_mut() {
                if let crate::module::Module::Locomotor { max_speed, .. } = m {
                    *max_speed = 1.0;
                }
            }
            w.agents.asleep.set(id as usize, asleep);
            let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
            desired[id as usize] = Vec2::new(1.0, 0.0);
            let before_pos = w.agents.position[id as usize];
            let before_en = w.agents.energy[id as usize];
            integrate_all(&mut w.agents, &desired, w.world_size, false, false);
            (
                (w.agents.position[id as usize] - before_pos).length(),
                before_en - w.agents.energy[id as usize],
            )
        };
        let (moved_awake, drain_awake) = run(false);
        let (moved_asleep, drain_asleep) = run(true);
        assert!(moved_awake > 3.0, "awake agent moves");
        assert_eq!(moved_asleep, 0.0, "asleep agent does not move");
        assert!(drain_asleep < drain_awake, "sleep is metabolically cheaper");
        // No move cost + discounted basal: exactly basal × SLEEP_METABOLISM_FACTOR.
        let expected = BASAL_METABOLISM_COST * 0.5 * crate::needs::SLEEP_METABOLISM_FACTOR;
        assert!(
            (drain_asleep - expected).abs() < 1e-5,
            "asleep drain = discounted basal only: {drain_asleep} vs {expected}"
        );
    }

    #[test]
    fn seeking_raises_effective_speed() {
        // Two identical max-speed agents; the one with a SEEKING activation must
        // travel farther under the same unit direction.
        let displacement = |seek: f32| -> f32 {
            let mut w = World::new(1);
            let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
            for m in w.agents.modules[id as usize].iter_mut() {
                if let crate::module::Module::Locomotor { max_speed, .. } = m {
                    *max_speed = 1.0;
                }
            }
            w.agents.affect[id as usize][crate::affect::SEEK] = seek;
            let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
            desired[id as usize] = Vec2::new(1.0, 0.0);
            let before = w.agents.position[id as usize];
            integrate_all(&mut w.agents, &desired, w.world_size, false, false);
            (w.agents.position[id as usize] - before).length()
        };
        let neutral = displacement(0.0);
        let seeking = displacement(1.0);
        assert!(seeking > neutral, "SEEKING boosts movement speed: {neutral} -> {seeking}");
    }
}
