//! Codex disease detectors: `EpidemicOutbreak` (60) and `MedicineContainment` (61).
//!
//! - **Outbreak** — per species with `count >= OUTBREAK_MIN_POP`: fires when
//!   the infected fraction crosses `OUTBREAK_FRACTION`, latched via
//!   `CodexState.epidemic_latched`; the latch re-arms when the fraction falls
//!   below `OUTBREAK_REARM`, so successive epidemic waves re-fire.
//! - **Containment** — the counter-pressure receipt: a latched species whose
//!   infected fraction falls below `OUTBREAK_REARM` **while ≥50% of its
//!   members hold Medicine** fires `MedicineContainment` instead of merely
//!   re-arming. One shot per wave (the latch is cleared either way; the next
//!   wave must re-outbreak before it can re-contain).
//!
//! Inert unless `World::disease_enabled`.

use super::*;
use crate::disease::{OUTBREAK_FRACTION, OUTBREAK_MIN_POP, OUTBREAK_REARM};

pub(super) fn detect_epidemic(world: &mut World, agg: &SpeciesAggTable) {
    if !world.disease_enabled {
        return;
    }
    let tick = world.tick;
    let medicine_bit = crate::invention::MEDICINE;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let Some(e) = agg.get(sid) else { continue };
        if e.count < OUTBREAK_MIN_POP {
            // Too small to outbreak-detect: drop any stale latch so a later
            // recovery doesn't read as a wave resolving.
            world.codex.epidemic_latched.remove(&sid);
            continue;
        }
        let frac = e.infected_count as f32 / e.count as f32;
        let latched = world.codex.epidemic_latched.contains(&sid);
        if !latched && frac >= OUTBREAK_FRACTION {
            world.codex.epidemic_latched.insert(sid);
            let (lx, ly) = centroid_of(agg, sid);
            to_push.push(CodexEvent {
                event_type: EventType::EpidemicOutbreak,
                tick,
                species_id: sid,
                value: frac,
                loc_x: lx,
                loc_y: ly,
            });
        } else if latched && frac < OUTBREAK_REARM {
            world.codex.epidemic_latched.remove(&sid);
            let adoption = e.invention_counts[medicine_bit] as f32 / e.count as f32;
            if adoption >= 0.5 {
                let (lx, ly) = centroid_of(agg, sid);
                to_push.push(CodexEvent {
                    event_type: EventType::MedicineContainment,
                    tick,
                    species_id: sid,
                    value: adoption,
                    loc_x: lx,
                    loc_y: ly,
                });
            }
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crowded_world(seed: u64, n: usize, disease: bool) -> World {
        let mut w = World::new(seed);
        w.disease_enabled = disease;
        for k in 0..n {
            w.spawn_agent(
                crate::prelude::Vec2::new(500.0 + k as f32 * 0.5, 500.0),
                crate::genome::Genome::neutral(),
            );
        }
        w
    }

    fn infect_all(w: &mut World, intensity: f32) {
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        for id in ids {
            w.agents.infection[id as usize] = intensity;
        }
    }

    fn count_events(w: &World, t: EventType) -> usize {
        w.codex.events.iter().filter(|e| e.event_type == t).count()
    }

    #[test]
    fn outbreak_fires_and_rearms() {
        let mut w = crowded_world(3, 40, true);
        let mut agg = SpeciesAggTable::default();
        agg.build(&w);
        detect_epidemic(&mut w, &agg);
        assert_eq!(count_events(&w, EventType::EpidemicOutbreak), 0, "healthy: no event");

        infect_all(&mut w, 0.6); // frac = 0.6 ≥ OUTBREAK_FRACTION
        agg.build(&w);
        detect_epidemic(&mut w, &agg);
        assert_eq!(count_events(&w, EventType::EpidemicOutbreak), 1, "crossing fires once");

        // Latched: sustained high fraction does not refire.
        agg.build(&w);
        detect_epidemic(&mut w, &agg);
        assert_eq!(count_events(&w, EventType::EpidemicOutbreak), 1, "latched: no refire");

        // Re-arm below OUTBREAK_REARM (no medicine held → no containment).
        infect_all(&mut w, 0.05);
        agg.build(&w);
        detect_epidemic(&mut w, &agg);
        assert_eq!(count_events(&w, EventType::EpidemicOutbreak), 1);
        assert_eq!(count_events(&w, EventType::MedicineContainment), 0);

        // Second wave re-fires.
        infect_all(&mut w, 0.6);
        agg.build(&w);
        detect_epidemic(&mut w, &agg);
        assert_eq!(count_events(&w, EventType::EpidemicOutbreak), 2, "re-armed wave re-fires");
    }

    #[test]
    fn containment_needs_medicine_adoption() {
        let mut w = crowded_world(4, 40, true);
        w.inventions_enabled = true; // agg.invention_counts only accumulates when the tree is on
        infect_all(&mut w, 0.6);
        let mut agg = SpeciesAggTable::default();
        agg.build(&w);
        detect_epidemic(&mut w, &agg);
        assert_eq!(count_events(&w, EventType::EpidemicOutbreak), 1);

        // Give every member a held Medicine channel.
        let ch = crate::invention::INVENTION_CHANNEL_BASE + crate::invention::MEDICINE;
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        for id in ids {
            w.agents.meme_vector[id as usize][ch] = crate::invention::HELD_THRESHOLD;
            w.agents.infection[id as usize] = 0.05; // wave resolved
        }
        agg.build(&w);
        detect_epidemic(&mut w, &agg);
        assert_eq!(count_events(&w, EventType::MedicineContainment), 1);
    }

    #[test]
    fn flag_off_is_inert() {
        let mut w = crowded_world(5, 40, false);
        infect_all(&mut w, 0.6);
        let mut agg = SpeciesAggTable::default();
        agg.build(&w);
        detect_epidemic(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "flag off: never fires");
    }
}
