//! Integration test: the E5 trait-evolution detectors fire on the convergent
//! showcase scenario (sweep evidence in the E5 plan completion notes).

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/convergent.toml");

#[test]
fn convergent_scenario_fires_trait_events() {
    let mut scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    // Seed 3 fires a TraitFixation within the 1500-tick window after the
    // PerceptionRadius gene was removed and non-cognition perception falls
    // back to a hardcoded neutral modulator.
    scenario.seed = 3;
    let mut world = scenario.instantiate();
    // Pin the cap for debug-profile speed; trait dynamics are unaffected.
    world.max_population = 1000;

    for _ in 0..1500 {
        step(&mut world);
    }

    let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
    assert!(
        saw(EventType::TraitFixation)
            || saw(EventType::RapidAdaptation)
            || saw(EventType::ConvergentEvolution),
        "expected at least one E5 trait event; got {:?}",
        world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
    );
}
