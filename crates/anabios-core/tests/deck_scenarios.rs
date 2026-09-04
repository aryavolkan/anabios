//! Smoke-test the showcase deck pins (the `scenarios/decks/` garden contract —
//! see `scenarios/decks/README.md`). Every curated deck JSON in
//! `game/showcase/` must declare its pin — `"seed": N` plus a `"_comment"`
//! containing `scenario=<name>` — and the pinned scenario must resolve
//! (searched in `scenarios/decks/` first, then the core `scenarios/` set),
//! instantiate at the pinned seed, and survive 200 ticks. Decks named
//! `smoke-*.json` are exempt (scenario-agnostic pipeline timelines).
//!
//! This catches the two ways a showcase asset silently rots: a deck whose
//! backing scenario was renamed/removed, and a pinned run that a sim change
//! drives into a bad state. The garden's reverse contract is pinned too:
//! every TOML under `scenarios/decks/` must back a same-named deck JSON.

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Extract `scenario=<name>` from a deck's `_comment` pin.
fn pinned_scenario(comment: &str) -> Option<String> {
    let at = comment.find("scenario=")? + "scenario=".len();
    let name: String = comment[at..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();
    (!name.is_empty()).then_some(name)
}

#[test]
fn curated_decks_declare_resolvable_pins() {
    let showcase_dir = repo_root().join("game/showcase");
    let scenarios_dir = repo_root().join("scenarios");

    let mut decks: Vec<PathBuf> = fs::read_dir(&showcase_dir)
        .expect("read game/showcase")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    decks.sort();
    assert!(!decks.is_empty(), "found no showcase deck JSONs");

    let mut curated = 0;
    for path in &decks {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        if stem.starts_with("smoke-") {
            continue; // scenario-agnostic pipeline timelines
        }
        curated += 1;

        let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {stem}: {e}"));
        let deck: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {stem}.json: {e}"));

        let seed = deck
            .get("seed")
            .and_then(|s| s.as_u64())
            .unwrap_or_else(|| panic!("{stem}: curated deck must declare a numeric \"seed\" pin"));
        let comment = deck
            .get("_comment")
            .and_then(|c| c.as_str())
            .unwrap_or_else(|| panic!("{stem}: curated deck must declare a \"_comment\" pin"));
        let name = pinned_scenario(comment)
            .unwrap_or_else(|| panic!("{stem}: _comment must contain `scenario=<name>`"));

        // Garden tier shadows the core set.
        let garden = scenarios_dir.join("decks").join(format!("{name}.toml"));
        let core = scenarios_dir.join(format!("{name}.toml"));
        let toml_path = if garden.exists() {
            garden
        } else {
            assert!(
                core.exists(),
                "{stem}: pinned scenario '{name}' not found in scenarios/decks/ or scenarios/"
            );
            core
        };

        let toml_text = fs::read_to_string(&toml_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", toml_path.display()));
        let mut scenario = Scenario::parse_toml(&toml_text)
            .unwrap_or_else(|e| panic!("parse {}: {e}", toml_path.display()));
        scenario.seed = seed; // run the pin, not the scenario's default seed
        let mut w = scenario.instantiate();
        // Honor the scenario's own max_population: it is pinned state the
        // recording ran under. Clamping it to 500 made the saga world (1184
        // founders, cap 3000) birth-free for the whole smoke — a pure
        // attrition trajectory the recorded asset never enters, so the test
        // wasn't exercising the pin it documents.

        for _ in 0..200 {
            step(&mut w);
        }

        let world_size = w.world_size;
        for id in w.agents.iter_alive() {
            let p = w.agents.position[id as usize];
            assert!(
                p.x.is_finite()
                    && p.y.is_finite()
                    // Inclusive upper bound: f32 rem_euclid can land exactly
                    // on world_size for a tiny negative coordinate, and the
                    // sim tolerates it (cell_coords clamps) — a legal state,
                    // not an escape.
                    && (0.0..=world_size).contains(&p.x)
                    && (0.0..=world_size).contains(&p.y),
                "{stem}: agent {id} left world bounds at {p:?}"
            );
        }
        assert_eq!(w.tick, 200, "{stem}: expected 200 ticks");
        eprintln!(
            "ok: {stem} → {} · seed {seed} ({} agents alive)",
            toml_path.display(),
            w.agents.live_count()
        );
    }
    assert!(curated > 0, "expected at least one curated (non-smoke) deck");
    eprintln!("validated {curated} curated deck pins");
}

/// Reverse contract: every garden TOML backs a same-named deck JSON. (Vacuous
/// until the first deck-dedicated scenario lands; keeps the tier honest.)
#[test]
fn every_garden_scenario_backs_a_deck() {
    let garden_dir = repo_root().join("scenarios/decks");
    let showcase_dir = repo_root().join("game/showcase");
    for entry in fs::read_dir(&garden_dir).expect("read scenarios/decks").filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_some_and(|x| x == "toml") {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let deck = showcase_dir.join(format!("{stem}.json"));
            assert!(
                deck.exists(),
                "scenarios/decks/{stem}.toml has no same-named deck in game/showcase/ \
                 — garden scenarios exist to back showcase assets"
            );
        }
    }
}
