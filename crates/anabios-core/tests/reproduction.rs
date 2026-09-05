//! Integration test: with reproduction (M2), the minimal scenario must
//! sustain its population over a window longer than the natural lifespan,
//! confirming that newborns are replacing deaths.

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

#[test]
fn population_sustains_past_one_lifespan() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();
    // Sustaining a population past a lifespan doesn't need scale — cap it so the
    // 5,000-tick run stays fast under the raised 10k default.
    world.max_population = 500;
    let initial_alive = world.agents.live_count();
    assert!(initial_alive > 0);

    // Run for 5,000 ticks — well past the natural lifespan (≈ 3,200 ticks
    // at LifespanBias = 0.6).
    for _ in 0..5_000 {
        step(&mut world);
    }

    let final_alive = world.agents.live_count();
    assert!(
        final_alive > 0,
        "population should sustain past one lifespan; initial={initial_alive}, final={final_alive}",
    );
}

/// O3 repro-biased learning: birth outcomes are counted iff the flag is on.
/// Same scenario, same seed, flag toggled — the flag-off world must keep every
/// counter at zero (the byte-identity contract), the flag-on world must have
/// credited surviving births to parents.
#[test]
fn birth_outcome_counters_are_flag_gated() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");

    let mut on = scenario.instantiate();
    on.repro_biased_learning = true;
    on.max_population = 500;
    let mut off = scenario.instantiate();
    off.max_population = 500;

    for _ in 0..2_000 {
        step(&mut on);
        step(&mut off);
    }

    let sum_ok: u32 = on.agents.births_ok.iter().map(|&b| b as u32).sum();
    assert!(sum_ok > 0, "flag on: surviving births must be credited to parents");
    let off_total: u32 =
        off.agents.births_ok.iter().chain(off.agents.births_failed.iter()).map(|&b| b as u32).sum();
    assert_eq!(off_total, 0, "flag off: no birth-outcome counting at all");
}
