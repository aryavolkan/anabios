//! Codex settlement & economy detectors (E8): SettlementFormed,
//! MarketEmerged, SpecializationSplit.

use super::*;

/// Settlement: per-species anchor cohesion streak. Anchors decouple the
/// settlement from day-to-day wandering — it is the place they return to.
pub(super) fn detect_settlement(world: &mut World, agg: &SpeciesAggTable) {
    if !world.settlement_enabled {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        let (spread, centroid) = if e.count >= SETTLEMENT_MIN_MEMBERS {
            let anchors: Vec<glam::Vec2> =
                e.member_idx.iter().map(|&i| world.agents.anchor[i]).collect();
            let spread = species_spread(&anchors, world.world_size);
            // Anchor centroid (plain mean — spreads are small by definition).
            let n = anchors.len().max(1) as f32;
            let cx = anchors.iter().map(|a| a.x).sum::<f32>() / n;
            let cy = anchors.iter().map(|a| a.y).sum::<f32>() / n;
            (spread, (cx, cy))
        } else {
            (f32::INFINITY, (0.0, 0.0))
        };
        let cohesive = spread <= SETTLEMENT_SPREAD_MAX;
        let streak = world.codex.settlement_streak.entry(sid).or_insert(0);
        if cohesive {
            *streak += 1;
        } else {
            *streak = 0;
        }
        let fired = *streak >= SETTLEMENT_WINDOW;
        if let Some(ev) =
            edge_trigger_species(&mut world.codex.settlement_active, sid, fired, || CodexEvent {
                event_type: EventType::SettlementFormed,
                tick,
                species_id: sid,
                value: spread,
                loc_x: centroid.0,
                loc_y: centroid.1,
            })
        {
            to_push.push(ev);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Market: per-cell density streak. Latch per cell, re-arm below half.
pub(super) fn detect_market(world: &mut World) {
    if !world.resources_enabled || world.market_field.is_empty() {
        return;
    }
    let tick = world.tick;
    let res = world.biome.res;
    let cs = world.biome.cell_size;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    let mut remove: Vec<u32> = Vec::new();
    for (idx, &density) in world.market_field.iter().enumerate() {
        let idx = idx as u32;
        if density >= MARKET_NODE_THRESHOLD {
            let streak = world.codex.market_streak.entry(idx).or_insert(0);
            *streak += 1;
            if *streak >= MARKET_WINDOW && world.codex.market_active.insert(idx) {
                let (col, row) = (idx as usize % res, idx as usize / res);
                to_push.push(CodexEvent {
                    event_type: EventType::MarketEmerged,
                    tick,
                    species_id: 0,
                    value: density,
                    loc_x: (col as f32 + 0.5) * cs,
                    loc_y: (row as f32 + 0.5) * cs,
                });
            }
        } else {
            if density < MARKET_NODE_THRESHOLD * 0.5 && world.codex.market_active.contains(&idx) {
                remove.push(idx);
            }
            world.codex.market_streak.remove(&idx);
        }
    }
    for idx in remove {
        world.codex.market_active.remove(&idx);
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Specialization split: within one species, two distinct goods each with a
/// ≥20% producer class whose experience share in that good is ≥60%.
pub(super) fn detect_specialization(world: &mut World, agg: &SpeciesAggTable) {
    if !world.resources_enabled {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        if world.codex.specialization_active.contains(&sid) {
            continue;
        }
        let e = agg.get(sid).expect("active species has an entry");
        if e.count < 10 {
            continue;
        }
        // Count producer classes per good.
        let mut class_counts = [0_u32; crate::resource::GOOD_COUNT];
        for &i in &e.member_idx {
            let exp = &world.agents.harvest_exp[i];
            let total: f32 = exp.iter().sum();
            if total <= 0.0 {
                continue;
            }
            for (k, &v) in exp.iter().enumerate() {
                if v / total >= SPECIALIST_SHARE {
                    class_counts[k] += 1;
                    break; // one dominant good per agent
                }
            }
        }
        let n = e.count as f32;
        let classes: Vec<f32> = class_counts
            .iter()
            .filter(|&&c| c as f32 >= SPECIALIZATION_MIN_CLASS * n)
            .map(|&c| c as f32 / n)
            .collect();
        if classes.len() >= 2 {
            world.codex.specialization_active.insert(sid);
            let smallest = classes.iter().copied().fold(f32::INFINITY, f32::min);
            let (lx, ly) = centroid_of(agg, sid);
            to_push.push(CodexEvent {
                event_type: EventType::SpecializationSplit,
                tick,
                species_id: sid,
                value: smallest,
                loc_x: lx,
                loc_y: ly,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;

    fn world_with_agents(n: u32, pos: Vec2) -> World {
        let mut w = World::new(1);
        for k in 0..n {
            let _ = w.spawn_agent(pos + Vec2::new(k as f32 * 0.1, 0.0), Genome::neutral());
        }
        w
    }

    fn agg_for(w: &World, sid: u32) -> SpeciesAggTable {
        let mut agg = SpeciesAggTable::default();
        let mut e = SpeciesAgg::default();
        for id in w.agents.iter_alive() {
            let i = id as usize;
            if w.agents.species_id[i] == sid {
                e.count += 1;
                e.member_idx.push(i);
                e.sum_x += w.agents.position[i].x as f64;
                e.sum_y += w.agents.position[i].y as f64;
            }
        }
        if sid as usize >= agg.entries.len() {
            agg.entries.resize(sid as usize + 1, SpeciesAgg::default());
        }
        agg.entries[sid as usize] = e;
        agg.active.push(sid);
        agg
    }

    #[test]
    fn anchored_cluster_forms_a_settlement() {
        let mut w = world_with_agents(12, Vec2::new(500.0, 500.0));
        w.settlement_enabled = true;
        let agg = agg_for(&w, 0);
        for _ in 0..SETTLEMENT_WINDOW {
            detect_settlement(&mut w, &agg);
        }
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::SettlementFormed));
    }

    #[test]
    fn dispersed_anchors_do_not_settle() {
        let mut w = world_with_agents(12, Vec2::new(500.0, 500.0));
        w.settlement_enabled = true;
        // Scatter anchors across the world.
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        for id in ids {
            let i = id as usize;
            w.agents.anchor[i] = Vec2::new((i * 83) as f32 % 1024.0, (i * 47) as f32 % 1024.0);
        }
        let agg = agg_for(&w, 0);
        for _ in 0..SETTLEMENT_WINDOW {
            detect_settlement(&mut w, &agg);
        }
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn dense_cell_becomes_a_market() {
        let mut w = world_with_agents(1, Vec2::new(500.0, 500.0));
        w.resources_enabled = true;
        w.market_field = vec![0.0; w.biome.cells.len()];
        w.market_field[42] = MARKET_NODE_THRESHOLD;
        for _ in 0..MARKET_WINDOW {
            detect_market(&mut w);
        }
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::MarketEmerged));
        // Latched.
        for _ in 0..10 {
            detect_market(&mut w);
        }
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::MarketEmerged).count(),
            1
        );
    }

    #[test]
    fn thin_cell_is_no_market() {
        let mut w = world_with_agents(1, Vec2::new(500.0, 500.0));
        w.resources_enabled = true;
        w.market_field = vec![0.0; w.biome.cells.len()];
        w.market_field[42] = MARKET_NODE_THRESHOLD * 0.5;
        for _ in 0..MARKET_WINDOW {
            detect_market(&mut w);
        }
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn bimodal_producers_split_uniform_does_not() {
        let mut w = world_with_agents(20, Vec2::new(500.0, 500.0));
        w.resources_enabled = true;
        // 5 agents 100% Salt, 5 agents 100% Obsidian, 10 generalists.
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        for id in ids {
            let i = id as usize;
            if i < 5 {
                w.agents.harvest_exp[i][0] = 10.0;
            } else if i < 10 {
                w.agents.harvest_exp[i][1] = 10.0;
            } else {
                w.agents.harvest_exp[i] = [2.5; crate::resource::GOOD_COUNT];
            }
        }
        let agg = agg_for(&w, 0);
        detect_specialization(&mut w, &agg);
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::SpecializationSplit));

        // Uniform: everyone spread evenly — no dominant class.
        let mut w2 = world_with_agents(20, Vec2::new(500.0, 500.0));
        w2.resources_enabled = true;
        let ids: Vec<u32> = w2.agents.iter_alive().collect();
        for id in ids {
            w2.agents.harvest_exp[id as usize] = [2.5; crate::resource::GOOD_COUNT];
        }
        let agg2 = agg_for(&w2, 0);
        detect_specialization(&mut w2, &agg2);
        assert!(w2.codex.events.is_empty());
    }
}
