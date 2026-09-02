//! O1 `autopsy` subcommand: run a scenario, log per-strategy aggregates each
//! window, and report the invasion fitness of a chosen mutant strategy.
//!
//! Diagnosis only — reads the world, never changes the sim.

use std::io::Write;
use std::path::PathBuf;

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anyhow::{Context, Result};

use crate::founder;
use crate::ledger::{
    invasion_fitness, invasion_fitness_share, sample_strategies, strategy_label, InvasionWindow,
    StrategyKind,
};

/// Frequency below which a strategy counts as "rare" for invasion analysis.
const RARE_FRAC_MAX: f64 = 0.10;

/// Which strategy key `autopsy` buckets by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FounderTagMode {
    /// Per-tick Communicator-module presence (the original O1 key).
    Module,
    /// Lineage-locked founder tag (mutation-robust; O2 Step 0).
    Founder,
}

pub fn run(
    scenario_path: PathBuf,
    seed: Option<u64>,
    ticks: u64,
    window: u64,
    out: PathBuf,
    mutant: StrategyKind,
    tag: FounderTagMode,
) -> Result<()> {
    let window = window.max(1);
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario {}", scenario_path.display()))?;
    let mut scenario = Scenario::parse_toml(&text)?;
    if let Some(s) = seed {
        scenario.seed = s;
    }
    let mut world = scenario.instantiate();
    let mut tracker = (tag == FounderTagMode::Founder).then(|| founder::init(&world));

    let mut csv = std::fs::File::create(&out)
        .with_context(|| format!("creating ledger {}", out.display()))?;
    writeln!(csv, "tick,strategy,count,freq,mean_energy,mean_skill,mean_iq,mean_era,max_era")?;

    let mut invasion: Vec<InvasionWindow> = Vec::new();

    for t in 0..ticks {
        step(&mut world);
        if let Some(t) = tracker.as_mut() {
            t.observe(&world);
        }
        if (t + 1) % window == 0 {
            // O3 diagnostic (env-gated, observability only): practice burden
            // and birth-outcome evidence among Communicator agents.
            if std::env::var("ANABIOS_O3_DIAG").is_ok() {
                let mut n_comm = 0u32;
                let mut n_comm_ape = 0u32;
                let mut hold = [0u32; anabios_core::practice::PRACTICE_COUNT];
                let mut bok = 0u64;
                let mut bfail = 0u64;
                // Invention-gate decomposition for the era-climb autopsy:
                // among apes — IQ clears the era-1 gate / materials cover
                // Stone Tools / any invention channel level > 0 / held.
                let mut ape_iq1 = 0u32;
                let mut ape_mat = 0u32;
                let mut ape_chan = 0u32;
                let mut n_held_any = 0u32;
                for id in world.agents.iter_alive() {
                    let i = id as usize;
                    if anabios_core::invention::held_mask(&world.agents.meme_vector[i]) != 0 {
                        n_held_any += 1;
                    }
                    if !anabios_core::module::has(
                        &world.agents.modules[i],
                        anabios_core::module::ModuleType::Communicator,
                    ) {
                        continue;
                    }
                    n_comm += 1;
                    if anabios_core::invention::is_ape(
                        &world.agents.genome[i],
                        &world.agents.modules[i],
                    ) {
                        n_comm_ape += 1;
                        if world.agents.iq[i] >= anabios_core::invention::IQ_REQ_BY_ERA[0] {
                            ape_iq1 += 1;
                        }
                        if anabios_core::invention::materials_permit(
                            &world.agents.inventory[i],
                            0,
                            world.resources_enabled,
                        ) {
                            ape_mat += 1;
                        }
                        if (0..anabios_core::invention::INVENTION_COUNT).any(|k| {
                            world.agents.meme_vector[i][anabios_core::invention::channel(k)] > 0.0
                        }) {
                            ape_chan += 1;
                        }
                    }
                    bok += world.agents.births_ok[i] as u64;
                    bfail += world.agents.births_failed[i] as u64;
                    for (p, h) in hold.iter_mut().enumerate() {
                        if anabios_core::practice::has(&world.agents.meme_vector[i], p) {
                            *h += 1;
                        }
                    }
                }
                eprintln!(
                    "[o3diag] t={} comm={} comm_ape={} ape_iq1={} ape_mat={} ape_chan={} held_any={} hold={:?} births_ok={} births_failed={}",
                    t + 1,
                    n_comm,
                    n_comm_ape,
                    ape_iq1,
                    ape_mat,
                    ape_chan,
                    n_held_any,
                    hold,
                    bok,
                    bfail
                );
            }
            let stats = match &tracker {
                Some(t) => founder::sample_by_founder(&world, t),
                None => sample_strategies(&world),
            };
            let total: u32 = stats.iter().map(|s| s.count).sum();
            for s in &stats {
                let freq = if total == 0 { 0.0 } else { s.count as f64 / total as f64 };
                writeln!(
                    csv,
                    "{},{},{},{:.4},{:.3},{:.4},{:.4},{:.3},{}",
                    world.tick,
                    strategy_label(s.kind),
                    s.count,
                    freq,
                    s.mean_energy,
                    s.mean_skill,
                    s.mean_iq,
                    s.mean_era,
                    s.max_era,
                )?;
                if s.kind == mutant {
                    invasion.push(InvasionWindow { mutant_n: s.count, total_n: total });
                }
            }
        }
    }
    csv.flush().with_context(|| format!("flushing {}", out.display()))?;

    match invasion_fitness(&invasion, RARE_FRAC_MAX) {
        Some(r) => {
            let verdict = if r > 0.0 { "INVADES" } else { "EXCLUDED" };
            println!("invasion_fitness mutant={} r={:.5} {}", strategy_label(mutant), r, verdict);
        }
        None => {
            println!(
                "invasion_fitness mutant={} r=none (no rare-phase data)",
                strategy_label(mutant)
            );
        }
    }

    match invasion_fitness_share(&invasion, RARE_FRAC_MAX) {
        Some(r) => {
            let verdict = if r > 0.0 { "INVADES" } else { "EXCLUDED" };
            println!(
                "invasion_fitness_share mutant={} r={:.5} {}",
                strategy_label(mutant),
                r,
                verdict
            );
        }
        None => println!(
            "invasion_fitness_share mutant={} r=none (no rare-phase data)",
            strategy_label(mutant)
        ),
    }
    println!("ledger written: {}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // Minimal mixed world; a couple hundred ticks is enough to emit >1 window.
    const MIX: &str = "\
name = \"t\"
seed = 7
[[agents]]
count = 30
archetype = \"asocial_forager\"
placement = { kind = \"uniform\" }
[[agents]]
count = 8
archetype = \"cultural_forager\"
placement = { kind = \"uniform\" }
";

    #[test]
    fn autopsy_writes_ledger_rows_for_both_strategies() {
        let dir = std::env::temp_dir().join("anabios_o1_autopsy_test");
        std::fs::create_dir_all(&dir).unwrap();
        let scen = dir.join("mix.toml");
        std::fs::File::create(&scen).unwrap().write_all(MIX.as_bytes()).unwrap();
        let csv = dir.join("ledger.csv");

        run(scen, Some(7), 200, 50, csv.clone(), StrategyKind::Cultural, FounderTagMode::Module)
            .unwrap();

        let body = std::fs::read_to_string(&csv).unwrap();
        let mut lines = body.lines();
        assert_eq!(
            lines.next().unwrap(),
            "tick,strategy,count,freq,mean_energy,mean_skill,mean_iq,mean_era,max_era"
        );
        assert!(body.contains(",cultural,"), "ledger has a cultural row");
        assert!(body.contains(",asocial,"), "ledger has an asocial row");
        // 200 ticks / 50-tick window = at least 4 sampled windows × 2 strategies.
        assert!(lines.count() >= 8, "expected >=8 data rows");
    }

    #[test]
    fn autopsy_reports_share_and_supports_founder_tag() {
        let dir = std::env::temp_dir().join("anabios_o2_step0_autopsy");
        std::fs::create_dir_all(&dir).unwrap();
        let scen = dir.join("mix.toml");
        std::fs::File::create(&scen).unwrap().write_all(MIX.as_bytes()).unwrap();

        // Both tag modes must run and write a ledger with both strategies.
        for mode in [FounderTagMode::Module, FounderTagMode::Founder] {
            let csv = dir.join(format!("ledger-{mode:?}.csv"));
            run(scen.clone(), Some(7), 200, 50, csv.clone(), StrategyKind::Cultural, mode).unwrap();
            let body = std::fs::read_to_string(&csv).unwrap();
            assert!(body.contains(",cultural,"), "ledger has a cultural row ({mode:?})");
            assert!(body.contains(",asocial,"), "ledger has an asocial row ({mode:?})");
        }
    }
}
