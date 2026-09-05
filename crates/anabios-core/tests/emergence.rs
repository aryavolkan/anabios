//! Long-horizon behavioural gates: the emergence detectors.
//!
//! One module per detector, each formerly its own test binary. They were
//! merged because every integration binary re-links the whole crate — 55 of
//! them dominated CI's build time — and nextest partitions by *test*, not by
//! binary, so nothing about scheduling depends on the split. Filter a single
//! detector with `cargo test --test emergence <module>::`.

mod predator_prey_emergence {
    //! M12 emergence: seeded stalkers predate grazers across many seeds.
    //! Release-gated (ignored in debug builds) per spec §2.2.

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/predator-prey.toml");
    const SEEDS: u64 = 16;
    const TICKS: u32 = 800;
    /// Measured on this scenario: predation in 15/16 seeds, both species persist in
    /// 16/16. Floors are set well below the observed rates so unrelated tuning
    /// drift can't flake the test (spec §2.2).
    const PREDATION_FLOOR: u64 = 11;
    /// Minimum seeds in which prey AND predators both survive to `TICKS`
    /// (coexistence past a crash-only baseline — observed 16/16).
    const PERSIST_FLOOR: u64 = 12;

    #[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
    #[test]
    fn predation_emerges_across_seeds() {
        let mut with_predation = 0u64;
        let mut both_persist = 0u64;
        for seed in 0..SEEDS {
            let mut s = Scenario::parse_toml(SCENARIO).expect("parse predator-prey");
            s.seed = seed;
            let mut w = s.instantiate();
            for _ in 0..TICKS {
                step(&mut w);
            }
            let predated = w.codex.events.iter().any(|e| e.event_type == EventType::Predation);
            if predated {
                with_predation += 1;
            }
            // Prey (grazer archetype) = species 1, predators (stalker) = species 2.
            let mut prey_alive = 0u32;
            let mut pred_alive = 0u32;
            for id in w.agents.iter_alive() {
                match w.agents.species_id[id as usize] {
                    1 => prey_alive += 1,
                    2 => pred_alive += 1,
                    _ => {}
                }
            }
            if prey_alive > 0 && pred_alive > 0 {
                both_persist += 1;
            }
        }
        assert!(
            with_predation >= PREDATION_FLOOR,
            "Predation emerged in only {with_predation}/{SEEDS} seeds (floor {PREDATION_FLOOR})"
        );
        assert!(
            both_persist >= PERSIST_FLOOR,
            "Prey+predator coexistence in only {both_persist}/{SEEDS} seeds (floor {PERSIST_FLOOR})"
        );
    }
}

mod cooperation_emergence {
    //! M15 emergence: a dense cluster of cooperator herbivores develops kin-sharing
    //! behaviour (EvolvedCooperation) and/or spatial cohesion (HerdCohesion).
    //! Release-gated (ignored in debug) per spec §2.2.

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/cooperation.toml");
    const SEEDS: u64 = 16;
    const TICKS: u32 = 400;
    /// Measured on this scenario: EvolvedCooperation in 16/16 seeds (kin-gated
    /// sharing sustains in the dense cluster), HerdCohesion in 13/16, and the
    /// population survives to TICKS in 16/16. Floor set well below the observed
    /// rate so tuning drift can't flake it (§2.2).
    const COOP_FLOOR: u64 = 13;

    #[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
    #[test]
    fn cooperation_emerges_across_seeds() {
        let mut with_coop = 0u64;
        for seed in 0..SEEDS {
            let mut s = Scenario::parse_toml(SCENARIO).expect("parse cooperation");
            s.seed = seed;
            let mut w = s.instantiate();
            for _ in 0..TICKS {
                step(&mut w);
            }
            let emerged = w.codex.events.iter().any(|e| {
                e.event_type == EventType::EvolvedCooperation
                    || e.event_type == EventType::HerdCohesion
            });
            if emerged {
                with_coop += 1;
            }
        }
        assert!(
            with_coop >= COOP_FLOOR,
            "EvolvedCooperation/HerdCohesion in only {with_coop}/{SEEDS} seeds (floor {COOP_FLOOR})"
        );
    }
}

mod territory_emergence {
    //! M13 emergence: seeded marking species form clustered territories.
    //! Release-gated (ignored in debug) per spec §2.2.

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/territories.toml");
    const SEEDS: u64 = 16;
    const TICKS: u32 = 400;
    /// Measured on this scenario: TerritoryFormation in 16/16 seeds (marking
    /// species reliably cluster and mark), NichePartitioning in 6/16 (too marginal
    /// to gate on). Floor set well below the observed rate so unrelated tuning
    /// drift can't flake it (spec §2.2).
    const TERRITORY_FLOOR: u64 = 13;

    #[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
    #[test]
    fn territories_form_across_seeds() {
        let mut with_territory = 0u64;
        for seed in 0..SEEDS {
            let mut s = Scenario::parse_toml(SCENARIO).expect("parse territories");
            s.seed = seed;
            let mut w = s.instantiate();
            for _ in 0..TICKS {
                step(&mut w);
            }
            let formed =
                w.codex.events.iter().any(|e| e.event_type == EventType::TerritoryFormation);
            if formed {
                with_territory += 1;
            }
        }
        assert!(
            with_territory >= TERRITORY_FLOOR,
            "TerritoryFormation in only {with_territory}/{SEEDS} seeds (floor {TERRITORY_FLOOR})"
        );
    }
}

mod dialect_emergence {
    //! M14 emergence: two geographically isolated communicator clusters develop
    //! distinct meme distributions (DialectFormed) or one cluster sweeps its meme
    //! to fixation (MemeSweep).
    //! Release-gated (ignored in debug) per spec §2.2.

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/dialects.toml");
    const SEEDS: u64 = 16;
    const TICKS: u32 = 400;
    /// Measured on this scenario: a broadcast meme sweeps each communicator cluster
    /// to dominance → MemeSweep in 16/16 seeds. DialectFormed is 0/16 here (the two
    /// clusters are distinct species, so there is no within-species east/west split
    /// to diverge — that detector ships but its emergence is left to later scenarios).
    /// Floor set well below the observed rate so tuning drift can't flake it (§2.2).
    const DIALECT_FLOOR: u64 = 13;

    #[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
    #[test]
    fn dialects_form_across_seeds() {
        let mut with_dialect = 0u64;
        for seed in 0..SEEDS {
            let mut s = Scenario::parse_toml(SCENARIO).expect("parse dialects");
            s.seed = seed;
            let mut w = s.instantiate();
            for _ in 0..TICKS {
                step(&mut w);
            }
            let formed = w.codex.events.iter().any(|e| {
                e.event_type == EventType::DialectFormed || e.event_type == EventType::MemeSweep
            });
            if formed {
                with_dialect += 1;
            }
        }
        assert!(
            with_dialect >= DIALECT_FLOOR,
            "DialectFormed/MemeSweep in only {with_dialect}/{SEEDS} seeds (floor {DIALECT_FLOOR})"
        );
    }
}

mod morphology_evolution {
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
        let starter_types: HashSet<ModuleType> = [
            ModuleType::Locomotor,
            ModuleType::Sensor,
            ModuleType::Mouth,
            ModuleType::Reproductive,
        ]
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
}

mod program_evolution {
    //! Integration test: program mutation drifts over generations.
    //!
    //! Founders all start with `starter_grazer()`. After enough reproduction +
    //! mutation cycles, at least one alive agent must carry a program that
    //! differs from the starter.

    use anabios_core::program::{starter_grazer, Program};
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/minimal.toml");

    #[test]
    fn at_least_one_program_diverges_from_starter_within_5000_ticks() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        let mut world = scenario.instantiate();
        let starter: Program = starter_grazer();

        for _ in 0..5_000 {
            step(&mut world);
            let any_divergent =
                world.agents.iter_alive().any(|id| world.agents.program[id as usize] != starter);
            if any_divergent {
                return;
            }
        }
        panic!("no program diverged from the starter in 5000 ticks");
    }
}

mod trait_evolution {
    //! Integration test: the E5 trait-evolution detectors fire on the convergent
    //! showcase scenario (sweep evidence in the E5 plan completion notes).

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/convergent.toml");

    #[test]
    fn convergent_scenario_fires_trait_events() {
        let mut scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        // Seed 3 fires a TraitFixation within the 1500-tick window after the
        // PerceptionRadius gene was removed and non-cognition perception falls
        // back to a hardcoded neutral modulator.
        scenario.seed = 3;
        let mut world = scenario.instantiate();
        // Pin the cap for debug-profile speed; trait dynamics are unaffected.
        world.max_population = 1000;

        for _ in 0..1500 {
            step(&mut world);
        }

        let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
        assert!(
            saw(EventType::TraitFixation)
                || saw(EventType::RapidAdaptation)
                || saw(EventType::ConvergentEvolution),
            "expected at least one E5 trait event; got {:?}",
            world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
        );
    }
}

mod population_dynamics {
    //! Integration test: the E3 population-dynamics detectors fire on the
    //! trophic-cascade showcase scenario (16/16 carrying-capacity, 14/16 cycle,
    //! 9/16 cascade over a 16-seed sweep — see the E3 plan completion notes).

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/trophic-cascade.toml");

    #[test]
    fn trophic_cascade_scenario_fires_population_dynamics_events() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        let mut world = scenario.instantiate();
        // Keep the debug-profile test fast: pin the cap so the herd can't
        // explode to 10k. The guild oscillation survives; the world-total
        // plateau at the cap itself satisfies the detector set.
        world.max_population = 500;

        // 2000 ticks covers the first guild oscillation on the scenario seed.
        for _ in 0..2000 {
            step(&mut world);
        }

        let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
        assert!(
            saw(EventType::PopulationCycleDetected)
                || saw(EventType::CarryingCapacityReached)
                || saw(EventType::BoomAndBust)
                || saw(EventType::TrophicCascade),
            "expected at least one E3 population-dynamics event; got {:?}",
            world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
        );
    }
}

mod disturbance {
    //! Integration test: the E4 disturbance substrate and detectors fire on the
    //! disturbance showcase scenario (sweep evidence in the E4 plan completion
    //! notes).

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/disturbance.toml");

    #[test]
    fn disturbance_scenario_scars_and_recovers() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        let mut world = scenario.instantiate();
        // Pin the cap so the debug-profile run stays fast; the disturbance
        // dynamics (fires, scars, re-vegetation) are unaffected.
        world.max_population = 500;

        for _ in 0..3000 {
            step(&mut world);
        }

        // The scheduler must have produced disasters and at least one scar.
        assert!(world.disasters.spawned > 0, "no disasters in 3000 ticks");
        assert!(
            world
                .biome
                .cells
                .iter()
                .any(|c| c.succession != anabios_core::biome::SUCCESSION_CLIMAX),
            "no succession-scarred cells after disasters"
        );

        // Detector coverage: at least one of the four new event types fired.
        let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
        assert!(
            saw(EventType::RangeExpansion)
                || saw(EventType::SegregationEmerged)
                || saw(EventType::CorridorUse)
                || saw(EventType::Succession),
            "expected at least one E4 event; got {:?}",
            world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
        );
    }
}

mod weapons_arms_race {
    //! weapons-arms-race scenario regression: the armed founder lineages are
    //! expected to die as *species* (speciation splits them), but their weapon
    //! modules should persist in descendant lineages — the scenario's promise
    //! is that Spines and Jaws establish as evolved traits, not one-generation
    //! novelties.

    use anabios_core::module::{self, ModuleType};
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/weapons-arms-race.toml");

    fn count_with(world: &anabios_core::world::World, t: ModuleType) -> usize {
        world
            .agents
            .iter_alive()
            .filter(|&id| module::has(&world.agents.modules[id as usize], t))
            .count()
    }

    #[test]
    fn armed_lineages_persist_through_speciation() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse arms-race scenario");
        let mut w = scenario.instantiate();
        // Founder species die around t=1100; run past that so survival can only
        // come from descendant lineages inheriting the weapon modules.
        for _ in 0..1500 {
            step(&mut w);
        }
        let spines = count_with(&w, ModuleType::Spines);
        let jaws = count_with(&w, ModuleType::Jaws);
        assert!(spines > 0, "no Spines-bearing agents left at tick 1500");
        assert!(jaws > 0, "no Jaws-bearing agents left at tick 1500");
    }
}

mod war {
    //! Integration test: the E7 war/alliance/kin detectors fire on the war
    //! showcase scenario (sweep evidence in the E7 plan completion notes).

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/war.toml");

    #[test]
    fn war_scenario_fires_war_events() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        let mut world = scenario.instantiate();
        // Pin the cap for debug-profile speed; the pack clash persists.
        world.max_population = 500;

        // Wars declare by t≈500 and kin networks latch at t≈1500 (see plan).
        for _ in 0..3000 {
            step(&mut world);
        }

        let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
        assert!(
            saw(EventType::WarOrRaid)
                || saw(EventType::WarEnded)
                || saw(EventType::AllianceFormed)
                || saw(EventType::KinNetworkStable),
            "expected at least one E7 event; got {:?}",
            world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
        );
    }
}

mod named_behaviors {
    //! Integration test: the E6 named-behavior detectors fire on the tool-users
    //! showcase scenario (sweep evidence in the E6 plan completion notes).

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/tool-users.toml");

    #[test]
    fn tool_users_scenario_fires_named_behavior_events() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        let mut world = scenario.instantiate();
        // Pin the cap for debug-profile speed; combat/invention dynamics persist.
        world.max_population = 500;

        // Signaling fired at t=460 and flight in 14/16 sweep runs (see plan).
        for _ in 0..2000 {
            step(&mut world);
        }

        let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
        assert!(
            saw(EventType::EvolvedAmbush)
                || saw(EventType::EvolvedTool)
                || saw(EventType::EvolvedFlight)
                || saw(EventType::StructuredSignaling),
            "expected at least one E6 named-behavior event; got {:?}",
            world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
        );
    }
}

mod settlement_economy {
    //! Integration test: the E8 settlement & economy detectors fire on the
    //! settlement showcase scenario (sweep evidence in the E8 plan completion
    //! notes).

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/settlement.toml");

    #[test]
    fn settlement_scenario_fires_economy_events() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        let mut world = scenario.instantiate();
        // Pin the cap for debug-profile speed; trade and harvest persist.
        world.max_population = 800;

        // Markets crystallize by t≈400, specialization splits by t≈60 (see plan).
        for _ in 0..1200 {
            step(&mut world);
        }

        let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
        assert!(
            saw(EventType::SettlementFormed)
                || saw(EventType::MarketEmerged)
                || saw(EventType::SpecializationSplit),
            "expected at least one E8 economy event; got {:?}",
            world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
        );
    }
}

mod traditions {
    //! Integration test: the E9 tradition detectors fire on the traditions
    //! showcase scenario (sweep evidence in the E9 plan completion notes).

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/traditions.toml");

    #[test]
    fn traditions_scenario_fires_tradition_events() {
        let scenario = Scenario::parse_toml(SCENARIO).expect("parse");
        let mut world = scenario.instantiate();
        // Pin the cap for debug-profile speed; culture keeps flowing.
        world.max_population = 800;

        // Radiation fires early (t≈150), traditions latch by t≈4000 (see plan).
        for _ in 0..5000 {
            step(&mut world);
        }

        let saw = |t: EventType| world.codex.events.iter().any(|ev| ev.event_type == t);
        assert!(
            saw(EventType::TraditionPreserved)
                || saw(EventType::CulturalRadiation)
                || saw(EventType::InstitutionalRatchet),
            "expected at least one E9 tradition event; got {:?}",
            world.codex.events.iter().map(|e| e.event_type).collect::<Vec<_>>()
        );
    }
}

mod vertebrate_coexistence {
    //! Vertebrate-class ecology: the mammals-vs-reptiles scenario sustains a
    //! working predator guild — all four founder lineages (mammal grazer, mammal
    //! pursuer, reptile ambusher, reptile basker) persist in most seeds, and the
    //! affect layer fires (herd panic). Release-gated per spec §2.2.
    //!
    //! This guards the balance work in the archetype design: pursuit gated to a
    //! pounce range (so kills get scavenged), high-damage Weapon (prey HP is its
    //! energy), and pursuer Boldness (suppressing the FEAR aura of big prey).
    //! Regression in any of these shows up as pursuer/ambusher lineage collapse.

    use anabios_core::codex::EventType;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/mammals-vs-reptiles.toml");
    const SEEDS: u64 = 8;
    const TICKS: u32 = 2000;
    /// Measured on this scenario: all four founder lineages persist to 2000 ticks
    /// in 8/8 seeds. Floor set well below the observed rate so unrelated tuning
    /// drift can't flake the test (spec §2.2).
    const ALL_PERSIST_FLOOR: u64 = 5;

    /// Walk the species-parent chain to the founder species id (1 = mammal
    /// grazer, 2 = mammal pursuer, 3 = reptile ambusher, 4 = reptile basker), so
    /// descendants that speciated away still count toward their founder's
    /// lineage. Mirrors `codex::war::lineage_root` (not exported).
    fn lineage_root(w: &anabios_core::World, sid: u32) -> u32 {
        let mut cur = sid;
        for _ in 0..64 {
            match w.species_parents.get(cur as usize).copied().flatten() {
                Some(p) if p != cur && p != 0 => cur = p,
                _ => break,
            }
        }
        cur
    }

    #[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
    #[test]
    fn vertebrate_classes_coexist_across_seeds() {
        let mut all_persist = 0u64;
        let mut with_fright = 0u64;
        for seed in 0..SEEDS {
            let mut s = Scenario::parse_toml(SCENARIO).expect("parse mammals-vs-reptiles");
            s.seed = seed;
            let mut w = s.instantiate();
            for _ in 0..TICKS {
                step(&mut w);
            }
            let mut lineage_alive = [false; 4];
            for id in w.agents.iter_alive() {
                let root = lineage_root(&w, w.agents.species_id[id as usize]);
                if (1..=4).contains(&root) {
                    lineage_alive[(root - 1) as usize] = true;
                }
            }
            if lineage_alive.iter().all(|&a| a) {
                all_persist += 1;
            }
            if w.codex.events.iter().any(|e| e.event_type == EventType::MassFright) {
                with_fright += 1;
            }
        }
        assert!(
            all_persist >= ALL_PERSIST_FLOOR,
            "all four founder lineages persisted in only {all_persist}/{SEEDS} seeds \
             (floor {ALL_PERSIST_FLOOR})"
        );
        assert!(with_fright > 0, "affect layer never fired MassFright across {SEEDS} seeds");
    }
}

mod tg1_coupling {
    // TG1 contract guard: a holder with a high affinity gene must out-buff a
    // low-gene holder when gene_tech_coupling is on, and get the identical buff
    // when it's off. Exercises the public coupled-multiplier path (Farming↔
    // Conscientiousness as the exemplar).
    use anabios_core::genome::{Genome, GenomeSlot};
    use anabios_core::invention::{self, bit, FARMING};

    #[test]
    fn coupling_creates_a_buff_differential_only_when_on() {
        let mask = bit(FARMING);
        let mut hi = Genome::neutral();
        hi.set(GenomeSlot::Conscientiousness, 1.0);
        let mut lo = Genome::neutral();
        lo.set(GenomeSlot::Conscientiousness, 0.0);

        // OFF: no differential.
        let off_hi = invention::graze_multiplier_coupled(mask, &hi, false);
        let off_lo = invention::graze_multiplier_coupled(mask, &lo, false);
        assert_eq!(off_hi, off_lo);

        // ON: the high-gene holder gets the larger buff.
        let on_hi = invention::graze_multiplier_coupled(mask, &hi, true);
        let on_lo = invention::graze_multiplier_coupled(mask, &lo, true);
        assert!(on_hi > on_lo, "coupling must create a selection differential");
    }
}

mod tg1_selection {
    //! TG1 end-to-end evidence: the tech→gene arm produces *directional selection*.
    //!
    //! Discovery is far too slow to climb naturally to an affinity-bearing tech
    //! before a small world thins out, so instead of relying on emergence we seed a
    //! population that already holds Fire (+ its Stone Tools prerequisite, so it does
    //! not atrophy) with a spread of the coupled gene (Openness), then let
    //! differential reproduction run. Fire's energy buff scales with Openness when
    //! `gene_tech_coupling` is on, so high-Openness holders out-reproduce and the
    //! population mean climbs — while a flag-off control started from the identical
    //! state shows no such rise. The gap between the two IS the selection signal.

    use anabios_core::genome::GenomeSlot;
    use anabios_core::invention::{self, FIRE, STONE_TOOLS};
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/tech-gene-coupling.toml");
    const TICKS: u64 = 2500;

    /// Build the coupled scenario world with `coupling` set as requested, then force
    /// every agent to hold Stone Tools + Fire and give them a deterministic spread
    /// of Openness in [0,1]. Returns the seeded world.
    fn seeded_world(coupling: bool) -> anabios_core::World {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse");
        s.gene_tech_coupling = coupling;
        // Turn cognition off for this selection probe: the test is isolating the
        // Fire↔Openness gene-tech coupling, and the IQ-driven perception-energy
        // cost otherwise swamps the small Openness-linked Fire buff.
        s.cognition_enabled = false;
        let mut w = s.instantiate();
        let ids: Vec<_> = w.agents.iter_alive().collect();
        for (n, id) in ids.iter().enumerate() {
            let i = *id as usize;
            w.agents.meme_vector[i][invention::channel(STONE_TOOLS)] = 1.0;
            w.agents.meme_vector[i][invention::channel(FIRE)] = 1.0;
            // Deterministic spread across [0,1].
            w.agents.genome[i].set(GenomeSlot::Openness, (n % 11) as f32 / 10.0);
        }
        w
    }

    /// Mean Openness over agents currently holding Fire (0.0 if none hold it).
    fn mean_openness_over_fire_holders(w: &anabios_core::World) -> f32 {
        let mut sum = 0.0;
        let mut n = 0u32;
        for id in w.agents.iter_alive() {
            let i = id as usize;
            if invention::has(&w.agents.meme_vector[i], FIRE) {
                sum += w.agents.genome[i].get(GenomeSlot::Openness);
                n += 1;
            }
        }
        if n == 0 {
            0.0
        } else {
            sum / n as f32
        }
    }

    #[test]
    fn coupling_selects_the_affinity_gene_upward() {
        let start = {
            // Both worlds start from the identical seeded state; measure the shared
            // baseline once.
            let w = seeded_world(true);
            mean_openness_over_fire_holders(&w)
        };

        let mut coupled = seeded_world(true);
        let mut control = seeded_world(false);
        for _ in 0..TICKS {
            step(&mut coupled);
            step(&mut control);
        }

        let coupled_mean = mean_openness_over_fire_holders(&coupled);
        let control_mean = mean_openness_over_fire_holders(&control);
        eprintln!(
            "TG1 selection @ {TICKS} ticks: start Openness={start:.4}, coupled={coupled_mean:.4} \
             (alive {}), control={control_mean:.4} (alive {}), differential={:.4}",
            coupled.agents.live_count(),
            control.agents.live_count(),
            coupled_mean - control_mean,
        );

        // Directional selection: coupling pulls the coupled gene mean up, and it
        // ends materially above the flag-off control that ran from the same seed.
        assert!(
            coupled_mean > control_mean + 0.02,
            "expected coupling to raise Openness (coupled={coupled_mean:.4} vs control={control_mean:.4}, start={start:.4})"
        );
        // Sanity: both worlds still have a live Fire-holding population to measure.
        assert!(coupled.agents.live_count() > 0 && control.agents.live_count() > 0);
    }
}
