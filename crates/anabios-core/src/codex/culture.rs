//! Codex culture detectors (split from the former monolithic codex.rs).

use super::*;
use crate::world::World;
use std::collections::{BTreeMap, BTreeSet};

/// DialectFormed: a Communicator-bearing species whose west/east spatial halves
/// maintain divergent meme vectors (L2 ≥ DIALECT_DIVERGENCE_MIN) for a full
/// DIALECT_WINDOW consecutive ticks. Edge-triggered per species; re-arms when
/// divergence drops (clears the buffer).
pub(super) fn detect_dialect_formed(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Gather per-species member indices; tag species that have a Communicator member.
    let mut members: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut has_comm: BTreeSet<u32> = BTreeSet::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        members.entry(sid).or_default().push(i);
        if crate::module::has(&world.agents.modules[i], crate::module::ModuleType::Communicator) {
            has_comm.insert(sid);
        }
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, idxs) in members.iter() {
        // Only Communicator-bearing species.
        if !has_comm.contains(sid) {
            world.codex.dialect_divergence.remove(sid);
            world.codex.dialect_active.remove(sid);
            continue;
        }
        // Species centroid x.
        let (cx, _cy) = centroid_of(centroids, *sid);
        // Split into west (pos.x < cx) and east (pos.x >= cx).
        let mut west: Vec<usize> = Vec::new();
        let mut east: Vec<usize> = Vec::new();
        for &i in idxs {
            if world.agents.position[i].x < cx {
                west.push(i);
            } else {
                east.push(i);
            }
        }
        // Require each half to have at least DIALECT_MIN_HALF members.
        if (west.len() as u32) < DIALECT_MIN_HALF || (east.len() as u32) < DIALECT_MIN_HALF {
            world.codex.dialect_divergence.remove(sid);
            world.codex.dialect_active.remove(sid);
            continue;
        }
        // Compute per-channel mean meme for each half.
        let mut west_mean = [0.0f32; MEME_CHANNELS];
        let mut east_mean = [0.0f32; MEME_CHANNELS];
        let wn = west.len() as f32;
        let en = east.len() as f32;
        for &i in &west {
            for (ch, w) in west_mean.iter_mut().enumerate() {
                *w += world.agents.meme_vector[i][ch];
            }
        }
        for w in west_mean.iter_mut() {
            *w /= wn;
        }
        for &i in &east {
            for (ch, e) in east_mean.iter_mut().enumerate() {
                *e += world.agents.meme_vector[i][ch];
            }
        }
        for e in east_mean.iter_mut() {
            *e /= en;
        }
        let div = meme_l2(&west_mean, &east_mean);
        // Bounded window push.
        let buf = world.codex.dialect_divergence.entry(*sid).or_default();
        if buf.len() == DIALECT_WINDOW {
            buf.pop_front();
        }
        buf.push_back(div);
        let full_and_diverged =
            buf.len() == DIALECT_WINDOW && buf.iter().all(|&d| d >= DIALECT_DIVERGENCE_MIN);
        if full_and_diverged && !world.codex.dialect_active.contains(sid) {
            let (lx, ly) = centroid_of(centroids, *sid);
            to_push.push(CodexEvent {
                event_type: EventType::DialectFormed,
                tick,
                species_id: *sid,
                value: div,
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.dialect_active.insert(*sid);
        } else if !full_and_diverged {
            world.codex.dialect_active.remove(sid);
            if div < DIALECT_DIVERGENCE_MIN {
                // Clear buffer so a new window starts fresh.
                world.codex.dialect_divergence.remove(sid);
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// MemeSweep: for each Communicator species with ≥ MEME_SWEEP_MIN_MEMBERS,
/// track per-channel mean meme values over a MEME_SWEEP_WINDOW window. Fire
/// once per (species, channel) when the front of the window was ≤ MEME_SWEEP_LOW
/// and the back is ≥ MEME_SWEEP_HIGH (a sweep from rare to dominant). Re-arms
/// when the back drops below MEME_SWEEP_LOW again.
pub(super) fn detect_meme_sweep(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Per-species meme sums for each channel; track species with Communicator members.
    let mut meme_sums: BTreeMap<u32, ([f64; MEME_CHANNELS], u32)> = BTreeMap::new();
    let mut has_comm: BTreeSet<u32> = BTreeSet::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        if crate::module::has(&world.agents.modules[i], crate::module::ModuleType::Communicator) {
            has_comm.insert(sid);
        }
        let entry = meme_sums.entry(sid).or_insert(([0.0f64; MEME_CHANNELS], 0));
        for ch in 0..MEME_CHANNELS {
            entry.0[ch] += world.agents.meme_vector[i][ch] as f64;
        }
        entry.1 += 1;
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, (sums, n)) in meme_sums.iter() {
        // Must have Communicator and enough members.
        if !has_comm.contains(sid) || *n < MEME_SWEEP_MIN_MEMBERS {
            continue;
        }
        let (lx, ly) = centroid_of(centroids, *sid);
        let nf = *n as f64;
        for (ch, &s) in sums.iter().enumerate() {
            let mean = (s / nf) as f32;
            let key = (*sid, ch as u8);
            let buf = world.codex.meme_mean_history.entry(key).or_default();
            if buf.len() == MEME_SWEEP_WINDOW {
                buf.pop_front();
            }
            buf.push_back(mean);
            let back = *buf.back().unwrap();
            // Re-arm: when back drops below MEME_SWEEP_LOW the latch is released.
            if back < MEME_SWEEP_LOW {
                world.codex.meme_sweep_active.remove(&key);
            }
            if buf.len() == MEME_SWEEP_WINDOW && !world.codex.meme_sweep_active.contains(&key) {
                let front = *buf.front().unwrap();
                if front <= MEME_SWEEP_LOW && back >= MEME_SWEEP_HIGH {
                    to_push.push(CodexEvent {
                        event_type: EventType::MemeSweep,
                        tick,
                        species_id: *sid,
                        value: back,
                        loc_x: lx,
                        loc_y: ly,
                    });
                    world.codex.meme_sweep_active.insert(key);
                }
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// AlarmCall: fires once (latched) when alarm-channel broadcasts co-occur
/// with nearby same-species agents fleeing a threat. Accumulates a cumulative
/// count of alarm→flee co-occurrences; emits when it reaches ALARM_MIN_RESPONSES.
pub(super) fn detect_alarm_call(world: &mut World) {
    if world.codex.alarm_emitted {
        return;
    }
    // The detector reads this tick's actions/sensors/movement scratch. In a real
    // tick these are always sized (step() calls resize_scratch first); guard so
    // a standalone observe_all (e.g. detector unit tests) is a safe no-op.
    let cap = world.agents.capacity();
    if world.actions.len() < cap || world.sensors.len() < cap || world.desired_direction.len() < cap
    {
        return;
    }
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    let tick = world.tick;
    let mut first_caller_species: Option<u32> = None;
    let mut first_caller_pos: (f32, f32) = (0.0, 0.0);
    for &id in &alive_ids {
        let i = id as usize;
        // Only Communicator agents broadcasting on the alarm channel.
        if !crate::module::has(&world.agents.modules[i], crate::module::ModuleType::Communicator) {
            continue;
        }
        if world.actions[i].broadcast_intent[ALARM_MEME_CHANNEL] <= MEME_BROADCAST_THRESHOLD {
            continue;
        }
        let range = crate::module::effective_communicator_range(&world.agents.modules[i])
            .min(PERCEPTION_MAX_RADIUS);
        if range <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let caller_species = world.agents.species_id[i];
        let mut responses_this_caller: u32 = 0;
        world.spatial.query(pos, range, |oid| {
            if oid == id {
                return;
            }
            let j = oid as usize;
            if world.agents.species_id[j] != caller_species {
                return;
            }
            let nearest_other_dist = world.sensors[j].nearest_other_dist;
            if !nearest_other_dist.is_finite() {
                return;
            }
            let dd = world.desired_direction[j];
            let threat_dir = world.sensors[j].nearest_other_dir;
            if dd.dot(threat_dir) < 0.0 {
                responses_this_caller += 1;
            }
        });
        world.codex.alarm_responses += responses_this_caller;
        // Record the first alarm broadcaster this tick as the event's location,
        // regardless of whether it drew responses — so the payload never
        // defaults to (0,0) when the threshold tips on a zero-response caller.
        if first_caller_species.is_none() {
            first_caller_species = Some(caller_species);
            first_caller_pos = (pos.x, pos.y);
        }
        if world.codex.alarm_responses >= ALARM_MIN_RESPONSES {
            let (lx, ly) = first_caller_pos;
            let sid = first_caller_species.unwrap_or(caller_species);
            world.codex.push_event(CodexEvent {
                event_type: EventType::AlarmCall,
                tick,
                species_id: sid,
                value: world.codex.alarm_responses as f32,
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.alarm_emitted = true;
            return;
        }
    }
}

/// EvolvedCooperation: prune share_events to the COOPERATION_WINDOW, tally per
/// species, and edge-trigger (per species) when a species reaches
/// COOPERATION_MIN_SHARES. Re-arms when the count drops below threshold.
pub(super) fn detect_evolved_cooperation(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Prune entries older than the rolling window (mirror detect_combat_raid).
    let cutoff = tick.saturating_sub(COOPERATION_WINDOW);
    while let Some(&(t, _)) = world.codex.share_events.front() {
        if t < cutoff {
            world.codex.share_events.pop_front();
        } else {
            break;
        }
    }
    // Tally shares per species (BTreeMap → deterministic).
    let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
    for &(_t, sid) in world.codex.share_events.iter() {
        *counts.entry(sid).or_insert(0) += 1;
    }
    // Edge-trigger per species; re-arm when the count drops.
    let mut to_push: Vec<CodexEvent> = Vec::new();
    // Collect species to re-arm (drop latch) — those in cooperation_active but below threshold.
    let mut to_rearm: Vec<u32> = Vec::new();
    for &sid in world.codex.cooperation_active.iter() {
        let count = counts.get(&sid).copied().unwrap_or(0);
        if count < COOPERATION_MIN_SHARES {
            to_rearm.push(sid);
        }
    }
    for sid in to_rearm {
        world.codex.cooperation_active.remove(&sid);
    }
    // Fire for species above threshold not yet latched.
    for (sid, count) in counts.iter() {
        if *count >= COOPERATION_MIN_SHARES && !world.codex.cooperation_active.contains(sid) {
            let (lx, ly) = centroid_of(centroids, *sid);
            to_push.push(CodexEvent {
                event_type: EventType::EvolvedCooperation,
                tick,
                species_id: *sid,
                value: *count as f32,
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.cooperation_active.insert(*sid);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}
