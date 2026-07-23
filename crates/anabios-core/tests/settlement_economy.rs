//! Integration test: the E8 settlement & economy detectors fire on the
//! settlement showcase scenario (sweep evidence in the E8 plan completion
//! notes).

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/settlement.toml");

#[test]
fn settlement_scenario_fires_economy_events() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    // Pin the cap for debug-profile speed; trade and harvest persist.
    world.max_population = 800;

    // Markets crystallize by t≈400, specialization splits by t≈60 (see plan).
    for _ in 0..1200 {
        step(&mut world);
    }

    let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
    assert!(
        saw(EventType::SettlementFormed)
            || saw(EventType::MarketEmerged)
            || saw(EventType::SpecializationSplit),
        "expected at least one E8 economy event; got {:?}",
        world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
    );
}
