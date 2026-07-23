//! Integration test: the E7 war/alliance/kin detectors fire on the war
//! showcase scenario (sweep evidence in the E7 plan completion notes).

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/war.toml");

#[test]
fn war_scenario_fires_war_events() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    // Pin the cap for debug-profile speed; the pack clash persists.
    world.max_population = 500;

    // Wars declare by t≈500 and kin networks latch at t≈1500 (see plan).
    for _ in 0..3000 {
        step(&mut world);
    }

    let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
    assert!(
        saw(EventType::WarOrRaid)
            || saw(EventType::WarEnded)
            || saw(EventType::AllianceFormed)
            || saw(EventType::KinNetworkStable),
        "expected at least one E7 event; got {:?}",
        world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
    );
}
