use anabios_core::biome::{BiomeField, ClimateParams, TerrainType};
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;

fn continental() -> ClimateParams {
    ClimateParams {
        continentality: 0.85,
        mountain_uplift: 0.6,
        rain_shadow: 0.4,
        river_threshold: 150.0,
        ..Default::default()
    }
}

#[test]
fn every_continental_seed_has_land_mountains_and_rivers() {
    let cfg = continental();
    for seed in 0..8u64 {
        let f = BiomeField::generate_with(seed, 256, 4096.0, &cfg);
        let land = f.cells.iter().filter(|c| c.terrain != TerrainType::Water).count();
        let rock = f.cells.iter().filter(|c| c.terrain == TerrainType::Rock).count();
        let rivers = f.cells.iter().filter(|c| c.river_flow > 0.0).count();
        assert!(land > f.cells.len() / 10, "seed {seed}: too little land ({land})");
        assert!(rock > 0, "seed {seed}: no mountains");
        assert!(rivers > 0, "seed {seed}: no rivers");
    }
}

#[test]
fn continental_generation_is_deterministic() {
    let cfg = continental();
    let a = BiomeField::generate_with(3, 256, 4096.0, &cfg);
    let b = BiomeField::generate_with(3, 256, 4096.0, &cfg);
    for (x, y) in a.cells.iter().zip(b.cells.iter()) {
        assert_eq!(x.terrain, y.terrain);
        assert_eq!(x.elevation, y.elevation);
        assert_eq!(x.river_flow, y.river_flow);
        assert_eq!(x.moisture, y.moisture);
    }
}

#[test]
fn continental_scenario_loads_and_runs_deterministically() {
    let toml = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scenarios/continental.toml"
    ))
    .expect("read continental.toml");
    let scenario = Scenario::parse_toml(&toml).expect("parse");
    let mut a = scenario.instantiate();
    let mut b = scenario.instantiate();
    for _ in 0..200 {
        step(&mut a);
        step(&mut b);
    }
    assert_eq!(state_hash(&a), state_hash(&b), "scenario must be deterministic");
    assert!(a.agents.iter_alive().count() > 0, "population should survive 200 ticks");
}
