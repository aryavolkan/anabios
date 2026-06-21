//! Headless runner for anabios scenarios.

mod sweep;

use std::io::Write;
use std::path::PathBuf;

use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "anabios-headless", version, about = "Headless runner for anabios.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a scenario for N ticks and report summary metrics.
    Run {
        /// Path to a `.toml` scenario file.
        #[arg(long)]
        scenario: PathBuf,
        /// Number of ticks to run. Default 1000.
        #[arg(long, default_value_t = 1000)]
        ticks: u64,
        /// Optional explicit seed; overrides the scenario seed.
        #[arg(long)]
        seed: Option<u64>,
        /// Optional path to write codex events as JSON Lines as they occur.
        /// One JSON object per event, drained from the codex buffer after
        /// every tick.
        #[arg(long)]
        events_jsonl: Option<PathBuf>,
    },
    /// Print summary of a scenario without running it.
    Info {
        #[arg(long)]
        scenario: PathBuf,
    },
    /// Sweep N seeds of a scenario in parallel; write per-run codex
    /// events as JSONL plus an aggregate CSV summary.
    Sweep {
        #[arg(long)]
        scenario: PathBuf,
        #[arg(long, default_value_t = 16)]
        seeds: u64,
        #[arg(long, default_value_t = 2000)]
        ticks: u64,
        #[arg(long)]
        out: PathBuf,
        /// Override the rayon thread pool size; defaults to logical CPUs.
        #[arg(long)]
        threads: Option<usize>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run { scenario, ticks, seed, events_jsonl } => {
            run(scenario, ticks, seed, events_jsonl)
        }
        Command::Info { scenario } => info(scenario),
        Command::Sweep { scenario, seeds, ticks, out, threads } => {
            sweep::run(scenario, seeds, ticks, out, threads)
        }
    }
}

fn run(
    scenario_path: PathBuf,
    ticks: u64,
    seed: Option<u64>,
    events_jsonl: Option<PathBuf>,
) -> Result<()> {
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario file {}", scenario_path.display()))?;
    let mut scenario = Scenario::parse_toml(&text)?;
    if let Some(s) = seed {
        scenario.seed = s;
    }

    let mut world = scenario.instantiate();
    println!(
        "scenario={} seed={} initial_agents={} initial_biomass={:.1}",
        scenario.name,
        world.seed,
        world.agents.live_count(),
        world.plant_biomass_total()
    );

    let mut events_file = match &events_jsonl {
        Some(p) => Some(
            std::fs::File::create(p)
                .with_context(|| format!("creating events file {}", p.display()))?,
        ),
        None => None,
    };

    for _ in 0..ticks {
        step(&mut world);
        if let Some(f) = events_file.as_mut() {
            for ev in world.codex.drain_events() {
                serde_json::to_writer(&mut *f, &ev).context("writing codex event")?;
                f.write_all(b"\n")?;
            }
        }
    }

    let hash = state_hash(&world);
    println!(
        "ticks={} alive={} biomass={:.1} energy_total={:.1} state_hash=0x{:016x}",
        world.tick,
        world.agents.live_count(),
        world.plant_biomass_total(),
        world.alive_energy_total(),
        hash
    );

    Ok(())
}

fn info(scenario_path: PathBuf) -> Result<()> {
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario file {}", scenario_path.display()))?;
    let scenario = Scenario::parse_toml(&text)?;
    println!("{:#?}", scenario);
    Ok(())
}
