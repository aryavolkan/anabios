//! Global invariants over any scenario × any seed.

use anabios_core::biome::WORLD_SIZE;
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude_test::Vec2;
use anabios_core::tick::step;
use anabios_core::world::World;
use proptest::prelude::*;

mod common;

fn build_world(seed: u64, agent_count: usize) -> World {
    let mut w = World::new(seed);
    // Invariants hold regardless of scale; cap population so the proptest cases
    // stay fast under the raised 10k default.
    w.max_population = 500;
    for i in 0..agent_count {
        let x = ((i * 17) % 1024) as f32 + 0.5;
        let y = ((i * 31) % 1024) as f32 + 0.5;
        let mut g = Genome::neutral();
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

    /// Every alive agent has a non-zero lineage_id (zero is reserved as
    /// LINEAGE_NONE for "no parent"). Newborns get fresh ids from
    /// `World.next_lineage()`.
    #[test]
    fn alive_agents_have_nonzero_lineage_id(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for id in w.agents.iter_alive() {
            let lin = w.agents.lineage_id[id as usize];
            prop_assert_ne!(lin, anabios_core::agent::LINEAGE_NONE,
                "agent {} has LINEAGE_NONE", id);
        }
    }

    /// Every alive agent's species_id refers to a slot in the species table.
    /// (Both empty and populated species are valid; out-of-range ids are not.)
    #[test]
    fn agent_species_ids_are_valid(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        let max_id = w.species_centroids.len() as u32;
        for id in w.agents.iter_alive() {
            let sid = w.agents.species_id[id as usize];
            prop_assert!(sid < max_id,
                "agent {id} has species_id {sid} but table has {max_id}");
        }
    }

    /// Every non-founder species has a parent recorded in the phylogeny.
    /// Species 0 is the founder.
    #[test]
    fn non_founder_species_have_parents(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for (sid, parent) in w.species_parents.iter().enumerate() {
            if sid == 0 {
                prop_assert_eq!(*parent, None, "species 0 should have no parent");
            } else {
                prop_assert!(parent.is_some(), "species {sid} has no recorded parent");
            }
        }
    }

    /// Every alive agent has at least one module (the structural_mutate
    /// operator preserves the "never empty" invariant).
    #[test]
    fn alive_agents_have_at_least_one_module(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for id in w.agents.iter_alive() {
            let n = w.agents.modules[id as usize].len();
            prop_assert!(n >= 1, "agent {id} has 0 modules");
        }
    }

    /// Module lists never exceed MODULE_LIST_MAX.
    #[test]
    fn modules_respect_max_list_size(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for id in w.agents.iter_alive() {
            let n = w.agents.modules[id as usize].len();
            prop_assert!(n <= anabios_core::module::MODULE_LIST_MAX,
                "agent {id} has {n} modules");
        }
    }

    /// Programs respect the PROGRAM_MAX_NODES hard cap.
    #[test]
    fn programs_respect_max_node_cap(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for id in w.agents.iter_alive() {
            let len = w.agents.program[id as usize].len();
            prop_assert!(len <= anabios_core::program::PROGRAM_MAX_NODES,
                "agent {id} program length {len}");
        }
    }

    /// All codex events reference valid species ids (or the global sentinel).
    #[test]
    fn codex_events_reference_valid_species(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        let max_id = w.species_centroids.len() as u32;
        for ev in &w.codex.events {
            prop_assert!(ev.species_id == u32::MAX || ev.species_id < max_id,
                "event references invalid species {}", ev.species_id);
        }
    }

    /// Codex event locations are finite and within world bounds (or 0,0).
    #[test]
    fn codex_event_locations_in_bounds(
        seed in 0u64..1_000,
        ticks in 0u64..500,
        count in 1usize..30,
    ) {
        let ws = anabios_core::biome::WORLD_SIZE;
        let mut w = build_world(seed, count);
        for _ in 0..ticks {
            step(&mut w);
        }
        for ev in &w.codex.events {
            prop_assert!(ev.loc_x.is_finite() && ev.loc_y.is_finite(),
                "event location not finite");
            prop_assert!(ev.loc_x >= 0.0 && ev.loc_x <= ws,
                "loc_x {} out of bounds", ev.loc_x);
            prop_assert!(ev.loc_y >= 0.0 && ev.loc_y <= ws,
                "loc_y {} out of bounds", ev.loc_y);
        }
    }
}

fn combined_energy(w: &World) -> f32 {
    use anabios_core::interact::FOOD_ENERGY_PER_BIOMASS;
    w.alive_energy_total() + w.plant_biomass_total() * FOOD_ENERGY_PER_BIOMASS
}

/// Territory layer: over a long flag-on run, no Land agent is ever on a Water
/// cell and no Water agent is ever on land (checked every 50 ticks).
#[test]
fn habitat_classes_never_leave_their_terrain() {
    use anabios_core::biome::TerrainType;
    use anabios_core::habitat::Locomotion;
    let mut w = common::world(include_str!("../../../scenarios/habitat-territories.toml"));
    assert!(w.territory_enabled);
    let horizon = common::ticks(2000);
    while w.tick < horizon {
        common::run(&mut w, 50);
        for id in w.agents.iter_alive() {
            let i = id as usize;
            let class = Locomotion::of(&w.agents.genome[i]);
            let t = w.biome.sample(w.agents.position[i]).terrain;
            match class {
                Locomotion::Land => {
                    assert_ne!(t, TerrainType::Water, "tick {} land agent {id} in water", w.tick)
                }
                Locomotion::Water => {
                    assert_eq!(t, TerrainType::Water, "tick {} water agent {id} on {t:?}", w.tick)
                }
                Locomotion::Air => {}
            }
        }
    }
}

/// Measurement probe (not a gate): 8 seeds × 20k ticks of the flagship
/// scenario, reporting per-class populations, habitat violations, deep
/// overlaps (colliding pairs closer than half their gap), shallow overlaps
/// (colliding pairs closer than their gap but at least half of it — still
/// touching, less severely), and the share of members inside their species'
/// territory. Run:
///   cargo test -p anabios-core --release --test invariants \
///     territory_measurement_probe -- --ignored --nocapture
#[test]
#[ignore]
fn territory_measurement_probe() {
    use anabios_core::biome::TerrainType;
    use anabios_core::collision::body_radius;
    use anabios_core::habitat::Locomotion;
    use anabios_core::scenario::Scenario;
    let base = include_str!("../../../scenarios/habitat-territories.toml");
    for seed in 1..=8u64 {
        let mut s = Scenario::parse_toml(base).expect("parse");
        s.seed = seed;
        let mut w = s.instantiate();
        common::run(&mut w, 20_000);
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        let mut pop = [0u32; 3];
        let mut inside_by_class = [0u32; 3];
        let (mut violations, mut deep, mut shallow, mut inside) = (0u32, 0u32, 0u32, 0u32);
        for &id in &ids {
            let i = id as usize;
            let c = Locomotion::of(&w.agents.genome[i]);
            pop[c.index()] += 1;
            let t = w.biome.sample(w.agents.position[i]).terrain;
            if !c.can_occupy(t) {
                violations += 1;
            }
            if let Some(tr) = w.species_territories.get(w.agents.species_id[i] as usize) {
                let d = anabios_core::spatial::torus_distance(
                    tr.centre(),
                    w.agents.position[i],
                    w.world_size,
                );
                if tr.is_set() && d <= tr.r {
                    inside += 1;
                    inside_by_class[c.index()] += 1;
                }
            }
        }
        // Per-class inside % (diagnosis §5 H5): the aggregate figure is
        // composition-dominated (Land ~100%, Water/Air vary widely), so a
        // single number can move purely because which classes survive
        // changed, not because containment did. "-" marks an extinct class.
        let inside_pct_by_class = |k: usize| {
            if pop[k] == 0 {
                "-".to_string()
            } else {
                format!("{:.1}", 100.0 * inside_by_class[k] as f32 / pop[k] as f32)
            }
        };
        for (k, &a) in ids.iter().enumerate() {
            for &b in &ids[k + 1..] {
                let (ga, gb) = (&w.agents.genome[a as usize], &w.agents.genome[b as usize]);
                if !Locomotion::of(ga).collides_with(Locomotion::of(gb)) {
                    continue;
                }
                let gap = body_radius(ga) + body_radius(gb);
                let d = anabios_core::spatial::torus_distance(
                    w.agents.position[a as usize],
                    w.agents.position[b as usize],
                    w.world_size,
                );
                if d < 0.5 * gap {
                    deep += 1;
                } else if d < gap {
                    shallow += 1;
                }
            }
        }
        let n = ids.len().max(1) as f32;
        println!(
            "seed={seed} alive={} land={} water={} air={} violations={violations} deep_overlaps={deep} shallow_overlaps={shallow} inside_territory={:.1}% inside_by_class(L/W/A)={}/{}/{} water_cells_with_biomass={}",
            ids.len(), pop[0], pop[1], pop[2], 100.0 * inside as f32 / n,
            inside_pct_by_class(0), inside_pct_by_class(1), inside_pct_by_class(2),
            w.biome.cells.iter().filter(|c| c.terrain == TerrainType::Water && c.plant_biomass > 0.1).count(),
        );
    }
}
