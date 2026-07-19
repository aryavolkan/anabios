//! Codex practice detectors: `PracticeDiscovered` (first holder anywhere,
//! latched globally per practice) and `PracticeAdopted` (≥50% of a species
//! holds it, latched per species with re-arm on drop below threshold). Mirrors
//! the invention detectors; gated on `World::cognition_enabled`.

use super::*;
use crate::world::World;

/// Minimum species size before adoption percentages are meaningful.
pub const ADOPT_MIN_MEMBERS: u32 = 5;
/// Species-level penetration fraction at/above which `PracticeAdopted` fires.
pub const ADOPT_THRESHOLD: f32 = 0.5;

pub(super) fn detect_practices(world: &mut World, agg: &SpeciesAggTable) {
    if !world.cognition_enabled {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();

    // PracticeDiscovered: first holder anywhere (scan members in ascending id
    // order via the agg table so the recorded location is deterministic).
    for p in 0..crate::practice::PRACTICE_COUNT {
        if world.codex.practices_discovered.contains(&(p as u8)) {
            continue;
        }
        'found: for &sid in agg.active() {
            let entry = agg.get(sid).expect("active species has an entry");
            if entry.practice_counts[p] == 0 {
                continue;
            }
            for &i in &entry.member_idx {
                if crate::practice::has(&world.agents.meme_vector[i], p) {
                    let pos = world.agents.position[i];
                    to_push.push(CodexEvent {
                        event_type: EventType::PracticeDiscovered,
                        tick,
                        species_id: sid,
                        value: p as f32,
                        loc_x: pos.x,
                        loc_y: pos.y,
                    });
                    world.codex.practices_discovered.insert(p as u8);
                    break 'found;
                }
            }
        }
    }

    // PracticeAdopted: per-species ≥50% penetration, edge-triggered with re-arm
    // when penetration falls back below the threshold.
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        if entry.count < ADOPT_MIN_MEMBERS {
            continue;
        }
        let (lx, ly) = centroid_of(agg, sid);
        for p in 0..crate::practice::PRACTICE_COUNT {
            let key = (sid, p as u8);
            let frac = entry.practice_counts[p] as f32 / entry.count as f32;
            let fired = frac >= ADOPT_THRESHOLD;
            if !fired {
                world.codex.practices_adopted.remove(&key);
            } else if world.codex.practices_adopted.insert(key) {
                to_push.push(CodexEvent {
                    event_type: EventType::PracticeAdopted,
                    tick,
                    species_id: sid,
                    value: p as f32,
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
