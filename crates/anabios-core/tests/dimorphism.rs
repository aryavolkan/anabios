//! E12 sexual dimorphism: scenario wiring, sex demographics, flag-off
//! identity, and a release-gated emergence check that the dimorphism gene
//! evolves under predation + mate choice.

use anabios_core::codex::EventType;
use anabios_core::genome::GenomeSlot;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/dimorphism.toml");

fn mean_dimorphism(w: &anabios_core::world::World) -> f32 {
    let mut sum = 0.0;
    let mut n = 0u32;
    for id in w.agents.iter_alive() {
        sum += w.agents.genome[id as usize].get(GenomeSlot::SexualDimorphism);
        n += 1;
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f32
    }
}

fn sex_counts(w: &anabios_core::world::World) -> (u32, u32) {
    let mut male = 0u32;
    for id in w.agents.iter_alive() {
        if w.agents.sex[id as usize] {
            male += 1;
        }
    }
    (male, w.agents.live_count() - male)
}

#[test]
fn scenario_instantiates_with_both_sexes() {
    let w = Scenario::parse_toml(SCENARIO).expect("parse dimorphism").instantiate();
    assert!(w.sexual_dimorphism_enabled, "scenario must enable the flag");
    let (male, female) = sex_counts(&w);
    assert!(male > 0 && female > 0, "founders split across sexes: {male}M/{female}F");
    // 68 founders at p=0.5: a ratio more lopsided than 5:1 is a ~1e-9 event.
    let ratio = male.max(female) as f32 / male.min(female) as f32;
    assert!(ratio < 5.0, "founder sex ratio sane: {male}M/{female}F");
}

#[test]
fn both_sexes_persist_through_generations() {
    let mut w = Scenario::parse_toml(SCENARIO).expect("parse dimorphism").instantiate();
    for _ in 0..800 {
        step(&mut w);
    }
    let (male, female) = sex_counts(&w);
    assert!(male > 0 && female > 0, "both sexes persist at tick 800: {male}M/{female}F");
    assert!(w.agents.live_count() > 0, "population alive at tick 800");
}

#[test]
fn dimorphism_state_survives_save_load_step() {
    let mut world = Scenario::parse_toml(SCENARIO).expect("parse dimorphism").instantiate();
    for _ in 0..200 {
        step(&mut world);
    }
    let bytes = save_to_bytes(&world).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&world), state_hash(&reloaded), "load restores identical state");
    step(&mut world);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&world),
        state_hash(&reloaded),
        "dimorphic world diverged after save→load→step (hidden non-serialized state?)",
    );
}

#[test]
fn flag_off_scenario_has_no_sex_bits_set() {
    const MINIMAL: &str = include_str!("../../../scenarios/minimal.toml");
    let mut w = Scenario::parse_toml(MINIMAL).expect("parse minimal").instantiate();
    assert!(!w.sexual_dimorphism_enabled);
    for _ in 0..100 {
        step(&mut w);
    }
    assert!(
        w.agents.sex.not_any(),
        "flag off: every sex bit stays false (unread) even after births"
    );
}

/// Emergence: across seeds the prey lineage persists under predation with
/// both sexes present, and the dimorphism gene moves decisively away from
/// the seeded 0.5 mean (0.3/0.7 cohorts) — direction is ecological (mate
/// competition + female efficiency pull up during booms; metabolic thrift
/// pulls down at saturation), so the assertion is direction-free.
/// Release-gated per spec §testing.
#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn dimorphism_evolves_across_seeds() {
    const SEEDS: u64 = 8;
    const TICKS: u32 = 5000;
    let mut persisted = 0u64;
    let mut moved = 0u64;
    let mut sexsel_fired = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse dimorphism");
        s.seed = seed;
        let mut w = s.instantiate();
        let mut saw_sexsel = false;
        for _ in 0..TICKS {
            step(&mut w);
            for ev in w.codex.drain_events() {
                if ev.event_type == EventType::SexualSelection {
                    saw_sexsel = true;
                }
            }
        }
        let (male, female) = sex_counts(&w);
        if male > 0 && female > 0 && w.agents.live_count() > 20 {
            persisted += 1;
        }
        let mean_d = mean_dimorphism(&w);
        // Seeded mean is 0.5 (0.3 + 0.7 cohorts); selection + drift move it.
        // Only count seeds with enough survivors for the mean to be a
        // population signal rather than a handful of individuals.
        if w.agents.live_count() >= 20 && (mean_d - 0.5).abs() > 0.08 {
            moved += 1;
        }
        if saw_sexsel {
            sexsel_fired += 1;
        }
        eprintln!(
            "seed {seed}: alive={} M={male} F={female} mean_d={mean_d:.3} sexsel={saw_sexsel}",
            w.agents.live_count()
        );
    }
    assert!(persisted >= 6, "prey persists with both sexes in ≥6/8 seeds: {persisted}");
    assert!(moved >= 4, "dimorphism moves decisively from 0.5 in ≥4/8 seeds: {moved}");
    eprintln!("SexualSelection fired in {sexsel_fired}/{SEEDS} seeds");
}
