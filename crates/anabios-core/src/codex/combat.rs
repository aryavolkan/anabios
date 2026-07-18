//! Codex combat detectors (split from the former monolithic codex.rs).

use super::*;
use crate::world::World;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Update per-species weapon/armor trend windows from the fused per-species
/// aggregates, then edge-trigger ArmsRace when a co-rising trend appears.
pub(super) fn detect_arms_race(world: &mut World, agg: &SpeciesAggTable) {
    let push = |hist: &mut BTreeMap<u32, VecDeque<f32>>, sid: u32, mean: f32| {
        let buf = hist.entry(sid).or_default();
        if buf.len() == ARMS_WINDOW {
            buf.pop_front();
        }
        buf.push_back(mean);
    };
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        let n = entry.count as f64;
        push(&mut world.codex.weapon_history, sid, (entry.weapon_sum / n) as f32);
        push(&mut world.codex.armor_history, sid, (entry.armor_sum / n) as f32);
    }

    let signal = arms_race_signal(&world.codex.weapon_history, &world.codex.armor_history);
    match signal {
        Some((sid, rise)) if !world.codex.arms_race_active => {
            let (lx, ly) = centroid_of(agg, sid);
            world.codex.push_event(CodexEvent {
                event_type: EventType::ArmsRace,
                tick: world.tick,
                species_id: sid,
                value: rise,
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.arms_race_active = true;
        }
        None => world.codex.arms_race_active = false,
        _ => {}
    }
}

/// PackHunting: ≥ PACK_MIN_ATTACKERS distinct same-species agents deal combat
/// damage to one target within PACK_WINDOW ticks. Prunes the rolling window,
/// groups hits by (target, species), and edge-fires on the `pack_active` latch.
/// Re-arms when no qualifying (target, species) group exists.
pub(super) fn detect_pack_hunting(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    // Prune entries older than the rolling window (mirror detect_combat_raid).
    let cutoff = tick.saturating_sub(PACK_WINDOW);
    while let Some(front) = world.codex.combat_hits.front() {
        if front.tick < cutoff {
            world.codex.combat_hits.pop_front();
        } else {
            break;
        }
    }
    // Group by target → species → set of distinct attacker ids (BTreeMap → deterministic).
    let mut groups: BTreeMap<u32, BTreeMap<u32, BTreeSet<u32>>> = BTreeMap::new();
    for hit in world.codex.combat_hits.iter() {
        groups
            .entry(hit.target_id)
            .or_default()
            .entry(hit.species)
            .or_default()
            .insert(hit.attacker_id);
    }
    // Check whether any (target, species) pair has ≥ PACK_MIN_ATTACKERS.
    let mut raiding = false;
    let mut event_species: u32 = 0;
    let mut event_loc: (f32, f32) = (0.0, 0.0);
    'outer: for by_species in groups.values() {
        for (sid, attackers) in by_species.iter() {
            if attackers.len() >= PACK_MIN_ATTACKERS {
                raiding = true;
                event_species = *sid;
                event_loc = centroid_of(agg, *sid);
                break 'outer;
            }
        }
    }
    // Edge-trigger: fire on rising edge, re-arm on falling edge.
    if raiding && !world.codex.pack_active {
        world.codex.push_event(CodexEvent {
            event_type: EventType::PackHunting,
            tick,
            species_id: event_species,
            value: PACK_MIN_ATTACKERS as f32,
            loc_x: event_loc.0,
            loc_y: event_loc.1,
        });
        world.codex.pack_active = true;
    } else if !raiding {
        world.codex.pack_active = false;
    }
}

/// Predation: emit once, the first tick a combat-attributed death is recorded.
/// Payload species = the attacker (predator) species.
pub(super) fn detect_predation(world: &mut World) {
    if world.codex.predation_emitted {
        return;
    }
    let tick = world.tick;
    if let Some(cd) = world.codex.combat_deaths.iter().find(|d| d.tick == tick) {
        let ev = CodexEvent {
            event_type: EventType::Predation,
            tick,
            species_id: cd.attacker_species,
            value: 1.0,
            loc_x: cd.loc_x,
            loc_y: cd.loc_y,
        };
        world.codex.push_event(ev);
        world.codex.predation_emitted = true;
    }
}

/// CombatRaid: prune the combat-death window, then edge-trigger when the count
/// reaches COMBAT_RAID_THRESHOLD. Re-arms when it drops back below threshold.
pub(super) fn detect_combat_raid(world: &mut World) {
    let tick = world.tick;
    let cutoff = tick.saturating_sub(COMBAT_RAID_WINDOW);
    while let Some(front) = world.codex.combat_deaths.front() {
        if front.tick < cutoff {
            world.codex.combat_deaths.pop_front();
        } else {
            break;
        }
    }
    let count = world.codex.combat_deaths.len();
    let raiding = count >= COMBAT_RAID_THRESHOLD;
    if raiding && !world.codex.raid_active {
        let last = world.codex.combat_deaths.back().expect("non-empty when raiding");
        let ev = CodexEvent {
            event_type: EventType::CombatRaid,
            tick,
            species_id: last.attacker_species,
            value: count as f32,
            loc_x: last.loc_x,
            loc_y: last.loc_y,
        };
        world.codex.push_event(ev);
        world.codex.raid_active = true;
    } else if !raiding {
        world.codex.raid_active = false;
    }
}
