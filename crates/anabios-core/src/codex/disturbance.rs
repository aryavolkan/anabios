//! Codex disturbance/spatial detectors (E4): range expansion, spatial
//! segregation, corridor use, and post-fire succession.

use super::*;
use crate::tick::BIOME_STEP_INTERVAL;

pub(super) fn update_range_history(world: &mut World, agg: &SpeciesAggTable) {
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        let buf = world.codex.range_occ_history.entry(sid).or_default();
        if buf.len() == RANGE_WINDOW {
            buf.pop_front();
        }
        buf.push_back(e.occ_cells.len() as u32);
    }
}

pub(super) fn detect_range_expansion(world: &mut World, agg: &SpeciesAggTable) {
    if !world.tick.is_multiple_of(CYCLE_CHECK_INTERVAL) {
        return;
    }
    let tick = world.tick;
    // Decide first (immutable borrows), apply after.
    let mut decisions: Vec<(u32, bool, f32)> = Vec::new();
    for (sid, buf) in world.codex.range_occ_history.iter() {
        if buf.len() < RANGE_WINDOW {
            continue;
        }
        let start = *buf.front().unwrap() as f32;
        let end = *buf.back().unwrap() as f32;
        let grown = start >= 1.0 && end >= RANGE_MIN_CELLS as f32 && end >= RANGE_GROWTH * start;
        // Require real movement too: centroid displacement over the
        // (shorter) migration window, so a pure density bloom doesn't count.
        let moved = world
            .codex
            .centroid_history
            .get(sid)
            .filter(|c| c.len() >= 2)
            .map(|c| {
                let f = *c.front().unwrap();
                let l = *c.back().unwrap();
                torus_distance(
                    glam::Vec2::new(f.0, f.1),
                    glam::Vec2::new(l.0, l.1),
                    world.world_size,
                )
            })
            .unwrap_or(0.0);
        decisions.push((*sid, grown && moved >= RANGE_MIN_DISPLACEMENT, end / start.max(1.0)));
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, fired, ratio) in decisions {
        if let Some(ev) = edge_trigger_species(&mut world.codex.range_active, sid, fired, || {
            let (lx, ly) = centroid_of(agg, sid);
            CodexEvent {
                event_type: EventType::RangeExpansion,
                tick,
                species_id: sid,
                value: ratio,
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

pub(super) fn detect_segregation(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    // Cells shared by ≥2 species this tick (ascending-order inserts).
    let mut owner: BTreeMap<u32, u32> = BTreeMap::new();
    let mut shared: BTreeSet<u32> = BTreeSet::new();
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        for &cell in &e.occ_cells {
            match owner.get(&cell) {
                None => {
                    owner.insert(cell, sid);
                }
                Some(&other) if other != sid => {
                    shared.insert(cell);
                }
                _ => {}
            }
        }
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        let eligible =
            e.count >= SEGREGATION_MIN_MEMBERS && e.occ_cells.len() >= SEGREGATION_MIN_CELLS;
        let overlap = if eligible {
            let shared_cells = e.occ_cells.iter().filter(|c| shared.contains(*c)).count();
            shared_cells as f32 / e.occ_cells.len() as f32
        } else {
            1.0 // ineligible species can't be "segregated"
        };
        let low = eligible && overlap < SEGREGATION_OVERLAP_MAX;
        // "Emerged" requires a prior mixed state: species founded apart are
        // not segregation events, they are founder geography.
        if eligible && !low {
            world.codex.segregation_was_mixed.insert(sid);
        }
        let streak = world.codex.segregation_streak.entry(sid).or_insert(0);
        if low {
            *streak += 1;
        } else {
            *streak = 0;
        }
        let fired =
            *streak >= SEGREGATION_WINDOW && world.codex.segregation_was_mixed.contains(&sid);
        if let Some(ev) =
            edge_trigger_species(&mut world.codex.segregation_active, sid, fired, || {
                let (lx, ly) = centroid_of(agg, sid);
                CodexEvent {
                    event_type: EventType::SegregationEmerged,
                    tick,
                    species_id: sid,
                    value: 1.0 - overlap,
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

pub(super) fn detect_corridor_use(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    let mut decisions: Vec<(u32, bool, f32)> = Vec::new();
    for (sid, dirs) in world.codex.migration_dirs.iter() {
        if dirs.len() < CORRIDOR_MIN_MIGRATIONS {
            decisions.push((*sid, false, 0.0));
            continue;
        }
        // Check the most recent CORRIDOR_MIN_MIGRATIONS records: all
        // pairwise direction cosines must agree, the span must be sustained,
        // AND every leg must cross barrier terrain (a corridor is a passage
        // through hostile ground, not plain grassland drift).
        let recent: Vec<(u64, f32, f32, u32)> =
            dirs.iter().rev().take(CORRIDOR_MIN_MIGRATIONS).copied().collect();
        let mut min_cos = f32::INFINITY;
        for i in 0..recent.len() {
            for j in (i + 1)..recent.len() {
                let cos = recent[i].1 * recent[j].1 + recent[i].2 * recent[j].2;
                min_cos = min_cos.min(cos);
            }
        }
        let span = recent
            .iter()
            .map(|e| e.0)
            .max()
            .unwrap_or(0)
            .saturating_sub(recent.iter().map(|e| e.0).min().unwrap_or(0));
        // Barrier hits aggregate across the legs: a species habitually
        // migrating one direction through mixed terrain punches through
        // water/rock bands repeatedly in total, even when no single leg is
        // barrier-dominated.
        let barrier_total: u32 = recent.iter().map(|e| e.3).sum();
        let angle = min_cos.clamp(-1.0, 1.0).acos();
        let corridor = min_cos >= CORRIDOR_MAX_ANGLE_COS
            && span >= CORRIDOR_MIN_SPAN
            && barrier_total >= CORRIDOR_MIN_BARRIER_HITS;
        decisions.push((*sid, corridor, angle));
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    let mut clear: Vec<u32> = Vec::new();
    for (sid, fired, angle) in decisions {
        if let Some(ev) = edge_trigger_species(&mut world.codex.corridor_active, sid, fired, || {
            let (lx, ly) = centroid_of(agg, sid);
            CodexEvent {
                event_type: EventType::CorridorUse,
                tick,
                species_id: sid,
                value: angle,
                loc_x: lx,
                loc_y: ly,
            }
        }) {
            to_push.push(ev);
            // Restart the direction log so the next corridor needs fresh
            // migrations (the latch alone would hold for a while anyway).
            clear.push(sid);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
    for sid in clear {
        world.codex.migration_dirs.remove(&sid);
    }
}

/// Scan tracked fire sites for recovery: ≥50% of scorched cells vegetated
/// again (Pioneer or Climax — grazers crop pioneer growth below its ceiling,
/// so requiring full Climax never completes under real grazing pressure).
/// Runs on the biome-step cadence (succession only moves then).
pub(super) fn detect_succession(world: &mut World) {
    if !world.disasters_enabled || !world.tick.is_multiple_of(BIOME_STEP_INTERVAL) {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for site in world.disasters.sites.iter_mut() {
        if site.succession_fired || site.cells.is_empty() {
            continue;
        }
        let recovered = site
            .cells
            .iter()
            .filter(|&&i| world.biome.cells[i as usize].succession != crate::biome::SUCCESSION_BARE)
            .count();
        if recovered as f32 >= SUCCESSION_RECOVERED_FRAC * site.cells.len() as f32 {
            site.succession_fired = true;
            let (col, row) = site.epicenter;
            let cs = world.biome.cell_size;
            to_push.push(CodexEvent {
                event_type: EventType::Succession,
                tick,
                species_id: 0,
                value: recovered as f32 / site.cells.len() as f32,
                loc_x: (col as f32 + 0.5) * cs,
                loc_y: (row as f32 + 0.5) * cs,
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
    use crate::disaster::DisasterSite;
    use crate::genome::Genome;
    use crate::prelude::Vec2;

    fn world_with_agent() -> World {
        let mut w = World::new(1);
        let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w
    }

    fn agg_with_occ(sid: u32, count: u32, cells: &[u32]) -> SpeciesAggTable {
        let mut agg = SpeciesAggTable::default();
        let e = SpeciesAgg {
            count,
            sum_x: 500.0 * count as f64,
            sum_y: 500.0 * count as f64,
            occ_cells: cells.iter().copied().collect(),
            ..Default::default()
        };
        if sid as usize >= agg.entries.len() {
            agg.entries.resize(sid as usize + 1, SpeciesAgg::default());
        }
        agg.entries[sid as usize] = e;
        agg.active.push(sid);
        agg
    }

    #[test]
    fn range_expansion_fires_on_growth_with_movement() {
        let mut w = world_with_agent();
        w.tick = 0;
        let buf: VecDeque<u32> = std::iter::repeat_n(10, RANGE_WINDOW / 2)
            .chain(std::iter::repeat_n(30, RANGE_WINDOW / 2))
            .collect();
        w.codex.range_occ_history.insert(0, buf);
        w.codex.centroid_history.insert(0, [(100.0, 100.0), (300.0, 100.0)].into_iter().collect());
        let agg = agg_with_occ(0, 30, &(0..30).collect::<Vec<_>>());
        detect_range_expansion(&mut w, &agg);
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::RangeExpansion));
    }

    #[test]
    fn range_expansion_needs_movement_not_just_density() {
        let mut w = world_with_agent();
        w.tick = 0;
        let buf: VecDeque<u32> = std::iter::repeat_n(10, RANGE_WINDOW / 2)
            .chain(std::iter::repeat_n(30, RANGE_WINDOW / 2))
            .collect();
        w.codex.range_occ_history.insert(0, buf);
        // Stationary centroid: growth without displacement must not fire.
        w.codex.centroid_history.insert(0, [(100.0, 100.0), (101.0, 100.0)].into_iter().collect());
        let agg = agg_with_occ(0, 30, &(0..30).collect::<Vec<_>>());
        detect_range_expansion(&mut w, &agg);
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn segregation_fires_after_sustained_isolation() {
        let mut w = world_with_agent();
        // Mixed phase: species 0 and 1 share every cell → species 0 is
        // marked as having been mixed.
        let mut agg_mixed = agg_with_occ(0, 30, &(0..30).collect::<Vec<_>>());
        let e1 = SpeciesAgg { count: 30, occ_cells: (0..30).collect(), ..Default::default() };
        agg_mixed.entries.resize(2, SpeciesAgg::default());
        agg_mixed.entries[1] = e1;
        agg_mixed.active.push(1);
        detect_segregation(&mut w, &agg_mixed);
        // Isolated phase: species 0 alone at 0..30, species 1 at 100..130.
        let mut agg = agg_with_occ(0, 30, &(0..30).collect::<Vec<_>>());
        let e2 = SpeciesAgg { count: 30, occ_cells: (100..130).collect(), ..Default::default() };
        agg.entries.resize(2, SpeciesAgg::default());
        agg.entries[1] = e2;
        agg.active.push(1);
        for _ in 0..SEGREGATION_WINDOW {
            detect_segregation(&mut w, &agg);
        }
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::SegregationEmerged));
    }

    #[test]
    fn segregation_does_not_fire_when_founded_apart() {
        let mut w = world_with_agent();
        // Species 0 isolated from the start — never mixed, so sustained
        // isolation must NOT fire (founder geography, not emergence).
        let agg = agg_with_occ(0, 30, &(0..30).collect::<Vec<_>>());
        for _ in 0..(2 * SEGREGATION_WINDOW) {
            detect_segregation(&mut w, &agg);
        }
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn segregation_needs_the_streak() {
        let mut w = world_with_agent();
        let agg = agg_with_occ(0, 30, &(0..30).collect::<Vec<_>>());
        for _ in 0..(SEGREGATION_WINDOW - 1) {
            detect_segregation(&mut w, &agg);
        }
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn overlapping_species_never_segregate() {
        let mut w = world_with_agent();
        // Both species share every cell.
        let mut agg = agg_with_occ(0, 30, &(0..30).collect::<Vec<_>>());
        let e1 = SpeciesAgg { count: 30, occ_cells: (0..30).collect(), ..Default::default() };
        agg.entries.resize(2, SpeciesAgg::default());
        agg.entries[1] = e1;
        agg.active.push(1);
        for _ in 0..(2 * SEGREGATION_WINDOW) {
            detect_segregation(&mut w, &agg);
        }
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn corridor_fires_on_sustained_agreeing_migrations() {
        let mut w = world_with_agent();
        let dirs: VecDeque<(u64, f32, f32, u32)> = [
            (0_u64, 1.0_f32, 0.0_f32, 6_u32),
            (160, 0.99, 0.1, 5),
            (320, 1.0, -0.05, 7),
            (480, 0.995, 0.02, 4),
        ]
        .into_iter()
        .map(|(t, x, y, b)| {
            let l = (x * x + y * y).sqrt();
            (t, x / l, y / l, b)
        })
        .collect();
        w.codex.migration_dirs.insert(0, dirs);
        let agg = agg_with_occ(0, 30, &(0..30).collect::<Vec<_>>());
        detect_corridor_use(&mut w, &agg);
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::CorridorUse));
    }

    #[test]
    fn corridor_rejects_disagreeing_directions() {
        let mut w = world_with_agent();
        let dirs: VecDeque<(u64, f32, f32, u32)> = [
            (0_u64, 1.0_f32, 0.0_f32, 6_u32),
            (160, -1.0, 0.0, 5),
            (320, 0.0, 1.0, 6),
            (480, 1.0, 0.1, 4),
        ]
        .into_iter()
        .collect();
        w.codex.migration_dirs.insert(0, dirs);
        let agg = agg_with_occ(0, 30, &(0..30).collect::<Vec<_>>());
        detect_corridor_use(&mut w, &agg);
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn corridor_rejects_bursts() {
        let mut w = world_with_agent();
        // Four agreeing barrier-crossing legs but all within 60 ticks — a
        // burst, not a corridor habit.
        let dirs: VecDeque<(u64, f32, f32, u32)> = [
            (0_u64, 1.0_f32, 0.0_f32, 6_u32),
            (20, 0.99, 0.05, 5),
            (40, 1.0, -0.02, 6),
            (60, 0.995, 0.01, 4),
        ]
        .into_iter()
        .collect();
        w.codex.migration_dirs.insert(0, dirs);
        let agg = agg_with_occ(0, 30, &(0..30).collect::<Vec<_>>());
        detect_corridor_use(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "burst migration must not be a corridor");
    }

    #[test]
    fn corridor_requires_barrier_crossing() {
        let mut w = world_with_agent();
        // Sustained, agreeing — but all legs over open ground (0 hits).
        let dirs: VecDeque<(u64, f32, f32, u32)> = [
            (0_u64, 1.0_f32, 0.0_f32, 0_u32),
            (160, 0.99, 0.1, 1),
            (320, 1.0, -0.05, 0),
            (480, 0.995, 0.02, 0),
        ]
        .into_iter()
        .collect();
        w.codex.migration_dirs.insert(0, dirs);
        let agg = agg_with_occ(0, 30, &(0..30).collect::<Vec<_>>());
        detect_corridor_use(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "grassland drift is not a corridor");
    }

    #[test]
    fn succession_fires_when_scar_recovers() {
        let mut w = world_with_agent();
        w.disasters_enabled = true;
        w.tick = 0; // biome-step cadence
                    // Mark two cells Climax and track them as a site; pre-set half.
        w.disasters.sites.push(DisasterSite {
            epicenter: (5, 5),
            cells: vec![0, 1, 2, 3],
            succession_fired: false,
        });
        // All four cells are Climax by default → fires immediately.
        detect_succession(&mut w);
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::Succession));
        // Latched per site.
        detect_succession(&mut w);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::Succession).count(),
            1
        );
    }

    #[test]
    fn succession_waits_for_recovery() {
        let mut w = world_with_agent();
        w.disasters_enabled = true;
        w.tick = 0;
        w.disasters.sites.push(DisasterSite {
            epicenter: (5, 5),
            cells: vec![0, 1, 2, 3],
            succession_fired: false,
        });
        // Only one of four cells vegetated; the rest Bare.
        for (i, cell) in w.biome.cells.iter_mut().enumerate() {
            if (1..=3).contains(&i) {
                cell.succession = crate::biome::SUCCESSION_BARE;
            }
        }
        detect_succession(&mut w);
        assert!(w.codex.events.is_empty());
        // Pioneer counts as recovered (grazed scars rarely reach full climax).
        w.biome.cells[1].succession = crate::biome::SUCCESSION_PIONEER;
        detect_succession(&mut w);
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::Succession));
    }
}
