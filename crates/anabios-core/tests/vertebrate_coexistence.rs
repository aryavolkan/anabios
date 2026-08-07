//! Vertebrate-class ecology: the mammals-vs-reptiles scenario sustains a
//! working predator guild — all four founder lineages (mammal grazer, mammal
//! pursuer, reptile ambusher, reptile basker) persist in most seeds, and the
//! affect layer fires (herd panic). Release-gated per spec §2.2.
//!
//! This guards the balance work in the archetype design: pursuit gated to a
//! pounce range (so kills get scavenged), high-damage Weapon (prey HP is its
//! energy), and pursuer Boldness (suppressing the FEAR aura of big prey).
//! Regression in any of these shows up as pursuer/ambusher lineage collapse.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/mammals-vs-reptiles.toml");
const SEEDS: u64 = 8;
const TICKS: u32 = 2000;
/// Measured on this scenario: all four founder lineages persist to 2000 ticks
/// in 8/8 seeds. Floor set well below the observed rate so unrelated tuning
/// drift can't flake the test (spec §2.2).
const ALL_PERSIST_FLOOR: u64 = 5;

/// Walk the species-parent chain to the founder species id (1 = mammal
/// grazer, 2 = mammal pursuer, 3 = reptile ambusher, 4 = reptile basker), so
/// descendants that speciated away still count toward their founder's
/// lineage. Mirrors `codex::war::lineage_root` (not exported).
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
fn vertebrate_classes_coexist_across_seeds() {
    let mut all_persist = 0u64;
    let mut with_fright = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse mammals-vs-reptiles");
        s.seed = seed;
        let mut w = s.instantiate();
        for _ in 0..TICKS {
            step(&mut w);
        }
        let mut lineage_alive = [false; 4];
        for id in w.agents.iter_alive() {
            let root = lineage_root(&w, w.agents.species_id[id as usize]);
            if (1..=4).contains(&root) {
                lineage_alive[(root - 1) as usize] = true;
            }
        }
        if lineage_alive.iter().all(|&a| a) {
            all_persist += 1;
        }
        if w.codex.events.iter().any(|e| e.event_type == EventType::MassFright) {
            with_fright += 1;
        }
    }
    assert!(
        all_persist >= ALL_PERSIST_FLOOR,
        "all four founder lineages persisted in only {all_persist}/{SEEDS} seeds \
         (floor {ALL_PERSIST_FLOOR})"
    );
    assert!(with_fright > 0, "affect layer never fired MassFright across {SEEDS} seeds");
}
