//! Emergence scoring: rarity-weighted scores over codex event types.
//!
//! Pure post-processing over drained event counts — never touches the sim.
//! See `docs/superpowers/specs/2026-07-22-e1-emergence-scorecard-design.md`.
//!
//! Default-weight regeneration recipe (bump `WEIGHTS_VERSION` when redone):
//! sweep 16 seeds × 5000 ticks of `divergent`, `inventions`, `predator-prey`,
//! `cooperation` into one dir (`runs/corpus-e1/`, 64 runs), compute per-type
//! IDF `ln(N / n_t)` over per-run type sets, paste values into
//! `DEFAULT_WEIGHTS` below.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anabios_core::codex::{EventType, EVENT_TYPE_COUNT};
use anyhow::{Context, Result};

/// Version of the default weight table; bump on every regeneration.
pub const WEIGHTS_VERSION: &str = "e1.1";

/// Number of runs in the reference corpus behind `DEFAULT_WEIGHTS`.
pub const CORPUS_RUNS: u64 = 64;

/// Weight of a type never observed in the corpus: `ln(CORPUS_RUNS) + 1`.
pub const NOVELTY_BONUS: f64 = 5.158_883_083_359_671;

/// Every scorable event name, in summary-CSV column order.
pub const ALL_EVENT_NAMES: [&str; 23] = [
    "extinction",
    "pop_crash",
    "speciation",
    "migration",
    "novel_module",
    "novel_behavior",
    "predation",
    "combat_raid",
    "arms_race",
    "territory_formation",
    "niche_partitioning",
    "dialect_formed",
    "meme_sweep",
    "alarm_call",
    "evolved_cooperation",
    "pack_hunting",
    "herd_cohesion",
    "invention_discovered",
    "invention_adopted",
    "practice_discovered",
    "practice_adopted",
    "resource_traded",
    "dowry_birth",
];

/// Rarity weights derived from the reference corpus (see module docs):
/// 16 seeds × 5000 ticks of `divergent`, `inventions`, `predator-prey`,
/// `cooperation` (64 runs, swept 2026-07-22). `n_t` comments record how
/// many corpus runs fired each type; unseen types sit at `NOVELTY_BONUS`.
pub const DEFAULT_WEIGHTS: [(&str, f64); 23] = [
    ("extinction", 0.048009_f64),           // n_t=61
    ("pop_crash", 0.133531_f64),            // n_t=56
    ("speciation", 0.081346_f64),           // n_t=59
    ("migration", 0.169899_f64),            // n_t=54
    ("novel_module", 0.081346_f64),         // n_t=59
    ("novel_behavior", 0.048009_f64),       // n_t=61
    ("predation", 1.386294_f64),            // n_t=16
    ("combat_raid", 1.450833_f64),          // n_t=15
    ("arms_race", 1.856298_f64),            // n_t=10
    ("territory_formation", 0.397683_f64),  // n_t=43
    ("niche_partitioning", 0.207639_f64),   // n_t=52
    ("dialect_formed", 0.287682_f64),       // n_t=48
    ("meme_sweep", 0.495321_f64),           // n_t=39
    ("alarm_call", NOVELTY_BONUS),          // n_t=0
    ("evolved_cooperation", 1.386294_f64),  // n_t=16
    ("pack_hunting", 3.060271_f64),         // n_t=3
    ("herd_cohesion", 0.169899_f64),        // n_t=54
    ("invention_discovered", 1.386294_f64), // n_t=16
    ("invention_adopted", 1.386294_f64),    // n_t=16
    ("practice_discovered", NOVELTY_BONUS), // n_t=0
    ("practice_adopted", NOVELTY_BONUS),    // n_t=0
    ("resource_traded", NOVELTY_BONUS),     // n_t=0
    ("dowry_birth", NOVELTY_BONUS),         // n_t=0
];

pub fn event_name(t: EventType) -> &'static str {
    match t {
        EventType::Extinction => "extinction",
        EventType::PopulationCrash => "pop_crash",
        EventType::SpeciationEvent => "speciation",
        EventType::Migration => "migration",
        EventType::NovelModuleAppeared => "novel_module",
        EventType::NovelBehaviorPattern => "novel_behavior",
        EventType::Predation => "predation",
        EventType::CombatRaid => "combat_raid",
        EventType::ArmsRace => "arms_race",
        EventType::TerritoryFormation => "territory_formation",
        EventType::NichePartitioning => "niche_partitioning",
        EventType::DialectFormed => "dialect_formed",
        EventType::MemeSweep => "meme_sweep",
        EventType::AlarmCall => "alarm_call",
        EventType::EvolvedCooperation => "evolved_cooperation",
        EventType::PackHunting => "pack_hunting",
        EventType::HerdCohesion => "herd_cohesion",
        EventType::InventionDiscovered => "invention_discovered",
        EventType::InventionAdopted => "invention_adopted",
        EventType::PracticeDiscovered => "practice_discovered",
        EventType::PracticeAdopted => "practice_adopted",
        EventType::ResourceTraded => "resource_traded",
        EventType::DowryBirth => "dowry_birth",
    }
}

/// IDF weight table plus the set of corpus-known event types.
pub struct ScoreTable {
    pub weights: BTreeMap<&'static str, f64>,
    pub known: BTreeSet<&'static str>,
}

impl ScoreTable {
    /// The shipped reference-corpus table.
    pub fn default_table() -> Self {
        let mut weights = BTreeMap::new();
        let mut known = BTreeSet::new();
        for (name, w) in DEFAULT_WEIGHTS {
            weights.insert(name, w);
            if w < NOVELTY_BONUS {
                known.insert(name);
            }
        }
        Self { weights, known }
    }

    /// Empirical IDF over per-run type sets: `ln(N / n_t)`, unseen types at
    /// `NOVELTY_BONUS`. An empty corpus yields all-bonus weights.
    pub fn from_corpus(runs: &[BTreeSet<&'static str>]) -> Self {
        let n = runs.len() as f64;
        let mut weights = BTreeMap::new();
        let mut known = BTreeSet::new();
        for name in ALL_EVENT_NAMES {
            let n_t = runs.iter().filter(|r| r.contains(name)).count();
            let w = if n_t == 0 { NOVELTY_BONUS } else { (n / n_t as f64).ln() };
            if n_t > 0 {
                known.insert(name);
            }
            weights.insert(name, w);
        }
        Self { weights, known }
    }
}

/// Rarity-weighted sum over distinct fired event types. Repetition within a
/// run adds nothing; names outside the table are ignored defensively.
pub fn score(counts: &BTreeMap<&'static str, u64>, table: &ScoreTable) -> f64 {
    counts
        .iter()
        .filter(|(_, &c)| c > 0)
        .filter_map(|(name, _)| table.weights.get(name).copied())
        .sum()
}

/// Fraction of all event types fired at least once.
pub fn coverage(counts: &BTreeMap<&'static str, u64>) -> f64 {
    let fired = counts.values().filter(|&&c| c > 0).count();
    fired as f64 / EVENT_TYPE_COUNT as f64
}

/// Distinct fired types absent from the corpus, sorted by name.
pub fn novel_types(counts: &BTreeMap<&'static str, u64>, table: &ScoreTable) -> Vec<&'static str> {
    counts
        .iter()
        .filter(|(_, &c)| c > 0)
        .map(|(name, _)| *name)
        .filter(|name| !table.known.contains(name))
        .collect()
}

/// Load every `*.events.jsonl` file under `dir` (recursively) as one corpus
/// run each, returning per-run sets of distinct fired event types. Malformed
/// lines are skipped with a warning rather than failing the sweep.
pub fn load_corpus(dir: &Path) -> Result<Vec<BTreeSet<&'static str>>> {
    let mut files = Vec::new();
    collect_jsonl(dir, &mut files)
        .with_context(|| format!("scanning archive {}", dir.display()))?;
    files.sort();

    let mut runs = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let mut set = BTreeSet::new();
        for (i, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<EventLine>(line) {
                Ok(ev) => {
                    set.insert(event_name(ev.event_type));
                }
                Err(e) => {
                    eprintln!(
                        "[score] skipping malformed line {}:{}: {}",
                        path.display(),
                        i + 1,
                        e
                    );
                }
            }
        }
        runs.push(set);
    }
    Ok(runs)
}

#[derive(serde::Deserialize)]
struct EventLine {
    event_type: EventType,
}

fn collect_jsonl(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            // `novel/` holds copies of flagged runs — never double-count them.
            if path.file_name().and_then(|n| n.to_str()) == Some("novel") {
                continue;
            }
            collect_jsonl(&path, out)?;
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".events.jsonl"))
        {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(pairs: &[(&'static str, u64)]) -> BTreeMap<&'static str, u64> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn novelty_bonus_matches_corpus_size() {
        assert!((NOVELTY_BONUS - ((CORPUS_RUNS as f64).ln() + 1.0)).abs() < 1e-9);
    }

    #[test]
    fn event_name_list_covers_every_variant() {
        assert_eq!(ALL_EVENT_NAMES.len(), EVENT_TYPE_COUNT);
        for name in ALL_EVENT_NAMES {
            assert!(
                DEFAULT_WEIGHTS.iter().any(|(n, _)| *n == name),
                "missing default weight for {name}"
            );
        }
    }

    #[test]
    fn corpus_idf_math() {
        let a: BTreeSet<_> = ["extinction", "speciation"].into_iter().collect();
        let b: BTreeSet<_> = ["extinction"].into_iter().collect();
        let table = ScoreTable::from_corpus(&[a, b]);
        // ubiquitous → ln(2/2) = 0
        assert_eq!(table.weights["extinction"], 0.0);
        // half the runs → ln(2/1)
        assert!((table.weights["speciation"] - std::f64::consts::LN_2).abs() < 1e-12);
        // unseen → bonus, and not known
        assert_eq!(table.weights["arms_race"], NOVELTY_BONUS);
        assert!(table.known.contains("extinction"));
        assert!(!table.known.contains("arms_race"));
    }

    #[test]
    fn empty_corpus_is_all_bonus() {
        let table = ScoreTable::from_corpus(&[]);
        assert!(table.known.is_empty());
        assert_eq!(table.weights["extinction"], NOVELTY_BONUS);
    }

    #[test]
    fn score_counts_distinct_types_only() {
        let a: BTreeSet<_> = ["extinction"].into_iter().collect();
        let b: BTreeSet<_> = ["speciation"].into_iter().collect();
        let table = ScoreTable::from_corpus(&[a, b]);
        let c = counts(&[("extinction", 5), ("speciation", 1), ("arms_race", 2)]);
        let expected = 2.0 * std::f64::consts::LN_2 + NOVELTY_BONUS;
        assert!((score(&c, &table) - expected).abs() < 1e-12);
    }

    #[test]
    fn score_ignores_unknown_names() {
        let table = ScoreTable::from_corpus(&[]);
        let c = counts(&[("not_a_real_event", 3)]);
        assert_eq!(score(&c, &table), 0.0);
    }

    #[test]
    fn coverage_is_fraction_of_all_types() {
        let c = counts(&[("extinction", 1), ("speciation", 4)]);
        assert!((coverage(&c) - 2.0 / EVENT_TYPE_COUNT as f64).abs() < 1e-12);
        assert_eq!(coverage(&BTreeMap::new()), 0.0);
    }

    #[test]
    fn novel_types_are_fired_minus_known() {
        let a: BTreeSet<_> = ["extinction"].into_iter().collect();
        let table = ScoreTable::from_corpus(&[a]);
        let c = counts(&[("extinction", 1), ("arms_race", 1), ("speciation", 0)]);
        assert_eq!(novel_types(&c, &table), vec!["arms_race"]);
    }

    #[test]
    fn load_corpus_reads_nested_jsonl_and_skips_bad_lines() {
        let dir = std::env::temp_dir().join(format!("anabios-e1-test-{}", std::process::id()));
        let nested = dir.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            dir.join("seed_00000001.events.jsonl"),
            "{\"event_type\":\"Extinction\",\"tick\":5}\n\
             {\"event_type\":\"SpeciationEvent\",\"tick\":9}\n\
             not json at all\n",
        )
        .unwrap();
        std::fs::write(
            nested.join("seed_00000002.events.jsonl"),
            "{\"event_type\":\"ArmsRace\",\"tick\":42}\n",
        )
        .unwrap();
        std::fs::write(dir.join("ignored.txt"), "{\"event_type\":\"Extinction\"}\n").unwrap();
        let novel = dir.join("novel");
        std::fs::create_dir_all(&novel).unwrap();
        std::fs::write(
            novel.join("seed_00000099.events.jsonl"),
            "{\"event_type\":\"DowryBirth\",\"tick\":1}\n",
        )
        .unwrap();

        let runs = load_corpus(&dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(runs.len(), 2);
        let first = runs.iter().find(|r| r.contains("extinction")).expect("a run with extinction");
        assert!(first.contains("speciation"));
        assert_eq!(first.len(), 2);
        assert!(runs.iter().any(|r| r.contains("arms_race")));
    }
}
