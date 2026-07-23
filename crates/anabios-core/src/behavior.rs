//! Behavior dispatch — delegates to each agent's evolvable program.
//!
//! `decide()` evaluates one agent's program and returns its full action
//! register, including a sensor-derived `target_id` (the overall-nearest
//! neighbor). The integrate stage consumes `move_x/move_y` (normalized by
//! `decide_all`); M12+ consumes the intents and target.

use crate::genome::Genome;
use crate::program::{evaluate, ActionRegister, EvalContext, Program, NO_TARGET};
use crate::sense::{SensorRegister, NO_NEIGHBOR_ID};

/// Evaluate one agent's program and return its full action register.
pub fn decide(
    program: &Program,
    genome: &Genome,
    sensor: &SensorRegister,
    meme: &[f32; crate::program::MEME_CHANNELS],
    energy: f32,
    age: u32,
    eval_stack: &mut Vec<f32>,
) -> ActionRegister {
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
        pheromone_sample: sensor.pheromone,
        meme_sample: *meme,
        nearest_kinship: sensor.nearest_kinship,
        hostility: sensor.hostility,
    };
    let mut action = evaluate(program, ctx, eval_stack);
    action.target_id = if sensor.nearest_neighbor_id == NO_NEIGHBOR_ID {
        NO_TARGET
    } else {
        sensor.nearest_neighbor_id
    };
    action
}
