//! Population-scale proof that pinned OCEAN traits change behavior. Each test
//! runs two same-seed populations differing only in one pinned trait and
//! asserts the predicted difference in an aggregate metric.

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

fn run(toml: &str, ticks: u64) -> anabios_core::world::World {
    let mut w = Scenario::parse_toml(toml).expect("parse").instantiate();
    for _ in 0..ticks {
        step(&mut w);
    }
    w
}

/// Mean per-agent movement speed this tick (velocity magnitude). This is the
/// direct, torus-safe signature of Openness: open agents move faster, so they
/// disperse/range more. (A position-spread metric is unreliable here because
/// fast agents wrap the torus within the run, corrupting a naive centroid.)
fn mean_speed(w: &anabios_core::world::World) -> f32 {
    let ids: Vec<u32> = w.agents.iter_alive().collect();
    if ids.is_empty() {
        return 0.0;
    }
    let mut s = 0.0;
    for &id in &ids {
        s += w.agents.velocity[id as usize].length();
    }
    s / ids.len() as f32
}

fn mean_crowding(w: &anabios_core::world::World) -> f32 {
    let ids: Vec<u32> = w.agents.iter_alive().collect();
    if ids.is_empty() {
        return 0.0;
    }
    let mut s = 0.0;
    for &id in &ids {
        s += w.sensors[id as usize].crowding as f32;
    }
    s / ids.len() as f32
}

fn mean_energy(w: &anabios_core::world::World) -> f32 {
    let ids: Vec<u32> = w.agents.iter_alive().collect();
    if ids.is_empty() {
        return 0.0;
    }
    let mut s = 0.0;
    for &id in &ids {
        s += w.agents.energy[id as usize];
    }
    s / ids.len() as f32
}

fn scenario(trait_line: &str) -> String {
    format!(
        "name = \"p\"\nseed = 7\n\n[[agents]]\ncount = 120\nplacement = {{ kind = \"cluster\", center_x = 512.0, center_y = 512.0, radius = 80.0 }}\n[agents.traits]\n{trait_line}\n"
    )
}

#[test]
fn openness_increases_movement() {
    let hi = run(&scenario("openness = 0.95"), 100);
    let lo = run(&scenario("openness = 0.05"), 100);
    let (sh, sl) = (mean_speed(&hi), mean_speed(&lo));
    assert!(sh > sl, "high-O mean speed {sh} should exceed low-O {sl}");
}

#[test]
fn extraversion_increases_clustering() {
    let hi = run(&scenario("extraversion = 0.95"), 300);
    let lo = run(&scenario("extraversion = 0.05"), 300);
    let (ch, cl) = (mean_crowding(&hi), mean_crowding(&lo));
    assert!(ch > cl, "high-E crowding {ch} should exceed low-E {cl}");
}

#[test]
fn conscientiousness_raises_mean_energy() {
    let hi = run(&scenario("conscientiousness = 0.95"), 300);
    let lo = run(&scenario("conscientiousness = 0.05"), 300);
    let (eh, el) = (mean_energy(&hi), mean_energy(&lo));
    assert!(eh > el, "high-C mean energy {eh} should exceed low-C {el}");
}
