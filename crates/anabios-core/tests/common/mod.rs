//! Shared helpers for the `anabios-core` integration suite.
//!
//! Each test binary pulls this in with `mod common;`. Binaries use different
//! subsets, so the module allows dead code rather than making every call site
//! annotate around the unused rest.

#![allow(dead_code)]

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;
use anabios_core::world::World;

/// Parse a scenario TOML source and instantiate its world.
pub fn world(src: &str) -> World {
    Scenario::parse_toml(src).expect("parse scenario").instantiate()
}

/// Step `w` forward by `ticks`.
pub fn run(w: &mut World, ticks: u64) {
    for _ in 0..ticks {
        step(w);
    }
}

/// Parse, instantiate, and warm a scenario in one call.
pub fn world_after(src: &str, ticks: u64) -> World {
    let mut w = world(src);
    run(&mut w, ticks);
    w
}

/// Codex events of `kind` currently in the ring buffer.
pub fn count_events(w: &World, kind: EventType) -> usize {
    w.codex.events.iter().filter(|e| e.event_type == kind).count()
}

/// Horizon scaled for coverage instrumentation. `cargo llvm-cov` sets
/// `--cfg coverage`, under which every tick runs ~5-10x slower; tests whose
/// claim does not depend on horizon length shorten there so a handful of long
/// simulations stop dominating the coverage job's wall-time.
pub fn ticks(full: u64) -> u64 {
    if cfg!(coverage) {
        full.min(100)
    } else {
        full
    }
}

/// Replay a scenario and compare `state_hash` at each of `golden`'s ticks.
///
/// This is the golden-trajectory gate: it pins the *exact* simulation
/// trajectory, so any unintended behaviour change shows up as a hash drift at
/// the first tick that diverges. Collapses the copy of this loop that each
/// golden-bearing binary used to carry.
///
/// When a change is deliberate, rerun with `UPDATE_HASHES=1` — the observed
/// table is printed in source form and the assertion is skipped, so the values
/// can be pasted straight back into the `golden` constant.
pub fn assert_golden(label: &str, src: &str, golden: &[(u64, u64)]) {
    let mut w = world(src);
    let max_tick = golden.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < golden.len() && golden[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        if w.tick == max_tick {
            break;
        }
        step(&mut w);
    }

    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// regenerated {label} hashes:");
        for (t, h) in &observed {
            println!("    ({t}, 0x{h:016x}),");
        }
        return;
    }

    assert_eq!(
        golden.len(),
        observed.len(),
        "{label}: recorded {} of {} golden ticks — the run ended early",
        observed.len(),
        golden.len()
    );
    for ((exp_tick, exp_hash), (got_tick, got_hash)) in golden.iter().zip(&observed) {
        assert_eq!(exp_tick, got_tick, "{label}: tick mismatch");
        assert_eq!(
            exp_hash, got_hash,
            "{label}: hash drift at tick {exp_tick}: expected 0x{exp_hash:016x}, got 0x{got_hash:016x}.\n\
             If intentional, rerun with UPDATE_HASHES=1 and copy the printed values.",
        );
    }
}

/// Warm a scenario, then assert it survives a save→load→step round-trip.
///
/// Guards the `#[serde(skip)]`-accumulator footgun: a skipped field that feeds
/// future ticks is invisible to `state_hash` at rest, yet makes a reloaded
/// world diverge on the very next tick. `flag` asserts the scenario really
/// enables the subsystem under test, so a scenario edit that silently drops the
/// flag fails loudly instead of vacuously passing.
pub fn assert_roundtrip(src: &str, warm: u64, flag: fn(&World) -> bool, what: &str) {
    let mut w = world(src);
    assert!(flag(&w), "scenario must enable {what}");
    // The round-trip semantics under test don't depend on warm length, so
    // shorten it under coverage instrumentation.
    run(&mut w, ticks(warm));
    assert_roundtrip_world(&mut w, what);
}

/// `assert_roundtrip` for a world built programmatically rather than from a
/// scenario file — already warmed by the caller, which also owns any
/// non-triviality precondition.
pub fn assert_roundtrip_world(world: &mut World, what: &str) {
    let bytes = save_to_bytes(world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(
        state_hash(world),
        state_hash(&reloaded),
        "{what}: load must restore identical state"
    );
    step(world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(world),
        state_hash(&reloaded),
        "{what}: diverged after save→load→step — hidden non-serialized state feeding the sim?"
    );
}
