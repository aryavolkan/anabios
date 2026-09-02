//! Disease subsystem (flag `disease_enabled`): scenario wiring, flag-off
//! no-op, end-to-end outbreak in a crowded world, medicine A/B recovery,
//! sparse-world negative, save/load round-trip, and the release-gated
//! emergence check that `scenarios/disease.toml` fires `EpidemicOutbreak`
//! across seeds (spec: `docs/superpowers/specs/2026-09-01-disease-epidemiology-design.md`).

use anabios_core::codex::EventType;
use anabios_core::genome::Genome;
use anabios_core::prelude_test::Vec2;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;
use anabios_core::world::World;

const SCENARIO: &str = include_str!("../../../scenarios/disease.toml");

/// Tight cluster of `n` same-species agents around (512, 512).
fn clustered_world(seed: u64, n: usize, disease: bool) -> World {
    let mut w = World::new(seed);
    w.disease_enabled = disease;
    for k in 0..n {
        let angle = k as f32 * 2.399_963; // golden-angle spiral, spacing ~1 unit
        let r = 0.5 * (k as f32).sqrt();
        w.spawn_agent(
            Vec2::new(512.0 + r * angle.cos(), 512.0 + r * angle.sin()),
            Genome::neutral(),
        );
    }
    w
}

fn infect_fraction(w: &mut World, frac: f32, intensity: f32) {
    let ids: Vec<u32> = w.agents.iter_alive().collect();
    let n_infect = (ids.len() as f32 * frac).round() as usize;
    for id in ids.into_iter().take(n_infect) {
        w.agents.infection[id as usize] = intensity;
    }
}

fn total_infection(w: &World) -> f32 {
    w.agents.iter_alive().map(|id| w.agents.infection[id as usize]).sum()
}

fn count_events(w: &World, t: EventType) -> usize {
    w.codex.events.iter().filter(|e| e.event_type == t).count()
}

#[test]
fn scenario_instantiates_with_flags_and_medicine_held() {
    use anabios_core::invention::{has, MEDICINE};
    let w = Scenario::parse_toml(SCENARIO).expect("parse disease").instantiate();
    assert!(w.disease_enabled && w.inventions_enabled);
    // Two bands: 150 susceptible grazers (no medicine) + 60 innovators
    // (seeded with the full era-3 chain). Exactly the innovators hold Medicine.
    let mut holders = 0;
    for id in w.agents.iter_alive() {
        if has(&w.agents.meme_vector[id as usize], MEDICINE) {
            holders += 1;
        }
    }
    assert_eq!(w.agents.live_count(), 210, "grazer herd + innovator band");
    assert_eq!(holders, 60, "only the innovator band holds Medicine at t0");
}

#[test]
fn outbreak_fires_end_to_end_in_crowded_world() {
    let mut w = clustered_world(3, 40, true);
    // Infect a wide margin: reproduction adds susceptibles each tick, diluting
    // the fraction — 90% of the founders keeps it ≥ OUTBREAK_FRACTION.
    infect_fraction(&mut w, 0.9, 0.6);
    for _ in 0..3 {
        step(&mut w);
    }
    assert!(
        count_events(&w, EventType::EpidemicOutbreak) >= 1,
        "crowded infected world must fire EpidemicOutbreak"
    );
}

#[test]
fn flag_off_is_noop() {
    let mut w = clustered_world(3, 40, false);
    infect_fraction(&mut w, 0.5, 0.6);
    for _ in 0..3 {
        step(&mut w);
    }
    assert_eq!(
        count_events(&w, EventType::EpidemicOutbreak)
            + count_events(&w, EventType::MedicineContainment),
        0,
        "flag off: no disease events"
    );
    // The stage early-returns: infection is never read or written.
    assert!((total_infection(&w) - 40.0 * 0.5 * 0.6).abs() < 1e-3, "flag off: infection untouched");
}

#[test]
fn medicine_holders_recover_faster() {
    use anabios_core::invention::{HELD_THRESHOLD, INVENTION_CHANNEL_BASE, MEDICINE};

    let mut holders = clustered_world(7, 40, true);
    let mut plain = clustered_world(7, 40, true);
    let ch = INVENTION_CHANNEL_BASE + MEDICINE;
    let ids: Vec<u32> = holders.agents.iter_alive().collect();
    for id in ids {
        holders.agents.meme_vector[id as usize][ch] = HELD_THRESHOLD;
    }
    infect_fraction(&mut holders, 1.0, 0.6);
    infect_fraction(&mut plain, 1.0, 0.6);

    for _ in 0..10 {
        step(&mut holders);
        step(&mut plain);
    }
    let h = total_infection(&holders);
    let p = total_infection(&plain);
    assert!(
        h < p * 0.7,
        "medicine holders must recover materially faster: holders={h:.2} vs plain={p:.2}"
    );
}

#[test]
fn sparse_world_no_spillover_no_events() {
    let mut w = World::new(9);
    w.disease_enabled = true;
    for k in 0..8u32 {
        // 100 units apart — far beyond SPILLOVER_RADIUS; never crowded.
        w.spawn_agent(Vec2::new(100.0 + k as f32 * 100.0, 500.0), Genome::neutral());
    }
    for _ in 0..500 {
        step(&mut w);
    }
    assert_eq!(total_infection(&w), 0.0, "sparse world: no spillover");
    assert_eq!(
        count_events(&w, EventType::EpidemicOutbreak)
            + count_events(&w, EventType::MedicineContainment),
        0,
        "sparse world: no disease events"
    );
}

#[test]
fn disease_state_survives_save_load_step() {
    let mut w = clustered_world(11, 40, true);
    infect_fraction(&mut w, 0.5, 0.6);
    for _ in 0..100 {
        step(&mut w);
    }
    assert!(total_infection(&w) > 0.0, "infection should be non-trivial after 100 ticks");
    let bytes = save_to_bytes(&w).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&w), state_hash(&reloaded), "load restores identical state");
    step(&mut w);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&w),
        state_hash(&reloaded),
        "disease world diverged after save→load→step",
    );
}

/// Emergence: the dense seeded medicine culture spills over, outbreaks, and
/// resolves — `EpidemicOutbreak` must fire across seeds. Release-gated:
/// spillover is rare per-tick (`SPILLOVER_P`), so this needs the long horizon.
#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn epidemic_outbreak_emerges_across_seeds() {
    const SEEDS: u64 = 5;
    const TICKS: u32 = 2000;
    let mut fired = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse disease");
        s.seed = seed;
        let mut w = s.instantiate();
        let mut saw_outbreak = false;
        let mut first_tick = None;
        for _ in 0..TICKS {
            step(&mut w);
            for ev in w.codex.drain_events() {
                if ev.event_type == EventType::EpidemicOutbreak && !saw_outbreak {
                    saw_outbreak = true;
                    first_tick = Some(ev.tick);
                }
            }
        }
        if saw_outbreak {
            fired += 1;
        }
        eprintln!(
            "seed {seed}: alive={} outbreak={saw_outbreak} first_tick={first_tick:?}",
            w.agents.live_count()
        );
    }
    assert!(fired >= 3, "EpidemicOutbreak fired in ≥3/{SEEDS} seeds: {fired}");
}
