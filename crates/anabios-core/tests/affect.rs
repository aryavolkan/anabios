//! End-to-end determinism for the flag-ON affect scenario. `determinism.rs`
//! locks the flag-OFF minimal scenario; this pins the affect layer's real
//! behavior (SEEKING-biased foraging) so it cannot drift silently, and proves
//! the serialized `affect` column survives a save→load→step round-trip.

use anabios_core::affect::RAGE;
use anabios_core::agent::SPAWN_ENERGY;
use anabios_core::codex::EventType;
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude_test::{reassign_to_new_species, Vec2};
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;
use anabios_core::world::World;

const SCENARIO: &str = include_str!("../../../scenarios/affect-seeking.toml");

#[test]
fn affect_scenario_parses_with_flag_on() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect scenario");
    assert!(s.affect_enabled, "scenario must enable the affect layer");
}

#[test]
fn affect_scenario_is_self_consistent() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse");
    let run = |ticks: u64| {
        let mut w = s.instantiate();
        for _ in 0..ticks {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(300), run(300), "same seed + flag on → bit-identical");
}

#[test]
fn affect_scenario_survives_save_load_step() {
    let mut world = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    assert!(world.affect_enabled);
    // Warm the world so SEEKING activations accumulate before the snapshot.
    for _ in 0..300 {
        step(&mut world);
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "load must restore identical state (affect column persisted)"
    );
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "affect world diverged after save→load→step (non-serialized affect state?)"
    );
}

/// Pinned flag-ON golden. Generated once with `UPDATE_HASHES=1` after the
/// SEEKING layer was wired; regenerate deliberately whenever an affect change
/// is intentional.
// Refreshed 2026-08-03 (M-C RAGE+LUST): agonistic + reproductive systems now act
// flag-on — RAGE bias (fire/approach), LUST bias (approach) + lowered repro gate,
// FEAR⊣RAGE inhibition, combat→RAGE impulse. A genuine flag-on behavior change
// (NOT layout): no serialized column added, FORMAT_VERSION unchanged; flag-off
// (minimal/cognition/inventions) byte-identical.
// Refreshed 2026-08-04 (M-D CARE+PANIC, FORMAT_VERSION 27→28): affect_enabled is
// true here, so CARE/PANIC now genuinely compute (kin proximity/isolation among
// the 200 same-species agents) and PANIC⊣SEEK inhibition damps SEEK — a real
// flag-on trajectory change, layered on the affect_prev_crowding layout growth
// (moved the flag-off minimal/cognition/inventions goldens too, see snapshot.rs v28).
// Refreshed 2026-08-04 (M-E PLAY): juveniles with same-species peers now accrue
// PLAY (movement approach-bias); a real flag-on behavior change (NO layout growth,
// no new serialized column, FORMAT_VERSION unchanged). tick-0 hash is unchanged
// (PLAY has not accrued yet); ticks 100/300 move. Flag-off goldens unaffected.
const AFFECT_GOLDEN: &[(u64, u64)] =
    &[(0, 0x3b9688a730461dc4), (100, 0x1d5c9c3005cc27df), (300, 0x0febbab089322258)];

#[test]
fn affect_scenario_matches_golden_hashes() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect scenario");
    let mut w = s.instantiate();
    let max_tick = AFFECT_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < AFFECT_GOLDEN.len() && AFFECT_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        if w.tick == max_tick {
            break;
        }
        step(&mut w);
    }
    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// regenerated affect hashes:");
        for (t, h) in &observed {
            println!("    ({t}, 0x{h:016x}),");
        }
        return;
    }
    for ((exp_tick, exp_hash), (got_tick, got_hash)) in AFFECT_GOLDEN.iter().zip(&observed) {
        assert_eq!(exp_tick, got_tick, "tick mismatch");
        assert_eq!(
            *exp_hash, *got_hash,
            "affect hash drift at tick {exp_tick}: expected 0x{exp_hash:016x}, got 0x{got_hash:016x}.\n\
             If intentional, rerun with UPDATE_HASHES=1 and copy the printed values.",
        );
    }
}

// --- M-B: FEAR / hijack flag-on tests (affect-threat scenario) ---

const THREAT_SCENARIO: &str = include_str!("../../../scenarios/affect-threat.toml");

#[test]
fn affect_threat_parses_with_flag_on() {
    let s = Scenario::parse_toml(THREAT_SCENARIO).expect("parse affect-threat");
    assert!(s.affect_enabled, "scenario must enable the affect layer");
}

#[test]
fn affect_threat_is_self_consistent() {
    let s = Scenario::parse_toml(THREAT_SCENARIO).expect("parse affect-threat");
    let run = || {
        let mut w = s.instantiate();
        for _ in 0..300 {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(), run(), "same seed + flag on ⇒ bit-identical");
}

#[test]
fn affect_threat_survives_save_load_step() {
    let s = Scenario::parse_toml(THREAT_SCENARIO).expect("parse affect-threat");
    let mut world = s.instantiate();
    for _ in 0..300 {
        step(&mut world);
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&world), state_hash(&reloaded), "load must restore identical state");
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "affect world diverged after save→load→step (hidden non-serialized affect state?)",
    );
}

#[test]
fn affect_threat_emits_mass_fright() {
    let s = Scenario::parse_toml(THREAT_SCENARIO).expect("parse affect-threat");
    let mut w = s.instantiate();
    let mut saw = false;
    for _ in 0..1500 {
        step(&mut w);
        if w.codex.events.iter().any(|e| e.event_type == EventType::MassFright) {
            saw = true;
            break;
        }
    }
    assert!(saw, "predator-driven FEAR must produce a MassFright within 1500 ticks");
}

/// Pinned golden for the flag-ON affect-threat scenario. Regenerate deliberately
/// with `UPDATE_HASHES=1` whenever an affect-behavior change is intentional.
// Created 2026-08-03 (M-B): first pin of the FEAR/hijack layer's real behavior.
// Refreshed 2026-08-03 (M-C RAGE+LUST): agonistic + reproductive systems act
// flag-on in this predator scenario too (RAGE from combat/frustration, LUST).
// Behavior change, not layout; FORMAT_VERSION unchanged.
// Refreshed 2026-08-04 (M-D CARE+PANIC, FORMAT_VERSION 27→28): affect_enabled is
// true here, so CARE/PANIC now genuinely compute (grazer herd kinship/crowding)
// and PANIC⊣SEEK inhibition damps SEEK — a real flag-on trajectory change,
// layered on the affect_prev_crowding layout growth.
// Refreshed 2026-08-04 (M-E PLAY): juvenile grazers near same-species peers accrue
// PLAY (movement approach-bias) — flag-on behavior change, no layout growth.
const THREAT_GOLDEN: &[(u64, u64)] =
    &[(0, 0x2a5f2e53b8351c8c), (100, 0x8cbe01fdcaf00bdc), (300, 0xd46da3afcdaadb06)];

#[test]
fn affect_threat_matches_golden_hashes() {
    let s = Scenario::parse_toml(THREAT_SCENARIO).expect("parse affect-threat");
    let mut w = s.instantiate();
    let max_tick = THREAT_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < THREAT_GOLDEN.len() && THREAT_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        if w.tick == max_tick {
            break;
        }
        step(&mut w);
    }
    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// regenerated affect-threat hashes:");
        for (t, h) in &observed {
            println!("    ({t}, 0x{h:016x}),");
        }
        return;
    }
    for ((et, eh), (gt, gh)) in THREAT_GOLDEN.iter().zip(&observed) {
        assert_eq!(et, gt, "tick mismatch");
        assert_eq!(
            *eh, *gh,
            "affect-threat hash drift at tick {et}: expected 0x{eh:016x}, got 0x{gh:016x}"
        );
    }
}

// --- M-C: RAGE / LUST behavior + determinism tests ---

/// Frustration (hungry + crowded) with an other-species neighbour present
/// drives RAGE up over a few ticks, which raises the agent's fire_intent.
#[test]
fn frustration_raises_fire_intent() {
    let mut w = World::new(7);
    w.affect_enabled = true;
    // A frustrated agent surrounded by an other-species crowd.
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Aggressiveness, 1.0);
    let me = w.spawn_agent(Vec2::new(500.0, 500.0), g) as usize;
    w.agents.energy[me] = 0.05 * SPAWN_ENERGY; // deep deficit → high drive
                                               // Ring of other-species neighbours to create crowding + an other target.
    for k in 0..6 {
        let other = w.spawn_agent(Vec2::new(500.5 + k as f32 * 0.3, 500.0), Genome::neutral());
        // Register the neighbour in a fresh species (a bare `species_id = 1`
        // write is unregistered — the species aggregation panics on it).
        reassign_to_new_species(&mut w, other);
    }
    let mut prev_rage = 0.0;
    for _ in 0..10 {
        step(&mut w);
        prev_rage = w.agents.affect[me][RAGE];
        if !w.agents.is_alive(me as u32) {
            break;
        }
    }
    assert!(prev_rage > 0.0, "frustrated, crowded agent accrues RAGE: {prev_rage}");
    // With RAGE up and an other-species target sensed, its action fires.
    assert!(w.actions[me].fire_intent > 0.0, "RAGE drives fire_intent");
}

/// The affect column (RAGE/LUST) round-trips through save→load→step.
#[test]
fn affect_survives_save_load_step() {
    let mut w = World::new(11);
    w.affect_enabled = true;
    // Seed a couple of agents and warm up so affect is non-zero.
    for k in 0..8 {
        let id = w.spawn_agent(Vec2::new(480.0 + k as f32, 500.0), Genome::neutral());
        if k % 2 == 1 {
            reassign_to_new_species(&mut w, id);
        }
    }
    for _ in 0..50 {
        step(&mut w);
    }
    let bytes = save_to_bytes(&w).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&w), state_hash(&reloaded), "load restores identical state");
    step(&mut w);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&w),
        state_hash(&reloaded),
        "affect world diverged after save→load→step (hidden non-serialized affect state?)",
    );
}
