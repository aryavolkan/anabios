//! Codex spatial detectors (split from the former monolithic codex.rs).

use super::*;
use crate::world::World;

/// TerritoryFormation: a pheromone-marking species that stays clustered (spread
/// ≤ TERRITORY_SPREAD_MAX) for TERRITORY_WINDOW consecutive ticks. Edge-
/// triggered per species; re-arms when the cluster disperses.
pub(super) fn detect_territory_formation(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        if entry.count < TERRITORY_MIN_MEMBERS || !entry.has_pheromone {
            world.codex.territory_spread.remove(&sid);
            world.codex.territory_active.remove(&sid);
            continue;
        }
        let spread = species_spread_indexed(&world.agents.position, &entry.member_idx);
        let buf = world.codex.territory_spread.entry(sid).or_default();
        if buf.len() == TERRITORY_WINDOW {
            buf.pop_front();
        }
        buf.push_back(spread);
        let clustered =
            buf.len() == TERRITORY_WINDOW && buf.iter().all(|&s| s <= TERRITORY_SPREAD_MAX);
        if let Some(ev) =
            edge_trigger_species(&mut world.codex.territory_active, sid, clustered, || {
                let (lx, ly) = centroid_of(agg, sid);
                CodexEvent {
                    event_type: EventType::TerritoryFormation,
                    tick,
                    species_id: sid,
                    value: *buf.back().unwrap(),
                    loc_x: lx,
                    loc_y: ly,
                }
            })
        {
            to_push.push(ev);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// RMS spread over the positions of the given member indices. Identical
/// summation order to `species_spread` over the equivalent compacted slice.
fn species_spread_indexed(positions: &[glam::Vec2], idx: &[usize]) -> f32 {
    if idx.len() < 2 {
        return 0.0;
    }
    let n = idx.len() as f32;
    let mut cx = 0.0f64;
    let mut cy = 0.0f64;
    for &i in idx {
        cx += positions[i].x as f64;
        cy += positions[i].y as f64;
    }
    let centroid = glam::Vec2::new((cx / n as f64) as f32, (cy / n as f64) as f32);
    let mut sumsq = 0.0f64;
    for &i in idx {
        let d = crate::spatial::torus_distance(positions[i], centroid);
        sumsq += (d as f64) * (d as f64);
    }
    ((sumsq / n as f64).sqrt()) as f32
}

/// NichePartitioning: two ≥NICHE_MIN_MEMBERS species whose terrain-type
/// distributions overlap ≤ NICHE_OVERLAP_MAX for NICHE_WINDOW consecutive ticks.
pub(super) fn detect_niche_partitioning(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    // Normalized terrain histograms for species with enough members.
    let mut hist: BTreeMap<u32, [f32; TERRAIN_SLOTS]> = BTreeMap::new();
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        if entry.count < NICHE_MIN_MEMBERS {
            continue;
        }
        let nf = entry.count as f32;
        let mut h = [0.0f32; TERRAIN_SLOTS];
        for (t, v) in h.iter_mut().enumerate() {
            *v = entry.terrain_counts[t] / nf;
        }
        hist.insert(sid, h);
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
                    let (lx, ly) = centroid_of(agg, a);
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
pub(super) fn detect_herd_cohesion(world: &mut World, agg: &SpeciesAggTable) {
    // Guard: sensors scratch is only sized after resize_scratch (like detect_alarm_call).
    if world.sensors.len() < world.agents.capacity() {
        return;
    }
    let tick = world.tick;
    // Prune entries for species that have gone below threshold or disappeared.
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        if entry.count < HERD_MIN_MEMBERS {
            world.codex.herd_crowding.remove(&sid);
            world.codex.herd_active.remove(&sid);
        }
    }
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        if entry.count < HERD_MIN_MEMBERS {
            continue;
        }
        let mean = (entry.crowding_sum / entry.count as f64) as f32;
        let buf = world.codex.herd_crowding.entry(sid).or_default();
        if buf.len() == HERD_WINDOW {
            buf.pop_front();
        }
        buf.push_back(mean);
        let cohesive = buf.len() == HERD_WINDOW && buf.iter().all(|&c| c >= HERD_CROWDING_MIN);
        if let Some(ev) = edge_trigger_species(&mut world.codex.herd_active, sid, cohesive, || {
            let (lx, ly) = centroid_of(agg, sid);
            CodexEvent {
                event_type: EventType::HerdCohesion,
                tick,
                species_id: sid,
                value: mean,
                loc_x: lx,
                loc_y: ly,
            }
        }) {
            to_push.push(ev);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}
