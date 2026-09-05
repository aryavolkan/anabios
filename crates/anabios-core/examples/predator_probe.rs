//! Why is a predator lineage failing? Steps a scenario and, every `every`
//! ticks, prints the second founder lineage's count, energy spread, thirst,
//! and the standing carcass count of the first lineage (a kill-rate proxy:
//! carcasses persist `CARCASS_DECAY_TICKS`).
//!
//! Run: `cargo run --release -p anabios-core --example predator_probe -- <scenario.toml> [ticks] [every] [seed]`

use anabios_core::agent::SPAWN_ENERGY;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let text = std::fs::read_to_string(&args[0]).expect("read scenario");
    let mut sc = Scenario::parse_toml(&text).expect("parse");
    let ticks: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let every: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(250);
    if let Some(seed) = args.get(3).and_then(|s| s.parse().ok()) {
        sc.seed = seed;
    }
    let mut w = sc.instantiate();
    println!(
        "SPAWN_ENERGY={SPAWN_ENERGY} max_population={} lineage_caps={:?}",
        w.max_population, w.lineage_caps
    );
    println!(
        "{:>6} {:>6} {:>8} {:>8} {:>8} {:>7} {:>10} {:>8}",
        "tick", "pred", "e_mean", "e_min", "e_max", "thirst", "prey_carc", "prey"
    );
    println!(
        "{:>6} {:>6} {:>8} {:>10} {:>9}",
        "", "newborn", "age_mean", "wants_mate", "same_seen"
    );
    for t in 0..=ticks {
        if t % every == 0 {
            let mut n = 0u32;
            let mut e_sum = 0.0f32;
            let mut e_min = f32::INFINITY;
            let mut e_max = f32::NEG_INFINITY;
            let mut th_sum = 0.0f32;
            let mut prey = 0u32;
            let mut newborn = 0u32; // younger than one report interval
            let mut age_sum = 0u64;
            let mut wants_mate = 0u32;
            let mut same_seen = 0u32; // has a pack-mate within perception
            for id in w.agents.iter_alive() {
                let i = id as usize;
                // Species 1 = first archetype spec (prey), 2 = second (predators);
                // splinters are ignored here — this is a coarse probe.
                match w.agents.species_id[i] {
                    2 => {
                        n += 1;
                        let e = w.agents.energy[i];
                        e_sum += e;
                        e_min = e_min.min(e);
                        e_max = e_max.max(e);
                        th_sum += w.agents.thirst[i];
                        let age = w.agents.age[i] as u64;
                        age_sum += age;
                        if age < every {
                            newborn += 1;
                        }
                        if w.actions.get(i).is_some_and(|a| a.mate_intent > 0.5) {
                            wants_mate += 1;
                        }
                        if w.sensors.get(i).is_some_and(|s| s.nearest_same_dist.is_finite()) {
                            same_seen += 1;
                        }
                    }
                    1 => prey += 1,
                    _ => {}
                }
            }
            let carc = w.carcasses.iter().filter(|c| c.species_id == 1).count();
            if n > 0 {
                println!(
                    "{t:>6} {n:>6} {:>8.1} {e_min:>8.1} {e_max:>8.1} {:>7.2} {carc:>10} {prey:>8}",
                    e_sum / n as f32,
                    th_sum / n as f32
                );
                println!(
                    "{:>6} {newborn:>6} {:>8.0} {wants_mate:>10} {same_seen:>9}",
                    "",
                    age_sum as f32 / n as f32
                );
            } else {
                println!(
                    "{t:>6} {n:>6} {:>8} {:>8} {:>8} {:>7} {carc:>10} {prey:>8}",
                    "-", "-", "-", "-"
                );
            }
        }
        if t < ticks {
            step(&mut w);
        }
    }
}
