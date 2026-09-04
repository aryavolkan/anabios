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
    let child = anabios_core::culture::inherit_meme(&a, &b, &mut rng, true, true, 1.0);
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

// --- O2 payoff-biased social learning (opt-in flag) -------------------------

/// A receiver + Communicator neighbours with hand-set energies and meme
/// levels, ready for a direct `culture_step`. Returns (world, receiver id).
fn payoff_world(seed: u64, flag: bool) -> (World, u32) {
    let mut w = World::new(seed);
    w.cognition_enabled = true;
    w.payoff_biased_learning = flag;
    let receiver = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
    w.agents.modules[receiver as usize] = communicator_kit();
    w.agents.iq[receiver as usize] = 0.5;
    w.agents.energy[receiver as usize] = 50.0;
    let cap = w.agents.capacity();
    w.actions.resize(cap, Default::default());
    (w, receiver)
}

fn add_neighbour(w: &mut World, x: f32, y: f32, energy: f32, practice: f32, skill: f32) -> u32 {
    let id = w.spawn_agent(Vec2::new(x, y), Genome::neutral());
    w.agents.modules[id as usize] = communicator_kit();
    w.agents.energy[id as usize] = energy;
    w.agents.meme_vector[id as usize][anabios_core::practice::channel(0)] = practice;
    w.agents.meme_vector[id as usize][anabios_core::culture::SKILL_CHANNEL] = skill;
    id
}

fn run_culture_step(w: &mut World) {
    let cap = w.agents.capacity();
    w.actions.resize(cap, Default::default());
    w.spatial.rebuild(&w.agents.position, |k| w.agents.is_alive(k as u32));
    anabios_core::culture::culture_step(w);
}

/// Content bias declines a locally-harmful trait even when the FITTEST model
/// carries it: the max-energy neighbour holds the practice (so model bias
/// alone would transmit), but holders' mean energy is below non-holders'.
#[test]
fn payoff_biased_content_bias_declines_maladaptive_practice() {
    let pch = anabios_core::practice::channel(0);
    let setup = |w: &mut World| {
        add_neighbour(w, 503.0, 500.0, 100.0, 1.0, 0.0); // fittest, holds practice
        add_neighbour(w, 497.0, 500.0, 20.0, 1.0, 0.0); // low-energy holder
        add_neighbour(w, 500.0, 503.0, 90.0, 0.0, 0.0); // non-holder
        add_neighbour(w, 500.0, 497.0, 90.0, 0.0, 0.0); // non-holder
                                                        // holder mean 60 < non-holder mean 90 → locally maladaptive
    };
    let (mut won, r) = payoff_world(21, true);
    setup(&mut won);
    run_culture_step(&mut won);
    assert_eq!(
        won.agents.meme_vector[r as usize][pch], 0.0,
        "content bias must decline the locally-harmful practice"
    );

    let (mut woff, r) = payoff_world(21, false);
    setup(&mut woff);
    run_culture_step(&mut woff);
    assert!(
        woff.agents.meme_vector[r as usize][pch] > 0.0,
        "flag off: payoff-blind transmission copies the practice (control)"
    );
}

/// Negative control: when holders are the FITTER group locally, payoff-biased
/// transmission still copies the trait (the bias declines only on evidence of
/// harm, never categorically).
#[test]
fn payoff_biased_keeps_trait_whose_holders_are_fitter() {
    let pch = anabios_core::practice::channel(0);
    let (mut w, r) = payoff_world(22, true);
    add_neighbour(&mut w, 503.0, 500.0, 100.0, 1.0, 0.0);
    add_neighbour(&mut w, 497.0, 500.0, 95.0, 1.0, 0.0);
    add_neighbour(&mut w, 500.0, 503.0, 90.0, 0.0, 0.0);
    add_neighbour(&mut w, 500.0, 497.0, 90.0, 0.0, 0.0);
    // holder mean 97.5 > non-holder mean 90 → not maladaptive here
    run_culture_step(&mut w);
    assert!(
        w.agents.meme_vector[r as usize][pch] > 0.0,
        "trait with fitter holders must still transmit under the flag"
    );
}

/// Model bias: the copy source is the highest-ENERGY neighbour, not the
/// highest-trait-level one.
#[test]
fn payoff_biased_model_bias_copies_from_highest_energy_model() {
    let setup = |w: &mut World| {
        add_neighbour(w, 503.0, 500.0, 10.0, 0.0, 1.0); // skill expert, low energy
        add_neighbour(w, 497.0, 500.0, 100.0, 0.0, 0.5); // fittest, half the skill
    };
    let (mut won, r) = payoff_world(23, true);
    setup(&mut won);
    run_culture_step(&mut won);
    let on_skill = won.agents.meme_vector[r as usize][anabios_core::culture::SKILL_CHANNEL];

    let (mut woff, r) = payoff_world(23, false);
    setup(&mut woff);
    run_culture_step(&mut woff);
    let off_skill = woff.agents.meme_vector[r as usize][anabios_core::culture::SKILL_CHANNEL];

    assert!(on_skill > 0.0, "still learns from the fittest model");
    assert!(
        on_skill < off_skill,
        "model bias targets the 0.5-skill fittest neighbour, not the 1.0-skill expert: \
         on={on_skill} off={off_skill}"
    );
}

// --- O3 repro-biased social learning (opt-in flag) --------------------------

/// Like `payoff_world`, but arming the reproductive-success bias instead of
/// the energy-proxy one.
fn repro_world(seed: u64, flag: bool) -> (World, u32) {
    let mut w = World::new(seed);
    w.cognition_enabled = true;
    w.repro_biased_learning = flag;
    let receiver = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
    w.agents.modules[receiver as usize] = communicator_kit();
    w.agents.iq[receiver as usize] = 0.5;
    w.agents.energy[receiver as usize] = 50.0;
    let cap = w.agents.capacity();
    w.actions.resize(cap, Default::default());
    (w, receiver)
}

/// Set a neighbour's observed birth outcomes.
fn set_births(w: &mut World, id: u32, ok: u16, failed: u16) {
    w.agents.births_ok[id as usize] = ok;
    w.agents.births_failed[id as usize] = failed;
}

/// Content bias declines a practice whose holders demonstrably bury more
/// infants — even though the holders are the higher-ENERGY group (so the O2b
/// energy proxy would have kept it; this is exactly the signal energy cannot
/// see).
#[test]
fn repro_biased_declines_practice_whose_holders_bury_more_infants() {
    let pch = anabios_core::practice::channel(0);
    let setup = |w: &mut World| {
        let h1 = add_neighbour(w, 503.0, 500.0, 100.0, 1.0, 0.0); // rich holder
        let h2 = add_neighbour(w, 497.0, 500.0, 95.0, 1.0, 0.0); // rich holder
        let n1 = add_neighbour(w, 500.0, 503.0, 20.0, 0.0, 0.0); // poor non-holder
        let n2 = add_neighbour(w, 500.0, 497.0, 20.0, 0.0, 0.0); // poor non-holder
        set_births(w, h1, 2, 3); // holders: 6/10 births lost
        set_births(w, h2, 2, 3);
        set_births(w, n1, 5, 0); // non-holders: 0/10 lost
        set_births(w, n2, 5, 0);
    };
    let (mut won, r) = repro_world(31, true);
    setup(&mut won);
    run_culture_step(&mut won);
    assert_eq!(
        won.agents.meme_vector[r as usize][pch], 0.0,
        "repro content bias must decline the practice whose holders bury more infants"
    );

    let (mut woff, r) = repro_world(31, false);
    setup(&mut woff);
    run_culture_step(&mut woff);
    assert!(
        woff.agents.meme_vector[r as usize][pch] > 0.0,
        "flag off: payoff-blind transmission copies the practice (control)"
    );
}

/// No birth evidence (no group has observed outcomes) → no decline. The bias
/// acts only on evidence of harm, never categorically.
#[test]
fn repro_biased_keeps_practice_without_birth_evidence() {
    let pch = anabios_core::practice::channel(0);
    let (mut w, r) = repro_world(32, true);
    add_neighbour(&mut w, 503.0, 500.0, 100.0, 1.0, 0.0);
    add_neighbour(&mut w, 497.0, 500.0, 95.0, 1.0, 0.0);
    add_neighbour(&mut w, 500.0, 503.0, 90.0, 0.0, 0.0);
    add_neighbour(&mut w, 500.0, 497.0, 90.0, 0.0, 0.0);
    // all counters zero — no observed births anywhere
    run_culture_step(&mut w);
    assert!(
        w.agents.meme_vector[r as usize][pch] > 0.0,
        "no birth evidence → the practice still transmits"
    );
}

/// Holders whose birth outcomes are no worse than non-holders' keep
/// transmitting the trait.
#[test]
fn repro_biased_keeps_practice_when_holders_do_no_worse() {
    let pch = anabios_core::practice::channel(0);
    let (mut w, r) = repro_world(33, true);
    let h1 = add_neighbour(&mut w, 503.0, 500.0, 100.0, 1.0, 0.0);
    let h2 = add_neighbour(&mut w, 497.0, 500.0, 95.0, 1.0, 0.0);
    let n1 = add_neighbour(&mut w, 500.0, 503.0, 90.0, 0.0, 0.0);
    let n2 = add_neighbour(&mut w, 500.0, 497.0, 90.0, 0.0, 0.0);
    set_births(&mut w, h1, 4, 1); // holders: 2/10 lost
    set_births(&mut w, h2, 4, 1);
    set_births(&mut w, n1, 4, 1); // non-holders: 2/10 lost — equal
    set_births(&mut w, n2, 4, 1);
    run_culture_step(&mut w);
    assert!(w.agents.meme_vector[r as usize][pch] > 0.0, "equal birth outcomes → no decline");
}

/// The repro bias is content-bias-only, scoped to practice channels: skill
/// transmission is untouched (no model bias — the O2b regression guard).
#[test]
fn repro_biased_leaves_skill_transmission_untouched() {
    let setup = |w: &mut World| {
        let h = add_neighbour(w, 503.0, 500.0, 10.0, 1.0, 1.0); // skill expert, holds practice
        add_neighbour(w, 497.0, 500.0, 100.0, 0.0, 0.2);
        set_births(w, h, 0, 5); // holder's births all fail
    };
    let (mut won, r) = repro_world(34, true);
    setup(&mut won);
    run_culture_step(&mut won);
    let on_skill = won.agents.meme_vector[r as usize][anabios_core::culture::SKILL_CHANNEL];

    let (mut woff, r) = repro_world(34, false);
    setup(&mut woff);
    run_culture_step(&mut woff);
    let off_skill = woff.agents.meme_vector[r as usize][anabios_core::culture::SKILL_CHANNEL];

    assert_eq!(
        on_skill.to_bits(),
        off_skill.to_bits(),
        "skill copies identically with the flag on — content bias never touches the skill channel"
    );
    assert!(on_skill > 0.0, "skill actually transmitted in the setup");
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
    let sid = anabios_core::prelude_test::fresh_species(&mut w);
    for &id in &ids {
        anabios_core::prelude_test::reassign_to_species(&mut w, id, sid);
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
