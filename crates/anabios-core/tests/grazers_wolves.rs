//! M12-style emergence for the grazers-and-wolves demo scenario. Verifies that
//! the mood-aware mammal archetypes sustain a working predator guild:
//! predation fires and both founder lineages persist across seeds.
//! Release-gated per spec §2.2.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/grazers-and-wolves.toml");
const SEEDS: u64 = 8;
const TICKS: u32 = 2000;
/// Measured on this scenario: predation in 8/8 seeds, both lineages persist
/// in 8/8 seeds. Floors are set well below observed rates so unrelated tuning
/// drift can't flake the test (spec §2.2).
const PREDATION_FLOOR: u64 = 6;
const PERSIST_FLOOR: u64 = 6;

/// Walk the species-parent chain to the founder species id (1 = mammal
/// grazer, 2 = mammal pursuer), so descendants that speciated away still
/// count toward their founder's lineage. Mirrors `codex::war::lineage_root`
/// (not exported).
fn lineage_root(w: &anabios_core::World, sid: u32) -> u32 {
    let mut cur = sid;
    for _ in 0..64 {
        match w.species_parents.get(cur as usize).copied().flatten() {
            Some(p) if p != cur && p != 0 => cur = p,
            _ => break,
        }
    }
    cur
}

#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn grazers_and_wolves_sustain_predation() {
    let mut with_predation = 0u64;
    let mut both_persist = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse grazers-and-wolves");
        s.seed = seed;
        let mut w = s.instantiate();
        for _ in 0..TICKS {
            step(&mut w);
        }
        let predated = w.codex.events.iter().any(|e| e.event_type == EventType::Predation);
        if predated {
            with_predation += 1;
        }
        let mut lineage_alive = [false; 2];
        for id in w.agents.iter_alive() {
            let root = lineage_root(&w, w.agents.species_id[id as usize]);
            if (1..=2).contains(&root) {
                lineage_alive[(root - 1) as usize] = true;
            }
        }
        if lineage_alive.iter().all(|&a| a) {
            both_persist += 1;
        }
    }
    assert!(
        with_predation >= PREDATION_FLOOR,
        "Predation emerged in only {with_predation}/{SEEDS} seeds (floor {PREDATION_FLOOR})"
    );
    assert!(
        both_persist >= PERSIST_FLOOR,
        "Grazer + wolf lineages both persisted in only {both_persist}/{SEEDS} seeds \
         (floor {PERSIST_FLOOR})"
    );
}
