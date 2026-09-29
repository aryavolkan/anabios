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
/// Per-tick energy cost of active perception, paid when `cognition_enabled`
/// is true. Scales with the agent's effective perception radius relative to
/// the world max: `PERCEPTION_ENERGY_COST * (radius / max_radius)`. This
/// replaces the old flat IQ metabolic surcharge: bigger/brighter sensors
/// literally cost more to run.
pub const PERCEPTION_ENERGY_COST: f32 = 0.005;

/// Maximum agent speed at `Locomotor.max_speed = 1.0`, in world units per
/// tick. Capping here keeps spatial-hash neighbor queries within their
/// `PERCEPTION_MAX_RADIUS` guarantee even when an agent has multiple
/// Locomotor modules (their max_speed contributions sum, then we clamp).
pub const SPEED_MAX_CAP: f32 = 4.0;

/// An agent's top speed this tick, in world units per tick: `SPEED_MAX_CAP`
/// scaled by its summed Locomotor speed (clamped to 1), Openness, the
/// Machinery buff (`inv_mask` is the held-invention mask of its meme vector)
/// and affect (SEEKING). `integrate_all` moves `desired_direction × top_speed`;
/// with gait on the direction's length is the fraction of this actually
/// used, so a viewer or test can read `|velocity| / top_speed` as the gait.
/// The product is evaluated in exactly the order `integrate_all` always used,
/// so factoring it out changed no bit.
#[inline]
pub fn top_speed(
    modules: &crate::module::ModuleList,
    genome: &crate::genome::Genome,
    inv_mask: u32,
    affect: &crate::affect::AffectState,
    gene_tech_coupling: bool,
) -> f32 {
    let module_speed = crate::module::effective_speed_max(modules).clamp(0.0, 1.0);
    // Openness scales effective speed (identity at neutral personality).
    let speed_factor = crate::personality::personality_speed_factor(genome);
    // Affect movement-speed factor (SEEKING + arousal). Exactly 1.0 at
    // neutral affect, so a flag-off world stays byte-identical.
    let affect_speed = crate::affect::affect_speed_factor(affect);
    // Machinery buff: powered locomotion.
    let inv_speed =
        crate::invention::speed_multiplier_coupled(inv_mask, genome, gene_tech_coupling);
    SPEED_MAX_CAP * module_speed * speed_factor * inv_speed * affect_speed
}

/// Apply `desired_direction[i]` to each alive agent, scaled by the agent's
/// effective Locomotor speed. Agents without a Locomotor still pay basal
/// metabolism but do not move. `dimorphism_enabled` (E12) switches on the
/// sex-linked basal-metabolism factor; pass `false` for exact identity.
/// `gene_tech_coupling` (TG1) scales the Machinery speed buff by the holder's
/// affinity gene; pass `false` for exact identity. `cognition_enabled` adds a
/// radius-scaled perception-energy cost (zero and byte-identical when false).
/// `habitat` (territory layer) is `Some(biome)` when `territory_enabled`:
/// each move passes through `habitat::gate_move` for the agent's Locomotion
/// class; `None` applies the move ungated (exact identity). `gait_enabled`
/// (`gait.rs`) reads the speed fraction `decide_all` folded into the
/// direction's length and multiplies the move cost by
/// `gait::move_cost_factor(frac)`; pass `false` for exact identity (the
/// direction is then a unit vector and the factor is never evaluated).
/// `growth_enabled` (`growth.rs`) scales basal metabolism and the move cost
/// by the agent's growth body scale and its speed by `growth::speed_scale`
/// (juveniles are smaller, cheaper and slightly slower); pass `false` for
/// exact identity (every multiplier is then exactly 1.0).
#[allow(clippy::too_many_arguments)]
pub fn integrate_all(
    agents: &mut AgentBuffers,
    desired_direction: &[Vec2],
    world_size: f32,
    dimorphism_enabled: bool,
    gene_tech_coupling: bool,
    cognition_enabled: bool,
    max_radius: f32,
    habitat: Option<&crate::biome::BiomeField>,
    gait_enabled: bool,
    growth_enabled: bool,
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
        age,
        alive,
        ..
    } = agents;
    let (modules, genome, meme_vector, iq, affect, thirst, asleep, sex, age, alive) = (
        &*modules,
        &*genome,
        &*meme_vector,
        &*iq,
        &*affect,
        &*thirst,
        &*asleep,
        &*sex,
        &*age,
        &*alive,
    );
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
            // Growth body scale: a juvenile's basal metabolism and move cost
            // scale with its body and it moves slightly slower
            // (`growth::speed_scale`). Exactly 1.0 with the flag off (and for
            // every grown agent), so flag-off worlds stay byte-identical.
            let growth = crate::growth::body_scale_of(growth_enabled, age[i], &genome[i]);
            // Cognition-driven perception cost: scales with the agent's actual
            // sensory radius (sensor + IQ + invention buffs). Zero when
            // cognition is off, so non-cognition worlds stay byte-identical.
            let perception_cost = if cognition_enabled && max_radius > 0.0 {
                let base_radius = crate::sense::perception_radius(
                    &modules[i],
                    &genome[i],
                    iq[i],
                    max_radius,
                    true,
                );
                let radius = base_radius
                    * crate::invention::perception_multiplier_coupled(
                        inv_mask,
                        &genome[i],
                        gene_tech_coupling,
                    );
                PERCEPTION_ENERGY_COST * (radius.min(max_radius) / max_radius)
            } else {
                0.0
            };

            // Asleep (basic needs): movement fully suppressed (no move cost),
            // basal metabolism discounted. `asleep` is all-false when the flag
            // is off, so this branch never runs there.
            if asleep[i] {
                *vel = Vec2::ZERO;
                let basal = BASAL_METABOLISM_COST
                    * genome[i].get(GenomeSlot::BasalMetabolism)
                    * crate::invention::metabolism_multiplier(inv_mask)
                    * dimorph_basal
                    * needs_basal
                    * crate::needs::SLEEP_METABOLISM_FACTOR
                    * growth;
                *en -= basal + perception_cost;
                return;
            }

            // Action gating: no Locomotor → no motion.
            if !crate::module::has(&modules[i], crate::module::ModuleType::Locomotor) {
                *vel = Vec2::ZERO;
                // Still pay basal metabolism (invention debuffs scale it).
                let basal = BASAL_METABOLISM_COST
                    * genome[i].get(GenomeSlot::BasalMetabolism)
                    * crate::invention::metabolism_multiplier(inv_mask)
                    * dimorph_basal
                    * needs_basal
                    * growth;
                *en -= basal + perception_cost;
                return;
            }

            // With gait on, `direction` is the unit heading times the speed
            // fraction `decide_all` chose (`gait.rs`); off, a unit vector.
            let direction = desired_direction[i];
            let v = direction
                * (top_speed(&modules[i], &genome[i], inv_mask, &affect[i], gene_tech_coupling)
                    * crate::growth::speed_scale(growth));
            // Habitat gate (territory layer): the move the agent's Locomotion
            // class allows — full, coastline slide, or none. `None` ⇒ `v`
            // untouched, so flag-off worlds are byte-identical.
            let v = match habitat {
                Some(biome) => crate::habitat::gate_move(
                    biome,
                    crate::habitat::Locomotion::of(&genome[i]),
                    *pos,
                    v,
                ),
                None => v,
            };
            *vel = v;

            let new_pos = *pos + v;
            *pos = wrap_torus(new_pos, Vec2::splat(world_size));

            let move_dist = v.length();
            let size = genome[i].get(GenomeSlot::Size).max(0.1);
            let mut move_cost = MOVE_ENERGY_COST * move_dist * size * growth;
            if gait_enabled {
                // Gait: sprinting is superlinearly expensive per unit distance
                // (`gait::GAIT_SPRINT_COST`). `|direction|` is the speed
                // fraction decide_all folded in. Flag off ⇒ never evaluated,
                // so the move cost is byte-identical.
                move_cost *= crate::gait::move_cost_factor(direction.length());
            }
            let basal = BASAL_METABOLISM_COST
                * genome[i].get(GenomeSlot::BasalMetabolism)
                * crate::invention::metabolism_multiplier(inv_mask)
                * dimorph_basal
                * needs_basal
                * growth;
            *en -= move_cost + basal + perception_cost;
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::SPAWN_ENERGY;
    use crate::genome::Genome;
    use crate::world::World;

    /// Spawn one agent with its locomotor pinned to `max_speed = 1.0`, so a
    /// movement test's step length is exactly the desired vector.
    fn spawn_at_unit_speed(w: &mut World, pos: Vec2) -> u32 {
        let id = w.spawn_agent(pos, Genome::neutral());
        for m in w.agents.modules[id as usize].iter_mut() {
            if let crate::module::Module::Locomotor { max_speed, .. } = m {
                *max_speed = 1.0;
            }
        }
        id
    }

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
        integrate_all(
            &mut w.agents,
            &desired,
            w.world_size,
            false,
            false,
            w.cognition_enabled,
            w.spatial.perception_max_radius(),
            None,
            false,
            false,
        );
        let p = w.agents.position[id as usize];
        assert!(p.x >= 0.0 && p.x < WORLD_SIZE);
        assert!((p.x - 3.0).abs() < 1e-3, "expected wrap-around to ~3.0, got {}", p.x);
    }

    #[test]
    fn motion_drains_energy_proportionally() {
        let mut w = World::new(1);
        let id = spawn_at_unit_speed(&mut w, Vec2::new(500.0, 500.0));
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        let before = w.agents.energy[id as usize];
        integrate_all(
            &mut w.agents,
            &desired,
            w.world_size,
            false,
            false,
            w.cognition_enabled,
            w.spatial.perception_max_radius(),
            None,
            false,
            false,
        );
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
        integrate_all(
            &mut w.agents,
            &desired,
            w.world_size,
            false,
            false,
            w.cognition_enabled,
            w.spatial.perception_max_radius(),
            None,
            false,
            false,
        );
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
            integrate_all(
                &mut w.agents,
                &desired,
                w.world_size,
                enabled,
                false,
                w.cognition_enabled,
                w.spatial.perception_max_radius(),
                None,
                false,
                false,
            );
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
    fn perception_radius_cost_scales_with_iq() {
        // With cognition on, the energy drain of perception scales with the
        // agent's effective sensory radius, which is driven by IQ. A high-IQ
        // agent with the same sensor pays strictly more per tick than a low-IQ
        // one; with cognition off the cost disappears.
        let drain = |iq: f32, cognition_on: bool| -> f32 {
            let mut w = World::new(1);
            w.cognition_enabled = cognition_on;
            let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
            w.agents.iq[id as usize] = iq;
            let desired = vec![Vec2::ZERO; w.agents.capacity()];
            let before = w.agents.energy[id as usize];
            integrate_all(
                &mut w.agents,
                &desired,
                w.world_size,
                false,
                false,
                w.cognition_enabled,
                w.spatial.perception_max_radius(),
                None,
                false,
                false,
            );
            before - w.agents.energy[id as usize]
        };

        let off_low = drain(0.0, false);
        let off_high = drain(1.0, false);
        assert_eq!(off_low, off_high, "cognition off: IQ must not affect energy drain");

        let on_low = drain(0.0, true);
        let on_high = drain(1.0, true);
        assert!(
            on_high > on_low,
            "cognition on: high-IQ (wider perception) must cost more: low={on_low} high={on_high}"
        );

        // Sensor radius in the starter kit is 0.6. At iq=0 the radius fraction
        // is 0.6 * 0.25 = 0.15; at iq=1 it is 0.6 * 1.0 = 0.60.
        let delta_low = PERCEPTION_ENERGY_COST * 0.15;
        let delta_high = PERCEPTION_ENERGY_COST * 0.60;
        assert!(
            (on_low - off_low - delta_low).abs() < 1e-4,
            "low-IQ perception cost mismatch: got {} expected ~{delta_low}",
            on_low - off_low
        );
        assert!(
            (on_high - off_high - delta_high).abs() < 1e-4,
            "high-IQ perception cost mismatch: got {} expected ~{delta_high}",
            on_high - off_high
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
        integrate_all(
            &mut w.agents,
            &desired,
            w.world_size,
            false,
            false,
            w.cognition_enabled,
            w.spatial.perception_max_radius(),
            None,
            false,
            false,
        );
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
            integrate_all(
                &mut w.agents,
                &desired,
                w.world_size,
                false,
                false,
                w.cognition_enabled,
                w.spatial.perception_max_radius(),
                None,
                false,
                false,
            );
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
            let id = spawn_at_unit_speed(&mut w, Vec2::new(500.0, 500.0));
            w.agents.asleep.set(id as usize, asleep);
            let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
            desired[id as usize] = Vec2::new(1.0, 0.0);
            let before_pos = w.agents.position[id as usize];
            let before_en = w.agents.energy[id as usize];
            integrate_all(
                &mut w.agents,
                &desired,
                w.world_size,
                false,
                false,
                w.cognition_enabled,
                w.spatial.perception_max_radius(),
                None,
                false,
                false,
            );
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
            let id = spawn_at_unit_speed(&mut w, Vec2::new(500.0, 500.0));
            w.agents.affect[id as usize][crate::affect::SEEK] = seek;
            let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
            desired[id as usize] = Vec2::new(1.0, 0.0);
            let before = w.agents.position[id as usize];
            integrate_all(
                &mut w.agents,
                &desired,
                w.world_size,
                false,
                false,
                w.cognition_enabled,
                w.spatial.perception_max_radius(),
                None,
                false,
                false,
            );
            (w.agents.position[id as usize] - before).length()
        };
        let neutral = displacement(0.0);
        let seeking = displacement(1.0);
        assert!(seeking > neutral, "SEEKING boosts movement speed: {neutral} -> {seeking}");
    }

    /// 256-unit world, all Grass except Water in columns >= 16 (coast at x=128).
    fn coast_world() -> World {
        let mut w = World::with_dims(1, 256.0, 32, 16);
        let res = w.biome.res;
        for row in 0..res {
            for col in 0..res {
                w.biome.at_mut(col, row).terrain = if col >= 16 {
                    crate::biome::TerrainType::Water
                } else {
                    crate::biome::TerrainType::Grass
                };
            }
        }
        w
    }

    fn step_once(w: &mut World, id: u32, dir: Vec2) -> Vec2 {
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = dir;
        let biome = w.biome.clone();
        integrate_all(
            &mut w.agents,
            &desired,
            w.world_size,
            false,
            false,
            w.cognition_enabled,
            w.spatial.perception_max_radius(),
            Some(&biome),
            false,
            false,
        );
        w.agents.position[id as usize]
    }

    #[test]
    fn habitat_gate_keeps_land_agents_out_of_water() {
        let mut w = coast_world();
        let id = spawn_at_unit_speed(&mut w, Vec2::new(126.0, 100.0));
        let p = step_once(&mut w, id, Vec2::new(1.0, 0.0));
        assert_eq!(p, Vec2::new(126.0, 100.0), "blocked at the shore");
        let p = step_once(&mut w, id, Vec2::new(0.70710677, 0.70710677));
        assert!((p.x - 126.0).abs() < 1e-4 && p.y > 100.0, "slides north along the coast: {p:?}");
    }

    #[test]
    fn habitat_gate_keeps_water_agents_in_water_and_lets_air_cross() {
        let mut w = coast_world();
        let mut g = crate::genome::Genome::neutral();
        g.set(crate::genome::GenomeSlot::Locomotion, 0.1);
        let fish = spawn_at_unit_speed(&mut w, Vec2::new(130.0, 60.0));
        w.agents.genome[fish as usize] = g;
        assert_eq!(step_once(&mut w, fish, Vec2::new(-1.0, 0.0)), Vec2::new(130.0, 60.0));
        let bird = spawn_at_unit_speed(&mut w, Vec2::new(126.0, 30.0));
        w.agents.genome[bird as usize].set(crate::genome::GenomeSlot::Locomotion, 0.9);
        let p = step_once(&mut w, bird, Vec2::new(1.0, 0.0));
        assert!((p.x - 130.0).abs() < 1e-4, "air crosses the coast: {p:?}");
    }

    #[test]
    fn habitat_none_is_the_ungated_move() {
        let mut w = coast_world();
        let id = spawn_at_unit_speed(&mut w, Vec2::new(126.0, 100.0));
        let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
        desired[id as usize] = Vec2::new(1.0, 0.0);
        integrate_all(
            &mut w.agents,
            &desired,
            w.world_size,
            false,
            false,
            w.cognition_enabled,
            w.spatial.perception_max_radius(),
            None,
            false,
            false,
        );
        assert!((w.agents.position[id as usize].x - 130.0).abs() < 1e-4);
    }

    #[test]
    fn gait_sprint_costs_more_per_unit_distance_than_a_walk() {
        use crate::gait::{move_cost_factor, GAIT_WALK};
        // Energy per unit distance for a direction of length `frac` (the
        // gait fraction) — the drain beyond basal over the distance covered.
        let per_unit = |frac: f32, gait_on: bool| -> f32 {
            let mut w = World::new(1);
            let id = spawn_at_unit_speed(&mut w, Vec2::new(500.0, 500.0));
            let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
            desired[id as usize] = Vec2::new(frac, 0.0);
            let before_pos = w.agents.position[id as usize];
            let before = w.agents.energy[id as usize];
            integrate_all(
                &mut w.agents,
                &desired,
                w.world_size,
                false,
                false,
                w.cognition_enabled,
                w.spatial.perception_max_radius(),
                None,
                gait_on,
                false,
            );
            let dist = (w.agents.position[id as usize] - before_pos).length();
            assert!((dist - frac * SPEED_MAX_CAP).abs() < 1e-4, "step {dist} at fraction {frac}");
            let basal = BASAL_METABOLISM_COST * 0.5; // neutral genome
            (before - w.agents.energy[id as usize] - basal) / dist
        };
        let sprint = per_unit(1.0, true);
        let walk = per_unit(GAIT_WALK, true);
        assert!(sprint > walk, "sprint {sprint} must cost more per unit distance than walk {walk}");
        // Exactly the documented surcharge: MOVE_ENERGY_COST × size × (1 + K·frac²).
        let expect = |f: f32| MOVE_ENERGY_COST * 0.5 * move_cost_factor(f);
        assert!((sprint - expect(1.0)).abs() < 2e-5, "sprint {sprint} vs {}", expect(1.0));
        assert!((walk - expect(GAIT_WALK)).abs() < 2e-5, "walk {walk} vs {}", expect(GAIT_WALK));
        // Flag off: the plain linear cost, no surcharge.
        let off = per_unit(1.0, false);
        assert!((off - MOVE_ENERGY_COST * 0.5).abs() < 2e-5, "flag off {off}");
    }

    /// Growth: a newborn pays `JUVENILE_BODY` of an adult's basal + move cost
    /// and moves at `speed_scale(JUVENILE_BODY)` of its speed; halfway to
    /// maturity it sits strictly between; from maturity on (and with the flag
    /// off, whatever the age) the arithmetic is exactly the adult's.
    #[test]
    fn juveniles_cost_less_and_move_slower_and_grow_into_adults() {
        use crate::growth::{maturity_ticks, speed_scale, JUVENILE_BODY};
        // (energy drained, distance moved) by one unit-speed agent in one tick.
        let run = |growth_on: bool, age: u32| -> (f32, f32) {
            let mut w = World::new(1);
            let id = spawn_at_unit_speed(&mut w, Vec2::new(500.0, 500.0));
            w.agents.age[id as usize] = age;
            let mut desired = vec![Vec2::ZERO; w.agents.capacity()];
            desired[id as usize] = Vec2::new(1.0, 0.0);
            let before_pos = w.agents.position[id as usize];
            let before_en = w.agents.energy[id as usize];
            integrate_all(
                &mut w.agents,
                &desired,
                w.world_size,
                false,
                false,
                w.cognition_enabled,
                w.spatial.perception_max_radius(),
                None,
                false,
                growth_on,
            );
            (
                before_en - w.agents.energy[id as usize],
                (w.agents.position[id as usize] - before_pos).length(),
            )
        };
        let m = maturity_ticks(crate::age::lifespan_of(&Genome::neutral()));
        let (adult_drain, adult_move) = run(false, 0);
        assert_eq!(run(false, m), (adult_drain, adult_move), "flag off: age unread");
        assert_eq!(run(true, m), (adult_drain, adult_move), "mature: exactly the adult");
        assert_eq!(run(true, m + 500), (adult_drain, adult_move));

        let (baby_drain, baby_move) = run(true, 0);
        let expected_move = adult_move * speed_scale(JUVENILE_BODY);
        assert!((baby_move - expected_move).abs() < 1e-3, "{baby_move} vs {expected_move}");
        // Neutral genome: Size 0.5, BasalMetabolism 0.5 — both costs scale
        // with the newborn body (the move cost also through the shorter step).
        let expected_drain =
            (BASAL_METABOLISM_COST * 0.5 + MOVE_ENERGY_COST * baby_move * 0.5) * JUVENILE_BODY;
        assert!((baby_drain - expected_drain).abs() < 1e-4, "{baby_drain} vs {expected_drain}");
        assert!(baby_drain < adult_drain);

        let (mid_drain, mid_move) = run(true, m / 2);
        assert!(
            baby_drain < mid_drain && mid_drain < adult_drain,
            "{baby_drain} {mid_drain} {adult_drain}"
        );
        assert!(
            baby_move < mid_move && mid_move < adult_move,
            "{baby_move} {mid_move} {adult_move}"
        );
    }
}
