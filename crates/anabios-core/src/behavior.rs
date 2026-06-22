//! Behavior dispatch — delegates to each agent's evolvable program.
//!
//! Replaces the M1/M2/M3 hardcoded forage/wander/mate function with a
//! per-agent program evaluator (`crate::program::evaluate`). The action
//! register's move vector is normalized to a unit direction; the integrate
//! stage scales it by the agent's effective Locomotor speed.

use crate::genome::Genome;
use crate::prelude::Vec2;
use crate::program::{evaluate, ActionRegister, EvalContext, Program};
use crate::sense::SensorRegister;

/// Choose a desired unit direction for one agent by evaluating its program.
/// Returns `Vec2::ZERO` when the program produces no net movement intent.
pub fn decide(
    program: &Program,
    genome: &Genome,
    sensor: &SensorRegister,
    energy: f32,
    age: u32,
    eval_stack: &mut Vec<f32>,
) -> Vec2 {
    let ctx = EvalContext {
        energy,
        age,
        genome,
        nearest_distance: sensor.nearest_neighbor_dist,
        nearest_dir: sensor.nearest_neighbor_dir,
        plant_dir: sensor.plant_direction,
        local_biomass: sensor.local_plant_biomass,
        same_distance: sensor.nearest_same_dist,
        same_dir: sensor.nearest_same_dir,
        other_distance: sensor.nearest_other_dist,
        other_dir: sensor.nearest_other_dir,
        rel_size: sensor.nearest_rel_size,
        rel_energy: sensor.nearest_rel_energy,
        crowding: sensor.crowding as f32,
    };
    let ActionRegister { move_x, move_y, .. } = evaluate(program, ctx, eval_stack);

    let v = Vec2::new(move_x, move_y);
    let len = v.length();
    if len < 1e-4 {
        Vec2::ZERO
    } else {
        v / len
    }
}
