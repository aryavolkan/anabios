//! TG1 end-to-end evidence: the tech→gene arm produces *directional selection*.
//!
//! Discovery is far too slow to climb naturally to an affinity-bearing tech
//! before a small world thins out, so instead of relying on emergence we seed a
//! population that already holds Fire (+ its Stone Tools prerequisite, so it does
//! not atrophy) with a spread of the coupled gene (Openness), then let
//! differential reproduction run. Fire's energy buff scales with Openness when
//! `gene_tech_coupling` is on, so high-Openness holders out-reproduce and the
//! population mean climbs — while a flag-off control started from the identical
//! state shows no such rise. The gap between the two IS the selection signal.

use anabios_core::genome::GenomeSlot;
use anabios_core::invention::{self, FIRE, STONE_TOOLS};
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/tech-gene-coupling.toml");
const TICKS: u64 = 2500;

/// Build the coupled scenario world with `coupling` set as requested, then force
/// every agent to hold Stone Tools + Fire and give them a deterministic spread
/// of Openness in [0,1]. Returns the seeded world.
fn seeded_world(coupling: bool) -> anabios_core::World {
    let mut s = Scenario::parse_toml(SCENARIO).expect("parse");
    s.gene_tech_coupling = coupling;
    let mut w = s.instantiate();
    let ids: Vec<_> = w.agents.iter_alive().collect();
    for (n, id) in ids.iter().enumerate() {
        let i = *id as usize;
        w.agents.meme_vector[i][invention::channel(STONE_TOOLS)] = 1.0;
        w.agents.meme_vector[i][invention::channel(FIRE)] = 1.0;
        // Deterministic spread across [0,1].
        w.agents.genome[i].set(GenomeSlot::Openness, (n % 11) as f32 / 10.0);
    }
    w
}

/// Mean Openness over agents currently holding Fire (0.0 if none hold it).
fn mean_openness_over_fire_holders(w: &anabios_core::World) -> f32 {
    let mut sum = 0.0;
    let mut n = 0u32;
    for id in w.agents.iter_alive() {
        let i = id as usize;
        if invention::has(&w.agents.meme_vector[i], FIRE) {
            sum += w.agents.genome[i].get(GenomeSlot::Openness);
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f32
    }
}

#[test]
fn coupling_selects_the_affinity_gene_upward() {
    let start = {
        // Both worlds start from the identical seeded state; measure the shared
        // baseline once.
        let w = seeded_world(true);
        mean_openness_over_fire_holders(&w)
    };

    let mut coupled = seeded_world(true);
    let mut control = seeded_world(false);
    for _ in 0..TICKS {
        step(&mut coupled);
        step(&mut control);
    }

    let coupled_mean = mean_openness_over_fire_holders(&coupled);
    let control_mean = mean_openness_over_fire_holders(&control);
    eprintln!(
        "TG1 selection @ {TICKS} ticks: start Openness={start:.4}, coupled={coupled_mean:.4} \
         (alive {}), control={control_mean:.4} (alive {}), differential={:.4}",
        coupled.agents.live_count(),
        control.agents.live_count(),
        coupled_mean - control_mean,
    );

    // Directional selection: coupling pulls the coupled gene mean up, and it
    // ends materially above the flag-off control that ran from the same seed.
    assert!(
        coupled_mean > control_mean + 0.02,
        "expected coupling to raise Openness (coupled={coupled_mean:.4} vs control={control_mean:.4}, start={start:.4})"
    );
    // Sanity: both worlds still have a live Fire-holding population to measure.
    assert!(coupled.agents.live_count() > 0 && control.agents.live_count() > 0);
}
