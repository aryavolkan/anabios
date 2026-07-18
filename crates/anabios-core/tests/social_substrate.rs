//! M11 mechanism tests: kin/threat sensing and action-intent plumbing end to
//! end through the public API.

use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude_test::Vec2;
use anabios_core::program::{Node, Program, NO_TARGET};
use anabios_core::tick::step;
use anabios_core::world::World;

// Move an agent into a fresh second species, keeping species bookkeeping
// tables consistent (`spawn_agent` always assigns species 0).
use anabios_core::prelude_test::reassign_to_new_species;

/// A predator program that fires whenever an other-species agent is in range,
/// driven through the full sense -> decide pipeline.
#[test]
fn predator_program_produces_fire_intent_and_target() {
    let mut w = World::new(5);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let prey = w.spawn_agent(Vec2::new(503.0, 500.0), Genome::neutral());
    let prey_species = reassign_to_new_species(&mut w, prey);
    assert_eq!(prey_species, 1); // sanity: helper returns the new species id
                                 // Fire when other_dist < 6. The postfix idiom `SenseOtherDist, Neg,
                                 // ThresholdGt(-6.0)` computes `(-other_dist) > -6`, i.e. `other_dist < 6`.
    w.agents.program[pred as usize] = Program::from_slice(&[
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-6.0),
        Node::FireWeapon,
    ]);
    step(&mut w);
    assert!(w.actions[pred as usize].fire_intent > 0.0);
    assert_eq!(w.actions[pred as usize].target_id, prey);
}

/// With no neighbor in range, the target resolves to NO_TARGET and intents are
/// zero.
#[test]
fn lone_agent_has_no_target_and_no_intents() {
    let mut w = World::new(5);
    let solo = w.spawn_agent(Vec2::new(100.0, 100.0), Genome::neutral());
    w.agents.program[solo as usize] = Program::from_slice(&[
        Node::SenseOtherDist,
        Node::Neg,
        Node::ThresholdGt(-6.0),
        Node::FireWeapon,
    ]);
    step(&mut w);
    let a = w.actions[solo as usize];
    assert_eq!(a.target_id, NO_TARGET);
    assert_eq!(a.fire_intent, 0.0); // other_dist is INFINITY -> condition false
}

/// Emit/broadcast intents reach the action buffer through the pipeline, on the
/// correct channels.
#[test]
fn emit_and_broadcast_intents_reach_action_buffer() {
    let mut w = World::new(5);
    let id = w.spawn_agent(Vec2::new(700.0, 700.0), Genome::neutral());
    w.agents.program[id as usize] = Program::from_slice(&[
        Node::Const(1.0),
        Node::EmitPheromone(0),
        Node::Const(1.0),
        Node::Broadcast(2),
    ]);
    step(&mut w);
    assert_eq!(w.actions[id as usize].emit_intent[0], 1.0);
    assert_eq!(w.actions[id as usize].broadcast_intent[2], 1.0);
}

/// Relative-size sensing drives a flee-from-bigger behavior through decide.
#[test]
fn relative_size_channel_is_visible_to_programs() {
    let mut w = World::new(5);
    let mut small = Genome::neutral();
    small.set(GenomeSlot::Size, 0.3);
    let mut big = Genome::neutral();
    big.set(GenomeSlot::Size, 1.0);
    let me = w.spawn_agent(Vec2::new(800.0, 800.0), small);
    let _bigger = w.spawn_agent(Vec2::new(804.0, 800.0), big); // present to be sensed; id not needed
                                                               // move_x = rel_size (will be >1 because neighbor is bigger)
    w.agents.program[me as usize] = Program::from_slice(&[Node::SenseRelSize, Node::MoveTowardX]);
    step(&mut w);
    // MoveTowardX is the only move node, so the raw vector is (rel_size, 0)
    // with rel_size > 0, which normalizes to exactly (1, 0). Assert > 0.9.
    assert!(w.desired_direction[me as usize].x > 0.9);
}
