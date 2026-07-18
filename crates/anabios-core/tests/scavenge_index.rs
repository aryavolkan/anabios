//! Regression tests for the spatial-hash scavenge pass
//! (`interact::scavenge_pass` + `World::carcass_spatial`).
//!
//! The indexed implementation must reproduce the exact selection semantics of
//! the original ascending-index linear scan: nearest non-depleted carcass
//! within `SCAVENGE_RANGE`, lowest-index tie-break, wrap-aware torus
//! distances.

use anabios_core::carcass::Carcass;
use anabios_core::genome::Genome;
use anabios_core::module::{Module, ModuleList};
use anabios_core::prelude_test::{reassign_to_new_species, Vec2};
use anabios_core::tick::step;
use anabios_core::world::World;

/// A stationary pure-carnivore kit: Mouth only (no Locomotor → cannot drift,
/// no Sensor → no perception, no Weapon → cannot kill the other test agent).
fn carnivore_mouth_only() -> ModuleList {
    let mut m = ModuleList::new();
    m.push(Module::Mouth { bite_size: 0.6, diet_affinity: 1.0 });
    m
}

fn spawn_carnivore(w: &mut World, pos: Vec2) -> u32 {
    let id = w.spawn_agent(pos, Genome::neutral());
    w.agents.modules[id as usize] = carnivore_mouth_only();
    id
}

/// When the first scavenger (ascending id order) depletes the shared nearest
/// carcass mid-pass, the second scavenger must fall through to its *next*
/// nearest carcass — the linear scan re-checked `flesh <= 0.0` per agent, so
/// the indexed pass must too (regression: the prefilter once used the
/// stale rebuild-time flesh snapshot here).
#[test]
fn depleted_carcass_falls_through_to_next_nearest() {
    let mut w = World::new(11);
    let a = spawn_carnivore(&mut w, Vec2::new(400.0, 400.0));
    let b = spawn_carnivore(&mut w, Vec2::new(401.0, 400.0));
    reassign_to_new_species(&mut w, b);
    // C0: nearest to both; too small to survive A's bite.
    w.carcasses.push(Carcass { pos: Vec2::new(400.4, 400.0), flesh: 0.1, age: 0, species_id: 0 });
    // C1: in range of B only; B's fall-through target once C0 is gone.
    w.carcasses.push(Carcass { pos: Vec2::new(401.8, 400.0), flesh: 5.0, age: 0, species_id: 0 });
    assert!(a < b, "A must scavenge first (ascending id)");

    step(&mut w);

    // C0 is fully depleted (and removed by carcass_step's retain).
    assert!(
        !w.carcasses.iter().any(|c| (c.pos.x - 400.4).abs() < 1e-3),
        "C0 depleted by A and removed"
    );
    // B fell through to C1: its flesh decreased even though C0 was nearer.
    let c1 = w.carcasses.iter().find(|c| (c.pos.x - 401.8).abs() < 1e-3).expect("C1 still present");
    assert!(c1.flesh < 5.0, "B fell through depleted C0 to C1 (flesh {})", c1.flesh);
}

/// Equal torus distances: the lower carcass index wins (the old scan's strict
/// `<` over ascending indices).
#[test]
fn equidistant_tie_breaks_to_lower_carcass_index() {
    let mut w = World::new(12);
    spawn_carnivore(&mut w, Vec2::new(400.0, 400.0));
    w.carcasses.push(Carcass {
        pos: Vec2::new(399.0, 400.0), // d = 1.0, index 0
        flesh: 5.0,
        age: 0,
        species_id: 0,
    });
    w.carcasses.push(Carcass {
        pos: Vec2::new(401.0, 400.0), // d = 1.0, index 1
        flesh: 5.0,
        age: 0,
        species_id: 0,
    });

    step(&mut w);

    assert!(w.carcasses[0].flesh < 5.0, "lower-index carcass scavenged");
    assert_eq!(w.carcasses[1].flesh, 5.0, "equidistant higher-index carcass untouched");
}

/// The carcass hash query must wrap around the torus the same way the linear
/// scan's `torus_distance` did.
#[test]
fn scavenge_wraps_around_torus() {
    let mut w = World::new(13);
    spawn_carnivore(&mut w, Vec2::new(1.0, 400.0));
    w.carcasses.push(Carcass {
        pos: Vec2::new(anabios_core::biome::WORLD_SIZE - 0.5, 400.0), // wrap d = 1.5
        flesh: 5.0,
        age: 0,
        species_id: 0,
    });

    step(&mut w);

    assert!(w.carcasses[0].flesh < 5.0, "carcass across the wrap was scavenged");
}

/// A carcass exactly at `SCAVENGE_RANGE` is out of reach (strict `<`), and a
/// depleted carcass left over from a prior tick is skipped.
#[test]
fn range_boundary_and_predepleted_carcass_are_skipped() {
    use anabios_core::carcass::SCAVENGE_RANGE;
    let mut w = World::new(14);
    spawn_carnivore(&mut w, Vec2::new(400.0, 400.0));
    w.carcasses.push(Carcass {
        pos: Vec2::new(400.0 + SCAVENGE_RANGE, 400.0), // exactly at the boundary
        flesh: 5.0,
        age: 0,
        species_id: 0,
    });
    w.carcasses.push(Carcass {
        pos: Vec2::new(400.5, 400.0), // in range but already depleted
        flesh: 0.0,
        age: 0,
        species_id: 0,
    });

    let e0 = w.agents.energy[0];
    step(&mut w);

    // carcass_step drops the flesh-0 entry; the boundary carcass must be whole.
    let boundary = w
        .carcasses
        .iter()
        .find(|c| (c.pos.x - (400.0 + SCAVENGE_RANGE)).abs() < 1e-3)
        .expect("boundary carcass present");
    assert_eq!(boundary.flesh, 5.0, "carcass exactly at SCAVENGE_RANGE untouched");
    assert!(w.agents.energy[0] < e0, "no flesh energy gained (only metabolism paid)");
}
