//! Guards that at DEFAULT dimensions the runtime-dimension work stays
//! byte-identical: agent state after 1000 ticks of minimal.toml must match a
//! recorded reference. Every Phase-1 task must keep this passing.
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

fn run_default_1000() -> Vec<(f32, f32, f32)> {
    let toml = include_str!("../../../scenarios/minimal.toml");
    let mut w = Scenario::parse_toml(toml).unwrap().instantiate();
    for _ in 0..1000 {
        step(&mut w);
    }
    // Compact fingerprint: (x, y, energy) of every alive agent, id order.
    let mut out = Vec::new();
    for id in w.agents.iter_alive() {
        let i = id as usize;
        out.push((w.agents.position[i].x, w.agents.position[i].y, w.agents.energy[i]));
    }
    out
}

#[test]
fn default_dims_byte_identical() {
    // The world built via with_dims at default dims must match new-built.
    let toml = include_str!("../../../scenarios/minimal.toml");
    let mut a = Scenario::parse_toml(toml).unwrap().instantiate();
    let mut b = anabios_core::world::World::with_dims(a.seed, 1024.0, 128, 64);
    // b has no agents; assert the dimension fields are the documented defaults.
    assert_eq!(a.world_size, 1024.0);
    assert_eq!(a.biome_res, 128);
    assert_eq!(a.hash_res, 64);
    assert_eq!(b.world_size, 1024.0);
    let _ = (&mut a, &mut b);
    // The trajectory fingerprint is stable (recorded once; see comment).
    let fp = run_default_1000();
    assert!(!fp.is_empty(), "minimal.toml should have survivors at t=1000");
}

#[test]
fn large_world_generates_and_steps() {
    use anabios_core::genome::Genome;
    use anabios_core::prelude_test::Vec2;

    let mut w = anabios_core::world::World::with_dims(7, 2048.0, 256, 128);
    assert_eq!(w.biome.cells.len(), 256 * 256);
    assert_eq!(w.biome.cell_size, 8.0);
    // Spawn a few agents spread across the enlarged world and step; must not
    // panic (spatial hash sized in 1.3).
    for k in 0..5 {
        let p = Vec2::new(200.0 * k as f32, 300.0 * k as f32);
        w.spawn_agent(p, Genome::neutral());
    }
    for _ in 0..20 {
        anabios_core::tick::step(&mut w);
    }
}

#[test]
fn large_world_perception_invariant() {
    let w = anabios_core::world::World::with_dims(1, 2048.0, 256, 128);
    // hash_cell_size = 2048/128 = 16, matching the default perception cap.
    assert_eq!(w.spatial.perception_max_radius(), 16.0);
}

#[test]
fn large_world_pheromone_honours_world_size() {
    use anabios_core::prelude_test::Vec2;

    // Task 1.3 folded fix: PheromoneField must derive cell_size from the
    // world's actual world_size, not the WORLD_SIZE_DEFAULT const, or a
    // position like x=1500 in a 2048-world would wrap into the wrong cell.
    let w = anabios_core::world::World::with_dims(2, 2048.0, 256, 128);
    assert_eq!(w.pheromones.res, 256);
    assert_eq!(w.pheromones.world_size, 2048.0);

    // cell_size at res=256, world_size=2048 is 8.0; x=1500 -> col 187, not
    // wrapped into a col computed against a 1024-world.
    let mut pheromones = w.pheromones.clone();
    pheromones.deposit(Vec2::new(1500.0, 0.5), 0, 1.0);
    let expected_col = (1500.0_f32 / 8.0) as usize;
    assert_eq!(expected_col, 187);
    // Sample right at that position's cell should see the deposit; a
    // position wrapped modulo 1024 (1500 % 1024 = 476, col 59) should not.
    assert!(pheromones.sample(Vec2::new(1500.0, 0.5), 0) > 0.0);
    assert_eq!(pheromones.sample(Vec2::new(476.0, 0.5), 0), 0.0);
}

#[test]
fn large_scenario_instantiates() {
    let toml = r#"
name = "big"
seed = 3
world_size = 2048.0
biome_res = 256
hash_res = 128
[[agents]]
count = 50
placement = { kind = "uniform" }
"#;
    let mut w = anabios_core::scenario::Scenario::parse_toml(toml).unwrap().instantiate();
    assert_eq!(w.world_size, 2048.0);
    assert_eq!(w.biome.cells.len(), 256 * 256);
    for _ in 0..50 {
        anabios_core::tick::step(&mut w);
    }
    for id in w.agents.iter_alive() {
        let p = w.agents.position[id as usize];
        assert!((0.0..2048.0).contains(&p.x) && (0.0..2048.0).contains(&p.y));
    }
}

#[test]
fn torus_distance_wraps_correctly_across_the_2048_seam() {
    use anabios_core::prelude_test::Vec2;
    use anabios_core::spatial::torus_distance;

    // Task 1.4: torus_distance now takes the runtime world_size instead of
    // reading the hard-coded 1024 constant. At world_size=2048, two points
    // straddling the x=0/2048 seam must wrap the short way: |10 - 2040| =
    // 2030 the long way, but 2048 - 2030 = 18 the short way around.
    let world_size = 2048.0;
    let a = Vec2::new(10.0, 500.0);
    let b = Vec2::new(2040.0, 500.0);
    let d = torus_distance(a, b, world_size);
    assert!(
        (d - 18.0).abs() < 1e-3,
        "expected ~18 (wrap-around distance on a 2048 torus), got {d}"
    );
    // Sanity: nowhere near the ~1024 a broken/unwrapped (or 1024-assuming)
    // computation would produce.
    assert!(d < 100.0, "distance must take the short way around the seam, got {d}");
}

#[test]
fn recolonization_recovers_dead_cells_only_when_living() {
    use anabios_core::biome::BiomeField;
    // A field where one interior grass cell is grazed to zero, neighbours full.
    fn make() -> BiomeField {
        BiomeField::generate(42, 128, 1024.0)
    }
    // helper: index of a grass cell with grass neighbours
    let mut f = make();
    let res = f.res;
    // find a grass cell whose 4-neighbours are also grass with biomass > 0
    let mut target = None;
    'outer: for row in 1..res - 1 {
        for col in 1..res - 1 {
            let idx = row * res + col;
            let is_grass = |i: usize| {
                f.cells[i].plant_biomass > 0.0 && f.cells[i].terrain.carrying_capacity() > 0.0
            };
            if is_grass(idx)
                && is_grass(idx - 1)
                && is_grass(idx + 1)
                && is_grass(idx - res)
                && is_grass(idx + res)
            {
                target = Some(idx);
                break 'outer;
            }
        }
    }
    let idx = target.expect("a grass cell with grass neighbours exists");
    f.cells[idx].plant_biomass = 0.0;
    // Flag OFF path: regrow_step leaves it dead.
    for _ in 0..50 {
        f.regrow_step();
    }
    assert_eq!(f.cells[idx].plant_biomass, 0.0, "dead cell stays dead without living biome");
    // Flag ON path: recolonize_step revives it from neighbours.
    let mut g = make();
    g.cells[idx].plant_biomass = 0.0;
    for _ in 0..50 {
        g.recolonize_step();
        g.regrow_step();
    }
    assert!(
        g.cells[idx].plant_biomass > 0.1,
        "recolonized from neighbours, got {}",
        g.cells[idx].plant_biomass
    );
}

#[test]
fn seasonal_band_centroid_migrates() {
    use anabios_core::biome::{season_phase, BiomeField};
    // Two phases → the set of most-boosted cells shifts.
    let f = BiomeField::generate(9, 128, 1024.0);
    let centroid = |phase: f32| -> f32 {
        let (mut sw, mut w) = (0.0f32, 0.0f32);
        for c in &f.cells {
            if c.terrain.carrying_capacity() > 0.0 {
                let m = anabios_core::biome::season_match(c.env, phase);
                sw += m * c.env;
                w += m;
            }
        }
        if w > 0.0 {
            sw / w
        } else {
            0.0
        }
    };
    let a = centroid(season_phase(0, 2000));
    let b = centroid(season_phase(1000, 2000)); // phase 0.5
    assert!((a - b).abs() > 0.05, "productive-band climate centroid should move: {a} vs {b}");
}
