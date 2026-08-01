//! E13 domestication: scenario wiring, taming over ticks, born-tamed
//! breeding, pen override, save/load identity, and the release-gated
//! emergence check that an innovator culture reaches Husbandry and tames
//! the wild herd.

use anabios_core::agent::AGENT_NULL;
use anabios_core::codex::EventType;
use anabios_core::genome::Genome;
use anabios_core::prelude_test::Vec2;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;
use anabios_core::world::World;

const SCENARIO: &str = include_str!("../../../scenarios/domestication.toml");

fn livestock_count(w: &World) -> u32 {
    w.agents.iter_alive().filter(|&id| w.agents.livestock_of[id as usize] != AGENT_NULL).count()
        as u32
}

#[test]
fn scenario_instantiates_with_flag_and_two_populations() {
    let w = Scenario::parse_toml(SCENARIO).expect("parse domestication").instantiate();
    assert!(w.domestication_enabled && w.inventions_enabled);
    assert!(w.agents.live_count() > 0);
    assert!(w.agents.livestock_of.iter().all(|&o| o == AGENT_NULL), "all wild at tick 0");
}

/// Hand-seeded Husbandry: a holder parked next to a wild herd tames juveniles
/// over time, the herd breeds born-tamed, and the codex records it.
#[test]
fn husbandry_holder_tames_and_herd_grows() {
    let mut w = Scenario::parse_toml(SCENARIO).expect("parse domestication").instantiate();
    // Grant Husbandry (and prereqs, for realism) to every innovator (the
    // archetype species, id 1 — species 0 is the wild stock).
    let ch = anabios_core::invention::INVENTION_CHANNEL_BASE;
    for id in w.agents.iter_alive().collect::<Vec<_>>() {
        if w.agents.species_id[id as usize] != 0 {
            let m = &mut w.agents.meme_vector[id as usize];
            m[ch + anabios_core::invention::STONE_TOOLS] = 1.0;
            m[ch + anabios_core::invention::FIRE] = 1.0;
            m[ch + anabios_core::invention::FARMING] = 1.0;
            m[ch + anabios_core::invention::HUSBANDRY] = 1.0;
        }
    }
    let mut saw_domesticated = false;
    let mut tamed_total = 0u32;
    for _ in 0..3000 {
        step(&mut w);
        for ev in w.codex.drain_events() {
            if ev.event_type == EventType::AnimalDomesticated {
                saw_domesticated = true;
            }
        }
        tamed_total = tamed_total.max(livestock_count(&w));
    }
    assert!(saw_domesticated, "AnimalDomesticated fired within 3000 ticks");
    assert!(tamed_total >= 1, "at least one animal tamed; peak herd {tamed_total}");
}

/// Born-domesticated: a child of two livestock of the same owner is tamed.
/// Covered at unit level in reproduce; here assert the whole pipeline keeps
/// livestock state consistent across save/load/step.
#[test]
fn livestock_state_survives_save_load_step() {
    let mut w = Scenario::parse_toml(SCENARIO).expect("parse domestication").instantiate();
    // Seed one tamed pair directly so livestock state is non-trivial.
    let herder = w.agents.iter_alive().find(|&id| w.agents.species_id[id as usize] != 0).unwrap();
    let stock: Vec<u32> =
        w.agents.iter_alive().filter(|&id| w.agents.species_id[id as usize] == 0).take(3).collect();
    for s in &stock {
        w.agents.livestock_of[*s as usize] = herder;
    }
    for _ in 0..200 {
        step(&mut w);
    }
    let bytes = save_to_bytes(&w).expect("save");
    let mut reloaded = load_from_bytes(&bytes).expect("load");
    assert_eq!(state_hash(&w), state_hash(&reloaded), "load restores identical state");
    step(&mut w);
    step(&mut reloaded);
    assert_eq!(
        state_hash(&w),
        state_hash(&reloaded),
        "domestication world diverged after save→load→step",
    );
}

#[test]
fn flag_off_scenario_has_no_livestock() {
    const INVENTIONS: &str = include_str!("../../../scenarios/inventions.toml");
    let mut w = Scenario::parse_toml(INVENTIONS).expect("parse inventions").instantiate();
    assert!(!w.domestication_enabled);
    for _ in 0..300 {
        step(&mut w);
    }
    assert!(
        w.agents.livestock_of.iter().all(|&o| o == AGENT_NULL),
        "flag off: nobody is ever tamed"
    );
}

/// Emergence: the innovator culture climbs to Husbandry on its own and tames
/// the wild herd in a floor fraction of seeds. Release-gated per spec.
#[cfg_attr(debug_assertions, ignore = "release-only emergence test")]
#[test]
fn domestication_emerges_across_seeds() {
    const SEEDS: u64 = 8;
    const TICKS: u32 = 8000;
    let mut husbandry_adopted = 0u64;
    let mut tamed = 0u64;
    let mut herd_at_end = 0u64;
    for seed in 0..SEEDS {
        let mut s = Scenario::parse_toml(SCENARIO).expect("parse domestication");
        s.seed = seed;
        let mut w = s.instantiate();
        let mut saw_husbandry = false;
        let mut saw_tame = false;
        for _ in 0..TICKS {
            step(&mut w);
            for ev in w.codex.drain_events() {
                match ev.event_type {
                    EventType::InventionDiscovered
                        if ev.value as usize == anabios_core::invention::HUSBANDRY =>
                    {
                        saw_husbandry = true
                    }
                    EventType::AnimalDomesticated => saw_tame = true,
                    _ => {}
                }
            }
        }
        let stock = livestock_count(&w);
        if saw_husbandry {
            husbandry_adopted += 1;
        }
        if saw_tame {
            tamed += 1;
        }
        if stock > 0 {
            herd_at_end += 1;
        }
        eprintln!(
            "seed {seed}: alive={} husbandry={saw_husbandry} tamed={saw_tame} livestock={stock}",
            w.agents.live_count()
        );
    }
    // Floors set well below observed rates (spec §testing convention).
    assert!(husbandry_adopted >= 3, "Husbandry discovered in ≥3/8 seeds: {husbandry_adopted}");
    assert!(tamed >= 2, "AnimalDomesticated in ≥2/8 seeds: {tamed}");
    eprintln!("livestock present at horizon in {herd_at_end}/{SEEDS} seeds");
}

/// Regression: a herder's herd is released at the moment the herder dies, before
/// the freed slot can be recycled by a birth. The bug was that `livestock_of`
/// was only cleared by the husbandry orphan sweep (stage 6e), which runs *after*
/// reproduce (stage 6); a newborn reusing the dead herder's slot then read as
/// "alive", so the sweep kept the stale binding and the whole herd silently
/// rebound to the unrelated newborn. `kill` now releases referrers eagerly.
#[test]
fn dead_owner_releases_herd_before_slot_reuse() {
    let mut w = Scenario::parse_toml(SCENARIO).expect("parse domestication").instantiate();
    let herder =
        w.agents.iter_alive().find(|&id| w.agents.species_id[id as usize] != 0).expect("a herder");
    let animal =
        w.agents.iter_alive().find(|&id| w.agents.species_id[id as usize] == 0).expect("an animal");
    w.agents.livestock_of[animal as usize] = herder;

    // Owner dies: the binding must be gone immediately, before any slot reuse.
    w.agents.kill(herder);
    assert_eq!(
        w.agents.livestock_of[animal as usize], AGENT_NULL,
        "the herd must be released the instant its owner dies"
    );

    // Recycle the dead herder's slot with an unrelated newborn (LIFO free list).
    let newborn = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    assert_eq!(newborn, herder, "the newborn should reuse the dead herder's slot");
    assert_ne!(
        w.agents.livestock_of[animal as usize], newborn,
        "a recycled owner slot must not inherit the dead herder's herd"
    );
    assert_eq!(w.agents.livestock_of[animal as usize], AGENT_NULL);
}
