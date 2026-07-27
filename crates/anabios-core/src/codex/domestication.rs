//! Codex domestication detector (E13): LivestockHerd — a livestock species
//! sustains ≥`LIVESTOCK_HERD_MIN` living tamed members for `LIVESTOCK_HERD_WINDOW` consecutive
//! ticks. (AnimalDomesticated is pushed directly from
//! `crate::domestication::husbandry_step` at the moment of the first tame.)
//! Inert unless `World::domestication_enabled`.

use super::*;

pub(super) fn detect_livestock_herd(world: &mut World, agg: &SpeciesAggTable) {
    if !world.domestication_enabled {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        let streak = world.codex.livestock_herd_streak.entry(sid).or_insert(0);
        if e.livestock_count >= LIVESTOCK_HERD_MIN {
            *streak += 1;
        } else {
            *streak = 0;
        }
        let streak_val = *streak;
        let count = e.livestock_count;
        if let Some(ev) = edge_trigger_species(
            &mut world.codex.livestock_herd_active,
            sid,
            streak_val >= LIVESTOCK_HERD_WINDOW,
            || {
                let (lx, ly) = centroid_of(agg, sid);
                CodexEvent {
                    event_type: EventType::LivestockHerd,
                    tick,
                    species_id: sid,
                    value: count as f32,
                    loc_x: lx,
                    loc_y: ly,
                }
            },
        ) {
            to_push.push(ev);
        }
    }
    // Prune streaks of species gone from the active set (extinct or
    // reclustered away) so the map can't grow unboundedly.
    let gone: Vec<u32> = world
        .codex
        .livestock_herd_streak
        .keys()
        .filter(|sid| !agg.active().contains(sid))
        .copied()
        .collect();
    for sid in gone {
        world.codex.livestock_herd_streak.remove(&sid);
        world.codex.livestock_herd_active.remove(&sid);
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;

    fn world_with_livestock(livestock: u32, wild: u32) -> World {
        let mut w = World::new(9);
        w.domestication_enabled = true;
        let herder = w.spawn_agent(crate::prelude::Vec2::new(500.0, 500.0), Genome::neutral());
        for k in 0..(livestock + wild) {
            let id =
                w.spawn_agent(crate::prelude::Vec2::new(10.0 + k as f32, 10.0), Genome::neutral());
            if k < livestock {
                w.agents.livestock_of[id as usize] = herder;
            }
        }
        w
    }

    #[test]
    fn herd_fires_after_window_and_rearms() {
        let mut w = world_with_livestock(LIVESTOCK_HERD_MIN, 4);
        let mut agg = SpeciesAggTable::default();
        for _ in 0..LIVESTOCK_HERD_WINDOW {
            agg.build(&w);
            detect_livestock_herd(&mut w, &agg);
        }
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::LivestockHerd).count(),
            1,
            "sustained herd fires exactly once at the window"
        );
        // Latched: further ticks do not refire while the herd persists.
        agg.build(&w);
        detect_livestock_herd(&mut w, &agg);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::LivestockHerd).count(),
            1
        );
        // Herd lost → latch re-arms.
        for id in w.agents.iter_alive().collect::<Vec<_>>() {
            w.agents.livestock_of[id as usize] = crate::agent::AGENT_NULL;
        }
        agg.build(&w);
        detect_livestock_herd(&mut w, &agg);
        assert!(
            !w.codex.livestock_herd_active.contains(&0),
            "drop below LIVESTOCK_HERD_MIN re-arms"
        );
    }

    #[test]
    fn below_minimum_never_fires() {
        let mut w = world_with_livestock(LIVESTOCK_HERD_MIN - 1, 4);
        let mut agg = SpeciesAggTable::default();
        for _ in 0..(LIVESTOCK_HERD_WINDOW + 10) {
            agg.build(&w);
            detect_livestock_herd(&mut w, &agg);
        }
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn flag_off_is_inert() {
        let mut w = world_with_livestock(LIVESTOCK_HERD_MIN + 2, 0);
        w.domestication_enabled = false;
        let mut agg = SpeciesAggTable::default();
        for _ in 0..(LIVESTOCK_HERD_WINDOW + 10) {
            agg.build(&w);
            detect_livestock_herd(&mut w, &agg);
        }
        assert!(w.codex.events.is_empty());
    }
}
