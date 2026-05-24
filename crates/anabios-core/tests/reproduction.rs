//! Integration test: with reproduction (M2), the minimal scenario must
//! sustain its population over a window longer than the natural lifespan,
//! confirming that newborns are replacing deaths.

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

#[test]
fn population_sustains_past_one_lifespan() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    let initial_alive = world.agents.live_count();
    assert!(initial_alive > 0);

    // Run for 5,000 ticks — well past the natural lifespan (≈ 3,200 ticks
    // at LifespanBias = 0.6).
    for _ in 0..5_000 {
        step(&mut world);
    }

    let final_alive = world.agents.live_count();
    assert!(
        final_alive > 0,
        "population should sustain past one lifespan; initial={initial_alive}, final={final_alive}",
    );
}
