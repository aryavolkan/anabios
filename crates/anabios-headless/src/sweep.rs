//! Sweep multiple seeds of a scenario in parallel, writing per-run codex
//! event JSONL files plus an aggregate CSV summary.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
struct RunSummary {
    seed: u64,
    ticks: u64,
    final_alive: u32,
    final_biomass: f32,
    state_hash: u64,
    counts: BTreeMap<&'static str, u64>,
}

pub fn run(
    scenario_path: PathBuf,
    seeds: u64,
    ticks: u64,
    out_dir: PathBuf,
    threads: Option<usize>,
) -> Result<()> {
    if let Some(n) = threads {
        rayon::ThreadPoolBuilder::new().num_threads(n).build_global().ok(); // ignore "already initialised" errors
    }
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating output dir {}", out_dir.display()))?;
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario {}", scenario_path.display()))?;

    let progress = Mutex::new(0_u64);
    let total = seeds;

    let summaries: Vec<RunSummary> = (0..seeds)
        .into_par_iter()
        .map(|seed| {
            let r = run_one(&text, seed, ticks, &out_dir);
            if let Ok(mut p) = progress.lock() {
                *p += 1;
                eprintln!("[sweep] {}/{} done (seed={})", *p, total, seed);
            }
            r
        })
        .collect::<Result<Vec<_>>>()?;

    write_summary_csv(&out_dir, &summaries)?;
    println!("sweep complete: {} runs × {} ticks → {}", seeds, ticks, out_dir.display());
    Ok(())
}

fn run_one(scenario_text: &str, seed: u64, ticks: u64, out_dir: &Path) -> Result<RunSummary> {
    let mut scenario = Scenario::parse_toml(scenario_text)?;
    scenario.seed = seed;
    let mut world = scenario.instantiate();

    let events_path = out_dir.join(format!("seed_{seed:08}.events.jsonl"));
    let mut f = File::create(&events_path)
        .with_context(|| format!("creating {}", events_path.display()))?;

    let mut counts: BTreeMap<&'static str, u64> = BTreeMap::new();
    for _ in 0..ticks {
        step(&mut world);
        for ev in world.codex.drain_events() {
            let name = event_name(ev.event_type);
            *counts.entry(name).or_insert(0) += 1;
            serde_json::to_writer(&mut f, &ev)?;
            f.write_all(b"\n")?;
        }
    }

    Ok(RunSummary {
        seed,
        ticks,
        final_alive: world.agents.live_count(),
        final_biomass: world.plant_biomass_total(),
        state_hash: state_hash(&world),
        counts,
    })
}

fn event_name(t: EventType) -> &'static str {
    match t {
        EventType::Extinction => "extinction",
        EventType::PopulationCrash => "pop_crash",
        EventType::SpeciationEvent => "speciation",
        EventType::Migration => "migration",
        EventType::NovelModuleAppeared => "novel_module",
        EventType::NovelBehaviorPattern => "novel_behavior",
    }
}

fn write_summary_csv(out_dir: &Path, runs: &[RunSummary]) -> Result<()> {
    let path = out_dir.join("summary.csv");
    let mut f = File::create(&path).with_context(|| format!("creating {}", path.display()))?;
    writeln!(
        f,
        "seed,ticks,final_alive,final_biomass,state_hash,\
         extinction,pop_crash,speciation,migration,novel_module,novel_behavior"
    )?;
    for r in runs {
        let g = |k: &str| r.counts.get(k).copied().unwrap_or(0);
        writeln!(
            f,
            "{},{},{},{:.1},0x{:016x},{},{},{},{},{},{}",
            r.seed,
            r.ticks,
            r.final_alive,
            r.final_biomass,
            r.state_hash,
            g("extinction"),
            g("pop_crash"),
            g("speciation"),
            g("migration"),
            g("novel_module"),
            g("novel_behavior"),
        )?;
    }
    Ok(())
}
