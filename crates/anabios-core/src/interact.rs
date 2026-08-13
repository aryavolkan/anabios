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
            let opt = crate::culture::env_optimum_at(
                world.tick,
                world.env_period,
                world.climate_drift_rate,
            );
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
        let coupling = world.gene_tech_coupling;
        desired_bite *=
            crate::invention::graze_multiplier_coupled(inv_mask, &world.agents.genome[i], coupling);
        // Individual technique learning (env mode): an ONGOING cognitive process
        // that runs each foraging tick, decoupled from whether this tick's bite
        // landed — so a learner's technique tracks the shifting optimum reliably,
        // while the feeding BONUS above still rewards actually matching it. The
        // per-tick energy cost is what makes learning a real (survivable) expense.
        if world.env_period > 0 && cultural && il {
            let opt = crate::culture::env_optimum_at(
                world.tick,
                world.env_period,
                world.climate_drift_rate,
            );
            let t = &mut world.agents.meme_vector[i][crate::culture::TECH_CHANNEL];
            *t += crate::culture::ENV_LEARN_RATE * (opt - *t);
            world.agents.energy[i] -= crate::culture::ENV_LEARN_COST;
        }
        let taken = world.biome.graze(pos, desired_bite);
        if taken > 0.0 {
            // Nutrient variation (opt-in): the local cell's static nutrient_quality
            // scales energy YIELD per biomass unit (not the biomass removed), so
            // poor cells cost the same bite for less energy — the foraging pressure.
            // Exactly 1.0 when the flag is off, so the payout is byte-identical.
            let quality_mult = if world.nutrient_variation {
                world.biome.sample(pos).nutrient_quality
            } else {
                1.0
            };
            // Fire buff: cooked food yields more energy per biomass unit.
            world.agents.energy[i] += taken
                * FOOD_ENERGY_PER_BIOMASS
                * crate::invention::food_energy_multiplier_coupled(
                    inv_mask,
                    &world.agents.genome[i],
                    coupling,
                )
                * quality_mult;
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
        // E13: a herder does not fire on its own livestock (no cost, no
        // damage). The owner's conspecifics are NOT exempt.
        if world.domestication_enabled && world.agents.livestock_of[t] == id {
            continue;
        }
        // Metalworking buff: better weapons deal more damage.
        let inv_weapon_mult = crate::invention::weapon_multiplier_coupled(
            crate::invention::held_mask(&world.agents.meme_vector[i]),
            &world.agents.genome[i],
            world.gene_tech_coupling,
        );
        // E12: males of dimorphic species hit harder (identity when the flag
        // is off).
        let dimorph_mult = crate::dimorphism::damage_factor(
            &world.agents.genome[i],
            world.agents.sex.get(i).map(|b| *b).unwrap_or(false),
            world.sexual_dimorphism_enabled,
        );
        let damage = weapon.damage * inv_weapon_mult * dimorph_mult;
        let armor = module::effective_armor_protection(&world.agents.modules[t]);
        let net = (damage - armor).max(0.0);
        world.agents.energy[t] -= net;
        world.agents.energy[i] -= weapon.energy_cost;
        world.combat_damaged[t] = true;
        world.combat_attacker[t] = world.agents.species_id[i];
        // M-C: being struck feeds RAGE. Written into the SERIALIZED affect
        // column here (same tick as the hit), so it round-trips through save/
        // load — unlike the #[serde(skip)] combat_damaged scratch, which the
        // affect stage must not read across a tick boundary. Gated + zero RNG.
        if world.affect_enabled {
            let r = &mut world.agents.affect[t][crate::affect::RAGE];
            *r = (*r + crate::affect::RAGE_ATTACK_IMPULSE).min(1.0);
        }
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
        // Compare by *squared* distance (monotonic on [0, ∞)), so the nearest
        // search drops a `sqrt` per visited carcass. `best_d2` is seeded with the
        // squared range so the strict-`<` bound matches the range gate.
        let mut best_d2 = SCAVENGE_RANGE * SCAVENGE_RANGE;
        world.carcass_spatial.query(pos, SCAVENGE_RANGE, |ci| {
            let ci = ci as usize;
            // Re-check flesh: an earlier scavenger this same tick may have
            // depleted this carcass below the prefilter snapshot.
            if world.carcasses[ci].flesh <= 0.0 {
                return;
            }
            let d2 =
                crate::spatial::torus_distance_sq(pos, world.carcasses[ci].pos, world.world_size);
            // Strict `<` plus lowest-index tie-break = deterministic nearest.
            if d2 < best_d2 || (d2 == best_d2 && best.is_some_and(|b| ci < b)) {
                best_d2 = d2;
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
                    * crate::invention::scavenge_multiplier_coupled(
                        crate::invention::held_mask(&world.agents.meme_vector[i]),
                        &world.agents.genome[i],
                        world.gene_tech_coupling,
                    );
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
        // Compare by *squared* distance (monotonic on [0, ∞)), dropping a `sqrt`
        // per visited node; `best_d2` seeded with the squared range.
        let mut best_d2 = HARVEST_RANGE * HARVEST_RANGE;
        world.resource_spatial.query(pos, HARVEST_RANGE, |ri| {
            let ri = ri as usize;
            if world.resources[ri].amount <= 0.0 {
                return;
            }
            let d2 =
                crate::spatial::torus_distance_sq(pos, world.resources[ri].pos, world.world_size);
            // Strict `<` plus lowest-index tie-break = deterministic nearest.
            if d2 < best_d2 || (d2 == best_d2 && best.is_some_and(|b| ri < b)) {
                best_d2 = d2;
                best = Some(ri);
            }
        });
        if let Some(ri) = best {
            let k = world.resources[ri].kind.index();
            // E8 harvest experience: practiced harvesters take more.
            let rate = crate::settlement::experienced_harvest(
                HARVEST_RATE,
                world.agents.harvest_exp[i][k],
            );
            let taken = rate.min(world.resources[ri].amount).min(room);
            if taken > 0.0 {
                world.agents.inventory[i][k] += taken;
                world.resources[ri].amount -= taken;
                crate::settlement::gain_harvest_exp(&mut *world, i, k);
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
        // Both `want(_, give)` terms are invariant across the inner `recv` loop;
        // hoist them (same operands, same subtraction order — bit-identical).
        let want_a_give = want(inv_a, give);
        let want_b_give = want(inv_b, give);
        for recv in 0..GOOD_COUNT {
            if recv == give || inv_b[recv] < TRADE_UNIT {
                continue; // same good, or B cannot spare `recv`
            }
            // A gives `give`, receives `recv`; B gives `recv`, receives `give`.
            let a_gain = want(inv_a, recv) - want_a_give;
            let b_gain = want_b_give - want(inv_b, recv);
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

/// Choose a unilateral surplus gift from A to B: A gives one `TRADE_UNIT` of
/// a good it holds above `STOCK_TARGET + TRADE_UNIT` — so giving costs A
/// nothing in `want` terms (its slot stays at/above target) — to B, who still
/// wants that good. This is the O2.6 escape from the measured trade freeze:
/// bilateral `pick_swap` self-terminates once holdings equalize or starve
/// (both parties must spare a `TRADE_UNIT`), but a surplus gift only needs a
/// giver above target and a receiver below it, so harvest-driven asymmetry
/// keeps exchange alive. Picks the good maximizing B's deficit reduction;
/// ties keep the lowest index. Deterministic, no RNG.
fn pick_gift(
    inv_a: &[f32; crate::resource::GOOD_COUNT],
    inv_b: &[f32; crate::resource::GOOD_COUNT],
) -> Option<usize> {
    use crate::resource::{want, STOCK_TARGET, TRADE_UNIT};
    let mut best: Option<usize> = None;
    let mut best_want = 0.0f32;
    for (give, &held) in inv_a.iter().enumerate() {
        if held < STOCK_TARGET + TRADE_UNIT {
            continue; // giving would cost A want — not surplus
        }
        let b_want = want(inv_b, give);
        if b_want > best_want {
            best_want = b_want;
            best = Some(give);
        }
    }
    best
}

/// Trade: each alive agent A (ascending) trades one `TRADE_UNIT` with its
/// nearest OTHER-species neighbor B (from the sensor register), if a mutually-
/// beneficial complementary swap exists and B is within `TRADE_RANGE`.
/// Conserves total units of each good. No RNG.
///
/// With `World.unilateral_trade` on, a failed bilateral swap falls back to a
/// one-sided surplus gift (`pick_gift`) — same conservation, same bookkeeping.
fn trade_pass(world: &mut World, alive_ids: &[u32]) {
    use crate::resource::TRADE_UNIT;
    // Keep the (serde-skipped) per-hub tally sized to the hub set: empty after a
    // snapshot load, and stale if hubs ever change. Cheap no-op once sized.
    if world.hub_trade_tally.len() != world.trade_hubs.len() {
        world.hub_trade_tally = vec![[0u64; crate::resource::GOOD_COUNT]; world.trade_hubs.len()];
    }
    for &id in alive_ids {
        let i = id as usize;
        // Trade only at hubs: an agent not near any predetermined hub cannot
        // barter this tick. Empty `trade_hubs` (resources off, or a hubless map)
        // therefore disables all trade — intended.
        if !crate::hub::near_any_hub(
            &world.trade_hubs,
            world.agents.position[i],
            world.world_size,
            crate::hub::HUB_TRADE_RANGE,
        ) {
            continue;
        }
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
        // The goods moving A→B and B→A; a bilateral swap moves two, a
        // unilateral gift moves one (recv == give, skipped below).
        let (give, recv) = match pick_swap(&inv_a, &inv_b) {
            Some(swap) => swap,
            None => {
                if !world.unilateral_trade {
                    continue;
                }
                match pick_gift(&inv_a, &inv_b) {
                    Some(give) => (give, give),
                    None => continue,
                }
            }
        };
        // Execute the transfer (totals conserved: each good's sum is unchanged).
        world.agents.inventory[i][give] -= TRADE_UNIT;
        world.agents.inventory[t][give] += TRADE_UNIT;
        if recv != give {
            world.agents.inventory[t][recv] -= TRADE_UNIT;
            world.agents.inventory[i][recv] += TRADE_UNIT;
        }
        world.total_trades += 1;
        // Viewer scratch: tally the traded good(s) to the nearest hub (the swap
        // happened within HUB_TRADE_RANGE of one). Never read by the sim.
        if let Some(h) = crate::hub::nearest_hub_index(
            &world.trade_hubs,
            world.agents.position[i],
            world.world_size,
        ) {
            world.hub_trade_tally[h][give] += 1;
            if recv != give {
                world.hub_trade_tally[h][recv] += 1;
            }
        }
        // E8 market field: the swap deposits density at the initiator's cell.
        crate::settlement::market_deposit(world, world.agents.position[i]);
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
    fn pick_gift_moves_surplus_to_a_wanter_at_zero_giver_cost() {
        use crate::resource::{Good, GOOD_COUNT, STOCK_TARGET, TRADE_UNIT};
        // A is hoarding Salt above STOCK_TARGET + TRADE_UNIT (the frozen
        // state's conserve-on-death concentrator); B holds nothing and wants
        // everything. No bilateral swap exists (B cannot give), but a surplus
        // gift does — the freeze escape.
        let mut a = [0.0f32; GOOD_COUNT];
        let mut b = [0.0f32; GOOD_COUNT];
        a[Good::Salt.index()] = STOCK_TARGET + TRADE_UNIT + 1.0;
        assert!(pick_swap(&a, &b).is_none(), "bilateral barter is stuck here");
        assert_eq!(pick_gift(&a, &b), Some(Good::Salt.index()));

        // Gifts the good B wants MOST when B has partial baskets.
        b[Good::Salt.index()] = 1.5; // want 0.5
        a[Good::Obsidian.index()] = STOCK_TARGET + TRADE_UNIT + 1.0; // B wants 2.0
        assert_eq!(pick_gift(&a, &b), Some(Good::Obsidian.index()));

        // No surplus (giving would cost A want) → no gift.
        let c = [STOCK_TARGET; GOOD_COUNT];
        let d = [0.0f32; GOOD_COUNT];
        assert_eq!(pick_gift(&c, &d), None);
        // B wants nothing → no gift.
        let e = [STOCK_TARGET; GOOD_COUNT];
        let f = [STOCK_TARGET + TRADE_UNIT + 1.0; GOOD_COUNT];
        assert_eq!(pick_gift(&f, &e), None);
    }

    #[test]
    fn trade_pass_gifts_unilaterally_only_with_flag_on() {
        use crate::resource::{Good, STOCK_TARGET, TRADE_UNIT};
        for flag in [false, true] {
            let mut w = World::new(5);
            w.resources_enabled = true;
            w.unilateral_trade = flag;
            let pos = Vec2::new(300.0, 300.0);
            let a = w.spawn_agent(pos, Genome::neutral());
            let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
            w.agents.species_id[b as usize] = 1;
            // Hoarder vs empty: bilateral pick_swap can never fire.
            w.agents.inventory[a as usize][Good::Salt.index()] = STOCK_TARGET + TRADE_UNIT + 2.0;

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
                false,
            );
            let alive: Vec<u32> = w.agents.iter_alive().collect();
            w.trade_hubs = vec![crate::hub::TradeHub { pos, cell: 0, goods: vec![] }];
            trade_pass(&mut w, &alive);

            let got = w.agents.inventory[b as usize][Good::Salt.index()];
            if flag {
                assert!(
                    got > 0.0,
                    "flag on: the hoarder gifts surplus Salt to the empty neighbour"
                );
                // Total Salt is conserved across the pair.
                let total: f32 = (0..2).map(|id| w.agents.inventory[id][Good::Salt.index()]).sum();
                assert!((total - (STOCK_TARGET + TRADE_UNIT + 2.0)).abs() < 1e-6);
            } else {
                assert_eq!(got, 0.0, "flag off: no bilateral swap exists, nothing moves");
            }
        }
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
            false,
        );

        let total_salt_before: f32 =
            (0..2).map(|id| w.agents.inventory[id][Good::Salt.index()]).sum();
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        w.trade_hubs = vec![crate::hub::TradeHub { pos, cell: 0, goods: vec![] }];
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
            false,
        );
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        w.trade_hubs = vec![crate::hub::TradeHub { pos, cell: 0, goods: vec![] }];
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
            false,
        );
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        w.trade_hubs = vec![crate::hub::TradeHub { pos, cell: 0, goods: vec![] }];
        trade_pass(&mut w, &alive);
        assert!(w.codex.first_cross_species_trade, "latch set after first trade");
        assert!(
            w.codex.events.iter().any(|e| e.event_type == EventType::ResourceTraded),
            "a ResourceTraded event was recorded"
        );
    }

    /// Build a weapon-bearing attacker adjacent to an other-species target,
    /// with `fire_intent` above `FIRE_THRESHOLD` and sensors/spatial
    /// populated so `combat_pass` will land a hit. Mirrors the fixture used
    /// by `trade_pass`'s tests and by `metalworking_raises_combat_damage` in
    /// `tests/inventions.rs`. Returns `(attacker, target)`.
    fn setup_adjacent_combat_pair(w: &mut World) -> (u32, u32) {
        let pos = Vec2::new(300.0, 300.0);
        let attacker = w.spawn_agent(pos, Genome::neutral());
        let target = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
        w.agents.modules[attacker as usize] = crate::module::predator_kit();
        w.agents.species_id[target as usize] = 1;
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
            false,
        );
        w.actions[attacker as usize].fire_intent = 1.0;
        (attacker, target)
    }

    #[test]
    fn combat_damage_bumps_target_rage_when_affect_on() {
        let mut w = World::new(13);
        w.affect_enabled = true;
        let (_attacker, target) = setup_adjacent_combat_pair(&mut w);
        let before = w.agents.affect[target as usize][crate::affect::RAGE];
        interact_all(&mut w);
        let after = w.agents.affect[target as usize][crate::affect::RAGE];
        assert!(after > before, "a struck agent accrues RAGE: {before} -> {after}");
        assert!(after <= 1.0, "RAGE stays clamped: {after}");

        // Flag-off: no write.
        let mut w2 = World::new(13);
        // affect_enabled defaults false
        let (_a2, t2) = setup_adjacent_combat_pair(&mut w2);
        interact_all(&mut w2);
        assert_eq!(
            w2.agents.affect[t2 as usize][crate::affect::RAGE],
            0.0,
            "flag off: combat must not touch affect"
        );
    }

    #[test]
    fn trade_pass_tallies_goods_to_nearest_hub() {
        use crate::hub::TradeHub;
        use crate::resource::Good;
        let mut w = World::new(5);
        w.resources_enabled = true;
        let pos = Vec2::new(300.0, 300.0);
        let a = w.spawn_agent(pos, Genome::neutral());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
        crate::prelude_test::reassign_to_new_species(&mut w, b);
        w.agents.inventory[a as usize][Good::Salt.index()] = 5.0;
        w.agents.inventory[b as usize][Good::Obsidian.index()] = 5.0;
        w.trade_hubs = vec![TradeHub { pos, cell: 0, goods: vec![] }];
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
            false,
        );
        let alive: Vec<u32> = w.agents.iter_alive().collect();
        trade_pass(&mut w, &alive);
        // A bilateral Salt<->Obsidian swap moves both goods; both counters at hub 0 rise.
        assert_eq!(w.hub_trade_tally.len(), 1);
        assert!(w.hub_trade_tally[0][Good::Salt.index()] >= 1);
        assert!(w.hub_trade_tally[0][Good::Obsidian.index()] >= 1);
    }

    #[test]
    fn hub_trade_tally_is_not_hashed_and_survives_reload() {
        use crate::hub::TradeHub;
        use crate::resource::Good;
        use crate::snapshot::{load_from_bytes, save_to_bytes, state_hash};
        let mut w = World::new(5);
        w.resources_enabled = true;
        let pos = Vec2::new(300.0, 300.0);
        let a = w.spawn_agent(pos, Genome::neutral());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), Genome::neutral());
        crate::prelude_test::reassign_to_new_species(&mut w, b);
        w.agents.inventory[a as usize][Good::Salt.index()] = 5.0;
        w.agents.inventory[b as usize][Good::Obsidian.index()] = 5.0;
        w.trade_hubs = vec![TradeHub { pos, cell: 0, goods: vec![] }];
        for _ in 0..20 {
            crate::tick::step(&mut w);
        }
        assert!(w.hub_trade_tally.iter().any(|t| t.iter().any(|&c| c > 0)), "tally populated");
        // Serde-skip: the tally is not in state_hash, so a reload (which drops it)
        // must hash identically, and must step without panicking (self-heal sizing).
        let bytes = save_to_bytes(&w).expect("save");
        let mut reload = load_from_bytes(&bytes).expect("load");
        assert_eq!(state_hash(&w), state_hash(&reload), "tally must not affect state_hash");
        crate::tick::step(&mut reload);
    }
}
