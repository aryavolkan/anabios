//! Interaction stage: feeding (grazing), combat, predation (scavenging),
//! pheromone deposition, and altruistic energy sharing.

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
/// Contact range (world units) within which basic combat can land. Mirrors
/// `reproduce::MATING_RANGE` and `module::WEAPON_RANGE`.
pub const COMBAT_RANGE: f32 = crate::module::WEAPON_RANGE;

/// Run all interaction rules for one tick: feed, combat, scavenge, pheromone
/// deposit, then share. Each pass iterates alive agents in ascending id
/// order (determinism).
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
    world.combat_streaks.clear();
    world.trade_routes.clear();

    feed_pass(world, &alive_ids);
    combat_pass(world, &alive_ids);
    scavenge_pass(world, &alive_ids);
    if world.resources_enabled {
        harvest_pass(world, &alive_ids);
    }
    deposit_pass(world, &alive_ids);
    share_pass(world, &alive_ids);
    if world.resources_enabled {
        trade_pass(world, &alive_ids);
    }
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
        // Invention buffs (Stone Tools / Farming / Machinery). Identity when
        // the agent holds nothing (flag-off masks are always 0).
        let inv_mask = crate::invention::held_mask(&world.agents.meme_vector[i]);
        desired_bite *= crate::invention::graze_multiplier(inv_mask);
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
            // Fire buff: cooked food yields more energy per biomass unit.
            world.agents.energy[i] += taken
                * FOOD_ENERGY_PER_BIOMASS
                * crate::invention::food_energy_multiplier(inv_mask);
            // C cumulative-skill learning-by-doing (env_period == 0) is still gated
            // on a successful graze — skill is mastery earned through feeding.
            if world.env_period == 0 && is_comm {
                let s = &mut world.agents.meme_vector[i][crate::culture::SKILL_CHANNEL];
                *s += crate::culture::SKILL_LEARN_RATE * (1.0 - *s);
            }
        }
    }
}

/// Combat: a weapon-bearing agent that fires deals `damage - target_armor`
/// energy damage to the nearest *other-species* agent within its weapon's
/// reach (contact for `Weapon`/`Jaws`, several units for `Spines`), spending
/// its own weapon `energy_cost`.
fn combat_pass(world: &mut World, alive_ids: &[u32]) {
    for &id in alive_ids {
        let i = id as usize;
        if world.actions[i].fire_intent <= FIRE_THRESHOLD {
            continue;
        }
        let Some(weapon) = module::effective_weapon(&world.agents.modules[i]) else {
            continue; // no weapon module → gated out
        };
        let tgt = world.sensors[i].nearest_other_id;
        if tgt == crate::sense::NO_NEIGHBOR_ID {
            continue;
        }
        if world.sensors[i].nearest_other_dist >= weapon.range {
            continue;
        }
        let t = tgt as usize;
        if t == i || !world.agents.is_alive(tgt) {
            continue;
        }
        // Metalworking buff: better weapons deal more damage.
        let inv_weapon_mult = crate::invention::weapon_multiplier(crate::invention::held_mask(
            &world.agents.meme_vector[i],
        ));
        let damage = weapon.damage * inv_weapon_mult;
        let armor = module::effective_armor_protection(&world.agents.modules[t]);
        let net = (damage - armor).max(0.0);
        world.agents.energy[t] -= net;
        world.agents.energy[i] -= weapon.energy_cost;
        world.combat_damaged[t] = true;
        world.combat_attacker[t] = world.agents.species_id[i];
        // E7 war substrate: cross-faction hits feed hostility (wars are
        // fought with hits, deaths are decisive moments).
        crate::codex::war::record_war_hit(
            world,
            world.agents.species_id[t],
            world.agents.species_id[i],
        );
        // E6 behavioral context at fire time: lying-in-wait attacker, and
        // invention-boosted damage.
        let ambush =
            world.still_ticks.get(i).copied().unwrap_or(0) >= crate::codex::AMBUSH_STILL_MIN;
        let tool_boosted = inv_weapon_mult > 1.05;
        // Viewer scratch: draw a streak from the attacker to its target,
        // tinted by the attacker's genome hue.
        world.combat_streaks.push((
            world.agents.position[i],
            world.agents.position[t],
            world.agents.genome[i].get(GenomeSlot::ColorHue),
        ));
        // Record hit for the PackHunting detector.
        world.codex.combat_hits.push_back(crate::codex::CombatHit {
            tick: world.tick,
            target_id: tgt,
            attacker_id: id,
            species: world.agents.species_id[i],
            ambush,
            tool_boosted,
        });
        // Rolling hit log for the E6 named-behavior detectors.
        world.codex.sig_hit_log.push_back(crate::codex::SigHit {
            tick: world.tick,
            species: world.agents.species_id[i],
            ambush,
            tool_boosted,
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
            let d = crate::spatial::torus_distance(pos, world.carcasses[ci].pos, world.world_size);
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
                world.agents.energy[i] += taken
                    * FLESH_ENERGY_PER_UNIT
                    * crate::invention::scavenge_multiplier(crate::invention::held_mask(
                        &world.agents.meme_vector[i],
                    ));
            }
        }
    }
}

/// Harvest: any agent standing within `HARVEST_RANGE` of a resource node pulls
/// up to `HARVEST_RATE` of its good into inventory, bounded by carrying
/// capacity, depleting the node. Nodes are indexed in `resource_spatial`
/// (rebuilt here) so each agent's search touches only nearby nodes.
fn harvest_pass(world: &mut World, alive_ids: &[u32]) {
    use crate::resource::{carrying_cap, inventory_total, HARVEST_RANGE, HARVEST_RATE};
    if world.resources.is_empty() {
        return;
    }
    world.resource_spatial.rebuild_indexed(
        world.resources.len(),
        |ri| world.resources[ri].pos,
        |ri| world.resources[ri].amount > 0.0,
    );
    for &id in alive_ids {
        let i = id as usize;
        let cap = carrying_cap(&world.agents.modules[i]);
        let room = cap - inventory_total(&world.agents.inventory[i]);
        if room <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let mut best: Option<usize> = None;
        let mut best_d = HARVEST_RANGE;
        world.resource_spatial.query(pos, HARVEST_RANGE, |ri| {
            let ri = ri as usize;
            if world.resources[ri].amount <= 0.0 {
                return;
            }
            let d = crate::spatial::torus_distance(pos, world.resources[ri].pos, world.world_size);
            // Strict `<` plus lowest-index tie-break = deterministic nearest.
            if d < best_d || (d == best_d && best.is_some_and(|b| ri < b)) {
                best_d = d;
                best = Some(ri);
            }
        });
        if let Some(ri) = best {
            let taken = HARVEST_RATE.min(world.resources[ri].amount).min(room);
            if taken > 0.0 {
                let k = world.resources[ri].kind.index();
                world.agents.inventory[i][k] += taken;
                world.resources[ri].amount -= taken;
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
        // Record for the EvolvedCooperation (and E7 alliance) detectors.
        world.codex.share_events.push_back((
            world.tick,
            world.agents.species_id[i],
            world.agents.species_id[t],
        ));
    }
}

/// Choose the most mutually-beneficial one-for-one swap between two inventories,
/// from A's perspective: A gives good `give` and receives good `recv`. Considers
/// every (give, recv) pair where A can spare a `TRADE_UNIT` of `give` and B can
/// spare a `TRADE_UNIT` of `recv`, and returns the pair maximizing the summed
/// deficit-reduction of both sides, requiring BOTH to strictly gain. `None` if no
/// such swap exists. Iteration is ascending (give, then recv) with a strict `>`
/// on the score, so ties keep the lowest-index pair — deterministic, no RNG.
fn pick_swap(
    inv_a: &[f32; crate::resource::GOOD_COUNT],
    inv_b: &[f32; crate::resource::GOOD_COUNT],
) -> Option<(usize, usize)> {
    use crate::resource::{want, GOOD_COUNT, TRADE_UNIT};
    let mut best: Option<(usize, usize)> = None;
    let mut best_score = 0.0f32;
    for give in 0..GOOD_COUNT {
        if inv_a[give] < TRADE_UNIT {
            continue; // A cannot spare `give`
        }
        for recv in 0..GOOD_COUNT {
            if recv == give || inv_b[recv] < TRADE_UNIT {
                continue; // same good, or B cannot spare `recv`
            }
            // A gives `give`, receives `recv`; B gives `recv`, receives `give`.
            let a_gain = want(inv_a, recv) - want(inv_a, give);
            let b_gain = want(inv_b, give) - want(inv_b, recv);
            if a_gain > 0.0 && b_gain > 0.0 {
                let score = a_gain + b_gain;
                if score > best_score {
                    best_score = score;
                    best = Some((give, recv));
                }
            }
        }
    }
    best
}

/// Trade: each alive agent A (ascending) trades one `TRADE_UNIT` with its
/// nearest OTHER-species neighbor B (from the sensor register), if a mutually-
/// beneficial complementary swap exists and B is within `TRADE_RANGE`.
/// Conserves total units of each good. No RNG.
fn trade_pass(world: &mut World, alive_ids: &[u32]) {
    use crate::resource::TRADE_UNIT;
    for &id in alive_ids {
        let i = id as usize;
        let tgt = world.sensors[i].nearest_other_id;
        if tgt == crate::sense::NO_NEIGHBOR_ID {
            continue;
        }
        if world.sensors[i].nearest_other_dist >= crate::resource::TRADE_RANGE {
            continue;
        }
        let t = tgt as usize;
        if t == i || !world.agents.is_alive(tgt) {
            continue;
        }
        let inv_a = world.agents.inventory[i];
        let inv_b = world.agents.inventory[t];
        let Some((give, recv)) = pick_swap(&inv_a, &inv_b) else {
            continue;
        };
        // Execute the swap (totals conserved: each side's sum is unchanged).
        world.agents.inventory[i][give] -= TRADE_UNIT;
        world.agents.inventory[t][give] += TRADE_UNIT;
        world.agents.inventory[t][recv] -= TRADE_UNIT;
        world.agents.inventory[i][recv] += TRADE_UNIT;
        world.total_trades += 1;
        // Viewer scratch: draw a route from the initiating trader to its
        // partner, tinted by the initiator's genome hue.
        world.trade_routes.push((
            world.agents.position[i],
            world.agents.position[t],
            world.agents.genome[i].get(GenomeSlot::ColorHue),
        ));
        if !world.codex.first_cross_species_trade {
            world.codex.first_cross_species_trade = true;
            world.codex.push_event(crate::codex::CodexEvent {
                event_type: crate::codex::EventType::ResourceTraded,
                tick: world.tick,
                species_id: world.agents.species_id[i],
                value: give as f32,
                loc_x: world.agents.position[i].x,
                loc_y: world.agents.position[i].y,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::world::World;

    #[test]
    fn harvest_fills_inventory_and_depletes_node() {
        use crate::resource::{Good, Resource, HARVEST_RATE};
        let mut w = World::new(3);
        w.resources_enabled = true;
        let pos = Vec2::new(200.0, 200.0);
        let id = w.spawn_agent(pos, Genome::neutral());
        w.resources.push(Resource { pos, kind: Good::Salt, amount: 5.0 });
        // Build the agent spatial hash (harvest_pass rebuilds resource_spatial itself).
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let alive: Vec<u32> = w.agents.iter_alive().collect();
        harvest_pass(&mut w, &alive);

        assert!((w.agents.inventory[id as usize][Good::Salt.index()] - HARVEST_RATE).abs() < 1e-6);
        assert!((w.resources[0].amount - (5.0 - HARVEST_RATE)).abs() < 1e-6);
    }

    #[test]
    fn harvest_respects_carrying_cap() {
        use crate::resource::{carrying_cap, Good, Resource};
        let mut w = World::new(3);
        w.resources_enabled = true;
        let pos = Vec2::new(200.0, 200.0);
        let id = w.spawn_agent(pos, Genome::neutral());
        let cap = carrying_cap(&w.agents.modules[id as usize]);
        // Pre-fill to the cap; no room to harvest more.
        w.agents.inventory[id as usize][Good::Amber.index()] = cap;
        w.resources.push(Resource { pos, kind: Good::Salt, amount: 5.0 });
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let alive: Vec<u32> = w.agents.iter_alive().collect();
        harvest_pass(&mut w, &alive);

        assert_eq!(w.agents.inventory[id as usize][Good::Salt.index()], 0.0, "at cap: no harvest");
        assert_eq!(w.resources[0].amount, 5.0, "node untouched");
    }

    #[test]
    fn pick_swap_is_mutually_beneficial_and_complementary() {
        use crate::resource::GOOD_COUNT;
        // A rich in good 0, poor in good 1; B the mirror image.
        let mut a = [0.0f32; GOOD_COUNT];
        let mut b = [0.0f32; GOOD_COUNT];
        a[0] = 5.0; // A surplus Salt
        b[1] = 5.0; // B surplus Obsidian
        let (give, recv) = pick_swap(&a, &b).expect("a beneficial swap exists");
        assert_eq!(give, 0, "A gives its surplus good 0");
        assert_eq!(recv, 1, "A receives good 1 (B's surplus)");
    }

    #[test]
    fn pick_swap_returns_none_without_complementary_surplus() {
        use crate::resource::GOOD_COUNT;
        // Both empty → nothing to give.
        let a = [0.0f32; GOOD_COUNT];
        let b = [0.0f32; GOOD_COUNT];
        assert!(pick_swap(&a, &b).is_none());
    }

    #[test]
    fn trade_pass_swaps_and_conserves_units() {
        use crate::resource::{Good, GOOD_COUNT, TRADE_UNIT};
        let mut w = World::new(5);
        w.resources_enabled = true;
        let pos = Vec2::new(300.0, 300.0);
        let a = w.spawn_agent(pos, Genome::neutral());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
        // Different species so it counts as cross-species trade.
        w.agents.species_id[b as usize] = 1;
        w.agents.inventory[a as usize][Good::Salt.index()] = 5.0;
        w.agents.inventory[b as usize][Good::Obsidian.index()] = 5.0;

        // Sense fills nearest_other_id/dist (trade_pass reads those, like combat_pass).
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        w.resize_scratch();
        crate::sense::sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &mut w.sensors,
            w.world_size,
        );

        let total_salt_before: f32 =
            (0..2).map(|id| w.agents.inventory[id][Good::Salt.index()]).sum();
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        trade_pass(&mut w, &alive);

        // Both goods are conserved across the pair, and A moved toward a
        // balanced basket (gave up some Salt, gained some Obsidian). Two
        // adjacent agents may each initiate a trade in one pass (the design
        // allows it), so we assert the invariants rather than a single-trade
        // exact value.
        let total_salt_after: f32 =
            (0..2).map(|id| w.agents.inventory[id][Good::Salt.index()]).sum();
        let total_obsidian_after: f32 =
            (0..2).map(|id| w.agents.inventory[id][Good::Obsidian.index()]).sum();
        assert!((total_salt_before - total_salt_after).abs() < 1e-6, "Salt conserved");
        assert!((total_obsidian_after - 5.0).abs() < 1e-6, "Obsidian conserved");
        assert!(w.agents.inventory[a as usize][Good::Salt.index()] < 5.0, "A gave up some Salt");
        assert!(
            w.agents.inventory[a as usize][Good::Obsidian.index()] > 0.0,
            "A received some Obsidian"
        );
        let _ = (GOOD_COUNT, TRADE_UNIT);
    }

    #[test]
    fn trade_pass_skips_same_species() {
        use crate::resource::Good;
        let mut w = World::new(5);
        w.resources_enabled = true;
        let pos = Vec2::new(300.0, 300.0);
        let a = w.spawn_agent(pos, Genome::neutral());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
        // SAME species (both 0).
        w.agents.inventory[a as usize][Good::Salt.index()] = 5.0;
        w.agents.inventory[b as usize][Good::Obsidian.index()] = 5.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        w.resize_scratch();
        crate::sense::sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &mut w.sensors,
            w.world_size,
        );
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        trade_pass(&mut w, &alive);
        assert_eq!(
            w.agents.inventory[a as usize][Good::Obsidian.index()],
            0.0,
            "no same-species trade"
        );
    }

    #[test]
    fn first_cross_species_trade_emits_event() {
        use crate::codex::EventType;
        use crate::resource::Good;
        let mut w = World::new(5);
        w.resources_enabled = true;
        let pos = Vec2::new(300.0, 300.0);
        let a = w.spawn_agent(pos, Genome::neutral());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
        w.agents.species_id[b as usize] = 1;
        w.agents.inventory[a as usize][Good::Salt.index()] = 5.0;
        w.agents.inventory[b as usize][Good::Obsidian.index()] = 5.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        w.resize_scratch();
        crate::sense::sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &mut w.sensors,
            w.world_size,
        );
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        trade_pass(&mut w, &alive);
        assert!(w.codex.first_cross_species_trade, "latch set after first trade");
        assert!(
            w.codex.events.iter().any(|e| e.event_type == EventType::ResourceTraded),
            "a ResourceTraded event was recorded"
        );
    }
}
