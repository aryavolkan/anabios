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

#[test]
fn smell_sensored_agent_reads_local_pheromone_sensorless_reads_zero() {
    // Sensor agent: has a Smell sensor; a plant marker is pre-seeded at its cell.
    let mut w = World::new(2);
    let smeller = w.spawn_agent(Vec2::new(700.0, 700.0), Genome::neutral());
    let mut kit = marker_kit();
    // marker_kit's Sensor is Vision; swap to Smell so sensing is gated ON.
    for m in kit.iter_mut() {
        if let Module::Sensor { sensor_type, .. } = m {
            *sensor_type = SensorType::Smell;
        }
    }
    w.agents.modules[smeller as usize] = kit;
    // Program: move_x = SensePheromone(2). Plant a Trail (channel 2) at its cell.
    let pos = w.agents.position[smeller as usize];
    w.pheromones.deposit(pos, 2, 3.0);
    w.agents.program[smeller as usize] =
        Program::from_slice(&[Node::SensePheromone(2), Node::MoveTowardX]);
    step(&mut w);
    // A positive pheromone read drives move_x > 0 → normalized to +1 on x.
    assert!(w.desired_direction[smeller as usize].x > 0.9, "Smell agent reads the pheromone");

    // Sensorless agent (no Smell) reads zero → no movement from the same program.
    let mut w2 = World::new(2);
    let blind = w2.spawn_agent(Vec2::new(700.0, 700.0), Genome::neutral());
    // Default starter_kit Sensor is Vision (not Smell).
    let pos2 = w2.agents.position[blind as usize];
    w2.pheromones.deposit(pos2, 2, 3.0);
    w2.agents.program[blind as usize] =
        Program::from_slice(&[Node::SensePheromone(2), Node::MoveTowardX]);
    step(&mut w2);
    assert_eq!(w2.desired_direction[blind as usize].x, 0.0, "no Smell → reads zero (gating)");
}

use anabios_core::codex::{species_spread, EventType, TERRITORY_SPREAD_MAX};

#[test]
fn species_spread_is_small_for_a_tight_cluster_large_for_a_dispersed_one() {
    let tight = [Vec2::new(500.0, 500.0), Vec2::new(505.0, 500.0), Vec2::new(500.0, 505.0)];
    let dispersed = [Vec2::new(100.0, 100.0), Vec2::new(900.0, 100.0), Vec2::new(500.0, 900.0)];
    let world_size = anabios_core::biome::WORLD_SIZE_DEFAULT;
    assert!(species_spread(&tight, world_size) < TERRITORY_SPREAD_MAX);
    assert!(species_spread(&dispersed, world_size) > TERRITORY_SPREAD_MAX);
}

#[test]
fn territory_formation_fires_for_a_clustered_marking_species() {
    use anabios_core::codex::observe_all;
    let mut w = World::new(5);
    // Spawn a tight cluster of pheromone-markers as their own species.
    let mut ids = Vec::new();
    for k in 0..6 {
        let id = w.spawn_agent(Vec2::new(500.0 + k as f32, 500.0), Genome::neutral());
        w.agents.modules[id as usize] = marker_kit(); // has a Pheromone module
        ids.push(id);
    }
    // Move them all into one fresh species so they are measured together.
    let sid = w.species_centroids.len() as u32;
    w.species_centroids.push(Genome::neutral());
    w.species_parents.push(Some(0));
    w.species_member_counts.push(0);
    w.next_species_id = sid + 1;
    for &id in &ids {
        w.remove_from_species(w.agents.species_id[id as usize]);
        w.agents.species_id[id as usize] = sid;
        w.add_to_species(sid);
    }
    // Run observe_all for a full window without moving them (tight cluster persists).
    let mut fired = false;
    for _ in 0..(anabios_core::codex::TERRITORY_WINDOW + 2) {
        observe_all(&mut w);
        w.tick += 1;
        if w.codex.events.iter().any(|e| e.event_type == EventType::TerritoryFormation) {
            fired = true;
            break;
        }
    }
    assert!(fired, "a tight, persistent marking species forms a territory");
}

use anabios_core::codex::{histogram_overlap, TERRAIN_SLOTS};

#[test]
fn histogram_overlap_is_one_for_identical_zero_for_disjoint() {
    let mut a = [0.0f32; TERRAIN_SLOTS];
    a[0] = 0.5;
    a[1] = 0.5;
    let identical = a;
    assert!((histogram_overlap(&a, &identical) - 1.0).abs() < 1e-6);

    let mut b = [0.0f32; TERRAIN_SLOTS];
    b[2] = 1.0; // disjoint terrain type
    assert_eq!(histogram_overlap(&a, &b), 0.0);
}
