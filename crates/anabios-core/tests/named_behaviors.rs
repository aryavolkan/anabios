//! Integration test: the E6 named-behavior detectors fire on the tool-users
//! showcase scenario (sweep evidence in the E6 plan completion notes).

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/tool-users.toml");

#[test]
fn tool_users_scenario_fires_named_behavior_events() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    // Pin the cap for debug-profile speed; combat/invention dynamics persist.
    world.max_population = 500;

    // Signaling fired at t=460 and flight in 14/16 sweep runs (see plan).
    for _ in 0..2000 {
        step(&mut world);
    }

    let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
    assert!(
        saw(EventType::EvolvedAmbush)
            || saw(EventType::EvolvedTool)
            || saw(EventType::EvolvedFlight)
            || saw(EventType::StructuredSignaling),
        "expected at least one E6 named-behavior event; got {:?}",
        world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
    );
}
