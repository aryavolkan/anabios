//! Gait on a curated world (`gait_enabled`, `src/gait.rs`): the population
//! no longer moves at its Locomotor maximum every tick — the mean step over
//! alive agents falls below half of `SPEED_MAX_CAP` — while the agents that
//! need their top speed (fleeing prey, hunting pursuers) still reach it. The
//! same world with the flag off is measured alongside as the "before".

use anabios_core::integrate::{top_speed, SPEED_MAX_CAP};
use anabios_core::invention::held_mask;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

mod common;

const SCENARIO: &str = include_str!("../../../scenarios/predator-prey.toml");
/// Population cap, set before `instantiate` so each lineage's `max_share`
/// scales with it; keeps the debug-profile run short (the claim is per agent).
const POP_CAP: u32 = 500;
/// Ticks stepped before sampling — past the founders' spawn-tick stillness
/// and into the first hunts.
const WARM: u64 = 300;
/// Ticks sampled after the warm-up.
const SAMPLE: u64 = 50;

struct Steps {
    /// Mean applied step (world units) over every alive agent-tick sampled.
    mean: f32,
    /// Agent-ticks whose step reached at least 0.9 × that agent's top speed.
    sprinters: usize,
    agent_ticks: usize,
    /// Largest step / top-speed fraction seen.
    max_frac: f32,
}

fn measure(gait_on: bool) -> Steps {
    let mut s = Scenario::parse_toml(SCENARIO).expect("parse predator-prey");
    s.max_population = Some(POP_CAP);
    let mut w = s.instantiate();
    w.gait_enabled = gait_on;
    common::run(&mut w, common::ticks(WARM));
    let (mut sum, mut n, mut sprinters, mut max_frac) = (0.0f64, 0usize, 0usize, 0.0f32);
    for _ in 0..common::ticks(SAMPLE) {
        step(&mut w);
        for id in w.agents.iter_alive() {
            let i = id as usize;
            let v = w.agents.velocity[i].length();
            let top = top_speed(
                &w.agents.modules[i],
                &w.agents.genome[i],
                held_mask(&w.agents.meme_vector[i]),
                &w.agents.affect[i],
                w.gene_tech_coupling,
            );
            sum += v as f64;
            n += 1;
            if top > 0.0 {
                let frac = v / top;
                max_frac = max_frac.max(frac);
                if frac >= 0.9 {
                    sprinters += 1;
                }
            }
        }
    }
    Steps { mean: (sum / n.max(1) as f64) as f32, sprinters, agent_ticks: n, max_frac }
}

#[test]
fn gait_slows_the_population_while_urgent_agents_still_sprint() {
    let off = measure(false);
    let on = measure(true);
    eprintln!(
        "predator-prey (cap {POP_CAP}), ticks {WARM}..{}: mean step over alive agents \
         off {:.3} -> on {:.3} world units (SPEED_MAX_CAP {SPEED_MAX_CAP}); agent-ticks at \
         >= 0.9 x top speed off {}/{} -> on {}/{}; largest fraction on {:.3}",
        WARM + SAMPLE,
        off.mean,
        on.mean,
        off.sprinters,
        off.agent_ticks,
        on.sprinters,
        on.agent_ticks,
        on.max_frac,
    );
    assert!(on.agent_ticks > 0, "the world must still be populated");
    assert!(
        on.mean < 0.5 * SPEED_MAX_CAP,
        "gait on: mean step {} must be below half of SPEED_MAX_CAP ({})",
        on.mean,
        0.5 * SPEED_MAX_CAP
    );
    assert!(on.mean < off.mean, "gait must slow the population: off {} on {}", off.mean, on.mean);
    assert!(
        on.sprinters > 0,
        "some agents must still reach 0.9 x their top speed (largest fraction {})",
        on.max_frac
    );
}
