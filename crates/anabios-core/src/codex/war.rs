//! Codex war & alliance & kin-network detectors (E7).

use super::*;

/// E7 war substrate feed: a combat-attributed death updates the hostility
/// record for the LINEAGE-ROOT pair (speciation splinters fight the same
/// war — keying by raw species id fragments every feud across splinters).
/// Called from `age_and_starve` alongside `record_combat_death`.
pub(crate) fn record_war_death(
    world: &mut World,
    victim_species: u32,
    attacker_species: u32,
    x: f32,
    y: f32,
) {
    if !feed_hostility(world, victim_species, attacker_species, 1.0) {
        return;
    }
    let a = lineage_root(world, attacker_species);
    let v = lineage_root(world, victim_species);
    let key = (a.min(v), a.max(v));
    let tick = world.tick;
    let rec = world.codex.hostility.entry(key).or_default();
    rec.kills += 1;
    // Running mean of kill locations (the front).
    let k = rec.kills as f32;
    rec.front_x += (x - rec.front_x) / k;
    rec.front_y += (y - rec.front_y) / k;
    rec.last_kill_tick = tick;
}

/// A cross-faction combat hit feeds hostility at `WAR_HIT_SCORE` — wars are
/// fought with hits (volleys, skirmishes), deaths are decisive moments.
/// Called from `combat_pass` on every hit.
pub(crate) fn record_war_hit(world: &mut World, target_species: u32, attacker_species: u32) {
    feed_hostility(world, target_species, attacker_species, WAR_HIT_SCORE);
}

/// Shared feed: adds `points` to the root-pair's decaying scores. Returns
/// false for same-faction/sentinel pairs (internal violence, not war).
fn feed_hostility(
    world: &mut World,
    target_species: u32,
    attacker_species: u32,
    points: f32,
) -> bool {
    if attacker_species == target_species || attacker_species == crate::sense::NO_NEIGHBOR_SPECIES {
        return false;
    }
    let a = lineage_root(world, attacker_species);
    let v = lineage_root(world, target_species);
    if a == v {
        return false;
    }
    let key = (a.min(v), a.max(v));
    let rec = world.codex.hostility.entry(key).or_default();
    rec.score += points;
    if a == key.0 {
        rec.score_lo += points;
    } else {
        rec.score_hi += points;
    }
    rec.below_ticks = 0;
    true
}

/// Walk `species_parents` to the FACTION root: the highest ancestor below
/// the universal placeholder root 0. Archetype founders all have parent
/// `Some(0)`, so walking into 0 would collapse every founder faction into
/// one "lineage" and no war could ever be declared (E5's convergence logic
/// treats LCA=0 as independent for the same reason).
fn lineage_root(world: &World, sid: u32) -> u32 {
    let mut cur = sid;
    for _ in 0..64 {
        match world.species_parents.get(cur as usize).copied().flatten() {
            Some(p) if p != cur && p != 0 => cur = p,
            _ => break,
        }
    }
    cur
}

/// Decay every pair's hostility score and evaluate war declare/end edges.
/// Runs per tick.
pub(super) fn detect_war(world: &mut World) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (&(lo, hi), rec) in world.codex.hostility.iter_mut() {
        rec.score *= WAR_DECAY;
        rec.score_lo *= WAR_DECAY;
        rec.score_hi *= WAR_DECAY;
        if rec.war_since == u64::MAX {
            // Rising edge: declare war. Sustained hostility of ANY shape
            // counts — the codex hierarchy is Predation (first kill) →
            // CombatRaid (one burst) → WarOrRaid (sustained campaign,
            // one-sided or mutual). One-way pressure IS a raid.
            if rec.score >= WAR_THRESHOLD {
                rec.war_since = tick;
                to_push.push(CodexEvent {
                    event_type: EventType::WarOrRaid,
                    tick,
                    species_id: lo,
                    value: rec.kills as f32,
                    loc_x: rec.front_x,
                    loc_y: rec.front_y,
                });
            }
        } else {
            // End edge: sustained decay below half-threshold.
            if rec.score < WAR_THRESHOLD * 0.5 {
                rec.below_ticks += 1;
                if rec.below_ticks >= WAR_END_TICKS {
                    let duration = tick.saturating_sub(rec.war_since);
                    let (fx, fy) = (rec.front_x, rec.front_y);
                    rec.war_since = u64::MAX;
                    rec.below_ticks = 0;
                    rec.kills = 0;
                    to_push.push(CodexEvent {
                        event_type: EventType::WarEnded,
                        tick,
                        species_id: hi,
                        value: duration as f32,
                        loc_x: fx,
                        loc_y: fy,
                    });
                }
            } else {
                rec.below_ticks = 0;
            }
        }
    }
    // Drop fully-cooled records so the map stays bounded.
    world.codex.hostility.retain(|_, rec| rec.score > 0.05 || rec.war_since != u64::MAX);
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Alliance: shared culture + zero cross-kills + sustained cross-species
/// sharing over `ALLIANCE_WINDOW`. One-shot per ordered pair.
pub(super) fn detect_alliance(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    // Tally cross-species shares in the window per ordered pair.
    let mut shares: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    for &(t, donor, recipient) in world.codex.share_events.iter() {
        if tick.saturating_sub(t) >= ALLIANCE_WINDOW || donor == recipient {
            continue;
        }
        let key = (donor.min(recipient), donor.max(recipient));
        *shares.entry(key).or_insert(0) += 1;
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for ((lo, hi), n) in shares {
        if n < ALLIANCE_MIN_SHARES || world.codex.alliance_active.contains(&(lo, hi)) {
            continue;
        }
        // Zero cross-kills in the window: no hostility record with a kill
        // inside it.
        let peaceful = world
            .codex
            .hostility
            .get(&(lo, hi))
            .map(|rec| tick.saturating_sub(rec.last_kill_tick) >= ALLIANCE_WINDOW)
            .unwrap_or(true);
        if !peaceful {
            continue;
        }
        // Shared culture: mean-meme L2 below the threshold.
        let (Some(a), Some(b)) = (agg.get(lo), agg.get(hi)) else { continue };
        let mut meme_l2 = 0.0_f64;
        for ch in 0..MEME_CHANNELS {
            let d =
                a.meme_sums[ch] / a.count.max(1) as f64 - b.meme_sums[ch] / b.count.max(1) as f64;
            meme_l2 += d * d;
        }
        if meme_l2.sqrt() as f32 >= ALLIANCE_MEME_MAX {
            continue;
        }
        world.codex.alliance_active.insert((lo, hi));
        let (lx, ly) = centroid_of(agg, lo);
        to_push.push(CodexEvent {
            event_type: EventType::AllianceFormed,
            tick,
            species_id: lo,
            value: n as f32,
            loc_x: lx,
            loc_y: ly,
        });
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Kin network: a species (genetic cluster = kin by construction) sustaining
/// size + spatial cohesion over `KIN_WINDOW`. Re-arms on collapse.
pub(super) fn detect_kin_network(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        let cohesive = e.count >= KIN_MIN_MEMBERS && {
            let positions: Vec<glam::Vec2> =
                e.member_idx.iter().map(|&i| world.agents.position[i]).collect();
            species_spread(&positions, world.world_size) <= KIN_SPREAD_MAX
        };
        let streak = world.codex.kin_streak.entry(sid).or_insert(0);
        if cohesive {
            *streak += 1;
        } else {
            *streak = 0;
        }
        let fired = *streak >= KIN_WINDOW;
        if let Some(ev) = edge_trigger_species(&mut world.codex.kin_active, sid, fired, || {
            let (lx, ly) = centroid_of(agg, sid);
            CodexEvent {
                event_type: EventType::KinNetworkStable,
                tick,
                species_id: sid,
                value: KIN_WINDOW as f32,
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

/// Map-level lookup (sense path — no World access needed there).
pub(crate) fn hostility_lookup(
    map: &BTreeMap<(u32, u32), HostilityRecord>,
    own: u32,
    other: u32,
) -> f32 {
    if own == other {
        return 0.0;
    }
    let key = (own.min(other), own.max(other));
    map.get(&key).map(|rec| (rec.score / WAR_THRESHOLD).clamp(0.0, 1.0)).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;

    fn world_with_agent() -> World {
        let mut w = World::new(1);
        let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w
    }

    #[test]
    fn sustained_campaign_declares_war_trickle_does_not() {
        // A 13-kill campaign crosses the threshold (one-sided or not — a
        // raid is one-sided violence); a spread-out trickle decays away.
        let mut w = world_with_agent();
        for _ in 0..13 {
            record_war_death(&mut w, 2, 1, 500.0, 500.0);
        }
        detect_war(&mut w);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::WarOrRaid).count(),
            1,
            "sustained campaign declares war"
        );

        let mut w2 = world_with_agent();
        // One kill per 100 real ticks: decay (per tick) outpaces the feed.
        for k in 0..40_u64 {
            for t in (k * 100)..((k + 1) * 100) {
                w2.tick = t;
                detect_war(&mut w2);
            }
            w2.tick = (k + 1) * 100 - 1;
            record_war_death(&mut w2, 2, 1, 500.0, 500.0);
        }
        assert!(w2.codex.events.is_empty(), "one kill per 100 ticks decays before the threshold");
    }

    #[test]
    fn cross_kills_declare_and_end_a_war() {
        let mut w = world_with_agent();
        // 13 kills one way, 4 back — mutual war crosses the bar.
        for _ in 0..13 {
            record_war_death(&mut w, 2, 1, 500.0, 500.0);
        }
        for _ in 0..4 {
            record_war_death(&mut w, 1, 2, 500.0, 500.0);
        }
        detect_war(&mut w);
        let wars: Vec<_> =
            w.codex.events.iter().filter(|e| e.event_type == EventType::WarOrRaid).collect();
        assert_eq!(wars.len(), 1, "13 kills declare one war");
        assert_eq!(wars[0].value, 17.0);
        // Latched: no redeclare while at war.
        detect_war(&mut w);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::WarOrRaid).count(),
            1
        );
        // Decay to below half-threshold for WAR_END_TICKS → WarEnded.
        let mut ended_at = None;
        for t in 1..20_000_u64 {
            w.tick = t;
            detect_war(&mut w);
            if w.codex.events.iter().any(|e| e.event_type == EventType::WarEnded) {
                ended_at = Some(t);
                break;
            }
        }
        assert!(ended_at.is_some(), "war must end after sustained peace");
    }

    #[test]
    fn same_species_kills_are_not_war() {
        let mut w = world_with_agent();
        for _ in 0..20 {
            record_war_death(&mut w, 1, 1, 500.0, 500.0);
        }
        detect_war(&mut w);
        assert!(w.codex.events.is_empty(), "internal violence is not war");
        assert!(w.codex.hostility.is_empty());
    }

    #[test]
    fn hostility_lookup_is_symmetric_and_normalized() {
        let mut w = world_with_agent();
        for _ in 0..6 {
            record_war_death(&mut w, 2, 1, 500.0, 500.0);
        }
        assert!((hostility_lookup(&w.codex.hostility, 1, 2) - 0.5).abs() < 1e-6);
        assert!((hostility_lookup(&w.codex.hostility, 2, 1) - 0.5).abs() < 1e-6);
        assert_eq!(hostility_lookup(&w.codex.hostility, 1, 1), 0.0);
        assert_eq!(hostility_lookup(&w.codex.hostility, 1, 9), 0.0);
    }
}
