//! Integration test: two genetically-distant founder populations should be
//! recognized as separate species by the first time `species_step` runs
//! (tick 200) or shortly after.

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/divergent.toml");

#[test]
fn distant_founder_populations_become_separate_species() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();

    // Run past the first speciation event (200 ticks) plus a buffer for
    // the algorithm to recognize the split.
    for _ in 0..400 {
        step(&mut world);
    }

    // At least two non-empty species expected.
    let non_empty: usize = world.species_member_counts.iter().filter(|&&c| c > 0).count();
    assert!(
        non_empty >= 2,
        "expected speciation, got species member counts {:?}",
        world.species_member_counts,
    );

    // At least one species has a recorded parent (non-founder).
    let any_child = world.species_parents.iter().any(|p| p.is_some());
    assert!(any_child, "no non-founder species recorded in phylogeny");
}
