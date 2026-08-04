//! Affect-observability detectors (M-F): FeedingFrenzy, TerritorialRage,
//! PanicCascade, MassGrief. Pure observers over the serialized `affect`
//! column + the per-tick `SpeciesAgg` aggregate; zero RNG. Each is gated on
//! `world.affect_enabled` by its caller in `observe_all` (all four functions
//! assume the flag is already known to be on).

use super::*;
use crate::spatial::torus_distance;
use crate::world::World;

/// Centroid + RMS spread (torus-aware) over an explicit position iterator.
/// Mirrors `metrics::rms_spread`'s summation order but also returns the
/// centroid (needed for the event `loc`), since several detectors here need
/// spread/centroid over an arbitrary subset of a species (not the whole
/// species, which `centroid_of`/`species_spread_indexed` already cover).
/// Returns `((0,0), 0.0)` for zero points; a single point returns itself with
/// 0.0 spread.
fn centroid_and_spread(
    positions: impl Iterator<Item = glam::Vec2> + Clone,
    count: usize,
    world_size: f32,
) -> ((f32, f32), f32) {
    if count == 0 {
        return ((0.0, 0.0), 0.0);
    }
    let n = count as f64;
    let mut cx = 0.0f64;
    let mut cy = 0.0f64;
    for p in positions.clone() {
        cx += p.x as f64;
        cy += p.y as f64;
    }
    let centroid = glam::Vec2::new((cx / n) as f32, (cy / n) as f32);
    if count < 2 {
        return ((centroid.x, centroid.y), 0.0);
    }
    let mut sumsq = 0.0f64;
    for p in positions {
        let d = torus_distance(p, centroid, world_size);
        sumsq += (d as f64) * (d as f64);
    }
    let spread = (sumsq / n).sqrt() as f32;
    ((centroid.x, centroid.y), spread)
}

/// FeedingFrenzy: a same-species cluster of `>= FRENZY_MIN_MEMBERS` high-SEEK
/// members that have converged (RMS spread `<= FRENZY_SPREAD_MAX`) on one
/// patch. Edge-triggered per species; re-arms when the cluster thins below
/// the member threshold or disperses.
pub(super) fn detect_feeding_frenzy(world: &mut World, agg: &SpeciesAggTable) {
    use crate::affect::SEEK;
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        if entry.high_seek < FRENZY_MIN_MEMBERS {
            world.codex.frenzy_active.remove(&sid);
            continue;
        }
        let idx: Vec<usize> = entry
            .member_idx
            .iter()
            .copied()
            .filter(|&i| world.agents.affect[i][SEEK] >= HIGH_SEEK)
            .collect();
        let ((lx, ly), spread) = centroid_and_spread(
            idx.iter().map(|&i| world.agents.position[i]),
            idx.len(),
            world.world_size,
        );
        let converged = spread <= FRENZY_SPREAD_MAX;
        let count = idx.len() as f32;
        if let Some(ev) =
            edge_trigger_species(&mut world.codex.frenzy_active, sid, converged, || CodexEvent {
                event_type: EventType::FeedingFrenzy,
                tick,
                species_id: sid,
                value: count,
                loc_x: lx,
                loc_y: ly,
            })
        {
            to_push.push(ev);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// TerritorialRage: a same-species cluster (`>= RAGE_MIN_MEMBERS`, RMS spread
/// `<= RAGE_CLUSTER_SPREAD_MAX` over ALL members — co-located, not just the
/// angry ones) sustains mean RAGE `>= RAGE_CLUSTER_MEAN` for a full
/// `RAGE_WINDOW` consecutive ticks. Streak-based, edge-triggered; re-arms
/// when the cluster disperses, thins, or cools off.
pub(super) fn detect_territorial_rage(world: &mut World, agg: &SpeciesAggTable) {
    use crate::affect::RAGE;
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        let mean_rage = (entry.affect_sum[RAGE] / entry.count.max(1) as f64) as f32;
        let qualifies = entry.count >= RAGE_MIN_MEMBERS && mean_rage >= RAGE_CLUSTER_MEAN && {
            let (_, spread) = centroid_and_spread(
                entry.member_idx.iter().map(|&i| world.agents.position[i]),
                entry.member_idx.len(),
                world.world_size,
            );
            spread <= RAGE_CLUSTER_SPREAD_MAX
        };
        let streak = world.codex.rage_streak.entry(sid).or_insert(0);
        if qualifies {
            *streak += 1;
        } else {
            *streak = 0;
        }
        let streak_val = *streak;
        if let Some(ev) = edge_trigger_species(
            &mut world.codex.rage_active,
            sid,
            streak_val >= RAGE_WINDOW,
            || {
                let (lx, ly) = centroid_of(agg, sid);
                CodexEvent {
                    event_type: EventType::TerritorialRage,
                    tick,
                    species_id: sid,
                    value: mean_rage,
                    loc_x: lx,
                    loc_y: ly,
                }
            },
        ) {
            to_push.push(ev);
        }
    }
    // Prune streaks/latches of species gone from the active set so the maps
    // can't grow unboundedly (mirrors domestication::detect_livestock_herd).
    let gone: Vec<u32> =
        world.codex.rage_streak.keys().filter(|sid| !agg.active().contains(sid)).copied().collect();
    for sid in gone {
        world.codex.rage_streak.remove(&sid);
        world.codex.rage_active.remove(&sid);
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// PanicCascade: contagion signal — a same-species cluster's high-FEAR member
/// count rises by `>= CASCADE_MIN_SPREAD` within a rolling `AFFECT_CASCADE_WINDOW`
/// (fear propagating through the group in a short span, not a synchronized
/// startle). Edge-triggered; once fired, stays latched while the high-FEAR
/// count remains `>= CASCADE_MIN_SPREAD`, and re-arms when it falls back below.
pub(super) fn detect_panic_cascade(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        let buf = world.codex.fear_count_history.entry(sid).or_default();
        if buf.len() == AFFECT_CASCADE_WINDOW {
            buf.pop_front();
        }
        buf.push_back(entry.high_fear);
        let rose = entry.count >= CASCADE_MIN_MEMBERS
            && buf.len() == AFFECT_CASCADE_WINDOW
            && buf.back().unwrap().saturating_sub(*buf.front().unwrap()) >= CASCADE_MIN_SPREAD;
        let already_active = world.codex.cascade_active.contains(&sid);
        let fired = rose || (already_active && entry.high_fear >= CASCADE_MIN_SPREAD);
        let count = entry.high_fear;
        if let Some(ev) = edge_trigger_species(&mut world.codex.cascade_active, sid, fired, || {
            let (lx, ly) = centroid_of(agg, sid);
            CodexEvent {
                event_type: EventType::PanicCascade,
                tick,
                species_id: sid,
                value: count as f32,
                loc_x: lx,
                loc_y: ly,
            }
        }) {
            to_push.push(ev);
        }
    }
    let gone: Vec<u32> = world
        .codex
        .fear_count_history
        .keys()
        .filter(|sid| !agg.active().contains(sid))
        .copied()
        .collect();
    for sid in gone {
        world.codex.fear_count_history.remove(&sid);
        world.codex.cascade_active.remove(&sid);
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// MassGrief: a species' population dropped `>= GRIEF_DROP_FRAC` over the
/// last `GRIEF_WINDOW` samples of `world.codex.pop_history` (already
/// maintained by `population::update_pop_history`, which runs before this in
/// `observe_all`), from a peak of `>= GRIEF_MIN_PEAK`, while the surviving
/// members show mean PANIC `>= GRIEF_MEAN_PANIC`. Edge-triggered; re-arms
/// when mean PANIC subsides.
pub(super) fn detect_mass_grief(world: &mut World, agg: &SpeciesAggTable) {
    use crate::affect::PANIC;
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    let sids: Vec<u32> = world.codex.pop_history.keys().copied().collect();
    for sid in sids {
        let Some(buf) = world.codex.pop_history.get(&sid) else { continue };
        if buf.len() < GRIEF_WINDOW {
            world.codex.grief_active.remove(&sid);
            continue;
        }
        let peak = buf.iter().rev().take(GRIEF_WINDOW).copied().max().unwrap_or(0);
        let cur = *buf.back().unwrap_or(&0);
        if peak < GRIEF_MIN_PEAK {
            world.codex.grief_active.remove(&sid);
            continue;
        }
        let drop_frac = 1.0 - (cur as f32 / peak as f32);
        let mean_panic = agg
            .get(sid)
            .map(|e| (e.affect_sum[PANIC] / e.count.max(1) as f64) as f32)
            .unwrap_or(0.0);
        let fired = drop_frac >= GRIEF_DROP_FRAC && mean_panic >= GRIEF_MEAN_PANIC;
        if let Some(ev) = edge_trigger_species(&mut world.codex.grief_active, sid, fired, || {
            let (lx, ly) = centroid_of(agg, sid);
            CodexEvent {
                event_type: EventType::MassGrief,
                tick,
                species_id: sid,
                value: mean_panic,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::affect::SEEK;
    use crate::genome::Genome;
    use crate::prelude::Vec2;

    fn frenzy_world(n: u32, seek: f32, spread: f32) -> (World, SpeciesAggTable) {
        let mut w = World::new(3);
        w.affect_enabled = true;
        for k in 0..n {
            // Tight cluster: x within `spread` of the centroid.
            let x = 500.0 + (k as f32 / n as f32) * spread;
            let id = w.spawn_agent(Vec2::new(x, 500.0), Genome::neutral());
            w.agents.affect[id as usize][SEEK] = seek;
        }
        let mut agg = SpeciesAggTable::default();
        agg.build(&w);
        (w, agg)
    }

    #[test]
    fn converged_high_seek_fires_and_latches() {
        let (mut w, agg) = frenzy_world(8, 0.9, 40.0);
        detect_feeding_frenzy(&mut w, &agg);
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::FeedingFrenzy));
        let before = w.codex.events.len();
        detect_feeding_frenzy(&mut w, &agg); // latched: no re-fire
        assert_eq!(w.codex.events.len(), before);
    }

    #[test]
    fn scattered_seekers_do_not_frenzy() {
        let (mut w, agg) = frenzy_world(8, 0.9, 400.0); // spread > FRENZY_SPREAD_MAX
        detect_feeding_frenzy(&mut w, &agg);
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn calm_species_does_not_frenzy() {
        let (mut w, agg) = frenzy_world(8, 0.1, 40.0); // SEEK below HIGH_SEEK
        detect_feeding_frenzy(&mut w, &agg);
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn sustained_angry_cluster_fires_after_window() {
        use crate::affect::RAGE;
        let mut w = World::new(4);
        w.affect_enabled = true;
        for k in 0..6 {
            let id = w.spawn_agent(Vec2::new(500.0 + k as f32 * 8.0, 500.0), Genome::neutral());
            w.agents.affect[id as usize][RAGE] = 0.8;
        }
        let mut agg = SpeciesAggTable::default();
        // Advance the streak past RAGE_WINDOW; agg is rebuilt each tick in the sim,
        // here we reuse a fresh build each iteration.
        for _ in 0..=RAGE_WINDOW {
            agg.build(&w);
            detect_territorial_rage(&mut w, &agg);
            w.tick += 1;
        }
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::TerritorialRage));
    }

    #[test]
    fn brief_anger_does_not_fire() {
        use crate::affect::RAGE;
        let mut w = World::new(4);
        w.affect_enabled = true;
        for k in 0..6 {
            let id = w.spawn_agent(Vec2::new(500.0 + k as f32 * 8.0, 500.0), Genome::neutral());
            w.agents.affect[id as usize][RAGE] = 0.8;
        }
        let mut agg = SpeciesAggTable::default();
        agg.build(&w);
        for _ in 0..5 {
            detect_territorial_rage(&mut w, &agg);
            w.tick += 1;
        }
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn fear_spreading_through_cluster_fires_cascade() {
        use crate::affect::FEAR;
        let mut w = World::new(6);
        w.affect_enabled = true;
        let ids: Vec<u32> = (0..10)
            .map(|k| w.spawn_agent(Vec2::new(500.0 + k as f32 * 5.0, 500.0), Genome::neutral()))
            .collect();
        let mut agg = SpeciesAggTable::default();
        // Window front: nobody afraid.
        agg.build(&w);
        detect_panic_cascade(&mut w, &agg);
        w.tick += 1;
        // Window back: fear has spread to the whole cluster.
        for &id in &ids {
            w.agents.affect[id as usize][FEAR] = 0.9;
        }
        for _ in 0..(AFFECT_CASCADE_WINDOW as u64) {
            agg.build(&w);
            detect_panic_cascade(&mut w, &agg);
            w.tick += 1;
        }
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::PanicCascade));
    }

    #[test]
    fn steady_low_fear_does_not_cascade() {
        let mut w = World::new(6);
        w.affect_enabled = true;
        for k in 0..10 {
            w.spawn_agent(Vec2::new(500.0 + k as f32 * 5.0, 500.0), Genome::neutral());
        }
        let mut agg = SpeciesAggTable::default();
        for _ in 0..(AFFECT_CASCADE_WINDOW as u64 + 2) {
            agg.build(&w);
            detect_panic_cascade(&mut w, &agg);
            w.tick += 1;
        }
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn dieoff_with_high_panic_fires_grief() {
        use crate::affect::PANIC;
        use std::collections::VecDeque;
        let mut w = World::new(8);
        w.affect_enabled = true;
        // A handful of grieving survivors.
        for k in 0..4 {
            let id = w.spawn_agent(Vec2::new(500.0 + k as f32 * 6.0, 500.0), Genome::neutral());
            w.agents.affect[id as usize][PANIC] = 0.8;
        }
        // Seed a population window that fell from 20 → 4 (a die-off).
        let mut buf: VecDeque<u32> = VecDeque::new();
        for _ in 0..(GRIEF_WINDOW - 1) {
            buf.push_back(20);
        }
        buf.push_back(4);
        w.codex.pop_history.insert(0, buf);
        let mut agg = SpeciesAggTable::default();
        agg.build(&w);
        detect_mass_grief(&mut w, &agg);
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::MassGrief));
    }

    #[test]
    fn dieoff_without_panic_is_not_grief() {
        use std::collections::VecDeque;
        let mut w = World::new(8);
        w.affect_enabled = true;
        for k in 0..4 {
            w.spawn_agent(Vec2::new(500.0 + k as f32 * 6.0, 500.0), Genome::neutral());
        }
        let mut buf: VecDeque<u32> = VecDeque::new();
        for _ in 0..(GRIEF_WINDOW - 1) {
            buf.push_back(20);
        }
        buf.push_back(4);
        w.codex.pop_history.insert(0, buf);
        let mut agg = SpeciesAggTable::default();
        agg.build(&w);
        detect_mass_grief(&mut w, &agg); // PANIC == 0 ⇒ no grief
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn observe_all_is_inert_when_affect_disabled() {
        let mut w = World::new(9);
        debug_assert!(!w.affect_enabled);
        for k in 0..12 {
            w.spawn_agent(Vec2::new(500.0 + k as f32 * 4.0, 500.0), Genome::neutral());
        }
        for _ in 0..5 {
            crate::tick::step(&mut w);
        }
        assert!(!w.codex.events.iter().any(|e| matches!(
            e.event_type,
            EventType::FeedingFrenzy
                | EventType::TerritorialRage
                | EventType::PanicCascade
                | EventType::MassGrief
        )));
        assert!(w.codex.frenzy_active.is_empty() && w.codex.rage_streak.is_empty());
    }
}
