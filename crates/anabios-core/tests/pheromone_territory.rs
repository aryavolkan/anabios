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

use anabios_core::genome::Genome;
use anabios_core::module::{Module, PheromoneChannel, SensorType};
use anabios_core::program::{Node, Program};
use anabios_core::tick::step;

/// Build a pheromone-marking kit: Locomotor + Vision + Mouth + Pheromone(Marker).
fn marker_kit() -> anabios_core::module::ModuleList {
    let mut m = anabios_core::module::ModuleList::new();
    m.push(Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 });
    m.push(Module::Sensor { sensor_type: SensorType::Vision, radius: 0.6, acuity: 0.6 });
    m.push(Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 });
    m.push(Module::Pheromone { channel: PheromoneChannel::Marker, strength: 1.0, decay: 0.1 });
    m
}

#[test]
fn agent_with_pheromone_module_deposits_on_emit() {
    let mut w = World::new(2);
    let id = w.spawn_agent(Vec2::new(600.0, 600.0), Genome::neutral());
    w.agents.modules[id as usize] = marker_kit();
    // Emit strongly on channel 3 (Marker).
    w.agents.program[id as usize] =
        Program::from_slice(&[Node::Const(5.0), Node::EmitPheromone(3)]);
    let pos = w.agents.position[id as usize];
    step(&mut w);
    assert!(w.pheromones.sample(pos, 3) > 0.0, "marker deposited at the agent's cell");
}

#[test]
fn agent_without_pheromone_module_deposits_nothing() {
    let mut w = World::new(2);
    let id = w.spawn_agent(Vec2::new(600.0, 600.0), Genome::neutral());
    // Default starter_kit has NO Pheromone module.
    w.agents.program[id as usize] =
        Program::from_slice(&[Node::Const(5.0), Node::EmitPheromone(3)]);
    let pos = w.agents.position[id as usize];
    step(&mut w);
    assert_eq!(w.pheromones.sample(pos, 3), 0.0, "no Pheromone module → no deposit (gating)");
}
