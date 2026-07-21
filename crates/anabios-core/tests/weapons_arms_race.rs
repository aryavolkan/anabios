//! weapons-arms-race scenario regression: the armed founder lineages are
//! expected to die as *species* (speciation splits them), but their weapon
//! modules should persist in descendant lineages — the scenario's promise
//! is that Spines and Jaws establish as evolved traits, not one-generation
//! novelties.

use anabios_core::module::{self, ModuleType};
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/weapons-arms-race.toml");

fn count_with(world: &anabios_core::world::World, t: ModuleType) -> usize {
    world
        .agents
        .iter_alive()
        .filter(|&id| module::has(&world.agents.modules[id as usize], t))
        .count()
}

#[test]
fn armed_lineages_persist_through_speciation() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse arms-race scenario");
    let mut w = scenario.instantiate();
    // Founder species die around t=1100; run past that so survival can only
    // come from descendant lineages inheriting the weapon modules.
    for _ in 0..1500 {
        step(&mut w);
    }
    let spines = count_with(&w, ModuleType::Spines);
    let jaws = count_with(&w, ModuleType::Jaws);
    assert!(spines > 0, "no Spines-bearing agents left at tick 1500");
    assert!(jaws > 0, "no Jaws-bearing agents left at tick 1500");
}
