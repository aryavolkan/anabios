//! End-to-end determinism for the flag-ON affect scenario. `determinism.rs`
//! locks the flag-OFF minimal scenario; this pins the affect layer's real
//! behavior (SEEKING-biased foraging) so it cannot drift silently, and proves
//! the serialized `affect` column survives a save→load→step round-trip.

use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/affect-seeking.toml");

#[test]
fn affect_scenario_parses_with_flag_on() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect scenario");
    assert!(s.affect_enabled, "scenario must enable the affect layer");
}

#[test]
fn affect_scenario_is_self_consistent() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse");
    let run = |ticks: u64| {
        let mut w = s.instantiate();
        for _ in 0..ticks {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(300), run(300), "same seed + flag on → bit-identical");
}

#[test]
fn affect_scenario_survives_save_load_step() {
    let mut world = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    assert!(world.affect_enabled);
    // Warm the world so SEEKING activations accumulate before the snapshot.
    for _ in 0..300 {
        step(&mut world);
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(
        state_hash(&world), state_hash(&reloaded),
        "load must restore identical state (affect column persisted)"
    );
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world), state_hash(&reloaded),
        "affect world diverged after save→load→step (non-serialized affect state?)"
    );
}

/// Pinned flag-ON golden. Generated once with `UPDATE_HASHES=1` after the
/// SEEKING layer was wired; regenerate deliberately whenever an affect change
/// is intentional.
const AFFECT_GOLDEN: &[(u64, u64)] =
    &[(0, 0x80a3ef4a7e8f1e13), (100, 0x94816c556ec83a61), (300, 0x9b2d5eaf36b5f7d8)];

#[test]
fn affect_scenario_matches_golden_hashes() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect scenario");
    let mut w = s.instantiate();
    let max_tick = AFFECT_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < AFFECT_GOLDEN.len() && AFFECT_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        if w.tick == max_tick {
            break;
        }
        step(&mut w);
    }
    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// regenerated affect hashes:");
        for (t, h) in &observed {
            println!("    ({t}, 0x{h:016x}),");
        }
        return;
    }
    for ((exp_tick, exp_hash), (got_tick, got_hash)) in AFFECT_GOLDEN.iter().zip(&observed) {
        assert_eq!(exp_tick, got_tick, "tick mismatch");
        assert_eq!(
            *exp_hash, *got_hash,
            "affect hash drift at tick {exp_tick}: expected 0x{exp_hash:016x}, got 0x{got_hash:016x}.\n\
             If intentional, rerun with UPDATE_HASHES=1 and copy the printed values.",
        );
    }
}
