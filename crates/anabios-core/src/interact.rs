//! Interaction stage: feeding (grazing), combat, and predation (scavenging).

use crate::genome::GenomeSlot;
use crate::module::{self, ModuleType};
use crate::world::World;

/// `share_intent` above this triggers a transfer.
pub const SHARE_THRESHOLD: f32 = 0.5;
/// Max fraction of the donor's energy shared in one tick (before altruism scale).
pub const SHARE_FRACTION: f32 = 0.2;
/// Contact range (world units) for sharing. Mirrors COMBAT_RANGE.
pub const SHARE_RANGE: f32 = 2.0;

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
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());
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
    deposit_pass(world, &alive_ids);
    share_pass(world, &alive_ids);
    world.agents.scratch_ids = alive_ids;
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
        let mut desired_bite = BITE_MAX * size * bite_cap * herbivory;
        // Cumulative cultural skill (experiment C): Communicator-capable agents
        // apply a learned foraging-skill multiplier and learn-by-doing. Gated on
        // the module so non-communicator baselines are unchanged.
        let is_comm = module::has(&world.agents.modules[i], ModuleType::Communicator);
        // DIT env mode (experiment): whether this agent is "cultural" (learns
        // its technique) and which technique it currently forages with. The env
        // match-bonus is mutually exclusive with the C skill bonus.
        let il = world.agents.genome[i].get(GenomeSlot::IndividualLearning) > 0.5;
        let sl = world.agents.genome[i].get(GenomeSlot::SocialLearning) > 0.5;
        let cultural = il || sl;
        if world.env_period > 0 {
            // env DIT mode: match-based bonus (mutually exclusive with C skill).
            let technique = if cultural {
                world.agents.meme_vector[i][crate::culture::TECH_CHANNEL]
            } else {
                world.agents.genome[i].get(GenomeSlot::InnateTechnique)
            };
            let opt = crate::culture::env_optimum_at(world.tick, world.env_period);
            let m = crate::culture::technique_match(technique, opt);
            desired_bite *= 1.0 + crate::culture::ENV_BONUS * m;
        } else if is_comm {
            let skill = world.agents.meme_vector[i][crate::culture::SKILL_CHANNEL];
            desired_bite *= 1.0 + crate::culture::SKILL_BONUS * skill.clamp(0.0, 1.0);
        }
        // Biome adaptation (opt-in): reward EnvAffinity matching the local
        // climate. Composes multiplicatively with the DIT/skill bonuses above.
        if world.biome_adaptation {
            let env = world.biome.sample(pos).env;
            let affinity = world.agents.genome[i].get(GenomeSlot::EnvAffinity);
            let m = crate::culture::env_affinity_match(affinity, env);
            desired_bite *= 1.0 + crate::culture::ENV_AFFINITY_BONUS * m;
        }
        // Individual technique learning (env mode): an ONGOING cognitive process
        // that runs each foraging tick, decoupled from whether this tick's bite
        // landed — so a learner's technique tracks the shifting optimum reliably,
        // while the feeding BONUS above still rewards actually matching it. The
        // per-tick energy cost is what makes learning a real (survivable) expense.
        if world.env_period > 0 && cultural && il {
            let opt = crate::culture::env_optimum_at(world.tick, world.env_period);
            let t = &mut world.agents.meme_vector[i][crate::culture::TECH_CHANNEL];
            *t += crate::culture::ENV_LEARN_RATE * (opt - *t);
            world.agents.energy[i] -= crate::culture::ENV_LEARN_COST;
        }
        let taken = world.biome.graze(pos, desired_bite);
        if taken > 0.0 {
            world.agents.energy[i] += taken * FOOD_ENERGY_PER_BIOMASS;
            // C cumulative-skill learning-by-doing (env_period == 0) is still gated
            // on a successful graze — skill is mastery earned through feeding.
            if world.env_period == 0 && is_comm {
                let s = &mut world.agents.meme_vector[i][crate::culture::SKILL_CHANNEL];
                *s += crate::culture::SKILL_LEARN_RATE * (1.0 - *s);
            }
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
        // Record hit for the PackHunting detector.
        world.codex.combat_hits.push_back(crate::codex::CombatHit {
            tick: world.tick,
            target_id: tgt,
            attacker_id: id,
            species: world.agents.species_id[i],
        });
    }
}

/// Pheromone deposition: an agent with a `Pheromone` module writes each of its
/// above-threshold `emit_intent` channels into the field cell at its position,
/// scaled by the module's strength. Gated on the `Pheromone` module.
fn deposit_pass(world: &mut World, alive_ids: &[u32]) {
    use crate::pheromone::{PHEROMONE_DEPOSIT_SCALE, PHEROMONE_EMIT_THRESHOLD};
    for &id in alive_ids {
        let i = id as usize;
        if !module::has(&world.agents.modules[i], ModuleType::Pheromone) {
            continue;
        }
        let strength = module::effective_pheromone_strength(&world.agents.modules[i]);
        if strength <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        for ch in 0..crate::program::PHEROMONE_CHANNELS {
            let intent = world.actions[i].emit_intent[ch];
            if intent > PHEROMONE_EMIT_THRESHOLD {
                world.pheromones.deposit(pos, ch, intent * strength * PHEROMONE_DEPOSIT_SCALE);
            }
        }
    }
}

/// Predation: a carnivore-capable Mouth bites the nearest carcass within
/// `SCAVENGE_RANGE`, converting its flesh into energy. Ties on distance break
/// toward the lower carcass index, keeping this deterministic. Carcasses are
/// indexed in `world.carcass_spatial` (rebuilt below) so the per-agent search
/// touches only nearby carcasses instead of scanning the whole list.
fn scavenge_pass(world: &mut World, alive_ids: &[u32]) {
    use crate::carcass::{FLESH_ENERGY_PER_UNIT, SCAVENGE_MAX, SCAVENGE_RANGE};
    if world.carcasses.is_empty() {
        return;
    }
    world.carcass_spatial.rebuild_indexed(
        world.carcasses.len(),
        |ci| world.carcasses[ci].pos,
        |ci| world.carcasses[ci].flesh > 0.0,
    );
    for &id in alive_ids {
        let i = id as usize;
        if !module::has(&world.agents.modules[i], ModuleType::Mouth) {
            continue;
        }
        let carn = module::effective_diet_carnivory(&world.agents.modules[i]);
        let bite_cap = module::effective_bite_size(&world.agents.modules[i]);
        if carn <= 0.0 || bite_cap <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let mut best: Option<usize> = None;
        let mut best_d = SCAVENGE_RANGE;
        world.carcass_spatial.query(pos, SCAVENGE_RANGE, |ci| {
            let ci = ci as usize;
            // Re-check flesh: an earlier scavenger this same tick may have
            // depleted this carcass below the prefilter snapshot.
            if world.carcasses[ci].flesh <= 0.0 {
                return;
            }
            let d = crate::spatial::torus_distance(pos, world.carcasses[ci].pos);
            // Strict `<` on distance plus lowest-index tie-break reproduces the
            // old ascending-index linear scan exactly.
            if d < best_d || (d == best_d && best.is_some_and(|b| ci < b)) {
                best_d = d;
                best = Some(ci);
            }
        });
        if let Some(ci) = best {
            let size = world.agents.genome[i].get(GenomeSlot::Size).max(0.1);
            let desired = SCAVENGE_MAX * size * bite_cap * carn;
            let taken = desired.min(world.carcasses[ci].flesh);
            if taken > 0.0 {
                world.carcasses[ci].flesh -= taken;
                world.agents.energy[i] += taken * FLESH_ENERGY_PER_UNIT;
            }
        }
    }
}

/// Altruism: a donor with `share_intent` transfers a fraction of its energy to
/// its action target (the nearest neighbor), scaled by the `Altruism` genome
/// slot. Donor loses, recipient gains. Program-level gating on `SenseKinship`
/// makes this kin-directed.
fn share_pass(world: &mut World, alive_ids: &[u32]) {
    for &id in alive_ids {
        let i = id as usize;
        if world.actions[i].share_intent <= SHARE_THRESHOLD {
            continue;
        }
        let altruism = world.agents.genome[i].get(GenomeSlot::Altruism);
        if altruism <= 0.0 {
            continue;
        }
        let tgt = world.actions[i].target_id;
        if tgt == crate::program::NO_TARGET {
            continue;
        }
        let t = tgt as usize;
        if t == i || !world.agents.is_alive(tgt) {
            continue;
        }
        if world.sensors[i].nearest_neighbor_dist >= SHARE_RANGE {
            continue;
        }
        let amount = SHARE_FRACTION * world.agents.energy[i].max(0.0) * altruism;
        if amount <= 0.0 {
            continue;
        }
        world.agents.energy[i] -= amount;
        world.agents.energy[t] += amount;
        // Record for the EvolvedCooperation detector.
        world.codex.share_events.push_back((world.tick, world.agents.species_id[i]));
    }
}
