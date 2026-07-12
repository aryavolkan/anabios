//! Gene-culture coevolution EXPERIMENT (A: validate the mechanism).
//!
//! Two lineages compete under scarcity: culture-users (Communicator gene + a
//! program that shares with kin only when the cooperation MEME is also present)
//! vs. asocial grazers. If culture is adaptive, the culture-users' share of the
//! population should rise (the meme's benefit selecting the Communicator gene).
//!
//! This is an analysis harness, not a pass/fail gate — run with:
//!   cargo test -p anabios-core --release --test gene_culture -- --nocapture --ignored

// Experiment labels (A/B/C) are meaningful shorthand for the design variants and
// intentionally capitalized in test names.
#![allow(non_snake_case)]

use anabios_core::module::{has, ModuleType};
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/gene-culture.toml");

/// Per-species snapshot: alive count, mean meme[2] (the cooperation norm),
/// fraction carrying a Communicator module (the culture-enabling gene).
fn snapshot(w: &anabios_core::world::World) -> std::collections::BTreeMap<u32, (u32, f32, f32)> {
    let mut acc: std::collections::BTreeMap<u32, (u32, f64, u32)> =
        std::collections::BTreeMap::new();
    for id in w.agents.iter_alive() {
        let i = id as usize;
        let sid = w.agents.species_id[i];
        let e = acc.entry(sid).or_insert((0, 0.0, 0));
        e.0 += 1;
        e.1 += w.agents.meme_vector[i][5] as f64;
        if has(&w.agents.modules[i], ModuleType::Communicator) {
            e.2 += 1;
        }
    }
    acc.into_iter()
        .map(|(sid, (n, meme, comm))| {
            (sid, (n, (meme / n.max(1) as f64) as f32, comm as f32 / n.max(1) as f32))
        })
        .collect()
}

#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn gene_culture_coevolution_A() {
    const SEEDS: u64 = 12;
    const TICKS: u32 = 1500;
    // Species 1 = cultural_cooperator (first archetype spec → fresh id 1),
    // species 2 = asocial grazer.
    let mut coop_wins = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse gene-culture");
        s.seed = seed;
        let mut w = s.instantiate();
        // Record the starting counts.
        let start = snapshot(&w);
        let c0 = start.get(&1).map(|t| t.0).unwrap_or(0);
        let a0 = start.get(&2).map(|t| t.0).unwrap_or(0);
        for t in 0..TICKS {
            step(&mut w);
            if t % 300 == 299 {
                let s = snapshot(&w);
                let c = s.get(&1).copied().unwrap_or((0, 0.0, 0.0));
                let a = s.get(&2).copied().unwrap_or((0, 0.0, 0.0));
                eprintln!(
                    "seed{seed} t{}: coop n={} meme2={:.2} comm={:.2} | asocial n={} | total={}",
                    t + 1,
                    c.0,
                    c.1,
                    c.2,
                    a.0,
                    c.0 + a.0
                );
            }
        }
        let end = snapshot(&w);
        let c1 = end.get(&1).map(|t| t.0).unwrap_or(0);
        let a1 = end.get(&2).map(|t| t.0).unwrap_or(0);
        // "Culture won" if the cooperator lineage grew its share of the population.
        let start_share = c0 as f32 / (c0 + a0).max(1) as f32;
        let end_share = c1 as f32 / (c1 + a1).max(1) as f32;
        if end_share > start_share {
            coop_wins += 1;
        }
        eprintln!(
            "SEED{seed} SUMMARY: coop {c0}->{c1}, asocial {a0}->{a1}, coop_share {start_share:.2}->{end_share:.2}"
        );
    }
    eprintln!("RESULT: culture-users grew their share in {coop_wins}/{SEEDS} seeds");
}

const SCENARIO_ALARM: &str = include_str!("../../../scenarios/gene-culture-alarm.toml");

/// A (alarm variant): culture-prey (Communicator + alarm early-warning) vs
/// asocial-prey (own-detection only), sharing the same predators. If cultural
/// early-warning is adaptive, culture-prey should out-survive the control.
#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn gene_culture_coevolution_A_alarm() {
    const SEEDS: u64 = 12;
    const TICKS: u32 = 800;
    let mut coop_wins = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO_ALARM).expect("parse alarm");
        s.seed = seed;
        let mut w = s.instantiate();
        let start = snapshot(&w);
        let c0 = start.get(&1).map(|t| t.0).unwrap_or(0);
        let a0 = start.get(&2).map(|t| t.0).unwrap_or(0);
        for _ in 0..TICKS {
            step(&mut w);
        }
        let end = snapshot(&w);
        let c1 = end.get(&1).map(|t| t.0).unwrap_or(0);
        let a1 = end.get(&2).map(|t| t.0).unwrap_or(0);
        if c1 > a1 {
            coop_wins += 1;
        }
        eprintln!("ALARM seed{seed}: culture_prey {c0}->{c1}, asocial_prey {a0}->{a1}");
    }
    eprintln!("ALARM RESULT: culture-prey out-survived asocial in {coop_wins}/{SEEDS} seeds");
}

const SCENARIO_HUNT: &str = include_str!("../../../scenarios/gene-culture-hunt.toml");

/// A (technique variant, addressing the "genes enable certain memes" insight):
/// FAST vs SLOW hunters share the SAME hunt-technique meme and the SAME prey.
/// If the meme's payoff is conditional on the speed GENE, fast hunters should
/// thrive and slow hunters decline — the gene gates the value of the meme.
#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn gene_culture_technique_A_hunt() {
    const SEEDS: u64 = 10;
    const TICKS: u32 = 1200;
    let mut fast_wins = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO_HUNT).expect("parse hunt");
        s.seed = seed;
        let mut w = s.instantiate();
        // species 1 = prey (grazer), 2 = fast_hunter, 3 = slow_hunter.
        for t in 0..TICKS {
            step(&mut w);
            if t % 400 == 399 {
                let sn = snapshot(&w);
                let prey = sn.get(&1).map(|x| x.0).unwrap_or(0);
                let fast = sn.get(&2).map(|x| x.0).unwrap_or(0);
                let slow = sn.get(&3).map(|x| x.0).unwrap_or(0);
                eprintln!("HUNT seed{seed} t{}: prey={prey} fast={fast} slow={slow}", t + 1);
            }
        }
        let sn = snapshot(&w);
        let fast = sn.get(&2).map(|x| x.0).unwrap_or(0);
        let slow = sn.get(&3).map(|x| x.0).unwrap_or(0);
        if fast > slow {
            fast_wins += 1;
        }
        eprintln!("HUNT SEED{seed}: fast {}->{fast}, slow {}->{slow}", 20, 20);
    }
    eprintln!("HUNT RESULT: fast (gene-enabled culture) beat slow in {fast_wins}/{SEEDS} seeds");
}

const SCENARIO_SKILL: &str = include_str!("../../../scenarios/gene-culture-skill.toml");

/// C (cumulative cultural skill): Communicator foragers who learn + socially
/// copy a foraging skill vs. an identical control that lacks the gene (so cannot
/// learn it). If culturally-transmitted skill is adaptive, the culture-gene
/// lineage should out-grow the control — gene-culture coevolution.
#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn gene_culture_skill_C() {
    const SEEDS: u64 = 20;
    const TICKS: u32 = 1500;
    let mut culture_wins = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO_SKILL).expect("parse skill");
        s.seed = seed;
        let mut w = s.instantiate();
        for t in 0..TICKS {
            step(&mut w);
            if t % 500 == 499 {
                let sn = snapshot(&w);
                let c = sn.get(&1).copied().unwrap_or((0, 0.0, 0.0));
                let a = sn.get(&2).map(|x| x.0).unwrap_or(0);
                eprintln!("SKILL seed{seed} t{}: culture n={} skill(meme5-proxy: meme2={:.2}) | asocial n={}", t + 1, c.0, c.1, a);
            }
        }
        let sn = snapshot(&w);
        let c = sn.get(&1).map(|x| x.0).unwrap_or(0);
        let a = sn.get(&2).map(|x| x.0).unwrap_or(0);
        if c > a {
            culture_wins += 1;
        }
        eprintln!("SKILL SEED{seed}: culture 40->{c}, asocial 40->{a}");
    }
    eprintln!(
        "SKILL RESULT: culture-gene lineage out-grew control in {culture_wins}/{SEEDS} seeds"
    );
}

/// B (first-principles): one INTERBREEDING population (species 0), seeded with
/// the Communicator gene at low frequency (~15%). Everyone forages identically.
/// If C's cultural skill is adaptive, natural selection + module crossover should
/// raise the Communicator-gene frequency over generations — gene-culture
/// coevolution arising from selection on standing variation, not from seeding.
#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn gene_culture_firstprinciples_B() {
    use anabios_core::genome::{Genome, GenomeSlot};
    use anabios_core::module::{communicator_kit, has, starter_kit, ModuleType};
    use anabios_core::program::starter_asocial_forager;

    const SEEDS: u64 = 6;
    const TICKS: u32 = 8000;
    const N: usize = 90;
    const INIT_COMM_FRAC: f64 = 0.15;

    for seed in 0..SEEDS {
        let mut w = anabios_core::world::World::new(seed);
        // Build one interbreeding species (id 0) of foragers on a food-rich patch.
        // ~15% carry the Communicator gene; the rest do not. Identical program.
        for k in 0..N {
            let ang = k as f32 * 0.7;
            let rad = 10.0 + (k % 11) as f32 * 6.0;
            let pos = anabios_core::prelude_test::Vec2::new(
                512.0 + rad * ang.cos(),
                512.0 + rad * ang.sin(),
            );
            let mut g = Genome::neutral();
            g.set(GenomeSlot::ReproductionThreshold, 0.3);
            let comm = (k as f64) / (N as f64) < INIT_COMM_FRAC;
            let kit = if comm { communicator_kit() } else { starter_kit() };
            w.spawn_seeded(pos, g, 0, kit, starter_asocial_forager());
        }
        let comm_frac = |w: &anabios_core::world::World| -> (usize, f64, f32) {
            let mut n = 0usize;
            let mut c = 0usize;
            let mut skill = 0.0f64;
            for id in w.agents.iter_alive() {
                let i = id as usize;
                n += 1;
                if has(&w.agents.modules[i], ModuleType::Communicator) {
                    c += 1;
                    skill += w.agents.meme_vector[i][5] as f64;
                }
            }
            (
                n,
                if n > 0 { c as f64 / n as f64 } else { 0.0 },
                if c > 0 { (skill / c as f64) as f32 } else { 0.0 },
            )
        };
        let (n0, f0, _) = comm_frac(&w);
        for t in 0..TICKS {
            step(&mut w);
            if t % 2000 == 1999 {
                let (n, f, sk) = comm_frac(&w);
                eprintln!(
                    "B seed{seed} t{}: pop={n} comm_gene_freq={:.3} mean_skill={:.2}",
                    t + 1,
                    f,
                    sk
                );
            }
        }
        let (n1, f1, _) = comm_frac(&w);
        eprintln!(
            "B SEED{seed}: pop {n0}->{n1}, comm_gene_freq {f0:.3}->{f1:.3} ({})",
            if f1 > f0 { "ROSE (selected)" } else { "fell/flat" }
        );
    }
}
