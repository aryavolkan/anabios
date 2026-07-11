//! M13 emergence: seeded marking species form clustered territories.
//! Release-gated (ignored in debug) per spec §2.2.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/territories.toml");
const SEEDS: u64 = 16;
const TICKS: u32 = 400;
/// Measured on this scenario: TerritoryFormation in 16/16 seeds (marking
/// species reliably cluster and mark), NichePartitioning in 6/16 (too marginal
/// to gate on). Floor set well below the observed rate so unrelated tuning
/// drift can't flake it (spec §2.2).
const TERRITORY_FLOOR: u64 = 13;

#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn territories_form_across_seeds() {
    let mut with_territory = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse territories");
        s.seed = seed;
        let mut w = s.instantiate();
        for _ in 0..TICKS {
            step(&mut w);
        }
        let formed = w.codex.events.iter().any(|e| e.event_type == EventType::TerritoryFormation);
        if formed {
            with_territory += 1;
        }
    }
    assert!(
        with_territory >= TERRITORY_FLOOR,
        "TerritoryFormation in only {with_territory}/{SEEDS} seeds (floor {TERRITORY_FLOOR})"
    );
}
