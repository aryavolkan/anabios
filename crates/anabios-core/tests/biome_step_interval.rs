//! Phase-1 "scale fields": `World::biome_step_interval` must (a) leave the
//! default cadence exactly as today, and (b) at `N > 1`, step the biome `N`
//! times less often — proven both mathematically (the shared cadence helper)
//! and behaviorally (a depleted world's total plant biomass stays flat on
//! ticks the coarser cadence skips, where the default cadence would have
//! grown it).
//!
//! Note: `tick == 0` always satisfies `is_multiple_of` regardless of
//! modulus (0 is a multiple of everything), so every world — any interval —
//! takes one biome step on its very first `step()` call. The behavioral test
//! below burns that shared, interval-independent step first and re-baselines
//! from tick 1, so what it measures is the *interval's* effect, not the
//! tick-0 artifact both worlds share identically.

use anabios_core::tick::{biome_step_interval, step, BIOME_STEP_INTERVAL};
use anabios_core::world::World;

/// Small no-agent world so a test run is cheap; deterministic same-seed biome
/// so the two worlds under comparison start identical.
fn make_world(seed: u64, interval: u32) -> World {
    let mut w = World::with_dims(seed, 256.0, 32, 16);
    w.biome_step_interval = interval;
    // Deplete every grass cell to 30% of capacity so `regrow_step` actually
    // changes biomass on a step tick — at full capacity (the fresh-generate
    // state) regrowth is a no-op regardless of cadence, which would prove
    // nothing about the interval.
    for cell in w.biome.cells.iter_mut() {
        let cap = cell.terrain.carrying_capacity();
        if cap > 0.0 {
            cell.plant_biomass = cap * 0.3;
        }
    }
    w
}

fn total_biomass(w: &World) -> f32 {
    w.biome.cells.iter().map(|c| c.plant_biomass).sum()
}

/// Step `w` until its Stage-10 gate has been checked at `entry_tick == target`
/// (the tick value the gate sees is `world.tick` *before* that call's
/// increment, so this must loop through `target` itself, not stop short of
/// it) — i.e. until `w.tick == target + 1`.
fn run_through_tick(w: &mut World, target: u64) {
    while w.tick <= target {
        step(w);
    }
}

#[test]
fn default_interval_matches_base_cadence() {
    let w = make_world(1, 1);
    assert_eq!(biome_step_interval(&w), BIOME_STEP_INTERVAL);
}

#[test]
fn interval_multiplies_the_base_cadence() {
    let w = make_world(1, 4);
    assert_eq!(biome_step_interval(&w), BIOME_STEP_INTERVAL * 4);
}

/// Direct proof the coarser cadence fires exactly `N` times less often: count
/// the ticks in `1..=window` (excluding the tick-0 artifact both cadences
/// share) where each world's effective cadence divides the tick, rather than
/// relying on any biome-content side effect.
#[test]
fn interval_4_steps_the_biome_exactly_a_quarter_as_often() {
    let baseline = make_world(1, 1);
    let coarser = make_world(1, 4);
    let window: u64 = 400; // a clean multiple of both cadences (10 and 40)
    let count =
        |w: &World| (1..=window).filter(|t| t.is_multiple_of(biome_step_interval(w))).count();
    let base_steps = count(&baseline);
    let coarse_steps = count(&coarser);
    assert_eq!(base_steps, 40, "sanity: ticks 10,20,..,400");
    assert_eq!(coarse_steps, 10, "sanity: ticks 40,80,..,400");
    assert_eq!(
        base_steps,
        coarse_steps * 4,
        "interval=4 must step the biome exactly 1/4 as often as interval=1 over the same window"
    );
}

/// Behavioral proof: with grazing/agents absent (nothing else touches
/// `plant_biomass`), the coarser-cadence world's total biomass stays exactly
/// flat between its step ticks (1 and 40), while the default-cadence world
/// visibly regrows at every multiple of 10 in between.
#[test]
fn biomass_stays_flat_between_step_ticks_under_a_coarser_interval() {
    let mut baseline = make_world(7, 1); // steps every 10 ticks (today's cadence)
    let mut coarser = make_world(7, 4); // steps every 40 ticks

    // Burn the shared tick-0 step (every world regrows once when its Stage-10
    // gate first sees `entry_tick == 0`, independent of the interval — 0 is a
    // multiple of everything — see the module doc) so what follows measures
    // the interval, not that shared artifact.
    run_through_tick(&mut baseline, 0);
    run_through_tick(&mut coarser, 0);
    let b1 = total_biomass(&baseline);
    let c1 = total_biomass(&coarser);
    assert!(
        (b1 - c1).abs() < 1e-6,
        "both worlds take the same tick-0 step regardless of interval: {b1} vs {c1}"
    );

    // Through tick 10: the default cadence's gate has now seen entry_tick=10
    // (stepped again, grown); the 4x-coarser cadence's next step isn't until
    // entry_tick=40, so it must still be flat since tick 0.
    run_through_tick(&mut baseline, 10);
    run_through_tick(&mut coarser, 10);
    let b10 = total_biomass(&baseline);
    let c10 = total_biomass(&coarser);
    assert!(
        b10 > b1 + 1e-3,
        "interval=1 world should have regrown again by tick 10: {b1} -> {b10}"
    );
    assert!(
        (c10 - c1).abs() < 1e-6,
        "interval=4 world must stay flat through tick 10 (next step is tick 40): {c1} -> {c10}"
    );

    // Through tick 30 (still short of the coarser world's next step at 40).
    run_through_tick(&mut baseline, 30);
    run_through_tick(&mut coarser, 30);
    let c30 = total_biomass(&coarser);
    assert!(
        (c30 - c1).abs() < 1e-6,
        "interval=4 world must still be flat through tick 30 (next step is tick 40): {c1} -> {c30}"
    );

    // Through tick 40: the coarser world's gate finally sees entry_tick=40
    // and steps again.
    run_through_tick(&mut baseline, 40);
    run_through_tick(&mut coarser, 40);
    let c40 = total_biomass(&coarser);
    assert!(
        c40 > c1 + 1e-3,
        "interval=4 world should have regrown again by tick 40: {c1} -> {c40}"
    );
}

/// `biome_step_interval = 1` (the default) must be byte-identical to a world
/// built without ever touching the field — the hard "no behavior change at
/// the default" requirement, checked via the deterministic state hash.
#[test]
fn interval_one_is_byte_identical_to_the_field_absent() {
    use anabios_core::snapshot::state_hash;

    let mut default_world = World::with_dims(3, 256.0, 32, 16);
    let mut explicit_one = World::with_dims(3, 256.0, 32, 16);
    explicit_one.biome_step_interval = 1;
    assert_eq!(default_world.biome_step_interval, 1, "World::new must default the field to 1");

    for _ in 0..50 {
        step(&mut default_world);
        step(&mut explicit_one);
    }
    assert_eq!(state_hash(&default_world), state_hash(&explicit_one));
}
