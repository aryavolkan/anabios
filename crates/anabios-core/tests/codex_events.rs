//! Integration test: codex emits SpeciationEvent on a divergent scenario
//! where two distant founder populations are forced to split.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/divergent.toml");

#[test]
fn divergent_scenario_emits_speciation_event() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();

    // 400 ticks is well past the first species_step (at tick 200).
    for _ in 0..400 {
        step(&mut world);
    }

    let saw_speciation =
        world.codex.events.iter().any(|ev| ev.event_type == EventType::SpeciationEvent);
    assert!(
        saw_speciation,
        "expected at least one SpeciationEvent; got {:?}",
        world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
    );
}
