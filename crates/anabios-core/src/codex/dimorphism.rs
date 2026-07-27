//! Codex sexual-dimorphism detectors (E12): directional sexual selection on
//! the dimorphism gene, and sex-ratio collapse (a species one bad tick from
//! losing a sex). Both are inert unless `World::sexual_dimorphism_enabled`.

use super::*;
use crate::genome::GenomeSlot;

const DIMORPHISM_SLOT: usize = GenomeSlot::SexualDimorphism as usize;

/// SexualSelection: per-species mean of the dimorphism gene rises
/// ≥`SEXSEL_MIN_DELTA` across the E5 genome-moment ring and ends
/// ≥`SEXSEL_MIN_MEAN`. Latched per species; re-arms below
/// `SEXSEL_REARM_MEAN`. Runs on the trait-moment cadence (the ring is only
/// appended then).
pub(super) fn detect_sexual_selection(world: &mut World, agg: &SpeciesAggTable) {
    if !world.sexual_dimorphism_enabled {
        return;
    }
    if !world.tick.is_multiple_of(CYCLE_CHECK_INTERVAL) {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    let mut rearm: Vec<u32> = Vec::new();
    for (&sid, buf) in world.codex.genome_moments.iter() {
        if buf.len() < MOMENT_RING {
            continue;
        }
        let Some(e) = agg.get(sid) else { continue };
        if e.count < FIX_MIN_MEMBERS {
            continue;
        }
        let newest = buf.back().expect("full ring").mean.0[DIMORPHISM_SLOT];
        let oldest = buf.front().expect("full ring").mean.0[DIMORPHISM_SLOT];
        if newest < SEXSEL_REARM_MEAN && world.codex.sexsel_active.contains(&sid) {
            rearm.push(sid);
            continue;
        }
        let fired = newest - oldest >= SEXSEL_MIN_DELTA && newest >= SEXSEL_MIN_MEAN;
        if let Some(ev) = edge_trigger_species(&mut world.codex.sexsel_active, sid, fired, || {
            let (lx, ly) = centroid_of(agg, sid);
            CodexEvent {
                event_type: EventType::SexualSelection,
                tick,
                species_id: sid,
                value: newest,
                loc_x: lx,
                loc_y: ly,
            }
        }) {
            to_push.push(ev);
        }
    }
    for sid in rearm {
        world.codex.sexsel_active.remove(&sid);
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// SexRatioCollapse: per-species minority-sex count drops below
/// `SEXRATIO_MIN_MINORITY` while the species has ≥`SEXRATIO_MIN_TOTAL`
/// members. Latched per species; re-arms when the minority recovers to
/// ≥`SEXRATIO_MIN_MINORITY`. Cheap per-tick check off the fused agg table.
pub(super) fn detect_sex_ratio_collapse(world: &mut World, agg: &SpeciesAggTable) {
    if !world.sexual_dimorphism_enabled {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        if e.count < SEXRATIO_MIN_TOTAL {
            world.codex.sexratio_active.remove(&sid);
            continue;
        }
        let male = e.male_count;
        let female = e.count - male;
        let minority = male.min(female);
        let fired = minority < SEXRATIO_MIN_MINORITY;
        if let Some(ev) = edge_trigger_species(&mut world.codex.sexratio_active, sid, fired, || {
            let (lx, ly) = centroid_of(agg, sid);
            CodexEvent {
                event_type: EventType::SexRatioCollapse,
                tick,
                species_id: sid,
                value: minority as f32 / e.count as f32,
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
    use crate::codex::TraitMoments;
    use crate::genome::Genome;
    use std::collections::VecDeque;

    fn moments_ring(d_oldest: f32, d_newest: f32) -> VecDeque<TraitMoments> {
        let mut ring = VecDeque::new();
        for k in 0..MOMENT_RING {
            let t = k as f32 / (MOMENT_RING - 1) as f32;
            let d = d_oldest + (d_newest - d_oldest) * t;
            let mut mean = Genome::neutral();
            mean.0[DIMORPHISM_SLOT] = d;
            ring.push_back(TraitMoments { tick: k as u64 * 10, mean, var: Genome::neutral() });
        }
        ring
    }

    fn world_with_ring(d_oldest: f32, d_newest: f32, males: u32, females: u32) -> World {
        let mut w = World::new(3);
        w.sexual_dimorphism_enabled = true;
        w.tick = 100;
        for k in 0..(males + females) {
            let id =
                w.spawn_agent(crate::prelude::Vec2::new(10.0 + k as f32, 10.0), Genome::neutral());
            w.agents.sex.set(id as usize, k < males);
        }
        w.codex.genome_moments.insert(0, moments_ring(d_oldest, d_newest));
        w
    }

    fn build_agg(w: &mut World) -> SpeciesAggTable {
        let mut agg = SpeciesAggTable::default();
        agg.build(w);
        agg
    }

    #[test]
    fn sexual_selection_fires_on_sustained_d_rise() {
        let mut w = world_with_ring(0.45, 0.7, 8, 8);
        let agg = build_agg(&mut w);
        detect_sexual_selection(&mut w, &agg);
        assert!(
            w.codex.events.iter().any(|e| e.event_type == EventType::SexualSelection),
            "rising mean d past the bar fires SexualSelection"
        );
    }

    #[test]
    fn sexual_selection_quiet_on_drift_or_low_level() {
        // Big rise but ends below SEXSEL_MIN_MEAN.
        let mut w = world_with_ring(0.1, 0.5, 8, 8);
        let agg = build_agg(&mut w);
        detect_sexual_selection(&mut w, &agg);
        // Drift-scale wiggle at a high level (delta too small).
        let mut w2 = world_with_ring(0.7, 0.75, 8, 8);
        let agg2 = build_agg(&mut w2);
        detect_sexual_selection(&mut w2, &agg2);
        assert!(
            !w.codex.events.iter().any(|e| e.event_type == EventType::SexualSelection)
                && !w2.codex.events.iter().any(|e| e.event_type == EventType::SexualSelection),
            "neither a low endpoint nor drift noise fires SexualSelection"
        );
    }

    #[test]
    fn detectors_are_inert_with_flag_off() {
        let mut w = world_with_ring(0.45, 0.9, 11, 0);
        w.sexual_dimorphism_enabled = false;
        let agg = build_agg(&mut w);
        detect_sexual_selection(&mut w, &agg);
        detect_sex_ratio_collapse(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "flag off → no dimorphism events");
    }

    #[test]
    fn sex_ratio_collapse_fires_and_rearms() {
        // 11 males, 1 female, 12 total ≥ SEXRATIO_MIN_TOTAL.
        let mut w = world_with_ring(0.5, 0.5, 11, 1);
        let agg = build_agg(&mut w);
        detect_sex_ratio_collapse(&mut w, &agg);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::SexRatioCollapse).count(),
            1,
            "minority sex below the floor fires once"
        );
        // Latched: a second check with the same ratio does not refire.
        detect_sex_ratio_collapse(&mut w, &agg);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::SexRatioCollapse).count(),
            1
        );
        // Recovery re-arms (add females until the minority reaches the floor).
        for k in 0..2 {
            let id =
                w.spawn_agent(crate::prelude::Vec2::new(30.0 + k as f32, 10.0), Genome::neutral());
            w.agents.sex.set(id as usize, false);
        }
        let agg2 = build_agg(&mut w);
        detect_sex_ratio_collapse(&mut w, &agg2);
        assert!(!w.codex.sexratio_active.contains(&0), "minority at the floor re-arms the latch");
    }

    #[test]
    fn sex_ratio_check_ignores_small_species() {
        let mut w = world_with_ring(0.5, 0.5, 5, 0);
        let agg = build_agg(&mut w);
        detect_sex_ratio_collapse(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "species below SEXRATIO_MIN_TOTAL churn harmlessly");
    }
}
