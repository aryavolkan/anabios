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

/// The scenario's *other* job is the mood body-color overlay: it is the demo
/// the viewer menu opens on body mode 5, and `gallery/README.md` documents the
/// palette it is supposed to show. Pin that claim so a tuning change can't
/// quietly collapse the demo to one or two colors.
///
/// Measured over 600 ticks: seeds 0-3 reach 8/7/7/7 distinct moods — every one
/// except `fight`, which needs RAGE to beat FEAR in `mood::compute_mood` and
/// only fires on some seeds (never for the wolves; see the scenario header).
/// The floor sits below that so unrelated drift can't flake the test.
#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn grazers_and_wolves_paint_the_mood_palette() {
    const OVERLAY_SEEDS: u64 = 4;
    const OVERLAY_TICKS: u32 = 600;
    const DISTINCT_MOOD_FLOOR: usize = 6;
    for seed in 0..OVERLAY_SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse grazers-and-wolves");
        s.seed = seed;
        let mut w = s.instantiate();
        let mut seen = [false; anabios_core::mood::MOOD_COUNT];
        for _ in 0..OVERLAY_TICKS {
            step(&mut w);
            for id in w.agents.iter_alive() {
                seen[w.agents.mood[id as usize] as usize] = true;
            }
        }
        let observed: Vec<&str> = seen
            .iter()
            .enumerate()
            .filter(|(_, &s)| s)
            .map(|(m, _)| anabios_core::mood::name(m as u8))
            .collect();
        assert!(
            observed.len() >= DISTINCT_MOOD_FLOOR,
            "seed {seed}: only {} distinct moods in {OVERLAY_TICKS} ticks \
             (floor {DISTINCT_MOOD_FLOOR}): {observed:?}",
            observed.len()
        );
    }
}
