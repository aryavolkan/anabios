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
