//! Stage 2 of the cultural-inventions experiment (the ambitious reach): in ONE
//! interbreeding population seeded ~15% with the `Inventiveness` gene (+ a
//! Communicator), does the gene frequency rise toward fixation — gene-culture
//! coevolution from standing variation, the bar the prior first-principles
//! experiment B failed?
//!
//! Run: cargo test -p anabios-core --release --test inventions_sweep -- --ignored --nocapture
//! Env: IS_SEEDS / IS_TICKS / IS_N override defaults.
//!
//! RESULT (2026-07-18) — POSITIVE and confound-controlled. The Inventiveness
//! gene sweeps from 15% standing variation toward fixation among surviving runs
//! (boom/bust extinctions occur in the harsh 2048 living+seasonal biome; the
//! sweep is read on runs that survive). With the naive default seeding, 6 seeds
//! x 5000 ticks gives 5/6 rising 0.15 -> 0.83..1.00.
//!
//! BUT the naive seeding is CONFOUNDED: the seeded gene-carriers also carry a
//! Communicator module, and `feed_pass`'s experiment-C foraging-skill bonus is
//! gated on that module (not the gene), giving carriers an independent edge.
//! Two controls settle the causal attribution.
//!
//! Control 1 — IS_INVENTIONS=0 (invention mechanism OFF, Communicator kept):
//! the gene STILL sweeps in most seeds (final freq ~0.06..1.00, variable), so
//! the Communicator/skill bonus drives much of the NAIVE sweep — the gene
//! hitchhikes on the module. This confirms the confound is real.
//!
//! Control 2 — IS_ALL_COMM=1 (EVERY agent gets a Communicator, so the skill
//! bonus is EQUAL across cohorts and the ONLY difference is the Inventiveness
//! gene): the gene STILL sweeps to fixation in every surviving seed (0.948,
//! 0.991, 0.986; the rest went extinct). This isolates the invention benefit
//! as a genuine, sufficient cause of the sweep, independent of the Communicator.
//!
//! Conclusion: with the module confound controlled, the ROBUST CUMULATIVE
//! invention benefit (additive domestication food plus industry upkeep/repro
//! discounts, compounded by Writing-accelerated transmission — none of which
//! saturate in abundance) drives the Inventiveness gene to fixation. This is
//! gene-culture coevolution attributable to inventions — the durable,
//! non-saturating cultural benefit the prior work (experiment B, the saturating
//! foraging-skill multiplier; see tests/living_sandbox.rs) concluded was the
//! missing ingredient.
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::module::starter_kit;
use anabios_core::prelude_test::Vec2;
use anabios_core::program::starter_asocial_forager;
use anabios_core::tick::step;
use anabios_core::world::World;

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Fraction of the alive population whose `Inventiveness` gene is inventive
/// (>0.5), and the alive count.
fn gene_freq(w: &World) -> (usize, f64) {
    let (mut n, mut c) = (0usize, 0usize);
    for id in w.agents.iter_alive() {
        let i = id as usize;
        n += 1;
        if w.agents.genome[i].get(GenomeSlot::Inventiveness) > 0.5 {
            c += 1;
        }
    }
    (n, if n > 0 { c as f64 / n as f64 } else { 0.0 })
}

#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn inventiveness_gene_sweep() {
    let seeds = env_u64("IS_SEEDS", 6);
    let ticks = env_u64("IS_TICKS", 6000) as u32;
    let n = env_u64("IS_N", 300) as usize;
    const INIT_FRAC: f64 = 0.15;
    eprintln!(
        "inventions gene sweep: {seeds} seeds x {ticks} ticks, N={n}, init inventive={INIT_FRAC}"
    );

    for seed in 0..seeds {
        // A 2048 living + seasonal world with inventions ON (same substrate as
        // the Stage-1 scenario), built directly so we can seed one mixed species.
        let mut w = World::with_dims(seed, 2048.0, 256, 128);
        w.living_biome = true;
        w.season_period = 2000;
        // Control knob: IS_INVENTIONS=0 disables the invention mechanism while
        // keeping the identical seeding (15% carry the gene + a Communicator).
        // If the gene STILL sweeps with inventions off, the sweep is NOT caused
        // by inventions (a falsification control); it must stay flat/fall.
        w.cultural_inventions = env_u64("IS_INVENTIONS", 1) != 0;
        w.max_population = 8000;
        // One interbreeding species (id 0), clustered near the world centre.
        // ~15% carry the Inventiveness gene AND a Communicator; the rest neither.
        for k in 0..n {
            let ang = k as f32 * 0.7;
            let rad = 20.0 + (k % 17) as f32 * 12.0;
            let pos = Vec2::new(1024.0 + rad * ang.cos(), 1024.0 + rad * ang.sin());
            let mut g = Genome::neutral();
            g.set(GenomeSlot::ReproductionThreshold, 0.3);
            let inventive = (k as f64) / (n as f64) < INIT_FRAC;
            let mut kit = starter_kit();
            if inventive {
                g.set(GenomeSlot::Inventiveness, 1.0);
            }
            // Confound control: IS_ALL_COMM=1 gives EVERY agent a Communicator, so
            // the experiment-C foraging-skill bonus (module-gated) is equal across
            // cohorts and the ONLY difference is the Inventiveness gene. A sweep
            // under this condition is attributable to inventions, not the module.
            let all_comm = env_u64("IS_ALL_COMM", 0) != 0;
            if inventive || all_comm {
                kit.push(anabios_core::module::Module::Communicator { range: 12.0, channel_id: 0 });
            }
            w.spawn_seeded(pos, g, 0, kit, starter_asocial_forager());
        }
        let (n0, f0) = gene_freq(&w);
        for t in 0..ticks {
            step(&mut w);
            if t % 2000 == 1999 {
                let (np, fp) = gene_freq(&w);
                eprintln!("  seed{seed} t{}: pop={np} inventive_gene_freq={fp:.3}", t + 1);
            }
        }
        let (n1, f1) = gene_freq(&w);
        eprintln!(
            "SEED{seed}: pop {n0}->{n1}, inventive_gene_freq {f0:.3}->{f1:.3} ({})",
            if f1 > f0 + 0.02 {
                "ROSE"
            } else if f1 < f0 - 0.02 {
                "fell"
            } else {
                "flat"
            }
        );
    }
}
