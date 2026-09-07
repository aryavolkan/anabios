//! Core substrate invariants: feeding, reproduction, speciation, module
//! gating, and the codex/serde audits. One module per former binary.

mod feeding {
    //! Integration test: a herbivore population on grass survives 500 ticks
    //! without total collapse or runaway plant blow-up.

    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

    #[test]
    fn population_persists_for_500_ticks() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        let mut world = scenario.instantiate();
        let initial_alive = world.agents.live_count();
        assert!(initial_alive > 0);

        let initial_biomass = world.plant_biomass_total();
        assert!(initial_biomass > 0.0);

        for _ in 0..500 {
            step(&mut world);
        }

        let final_alive = world.agents.live_count();
        let final_biomass = world.plant_biomass_total();
        // Population must persist (M2 dynamic: reproduction sustains the pop).
        assert!(
            final_alive > 0,
            "population went extinct in 500 ticks: {} -> {}",
            initial_alive,
            final_alive
        );
        // Biomass should remain in a reasonable band — not zero, not multiples
        // of carrying capacity.
        assert!(final_biomass > 0.0);
        assert!(final_biomass < initial_biomass * 1.5);
    }
}

mod speciation {
    //! Integration test: two genetically-distant founder populations should be
    //! recognized as separate species by the first time `species_step` runs
    //! (tick 200) or shortly after.

    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/divergent.toml");

    #[test]
    fn distant_founder_populations_become_separate_species() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        let mut world = scenario.instantiate();
        // The split is genetic, not population-driven — cap population so the run
        // stays fast under the raised 10k default.
        world.max_population = 500;

        // Run past the first speciation event (200 ticks) plus a buffer for
        // the algorithm to recognize the split.
        for _ in 0..400 {
            step(&mut world);
        }

        // At least two non-empty species expected.
        let non_empty: usize = world.species_member_counts.iter().filter(|&&c| c > 0).count();
        assert!(
            non_empty >= 2,
            "expected speciation, got species member counts {:?}",
            world.species_member_counts,
        );

        // At least one species has a recorded parent (non-founder).
        let any_child = world.species_parents.iter().any(|p| p.is_some());
        assert!(any_child, "no non-founder species recorded in phylogeny");
    }
}

mod reproduction {
    //! Integration test: with reproduction (M2), the minimal scenario must
    //! sustain its population over a window longer than the natural lifespan,
    //! confirming that newborns are replacing deaths.

    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

    #[test]
    fn population_sustains_past_one_lifespan() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        let mut world = scenario.instantiate();
        // Sustaining a population past a lifespan doesn't need scale — cap it so the
        // 5,000-tick run stays fast under the raised 10k default.
        world.max_population = 500;
        let initial_alive = world.agents.live_count();
        assert!(initial_alive > 0);

        // Run for 5,000 ticks — well past the natural lifespan (≈ 3,200 ticks
        // at LifespanBias = 0.6).
        for _ in 0..5_000 {
            step(&mut world);
        }

        let final_alive = world.agents.live_count();
        assert!(
            final_alive > 0,
            "population should sustain past one lifespan; initial={initial_alive}, final={final_alive}",
        );
    }

    /// O3 repro-biased learning: birth outcomes are counted iff the flag is on.
    /// Same scenario, same seed, flag toggled — the flag-off world must keep every
    /// counter at zero (the byte-identity contract), the flag-on world must have
    /// credited surviving births to parents.
    #[test]
    fn birth_outcome_counters_are_flag_gated() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");

        let mut on = scenario.instantiate();
        on.repro_biased_learning = true;
        on.max_population = 500;
        let mut off = scenario.instantiate();
        off.max_population = 500;

        for _ in 0..2_000 {
            step(&mut on);
            step(&mut off);
        }

        let sum_ok: u32 = on.agents.births_ok.iter().map(|&b| b as u32).sum();
        assert!(sum_ok > 0, "flag on: surviving births must be credited to parents");
        let off_total: u32 = off
            .agents
            .births_ok
            .iter()
            .chain(off.agents.births_failed.iter())
            .map(|&b| b as u32)
            .sum();
        assert_eq!(off_total, 0, "flag off: no birth-outcome counting at all");
    }
}

mod module_gating {
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
                    spawn =
                        Vec2::new((col as f32 + 0.5) * CELL_SIZE, (row as f32 + 0.5) * CELL_SIZE);
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
}

mod codex_events {
    //! Integration test: codex emits SpeciationEvent on a divergent scenario
    //! where two distant founder populations are forced to split.

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/divergent.toml");

    #[test]
    fn divergent_scenario_emits_speciation_event() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        let mut world = scenario.instantiate();
        // Speciation needs divergence between the two founder clusters, not scale —
        // cap population so the test stays fast under the raised 10k default.
        world.max_population = 500;

        // 400 ticks is well past the first species_step (at tick 200).
        for _ in 0..400 {
            step(&mut world);
        }

        let saw_speciation =
            world.codex.events.iter().any(|ev| ev.event_type == EventType::SpeciationEvent);
        assert!(
            saw_speciation,
            "expected at least one SpeciationEvent; got {:?}",
            world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
        );
    }

    const AFFECT_SHOWCASE: &str = include_str!("../../../scenarios/affect-showcase.toml");

    #[test]
    fn affect_showcase_emits_an_affect_event() {
        let scenario = Scenario::parse_toml(AFFECT_SHOWCASE).expect("parse affect showcase");
        let mut world = scenario.instantiate();
        assert!(world.affect_enabled, "showcase scenario must enable affect");
        for _ in 0..800 {
            step(&mut world);
        }
        let saw = world.codex.events.iter().any(|e| {
            matches!(
                e.event_type,
                EventType::FeedingFrenzy
                    | EventType::PanicCascade
                    | EventType::TerritorialRage
                    | EventType::MassGrief,
            )
        });
        assert!(
            saw,
            "expected an affect event; got {:?}",
            world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
        );
    }
}

mod serde_skip_audit {
    //! `#[serde(skip)]` audit guard: every skipped cache that must be re-derived
    //! at load time is actually re-derived. `state_hash` hashes exactly the
    //! serialized fields, so a skipped field that feeds future ticks breaks
    //! replay invisibly (the v13 `still_ticks` footgun, and the spatial-hash dims
    //! bug found by `save_load_roundtrip` — see `docs/determinism-contract.md`).

    use anabios_core::scenario::Scenario;
    use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
    use anabios_core::tick::step;

    /// A pheromone-active + domestication scenario, warmed so both documented
    /// re-derivation caches are non-default: after load, `track_livestock` must
    /// be re-derived from the persisted flag (not left false) and the pheromone
    /// nonzero cache must match a fresh recompute (decay would no-op otherwise).
    #[test]
    fn load_rederives_skipped_caches() {
        let mut w = Scenario::parse_toml(include_str!("../../../scenarios/domestication.toml"))
            .unwrap()
            .instantiate();
        for _ in 0..300 {
            step(&mut w);
        }
        let reloaded = load_from_bytes(&save_to_bytes(&w).unwrap()).unwrap();
        // track_livestock must be re-derived from the persisted flag, not left false.
        assert_eq!(reloaded.agents.track_livestock, reloaded.domestication_enabled);
        // pheromone nonzero cache must match a fresh recompute (decay would no-op
        // otherwise) — identity of the full serialized state covers it.
        assert_eq!(state_hash(&w), state_hash(&reloaded));
    }

    /// The serde-skipped spatial hashes reset to `Default` (1024/64) on load;
    /// `load_from_bytes` must re-derive them from the persisted world dims, or a
    /// custom-dims world reloads with the wrong `cell_size` (clamping perception
    /// radii wrong) and wrong torus extent — the bug `season_period_roundtrip`
    /// and `living_biome_roundtrip` surfaced. Pin the re-derivation directly.
    #[test]
    fn load_rederives_spatial_hash_dims() {
        let mut w = Scenario::parse_toml(include_str!("../../../scenarios/sandbox-large.toml"))
            .unwrap()
            .instantiate();
        assert_eq!(w.world_size, 2048.0, "scenario pins non-default dims");
        for _ in 0..10 {
            step(&mut w);
        }
        let reloaded = load_from_bytes(&save_to_bytes(&w).unwrap()).unwrap();
        let want = w.spatial.perception_max_radius();
        for (name, got) in [
            ("spatial", reloaded.spatial.perception_max_radius()),
            ("carcass_spatial", reloaded.carcass_spatial.perception_max_radius()),
            ("resource_spatial", reloaded.resource_spatial.perception_max_radius()),
        ] {
            assert_eq!(got, want, "{name} reloaded with default dims (1024/64)");
        }
    }
}
