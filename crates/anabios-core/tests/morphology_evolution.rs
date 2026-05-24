//! Integration test: over many generations, structural mutation introduces
//! module types that were not present in the founders' starter kit.

use anabios_core::module::{ModuleType, MODULE_LIST_MAX};
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use std::collections::HashSet;

const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

#[test]
fn novel_module_types_appear_within_5000_ticks() {
    let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
    let mut world = scenario.instantiate();

    // Founders all have the starter kit: Locomotor, Sensor, Mouth,
    // Reproductive. Any other module type appearing in the alive
    // population indicates structural mutation introduced it.
    let starter_types: HashSet<ModuleType> =
        [ModuleType::Locomotor, ModuleType::Sensor, ModuleType::Mouth, ModuleType::Reproductive]
            .into_iter()
            .collect();

    let mut seen_novel = false;
    for _ in 0..5_000 {
        step(&mut world);
        for id in world.agents.iter_alive() {
            for m in &world.agents.modules[id as usize] {
                if !starter_types.contains(&m.module_type()) {
                    seen_novel = true;
                    break;
                }
            }
            if seen_novel {
                break;
            }
        }
        if seen_novel {
            break;
        }
    }
    assert!(seen_novel, "no novel module types appeared in 5000 ticks");

    // Sanity: nobody overflows the cap.
    for id in world.agents.iter_alive() {
        assert!(world.agents.modules[id as usize].len() <= MODULE_LIST_MAX);
    }
}
