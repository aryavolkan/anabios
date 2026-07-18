//! Mutation-gated cultural inventions: the invention-level RATCHET (Task 1.2).
//!
//! An `Inventiveness`-gened Communicator makes slow SOLO progress on channel 7
//! by grazing (`interact::feed_pass`); any inventive Communicator copies FAST
//! from the best inventing neighbour (`culture::culture_step`). Everything is
//! gated on `World.cultural_inventions` so the mechanism is fully inert
//! (byte-identical) when the flag is off — the golden must never move.

use anabios_core::culture::INVENTION_CHANNEL;
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::module::{starter_kit, Module, ModuleList};
use anabios_core::prelude_test::Vec2;
use anabios_core::program::{Node, Program};
use anabios_core::tick::step;
use anabios_core::world::World;

const TICKS: u32 = 200;
/// A fixed spawn position, forced to an abundant, non-depleting grass cell
/// (see `force_food_here`) so grazing succeeds deterministically regardless
/// of the procedurally-generated biome under any given seed.
const POS: Vec2 = Vec2::new(512.0, 512.0);

/// A Communicator-appended `starter_kit` (mirrors `dit_boundary.rs`'s
/// `kit_for`): identical forager kit, with a Communicator module added only
/// when the strategy needs the culture channel.
fn kit_for(comm: bool) -> ModuleList {
    let mut k = starter_kit();
    if comm {
        k.push(Module::Communicator { range: 12.0, channel_id: 0 });
    }
    k
}

/// Force the cell under `pos` to abundant, renewable grass so a stationary
/// grazer always has food to bite — isolates the ratchet logic from biome
/// generation/depletion, which is not what this test is about.
fn force_food_here(w: &mut World, pos: Vec2) {
    let (col, row) = w.biome.cell_coords(pos);
    let cell = w.biome.at_mut(col, row);
    cell.terrain = anabios_core::biome::TerrainType::Grass;
    cell.plant_biomass = cell.terrain.carrying_capacity();
}

/// Spawn a stationary (Idle program) grazer at `POS`, with `inventiveness`
/// gene value and an optional Communicator module.
fn spawn_forager(w: &mut World, inventiveness: f32, comm: bool) -> u32 {
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Inventiveness, inventiveness);
    let id = w.spawn_seeded(POS, g, 0, kit_for(comm), Program::from_slice(&[Node::Idle]));
    force_food_here(w, POS);
    id
}

/// Re-top-up the food cell each tick so a long-lived agent never runs the
/// biome dry mid-test (grazing success is what we're asserting, not biome
/// carrying-capacity dynamics).
fn run_with_food(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        force_food_here(w, POS);
        step(w);
    }
}

#[test]
fn inventive_communicator_ratchets_up_solo() {
    let mut w = World::new(1);
    w.cultural_inventions = true;
    let id = spawn_forager(&mut w, 0.9, true); // Inventiveness > 0.5 (threshold)
    run_with_food(&mut w, TICKS);
    let level = w.agents.meme_vector[id as usize][INVENTION_CHANNEL];
    assert!(level > 0.1, "inventive Communicator should ratchet up invention level, got {level}");
}

#[test]
fn non_inventive_communicator_stays_at_zero() {
    let mut w = World::new(1);
    w.cultural_inventions = true;
    let id = spawn_forager(&mut w, 0.5, true); // gene == threshold, NOT inventive (strict >)
    run_with_food(&mut w, TICKS);
    let level = w.agents.meme_vector[id as usize][INVENTION_CHANNEL];
    assert_eq!(level, 0.0, "non-inventive gene must never gain invention level");
}

#[test]
fn flag_off_keeps_inventive_agent_at_zero() {
    let mut w = World::new(1);
    // cultural_inventions left at its default (false).
    assert!(!w.cultural_inventions);
    let id = spawn_forager(&mut w, 0.9, true);
    run_with_food(&mut w, TICKS);
    let level = w.agents.meme_vector[id as usize][INVENTION_CHANNEL];
    assert_eq!(level, 0.0, "flag OFF must leave the invention channel untouched");
}

#[test]
fn non_communicator_inventive_agent_stays_at_zero() {
    // Gene alone (without a Communicator) should never invent — the
    // mechanism is culture-capable-cognition-gated, mirroring the C skill.
    let mut w = World::new(1);
    w.cultural_inventions = true;
    let id = spawn_forager(&mut w, 0.9, false);
    run_with_food(&mut w, TICKS);
    let level = w.agents.meme_vector[id as usize][INVENTION_CHANNEL];
    assert_eq!(level, 0.0, "no Communicator → no invention, even with the gene");
}

#[test]
fn best_neighbour_invention_copies_fast() {
    // Two inventive Communicators: one already advanced (seeded high), one
    // fresh. The fresh one should copy toward the advanced one's level far
    // faster than solo INVENT_RATE alone would produce in a couple of ticks.
    let mut w = World::new(2);
    w.cultural_inventions = true;
    let advanced_pos = Vec2::new(512.0, 512.0);
    let fresh_pos = Vec2::new(514.0, 512.0); // well within Communicator range (12.0)
    let mut g_adv = Genome::neutral();
    g_adv.set(GenomeSlot::Inventiveness, 0.9);
    let advanced =
        w.spawn_seeded(advanced_pos, g_adv, 0, kit_for(true), Program::from_slice(&[Node::Idle]));
    w.agents.meme_vector[advanced as usize][INVENTION_CHANNEL] = 0.8;
    force_food_here(&mut w, advanced_pos);

    let mut g_fresh = Genome::neutral();
    g_fresh.set(GenomeSlot::Inventiveness, 0.9);
    let fresh =
        w.spawn_seeded(fresh_pos, g_fresh, 0, kit_for(true), Program::from_slice(&[Node::Idle]));
    force_food_here(&mut w, fresh_pos);

    step(&mut w);
    let fresh_level = w.agents.meme_vector[fresh as usize][INVENTION_CHANNEL];
    // One tick of social copy at INVENT_SOCIAL_RATE=0.15 toward 0.8 from 0.0
    // gives 0.12 — far more than a couple of solo INVENT_RATE=0.01 ticks (0.02).
    assert!(
        fresh_level > 0.05,
        "fresh inventive neighbour should copy fast from the advanced one, got {fresh_level}"
    );
}
