//! Meme-variant lineage tracking and the tradition detectors (E9):
//! TraditionPreserved, CulturalRadiation, InstitutionalRatchet.

use super::*;
use crate::program::MEME_CHANNELS;

/// Quantize a channel value to its band: `floor(v × 10)` clamped to [0, 10].
pub(crate) fn band_of(v: f32) -> u8 {
    (v * 10.0).floor().clamp(0.0, 10.0) as u8
}

/// Register a new variant (or return the id of the parent's variant when the
/// band is unchanged).
pub(crate) fn variant_for(
    world: &mut World,
    parent_variant: u32,
    channel: u8,
    band: u8,
    species: u32,
) -> u32 {
    let tick = world.tick;
    if parent_variant != 0 {
        let same = world
            .codex
            .meme_variants
            .get(&parent_variant)
            .map(|v| v.channel == channel && v.band == band)
            .unwrap_or(false);
        if same {
            return parent_variant;
        }
    }
    if world.codex.meme_variants.len() >= VARIANT_REGISTRY_CAP {
        return parent_variant; // registry full: collapse onto the parent
    }
    let id = world.codex.next_variant_id.max(1);
    world.codex.next_variant_id = id + 1;
    let parent = (parent_variant != 0).then_some(parent_variant);
    let root = parent
        .and_then(|p| world.codex.meme_variants.get(&p).map(|pv| pv.root))
        .unwrap_or(id);
    world.codex.meme_variants.insert(
        id,
        MemeVariant { id, channel, band, parent, root, born_tick: tick, born_species: species },
    );
    id
}

/// Assign a newborn's per-channel variants from its two parents (called at
/// reproduction for communicator children, after `inherit_meme`).
pub(crate) fn assign_birth_variants(world: &mut World, child: usize, a: usize, b: usize) {
    let species = world.agents.species_id[child];
    for ch in 0..MEME_CHANNELS {
        let band = band_of(world.agents.meme_vector[child][ch]);
        if band == 0 {
            world.agents.meme_lineage[child][ch] = 0;
            continue;
        }
        let va = world.agents.meme_lineage[a][ch];
        let vb = world.agents.meme_lineage[b][ch];
        // Prefer the parent whose variant already matches the child's band.
        let matches = |v: u32, world: &World| {
            v != 0
                && world
                    .codex
                    .meme_variants
                    .get(&v)
                    .map(|mv| mv.channel as usize == ch && mv.band == band)
                    .unwrap_or(false)
        };
        let parent = if matches(va, world) {
            va
        } else if matches(vb, world) {
            vb
        } else if va != 0 {
            va
        } else {
            vb
        };
        let id = variant_for(world, parent, ch as u8, band, species);
        world.agents.meme_lineage[child][ch] = id;
    }
}

/// Periodic band-transition sweep (every `VARIANT_SWEEP_INTERVAL` ticks):
/// an agent whose channel value drifted out of its variant's band carries a
/// new variant parented by the old one (drift / social-learning descent).
pub(super) fn variant_sweep(world: &mut World) {
    if !world.tick.is_multiple_of(VARIANT_SWEEP_INTERVAL) {
        return;
    }
    let mut ids = std::mem::take(&mut world.agents.scratch_ids);
    ids.clear();
    ids.extend(world.agents.iter_alive());
    for &id in &ids {
        let i = id as usize;
        let species = world.agents.species_id[i];
        for ch in 0..MEME_CHANNELS {
            let band = band_of(world.agents.meme_vector[i][ch]);
            let cur = world.agents.meme_lineage[i][ch];
            let stale = if cur == 0 {
                band > 0
            } else {
                world
                    .codex
                    .meme_variants
                    .get(&cur)
                    .map(|v| v.band != band)
                    .unwrap_or(true)
            };
            if stale {
                let parent = cur;
                let id = variant_for(world, parent, ch as u8, band, species);
                world.agents.meme_lineage[i][ch] = id;
            }
        }
    }
    world.agents.scratch_ids = ids;
}

/// Tradition: a variant LINEAGE (variant + all descendants) held by
/// ≥`TRADITION_ADOPTION_SHARE` of a species for `TRADITION_WINDOW` ticks,
/// with the root at least that old. Drift mints new variants constantly, so
/// per-variant holds never last — the tradition is the family that
/// outlives its founders, not one frozen value.
pub(super) fn detect_tradition(world: &mut World, agg: &SpeciesAggTable) {
    if !world.tick.is_multiple_of(VARIANT_SWEEP_INTERVAL) {
        return;
    }
    let tick = world.tick;
    // Tally holders per (variant, species), then credit every ancestor.
    let mut held: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut ids = std::mem::take(&mut world.agents.scratch_ids);
    ids.clear();
    ids.extend(world.agents.iter_alive());
    for &id in &ids {
        let i = id as usize;
        let sid = world.agents.species_id[i];
        for ch in 0..MEME_CHANNELS {
            let v = world.agents.meme_lineage[i][ch];
            if v == 0 {
                continue;
            }
            *held.entry((v, sid)).or_insert(0) += 1;
        }
    }
    world.agents.scratch_ids = ids;
    // Credit lineage roots directly (root stored at birth — O(1) per holder).
    let mut lineage_held: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    for ((v, sid), n) in held {
        let root = world.codex.meme_variants.get(&v).map(|mv| mv.root).unwrap_or(v);
        *lineage_held.entry((root, sid)).or_insert(0) += n;
    }
    // Which (lineage-root, species) pairs are above the share this sweep?
    let mut above: BTreeSet<(u32, u32)> = BTreeSet::new();
    for ((v, sid), n) in lineage_held {
        let Some(e) = agg.get(sid) else { continue };
        if n as f32 >= TRADITION_ADOPTION_SHARE * e.count as f32 {
            above.insert((v, sid));
        }
    }
    // Advance streaks for pairs above; decay others.
    let active_keys: Vec<(u32, u32)> = world.codex.tradition_streaks.keys().copied().collect();
    for key in active_keys {
        if !above.contains(&key) {
            world.codex.tradition_streaks.remove(&key);
        }
    }
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for key in above {
        let (vid, sid) = key;
        // The latch is per (channel, faction): a culture's tradition is
        // reported once, not once per speciation splinter.
        let faction = crate::codex::war::lineage_root(world, sid);
        let streak = world.codex.tradition_streaks.entry(key).or_insert(0);
        // Cadence is VARIANT_SWEEP_INTERVAL ticks per call — count ticks.
        *streak += VARIANT_SWEEP_INTERVAL as u32;
        let channel = world
            .codex
            .meme_variants
            .get(&vid)
            .map(|v| v.channel)
            .unwrap_or(0);
        let latch_key = (channel as u32, faction);
        if *streak < TRADITION_WINDOW
            || world.codex.tradition_active.contains(&latch_key)
        {
            continue;
        }
        let old_enough = world
            .codex
            .meme_variants
            .get(&vid)
            .map(|v| tick.saturating_sub(v.born_tick) >= TRADITION_MIN_AGE)
            .unwrap_or(false);
        if !old_enough {
            continue;
        }
        world.codex.tradition_active.insert(latch_key);
        let (lx, ly) = centroid_of(agg, sid);
        to_push.push(CodexEvent {
            event_type: EventType::TraditionPreserved,
            tick,
            species_id: sid,
            value: vid as f32,
            loc_x: lx,
            loc_y: ly,
        });
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Radiation: a variant whose descendant tree reaches ≥
/// `RADIATION_MIN_DESCENDANTS` distinct variants across ≥2 species.
pub(super) fn detect_radiation(world: &mut World, agg: &SpeciesAggTable) {
    if !world.tick.is_multiple_of(VARIANT_SWEEP_INTERVAL) {
        return;
    }
    let tick = world.tick;
    // Compute each variant's ancestor chain once (small registry).
    // Group by lineage root (stored at birth): descendants per root +
    // factions per root, O(V) per sweep — the O(V²) ancestor walks were the
    // dominant tick cost in the traditions profile.
    let mut desc: BTreeMap<u32, usize> = BTreeMap::new();
    let mut factions_of: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for v in world.codex.meme_variants.values() {
        if v.id == v.root {
            continue;
        }
        *desc.entry(v.root).or_insert(0) += 1;
        factions_of
            .entry(v.root)
            .or_default()
            .insert(crate::codex::war::lineage_root(world, v.born_species));
    }
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (anc, n) in desc {
        if world.codex.radiation_active.contains(&anc) {
            continue;
        }
        let factions = factions_of.get(&anc);
        if n >= RADIATION_MIN_DESCENDANTS && factions.map(|f| f.len() >= 2).unwrap_or(false) {
            world.codex.radiation_active.insert(anc);
            let sid = world
                .codex
                .meme_variants
                .get(&anc)
                .map(|v| v.born_species)
                .unwrap_or(0);
            let (lx, ly) = centroid_of(agg, sid);
            to_push.push(CodexEvent {
                event_type: EventType::CulturalRadiation,
                tick,
                species_id: sid,
                value: n as f32,
                loc_x: lx,
                loc_y: ly,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Institutional ratchet: a species holds tech era ≥ `RATCHET_MIN_ERA`
/// continuously for `RATCHET_WINDOW` ticks.
pub(super) fn detect_ratchet(world: &mut World, agg: &SpeciesAggTable) {
    if !world.inventions_enabled {
        return;
    }
    if !world.tick.is_multiple_of(CYCLE_CHECK_INTERVAL) {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        let era = (0..crate::invention::INVENTION_COUNT)
            .filter(|&k| {
                e.invention_counts[k] as f32 >= 0.5 * e.count.max(1) as f32
                    && crate::invention::INVENTIONS[k].era >= RATCHET_MIN_ERA
            })
            .map(|k| crate::invention::INVENTIONS[k].era)
            .max()
            .unwrap_or(0);
        let faction = crate::codex::war::lineage_root(world, sid);
        let streak = world.codex.ratchet_streak.entry(sid).or_insert(0);
        if era >= RATCHET_MIN_ERA {
            *streak += CYCLE_CHECK_INTERVAL as u32;
        } else {
            *streak = 0;
        }
        if *streak >= RATCHET_WINDOW && world.codex.ratchet_active.insert(faction) {
            let (lx, ly) = centroid_of(agg, sid);
            to_push.push(CodexEvent {
                event_type: EventType::InstitutionalRatchet,
                tick,
                species_id: sid,
                value: era as f32,
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

    fn world_with_agents(n: u32) -> World {
        let mut w = World::new(1);
        for _ in 0..n {
            let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        }
        w
    }

    #[test]
    fn band_quantization() {
        assert_eq!(band_of(0.0), 0);
        assert_eq!(band_of(0.05), 0);
        assert_eq!(band_of(0.15), 1);
        assert_eq!(band_of(0.99), 9);
        assert_eq!(band_of(1.0), 10);
    }

    #[test]
    fn same_band_inherits_variant_band_jump_spawns_child() {
        let mut w = world_with_agents(1);
        let root = variant_for(&mut w, 0, 3, 5, 0);
        assert_eq!(variant_for(&mut w, root, 3, 5, 0), root, "same band inherits");
        let child = variant_for(&mut w, root, 3, 6, 0);
        assert_ne!(child, root);
        assert_eq!(w.codex.meme_variants[&child].parent, Some(root));
    }

    #[test]
    fn birth_variants_follow_child_bands() {
        let mut w = world_with_agents(2);
        let ids: Vec<usize> = w.agents.iter_alive().map(|i| i as usize).collect();
        let (a, b) = (ids[0], ids[1]);
        // Parent A carries a band-5 variant on channel 2.
        w.agents.meme_vector[a][2] = 0.55;
        let root = variant_for(&mut w, 0, 2, 5, 0);
        w.agents.meme_lineage[a][2] = root;
        // Fake a child whose meme matches A on channel 2.
        let child = b;
        w.agents.meme_vector[child][2] = 0.52;
        assign_birth_variants(&mut w, child, a, a);
        assert_eq!(w.agents.meme_lineage[child][2], root, "matching band inherits the parent's variant");
    }

    #[test]
    fn radiation_counts_cross_species_descendants() {
        let mut w = world_with_agents(1);
        let agg = SpeciesAggTable::default();
        let root = variant_for(&mut w, 0, 0, 0, 0);
        // 55 descendants across species 0 and 1 (two factions), chained so
        // every new variant parents into the same lineage.
        let mut last = root;
        for i in 0..55u8 {
            let ch = 1 + (i / 11) % 5; // channels 1..5
            let band = i % 11;
            last = variant_for(&mut w, last, ch, band, (i % 2) as u32);
        }
        detect_radiation(&mut w, &agg);
        assert!(w
            .codex
            .events
            .iter()
            .any(|e| e.event_type == EventType::CulturalRadiation));
        // Latched: no refire.
        detect_radiation(&mut w, &agg);
        assert_eq!(
            w.codex
                .events
                .iter()
                .filter(|e| e.event_type == EventType::CulturalRadiation)
                .count(),
            1
        );
    }

    #[test]
    fn ratchet_requires_sustained_era() {
        let mut w = world_with_agents(1);
        w.inventions_enabled = true;
        // Species entry with ≥50% adoption of an era-2 invention.
        let mut agg = SpeciesAggTable::default();
        let era2 = (0..crate::invention::INVENTION_COUNT)
            .find(|&k| crate::invention::INVENTIONS[k].era >= 2)
            .expect("an era≥2 invention exists");
        let mut e = SpeciesAgg { count: 10, ..Default::default() };
        e.invention_counts[era2] = 6;
        agg.entries.resize(1, SpeciesAgg::default());
        agg.entries[0] = e;
        agg.active.push(0);
        for t in 0..RATCHET_WINDOW as u64 {
            w.tick = t * CYCLE_CHECK_INTERVAL;
            detect_ratchet(&mut w, &agg);
        }
        assert!(w
            .codex
            .events
            .iter()
            .any(|e| e.event_type == EventType::InstitutionalRatchet));
    }

    #[test]
    fn ratchet_rejects_era_dips() {
        let mut w = world_with_agents(1);
        w.inventions_enabled = true;
        let mut agg = SpeciesAggTable::default();
        agg.entries.resize(1, SpeciesAgg::default());
        agg.entries[0].count = 10;
        agg.active.push(0);
        for t in 0..RATCHET_WINDOW as u64 {
            w.tick = t * CYCLE_CHECK_INTERVAL;
            detect_ratchet(&mut w, &agg);
        }
        assert!(w.codex.events.is_empty(), "no adoption, no ratchet");
    }

    fn species_agg(count: u32) -> SpeciesAggTable {
        let mut agg = SpeciesAggTable::default();
        agg.entries.resize(1, SpeciesAgg::default());
        agg.entries[0] = SpeciesAgg { count, ..Default::default() };
        agg.active.push(0);
        agg
    }

    fn hold_variant(w: &mut World, holders: usize, channel: u8, band: u8) -> u32 {
        let ids: Vec<usize> = w.agents.iter_alive().map(|i| i as usize).collect();
        let root = variant_for(w, 0, channel, band, 0);
        for &i in ids.iter().take(holders) {
            w.agents.meme_lineage[i][channel as usize] = root;
        }
        root
    }

    #[test]
    fn tradition_needs_sustained_majority_and_an_old_variant() {
        let mut w = world_with_agents(10);
        // A band-5 variant on channel 3, born at tick 0, held by 6 of 10.
        hold_variant(&mut w, 6, 3, 5);
        let agg = species_agg(10);
        // Sustain past BOTH the adoption streak and the minimum variant age.
        let mut t = 0u64;
        while t <= TRADITION_WINDOW as u64 + TRADITION_MIN_AGE + VARIANT_SWEEP_INTERVAL {
            w.tick = t;
            detect_tradition(&mut w, &agg);
            t += VARIANT_SWEEP_INTERVAL;
        }
        assert!(
            w.codex.events.iter().any(|e| e.event_type == EventType::TraditionPreserved),
            "a majority-held variant that outlives TRADITION_MIN_AGE is a tradition"
        );
    }

    #[test]
    fn tradition_rejects_a_young_variant() {
        let mut w = world_with_agents(10);
        hold_variant(&mut w, 6, 3, 5);
        let agg = species_agg(10);
        // Streak window is met, but stop before TRADITION_MIN_AGE — the
        // carriers have not turned over, so majority adoption alone is not
        // yet a tradition. (TRADITION_MIN_AGE > TRADITION_WINDOW makes this a
        // real distinction.)
        let mut t = 0u64;
        while t <= TRADITION_WINDOW as u64 + VARIANT_SWEEP_INTERVAL {
            w.tick = t;
            detect_tradition(&mut w, &agg);
            t += VARIANT_SWEEP_INTERVAL;
        }
        assert!(
            w.codex.events.is_empty(),
            "a variant younger than TRADITION_MIN_AGE is not a tradition even at majority"
        );
    }

    #[test]
    fn tradition_rejects_a_minority_hold() {
        let mut w = world_with_agents(10);
        // Only 3 of 10 hold it — below TRADITION_ADOPTION_SHARE.
        hold_variant(&mut w, 3, 3, 5);
        let agg = species_agg(10);
        let mut t = 0u64;
        while t <= TRADITION_WINDOW as u64 + TRADITION_MIN_AGE + VARIANT_SWEEP_INTERVAL {
            w.tick = t;
            detect_tradition(&mut w, &agg);
            t += VARIANT_SWEEP_INTERVAL;
        }
        assert!(w.codex.events.is_empty(), "a minority custom is not a tradition");
    }

    #[test]
    fn settled_fidelity_shrinks_inheritance_jitter() {
        use crate::rng::Rng;
        let a = [0.8f32; MEME_CHANNELS];
        let b = [0.2f32; MEME_CHANNELS];
        // Same seed → identical draws across all three calls; only the fidelity
        // scale differs. `fidelity = 0.0` yields the pure parent baseline (no
        // jitter) without changing the draw sequence, so we can read off the
        // exact per-channel jitter and check the scaling assumption-free.
        let base = crate::culture::inherit_meme(&a, &b, &mut Rng::from_seed(7), false, false, 0.0);
        let full = crate::culture::inherit_meme(&a, &b, &mut Rng::from_seed(7), false, false, 1.0);
        let settled =
            crate::culture::inherit_meme(&a, &b, &mut Rng::from_seed(7), false, false, SETTLED_FIDELITY);
        let mut jittered = 0;
        for ch in 0..MEME_CHANNELS {
            let jf = full[ch] - base[ch];
            let js = settled[ch] - base[ch];
            assert!(
                (js - jf * SETTLED_FIDELITY).abs() < 1e-6,
                "settled jitter must be SETTLED_FIDELITY× the baseline on ch {ch}"
            );
            if jf.abs() > 1e-6 {
                jittered += 1;
            }
        }
        assert!(jittered > 0, "at least one channel must actually jitter for this test to bite");
    }
}
