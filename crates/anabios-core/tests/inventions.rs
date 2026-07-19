//! Mutation-gated cultural inventions: the invention-level RATCHET (Task 1.2)
//! and the named tech-tree of stacking robust benefits it unlocks (Task 2.1):
//! Domestication (steady food), Writing (faster cultural copy), and the
//! Industrial Revolution (upkeep + reproduction efficiency). Every benefit is
//! gated end-to-end through `culture::invention_active` on
//! `World.cultural_inventions && is_inventive(genome) && has(Communicator) &&
//! invention_level(meme) >= threshold`, so the mechanism is fully inert
//! (byte-identical) when the flag is off — the golden must never move.

use anabios_core::agent::SPAWN_ENERGY;
use anabios_core::culture::{
    culture_step, invention_active, DOMESTICATION_ENERGY, DOMESTICATION_THRESHOLD,
    INDUSTRY_REPRO_DISCOUNT, INDUSTRY_THRESHOLD, INDUSTRY_UPKEEP_DISCOUNT, INVENTION_CHANNEL,
    INVENT_SOCIAL_RATE, WRITING_COPY_BONUS, WRITING_THRESHOLD,
};
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::module::{starter_kit, upkeep_all, Module, ModuleList};
use anabios_core::prelude_test::Vec2;
use anabios_core::program::{Node, Program, MEME_CHANNELS};
use anabios_core::reproduce::reproduce_all;
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

// --- Task 2.1: the named tech-tree ---

/// Run a single forager for one tick and return its post-tick energy. Both
/// the invention level and Inventiveness gene are directly controlled so the
/// only variable between two calls is exactly what the test wants to isolate
/// (the biome/graze outcome is otherwise fully deterministic and identical).
fn energy_after_one_tick(inv_level: f32, inventiveness: f32, flag: bool, comm: bool) -> f32 {
    let mut w = World::new(1);
    w.cultural_inventions = flag;
    let id = spawn_forager(&mut w, inventiveness, comm);
    w.agents.meme_vector[id as usize][INVENTION_CHANNEL] = inv_level;
    force_food_here(&mut w, POS);
    step(&mut w);
    w.agents.energy[id as usize]
}

#[test]
fn domestication_adds_energy_only_at_or_above_threshold() {
    let below = energy_after_one_tick(DOMESTICATION_THRESHOLD - 0.01, 0.9, true, true);
    let at = energy_after_one_tick(DOMESTICATION_THRESHOLD, 0.9, true, true);
    assert!(
        (at - below - DOMESTICATION_ENERGY).abs() < 1e-4,
        "expected exactly +DOMESTICATION_ENERGY crossing the threshold: below={below} at={at}"
    );
}

#[test]
fn domestication_inert_when_flag_off() {
    let below = energy_after_one_tick(0.0, 0.9, false, true);
    let above = energy_after_one_tick(1.0, 0.9, false, true);
    assert!(
        (above - below).abs() < 1e-4,
        "flag OFF must be inert to invention level regardless of tier crossed"
    );
}

#[test]
fn domestication_inert_when_gene_not_inventive() {
    // Inventiveness == INVENTIVE_THRESHOLD (0.5) is NOT inventive (strict >).
    let below = energy_after_one_tick(0.0, 0.5, true, true);
    let above = energy_after_one_tick(1.0, 0.5, true, true);
    assert!(
        (above - below).abs() < 1e-4,
        "non-inventive gene must never unlock Domestication, even at inv=1.0"
    );
}

#[test]
fn domestication_inert_without_communicator() {
    let below = energy_after_one_tick(0.0, 0.9, true, false);
    let above = energy_after_one_tick(1.0, 0.9, true, false);
    assert!(
        (above - below).abs() < 1e-4,
        "no Communicator must never unlock Domestication, even at inv=1.0"
    );
}

/// Build two Communicator neighbours, with independently controlled
/// invention levels, positioned so `culture_step` (called directly, not via
/// a full tick) can see them. Returns the world and the "copier" agent's id.
///
/// Spawns them world-diagonal apart first and runs one full `step()` — purely
/// to grow `World`'s per-tick scratch buffers (`actions`, `sensors`, ...),
/// which `resize_scratch` normally does but is crate-private; kept far apart
/// so that warm-up tick can't have them mate or transmit memes. Then moves
/// the neighbour next to the copier, rebuilds the spatial hash, and stamps in
/// the desired invention levels — the state `culture_step` will actually see.
fn setup_copy_pair(
    copier_inv: f32,
    neighbour_inv: f32,
    inventiveness: f32,
    flag: bool,
) -> (World, u32) {
    let mut w = World::new(7);
    w.cultural_inventions = flag;
    let mut g_copier = Genome::neutral();
    g_copier.set(GenomeSlot::Inventiveness, inventiveness);
    let copier = w.spawn_seeded(
        Vec2::new(100.0, 100.0),
        g_copier,
        0,
        kit_for(true),
        Program::from_slice(&[Node::Idle]),
    );
    let mut g_neighbour = Genome::neutral();
    g_neighbour.set(GenomeSlot::Inventiveness, inventiveness);
    let neighbour = w.spawn_seeded(
        Vec2::new(900.0, 900.0),
        g_neighbour,
        0,
        kit_for(true),
        Program::from_slice(&[Node::Idle]),
    );
    step(&mut w); // warm-up only: grows scratch buffers, agents too far apart to interact

    let copier_pos = POS;
    let neighbour_pos = Vec2::new(POS.x + 1.0, POS.y);
    w.agents.position[copier as usize] = copier_pos;
    w.agents.position[neighbour as usize] = neighbour_pos;
    w.agents.meme_vector[copier as usize][INVENTION_CHANNEL] = copier_inv;
    w.agents.meme_vector[neighbour as usize][INVENTION_CHANNEL] = neighbour_inv;
    // Callers invoke `culture::culture_step` directly right after this, so
    // `interact::feed_pass` never runs again — the copier's invention level
    // moves ONLY via the social-copy step under test.
    w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
    (w, copier)
}

#[test]
fn writing_accelerates_copy_rate_at_or_above_threshold() {
    let below_start = WRITING_THRESHOLD - 0.01;
    let (mut w_below, copier_below) = setup_copy_pair(below_start, 1.0, 0.9, true);
    culture_step(&mut w_below);
    let after_below = w_below.agents.meme_vector[copier_below as usize][INVENTION_CHANNEL];
    let expected_below = below_start + INVENT_SOCIAL_RATE * (1.0 - below_start);
    assert!(
        (after_below - expected_below).abs() < 1e-5,
        "below Writing threshold: expected plain INVENT_SOCIAL_RATE copy, got {after_below} want {expected_below}"
    );

    let (mut w_at, copier_at) = setup_copy_pair(WRITING_THRESHOLD, 1.0, 0.9, true);
    culture_step(&mut w_at);
    let after_at = w_at.agents.meme_vector[copier_at as usize][INVENTION_CHANNEL];
    let expected_at =
        WRITING_THRESHOLD + (INVENT_SOCIAL_RATE + WRITING_COPY_BONUS) * (1.0 - WRITING_THRESHOLD);
    assert!(
        (after_at - expected_at).abs() < 1e-5,
        "at/above Writing threshold: expected boosted copy rate, got {after_at} want {expected_at}"
    );
    // The boosted rate really is faster: same neighbour, copier starts at
    // (almost) the same level, but the at-threshold copier moves further.
    assert!(after_at - WRITING_THRESHOLD > after_below - below_start);
}

#[test]
fn writing_bonus_requires_the_flag() {
    // Flag off: the whole ratchet block (`if inventions_on && is_inventive`)
    // is skipped, so the invention channel can never be pulled UP toward a
    // more-advanced neighbour — regardless of whatever unrelated pre-existing
    // per-channel lerp channel 7 is still subject to when the flag is off
    // (Task 1.2 deliberately leaves it in the generic lerp in that case, to
    // stay golden-neutral for non-invention scenarios that still touch it).
    let (mut w, copier) = setup_copy_pair(WRITING_THRESHOLD, 1.0, 0.9, false);
    culture_step(&mut w);
    let level = w.agents.meme_vector[copier as usize][INVENTION_CHANNEL];
    assert!(
        level <= WRITING_THRESHOLD,
        "flag off must never ratchet the invention level up toward a neighbour, got {level}"
    );
}

#[test]
fn writing_bonus_requires_inventive_gene() {
    // Non-inventive gene (<=0.5), flag ON: channel 7 IS excluded from the
    // generic lerp whenever the flag is on (regardless of `is_inventive`),
    // and the ratchet's own copy step separately requires `is_inventive` —
    // so a non-inventive copier's invention channel must not move at all,
    // even next to a far-more-advanced Communicator neighbour.
    let (mut w, copier) = setup_copy_pair(WRITING_THRESHOLD, 1.0, 0.5, true);
    culture_step(&mut w);
    let level = w.agents.meme_vector[copier as usize][INVENTION_CHANNEL];
    assert_eq!(level, WRITING_THRESHOLD, "non-inventive gene: invention channel must not move");
}

#[test]
fn industry_discounts_upkeep_only_at_or_above_threshold_and_gated() {
    let mut w = World::new(1);
    w.cultural_inventions = true;
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Inventiveness, 0.9);
    let id = w.spawn_seeded(POS, g, 0, kit_for(true), Program::from_slice(&[Node::Idle]));
    let start_energy = w.agents.energy[id as usize];

    w.agents.meme_vector[id as usize][INVENTION_CHANNEL] = INDUSTRY_THRESHOLD - 0.01;
    upkeep_all(&mut w.agents, w.cultural_inventions);
    let full_cost = start_energy - w.agents.energy[id as usize];

    w.agents.energy[id as usize] = start_energy;
    w.agents.meme_vector[id as usize][INVENTION_CHANNEL] = INDUSTRY_THRESHOLD;
    upkeep_all(&mut w.agents, w.cultural_inventions);
    let discounted_cost = start_energy - w.agents.energy[id as usize];
    assert!(
        (full_cost - discounted_cost - INDUSTRY_UPKEEP_DISCOUNT).abs() < 1e-5,
        "expected upkeep to drop by exactly INDUSTRY_UPKEEP_DISCOUNT at the Industry tier: full={full_cost} discounted={discounted_cost}"
    );

    // Flag off: inert even at inv=1.0.
    w.agents.energy[id as usize] = start_energy;
    w.agents.meme_vector[id as usize][INVENTION_CHANNEL] = 1.0;
    w.cultural_inventions = false;
    upkeep_all(&mut w.agents, w.cultural_inventions);
    let flag_off_cost = start_energy - w.agents.energy[id as usize];
    assert!(
        (flag_off_cost - full_cost).abs() < 1e-5,
        "flag off must not discount upkeep even at inv=1.0"
    );
}

#[test]
fn industry_lowers_reproduction_threshold_only_at_full_invention_and_gated() {
    // Two identical inventive Communicators with energy strictly between the
    // Industry-discounted and the full reproduction threshold: they should
    // mate only when the Industry tier (flag on + inv >= 1.0) is active.
    let repro_gene = 0.4;
    let base_threshold = SPAWN_ENERGY * repro_gene * 1.5; // personality-neutral factor == 1.0
    let discounted_threshold =
        SPAWN_ENERGY * (repro_gene - INDUSTRY_REPRO_DISCOUNT).max(0.05) * 1.5;
    let energy = (base_threshold + discounted_threshold) / 2.0;

    let mate_and_count = |flag: bool| -> (u32, u32) {
        let mut w = World::new(1);
        w.cultural_inventions = flag;
        let mut g0 = Genome::neutral();
        g0.set(GenomeSlot::Inventiveness, 0.9);
        g0.set(GenomeSlot::ReproductionThreshold, repro_gene);
        let a = w.spawn_seeded(POS, g0, 0, kit_for(true), Program::from_slice(&[Node::Idle]));
        let mut g1 = Genome::neutral();
        g1.set(GenomeSlot::Inventiveness, 0.9);
        g1.set(GenomeSlot::ReproductionThreshold, repro_gene);
        let b = w.spawn_seeded(
            Vec2::new(POS.x + 1.0, POS.y),
            g1,
            0,
            kit_for(true),
            Program::from_slice(&[Node::Idle]),
        );
        w.agents.energy[a as usize] = energy;
        w.agents.energy[b as usize] = energy;
        w.agents.meme_vector[a as usize][INVENTION_CHANNEL] = 1.0;
        w.agents.meme_vector[b as usize][INVENTION_CHANNEL] = 1.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        (before, w.agents.live_count())
    };

    let (before_on, after_on) = mate_and_count(true);
    assert_eq!(
        after_on,
        before_on + 1,
        "Industry-discounted threshold should allow mating at this energy"
    );

    let (before_off, after_off) = mate_and_count(false);
    assert_eq!(
        after_off, before_off,
        "flag off: threshold must not be discounted, no mating at this energy"
    );
}

#[test]
fn invention_active_stacks_all_tiers_at_full_level_and_is_fully_gated() {
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Inventiveness, 0.9); // inventive
    let mut meme_full = [0.0f32; MEME_CHANNELS];
    meme_full[INVENTION_CHANNEL] = 1.0;

    // All three tiers active simultaneously (stacking) at full invention.
    assert!(invention_active(true, &g, &meme_full, true, DOMESTICATION_THRESHOLD));
    assert!(invention_active(true, &g, &meme_full, true, WRITING_THRESHOLD));
    assert!(invention_active(true, &g, &meme_full, true, INDUSTRY_THRESHOLD));

    // Flag off: all three inert.
    assert!(!invention_active(false, &g, &meme_full, true, DOMESTICATION_THRESHOLD));
    assert!(!invention_active(false, &g, &meme_full, true, WRITING_THRESHOLD));
    assert!(!invention_active(false, &g, &meme_full, true, INDUSTRY_THRESHOLD));

    // No Communicator: all three inert.
    assert!(!invention_active(true, &g, &meme_full, false, DOMESTICATION_THRESHOLD));
    assert!(!invention_active(true, &g, &meme_full, false, WRITING_THRESHOLD));
    assert!(!invention_active(true, &g, &meme_full, false, INDUSTRY_THRESHOLD));

    // Non-inventive gene (<=0.5): all three inert even at inv=1.0.
    let mut g_non = Genome::neutral();
    g_non.set(GenomeSlot::Inventiveness, 0.5);
    assert!(!invention_active(true, &g_non, &meme_full, true, DOMESTICATION_THRESHOLD));
    assert!(!invention_active(true, &g_non, &meme_full, true, WRITING_THRESHOLD));
    assert!(!invention_active(true, &g_non, &meme_full, true, INDUSTRY_THRESHOLD));
}
