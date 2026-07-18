//! Codex population detectors (split from the former monolithic codex.rs).

use super::*;
use crate::world::World;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn update_pop_history(world: &mut World) {
    let counts: Vec<u32> = world.species_member_counts.clone();
    for (sid, count) in counts.into_iter().enumerate() {
        let buf = world.codex.pop_history.entry(sid as u32).or_default();
        if buf.len() == POP_HISTORY_WINDOW {
            buf.pop_front();
        }
        buf.push_back(count);
    }
}

pub(super) fn detect_extinction(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, buf) in world.codex.pop_history.iter() {
        if buf.len() < 2 {
            continue;
        }
        let prev = buf[buf.len() - 2];
        let cur = buf[buf.len() - 1];
        if prev > 0 && cur == 0 {
            let (lx, ly) = centroid_of(centroids, *sid);
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

pub(super) fn detect_population_crash(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
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
            let (lx, ly) = centroid_of(centroids, *sid);
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

pub(super) fn detect_migration(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Push current centroids into history.
    for (sid, c) in centroids.iter() {
        let buf = world.codex.centroid_history.entry(*sid).or_default();
        if buf.len() == MIGRATION_WINDOW {
            buf.pop_front();
        }
        buf.push_back(*c);
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    let mut clear: Vec<u32> = Vec::new();
    for (sid, buf) in world.codex.centroid_history.iter() {
        if buf.len() < MIGRATION_WINDOW {
            continue;
        }
        let first = *buf.front().unwrap();
        let last = *buf.back().unwrap();
        let d = torus_distance(glam::Vec2::new(first.0, first.1), glam::Vec2::new(last.0, last.1));
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

pub(super) fn detect_novel_modules(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    // Current module types present per species this tick.
    let mut current: BTreeMap<u32, BTreeSet<u8>> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        let set = current.entry(sid).or_default();
        for m in world.agents.modules[i].iter() {
            set.insert(m.module_type() as u8);
        }
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, types) in current {
        match world.codex.seen_modules.get_mut(&sid) {
            None => {
                // Debut: seed silently.
                world.codex.seen_modules.insert(sid, types);
            }
            Some(seen) => {
                for t in types {
                    if seen.insert(t) {
                        let (lx, ly) = centroid_of(centroids, sid);
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

pub(super) fn detect_novel_behavior(world: &mut World, centroids: &BTreeMap<u32, (f32, f32)>) {
    let tick = world.tick;
    let mut current: BTreeMap<u32, BTreeSet<u8>> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        let set = current.entry(sid).or_default();
        for node in world.agents.program[i].nodes.iter().copied() {
            set.insert(crate::program::Program::node_kind(node));
        }
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, kinds) in current {
        match world.codex.seen_node_kinds.get_mut(&sid) {
            None => {
                world.codex.seen_node_kinds.insert(sid, kinds);
            }
            Some(seen) => {
                for k in kinds {
                    if seen.insert(k) {
                        let (lx, ly) = centroid_of(centroids, sid);
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
