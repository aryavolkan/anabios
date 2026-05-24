//! End-to-end gating: stripping a module type from an agent prevents
//! the corresponding action through one full tick.

use anabios_core::genome::Genome;
use anabios_core::module::{Module, ModuleType};
use anabios_core::prelude_test::Vec2;
use anabios_core::tick::step;
use anabios_core::world::World;

#[test]
fn no_locomotor_no_motion_through_step() {
    let mut w = World::new(1);
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    w.agents.modules[id as usize].retain(|m| !matches!(m, Module::Locomotor { .. }));
    let pos_before = w.agents.position[id as usize];
    step(&mut w);
    let pos_after = w.agents.position[id as usize];
    assert_eq!(pos_before, pos_after);
}

#[test]
fn no_mouth_no_energy_gain_through_step() {
    let mut w = World::new(13);
    // Find a grass cell.
    let mut spawn = Vec2::ZERO;
    use anabios_core::biome::{BIOME_RES, CELL_SIZE};
    'outer: for row in 0..BIOME_RES {
        for col in 0..BIOME_RES {
            if w.biome.at(col, row).terrain == anabios_core::biome::TerrainType::Grass {
                spawn = Vec2::new((col as f32 + 0.5) * CELL_SIZE, (row as f32 + 0.5) * CELL_SIZE);
                break 'outer;
            }
        }
    }
    let id = w.spawn_agent(spawn, Genome::neutral());
    w.agents.modules[id as usize].retain(|m| !matches!(m, Module::Mouth { .. }));

    let energy_before = w.agents.energy[id as usize];
    let biomass_before = w.biome.sample(spawn).plant_biomass;
    step(&mut w);
    let biomass_after = w.biome.sample(spawn).plant_biomass;
    assert_eq!(biomass_after, biomass_before, "biomass unchanged when no Mouth");
    // Energy may have dropped from upkeep + metabolism, but not increased.
    assert!(w.agents.energy[id as usize] <= energy_before);
}

#[test]
fn no_sensor_population_count_unchanged() {
    // Mainly a smoke test that no Sensor doesn't panic in sense_all.
    let mut w = World::new(1);
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    w.agents.modules[id as usize].retain(|m| !matches!(m, Module::Sensor { .. }));
    for _ in 0..20 {
        step(&mut w);
        if !w.agents.is_alive(id) {
            break;
        }
    }
    // Either still alive (with no Sensor, can't find food) or starved —
    // either way no panic.
    let _ = ModuleType::Sensor;
}
