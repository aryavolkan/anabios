//! Codex invention detectors: `InventionDiscovered` (first holder anywhere,
//! latched globally per invention) and `InventionAdopted` (≥50% of a species
//! holds it, latched per species with re-arm on drop below threshold).

use super::*;
use crate::world::World;

/// Minimum species size before adoption percentages are meaningful (mirrors
/// the MemeSweep detector's floor).
pub const ADOPT_MIN_MEMBERS: u32 = 5;
/// Species-level adoption fraction at/above which `InventionAdopted` fires.
pub const ADOPT_THRESHOLD: f32 = 0.5;

pub(super) fn detect_inventions(world: &mut World, agg: &SpeciesAggTable) {
    if !world.inventions_enabled {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();

    // InventionDiscovered: first holder anywhere (scan members in ascending
    // id order via the agg table so the recorded location is deterministic).
    for k in 0..crate::invention::INVENTION_COUNT {
        if world.codex.inventions_discovered.contains(&(k as u8)) {
            continue;
        }
        'found: for &sid in agg.active() {
            let entry = agg.get(sid).expect("active species has an entry");
            if entry.invention_counts[k] == 0 {
                continue;
            }
            for &i in &entry.member_idx {
                if crate::invention::has(&world.agents.meme_vector[i], k) {
                    let pos = world.agents.position[i];
                    to_push.push(CodexEvent {
                        event_type: EventType::InventionDiscovered,
                        tick,
                        species_id: sid,
                        value: k as f32,
                        loc_x: pos.x,
                        loc_y: pos.y,
                    });
                    world.codex.inventions_discovered.insert(k as u8);
                    break 'found;
                }
            }
        }
    }

    // InventionAdopted: per-species ≥50% penetration, edge-triggered with
    // re-arm when adoption falls back below the threshold.
    for &sid in agg.active() {
        let entry = agg.get(sid).expect("active species has an entry");
        if entry.count < ADOPT_MIN_MEMBERS {
            continue;
        }
        let (lx, ly) = centroid_of(agg, sid);
        for k in 0..crate::invention::INVENTION_COUNT {
            let key = (sid, k as u8);
            let frac = entry.invention_counts[k] as f32 / entry.count as f32;
            let fired = frac >= ADOPT_THRESHOLD;
            if !fired {
                world.codex.inventions_adopted.remove(&key);
            } else if world.codex.inventions_adopted.insert(key) {
                to_push.push(CodexEvent {
                    event_type: EventType::InventionAdopted,
                    tick,
                    species_id: sid,
                    value: k as f32,
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
