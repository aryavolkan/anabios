//! Interaction stage: feeding (grazing), combat, and predation (scavenging).

use crate::genome::GenomeSlot;
use crate::module::{self, ModuleType};
use crate::world::World;

/// Max biomass an agent can bite from the biome in one tick (before scaling).
pub const BITE_MAX: f32 = 0.5;
/// Energy yielded per unit of plant biomass eaten.
pub const FOOD_ENERGY_PER_BIOMASS: f32 = 4.0;
/// `fire_intent` above this threshold triggers a weapon attack.
pub const FIRE_THRESHOLD: f32 = 0.5;
/// Contact range (world units) within which combat can land. Mirrors
/// `reproduce::MATING_RANGE`.
pub const COMBAT_RANGE: f32 = 2.0;

/// Run all interaction rules for one tick: feed, then combat, then scavenge.
/// Each pass iterates alive agents in ascending id order (determinism).
pub fn interact_all(world: &mut World) {
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    // Reset combat attribution scratch for this tick. `combat_attacker` is only
    // read where `combat_damaged` is set, but reset it too so stale attacker
    // species from a prior tick can never leak into a consumer.
    for b in world.combat_damaged.iter_mut() {
        *b = false;
    }
    for v in world.combat_attacker.iter_mut() {
        *v = crate::sense::NO_NEIGHBOR_SPECIES;
    }

    feed_pass(world, &alive_ids);
    combat_pass(world, &alive_ids);
    scavenge_pass(world, &alive_ids);
}

/// Grazing: a herbivore-capable Mouth bites plant biomass at its cell.
fn feed_pass(world: &mut World, alive_ids: &[u32]) {
    for &id in alive_ids {
        let i = id as usize;
        if !module::has(&world.agents.modules[i], ModuleType::Mouth) {
            continue;
        }
        let bite_cap = module::effective_bite_size(&world.agents.modules[i]);
        let diet_carn = module::effective_diet_carnivory(&world.agents.modules[i]);
        let herbivory = (1.0 - diet_carn).clamp(0.0, 1.0);
        if herbivory <= 0.0 || bite_cap <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let size = world.agents.genome[i].get(GenomeSlot::Size).max(0.1);
        let desired_bite = BITE_MAX * size * bite_cap * herbivory;
        let taken = world.biome.graze(pos, desired_bite);
        if taken > 0.0 {
            world.agents.energy[i] += taken * FOOD_ENERGY_PER_BIOMASS;
        }
    }
}

/// Combat: a Weapon-bearing agent that fires deals `damage - target_armor`
/// energy damage to the nearest *other-species* agent within `COMBAT_RANGE`,
/// spending its own weapon `energy_cost`.
fn combat_pass(world: &mut World, alive_ids: &[u32]) {
    for &id in alive_ids {
        let i = id as usize;
        if world.actions[i].fire_intent <= FIRE_THRESHOLD {
            continue;
        }
        let Some((damage, cost)) = module::effective_weapon(&world.agents.modules[i]) else {
            continue; // no Weapon module → gated out
        };
        let tgt = world.sensors[i].nearest_other_id;
        if tgt == crate::sense::NO_NEIGHBOR_ID {
            continue;
        }
        if world.sensors[i].nearest_other_dist >= COMBAT_RANGE {
            continue;
        }
        let t = tgt as usize;
        if t == i || !world.agents.is_alive(tgt) {
            continue;
        }
        let armor = module::effective_armor_protection(&world.agents.modules[t]);
        let net = (damage - armor).max(0.0);
        world.agents.energy[t] -= net;
        world.agents.energy[i] -= cost;
        world.combat_damaged[t] = true;
        world.combat_attacker[t] = world.agents.species_id[i];
    }
}

/// Predation: filled in by Task 3 (carnivore Mouth scavenges carcasses).
fn scavenge_pass(_world: &mut World, _alive_ids: &[u32]) {}
