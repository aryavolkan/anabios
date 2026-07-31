//! Headless runner for anabios scenarios.

mod demo;
mod record;
mod replay;
mod score;
mod soak;
mod sweep;

use std::io::{BufWriter, Write};
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
        /// Directory of prior sweep outputs to score against: every
        /// `*.events.jsonl` found recursively counts as one corpus run.
        /// Runs firing corpus-unseen event types are copied to `<out>/novel/`.
        #[arg(long)]
        archive: Option<PathBuf>,
    },
    /// Soak one scenario over a long horizon; every window record novelty
    /// (new vs cumulative distinct event types), activity, and telemetry
    /// (ticks/s, live agents, serialized state size). Writes a per-window CSV.
    Soak {
        #[arg(long)]
        scenario: PathBuf,
        /// Total ticks to run. Default 1,000,000.
        #[arg(long, default_value_t = 1_000_000)]
        ticks: u64,
        /// Window size in ticks for each measurement row. Default 100,000.
        #[arg(long, default_value_t = 100_000)]
        window: u64,
        /// Optional explicit seed; overrides the scenario seed.
        #[arg(long)]
        seed: Option<u64>,
        /// Path to write the per-window curve CSV. Default `soak-curve.csv`.
        #[arg(long, default_value = "soak-curve.csv")]
        out: PathBuf,
    },
    /// Replay-verify codex events: re-simulate each event from the nearest
    /// periodic snapshot and assert the same state hash and refire tick.
    /// Exits non-zero on any mismatch (detector regression gate).
    Replay {
        #[arg(long)]
        scenario: PathBuf,
        /// Optional explicit seed; overrides the scenario seed.
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long, default_value_t = 2000)]
        ticks: u64,
        /// Ticks between rewind snapshots.
        #[arg(long, default_value_t = 250)]
        snapshot_every: u64,
        /// Replay only the event with this stream index; default replays all.
        #[arg(long)]
        event: Option<usize>,
        /// Explicit no-op for readability; replaying all is the default.
        #[arg(long)]
        all: bool,
    },
    /// Record a scenario deterministically into a compact replay stream
    /// (`showcase/replay.js`) for the web showcase player. Reading agent draw
    /// data is RNG-free, so the recorded run is bit-identical to `run`.
    Record {
        #[arg(long)]
        scenario: PathBuf,
        /// Number of ticks to run. Default 4000 (reaches the later eras).
        #[arg(long, default_value_t = 4000)]
        ticks: u64,
        /// Optional explicit seed; overrides the scenario seed.
        #[arg(long)]
        seed: Option<u64>,
        /// Sample a frame every N ticks. Smaller = smoother, larger file.
        /// The player interpolates between frames, so ~24 stays fluid.
        #[arg(long, default_value_t = 24)]
        sample: u64,
        /// Cap agents per frame via stable stride subsampling (0 = all).
        #[arg(long, default_value_t = 260)]
        max_agents: usize,
        /// Cap events, keeping each type's first plus a uniform sample (0 = all).
        #[arg(long, default_value_t = 1000)]
        max_events: usize,
        /// Biome map resolution per axis (downsampled from the sim's grid).
        #[arg(long, default_value_t = 96)]
        biome_res: usize,
        /// Number of biome-map keyframes across the run (0 disables the map).
        #[arg(long, default_value_t = 6)]
        biome_frames: usize,
        /// Output path. `.js` wraps `window.ANABIOS_REPLAY=…`; `.json` is raw.
        #[arg(long, default_value = "showcase/replay.js")]
        out: PathBuf,
    },
    /// Narrate the cultural invention race: stream discovery/adoption events
    /// and periodic per-species tech tables. Best with
    /// `scenarios/inventions.toml`.
    Demo {
        #[arg(long)]
        scenario: PathBuf,
        /// Number of ticks to run. Default 8000 (enough for era 3+).
        #[arg(long, default_value_t = 8000)]
        ticks: u64,
        /// Optional explicit seed; overrides the scenario seed.
        #[arg(long)]
        seed: Option<u64>,
        /// Ticks between per-species tech reports. Default 1000.
        #[arg(long, default_value_t = 1000)]
        report_every: u64,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run { scenario, ticks, seed, events_jsonl } => {
            run(scenario, ticks, seed, events_jsonl)
        }
        Command::Info { scenario } => info(scenario),
        Command::Sweep { scenario, seeds, ticks, out, threads, archive } => {
            sweep::run(scenario, seeds, ticks, out, threads, archive)
        }
        Command::Demo { scenario, ticks, seed, report_every } => {
            demo::run(scenario, ticks, seed, report_every)
        }
        Command::Record {
            scenario,
            ticks,
            seed,
            sample,
            max_agents,
            max_events,
            biome_res,
            biome_frames,
            out,
        } => record::run(
            scenario,
            ticks,
            seed,
            sample,
            max_agents,
            max_events,
            biome_res,
            biome_frames,
            out,
        ),
        Command::Soak { scenario, ticks, window, seed, out } => {
            soak::run(scenario, ticks, window, seed, out)
        }
        Command::Replay { scenario, seed, ticks, snapshot_every, event, all } => {
            let ok = replay::run(scenario, seed, ticks, snapshot_every, event, all)?;
            if !ok {
                std::process::exit(1);
            }
            Ok(())
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

    // Buffer the per-event JSONL writes (two raw syscalls per event otherwise).
    // Output bytes are unchanged.
    let mut events_file = match &events_jsonl {
        Some(p) => Some(BufWriter::new(
            std::fs::File::create(p)
                .with_context(|| format!("creating events file {}", p.display()))?,
        )),
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
    // Surface any buffered-write error rather than letting drop swallow it.
    if let Some(f) = events_file.as_mut() {
        f.flush().context("flushing events file")?;
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
