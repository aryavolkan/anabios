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
    // Speciation needs divergence between the two founder clusters, not scale —
    // cap population so the test stays fast under the raised 10k default.
    world.max_population = 500;

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

const AFFECT_SHOWCASE: &str = include_str!("../../../scenarios/affect-showcase.toml");

#[test]
fn affect_showcase_emits_an_affect_event() {
    let scenario = Scenario::parse_toml(AFFECT_SHOWCASE).expect("parse affect showcase");
    let mut world = scenario.instantiate();
    assert!(world.affect_enabled, "showcase scenario must enable affect");
    for _ in 0..800 {
        step(&mut world);
    }
    let saw = world.codex.events.iter().any(|e| {
        matches!(
            e.event_type,
            EventType::FeedingFrenzy
                | EventType::PanicCascade
                | EventType::TerritorialRage
                | EventType::MassGrief,
        )
    });
    assert!(
        saw,
        "expected an affect event; got {:?}",
        world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
    );
}
