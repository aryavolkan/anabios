//! Affect-layer codex detector (M-B): mass fright / panic rout.
//!
//! Latch-free by design — deduped against the already-serialized `codex.events`
//! ring rather than a new `CodexState` latch, so the affect layer adds ZERO
//! serialized state and flag-off stays byte-identical to the pre-affect
//! baseline. Reads only serialized `affect[FEAR]` ⇒ replay-safe. Zero RNG.

use super::*;
use crate::affect::FEAR;
use crate::world::World;

/// A member counts as frightened at/above this FEAR activation.
pub const FRIGHT_FEAR_MIN: f32 = 0.6;
/// Minimum simultaneously-frightened same-species members for a "mass" event.
pub const FRIGHT_MIN_MEMBERS: usize = 4;
/// Ticks a species is suppressed from re-firing after a MassFright.
pub const FRIGHT_COOLDOWN: u64 = 50;

/// MassFright: a same-species cluster of ≥`FRIGHT_MIN_MEMBERS` members
/// simultaneously at/above `FRIGHT_FEAR_MIN` FEAR. Latch-free: instead of a
/// `CodexState` latch set, re-fires are suppressed by scanning the existing
/// `world.codex.events` ring for a same-species `MassFright` within
/// `FRIGHT_COOLDOWN` ticks — no new serialized state.
pub(super) fn detect_mass_fright(world: &mut World, agg: &SpeciesAggTable) {
    // Cheap, provable no-op when the layer is off (affect is all-zero anyway).
    if !world.affect_enabled {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        let frightened = entry
            .member_idx
            .iter()
            .filter(|&&i| world.agents.affect[i][FEAR] >= FRIGHT_FEAR_MIN)
            .count();
        if frightened < FRIGHT_MIN_MEMBERS {
            continue;
        }
        // Dedup against the serialized event log (no new serialized latch).
        let recently = world.codex.events.iter().any(|e| {
            e.event_type == EventType::MassFright
                && e.species_id == sid
                && tick.saturating_sub(e.tick) < FRIGHT_COOLDOWN
        });
        if recently {
            continue;
        }
        let (lx, ly) = centroid_of(agg, sid);
        to_push.push(CodexEvent {
            event_type: EventType::MassFright,
            tick,
            species_id: sid,
            value: frightened as f32,
            loc_x: lx,
            loc_y: ly,
        });
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
    use crate::world::World;

    #[test]
    fn mass_fright_fires_once_per_cooldown_when_a_cluster_panics() {
        use crate::affect::FEAR;
        let mut w = World::new(5);
        w.affect_enabled = true;
        // Five same-species agents; drive their FEAR over the detector threshold.
        let mut ids = Vec::new();
        for k in 0..5 {
            ids.push(w.spawn_agent(Vec2::new(500.0 + k as f32 * 5.0, 500.0), Genome::neutral()));
        }
        for &id in &ids {
            w.agents.affect[id as usize][FEAR] = 0.9;
        }
        // Build the per-species aggregate and run the detector directly.
        let mut agg = std::mem::take(&mut w.codex_agg);
        agg.build(&w);
        super::detect_mass_fright(&mut w, &agg);
        w.codex_agg = agg;
        let n = w.codex.events.iter().filter(|e| e.event_type == EventType::MassFright).count();
        assert_eq!(n, 1, "one MassFright for the frightened cluster");

        // Immediately re-running within the cooldown must NOT double-fire.
        let mut agg2 = std::mem::take(&mut w.codex_agg);
        agg2.build(&w);
        super::detect_mass_fright(&mut w, &agg2);
        w.codex_agg = agg2;
        let n2 = w.codex.events.iter().filter(|e| e.event_type == EventType::MassFright).count();
        assert_eq!(n2, 1, "cooldown dedup prevents a re-fire");
    }
}
