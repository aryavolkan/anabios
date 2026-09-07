//! E14 knowledge: flag+validation parse test (Task 1), plus the
//! `KnowledgeRatchet` detector's scenario wiring, flag-off no-op, save/load
//! round-trip, and the release-gated emergence check that the
//! knowledge-ratchet scenario's innovator culture crosses
//! `KNOWLEDGE_RATCHET_MIN` and fires the event (Task 4).

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

mod common;

const SCENARIO: &str = include_str!("../../../scenarios/knowledge-ratchet.toml");

#[test]
fn knowledge_flag_requires_inventions() {
    let bad = "name=\"k\"\nseed=1\nworld_size=64\nknowledge_enabled=true\n[[agents]]\narchetype=\"grazer\"\ncount=4\n";
    assert!(Scenario::parse_toml(bad).is_err());
    let ok = "name=\"k\"\nseed=1\nworld_size=64\ninventions_enabled=true\nknowledge_enabled=true\n[[agents]]\narchetype=\"grazer\"\ncount=4\n";
    let w = Scenario::parse_toml(ok).unwrap().instantiate();
    assert!(w.knowledge_enabled);
}

#[test]
fn scenario_instantiates_with_writing_held_from_t0() {
    use anabios_core::invention::{has, WRITING};
    let w = Scenario::parse_toml(SCENARIO).expect("parse knowledge-ratchet").instantiate();
    assert!(w.knowledge_enabled && w.inventions_enabled);
    assert!(w.agents.live_count() > 0);
    for id in w.agents.iter_alive() {
        assert!(
            has(&w.agents.meme_vector[id as usize], WRITING),
            "every seeded agent holds Writing at tick 0"
        );
    }
}

/// Flag-off no-op: a scenario with inventions but not knowledge never emits
/// `KnowledgeRatchet`, even though the invention tree (and Writing) is live.
#[test]
fn flag_off_scenario_has_no_knowledge_ratchet() {
    const INVENTIONS: &str = include_str!("../../../scenarios/inventions.toml");
    let mut w = Scenario::parse_toml(INVENTIONS).expect("parse inventions").instantiate();
    assert!(!w.knowledge_enabled);
    for _ in 0..300 {
        step(&mut w);
        for ev in w.codex.drain_events() {
            assert_ne!(
                ev.event_type,
                EventType::KnowledgeRatchet,
                "flag off: KnowledgeRatchet must never fire"
            );
        }
    }
    assert!(w.codex.knowledge_by_species.is_empty(), "flag off: no knowledge state tracked");
}

/// Knowledge actually accrues per species over a normal run — the
/// non-triviality precondition behind `knowledge_roundtrip` in
/// `save_load_roundtrip.rs`, which would otherwise round-trip an empty map.
#[test]
fn knowledge_accrues_per_species() {
    let w = common::world_after(SCENARIO, 300);
    assert!(
        !w.codex.knowledge_by_species.is_empty(),
        "knowledge state should be non-trivial after 300 ticks"
    );
}

/// Emergence: the innovator culture (Writing held from t0) accrues knowledge
/// past `KNOWLEDGE_RATCHET_MIN` and fires `KnowledgeRatchet` across seeds.
/// Release-gated: `KNOWLEDGE_GAIN` (0.002/tick) × `KNOWLEDGE_RATCHET_MIN`
/// (0.5) needs ~250 ticks of a live Writing-holder; 2000 ticks gives ample
/// headroom for the population to establish first.
#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn knowledge_ratchet_emerges_across_seeds() {
    const SEEDS: u64 = 5;
    const TICKS: u32 = 2000;
    let mut fired = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse knowledge-ratchet");
        s.seed = seed;
        let mut w = s.instantiate();
        let mut saw_ratchet = false;
        let mut first_tick = None;
        for _ in 0..TICKS {
            step(&mut w);
            for ev in w.codex.drain_events() {
                if ev.event_type == EventType::KnowledgeRatchet && !saw_ratchet {
                    saw_ratchet = true;
                    first_tick = Some(ev.tick);
                }
            }
        }
        if saw_ratchet {
            fired += 1;
        }
        eprintln!(
            "seed {seed}: alive={} knowledge_ratchet_fired={saw_ratchet} first_tick={first_tick:?}",
            w.agents.live_count()
        );
    }
    assert!(fired >= 4, "KnowledgeRatchet fired in ≥4/{SEEDS} seeds: {fired}");
}
