//! End-to-end determinism for the flag-ON affect scenario. `determinism.rs`
//! locks the flag-OFF minimal scenario; this pins the affect layer's real
//! behavior (SEEKING-biased foraging) so it cannot drift silently, and proves
//! the serialized `affect` column survives a save→load→step round-trip.

use anabios_core::codex::EventType;
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
        state_hash(&world),
        state_hash(&reloaded),
        "load must restore identical state (affect column persisted)"
    );
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "affect world diverged after save→load→step (non-serialized affect state?)"
    );
}

/// Pinned flag-ON golden. Generated once with `UPDATE_HASHES=1` after the
/// SEEKING layer was wired; regenerate deliberately whenever an affect change
/// is intentional.
const AFFECT_GOLDEN: &[(u64, u64)] =
    &[(0, 0x48945c0200cea750), (100, 0x29432d7fb5e74d12), (300, 0x22a1de3ba06ffacb)];

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

// --- M-B: FEAR / hijack flag-on tests (affect-threat scenario) ---

const THREAT_SCENARIO: &str = include_str!("../../../scenarios/affect-threat.toml");

#[test]
fn affect_threat_parses_with_flag_on() {
    let s = Scenario::parse_toml(THREAT_SCENARIO).expect("parse affect-threat");
    assert!(s.affect_enabled, "scenario must enable the affect layer");
}

#[test]
fn affect_threat_is_self_consistent() {
    let s = Scenario::parse_toml(THREAT_SCENARIO).expect("parse affect-threat");
    let run = || {
        let mut w = s.instantiate();
        for _ in 0..300 {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(), run(), "same seed + flag on ⇒ bit-identical");
}

#[test]
fn affect_threat_survives_save_load_step() {
    let s = Scenario::parse_toml(THREAT_SCENARIO).expect("parse affect-threat");
    let mut world = s.instantiate();
    for _ in 0..300 {
        step(&mut world);
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&world), state_hash(&reloaded), "load must restore identical state");
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "affect world diverged after save→load→step (hidden non-serialized affect state?)",
    );
}

#[test]
fn affect_threat_emits_mass_fright() {
    let s = Scenario::parse_toml(THREAT_SCENARIO).expect("parse affect-threat");
    let mut w = s.instantiate();
    let mut saw = false;
    for _ in 0..1500 {
        step(&mut w);
        if w.codex.events.iter().any(|e| e.event_type == EventType::MassFright) {
            saw = true;
            break;
        }
    }
    assert!(saw, "predator-driven FEAR must produce a MassFright within 1500 ticks");
}

/// Pinned golden for the flag-ON affect-threat scenario. Regenerate deliberately
/// with `UPDATE_HASHES=1` whenever an affect-behavior change is intentional.
// Created 2026-08-03 (M-B): first pin of the FEAR/hijack layer's real behavior.
const THREAT_GOLDEN: &[(u64, u64)] =
    &[(0, 0xc7a838c2945a1d94), (100, 0x01ce1a3a552fa28e), (300, 0x9467ba4b761fcdf6)];

#[test]
fn affect_threat_matches_golden_hashes() {
    let s = Scenario::parse_toml(THREAT_SCENARIO).expect("parse affect-threat");
    let mut w = s.instantiate();
    let max_tick = THREAT_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < THREAT_GOLDEN.len() && THREAT_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        if w.tick == max_tick {
            break;
        }
        step(&mut w);
    }
    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// regenerated affect-threat hashes:");
        for (t, h) in &observed {
            println!("    ({t}, 0x{h:016x}),");
        }
        return;
    }
    for ((et, eh), (gt, gh)) in THREAT_GOLDEN.iter().zip(&observed) {
        assert_eq!(et, gt, "tick mismatch");
        assert_eq!(
            *eh, *gh,
            "affect-threat hash drift at tick {et}: expected 0x{eh:016x}, got 0x{gh:016x}"
        );
    }
}
