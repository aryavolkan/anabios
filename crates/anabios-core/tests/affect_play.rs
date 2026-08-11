//! M-E flag-ON end-to-end: PLAY trigger + social-approach bias + IQ-enrichment
//! coupling. Both `affect_enabled` and `cognition_enabled` are on in the scenario
//! so all three PLAY touchpoints are exercised. Models `cognition.rs`.

use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/affect-play.toml");

#[test]
fn affect_play_scenario_parses_with_both_flags() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect-play scenario");
    assert!(s.affect_enabled, "affect layer must be on");
    assert!(s.cognition_enabled, "cognition must be on");
}

#[test]
fn affect_play_scenario_is_self_consistent() {
    let run = || {
        let mut w = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
        for _ in 0..200 {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(), run(), "same seed + flags on → bit-identical");
}

/// Flag-ON trajectory pin for M-E PLAY + enrichment (affect_enabled +
/// cognition_enabled). Regenerate deliberately with `UPDATE_HASHES=1` when the
/// PLAY behaviour changes on purpose.
// Refreshed 2026-08-07 (M-F observability, FORMAT_VERSION 28→29): new serialized
// CodexState detector fields grow the state-hash layout (detectors run flag-on),
// so all ticks move.
// Refreshed 2026-08-07 (O2 payoff-biased learning, FORMAT_VERSION 29→30):
// World.payoff_biased_learning layout growth only (off here).
const PLAY_GOLDEN: &[(u64, u64)] =
    &[(0, 0xdba85ce2324c26b1), (100, 0xaff2d8183f5ce210), (200, 0x83d540ed7b286b79)];

#[test]
fn affect_play_matches_golden_hashes() {
    let mut w = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    let max_tick = PLAY_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < PLAY_GOLDEN.len() && PLAY_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        step(&mut w);
    }
    if std::env::var("UPDATE_HASHES").is_ok() {
        for (t, h) in &observed {
            println!("({t}, {h:#018x}),");
        }
    }
    assert_eq!(observed, PLAY_GOLDEN.to_vec(), "affect-play flag-on trajectory changed");
}

#[test]
fn affect_play_survives_save_load_step() {
    let mut world = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    for _ in 0..80 {
        step(&mut world); // warm juveniles so PLAY activations + enrichment accumulate
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&world), state_hash(&reloaded), "load must restore identical state");
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "affect-play world diverged after save→load→step (hidden non-serialized PLAY state?)",
    );
}
