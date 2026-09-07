//! World generation: the climate/earth/continental field generators and the
//! biome-derived agent traits that read them. One module per former binary.

mod earth_worldgen {
    use anabios_core::biome::{BiomeField, TerrainType, EARTH_RES};

    /// from_earth builds a full 256x256 field with a plausible land fraction and
    /// real coastlines: central Africa is land, the mid-Pacific is open water.
    #[test]
    fn from_earth_has_real_coastlines() {
        let f = BiomeField::from_earth(EARTH_RES, 4096.0);
        assert_eq!(f.cells.len(), EARTH_RES * EARTH_RES);
        let land = f.cells.iter().filter(|c| c.terrain != TerrainType::Water).count();
        let frac = land as f32 / f.cells.len() as f32;
        assert!((0.15..0.45).contains(&frac), "implausible land fraction {frac}");

        // Equirectangular: x = (lon+180)/360*W, y = (90-lat)/180*W ; cell = pos/cell_size.
        let cell = |lat: f32, lon: f32| {
            let x = (lon + 180.0) / 360.0 * f.world_size;
            let y = (90.0 - lat) / 180.0 * f.world_size;
            let col = (x / f.cell_size) as usize;
            let row = (y / f.cell_size) as usize;
            f.cells[row * f.res + col].terrain
        };
        assert_ne!(cell(0.0, 20.0), TerrainType::Water, "central Africa should be land");
        assert_eq!(cell(0.0, -150.0), TerrainType::Water, "mid-Pacific should be water");
    }

    /// from_earth is a pure function of the embedded assets: identical every call.
    #[test]
    fn from_earth_is_deterministic() {
        let a = BiomeField::from_earth(EARTH_RES, 4096.0);
        let b = BiomeField::from_earth(EARTH_RES, 4096.0);
        assert!(a.cells.iter().zip(&b.cells).all(|(x, y)| x.terrain == y.terrain
            && x.elevation == y.elevation
            && x.env == y.env
            && x.moisture == y.moisture));
    }

    use anabios_core::scenario::Scenario;

    #[test]
    fn scenario_world_map_earth_uses_from_earth() {
        let toml = r#"
    name = "t"
    seed = 1
    world_size = 4096.0
    biome_res = 256
    hash_res = 256
    world_map = "earth"
    [[agents]]
    count = 1
    placement = { kind = "uniform" }
    "#;
        let w = Scenario::parse_toml(toml).expect("parse").instantiate();
        assert_eq!(w.biome.res, 256);
        // Matches from_earth's real map: central Africa is land.
        let x = (20.0 + 180.0) / 360.0 * w.world_size;
        let y = (90.0 - 0.0) / 180.0 * w.world_size;
        let (col, row) = ((x / w.biome.cell_size) as usize, (y / w.biome.cell_size) as usize);
        assert_ne!(
            w.biome.cells[row * w.biome.res + col].terrain,
            anabios_core::biome::TerrainType::Water,
        );
    }

    /// A geo placement lands agents near the mapped lat/lon; radius spread only.
    #[test]
    fn geo_placement_maps_latlon_to_position() {
        let toml = r#"
    name = "t"
    seed = 7
    world_size = 4096.0
    biome_res = 256
    hash_res = 256
    world_map = "earth"
    [[agents]]
    count = 200
    placement = { kind = "geo", lat = 0.0, lon = 20.0, radius = 30.0 }
    "#;
        let w = Scenario::parse_toml(toml).expect("parse").instantiate();
        let cx = (20.0 + 180.0) / 360.0 * w.world_size;
        let cy = (90.0 - 0.0) / 180.0 * w.world_size;
        // Single agent spec → ids 0..200 are the geo-placed founders.
        for id in 0..200usize {
            let p = w.agents.position[id];
            let d = ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt();
            assert!(d <= 30.0 + 1e-3, "agent {id} at distance {d} beyond geo radius");
        }
    }
}

mod continental_worldgen {
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
}

mod biome_adaptation {
    //! With biome_adaptation on, populations evolve a spatial EnvAffinity cline
    //! matched to the local climate — agents in high-climate cells carry higher
    //! affinity than those in low-climate cells (in-place local adaptation).

    use anabios_core::genome::GenomeSlot;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const SCENARIO: &str = include_str!("../../../scenarios/biome-adaptation.toml");

    #[test]
    fn affinity_cline_tracks_local_climate() {
        let mut w = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
        assert!(w.biome_adaptation);
        // Clamp the cap: 2500 ticks at the default 10k cap is minutes-slow; the
        // cline signal this test asserts forms well below 500 agents.
        w.max_population = 500;
        for _ in 0..2500 {
            step(&mut w);
        }
        // Bucket alive agents by their local cell env (low half < 0.5 <= high half).
        let (mut lo_sum, mut lo_n, mut hi_sum, mut hi_n) = (0.0f32, 0u32, 0.0f32, 0u32);
        for id in w.agents.iter_alive() {
            let i = id as usize;
            let env = w.biome.sample(w.agents.position[i]).env;
            let aff = w.agents.genome[i].get(GenomeSlot::EnvAffinity);
            if env < 0.5 {
                lo_sum += aff;
                lo_n += 1;
            } else {
                hi_sum += aff;
                hi_n += 1;
            }
        }
        assert!(lo_n > 0 && hi_n > 0, "need agents in both climate halves ({lo_n}/{hi_n})");
        let lo_mean = lo_sum / lo_n as f32;
        let hi_mean = hi_sum / hi_n as f32;
        assert!(
            hi_mean > lo_mean,
            "high-climate agents should carry higher EnvAffinity than low-climate: hi={hi_mean} lo={lo_mean}"
        );
    }
}

mod nutrient_fertility {
    //! Varied nutrient value + soil fertility: field generation, inertness when
    //! flagged off, and (later tasks) consumption behavior.

    use anabios_core::biome::{
        BiomeField, TerrainType, FERTILITY_MAX, FERTILITY_MIN, NUTRIENT_QUALITY_MAX,
        NUTRIENT_QUALITY_MIN, SUCCESSION_CLIMAX,
    };
    use anabios_core::metrics::{forage_fertility_gain, forage_quality_gain};
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const MINIMAL: &str = include_str!("../../../scenarios/minimal.toml");
    const FORAGING: &str = include_str!("../../../scenarios/foraging-selection.toml");

    #[test]
    fn generated_fields_land_in_range() {
        let b = BiomeField::generate(0, 8, 1024.0);
        for cell in &b.cells {
            assert!(
                (NUTRIENT_QUALITY_MIN..=NUTRIENT_QUALITY_MAX).contains(&cell.nutrient_quality),
                "nutrient_quality {} out of range",
                cell.nutrient_quality
            );
            assert!(
                (FERTILITY_MIN..=FERTILITY_MAX).contains(&cell.fertility),
                "fertility {} out of range",
                cell.fertility
            );
        }
    }

    /// With both flags OFF (default), the nutrient_quality/fertility field VALUES
    /// must not influence simulation dynamics: mutating them to extremes leaves the
    /// biomass trajectory and agent energies bit-identical.
    #[test]
    fn fields_are_inert_when_flags_off() {
        let base = Scenario::parse_toml(MINIMAL).expect("parse");
        let mut a = base.clone().instantiate();
        let mut b = base.instantiate();
        assert!(!b.nutrient_variation && !b.soil_fertility);
        // Perturb every cell's new fields in world B only.
        for cell in b.biome.cells.iter_mut() {
            cell.nutrient_quality = 0.1;
            cell.fertility = 0.1;
        }
        for _ in 0..100 {
            step(&mut a);
            step(&mut b);
        }
        let biomass_a: Vec<f32> = a.biome.cells.iter().map(|c| c.plant_biomass).collect();
        let biomass_b: Vec<f32> = b.biome.cells.iter().map(|c| c.plant_biomass).collect();
        assert_eq!(biomass_a, biomass_b, "field values leaked into biomass dynamics");
        let energy_a: Vec<f32> =
            a.agents.iter_alive().map(|id| a.agents.energy[id as usize]).collect();
        let energy_b: Vec<f32> =
            b.agents.iter_alive().map(|id| b.agents.energy[id as usize]).collect();
        assert_eq!(energy_a, energy_b, "field values leaked into agent energy");
    }

    /// With soil_fertility ON, a high-fertility Grass cell reaches a higher standing
    /// crop than a low-fertility one; Water stays barren regardless.
    #[test]
    fn fertility_scales_capacity_and_regrowth() {
        let mut b = BiomeField::generate(0, 8, 1024.0);
        // Cell 0: fertile grass. Cell 1: poor grass. Cell 2: water (barren).
        for (idx, (terr, fert)) in
            [(TerrainType::Grass, 1.5), (TerrainType::Grass, 0.5), (TerrainType::Water, 1.5)]
                .into_iter()
                .enumerate()
        {
            let c = &mut b.cells[idx];
            c.terrain = terr;
            c.fertility = fert;
            c.plant_biomass = if terr == TerrainType::Water { 0.0 } else { 1.0 };
            c.succession = SUCCESSION_CLIMAX;
            c.pollution = 0.0;
        }
        for _ in 0..2000 {
            b.regrow_step(true);
        }
        assert!(
            b.cells[0].plant_biomass > b.cells[1].plant_biomass,
            "fertile {} should exceed poor {}",
            b.cells[0].plant_biomass,
            b.cells[1].plant_biomass
        );
        // Fertile grass should exceed the flat carrying capacity (10.0) it would cap
        // at with fertility ignored.
        assert!(b.cells[0].plant_biomass > 10.0);
        assert_eq!(b.cells[2].plant_biomass, 0.0, "water stays barren");
    }

    /// With nutrient_variation ON, uniformly high-quality cells yield more total
    /// forage energy than uniformly low-quality cells over the same run.
    #[test]
    fn nutrient_quality_scales_forage_energy() {
        let make = |q: f32| {
            let mut w = Scenario::parse_toml(MINIMAL).expect("parse").instantiate();
            w.nutrient_variation = true;
            for cell in w.biome.cells.iter_mut() {
                cell.nutrient_quality = q;
            }
            w
        };
        let mut hi = make(1.4);
        let mut lo = make(0.6);
        for _ in 0..20 {
            step(&mut hi);
            step(&mut lo);
        }
        let sum = |w: &anabios_core::world::World| -> f32 {
            w.agents.iter_alive().map(|id| w.agents.energy[id as usize]).sum()
        };
        assert!(
            sum(&hi) > sum(&lo),
            "high-quality total energy {} should exceed low {}",
            sum(&hi),
            sum(&lo)
        );
    }

    /// End-to-end: the foraging-selection scenario (both flags on) runs without
    /// collapsing and the forage-gain observables are computable. The scientific
    /// result (does the gain rise over generations?) is read from a long
    /// `emergence.sh soak` run, not asserted here.
    #[test]
    fn foraging_scenario_runs_and_metrics_are_finite() {
        let mut w = Scenario::parse_toml(FORAGING).expect("parse").instantiate();
        assert!(w.nutrient_variation && w.soil_fertility, "flags must be on");
        w.max_population = 500; // keep the run fast
        for _ in 0..300 {
            step(&mut w);
        }
        assert!(w.agents.iter_alive().count() > 0, "population collapsed");
        let q = forage_quality_gain(&w);
        let f = forage_fertility_gain(&w);
        assert!(q.is_finite() && f.is_finite(), "metrics must be finite: q={q} f={f}");
    }
}

mod scavenge_index {
    //! Regression tests for the spatial-hash scavenge pass
    //! (`interact::scavenge_pass` + `World::carcass_spatial`).
    //!
    //! The indexed implementation must reproduce the exact selection semantics of
    //! the original ascending-index linear scan: nearest non-depleted carcass
    //! within `SCAVENGE_RANGE`, lowest-index tie-break, wrap-aware torus
    //! distances.

    use anabios_core::carcass::Carcass;
    use anabios_core::genome::Genome;
    use anabios_core::module::{Module, ModuleList};
    use anabios_core::prelude_test::{reassign_to_new_species, Vec2};
    use anabios_core::tick::step;
    use anabios_core::world::World;

    /// A stationary pure-carnivore kit: Mouth only (no Locomotor → cannot drift,
    /// no Sensor → no perception, no Weapon → cannot kill the other test agent).
    fn carnivore_mouth_only() -> ModuleList {
        let mut m = ModuleList::new();
        m.push(Module::Mouth { bite_size: 0.6, diet_affinity: 1.0 });
        m
    }

    fn spawn_carnivore(w: &mut World, pos: Vec2) -> u32 {
        let id = w.spawn_agent(pos, Genome::neutral());
        w.agents.modules[id as usize] = carnivore_mouth_only();
        id
    }

    /// When the first scavenger (ascending id order) depletes the shared nearest
    /// carcass mid-pass, the second scavenger must fall through to its *next*
    /// nearest carcass — the linear scan re-checked `flesh <= 0.0` per agent, so
    /// the indexed pass must too (regression: the prefilter once used the
    /// stale rebuild-time flesh snapshot here).
    #[test]
    fn depleted_carcass_falls_through_to_next_nearest() {
        let mut w = World::new(11);
        let a = spawn_carnivore(&mut w, Vec2::new(400.0, 400.0));
        let b = spawn_carnivore(&mut w, Vec2::new(401.0, 400.0));
        reassign_to_new_species(&mut w, b);
        // C0: nearest to both; too small to survive A's bite.
        w.carcasses.push(Carcass {
            pos: Vec2::new(400.4, 400.0),
            flesh: 0.1,
            age: 0,
            species_id: 0,
        });
        // C1: in range of B only; B's fall-through target once C0 is gone.
        w.carcasses.push(Carcass {
            pos: Vec2::new(401.8, 400.0),
            flesh: 5.0,
            age: 0,
            species_id: 0,
        });
        assert!(a < b, "A must scavenge first (ascending id)");

        step(&mut w);

        // C0 is fully depleted (and removed by carcass_step's retain).
        assert!(
            !w.carcasses.iter().any(|c| (c.pos.x - 400.4).abs() < 1e-3),
            "C0 depleted by A and removed"
        );
        // B fell through to C1: its flesh decreased even though C0 was nearer.
        let c1 =
            w.carcasses.iter().find(|c| (c.pos.x - 401.8).abs() < 1e-3).expect("C1 still present");
        assert!(c1.flesh < 5.0, "B fell through depleted C0 to C1 (flesh {})", c1.flesh);
    }

    /// Equal torus distances: the lower carcass index wins (the old scan's strict
    /// `<` over ascending indices).
    #[test]
    fn equidistant_tie_breaks_to_lower_carcass_index() {
        let mut w = World::new(12);
        spawn_carnivore(&mut w, Vec2::new(400.0, 400.0));
        w.carcasses.push(Carcass {
            pos: Vec2::new(399.0, 400.0), // d = 1.0, index 0
            flesh: 5.0,
            age: 0,
            species_id: 0,
        });
        w.carcasses.push(Carcass {
            pos: Vec2::new(401.0, 400.0), // d = 1.0, index 1
            flesh: 5.0,
            age: 0,
            species_id: 0,
        });

        step(&mut w);

        assert!(w.carcasses[0].flesh < 5.0, "lower-index carcass scavenged");
        assert_eq!(w.carcasses[1].flesh, 5.0, "equidistant higher-index carcass untouched");
    }

    /// The carcass hash query must wrap around the torus the same way the linear
    /// scan's `torus_distance` did.
    #[test]
    fn scavenge_wraps_around_torus() {
        let mut w = World::new(13);
        spawn_carnivore(&mut w, Vec2::new(1.0, 400.0));
        w.carcasses.push(Carcass {
            pos: Vec2::new(anabios_core::biome::WORLD_SIZE - 0.5, 400.0), // wrap d = 1.5
            flesh: 5.0,
            age: 0,
            species_id: 0,
        });

        step(&mut w);

        assert!(w.carcasses[0].flesh < 5.0, "carcass across the wrap was scavenged");
    }

    /// A carcass exactly at `SCAVENGE_RANGE` is out of reach (strict `<`), and a
    /// depleted carcass left over from a prior tick is skipped.
    #[test]
    fn range_boundary_and_predepleted_carcass_are_skipped() {
        use anabios_core::carcass::SCAVENGE_RANGE;
        let mut w = World::new(14);
        spawn_carnivore(&mut w, Vec2::new(400.0, 400.0));
        w.carcasses.push(Carcass {
            pos: Vec2::new(400.0 + SCAVENGE_RANGE, 400.0), // exactly at the boundary
            flesh: 5.0,
            age: 0,
            species_id: 0,
        });
        w.carcasses.push(Carcass {
            pos: Vec2::new(400.5, 400.0), // in range but already depleted
            flesh: 0.0,
            age: 0,
            species_id: 0,
        });

        let e0 = w.agents.energy[0];
        step(&mut w);

        // carcass_step drops the flesh-0 entry; the boundary carcass must be whole.
        let boundary = w
            .carcasses
            .iter()
            .find(|c| (c.pos.x - (400.0 + SCAVENGE_RANGE)).abs() < 1e-3)
            .expect("boundary carcass present");
        assert_eq!(boundary.flesh, 5.0, "carcass exactly at SCAVENGE_RANGE untouched");
        assert!(w.agents.energy[0] < e0, "no flesh energy gained (only metabolism paid)");
    }
}
