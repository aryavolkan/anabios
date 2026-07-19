//! End-to-end determinism for the flag-ON cognitive gene–culture scenario.
//! `determinism.rs` only locks the flag-OFF minimal scenario and `inventions.rs`
//! the inventions demo; this pins the cognitive layer's actual behavior (IQ
//! development, IQ-gated acquisition, practice discovery/spread, reproductive
//! effects) so it cannot drift silently.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/cognitive-coevolution.toml");

#[test]
fn cognitive_scenario_parses_with_both_flags() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse cognitive scenario");
    assert!(s.cognition_enabled);
    assert!(s.inventions_enabled);
}

#[test]
fn cognitive_scenario_is_self_consistent() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse cognitive scenario");
    let run = |ticks: u64| {
        let mut w = s.instantiate();
        for _ in 0..ticks {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(300), run(300), "same seed + flags on → bit-identical");
}

/// Pinned golden for the flag-ON cognitive scenario. Regenerate deliberately
/// with `UPDATE_HASHES=1` (prints new values) whenever a cognitive change is
/// intentional.
const COGNITIVE_GOLDEN: &[(u64, u64)] =
    &[(0, 0x42f7a6e1c6717d0b), (100, 0xe113d89d6b29b3c6), (300, 0x4da78f4c4a5e0e5a)];

#[test]
fn cognitive_scenario_matches_golden_hashes() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse cognitive scenario");
    let mut w = s.instantiate();
    let max_tick = COGNITIVE_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < COGNITIVE_GOLDEN.len() && COGNITIVE_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        if w.tick == max_tick {
            break;
        }
        step(&mut w);
    }
    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// regenerated cognitive hashes:");
        for (t, h) in &observed {
            println!("    ({t}, 0x{h:016x}),");
        }
        return;
    }
    for ((exp_tick, exp_hash), (got_tick, got_hash)) in COGNITIVE_GOLDEN.iter().zip(&observed) {
        assert_eq!(exp_tick, got_tick, "tick mismatch");
        assert_eq!(
            *exp_hash, *got_hash,
            "cognitive hash drift at tick {exp_tick}: expected 0x{exp_hash:016x}, got 0x{got_hash:016x}.\n\
             If intentional, rerun with UPDATE_HASHES=1 and copy the printed values.",
        );
    }
}

/// The demo's promise: with cognition on, both beneficial tech and maladaptive
/// practices appear in the codex event stream within a few hundred ticks.
#[test]
fn cognitive_scenario_produces_invention_and_practice_events() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse cognitive scenario");
    let mut w = s.instantiate();
    let mut saw_invention = false;
    let mut saw_practice = false;
    for _ in 0..5000 {
        step(&mut w);
        for ev in w.codex.drain_events() {
            match ev.event_type {
                EventType::InventionDiscovered | EventType::InventionAdopted => {
                    saw_invention = true
                }
                EventType::PracticeDiscovered | EventType::PracticeAdopted => saw_practice = true,
                _ => {}
            }
        }
        if saw_invention && saw_practice {
            break;
        }
    }
    assert!(saw_invention, "cognitive scenario should climb the tech tree");
    assert!(saw_practice, "cognitive scenario should surface a maladaptive practice");
}
