//! M14 mechanism tests: meme transmission, sensing, inheritance, and detectors.

use anabios_core::genome::Genome;
use anabios_core::module::{Module, ModuleType};
use anabios_core::prelude_test::Vec2;
use anabios_core::program::MEME_CHANNELS;
use anabios_core::world::World;

/// A kit with a Communicator (so meme ops are enabled) + basics.
fn communicator_kit() -> anabios_core::module::ModuleList {
    let mut m = anabios_core::module::ModuleList::new();
    m.push(Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 });
    m.push(Module::Sensor { sensor_type: anabios_core::module::SensorType::Vision, radius: 0.6, acuity: 0.6 });
    m.push(Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 });
    m.push(Module::Communicator { range: 10.0, channel_id: 0 });
    m
}

#[test]
fn new_agent_has_zeroed_meme_vector() {
    let mut w = World::new(1);
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    assert_eq!(w.agents.meme_vector[id as usize], [0.0; MEME_CHANNELS]);
}

#[test]
fn effective_communicator_range_reports_max() {
    let kit = communicator_kit();
    assert_eq!(anabios_core::module::effective_communicator_range(&kit), 10.0);
    // A kit without a Communicator reports 0.
    let mut bare = anabios_core::module::ModuleList::new();
    bare.push(Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 });
    assert_eq!(anabios_core::module::effective_communicator_range(&bare), 0.0);
    // Silence unused warning until later tasks use it.
    let _ = ModuleType::Communicator;
}

use anabios_core::program::{Node, Program};
use anabios_core::tick::step;

#[test]
fn sense_meme_reads_the_agents_own_meme_vector() {
    let mut w = World::new(2);
    let id = w.spawn_agent(Vec2::new(700.0, 700.0), Genome::neutral());
    // Plant a meme value on channel 2, then program move_x = SenseMeme(2).
    w.agents.meme_vector[id as usize][2] = 1.0;
    w.agents.program[id as usize] =
        Program::from_slice(&[Node::SenseMeme(2), Node::MoveTowardX]);
    step(&mut w);
    // Positive meme read → move_x > 0 → normalized to +1 on x.
    assert!(w.desired_direction[id as usize].x > 0.9, "SenseMeme reads the meme vector");
}

#[test]
fn culture_step_transmits_broadcast_toward_receiver_meme() {
    let mut w = World::new(3);
    let sender = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let receiver = w.spawn_agent(Vec2::new(503.0, 500.0), Genome::neutral()); // within range
    w.agents.modules[sender as usize] = communicator_kit();
    w.agents.modules[receiver as usize] = communicator_kit();
    // Sender broadcasts a high value on channel 1 every tick; receiver just reads.
    w.agents.program[sender as usize] =
        Program::from_slice(&[Node::Const(4.0), Node::Broadcast(1)]);
    w.agents.program[receiver as usize] = Program::from_slice(&[Node::Idle]);
    let before = w.agents.meme_vector[receiver as usize][1];
    step(&mut w);
    let after = w.agents.meme_vector[receiver as usize][1];
    assert!(after > before, "receiver's meme[1] moved toward the sender's broadcast");
}

#[test]
fn no_communicator_means_no_transmission() {
    let mut w = World::new(3);
    let sender = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let receiver = w.spawn_agent(Vec2::new(503.0, 500.0), Genome::neutral());
    w.agents.modules[sender as usize] = communicator_kit();
    // Receiver has the DEFAULT kit — no Communicator.
    w.agents.program[sender as usize] =
        Program::from_slice(&[Node::Const(4.0), Node::Broadcast(1)]);
    step(&mut w);
    assert_eq!(w.agents.meme_vector[receiver as usize][1], 0.0, "no Communicator → no receive (gating)");
}
