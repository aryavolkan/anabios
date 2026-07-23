//! Codex trait-evolution detectors (E5): trait fixation, rapid adaptation,
//! and convergent evolution between independent lineages.

use super::*;
use crate::genome::Genome;

/// Push one genome-moment sample per active species (10-tick cadence);
/// prune species gone from the active set for a full `MOMENT_SPAN`.
pub(super) fn update_genome_moments(world: &mut World, agg: &SpeciesAggTable) {
    if !world.tick.is_multiple_of(CYCLE_CHECK_INTERVAL) {
        return;
    }
    let tick = world.tick;
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        let n = e.count.max(1) as f64;
        let mut mean = Genome::neutral();
        let mut var = Genome::neutral();
        for slot in 0..50 {
            let m = e.genome_sums[slot] / n;
            // Population variance: E[x²] − E[x]², clamped against fp noise.
            let v = (e.genome_sumsq[slot] / n - m * m).max(0.0);
            mean.0[slot] = m as f32;
            var.0[slot] = v as f32;
        }
        let buf = world.codex.genome_moments.entry(sid).or_default();
        if buf.len() == MOMENT_RING {
            buf.pop_front();
        }
        buf.push_back(TraitMoments { tick, mean, var });
    }
    // Prune species absent from the active set with a stale newest sample.
    let stale: Vec<u32> = world
        .codex
        .genome_moments
        .iter()
        .filter(|(sid, buf)| {
            !agg.active().contains(sid)
                && buf.back().map(|m| tick.saturating_sub(m.tick) > MOMENT_SPAN).unwrap_or(true)
        })
        .map(|(sid, _)| *sid)
        .collect();
    for sid in stale {
        world.codex.genome_moments.remove(&sid);
    }
}

/// Fixation analysis over a moment ring: returns the fixed slot ids.
/// A slot qualifies when it was polymorphic in ≥half of the first-half
/// samples and collapsed in ALL of the last 10 samples.
fn fixed_slots(buf: &VecDeque<TraitMoments>) -> Vec<u8> {
    if buf.len() < MOMENT_RING {
        return Vec::new();
    }
    let half = MOMENT_RING / 2;
    let mut out = Vec::new();
    for slot in 0..50u8 {
        let s = slot as usize;
        let poly_samples = buf.iter().take(half).filter(|m| m.var.0[s] >= FIX_POLY_VAR).count();
        if poly_samples * 2 < half {
            continue;
        }
        let collapsed = buf.iter().rev().take(10).all(|m| m.var.0[s] <= FIX_COLLAPSE_VAR);
        if collapsed {
            out.push(slot);
        }
    }
    out
}

pub(super) fn detect_trait_fixation(world: &mut World, agg: &SpeciesAggTable) {
    if !world.tick.is_multiple_of(CYCLE_CHECK_INTERVAL) {
        return;
    }
    let tick = world.tick;
    // Decide first, apply after (borrow discipline).
    let mut newly: Vec<(u32, u8, f32)> = Vec::new();
    for (&sid, buf) in world.codex.genome_moments.iter() {
        let Some(e) = agg.get(sid) else { continue };
        if e.count < FIX_MIN_MEMBERS {
            continue;
        }
        for slot in fixed_slots(buf) {
            let key = (sid, slot);
            if !world.codex.fixation_latches.contains(&key) {
                let mean = buf.back().expect("full ring").mean.0[slot as usize];
                newly.push((sid, slot, mean));
            }
        }
    }
    // Re-arm latched pairs whose variance re-opened.
    let reopen: Vec<(u32, u8)> = world
        .codex
        .fixation_latches
        .iter()
        .filter(|&&(sid, slot)| {
            world
                .codex
                .genome_moments
                .get(&sid)
                .and_then(|buf| buf.back())
                .map(|m| m.var.0[slot as usize] >= FIX_POLY_VAR)
                .unwrap_or(true)
        })
        .copied()
        .collect();

    for key in reopen {
        world.codex.fixation_latches.remove(&key);
    }
    for (sid, slot, mean) in newly {
        world.codex.fixation_latches.insert((sid, slot));
        let (lx, ly) = centroid_of(agg, sid);
        world.codex.push_event(CodexEvent {
            event_type: EventType::TraitFixation,
            tick,
            species_id: sid,
            value: slot as f32,
            loc_x: lx,
            loc_y: ly,
        });
        let archive = &mut world.codex.fixation_archive;
        if archive.len() == FIXATION_ARCHIVE_CAP {
            archive.pop_front();
        }
        archive.push_back((sid, slot, mean));
    }
}

pub(super) fn detect_rapid_adaptation(world: &mut World, agg: &SpeciesAggTable) {
    if !world.tick.is_multiple_of(CYCLE_CHECK_INTERVAL) {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    let mut fired: Vec<(u32, u8)> = Vec::new();
    for (&sid, buf) in world.codex.genome_moments.iter() {
        if buf.len() < MOMENT_RING {
            continue;
        }
        let Some(e) = agg.get(sid) else { continue };
        if e.count < FIX_MIN_MEMBERS {
            continue;
        }
        let newest = buf.back().expect("full ring");
        let baseline = &buf[MOMENT_RING - 1 - RAPID_WINDOW];
        for slot in 0..50u8 {
            let s = slot as usize;
            let delta = (newest.mean.0[s] - baseline.mean.0[s]).abs();
            if delta < RAPID_MIN_DELTA {
                continue;
            }
            // Recent σ: mean of per-sample σ over the window.
            let sigma = buf.iter().rev().take(RAPID_WINDOW).map(|m| m.var.0[s].sqrt()).sum::<f32>()
                / RAPID_WINDOW as f32;
            if delta < RAPID_SIGMA_MULT * sigma {
                continue;
            }
            let key = (sid, slot);
            let last = world.codex.rapid_cooldown.get(&key).copied().unwrap_or(0);
            if tick.saturating_sub(last) < RAPID_COOLDOWN {
                continue;
            }
            fired.push(key);
            let (lx, ly) = centroid_of(agg, sid);
            to_push.push(CodexEvent {
                event_type: EventType::RapidAdaptation,
                tick,
                species_id: sid,
                value: slot as f32,
                loc_x: lx,
                loc_y: ly,
            });
        }
    }
    for key in fired {
        world.codex.rapid_cooldown.insert(key, tick);
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Ancestor chain of a species, self-first, capped + cycle-guarded.
fn ancestry(world: &World, sid: u32) -> Vec<u32> {
    let mut out = vec![sid];
    let mut cur = sid;
    for _ in 0..64 {
        match world.species_parents.get(cur as usize).copied().flatten() {
            Some(p) if p != cur && !out.contains(&p) => {
                out.push(p);
                cur = p;
            }
            _ => break,
        }
    }
    out
}

/// Lineages are independent when their lowest common ancestor is the
/// universal founder root (species 0) — no shared post-founder ancestor.
fn lineages_independent(world: &World, a: u32, b: u32) -> bool {
    if a == b {
        return false;
    }
    let aa = ancestry(world, a);
    let ba = ancestry(world, b);
    let lca = aa.iter().find(|s| ba.contains(s)).copied();
    matches!(lca, None | Some(0))
}

pub(super) fn detect_convergent_evolution(world: &mut World, agg: &SpeciesAggTable) {
    if !world.tick.is_multiple_of(CYCLE_CHECK_INTERVAL) {
        return;
    }
    let tick = world.tick;
    // The newest archive entries since the last check are the fixations
    // pushed THIS observation cycle by detect_trait_fixation (which runs
    // first in observe_all). Find convergence against all earlier entries.
    let archive: Vec<(u32, u8, f32)> = world.codex.fixation_archive.iter().copied().collect();
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (i, &(sid_a, slot, mean_a)) in archive.iter().enumerate() {
        // Only evaluate each archive entry once — track by index watermark.
        if i < world.codex.converge_watermark {
            continue;
        }
        for &(sid_b, _, mean_b) in archive.iter().take(i) {
            if (mean_a - mean_b).abs() > CONVERGE_MEAN_TOL {
                continue;
            }
            if lineages_independent(world, sid_a, sid_b) {
                let (lx, ly) = centroid_of(agg, sid_a);
                to_push.push(CodexEvent {
                    event_type: EventType::ConvergentEvolution,
                    tick,
                    species_id: sid_a,
                    value: slot as f32,
                    loc_x: lx,
                    loc_y: ly,
                });
                break; // one convergence per fixation
            }
        }
    }
    world.codex.converge_watermark = archive.len();
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::Vec2;

    fn world_with_agents(n: u32) -> World {
        let mut w = World::new(1);
        for _ in 0..n {
            let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        }
        w
    }

    fn moments(poly_var: f32, collapse: bool) -> VecDeque<TraitMoments> {
        let half = MOMENT_RING / 2;
        (0..MOMENT_RING)
            .map(|i| {
                let mut mean = Genome::neutral();
                let mut var = Genome::neutral();
                mean.0.fill(0.5);
                let v = if i < half || !collapse { poly_var } else { 0.0 };
                var.0.fill(v);
                TraitMoments { tick: i as u64 * 10, mean, var }
            })
            .collect()
    }

    fn agg_for(sid: u32, count: u32) -> SpeciesAggTable {
        let mut agg = SpeciesAggTable::default();
        let e = SpeciesAgg {
            count,
            sum_x: 500.0 * count as f64,
            sum_y: 500.0 * count as f64,
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
    fn polymorphic_then_collapsed_fires_fixation() {
        let mut w = world_with_agents(12);
        w.tick = 0;
        w.codex.genome_moments.insert(0, moments(FIX_POLY_VAR, true));
        let agg = agg_for(0, 20);
        detect_trait_fixation(&mut w, &agg);
        let fired: Vec<_> =
            w.codex.events.iter().filter(|e| e.event_type == EventType::TraitFixation).collect();
        // The stuffed ring fixes all 50 slots at once — one event each.
        assert_eq!(fired.len(), 50);
        assert_eq!(w.codex.fixation_archive.len(), 50);
        // Latched: no refire.
        detect_trait_fixation(&mut w, &agg);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::TraitFixation).count(),
            50
        );
    }

    #[test]
    fn always_collapsed_never_fires() {
        let mut w = world_with_agents(12);
        w.tick = 0;
        // Never polymorphic — fixation requires a polymorphic past.
        w.codex.genome_moments.insert(0, moments(0.0, true));
        let agg = agg_for(0, 20);
        detect_trait_fixation(&mut w, &agg);
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn rapid_shift_fires_adaptation() {
        let mut w = world_with_agents(12);
        w.tick = 400;
        let mut ring: VecDeque<TraitMoments> = (0..MOMENT_RING)
            .map(|i| {
                let mut mean = Genome::neutral();
                let mut var = Genome::neutral();
                mean.0.fill(0.5);
                var.0.fill(0.0001); // tiny σ: any real shift exceeds 3σ
                TraitMoments { tick: i as u64 * 10, mean, var }
            })
            .collect();
        // Shift slot 5 by +0.3 over the last 10 samples.
        for m in ring.iter_mut().rev().take(RAPID_WINDOW) {
            m.mean.0[5] = 0.8;
        }
        w.codex.genome_moments.insert(0, ring);
        let agg = agg_for(0, 20);
        detect_rapid_adaptation(&mut w, &agg);
        let fired: Vec<_> =
            w.codex.events.iter().filter(|e| e.event_type == EventType::RapidAdaptation).collect();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].value, 5.0);
    }

    #[test]
    fn slow_drift_does_not_fire_adaptation() {
        let mut w = world_with_agents(12);
        w.tick = 400;
        // Same total shift spread evenly: within-window delta too small.
        let ring: VecDeque<TraitMoments> = (0..MOMENT_RING)
            .map(|i| {
                let mut mean = Genome::neutral();
                let mut var = Genome::neutral();
                mean.0.fill(0.5 + 0.3 * i as f32 / MOMENT_RING as f32);
                var.0.fill(0.0001);
                TraitMoments { tick: i as u64 * 10, mean, var }
            })
            .collect();
        w.codex.genome_moments.insert(0, ring);
        let agg = agg_for(0, 20);
        detect_rapid_adaptation(&mut w, &agg);
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn independent_lineages_converge() {
        let mut w = world_with_agents(12);
        w.tick = 0;
        // Phylogeny: 1 and 2 are founders (parent root 0).
        w.species_parents = vec![None, Some(0), Some(0), Some(1)];
        w.codex.fixation_archive.push_back((1, 5, 0.7));
        w.codex.converge_watermark = 1;
        w.codex.fixation_archive.push_back((2, 5, 0.72));
        let agg = agg_for(2, 20);
        detect_convergent_evolution(&mut w, &agg);
        assert_eq!(
            w.codex
                .events
                .iter()
                .filter(|e| e.event_type == EventType::ConvergentEvolution)
                .count(),
            1,
            "independent founders converge"
        );
    }

    #[test]
    fn sister_splinters_do_not_converge() {
        let mut w = world_with_agents(12);
        w.tick = 0;
        // Phylogeny: 3 splinters from 1 (LCA(3,1) = 1, not the root).
        w.species_parents = vec![None, Some(0), Some(0), Some(1)];
        w.codex.fixation_archive.push_back((1, 5, 0.7));
        w.codex.converge_watermark = 1;
        w.codex.fixation_archive.push_back((3, 5, 0.71));
        let agg = agg_for(3, 20);
        detect_convergent_evolution(&mut w, &agg);
        assert!(
            w.codex.events.is_empty(),
            "a splinter re-fixing its parent lineage's slot is not convergence"
        );
    }
}
