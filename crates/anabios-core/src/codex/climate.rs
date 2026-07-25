//! Codex climate-maladaptation detector (E11): MaladaptationLag.
//!
//! The drifting climate (`env_optimum_at`) makes the environmental optimum a
//! MOVING target. A species whose learned skill can't keep up — its mean skill
//! stays chronically far from the current optimum for a long stretch — is
//! *maladapted*. This stress phenomenon only exists BECAUSE the optimum moves,
//! so the detector short-circuits when `env_period == 0` (no seasonal optimum);
//! it never fires in non-DIT scenarios.

use super::*;

/// Detect chronic maladaptation: a species whose mean skill-channel value stays
/// `>= MALADAPT_LAG_MIN` from the current environmental optimum for
/// `MALADAPT_WINDOW` ticks. Latched per species (via `edge_trigger_species`);
/// re-arms when the species catches up (lag drops below the threshold, which
/// resets the streak). Amortized on `CYCLE_CHECK_INTERVAL`.
pub(super) fn detect_maladaptation(world: &mut World, agg: &SpeciesAggTable) {
    // Only meaningful under a moving optimum — no seasonal optimum, no lag.
    if world.env_period == 0 {
        return;
    }
    if !world.tick.is_multiple_of(CYCLE_CHECK_INTERVAL) {
        return;
    }
    let tick = world.tick;
    let optimum = crate::culture::env_optimum_at(tick, world.env_period, world.climate_drift_rate);
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        // Reuse the fused aggregate's per-channel meme sums (built once per
        // tick) instead of a fresh member scan.
        let mean_skill =
            (e.meme_sums[crate::culture::SKILL_CHANNEL] / e.count.max(1) as f64) as f32;
        let lag = (mean_skill - optimum).abs();
        // Advance / reset the per-species streak, then read it back so the
        // mutable borrow ends before the latch borrow below.
        let streak = {
            let s = world.codex.maladapt_streak.entry(sid).or_insert(0);
            if lag >= MALADAPT_LAG_MIN {
                *s += CYCLE_CHECK_INTERVAL as u32;
            } else {
                *s = 0;
            }
            *s
        };
        let fired = streak >= MALADAPT_WINDOW;
        let (lx, ly) = centroid_of(agg, sid);
        if let Some(ev) =
            edge_trigger_species(&mut world.codex.maladapt_active, sid, fired, || CodexEvent {
                event_type: EventType::MaladaptationLag,
                tick,
                species_id: sid,
                value: lag,
                loc_x: lx,
                loc_y: ly,
            })
        {
            to_push.push(ev);
        }
    }
    // Prune BOTH per-species maps for species absent from the active set —
    // per-species scratch grows unbounded on extinction otherwise.
    let active = agg.active();
    world.codex.maladapt_streak.retain(|sid, _| active.contains(sid));
    world.codex.maladapt_active.retain(|sid| active.contains(sid));
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    /// A one-species `SpeciesAggTable` whose members' mean `SKILL_CHANNEL` value
    /// is `skill` (encoded through `meme_sums`, exactly as `build` would).
    fn agg_with_skill(count: u32, skill: f32) -> SpeciesAggTable {
        let mut agg = SpeciesAggTable::default();
        let mut e = SpeciesAgg { count, ..Default::default() };
        e.meme_sums[crate::culture::SKILL_CHANNEL] = skill as f64 * count as f64;
        agg.entries.resize(1, SpeciesAgg::default());
        agg.entries[0] = e;
        agg.active.push(0);
        agg
    }

    #[test]
    fn pinned_far_from_optimum_fires() {
        // Mean skill pinned at 0.0 while the optimum sweeps [0.25, 0.75] → the
        // lag equals the optimum, always >= MALADAPT_LAG_MIN. After the streak
        // reaches MALADAPT_WINDOW, MaladaptationLag fires once (latched).
        let mut w = World::new(1);
        w.env_period = 400;
        let agg = agg_with_skill(20, 0.0);
        let mut t = 0u64;
        while t <= MALADAPT_WINDOW as u64 + CYCLE_CHECK_INTERVAL {
            w.tick = t;
            detect_maladaptation(&mut w, &agg);
            t += CYCLE_CHECK_INTERVAL;
        }
        let fired =
            w.codex.events.iter().filter(|e| e.event_type == EventType::MaladaptationLag).count();
        assert_eq!(fired, 1, "a species pinned far from the optimum is maladapted (once, latched)");
    }

    #[test]
    fn tracking_optimum_does_not_fire() {
        // Mean skill tracks the optimum exactly each check → lag 0 → the streak
        // resets every check → never fires.
        let mut w = World::new(1);
        w.env_period = 400;
        let mut t = 0u64;
        while t <= MALADAPT_WINDOW as u64 * 4 {
            w.tick = t;
            let opt = crate::culture::env_optimum_at(t, w.env_period, w.climate_drift_rate);
            let agg = agg_with_skill(20, opt);
            detect_maladaptation(&mut w, &agg);
            t += CYCLE_CHECK_INTERVAL;
        }
        assert!(
            w.codex.events.is_empty(),
            "a species tracking the moving optimum is not maladapted"
        );
    }

    #[test]
    fn no_env_period_never_fires() {
        // Without a seasonal optimum (env_period == 0) the detector is inert,
        // even for a species maximally far from any fixed value.
        let mut w = World::new(1);
        w.env_period = 0;
        let agg = agg_with_skill(20, 0.0);
        let mut t = 0u64;
        while t <= MALADAPT_WINDOW as u64 * 2 {
            w.tick = t;
            detect_maladaptation(&mut w, &agg);
            t += CYCLE_CHECK_INTERVAL;
        }
        assert!(w.codex.events.is_empty(), "no seasonal optimum ⇒ no maladaptation");
    }
}
