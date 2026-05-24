//! M1 hardcoded behavior function.
//!
//! Replaced in M4 by the evolvable behavior program. The function returns a
//! desired-direction unit vector given the agent's genome and current sensor
//! register. Two drives:
//!
//! - **Forage** — when energy is below `reproduction_threshold * SPAWN_ENERGY`,
//!   move toward the best plant in perception.
//! - **Wander** — otherwise drift with low-amplitude correlated noise sampled
//!   from a per-tick uniform draw.

use crate::agent::SPAWN_ENERGY;
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;
use crate::rng::Rng;
use crate::sense::SensorRegister;

/// Choose a desired direction (unit vector) for one agent. Pure function of
/// inputs. The integrate stage scales this direction by the agent's effective
/// Locomotor speed.
///
/// `rng` is used for the wander noise. It is the *world's* RNG passed in by
/// the tick orchestrator; deterministic ordering is preserved by iterating
/// agents in ascending id order in `decide_all`.
pub fn decide(
    genome: &Genome,
    sensor: &SensorRegister,
    energy: f32,
    own_species: u32,
    rng: &mut Rng,
) -> Vec2 {
    let hunger_threshold = SPAWN_ENERGY * genome.get(GenomeSlot::ReproductionThreshold);
    let is_hungry = energy < hunger_threshold;

    // Reproduce threshold is a separate (higher) bar: agents save up surplus
    // energy before mating becomes attractive. Scale by 1.5× the hunger
    // threshold so well-fed agents pursue mates instead of just wandering.
    let mate_ready_threshold = hunger_threshold * 1.5;
    let mate_ready = energy >= mate_ready_threshold
        && sensor.has_neighbor
        && sensor.nearest_neighbor_species == own_species;

    if is_hungry && sensor.plant_direction != Vec2::ZERO {
        sensor.plant_direction
    } else if mate_ready {
        // Head toward the same-species neighbor; reproduction happens in the
        // reproduce stage when proximity drops below the mating range.
        sensor.nearest_neighbor_dir
    } else {
        // Wander: random unit vector.
        let theta = rng.f32_unit() * std::f32::consts::TAU;
        Vec2::new(crate::mathf::cosf(theta), crate::mathf::sinf(theta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_speed_max_yields_zero_velocity() {
        // No longer meaningful: decide() always returns a direction, the
        // speed cap moved to integrate.rs. Replace this test with:
        // wander direction is a unit vector.
        let g = Genome::neutral();
        let s = SensorRegister::default();
        let mut rng = Rng::from_seed(1);
        let v = decide(&g, &s, 0.0, 0, &mut rng);
        assert!(
            (v.length() - 1.0).abs() < 1e-3 || v == Vec2::ZERO,
            "wander direction should be unit-length (got {:?})",
            v
        );
    }

    #[test]
    fn hungry_agent_with_plant_returns_plant_direction() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 1.0);
        let s = SensorRegister { plant_direction: Vec2::new(1.0, 0.0), ..Default::default() };
        let mut rng = Rng::from_seed(1);
        let v = decide(&g, &s, 0.0, 0, &mut rng);
        assert_eq!(v, Vec2::new(1.0, 0.0));
    }

    #[test]
    fn well_fed_agent_wanders() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.0);
        let s = SensorRegister { plant_direction: Vec2::new(1.0, 0.0), ..Default::default() };
        let mut directions = std::collections::HashSet::new();
        for seed in 0..16 {
            let mut rng = Rng::from_seed(seed);
            let v = decide(&g, &s, SPAWN_ENERGY, 0, &mut rng);
            let key = ((v.x * 100.0) as i32, (v.y * 100.0) as i32);
            directions.insert(key);
        }
        assert!(directions.len() >= 4);
    }

    #[test]
    fn mate_ready_agent_heads_toward_same_species_neighbor() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.5);
        let s = SensorRegister {
            plant_direction: Vec2::new(0.0, -1.0),
            has_neighbor: true,
            nearest_neighbor_dist: 5.0,
            nearest_neighbor_dir: Vec2::new(1.0, 0.0),
            nearest_neighbor_species: 0,
            ..Default::default()
        };
        let mut rng = Rng::from_seed(1);
        let v = decide(&g, &s, 50.0, 0, &mut rng);
        assert!(v.x > 0.5);
        assert!(v.y.abs() < 0.5);
    }

    #[test]
    fn mate_ready_with_different_species_does_not_mate_seek() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.5);
        let s = SensorRegister {
            has_neighbor: true,
            nearest_neighbor_dist: 5.0,
            nearest_neighbor_dir: Vec2::new(1.0, 0.0),
            nearest_neighbor_species: 1,
            ..Default::default()
        };
        let mut wandered = std::collections::HashSet::new();
        for seed in 1..16 {
            let mut r = Rng::from_seed(seed);
            let vw = decide(&g, &s, 50.0, 0, &mut r);
            wandered.insert(((vw.x * 10.0) as i32, (vw.y * 10.0) as i32));
        }
        assert!(wandered.len() >= 4);
    }
}
