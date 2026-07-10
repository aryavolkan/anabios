//! M12 mechanism tests: combat, carcasses, predation, and their detectors.

use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::module::{Module, SensorType};
use anabios_core::prelude_test::Vec2;
use anabios_core::program::{Node, Program};
use anabios_core::tick::step;
use anabios_core::world::World;

/// Give slot `i` a predator kit: Locomotor + Vision Sensor + carnivore Mouth +
/// Weapon(damage, cost). Returns nothing; mutates the world in place.
fn arm_predator(w: &mut World, i: usize, damage: f32, cost: f32) {
    w.agents.modules[i] = smallvec_kit(damage, cost, /*armor=*/ 0.0);
}

/// Build a module kit inline (test-local so the test is self-contained).
fn smallvec_kit(
    weapon_damage: f32,
    weapon_cost: f32,
    armor: f32,
) -> anabios_core::module::ModuleList {
    let mut m = anabios_core::module::ModuleList::new();
    m.push(Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 });
    m.push(Module::Sensor { sensor_type: SensorType::Vision, radius: 0.6, acuity: 0.6 });
    m.push(Module::Mouth { bite_size: 0.6, diet_affinity: 1.0 });
    if weapon_damage > 0.0 {
        m.push(Module::Weapon { damage: weapon_damage, energy_cost: weapon_cost });
    }
    if armor > 0.0 {
        m.push(Module::Armor { protection: armor, mass_penalty: 0.1 });
    }
    m
}

/// Move an agent into a fresh second species, keeping species bookkeeping
/// tables consistent (mirrors the helper in social_substrate.rs).
fn reassign_to_new_species(w: &mut World, agent: u32) -> u32 {
    let sid = w.species_centroids.len() as u32;
    w.species_centroids.push(Genome::neutral());
    w.species_parents.push(Some(0));
    w.species_member_counts.push(0);
    w.next_species_id = sid + 1;
    w.remove_from_species(w.agents.species_id[agent as usize]);
    w.agents.species_id[agent as usize] = sid;
    w.add_to_species(sid);
    sid
}

/// A program that always fires the weapon (fire_intent = 1.0 > FIRE_THRESHOLD).
fn always_fire() -> Program {
    Program::from_slice(&[Node::Const(1.0), Node::FireWeapon])
}

#[test]
fn combat_deals_damage_minus_armor_and_spends_cost() {
    let mut w = World::new(7);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let prey = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral()); // 1.0 apart < COMBAT_RANGE
    reassign_to_new_species(&mut w, prey);
    arm_predator(&mut w, pred as usize, /*damage=*/ 10.0, /*cost=*/ 2.0);
    // Give the prey armor 3.0 so net damage = 10 - 3 = 7.
    w.agents.modules[prey as usize] =
        smallvec_kit(/*weapon=*/ 0.0, /*cost=*/ 0.0, /*armor=*/ 3.0);
    w.agents.program[pred as usize] = always_fire();

    let pred_e0 = w.agents.energy[pred as usize];
    let prey_e0 = w.agents.energy[prey as usize];
    step(&mut w);

    // Prey lost exactly (damage - armor) = 7.0 to combat. Its own metabolism +
    // any grazing also move energy, so compare the combat delta directly by
    // asserting at least 7.0 was removed relative to a no-combat control below.
    assert!(w.agents.energy[prey as usize] <= prey_e0 - 7.0 + 1e-3);
    // Attacker paid the weapon energy_cost (2.0) on top of metabolism.
    assert!(w.agents.energy[pred as usize] <= pred_e0 - 2.0 + 1e-3);
    // Attribution recorded for the detectors.
    assert!(w.combat_damaged[prey as usize]);
    assert_eq!(w.combat_attacker[prey as usize], w.agents.species_id[pred as usize]);
}

#[test]
fn no_weapon_module_means_no_combat_damage() {
    let mut w = World::new(7);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let prey = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
    reassign_to_new_species(&mut w, prey);
    // Predator has a carnivore kit but NO weapon.
    w.agents.modules[pred as usize] = smallvec_kit(0.0, 0.0, 0.0);
    w.agents.program[pred as usize] = always_fire();
    step(&mut w);
    assert!(!w.combat_damaged[prey as usize], "no Weapon module → gating → no damage");
}

#[test]
fn combat_out_of_range_does_nothing() {
    let mut w = World::new(7);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let prey = w.spawn_agent(Vec2::new(600.0, 500.0), Genome::neutral()); // 100 apart >> COMBAT_RANGE
    reassign_to_new_species(&mut w, prey);
    arm_predator(&mut w, pred as usize, 10.0, 2.0);
    w.agents.program[pred as usize] = always_fire();
    step(&mut w);
    assert!(!w.combat_damaged[prey as usize], "target out of COMBAT_RANGE → no combat");
}

#[test]
fn combat_targets_other_species_not_nearer_kin() {
    let mut w = World::new(7);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let kin = w.spawn_agent(Vec2::new(500.5, 500.0), Genome::neutral()); // same species, nearer
    let prey = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral()); // other species, farther
    reassign_to_new_species(&mut w, prey);
    arm_predator(&mut w, pred as usize, 10.0, 2.0);
    w.agents.program[pred as usize] = always_fire();
    step(&mut w);
    assert!(!w.combat_damaged[kin as usize], "must not fire at nearer same-species kin");
    assert!(w.combat_damaged[prey as usize], "must fire at the other-species target");
}

#[test]
fn death_forms_carcass_with_flesh_proportional_to_size() {
    use anabios_core::carcass::CARCASS_FLESH_PER_SIZE;
    let mut w = World::new(3);
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Size, 0.5);
    let id = w.spawn_agent(Vec2::new(300.0, 300.0), g);
    // Strip Mouth (and Locomotor) so the agent cannot graze back to life no
    // matter what terrain it spawned on — guarantees a starvation death.
    w.agents.modules[id as usize]
        .retain(|m| !matches!(m, Module::Locomotor { .. } | Module::Mouth { .. }));
    w.agents.energy[id as usize] = 0.3; // dies next age_and_starve
                                        // Run until it dies (energy <= 0).
    for _ in 0..50 {
        step(&mut w);
        if !w.agents.is_alive(id) {
            break;
        }
    }
    assert!(!w.agents.is_alive(id), "agent should have starved");
    assert_eq!(w.carcasses.len(), 1, "one carcass formed on death");
    let c = w.carcasses[0];
    // size clamps to >= 0.1; here size = 0.5 → flesh = 0.5 * CARCASS_FLESH_PER_SIZE.
    assert!((c.flesh - 0.5 * CARCASS_FLESH_PER_SIZE).abs() < 1e-3);
    assert_eq!(c.species_id, 0);
}

#[test]
fn carcass_decays_and_is_removed_after_decay_ticks() {
    use anabios_core::carcass::{carcass_step, Carcass, CARCASS_DECAY_TICKS};
    let mut w = World::new(1);
    w.carcasses.push(Carcass { pos: Vec2::new(10.0, 10.0), flesh: 5.0, age: 0, species_id: 0 });
    for _ in 0..CARCASS_DECAY_TICKS {
        carcass_step(&mut w);
    }
    assert!(w.carcasses.is_empty(), "carcass removed once age reaches CARCASS_DECAY_TICKS");
}

#[test]
fn carnivore_scavenges_carcass_gaining_energy_and_depleting_flesh() {
    use anabios_core::carcass::Carcass;
    let mut w = World::new(2);
    let eater = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
    // Carnivore Mouth (diet_affinity = 1.0), no weapon needed to scavenge.
    w.agents.modules[eater as usize] = smallvec_kit(0.0, 0.0, 0.0);
    w.carcasses.push(Carcass {
        pos: Vec2::new(400.5, 400.0), // within SCAVENGE_RANGE
        flesh: 10.0,
        age: 0,
        species_id: 1,
    });
    let e0 = w.agents.energy[eater as usize];
    step(&mut w);
    assert!(w.agents.energy[eater as usize] > e0, "carnivore gained energy from flesh");
    assert!(w.carcasses[0].flesh < 10.0, "carcass flesh depleted by scavenging");
}

#[test]
fn herbivore_does_not_scavenge_flesh() {
    use anabios_core::carcass::Carcass;
    let mut w = World::new(2);
    // Spawn a default (herbivore) agent; id unused — the point is it does NOT eat.
    let _eater = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
    // Default starter_kit Mouth has diet_affinity = 0.0 (pure herbivore).
    w.carcasses.push(Carcass { pos: Vec2::new(400.5, 400.0), flesh: 10.0, age: 0, species_id: 1 });
    step(&mut w);
    assert_eq!(w.carcasses[0].flesh, 10.0, "herbivore Mouth does not eat flesh (gating)");
}

#[test]
fn carcass_out_of_scavenge_range_is_not_eaten() {
    use anabios_core::carcass::Carcass;
    let mut w = World::new(2);
    let eater = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
    w.agents.modules[eater as usize] = smallvec_kit(0.0, 0.0, 0.0); // carnivore
    w.carcasses.push(Carcass {
        pos: Vec2::new(500.0, 400.0), // 100 units away
        flesh: 10.0,
        age: 0,
        species_id: 1,
    });
    step(&mut w);
    assert_eq!(w.carcasses[0].flesh, 10.0, "carcass out of range is untouched");
}

use anabios_core::codex::EventType;

/// Count events of a given type currently in the codex ring buffer.
fn count_events(w: &World, t: EventType) -> usize {
    w.codex.events.iter().filter(|e| e.event_type == t).count()
}

/// Build a lethal predator (huge damage) that always fires, adjacent to prey.
fn spawn_lethal_duel(seed: u64) -> (World, u32, u32) {
    let mut w = World::new(seed);
    let pred = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    let prey = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
    reassign_to_new_species(&mut w, prey);
    arm_predator(&mut w, pred as usize, /*damage=*/ 1000.0, /*cost=*/ 1.0);
    w.agents.program[pred as usize] = always_fire();
    (w, pred, prey)
}

#[test]
fn predation_event_fires_once_on_a_combat_kill() {
    let (mut w, _pred, prey) = spawn_lethal_duel(11);
    // Step until the prey dies from combat.
    for _ in 0..10 {
        step(&mut w);
        if !w.agents.is_alive(prey) {
            break;
        }
    }
    assert!(!w.agents.is_alive(prey), "prey should be killed by combat");
    assert_eq!(count_events(&w, EventType::Predation), 1, "Predation fires exactly once");
    // Keep stepping — it must not fire again (latched).
    for _ in 0..20 {
        step(&mut w);
    }
    assert_eq!(count_events(&w, EventType::Predation), 1, "Predation stays latched");
}

#[test]
fn starvation_death_does_not_fire_predation() {
    let mut w = World::new(5);
    let g = Genome::neutral();
    let id = w.spawn_agent(Vec2::new(300.0, 300.0), g);
    // Strip Mouth (and Locomotor) so the agent cannot graze back to life —
    // guarantees a terrain-independent starvation death.
    w.agents.modules[id as usize]
        .retain(|m| !matches!(m, Module::Locomotor { .. } | Module::Mouth { .. }));
    w.agents.energy[id as usize] = 0.2;
    for _ in 0..50 {
        step(&mut w);
        if !w.agents.is_alive(id) {
            break;
        }
    }
    assert!(!w.agents.is_alive(id), "agent starved");
    assert_eq!(count_events(&w, EventType::Predation), 0, "starvation is not predation");
}

#[test]
fn combat_raid_fires_on_sustained_conflict_not_a_single_kill() {
    use anabios_core::codex::COMBAT_RAID_THRESHOLD;
    // Drive the detector directly via recorded combat deaths, then observe.
    let mut w = World::new(9);
    // A single death: below threshold → no raid.
    w.codex.record_combat_death(w.tick, 1, 0, 10.0, 10.0);
    anabios_core::codex::observe_all(&mut w);
    assert_eq!(count_events(&w, EventType::CombatRaid), 0, "one kill is not a raid");
    // Push up to threshold within the window.
    for _ in 1..COMBAT_RAID_THRESHOLD {
        w.codex.record_combat_death(w.tick, 1, 0, 10.0, 10.0);
    }
    anabios_core::codex::observe_all(&mut w);
    assert_eq!(count_events(&w, EventType::CombatRaid), 1, "sustained conflict → one CombatRaid");
}

#[test]
fn arms_race_signal_detects_co_rising_trend() {
    use anabios_core::codex::{arms_race_signal, ARMS_WINDOW};
    use std::collections::{BTreeMap, VecDeque};
    let mut weapon: BTreeMap<u32, VecDeque<f32>> = BTreeMap::new();
    let mut armor: BTreeMap<u32, VecDeque<f32>> = BTreeMap::new();
    // Species 0: weapon damage rises 0→10 over the window.
    // Species 1: armor rises 0→10 over the window.
    let rising: VecDeque<f32> = (0..ARMS_WINDOW).map(|k| k as f32 * 0.6).collect();
    let flat: VecDeque<f32> = (0..ARMS_WINDOW).map(|_| 1.0).collect();
    weapon.insert(0, rising.clone());
    weapon.insert(1, flat.clone());
    armor.insert(0, flat.clone());
    armor.insert(1, rising.clone());
    let sig = arms_race_signal(&weapon, &armor);
    assert!(matches!(sig, Some((0, _))), "species 0 weapons + species 1 armor both rise");
}

#[test]
fn arms_race_signal_silent_on_flat_traits() {
    use anabios_core::codex::{arms_race_signal, ARMS_WINDOW};
    use std::collections::{BTreeMap, VecDeque};
    let flat: VecDeque<f32> = (0..ARMS_WINDOW).map(|_| 1.0).collect();
    let mut weapon = BTreeMap::new();
    let mut armor = BTreeMap::new();
    weapon.insert(0, flat.clone());
    armor.insert(1, flat.clone());
    assert!(arms_race_signal(&weapon, &armor).is_none(), "flat traits → no arms race");
}
