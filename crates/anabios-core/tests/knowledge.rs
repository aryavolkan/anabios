//! E14 knowledge: flag+validation parse test (Task 1), plus the
//! `KnowledgeRatchet` detector's scenario wiring, flag-off no-op, save/load
//! round-trip, and the release-gated emergence check that the
//! knowledge-ratchet scenario's innovator culture crosses
//! `KNOWLEDGE_RATCHET_MIN` and fires the event (Task 4). `tribes` absorbed
//! `knowledge-ratchet.toml` but seeds no Writing, so the scenario-backed tests
//! keep the retired file as an inline fixture (`common::fixtures`).

use anabios_core::codex::EventType;
use anabios_core::scenario::{Scenario, ScenarioError};
use anabios_core::tick::step;

mod common;

#[test]
fn knowledge_flag_requires_inventions() {
    // The schema defaults inventions on, so the invalid combination is an
    // explicit inventions opt-out with knowledge still on. Every other knob is
    // off: the 64-wide world's hash cells are too fine for disease (which
    // `parse_toml` would reject), and this pair is the only one under test.
    let bad = common::fixtures::with_opt_outs("name=\"k\"\nseed=1\nworld_size=64\ninventions_enabled=false\nknowledge_enabled=true\n[[agents]]\narchetype=\"grazer\"\ncount=4\n");
    let err =
        Scenario::parse_toml(&bad).expect_err("knowledge without inventions must be rejected");
    assert!(matches!(err, ScenarioError::KnowledgeNeedsInventions), "got {err}");
    let ok = common::fixtures::with_opt_outs("name=\"k\"\nseed=1\nworld_size=64\ninventions_enabled=true\nknowledge_enabled=true\n[[agents]]\narchetype=\"grazer\"\ncount=4\n");
    let w = Scenario::parse_toml(&ok).unwrap().instantiate();
    assert!(w.knowledge_enabled);
}

// Fixture: `tribes` absorbed knowledge-ratchet.toml but seeds only Stone Tools
// (no Writing holder at t0), so the retired Writing-seeded band is kept.
#[test]
fn scenario_instantiates_with_writing_held_from_t0() {
    use anabios_core::invention::{has, WRITING};
    let w = Scenario::parse_toml(&common::fixtures::knowledge_ratchet_flag_off())
        .expect("parse knowledge-ratchet fixture")
        .instantiate();
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
// Fixture: `tribes` (which absorbed inventions.toml) runs knowledge on; the
// pre-flip inventions world is the only one with inventions on and it off.
#[test]
fn flag_off_scenario_has_no_knowledge_ratchet() {
    let mut w = Scenario::parse_toml(&common::fixtures::inventions_flag_off())
        .expect("parse inventions fixture")
        .instantiate();
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
/// non-triviality precondition behind the knowledge fixture in
/// `save_load_roundtrip.rs::retired_state_fixtures::knowledge_ratchet_roundtrip`, which would
/// otherwise round-trip an empty map.
// Fixture: on `tribes` no species holds Writing within 300 ticks, so
// `knowledge_by_species` stays empty; the Writing-seeded band is kept.
#[test]
fn knowledge_accrues_per_species() {
    let w = common::world_after(&common::fixtures::knowledge_ratchet_flag_off(), 300);
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
// Fixture: on `tribes` (which seeds no Writing) KnowledgeRatchet fired in 0/5
// seeds by tick 2000; the retired Writing-seeded band is kept.
#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn knowledge_ratchet_emerges_across_seeds() {
    const SEEDS: u64 = 5;
    const TICKS: u32 = 2000;
    let mut fired = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(&common::fixtures::knowledge_ratchet_flag_off())
            .expect("parse knowledge-ratchet fixture");
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
