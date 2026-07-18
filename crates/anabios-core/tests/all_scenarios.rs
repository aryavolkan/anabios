//! Smoke-test EVERY scenario TOML in `scenarios/`: each must parse, instantiate,
//! and run without panicking while keeping agents in world bounds. Most scenarios
//! have no dedicated test, so this is the regression guard that catches a substrate
//! change silently breaking a scenario (e.g. a new genome slot, a new World field,
//! or the DIT env mechanism).

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
        // Clamp the population cap so fertile scenarios keep this smoke test
        // fast (the default 10k cap made 200-tick runs minutes-slow).
        w.max_population = w.max_population.min(500);

        for _ in 0..200 {
            step(&mut w);
        }

        // Every alive agent must remain within the (toroidal) world bounds — a cheap
        // catch-all that a scenario didn't drive the sim into a bad state. Uses this
        // world's own `world_size` (not the crate-default `WORLD_SIZE` constant) so
        // scenarios that opt into a larger world (e.g. `world_size = 2048.0`) are
        // checked against their actual bounds.
        let world_size = w.world_size;
        for id in w.agents.iter_alive() {
            let p = w.agents.position[id as usize];
            assert!(
                p.x.is_finite()
                    && p.y.is_finite()
                    && (0.0..world_size).contains(&p.x)
                    && (0.0..world_size).contains(&p.y),
                "{name}: agent {id} left world bounds at {p:?}"
            );
        }
        assert_eq!(w.tick, 200, "{name}: expected 200 ticks");
        eprintln!("ok: {name} ({} agents alive)", w.agents.live_count());
    }
    eprintln!("validated {} scenarios", files.len());
}

/// Dedicated smoke test for the Task 3.1 living-sandbox scenario (in addition
/// to the glob-based test above): both cohorts must start alive, and the fair
/// culture-vs-control design must actually hold — species 1 (culture) carries
/// a Communicator, species 2 (control) does not, and BOTH carry Reproductive
/// (the prior `skilled_forager` design would have dropped Reproductive from
/// the culture cohort via `communicator_kit()`, biasing the experiment).
#[test]
fn living_sandbox_smoke() {
    let toml = include_str!("../../../scenarios/living-sandbox-coevolution.toml");
    let mut w = anabios_core::scenario::Scenario::parse_toml(toml).unwrap().instantiate();

    let species1_alive =
        w.agents.iter_alive().filter(|&id| w.agents.species_id[id as usize] == 1).count();
    let species2_alive =
        w.agents.iter_alive().filter(|&id| w.agents.species_id[id as usize] == 2).count();
    assert!(species1_alive > 0, "culture cohort (species 1) should start alive");
    assert!(species2_alive > 0, "control cohort (species 2) should start alive");

    for id in w.agents.iter_alive() {
        let mods = &w.agents.modules[id as usize];
        let has_communicator =
            anabios_core::module::has(mods, anabios_core::module::ModuleType::Communicator);
        let has_reproductive =
            anabios_core::module::has(mods, anabios_core::module::ModuleType::Reproductive);
        assert!(has_reproductive, "agent {id}: BOTH cohorts must keep Reproductive (fair design)");
        match w.agents.species_id[id as usize] {
            1 => assert!(has_communicator, "agent {id}: culture cohort must have a Communicator"),
            2 => {
                assert!(
                    !has_communicator,
                    "agent {id}: control cohort must NOT have a Communicator"
                )
            }
            _ => {}
        }
    }

    for _ in 0..200 {
        anabios_core::tick::step(&mut w);
    }
    assert!(w.agents.live_count() > 0, "population should survive 200 ticks");
}
