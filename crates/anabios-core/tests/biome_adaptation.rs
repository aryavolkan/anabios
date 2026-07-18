//! With biome_adaptation on, populations evolve a spatial EnvAffinity cline
//! matched to the local climate — agents in high-climate cells carry higher
//! affinity than those in low-climate cells (in-place local adaptation).

use anabios_core::genome::GenomeSlot;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/biome-adaptation.toml");

#[test]
fn affinity_cline_tracks_local_climate() {
    let mut w = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    assert!(w.biome_adaptation);
    for _ in 0..2500 {
        step(&mut w);
    }
    // Bucket alive agents by their local cell env (low half < 0.5 <= high half).
    let (mut lo_sum, mut lo_n, mut hi_sum, mut hi_n) = (0.0f32, 0u32, 0.0f32, 0u32);
    for id in w.agents.iter_alive() {
        let i = id as usize;
        let env = w.biome.sample(w.agents.position[i]).env;
        let aff = w.agents.genome[i].get(GenomeSlot::EnvAffinity);
        if env < 0.5 {
            lo_sum += aff;
            lo_n += 1;
        } else {
            hi_sum += aff;
            hi_n += 1;
        }
    }
    assert!(lo_n > 0 && hi_n > 0, "need agents in both climate halves ({lo_n}/{hi_n})");
    let lo_mean = lo_sum / lo_n as f32;
    let hi_mean = hi_sum / hi_n as f32;
    assert!(
        hi_mean > lo_mean,
        "high-climate agents should carry higher EnvAffinity than low-climate: hi={hi_mean} lo={lo_mean}"
    );
}
