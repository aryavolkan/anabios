//! M15 emergence: a dense cluster of cooperator herbivores develops kin-sharing
//! behaviour (EvolvedCooperation) and/or spatial cohesion (HerdCohesion).
//! Release-gated (ignored in debug) per spec §2.2.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/cooperation.toml");
const SEEDS: u64 = 16;
const TICKS: u32 = 400;
/// Measured on this scenario: EvolvedCooperation in 16/16 seeds (kin-gated
/// sharing sustains in the dense cluster), HerdCohesion in 13/16, and the
/// population survives to TICKS in 16/16. Floor set well below the observed
/// rate so tuning drift can't flake it (§2.2).
const COOP_FLOOR: u64 = 13;

#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn cooperation_emerges_across_seeds() {
    let mut with_coop = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse cooperation");
        s.seed = seed;
        let mut w = s.instantiate();
        for _ in 0..TICKS {
            step(&mut w);
        }
        let emerged = w.codex.events.iter().any(|e| {
            e.event_type == EventType::EvolvedCooperation || e.event_type == EventType::HerdCohesion
        });
        if emerged {
            with_coop += 1;
        }
    }
    assert!(
        with_coop >= COOP_FLOOR,
        "EvolvedCooperation/HerdCohesion in only {with_coop}/{SEEDS} seeds (floor {COOP_FLOOR})"
    );
}
