//! Basic-needs integration tests: the flag-on trajectory actually expresses
//! the designed pressures (drinkable water exists in the flagship scenario,
//! dehydration shortens survival, sleep cycles run) — complementing the
//! flag-off inertness unit tests in `src/needs.rs`.

use anabios_core::genome::Genome;
use anabios_core::needs;
use anabios_core::prelude_test::Vec2;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anabios_core::world::World;

const SCENARIO: &str = include_str!("../../../scenarios/basic-needs.toml");

#[test]
fn scenario_instantiates_with_drinkable_water_in_reach() {
    let s = Scenario::parse_toml(SCENARIO).expect("basic-needs.toml parses");
    let w = s.instantiate();
    assert!(w.basic_needs_enabled, "flagship scenario opts in");
    let drinkable = (0..w.biome.res)
        .flat_map(|row| (0..w.biome.res).map(move |col| (col, row)))
        .filter(|&(col, row)| needs::drinkable_cell(&w.biome, col, row))
        .count();
    // Default sea level provides lakes/seas; river_threshold carves rivers on
    // top. A meaningfully-watered map has plenty of drinkable cells — this is
    // the guard that keeps the scenario from silently drying out under future
    // worldgen changes.
    assert!(drinkable > 100, "expected a watered map, got {drinkable} drinkable cells");
}

/// Dehydration must shorten survival through the existing starvation path:
/// an immobile agent on a barren cell dies strictly earlier when parched
/// (flag on, no water anywhere) than the identical flag-off control.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn dehydration_hastens_starvation() {
    let death_tick = |basic_needs: bool| -> u64 {
        let mut w = World::new(1);
        w.basic_needs_enabled = basic_needs;
        // Dry the whole map so there is nothing to drink.
        if basic_needs {
            for c in w.biome.cells.iter_mut() {
                if c.terrain == anabios_core::biome::TerrainType::Water {
                    c.terrain = anabios_core::biome::TerrainType::Rock;
                }
                c.river_flow = 0.0;
            }
        }
        // Immobile agent on a barren cell (the `starving_agent_dies` setup).
        let mut spawn = Vec2::ZERO;
        'outer: for row in 0..w.biome.res {
            for col in 0..w.biome.res {
                if w.biome.at(col, row).plant_biomass <= 0.0 {
                    spawn = Vec2::new(
                        (col as f32 + 0.5) * w.biome.cell_size,
                        (row as f32 + 0.5) * w.biome.cell_size,
                    );
                    break 'outer;
                }
            }
        }
        let id = w.spawn_agent(spawn, Genome::neutral());
        w.agents.modules[id as usize]
            .retain(|m| !matches!(m, anabios_core::module::Module::Locomotor { .. }));
        w.agents.energy[id as usize] = 20.0;
        if basic_needs {
            w.agents.thirst[id as usize] = 1.0; // already parched
        }
        for _ in 0..2000u64 {
            step(&mut w);
            if !w.agents.is_alive(id) {
                return w.tick;
            }
        }
        panic!("agent should starve within 2000 ticks (basic_needs={basic_needs})");
    };
    let control = death_tick(false);
    let parched = death_tick(true);
    assert!(
        parched < control,
        "full thirst must hasten death: parched died at {parched}, control at {control}"
    );
}

/// Over a long flag-on run, fatigue forces real sleep cycles: agents fall
/// asleep and later wake again (the hysteresis actually cycles in vivo).
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn agents_sleep_and_wake_over_a_long_run() {
    let s = Scenario::parse_toml(SCENARIO).expect("basic-needs.toml parses");
    let mut w = s.instantiate();
    let mut ever_asleep = false;
    let mut woke_after_sleep = false;
    let mut slept: std::collections::BTreeSet<u32> = Default::default();
    for _ in 0..1500u64 {
        step(&mut w);
        for id in w.agents.iter_alive() {
            if w.agents.asleep[id as usize] {
                ever_asleep = true;
                slept.insert(id);
            } else if slept.contains(&id) {
                woke_after_sleep = true;
            }
        }
        if ever_asleep && woke_after_sleep {
            break;
        }
    }
    assert!(ever_asleep, "fatigue should force sleep within 1500 ticks");
    assert!(woke_after_sleep, "sleepers should recover and wake again");
}
