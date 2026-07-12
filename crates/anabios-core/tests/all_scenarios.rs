//! Smoke-test EVERY scenario TOML in `scenarios/`: each must parse, instantiate,
//! and run without panicking while keeping agents in world bounds. Most scenarios
//! have no dedicated test, so this is the regression guard that catches a substrate
//! change silently breaking a scenario (e.g. a new genome slot, a new World field,
//! or the DIT env mechanism).

use anabios_core::biome::WORLD_SIZE;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use std::fs;
use std::path::PathBuf;

fn scenarios_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios")
}

/// Collect every `*.toml` under `scenarios/`, sorted for determinism.
fn scenario_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(scenarios_dir())
        .expect("read scenarios dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    files
}

#[test]
fn every_scenario_parses_instantiates_and_runs() {
    let files = scenario_files();
    assert!(!files.is_empty(), "found no scenario TOMLs to validate");

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {name}: {e}"));
        let scenario = Scenario::parse_toml(&text).unwrap_or_else(|e| panic!("parse {name}: {e}"));
        let mut w = scenario.instantiate();

        for _ in 0..200 {
            step(&mut w);
        }

        // Every alive agent must remain within the (toroidal) world bounds — a cheap
        // catch-all that a scenario didn't drive the sim into a bad state.
        for id in w.agents.iter_alive() {
            let p = w.agents.position[id as usize];
            assert!(
                p.x.is_finite()
                    && p.y.is_finite()
                    && (0.0..WORLD_SIZE).contains(&p.x)
                    && (0.0..WORLD_SIZE).contains(&p.y),
                "{name}: agent {id} left world bounds at {p:?}"
            );
        }
        assert_eq!(w.tick, 200, "{name}: expected 200 ticks");
        eprintln!("ok: {name} ({} agents alive)", w.agents.live_count());
    }
    eprintln!("validated {} scenarios", files.len());
}
