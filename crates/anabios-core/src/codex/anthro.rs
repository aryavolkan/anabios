//! Anthropogenic arms-race detector (HuntedAdaptation).
//!
//! Pairs a scenario-tagged culture-bearing ("human") lineage against each
//! prey lineage it hunts (established via the war substrate's kill records)
//! and fires when the prey answers the culture lineage's rise: the culture
//! power index (tech era + weapon damage) must climb ≥
//! `HUNTED_MIN_CULTURE_DELTA` from the baseline latched when the pairing
//! qualified, AND at least one prey defense channel — armor, speed, or the
//! Vigilance gene — must climb ≥ `HUNTED_MIN_PREY_DELTA` from its own
//! baseline. Runs only under `World::anthro_race_enabled` (gated in
//! `observe_all`).
//!
//! Why per-channel max and not a summed index: the measured response (see
//! `docs/superpowers/specs/2026-08-19-anthro-arms-race-design.md`) is that a
//! hunted herd answers with ONE channel — speed rose while vigilance fell in
//! the A/B — so a sum lets one channel's decline mask another's rise. Why
//! baselines instead of a rolling co-rise window: the culture climbs in
//! ~1k-tick era bursts and then sits flat, while prey traits grind slowly —
//! the two signals never peak inside one short window, but their
//! divergence-from-pairing is the actual arms-race outcome. And baselines
//! are STICKY across war episodes: hostility records cool (and their kill
//! counts reset at each `WarEnded`), but the race between two lineages spans
//! many episodes, so a pair's baseline survives until a lineage goes
//! extinct. The detector credits correlation (hunted + culture rose + prey
//! trait rose), not causation — the same standard as the world-global
//! ArmsRace detector.

use super::*;

/// Per-lineage-root fold of the species-level aggregates. Species splinters
/// of one lineage contribute to a single root entry, mirroring how the war
/// substrate keys feuds by `lineage_root`.
#[derive(Default)]
struct RootAgg {
    count: u32,
    sum_x: f64,
    sum_y: f64,
    armor: f64,
    speed: f64,
    vigilance: f64,
    weapon: f64,
    inv_counts: [u32; crate::invention::INVENTION_COUNT],
    /// Lowest active species id in the lineage (stable event attribution).
    lowest_sid: u32,
}

impl RootAgg {
    #[inline]
    fn mean(&self, sum: f64) -> f32 {
        (sum / self.count.max(1) as f64) as f32
    }
    /// Culture power index: highest tech era at ≥50% lineage adoption (0
    /// when the invention tree is off) plus normalized mean weapon damage —
    /// both the cultural climb and raw weapon escalation register.
    fn culture_index(&self) -> f32 {
        let mut era = 0u8;
        for (k, &c) in self.inv_counts.iter().enumerate() {
            if 2 * c >= self.count.max(1) {
                era = era.max(crate::invention::INVENTIONS[k].era);
            }
        }
        era as f32 + self.mean(self.weapon) / crate::module::DAMAGE_MAX
    }
    /// The three prey defense channels: normalized armor, raw mean speed,
    /// mean Vigilance gene.
    fn defense_channels(&self) -> (f32, f32, f32) {
        (
            self.mean(self.armor) / crate::module::PROTECTION_MAX,
            self.mean(self.speed),
            self.mean(self.vigilance),
        )
    }
}

/// Fire HuntedAdaptation when a hunted prey lineage answers its culture
/// predator's rise (see the module doc for the signal design). One shot per
/// qualifying pair: the latch is pruned only if the pair stops qualifying
/// (hostility record cooled away, or a lineage went extinct), in which case
/// a later re-heated feud re-baselines and may fire again.
pub(super) fn detect_hunted_adaptation(world: &mut World, agg: &SpeciesAggTable) {
    // Candidate hunter/hunted pairs from the war substrate: exactly one root
    // culture-tagged, with enough kills in the current hostility episode to
    // call it hunting.
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    for (&(lo, hi), rec) in world.codex.hostility.iter() {
        if rec.kills < HUNTED_MIN_KILLS {
            continue;
        }
        let lo_culture = world.culture_roots.contains(&lo);
        let hi_culture = world.culture_roots.contains(&hi);
        match (lo_culture, hi_culture) {
            (true, false) => pairs.push((lo, hi)),
            (false, true) => pairs.push((hi, lo)),
            _ => {}
        }
    }

    // Roots we must fold this tick: candidate pairs plus every baselined
    // pair (baselines are sticky across war episodes — a cooled feud pauses
    // qualification, not the race).
    let mut wanted: BTreeSet<u32> = BTreeSet::new();
    for &(culture, prey) in &pairs {
        wanted.insert(culture);
        wanted.insert(prey);
    }
    for &(culture, prey) in world.codex.hunted_baselines.keys() {
        wanted.insert(culture);
        wanted.insert(prey);
    }
    let mut roots: BTreeMap<u32, RootAgg> = BTreeMap::new();
    for &sid in agg.active() {
        let root = war::lineage_root(world, sid);
        if !wanted.contains(&root) {
            continue;
        }
        let e = agg.get(sid).expect("active species has an entry");
        let r =
            roots.entry(root).or_insert_with(|| RootAgg { lowest_sid: sid, ..RootAgg::default() });
        r.count += e.count;
        r.sum_x += e.sum_x;
        r.sum_y += e.sum_y;
        r.armor += e.armor_sum;
        r.speed += e.speed_sum;
        r.vigilance += e.genome_sums[crate::genome::GenomeSlot::Vigilance.idx()];
        r.weapon += e.weapon_sum;
        r.lowest_sid = r.lowest_sid.min(sid);
        for (k, dst) in r.inv_counts.iter_mut().enumerate() {
            *dst += e.invention_counts[k];
        }
    }
    let alive = |root: u32| roots.get(&root).is_some_and(|r| r.count > 0);

    // Qualify new pairs: latch both sides' indices as the baseline.
    for &(culture, prey) in &pairs {
        if alive(culture) && alive(prey) {
            let c = &roots[&culture];
            let p = &roots[&prey];
            let (armor, speed, vig) = p.defense_channels();
            world.codex.hunted_baselines.entry((culture, prey)).or_insert((
                c.culture_index(),
                armor,
                speed,
                vig,
            ));
        }
    }

    // Every baselined pair with both lineages alive checks its divergence —
    // including pairs whose hostility record has cooled (the race continues
    // between episodes).
    let tick = world.tick;
    let mut events: Vec<CodexEvent> = Vec::new();
    let baselined: Vec<(u32, u32)> = world.codex.hunted_baselines.keys().copied().collect();
    for (culture, prey) in baselined {
        if world.codex.hunted_active.contains(&(culture, prey)) {
            continue;
        }
        if !alive(culture) || !alive(prey) {
            continue;
        }
        let base = world.codex.hunted_baselines[&(culture, prey)];
        let c = &roots[&culture];
        let p = &roots[&prey];
        let culture_delta = c.culture_index() - base.0;
        let (armor_now, speed_now, vig_now) = p.defense_channels();
        let prey_delta = (armor_now - base.1).max(speed_now - base.2).max(vig_now - base.3);
        if culture_delta >= HUNTED_MIN_CULTURE_DELTA && prey_delta >= HUNTED_MIN_PREY_DELTA {
            world.codex.hunted_active.insert((culture, prey));
            let n = p.count.max(1) as f64;
            events.push(CodexEvent {
                event_type: EventType::HuntedAdaptation,
                tick,
                species_id: p.lowest_sid,
                value: prey_delta,
                loc_x: (p.sum_x / n) as f32,
                loc_y: (p.sum_y / n) as f32,
            });
        }
    }
    // Prune only on extinction: a root with no alive members is gone for
    // good (roots are never reused), so its baselines/latches are dead
    // state. Cooled-but-alive pairs keep their baselines (sticky, above).
    world
        .codex
        .hunted_baselines
        .retain(|&(c, p), _| wanted.contains(&c) && wanted.contains(&p) && alive(c) && alive(p));
    world
        .codex
        .hunted_active
        .retain(|&(c, p)| wanted.contains(&c) && wanted.contains(&p) && alive(c) && alive(p));
    for ev in events {
        world.codex.push_event(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::{Genome, GenomeSlot};
    use crate::prelude::Vec2;

    /// A world with a culture-tagged founder (species 1) and a wild prey
    /// founder (species 2), plus a hostility record with enough kills to
    /// qualify as a hunter/hunted pair.
    fn world_with_pair() -> (World, u32, u32) {
        let mut w = World::new(3);
        w.anthro_race_enabled = true;
        let hunter = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
        let prey = w.spawn_agent(Vec2::new(420.0, 400.0), Genome::neutral());
        let hunter_sp = crate::prelude_test::reassign_to_new_species(&mut w, hunter);
        let prey_sp = crate::prelude_test::reassign_to_new_species(&mut w, prey);
        w.culture_roots.insert(hunter_sp);
        for _ in 0..HUNTED_MIN_KILLS {
            war::record_war_death(&mut w, prey_sp, hunter_sp, 410.0, 400.0);
        }
        (w, hunter, prey)
    }

    /// Step the detector once with real aggregates built from the world.
    fn observe(w: &mut World) {
        let mut agg = SpeciesAggTable::default();
        agg.build(w);
        detect_hunted_adaptation(w, &agg);
    }

    #[test]
    fn baseline_latches_then_divergence_fires_once() {
        let (mut w, hunter, prey) = world_with_pair();
        let (h, p) = (hunter as usize, prey as usize);
        // First observation latches the baseline (unarmed hunter, plain prey).
        observe(&mut w);
        assert!(w.codex.hunted_baselines.contains_key(&(1, 2)));
        assert!(w.codex.events.is_empty(), "baseline latch alone must not fire");

        // Culture rises (weapon damage climb) AND the prey answers on one
        // channel (Vigilance) — armor/speed flat: the per-channel max must
        // still credit the response.
        w.agents.modules[h] =
            smallvec::smallvec![crate::module::Module::Weapon { damage: 6.0, energy_cost: 1.0 }];
        w.agents.genome[p].set(GenomeSlot::Vigilance, 0.5 + HUNTED_MIN_PREY_DELTA + 0.05);
        w.tick = 10;
        observe(&mut w);
        let fires: Vec<_> =
            w.codex.events.iter().filter(|e| e.event_type == EventType::HuntedAdaptation).collect();
        assert_eq!(fires.len(), 1, "the divergence fires exactly once");
        assert_eq!(fires[0].species_id, 2, "attributed to the prey lineage");
        assert!(fires[0].value >= HUNTED_MIN_PREY_DELTA);

        // One-shot: further rises do not refire while the pair stays live.
        w.agents.genome[p].set(GenomeSlot::Vigilance, 1.0);
        w.tick = 20;
        observe(&mut w);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::HuntedAdaptation).count(),
            1,
            "latched pair does not refire"
        );
    }

    #[test]
    fn prey_rise_without_culture_rise_does_not_fire() {
        let (mut w, _hunter, prey) = world_with_pair();
        observe(&mut w); // baseline
        w.agents.genome[prey as usize].set(GenomeSlot::Vigilance, 1.0);
        observe(&mut w);
        assert!(
            w.codex.events.is_empty(),
            "prey adaptation without a culture rise is not an arms race"
        );
    }

    #[test]
    fn no_kills_no_pair() {
        let mut w = World::new(3);
        w.anthro_race_enabled = true;
        let a = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
        let sp = crate::prelude_test::reassign_to_new_species(&mut w, a);
        w.culture_roots.insert(sp);
        observe(&mut w);
        assert!(w.codex.hunted_baselines.is_empty(), "no kills ⇒ no candidate pair");
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn flag_off_detector_is_silent() {
        // The observe_all gate is the guard; called directly with the flag
        // off there are no culture roots, so no pairs can form.
        let mut w = World::new(3);
        observe(&mut w);
        assert!(w.codex.events.is_empty());
        assert!(w.codex.hunted_baselines.is_empty());
    }
}
