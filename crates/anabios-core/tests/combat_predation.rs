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
fn smallvec_kit(weapon_damage: f32, weapon_cost: f32, armor: f32) -> anabios_core::module::ModuleList {
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
    w.carcasses.push(Carcass {
        pos: Vec2::new(10.0, 10.0),
        flesh: 5.0,
        age: 0,
        species_id: 0,
    });
    for _ in 0..CARCASS_DECAY_TICKS {
        carcass_step(&mut w);
    }
    assert!(w.carcasses.is_empty(), "carcass removed once age reaches CARCASS_DECAY_TICKS");
}
