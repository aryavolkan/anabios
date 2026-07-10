//! M12 emergence: seeded stalkers predate grazers across many seeds.
//! Release-gated (ignored in debug builds) per spec §2.2.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/predator-prey.toml");
const SEEDS: u64 = 16;
const TICKS: u32 = 800;
/// Measured on this scenario: predation in 15/16 seeds, both species persist in
/// 16/16. Floors are set well below the observed rates so unrelated tuning
/// drift can't flake the test (spec §2.2).
const PREDATION_FLOOR: u64 = 11;
/// Minimum seeds in which prey AND predators both survive to `TICKS`
/// (coexistence past a crash-only baseline — observed 16/16).
const PERSIST_FLOOR: u64 = 12;

#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn predation_emerges_across_seeds() {
    let mut with_predation = 0u64;
    let mut both_persist = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse predator-prey");
        s.seed = seed;
        let mut w = s.instantiate();
        for _ in 0..TICKS {
            step(&mut w);
        }
        let predated = w.codex.events.iter().any(|e| e.event_type == EventType::Predation);
        if predated {
            with_predation += 1;
        }
        // Prey (grazer archetype) = species 1, predators (stalker) = species 2.
        let mut prey_alive = 0u32;
        let mut pred_alive = 0u32;
        for id in w.agents.iter_alive() {
            match w.agents.species_id[id as usize] {
                1 => prey_alive += 1,
                2 => pred_alive += 1,
                _ => {}
            }
        }
        if prey_alive > 0 && pred_alive > 0 {
            both_persist += 1;
        }
    }
    assert!(
        with_predation >= PREDATION_FLOOR,
        "Predation emerged in only {with_predation}/{SEEDS} seeds (floor {PREDATION_FLOOR})"
    );
    assert!(
        both_persist >= PERSIST_FLOOR,
        "Prey+predator coexistence in only {both_persist}/{SEEDS} seeds (floor {PERSIST_FLOOR})"
    );
}
