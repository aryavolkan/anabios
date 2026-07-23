//! Integration test: the E4 disturbance substrate and detectors fire on the
//! disturbance showcase scenario (sweep evidence in the E4 plan completion
//! notes).

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/disturbance.toml");

#[test]
fn disturbance_scenario_scars_and_recovers() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    // Pin the cap so the debug-profile run stays fast; the disturbance
    // dynamics (fires, scars, re-vegetation) are unaffected.
    world.max_population = 500;

    for _ in 0..3000 {
        step(&mut world);
    }

    // The scheduler must have produced disasters and at least one scar.
    assert!(world.disasters.spawned > 0, "no disasters in 3000 ticks");
    assert!(
        world.biome.cells.iter().any(|c| c.succession != anabios_core::biome::SUCCESSION_CLIMAX),
        "no succession-scarred cells after disasters"
    );

    // Detector coverage: at least one of the four new event types fired.
    let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
    assert!(
        saw(EventType::RangeExpansion)
            || saw(EventType::SegregationEmerged)
            || saw(EventType::CorridorUse)
            || saw(EventType::Succession),
        "expected at least one E4 event; got {:?}",
        world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
    );
}
