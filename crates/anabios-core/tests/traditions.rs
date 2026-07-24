//! Integration test: the E9 tradition detectors fire on the traditions
//! showcase scenario (sweep evidence in the E9 plan completion notes).

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/traditions.toml");

#[test]
fn traditions_scenario_fires_tradition_events() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    // Pin the cap for debug-profile speed; culture keeps flowing.
    world.max_population = 800;

    // Radiation fires early (t≈150), traditions latch by t≈4000 (see plan).
    for _ in 0..5000 {
        step(&mut world);
    }

    let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
    assert!(
        saw(EventType::TraditionPreserved)
            || saw(EventType::CulturalRadiation)
            || saw(EventType::InstitutionalRatchet),
        "expected at least one E9 tradition event; got {:?}",
        world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
    );
}
