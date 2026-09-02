//! Basic-needs codex detector: `Dehydration`.
//!
//! Latch-free by design (the `MassFright` pattern): deduped against the
//! already-serialized `codex.events` ring rather than a new `CodexState`
//! latch, so the basic-needs subsystem adds ZERO serialized codex state.
//! Reads only serialized `thirst` ⇒ replay-safe. Zero RNG. Inert unless
//! `World::basic_needs_enabled`.

use super::*;
use crate::world::World;

/// Mean species thirst at/above which `Dehydration` fires.
pub const DEHYDRATION_EVENT_MIN: f32 = 0.8;
/// Minimum live members for a species-level dehydration event.
pub const DEHYDRATION_MIN_MEMBERS: u32 = 4;
/// Ticks a species is suppressed from re-firing after a `Dehydration`.
pub const DEHYDRATION_COOLDOWN: u64 = 200;

/// Dehydration: a species whose live members' mean thirst reached
/// `DEHYDRATION_EVENT_MIN` — the population is failing to find water (drought,
/// range collapse, or a dry scenario). Re-fires only after
/// `DEHYDRATION_COOLDOWN` ticks, via the event-ring dedup.
pub(super) fn detect_dehydration(world: &mut World, agg: &SpeciesAggTable) {
    if !world.basic_needs_enabled {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        if entry.count < DEHYDRATION_MIN_MEMBERS {
            continue;
        }
        let mean_thirst = entry.member_idx.iter().map(|&i| world.agents.thirst[i]).sum::<f32>()
            / entry.count as f32;
        if mean_thirst < DEHYDRATION_EVENT_MIN {
            continue;
        }
        let recently = world.codex.events.iter().any(|e| {
            e.event_type == EventType::Dehydration
                && e.species_id == sid
                && tick.saturating_sub(e.tick) < DEHYDRATION_COOLDOWN
        });
        if recently {
            continue;
        }
        let (lx, ly) = centroid_of(agg, sid);
        to_push.push(CodexEvent {
            event_type: EventType::Dehydration,
            tick,
            species_id: sid,
            value: mean_thirst,
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

    /// A world with the flag on and `n` parched same-species agents.
    fn parched_world(n: usize, thirst: f32) -> (World, SpeciesAggTable) {
        let mut w = World::new(11);
        w.basic_needs_enabled = true;
        for k in 0..n {
            let id = w.spawn_agent(
                crate::prelude::Vec2::new(500.0 + k as f32, 500.0),
                crate::genome::Genome::neutral(),
            );
            w.agents.thirst[id as usize] = thirst;
        }
        let mut agg = SpeciesAggTable::default();
        agg.build(&w);
        (w, agg)
    }

    #[test]
    fn fires_on_high_mean_thirst_and_respects_cooldown() {
        let (mut w, agg) = parched_world(4, DEHYDRATION_EVENT_MIN);
        detect_dehydration(&mut w, &agg);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::Dehydration).count(),
            1,
            "threshold met with enough members: fires"
        );
        // Same tick window: the event-ring cooldown suppresses a re-fire.
        detect_dehydration(&mut w, &agg);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::Dehydration).count(),
            1,
            "cooldown: no immediate re-fire"
        );
        // Past the cooldown it can fire again (drought persists).
        w.tick += DEHYDRATION_COOLDOWN;
        detect_dehydration(&mut w, &agg);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::Dehydration).count(),
            2,
            "after cooldown: re-fires while still parched"
        );
    }

    #[test]
    fn below_threshold_or_too_few_members_is_quiet() {
        let (mut w, agg) = parched_world(4, DEHYDRATION_EVENT_MIN - 0.05);
        detect_dehydration(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "below mean-thirst threshold: quiet");
        let (mut w2, agg2) = parched_world((DEHYDRATION_MIN_MEMBERS - 1) as usize, 1.0);
        detect_dehydration(&mut w2, &agg2);
        assert!(w2.codex.events.is_empty(), "too few members: quiet");
    }

    #[test]
    fn flag_off_is_inert() {
        let (mut w, agg) = parched_world(4, 1.0);
        w.basic_needs_enabled = false;
        detect_dehydration(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "flag off: never fires");
    }
}
