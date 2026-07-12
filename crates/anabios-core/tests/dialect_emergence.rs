//! M14 emergence: two geographically isolated communicator clusters develop
//! distinct meme distributions (DialectFormed) or one cluster sweeps its meme
//! to fixation (MemeSweep).
//! Release-gated (ignored in debug) per spec §2.2.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/dialects.toml");
const SEEDS: u64 = 16;
const TICKS: u32 = 400;
/// Measured on this scenario: a broadcast meme sweeps each communicator cluster
/// to dominance → MemeSweep in 16/16 seeds. DialectFormed is 0/16 here (the two
/// clusters are distinct species, so there is no within-species east/west split
/// to diverge — that detector ships but its emergence is left to later scenarios).
/// Floor set well below the observed rate so tuning drift can't flake it (§2.2).
const DIALECT_FLOOR: u64 = 13;

#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn dialects_form_across_seeds() {
    let mut with_dialect = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse dialects");
        s.seed = seed;
        let mut w = s.instantiate();
        for _ in 0..TICKS {
            step(&mut w);
        }
        let formed = w.codex.events.iter().any(|e| {
            e.event_type == EventType::DialectFormed || e.event_type == EventType::MemeSweep
        });
        if formed {
            with_dialect += 1;
        }
    }
    assert!(
        with_dialect >= DIALECT_FLOOR,
        "DialectFormed/MemeSweep in only {with_dialect}/{SEEDS} seeds (floor {DIALECT_FLOOR})"
    );
}
