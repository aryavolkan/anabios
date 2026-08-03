//! O1 `autopsy` subcommand: run a scenario, log per-strategy aggregates each
//! window, and report the invasion fitness of a chosen mutant strategy.
//!
//! Diagnosis only — reads the world, never changes the sim.

use std::io::Write;
use std::path::PathBuf;

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anyhow::{Context, Result};

use crate::ledger::{
    invasion_fitness, sample_strategies, strategy_label, InvasionWindow, StrategyKind,
};

/// Frequency below which a strategy counts as "rare" for invasion analysis.
const RARE_FRAC_MAX: f64 = 0.10;

pub fn run(
    scenario_path: PathBuf,
    seed: Option<u64>,
    ticks: u64,
    window: u64,
    out: PathBuf,
    mutant: StrategyKind,
) -> Result<()> {
    let window = window.max(1);
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario {}", scenario_path.display()))?;
    let mut scenario = Scenario::parse_toml(&text)?;
    if let Some(s) = seed {
        scenario.seed = s;
    }
    let mut world = scenario.instantiate();

    let mut csv = std::fs::File::create(&out)
        .with_context(|| format!("creating ledger {}", out.display()))?;
    writeln!(csv, "tick,strategy,count,freq,mean_energy,mean_skill,mean_iq,mean_era,max_era")?;

    let mut invasion: Vec<InvasionWindow> = Vec::new();

    for t in 0..ticks {
        step(&mut world);
        if (t + 1) % window == 0 {
            let stats = sample_strategies(&world);
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

        run(scen, Some(7), 200, 50, csv.clone(), StrategyKind::Cultural).unwrap();

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
}
