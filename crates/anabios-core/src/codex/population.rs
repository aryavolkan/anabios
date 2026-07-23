//! Codex population detectors (split from the former monolithic codex.rs).

use super::*;
use crate::world::World;
use std::collections::BTreeSet;

pub(super) fn update_pop_history(world: &mut World) {
    for sid in 0..world.species_member_counts.len() {
        let count = world.species_member_counts[sid];
        let buf = world.codex.pop_history.entry(sid as u32).or_default();
        if buf.len() == POP_HISTORY_WINDOW {
            buf.pop_front();
        }
        buf.push_back(count);
    }
}

pub(super) fn detect_extinction(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, buf) in world.codex.pop_history.iter() {
        if buf.len() < 2 {
            continue;
        }
        let prev = buf[buf.len() - 2];
        let cur = buf[buf.len() - 1];
        if prev > 0 && cur == 0 {
            let (lx, ly) = centroid_of(agg, *sid);
            to_push.push(CodexEvent {
                event_type: EventType::Extinction,
                tick,
                species_id: *sid,
                value: prev as f32,
                loc_x: lx,
                loc_y: ly,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

pub(super) fn detect_population_crash(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, buf) in world.codex.pop_history.iter() {
        if buf.len() < POP_HISTORY_WINDOW {
            continue;
        }
        let peak = *buf.iter().max().unwrap_or(&0);
        let cur = *buf.back().unwrap_or(&0);
        if peak == 0 || cur == 0 {
            continue;
        }
        let drop = 1.0 - (cur as f32 / peak as f32);
        if drop >= CRASH_FRACTION {
            let (lx, ly) = centroid_of(agg, *sid);
            to_push.push(CodexEvent {
                event_type: EventType::PopulationCrash,
                tick,
                species_id: *sid,
                value: drop,
                loc_x: lx,
                loc_y: ly,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

pub(super) fn detect_migration(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    // Push current centroids into history.
    for &sid in agg.active() {
        let c = agg.get(sid).expect("active species has an entry").centroid();
        let buf = world.codex.centroid_history.entry(sid).or_default();
        if buf.len() == MIGRATION_WINDOW {
            buf.pop_front();
        }
        buf.push_back(c);
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    let mut clear: Vec<u32> = Vec::new();
    for (sid, buf) in world.codex.centroid_history.iter() {
        if buf.len() < MIGRATION_WINDOW {
            continue;
        }
        let first = *buf.front().unwrap();
        let last = *buf.back().unwrap();
        let d = torus_distance(
            glam::Vec2::new(first.0, first.1),
            glam::Vec2::new(last.0, last.1),
            world.world_size,
        );
        if d >= MIGRATION_DISTANCE {
            to_push.push(CodexEvent {
                event_type: EventType::Migration,
                tick,
                species_id: *sid,
                value: d,
                loc_x: last.0,
                loc_y: last.1,
            });
            clear.push(*sid);
            // Feed the corridor detector: shortest-path torus direction of
            // the displacement, normalized.
            let ws = world.world_size;
            let mut dx = last.0 - first.0;
            let mut dy = last.1 - first.1;
            if dx > ws * 0.5 {
                dx -= ws;
            } else if dx < -ws * 0.5 {
                dx += ws;
            }
            if dy > ws * 0.5 {
                dy -= ws;
            } else if dy < -ws * 0.5 {
                dy += ws;
            }
            let len = (dx * dx + dy * dy).sqrt().max(1e-6);
            // Corridor leg check: sample along the unwrapped displacement
            // and count barrier-terrain (water/rock) cells crossed.
            let mut barrier_hits = 0_u32;
            for s in 1..CORRIDOR_BARRIER_SAMPLES {
                let t = s as f32 / CORRIDOR_BARRIER_SAMPLES as f32;
                let px = (first.0 + dx * t).rem_euclid(world.world_size);
                let py = (first.1 + dy * t).rem_euclid(world.world_size);
                let (bc, br) = world.biome.cell_coords(glam::Vec2::new(px, py));
                match world.biome.at(bc, br).terrain {
                    crate::biome::TerrainType::Water | crate::biome::TerrainType::Rock => {
                        barrier_hits += 1;
                    }
                    _ => {}
                }
            }
            let dirs = world.codex.migration_dirs.entry(*sid).or_default();
            if dirs.len() == CORRIDOR_DIR_CAP {
                dirs.pop_front();
            }
            dirs.push_back((tick, dx / len, dy / len, barrier_hits));
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
    // Reset history for migrated species so they don't re-fire each tick.
    for sid in clear {
        world.codex.centroid_history.remove(&sid);
    }
}

pub(super) fn detect_novel_modules(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let mask = agg.get(sid).expect("active species has an entry").module_mask;
        match world.codex.seen_modules.get_mut(&sid) {
            None => {
                // Debut: seed silently.
                world.codex.seen_modules.insert(sid, bits_to_set_u16(mask));
            }
            Some(seen) => {
                let mut m = mask;
                while m != 0 {
                    let t = m.trailing_zeros() as u8;
                    m &= m - 1;
                    if seen.insert(t) {
                        let (lx, ly) = centroid_of(agg, sid);
                        to_push.push(CodexEvent {
                            event_type: EventType::NovelModuleAppeared,
                            tick,
                            species_id: sid,
                            value: t as f32,
                            loc_x: lx,
                            loc_y: ly,
                        });
                    }
                }
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

pub(super) fn detect_novel_behavior(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let mask = agg.get(sid).expect("active species has an entry").node_mask;
        match world.codex.seen_node_kinds.get_mut(&sid) {
            None => {
                world.codex.seen_node_kinds.insert(sid, bits_to_set_u64(mask));
            }
            Some(seen) => {
                let mut m = mask;
                while m != 0 {
                    let k = m.trailing_zeros() as u8;
                    m &= m - 1;
                    if seen.insert(k) {
                        let (lx, ly) = centroid_of(agg, sid);
                        to_push.push(CodexEvent {
                            event_type: EventType::NovelBehaviorPattern,
                            tick,
                            species_id: sid,
                            value: k as f32,
                            loc_x: lx,
                            loc_y: ly,
                        });
                    }
                }
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Set bits of a u16 mask as a BTreeSet of discriminants (ascending).
fn bits_to_set_u16(mask: u16) -> BTreeSet<u8> {
    let mut out = BTreeSet::new();
    let mut m = mask;
    while m != 0 {
        out.insert(m.trailing_zeros() as u8);
        m &= m - 1;
    }
    out
}

/// Set bits of a u64 mask as a BTreeSet of discriminants (ascending).
fn bits_to_set_u64(mask: u64) -> BTreeSet<u8> {
    let mut out = BTreeSet::new();
    let mut m = mask;
    while m != 0 {
        out.insert(m.trailing_zeros() as u8);
        m &= m - 1;
    }
    out
}
