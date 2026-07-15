//! Codex spatial detectors (split from the former monolithic codex.rs).

use super::*;
use crate::world::World;
use std::collections::{BTreeMap, BTreeSet};

/// TerritoryFormation: a pheromone-marking species that stays clustered (spread
/// ≤ TERRITORY_SPREAD_MAX) for TERRITORY_WINDOW consecutive ticks. Edge-
/// triggered per species; re-arms when the cluster disperses.
pub(super) fn detect_territory_formation(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Gather per-species member positions and whether the species marks (has a
    // Pheromone module member). BTreeMap → deterministic.
    let mut positions: BTreeMap<u32, Vec<glam::Vec2>> = BTreeMap::new();
    let mut marks: BTreeSet<u32> = BTreeSet::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        positions.entry(sid).or_default().push(world.agents.position[i]);
        if crate::module::has(&world.agents.modules[i], crate::module::ModuleType::Pheromone) {
            marks.insert(sid);
        }
    }
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, ps) in positions.iter() {
        if (ps.len() as u32) < TERRITORY_MIN_MEMBERS || !marks.contains(sid) {
            world.codex.territory_spread.remove(sid);
            world.codex.territory_active.remove(sid);
            continue;
        }
        let spread = species_spread(ps);
        let buf = world.codex.territory_spread.entry(*sid).or_default();
        if buf.len() == TERRITORY_WINDOW {
            buf.pop_front();
        }
        buf.push_back(spread);
        let clustered =
            buf.len() == TERRITORY_WINDOW && buf.iter().all(|&s| s <= TERRITORY_SPREAD_MAX);
        if clustered && !world.codex.territory_active.contains(sid) {
            let (lx, ly) = centroid_of(centroids, *sid);
            to_push.push(CodexEvent {
                event_type: EventType::TerritoryFormation,
                tick,
                species_id: *sid,
                value: *buf.back().unwrap(),
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.territory_active.insert(*sid);
        } else if !clustered {
            world.codex.territory_active.remove(sid);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// NichePartitioning: two ≥NICHE_MIN_MEMBERS species whose terrain-type
/// distributions overlap ≤ NICHE_OVERLAP_MAX for NICHE_WINDOW consecutive ticks.
pub(super) fn detect_niche_partitioning(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Per-species normalized terrain histogram (terrain discriminant → fraction).
    let mut counts: BTreeMap<u32, BTreeMap<u8, f32>> = BTreeMap::new();
    let mut totals: BTreeMap<u32, u32> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        let (col, row) = crate::biome::BiomeField::cell_coords(world.agents.position[i]);
        let terrain = world.biome.at(col, row).terrain as u8;
        *counts.entry(sid).or_default().entry(terrain).or_insert(0.0) += 1.0;
        *totals.entry(sid).or_insert(0) += 1;
    }
    // Normalize and keep only species with enough members.
    let mut hist: BTreeMap<u32, BTreeMap<u8, f32>> = BTreeMap::new();
    for (sid, h) in counts.into_iter() {
        let n = *totals.get(&sid).unwrap_or(&0);
        if n < NICHE_MIN_MEMBERS {
            continue;
        }
        let nf = n as f32;
        hist.insert(sid, h.into_iter().map(|(t, c)| (t, c / nf)).collect());
    }
    let sids: Vec<u32> = hist.keys().copied().collect();
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for ai in 0..sids.len() {
        for bi in (ai + 1)..sids.len() {
            let (a, b) = (sids[ai], sids[bi]); // a < b (ascending keys)
            let overlap = histogram_overlap(&hist[&a], &hist[&b]);
            let key = (a, b);
            if overlap <= NICHE_OVERLAP_MAX {
                let s = world.codex.niche_streak.entry(key).or_insert(0);
                *s += 1;
                if *s >= NICHE_WINDOW && !world.codex.niche_active.contains(&key) {
                    let (lx, ly) = centroid_of(centroids, a);
                    to_push.push(CodexEvent {
                        event_type: EventType::NichePartitioning,
                        tick,
                        species_id: a,
                        value: overlap,
                        loc_x: lx,
                        loc_y: ly,
                    });
                    world.codex.niche_active.insert(key);
                }
            } else {
                world.codex.niche_streak.remove(&key);
                world.codex.niche_active.remove(&key);
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// HerdCohesion: a species with ≥ HERD_MIN_MEMBERS sustains mean per-member
/// same-species crowding ≥ HERD_CROWDING_MIN for a full HERD_WINDOW of
/// consecutive ticks. Edge-triggered per species; re-arms when cohesion drops.
pub(super) fn detect_herd_cohesion(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    // Guard: sensors scratch is only sized after resize_scratch (like detect_alarm_call).
    if world.sensors.len() < world.agents.capacity() {
        return;
    }
    let tick = world.tick;
    // Gather per-species member count and summed crowding (BTreeMap → deterministic).
    let mut member_count: BTreeMap<u32, u32> = BTreeMap::new();
    let mut crowding_sum: BTreeMap<u32, f64> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        *member_count.entry(sid).or_insert(0) += 1;
        *crowding_sum.entry(sid).or_insert(0.0) += world.sensors[i].crowding as f64;
    }
    // Collect all species ids present this tick.
    let all_sids: Vec<u32> = member_count.keys().copied().collect();
    // Prune entries for species that have gone below threshold or disappeared.
    for &sid in &all_sids {
        if member_count.get(&sid).copied().unwrap_or(0) < HERD_MIN_MEMBERS {
            world.codex.herd_crowding.remove(&sid);
            world.codex.herd_active.remove(&sid);
        }
    }
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in &all_sids {
        let n = member_count[&sid];
        if n < HERD_MIN_MEMBERS {
            continue;
        }
        let mean = (crowding_sum[&sid] / n as f64) as f32;
        let buf = world.codex.herd_crowding.entry(sid).or_default();
        if buf.len() == HERD_WINDOW {
            buf.pop_front();
        }
        buf.push_back(mean);
        let cohesive = buf.len() == HERD_WINDOW && buf.iter().all(|&c| c >= HERD_CROWDING_MIN);
        if cohesive && !world.codex.herd_active.contains(&sid) {
            let (lx, ly) = centroid_of(centroids, sid);
            to_push.push(CodexEvent {
                event_type: EventType::HerdCohesion,
                tick,
                species_id: sid,
                value: mean,
                loc_x: lx,
                loc_y: ly,
            });
            world.codex.herd_active.insert(sid);
        } else if !cohesive {
            world.codex.herd_active.remove(&sid);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}
