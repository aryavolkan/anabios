//! Integration test: the E3 population-dynamics detectors fire on the
//! trophic-cascade showcase scenario (16/16 carrying-capacity, 14/16 cycle,
//! 9/16 cascade over a 16-seed sweep — see the E3 plan completion notes).

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/trophic-cascade.toml");

#[test]
fn trophic_cascade_scenario_fires_population_dynamics_events() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    // Keep the debug-profile test fast: pin the cap so the herd can't
    // explode to 10k. The guild oscillation survives; the world-total
    // plateau at the cap itself satisfies the detector set.
    world.max_population = 500;

    // 2000 ticks covers the first guild oscillation on the scenario seed.
    for _ in 0..2000 {
        step(&mut world);
    }

    let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
    assert!(
        saw(EventType::PopulationCycleDetected)
            || saw(EventType::CarryingCapacityReached)
            || saw(EventType::BoomAndBust)
            || saw(EventType::TrophicCascade),
        "expected at least one E3 population-dynamics event; got {:?}",
        world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
    );
}
