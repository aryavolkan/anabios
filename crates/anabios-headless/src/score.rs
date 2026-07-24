//! Emergence scoring: rarity-weighted scores over codex event types.
//!
//! Pure post-processing over drained event counts — never touches the sim.
//! See `docs/superpowers/specs/2026-07-22-e1-emergence-scorecard-design.md`.
//!
//! Default-weight regeneration recipe (bump `WEIGHTS_VERSION` when redone):
//! sweep 16 seeds × 5000 ticks of `divergent`, `inventions`, `predator-prey`,
//! `cooperation` into one dir (`runs/corpus-e1/`, 64 runs), then paste the
//! per-type run counts into `DEFAULT_CORPUS_NT` below. Weights are *derived*
//! from those counts as IDF `ln(N / n_t)` — the counts are the single source
//! of truth, so a mis-transcribed count can never desync from its weight.
//! Event types added after the reference sweep (E3+) sit at `n_t = 0`
//! (unseen → `NOVELTY_BONUS`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anabios_core::codex::{EventType, EVENT_TYPE_COUNT};
use anyhow::{Context, Result};

/// Version of the default weight table; bump on every regeneration.
pub const WEIGHTS_VERSION: &str = "e1.1";

/// Number of runs in the reference corpus behind `DEFAULT_CORPUS_NT`.
pub const CORPUS_RUNS: u64 = 64;

/// Weight of a type never observed in the corpus: `ln(CORPUS_RUNS) + 1`.
pub const NOVELTY_BONUS: f64 = 5.158_883_083_359_671;

/// IDF rarity weight for a type fired by `n_t` of the `CORPUS_RUNS` runs.
/// Unseen types (`n_t == 0`) get the fixed novelty bonus; otherwise the
/// standard inverse-document-frequency `ln(N / n_t)`.
pub fn idf_weight(n_t: u64) -> f64 {
    if n_t == 0 {
        NOVELTY_BONUS
    } else {
        (CORPUS_RUNS as f64 / n_t as f64).ln()
    }
}

/// Every scorable event name, in summary-CSV column order.
pub const ALL_EVENT_NAMES: [&str; 45] = [
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
    "pop_cycle",
    "boom_bust",
    "carrying_capacity",
    "trophic_cascade",
    "range_expansion",
    "segregation",
    "corridor_use",
    "succession",
    "trait_fixation",
    "rapid_adaptation",
    "convergent_evolution",
    "evolved_ambush",
    "evolved_tool",
    "evolved_flight",
    "structured_signaling",
    "war",
    "war_ended",
    "alliance",
    "kin_network",
    "settlement",
    "market",
    "specialization_split",
];

/// Per-type corpus run counts from the reference sweep (see module docs):
/// 16 seeds × 5000 ticks of `divergent`, `inventions`, `predator-prey`,
/// `cooperation` (64 runs, swept 2026-07-22) — how many runs fired each type.
/// `0` means unseen in the corpus (weight `NOVELTY_BONUS`). Event types added
/// after the reference sweep (E3+) are definitionally unseen (`n_t = 0`) until
/// the next regeneration. Weights are derived via [`idf_weight`], so this
/// table is the *only* thing to update on a regeneration.
pub const DEFAULT_CORPUS_NT: [(&str, u64); 45] = [
    ("extinction", 61),
    ("pop_crash", 56),
    ("speciation", 59),
    ("migration", 54),
    ("novel_module", 59),
    ("novel_behavior", 61),
    ("predation", 16),
    ("combat_raid", 15),
    ("arms_race", 10),
    ("territory_formation", 43),
    ("niche_partitioning", 52),
    ("dialect_formed", 48),
    ("meme_sweep", 39),
    ("alarm_call", 0),
    ("evolved_cooperation", 16),
    ("pack_hunting", 3),
    ("herd_cohesion", 54),
    ("invention_discovered", 16),
    ("invention_adopted", 16),
    ("practice_discovered", 0),
    ("practice_adopted", 0),
    ("resource_traded", 0),
    ("dowry_birth", 0),
    ("pop_cycle", 0),            // post-corpus (E3)
    ("boom_bust", 0),            // post-corpus (E3)
    ("carrying_capacity", 0),    // post-corpus (E3)
    ("trophic_cascade", 0),      // post-corpus (E3)
    ("range_expansion", 0),      // post-corpus (E4)
    ("segregation", 0),          // post-corpus (E4)
    ("corridor_use", 0),         // post-corpus (E4)
    ("succession", 0),           // post-corpus (E4)
    ("trait_fixation", 0),       // post-corpus (E5)
    ("rapid_adaptation", 0),     // post-corpus (E5)
    ("convergent_evolution", 0), // post-corpus (E5)
    ("evolved_ambush", 0),       // post-corpus (E6)
    ("evolved_tool", 0),         // post-corpus (E6)
    ("evolved_flight", 0),       // post-corpus (E6)
    ("structured_signaling", 0), // post-corpus (E6)
    ("war", 0),                  // post-corpus (E7)
    ("war_ended", 0),            // post-corpus (E7)
    ("alliance", 0),             // post-corpus (E7)
    ("kin_network", 0),          // post-corpus (E7)
    ("settlement", 0),           // post-corpus (E8)
    ("market", 0),               // post-corpus (E8)
    ("specialization_split", 0), // post-corpus (E8)
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
        EventType::PopulationCycleDetected => "pop_cycle",
        EventType::BoomAndBust => "boom_bust",
        EventType::CarryingCapacityReached => "carrying_capacity",
        EventType::TrophicCascade => "trophic_cascade",
        EventType::RangeExpansion => "range_expansion",
        EventType::SegregationEmerged => "segregation",
        EventType::CorridorUse => "corridor_use",
        EventType::Succession => "succession",
        EventType::TraitFixation => "trait_fixation",
        EventType::RapidAdaptation => "rapid_adaptation",
        EventType::ConvergentEvolution => "convergent_evolution",
        EventType::EvolvedAmbush => "evolved_ambush",
        EventType::EvolvedTool => "evolved_tool",
        EventType::EvolvedFlight => "evolved_flight",
        EventType::StructuredSignaling => "structured_signaling",
        EventType::WarOrRaid => "war",
        EventType::WarEnded => "war_ended",
        EventType::AllianceFormed => "alliance",
        EventType::KinNetworkStable => "kin_network",
        EventType::SettlementFormed => "settlement",
        EventType::MarketEmerged => "market",
        EventType::SpecializationSplit => "specialization_split",
    }
}

/// IDF weight table plus the set of corpus-known event types.
pub struct ScoreTable {
    pub weights: BTreeMap<&'static str, f64>,
    pub known: BTreeSet<&'static str>,
}

impl ScoreTable {
    /// The shipped reference-corpus table, derived from `DEFAULT_CORPUS_NT`.
    pub fn default_table() -> Self {
        let mut weights = BTreeMap::new();
        let mut known = BTreeSet::new();
        for (name, n_t) in DEFAULT_CORPUS_NT {
            weights.insert(name, idf_weight(n_t));
            if n_t > 0 {
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
        assert_eq!(DEFAULT_CORPUS_NT.len(), EVENT_TYPE_COUNT);
        for name in ALL_EVENT_NAMES {
            assert!(
                DEFAULT_CORPUS_NT.iter().any(|(n, _)| *n == name),
                "missing corpus count for {name}"
            );
        }
    }

    #[test]
    fn default_weights_match_documented_values() {
        // Regression guard: the derived weights must equal the human-audited
        // reference values (from the 2026-07-22 sweep). If a corpus count in
        // `DEFAULT_CORPUS_NT` is mis-transcribed, this pins where. Post-corpus
        // types (n_t=0) sit at NOVELTY_BONUS.
        let expected: &[(&str, f64)] = &[
            ("extinction", 0.048009),
            ("pop_crash", 0.133531),
            ("speciation", 0.081346),
            ("migration", 0.169899),
            ("novel_module", 0.081346),
            ("novel_behavior", 0.048009),
            ("predation", 1.386294),
            ("combat_raid", 1.450833),
            ("arms_race", 1.856298),
            ("territory_formation", 0.397683),
            ("niche_partitioning", 0.207639),
            ("dialect_formed", 0.287682),
            ("meme_sweep", 0.495321),
            ("alarm_call", NOVELTY_BONUS),
            ("evolved_cooperation", 1.386294),
            ("pack_hunting", 3.060271),
            ("herd_cohesion", 0.169899),
            ("invention_discovered", 1.386294),
            ("invention_adopted", 1.386294),
            ("practice_discovered", NOVELTY_BONUS),
            ("practice_adopted", NOVELTY_BONUS),
            ("resource_traded", NOVELTY_BONUS),
            ("dowry_birth", NOVELTY_BONUS),
        ];
        let table = ScoreTable::default_table();
        for (name, want) in expected {
            let got = table.weights[name];
            assert!((got - want).abs() < 5e-6, "{name}: derived weight {got} != documented {want}");
        }
        // Every post-corpus (E3+) type is unseen → NOVELTY_BONUS.
        for name in ["pop_cycle", "war", "settlement", "specialization_split"] {
            assert_eq!(table.weights[name], NOVELTY_BONUS);
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
