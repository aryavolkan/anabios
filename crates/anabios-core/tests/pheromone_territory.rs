//! M13 mechanism tests: pheromone deposition, decay, sensing, and detectors.

use anabios_core::pheromone::{PheromoneField, PHEROMONE_DECAY};
use anabios_core::prelude_test::Vec2;
use anabios_core::world::World;

#[test]
fn deposit_then_sample_reads_back_on_the_right_channel() {
    let mut f = PheromoneField::new();
    let p = Vec2::new(400.0, 400.0);
    f.deposit(p, 3, 2.0);
    assert!((f.sample(p, 3) - 2.0).abs() < 1e-6, "channel 3 holds the deposit");
    assert_eq!(f.sample(p, 0), 0.0, "other channels untouched");
    // A far-away cell is unaffected.
    assert_eq!(f.sample(Vec2::new(10.0, 10.0), 3), 0.0);
}

#[test]
fn decay_step_multiplies_every_cell_by_one_minus_decay() {
    let mut f = PheromoneField::new();
    let p = Vec2::new(200.0, 200.0);
    f.deposit(p, 1, 10.0);
    f.decay_step();
    let expected = 10.0 * (1.0 - PHEROMONE_DECAY);
    assert!((f.sample(p, 1) - expected).abs() < 1e-4, "one decay step");
}

#[test]
fn world_starts_with_an_empty_pheromone_field() {
    let w = World::new(1);
    assert_eq!(w.pheromones.sample(Vec2::new(500.0, 500.0), 0), 0.0);
}
