//! M1 hardcoded behavior function.
//!
//! Replaced in M4 by the evolvable behavior program. The function returns a
//! desired-velocity vector given the agent's genome and current sensor
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

/// Maximum agent speed at `SpeedMax = 1.0`. In world units per tick.
pub const SPEED_MAX_CAP: f32 = 4.0;

/// Choose a desired velocity for one agent. Pure function of inputs.
///
/// `rng` is used for the wander noise. It is the *world's* RNG passed in by
/// the tick orchestrator; deterministic ordering is preserved by iterating
/// agents in ascending id order in `decide_all`.
pub fn decide(genome: &Genome, sensor: &SensorRegister, energy: f32, rng: &mut Rng) -> Vec2 {
    let speed_max = SPEED_MAX_CAP * genome.get(GenomeSlot::SpeedMax);
    if speed_max <= 0.0 {
        return Vec2::ZERO;
    }

    let hunger_threshold = SPAWN_ENERGY * genome.get(GenomeSlot::ReproductionThreshold);
    let is_hungry = energy < hunger_threshold;

    let direction = if is_hungry && sensor.plant_direction != Vec2::ZERO {
        sensor.plant_direction
    } else {
        // Wander: random unit vector blended with previous direction. We
        // don't have access to previous direction here without making the
        // sensor register stateful, so use a fresh random unit each tick;
        // the tick rate makes this look correlated enough at small dt.
        let theta = rng.f32_unit() * std::f32::consts::TAU;
        Vec2::new(theta.cos(), theta.sin())
    };

    direction * speed_max
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neutral_with(slot: GenomeSlot, v: f32) -> Genome {
        let mut g = Genome::neutral();
        g.set(slot, v);
        g
    }

    #[test]
    fn zero_speed_max_yields_zero_velocity() {
        let g = neutral_with(GenomeSlot::SpeedMax, 0.0);
        let s = SensorRegister::default();
        let mut rng = Rng::from_seed(1);
        let v = decide(&g, &s, 0.0, &mut rng);
        assert_eq!(v, Vec2::ZERO);
    }

    #[test]
    fn hungry_agent_with_plant_moves_toward_plant() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 1.0);
        g.set(GenomeSlot::ReproductionThreshold, 1.0); // always "hungry"
        let s = SensorRegister { plant_direction: Vec2::new(1.0, 0.0), ..Default::default() };
        let mut rng = Rng::from_seed(1);
        let v = decide(&g, &s, 0.0, &mut rng);
        assert!(v.x > 0.0);
        assert!((v.length() - SPEED_MAX_CAP).abs() < 1e-3);
    }

    #[test]
    fn well_fed_agent_wanders() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 1.0);
        g.set(GenomeSlot::ReproductionThreshold, 0.0); // never hungry
        let s = SensorRegister { plant_direction: Vec2::new(1.0, 0.0), ..Default::default() };
        // Even when a plant is in the sensor, a fed agent shouldn't be locked
        // onto +x; multiple draws should produce varying directions.
        let mut directions = std::collections::HashSet::new();
        for seed in 0..16 {
            let mut rng = Rng::from_seed(seed);
            let v = decide(&g, &s, SPAWN_ENERGY, &mut rng);
            let key = ((v.x * 100.0) as i32, (v.y * 100.0) as i32);
            directions.insert(key);
        }
        assert!(directions.len() >= 4, "wander should produce varied directions: {:?}", directions);
    }
}
