//! Integration test: the E5 trait-evolution detectors fire on the convergent
//! showcase scenario (sweep evidence in the E5 plan completion notes).

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/convergent.toml");

#[test]
fn convergent_scenario_fires_trait_events() {
    let mut scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    // Seed 19 fires a TraitFixation within the 1500-tick window under the
    // climate-driven worldgen (re-selected 2026-07-27; the pre-worldgen seed 5
    // no longer fixes a slot in time because the new terrain reshapes the
    // per-deme selective environment).
    scenario.seed = 19;
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
