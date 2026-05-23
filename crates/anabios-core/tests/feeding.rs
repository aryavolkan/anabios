//! Integration test: a herbivore population on grass survives 500 ticks
//! without total collapse or runaway plant blow-up.

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

#[test]
fn population_persists_for_500_ticks() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    let initial_alive = world.agents.live_count();
    assert!(initial_alive > 0);

    let initial_biomass = world.plant_biomass_total();
    assert!(initial_biomass > 0.0);

    for _ in 0..500 {
        step(&mut world);
    }

    let final_alive = world.agents.live_count();
    let final_biomass = world.plant_biomass_total();
    // We expect attrition, but not extinction.
    assert!(
        final_alive > 0,
        "population went extinct in 500 ticks: {} -> {}",
        initial_alive,
        final_alive
    );
    // Biomass should remain in a reasonable band — not zero, not multiples
    // of carrying capacity.
    assert!(final_biomass > 0.0);
    assert!(final_biomass < initial_biomass * 1.5);
}
