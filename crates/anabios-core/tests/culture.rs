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
    m.push(Module::Sensor {
        sensor_type: anabios_core::module::SensorType::Vision,
        radius: 0.6,
        acuity: 0.6,
    });
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
    w.agents.program[id as usize] = Program::from_slice(&[Node::SenseMeme(2), Node::MoveTowardX]);
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
    assert_eq!(
        w.agents.meme_vector[receiver as usize][1], 0.0,
        "no Communicator → no receive (gating)"
    );
}

#[test]
fn child_inherits_parent_meme_average_with_jitter() {
    use anabios_core::rng::Rng;
    let a = [1.0f32; MEME_CHANNELS];
    let b = [3.0f32; MEME_CHANNELS];
    let mut rng = Rng::from_seed(42);
    let child = anabios_core::culture::inherit_meme(&a, &b, &mut rng);
    // Average is 2.0; jitter is small (MEME_INHERIT_JITTER = 0.05), so each channel is near 2.0.
    for &v in &child {
        assert!((v - 2.0).abs() < 0.5, "child meme near parent average ({v})");
    }
}

#[test]
fn meme_l2_is_zero_for_equal_positive_for_divergent() {
    use anabios_core::codex::meme_l2;
    let a = [0.0f32; MEME_CHANNELS];
    let b = [0.0f32; MEME_CHANNELS];
    assert_eq!(meme_l2(&a, &b), 0.0);
    let mut c = [0.0f32; MEME_CHANNELS];
    c[0] = 1.0;
    assert!(meme_l2(&a, &c) > 0.5);
}

#[test]
fn dialect_formed_fires_for_two_divergent_halves() {
    use anabios_core::codex::{observe_all, EventType, DIALECT_WINDOW};
    let mut w = World::new(9);
    // West half at x=300 with meme[0]=0; east half at x=700 with meme[0]=1.
    let mut ids = Vec::new();
    for k in 0..4 {
        let id = w.spawn_agent(Vec2::new(300.0, 500.0 + k as f32), Genome::neutral());
        w.agents.modules[id as usize] = communicator_kit();
        ids.push(id);
    }
    for k in 0..4 {
        let id = w.spawn_agent(Vec2::new(700.0, 500.0 + k as f32), Genome::neutral());
        w.agents.modules[id as usize] = communicator_kit();
        w.agents.meme_vector[id as usize][0] = 1.0;
        ids.push(id);
    }
    // Put all 8 in one fresh species.
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
    // Drive observe_all for a full window WITHOUT stepping (memes/positions fixed).
    let mut fired = false;
    for _ in 0..(DIALECT_WINDOW + 2) {
        observe_all(&mut w);
        w.tick += 1;
        if w.codex.events.iter().any(|e| e.event_type == EventType::DialectFormed) {
            fired = true;
            break;
        }
    }
    assert!(fired, "two divergent meme halves form a dialect");
}

#[test]
fn alarm_call_fires_on_broadcast_plus_nearby_flee() {
    use anabios_core::codex::{observe_all, EventType, ALARM_MIN_RESPONSES};
    use anabios_core::program::ActionRegister;
    use anabios_core::sense::SensorRegister;
    let mut w = World::new(11);
    let caller = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let responder = w.spawn_agent(Vec2::new(503.0, 500.0), Genome::neutral());
    w.agents.modules[caller as usize] = communicator_kit();
    w.agents.modules[responder as usize] = communicator_kit();
    // Manually resize scratch buffers (resize_scratch is pub(crate); we size
    // directly since sensors / desired_direction / actions are pub fields).
    let cap = w.agents.capacity();
    w.sensors.resize(cap, SensorRegister::default());
    w.desired_direction.resize(cap, Vec2::ZERO);
    w.actions.resize(cap, ActionRegister::default());
    let mut fired = false;
    for _ in 0..(ALARM_MIN_RESPONSES + 5) {
        // Rebuild the spatial hash so the query finds the responder.
        w.spatial.rebuild(&w.agents.position, |k| w.agents.is_alive(k as u32));
        w.actions[caller as usize].broadcast_intent[0] = 1.0;
        // Responder senses a threat to its +x and flees to -x.
        w.sensors[responder as usize].nearest_other_dist = 4.0;
        w.sensors[responder as usize].nearest_other_dir = Vec2::new(1.0, 0.0);
        w.desired_direction[responder as usize] = Vec2::new(-1.0, 0.0);
        observe_all(&mut w);
        w.tick += 1;
        if w.codex.events.iter().any(|e| e.event_type == EventType::AlarmCall) {
            fired = true;
            break;
        }
    }
    assert!(fired, "alarm broadcast + nearby flee triggers AlarmCall");
}
