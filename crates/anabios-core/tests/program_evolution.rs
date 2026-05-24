//! Integration test: program mutation drifts over generations.
//!
//! Founders all start with `starter_grazer()`. After enough reproduction +
//! mutation cycles, at least one alive agent must carry a program that
//! differs from the starter.

use anabios_core::program::{starter_grazer, Program};
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

#[test]
fn at_least_one_program_diverges_from_starter_within_5000_ticks() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    let starter: Program = starter_grazer();

    for _ in 0..5_000 {
        step(&mut world);
        let any_divergent =
            world.agents.iter_alive().any(|id| world.agents.program[id as usize] != starter);
        if any_divergent {
            return;
        }
    }
    panic!("no program diverged from the starter in 5000 ticks");
}
