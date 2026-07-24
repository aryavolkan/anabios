//! Sweep multiple seeds of a scenario in parallel, writing per-run codex
//! event JSONL files plus an aggregate CSV summary.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;

use crate::score::{self, ScoreTable};

#[derive(Serialize)]
struct RunSummary {
    seed: u64,
    ticks: u64,
    final_alive: u32,
    final_biomass: f32,
    state_hash: u64,
    counts: BTreeMap<&'static str, u64>,
    emergence_score: f64,
    novel_events: u64,
    coverage: f64,
    novel_types: Vec<&'static str>,
}

pub fn run(
    scenario_path: PathBuf,
    seeds: u64,
    ticks: u64,
    out_dir: PathBuf,
    threads: Option<usize>,
    archive: Option<PathBuf>,
) -> Result<()> {
    if let Some(n) = threads {
        rayon::ThreadPoolBuilder::new().num_threads(n).build_global().ok(); // ignore "already initialised" errors
    }
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating output dir {}", out_dir.display()))?;
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario {}", scenario_path.display()))?;

    let table = match &archive {
        Some(dir) => {
            let corpus = score::load_corpus(dir)?;
            eprintln!(
                "[sweep] archive: {} corpus runs from {} (scoring weights are empirical)",
                corpus.len(),
                dir.display()
            );
            ScoreTable::from_corpus(&corpus)
        }
        None => {
            eprintln!(
                "[sweep] no archive: scoring with default table v{} ({}-run reference corpus)",
                score::WEIGHTS_VERSION,
                score::CORPUS_RUNS
            );
            ScoreTable::default_table()
        }
    };

    let progress = Mutex::new(0_u64);
    let total = seeds;

    let summaries: Vec<RunSummary> = (0..seeds)
        .into_par_iter()
        .map(|seed| {
            let r = run_one(&text, seed, ticks, &out_dir, &table);
            if let Ok(mut p) = progress.lock() {
                *p += 1;
                eprintln!("[sweep] {}/{} done (seed={})", *p, total, seed);
            }
            r
        })
        .collect::<Result<Vec<_>>>()?;

    write_summary_csv(&out_dir, &summaries)?;
    report_novelty(&out_dir, &summaries)?;
    println!("sweep complete: {} runs × {} ticks → {}", seeds, ticks, out_dir.display());
    Ok(())
}

fn run_one(
    scenario_text: &str,
    seed: u64,
    ticks: u64,
    out_dir: &Path,
    table: &ScoreTable,
) -> Result<RunSummary> {
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
            let name = score::event_name(ev.event_type);
            *counts.entry(name).or_insert(0) += 1;
            serde_json::to_writer(&mut f, &ev)?;
            f.write_all(b"\n")?;
        }
    }

    let novel_types = score::novel_types(&counts, table);
    Ok(RunSummary {
        seed,
        ticks,
        final_alive: world.agents.live_count(),
        final_biomass: world.plant_biomass_total(),
        state_hash: state_hash(&world),
        emergence_score: score::score(&counts, table),
        novel_events: novel_types.len() as u64,
        coverage: score::coverage(&counts),
        novel_types,
        counts,
    })
}

fn report_novelty(out_dir: &Path, runs: &[RunSummary]) -> Result<()> {
    let mut ranked: Vec<&RunSummary> = runs.iter().collect();
    ranked.sort_by(|a, b| {
        b.emergence_score
            .partial_cmp(&a.emergence_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.seed.cmp(&b.seed))
    });
    println!("top runs by emergence score:");
    for r in ranked.iter().take(5) {
        println!(
            "  seed={:>3} score={:.3} coverage={:.3} novel={}",
            r.seed, r.emergence_score, r.coverage, r.novel_events
        );
    }

    let novel_runs: Vec<&&RunSummary> = ranked.iter().filter(|r| r.novel_events > 0).collect();
    if novel_runs.is_empty() {
        return Ok(());
    }
    let novel_dir = out_dir.join("novel");
    std::fs::create_dir_all(&novel_dir)
        .with_context(|| format!("creating {}", novel_dir.display()))?;
    println!("novel runs (fired corpus-unseen event types):");
    for r in novel_runs {
        println!("  seed={:>3} novel_types={}", r.seed, r.novel_types.join(","));
        let src = out_dir.join(format!("seed_{:08}.events.jsonl", r.seed));
        std::fs::copy(&src, novel_dir.join(src.file_name().unwrap()))
            .with_context(|| format!("copying {}", src.display()))?;
    }
    Ok(())
}

fn write_summary_csv(out_dir: &Path, runs: &[RunSummary]) -> Result<()> {
    let path = out_dir.join("summary.csv");
    let mut f = File::create(&path).with_context(|| format!("creating {}", path.display()))?;
    writeln!(
        f,
        "seed,ticks,final_alive,final_biomass,state_hash,\
         extinction,pop_crash,speciation,migration,novel_module,novel_behavior,\
         predation,combat_raid,arms_race,\
         territory_formation,niche_partitioning,\
         dialect_formed,meme_sweep,alarm_call,\
         evolved_cooperation,pack_hunting,herd_cohesion,\
         invention_discovered,invention_adopted,\
         practice_discovered,practice_adopted,\
         resource_traded,dowry_birth,\
         pop_cycle,boom_bust,carrying_capacity,trophic_cascade,\
         range_expansion,segregation,corridor_use,succession,\
         trait_fixation,rapid_adaptation,convergent_evolution,\
         evolved_ambush,evolved_tool,evolved_flight,structured_signaling,\
         war,war_ended,alliance,kin_network,\
         settlement,market,specialization_split,\
         tradition,cultural_radiation,institutional_ratchet,\
         emergence_score,novel_events,coverage"
    )?;
    for r in runs {
        let g = |k: &str| r.counts.get(k).copied().unwrap_or(0);
        writeln!(
            f,
            "{},{},{},{:.1},0x{:016x},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{},{:.3}",
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
            g("predation"),
            g("combat_raid"),
            g("arms_race"),
            g("territory_formation"),
            g("niche_partitioning"),
            g("dialect_formed"),
            g("meme_sweep"),
            g("alarm_call"),
            g("evolved_cooperation"),
            g("pack_hunting"),
            g("herd_cohesion"),
            g("invention_discovered"),
            g("invention_adopted"),
            g("practice_discovered"),
            g("practice_adopted"),
            g("resource_traded"),
            g("dowry_birth"),
            g("pop_cycle"),
            g("boom_bust"),
            g("carrying_capacity"),
            g("trophic_cascade"),
            g("range_expansion"),
            g("segregation"),
            g("corridor_use"),
            g("succession"),
            g("trait_fixation"),
            g("rapid_adaptation"),
            g("convergent_evolution"),
            g("evolved_ambush"),
            g("evolved_tool"),
            g("evolved_flight"),
            g("structured_signaling"),
            g("war"),
            g("war_ended"),
            g("alliance"),
            g("kin_network"),
            g("settlement"),
            g("market"),
            g("specialization_split"),
            g("tradition"),
            g("cultural_radiation"),
            g("institutional_ratchet"),
            r.emergence_score,
            r.novel_events,
            r.coverage,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn event_name_covers_m12_events() {
        use anabios_core::codex::EventType;
        assert_eq!(super::score::event_name(EventType::Predation), "predation");
        assert_eq!(super::score::event_name(EventType::CombatRaid), "combat_raid");
        assert_eq!(super::score::event_name(EventType::ArmsRace), "arms_race");
    }

    #[test]
    fn event_name_covers_m14_events() {
        use anabios_core::codex::EventType;
        assert_eq!(super::score::event_name(EventType::DialectFormed), "dialect_formed");
        assert_eq!(super::score::event_name(EventType::MemeSweep), "meme_sweep");
        assert_eq!(super::score::event_name(EventType::AlarmCall), "alarm_call");
    }

    #[test]
    fn event_name_covers_m15_events() {
        use anabios_core::codex::EventType;
        assert_eq!(super::score::event_name(EventType::EvolvedCooperation), "evolved_cooperation");
        assert_eq!(super::score::event_name(EventType::PackHunting), "pack_hunting");
        assert_eq!(super::score::event_name(EventType::HerdCohesion), "herd_cohesion");
    }

    #[test]
    fn event_name_covers_m13_events() {
        use anabios_core::codex::EventType;
        assert_eq!(super::score::event_name(EventType::TerritoryFormation), "territory_formation");
        assert_eq!(super::score::event_name(EventType::NichePartitioning), "niche_partitioning");
    }

    #[test]
    fn event_name_covers_invention_events() {
        use anabios_core::codex::EventType;
        assert_eq!(
            super::score::event_name(EventType::InventionDiscovered),
            "invention_discovered"
        );
        assert_eq!(super::score::event_name(EventType::InventionAdopted), "invention_adopted");
    }
}
