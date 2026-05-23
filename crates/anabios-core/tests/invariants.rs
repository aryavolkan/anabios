//! Global invariants over any scenario × any seed.

use anabios_core::biome::WORLD_SIZE;
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude_test::Vec2;
use anabios_core::tick::step;
use anabios_core::world::World;
use proptest::prelude::*;

fn build_world(seed: u64, agent_count: usize) -> World {
    let mut w = World::new(seed);
    for i in 0..agent_count {
        let x = ((i * 17) % 1024) as f32 + 0.5;
        let y = ((i * 31) % 1024) as f32 + 0.5;
        let mut g = Genome::neutral();
        g.set(GenomeSlot::SpeedMax, 0.4);
        g.set(GenomeSlot::DietCarnivory, 0.0);
        g.set(GenomeSlot::Size, 0.4);
        g.set(GenomeSlot::LifespanBias, 0.5);
        w.spawn_agent(Vec2::new(x, y), g);
    }
    w
}

proptest! {
    /// All agent positions are inside the world bounds after any number of ticks.
    #[test]
    fn positions_stay_in_world(seed in 0u64..1_000, ticks in 0u64..500, count in 0usize..50) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for id in w.agents.iter_alive() {
            let p = w.agents.position[id as usize];
            prop_assert!(p.x >= 0.0 && p.x < WORLD_SIZE,
                "x out of range: {} (seed={seed} ticks={ticks})", p.x);
            prop_assert!(p.y >= 0.0 && p.y < WORLD_SIZE,
                "y out of range: {} (seed={seed} ticks={ticks})", p.y);
        }
    }

    /// Total plant biomass + agent energy can only grow due to regrowth, never
    /// from feeding alone. So between two adjacent non-regrowth ticks, total
    /// (biomass*FOOD_ENERGY_PER_BIOMASS + energy) should be non-increasing.
    #[test]
    fn energy_plus_biomass_does_not_grow_between_regrowth_ticks(
        seed in 0u64..1_000,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        // Drive the tick forward to a non-regrowth boundary first.
        step(&mut w);
        let before = combined_energy(&w);
        // Take 9 more steps to land just before the next regrowth tick
        // (BIOME_STEP_INTERVAL = 10).
        for _ in 0..8 {
            step(&mut w);
            let now = combined_energy(&w);
            prop_assert!(now <= before + 1e-1,
                "energy grew without regrowth: before={before} now={now}");
        }
    }

    /// Agent ids are never re-used while the original slot is still alive.
    #[test]
    fn ids_unique_among_alive(seed in 0u64..1_000, ticks in 0u64..200, count in 0usize..40) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        let mut sorted = alive.clone();
        sorted.sort();
        sorted.dedup();
        prop_assert_eq!(alive.len(), sorted.len());
    }
}

fn combined_energy(w: &World) -> f32 {
    use anabios_core::interact::FOOD_ENERGY_PER_BIOMASS;
    w.alive_energy_total() + w.plant_biomass_total() * FOOD_ENERGY_PER_BIOMASS
}
