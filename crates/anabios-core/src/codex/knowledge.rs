//! Codex knowledge detector (E14): `KnowledgeRatchet`.
//!
//! v1 semantics (deliberately narrower than the original plan wording): this
//! fires **once per species**, latched via `knowledge_ratchet_fired`, the
//! first tick `knowledge_by_species[sp] >= KNOWLEDGE_RATCHET_MIN` — i.e. it
//! marks "this culture built durable, transmissible knowledge" (a threshold
//! crossing), not "this culture re-acquired an invention it had lost after a
//! bottleneck." The causal re-acquisition version would need per-species
//! held-invention *history* (what was held before vs. after a gap), which is
//! another serialized `CodexState` field and an FORMAT_VERSION bump — deferred
//! per the task-4 brief's measure-before-plan guidance. This detector adds no
//! new serialized state: it only reads/writes the two `CodexState` fields
//! already introduced in Task 2 (`knowledge_by_species`,
//! `knowledge_ratchet_fired`).
//!
//! Inert unless `World::knowledge_enabled`.

use super::*;
use crate::knowledge::KNOWLEDGE_RATCHET_MIN;

pub(super) fn detect_knowledge_ratchet(world: &mut World, agg: &SpeciesAggTable) {
    if !world.knowledge_enabled {
        return;
    }
    let tick = world.tick;
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for &sid in agg.active() {
        let Some(&level) = world.codex.knowledge_by_species.get(&sid) else {
            continue;
        };
        if level >= KNOWLEDGE_RATCHET_MIN && world.codex.knowledge_ratchet_fired.insert(sid) {
            let (lx, ly) = centroid_of(agg, sid);
            to_push.push(CodexEvent {
                event_type: EventType::KnowledgeRatchet,
                tick,
                species_id: sid,
                value: level,
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

    #[test]
    fn fires_once_per_species_when_threshold_crossed() {
        let mut w = World::new(3);
        w.knowledge_enabled = true;
        let mut agg = SpeciesAggTable::default();
        // No species active yet: build() with no agents leaves agg.active()
        // empty, so hand-roll the aggregate table's active set via a spawned
        // agent to give the detector something to iterate.
        let id = w
            .spawn_agent(crate::prelude::Vec2::new(500.0, 500.0), crate::genome::Genome::neutral());
        let sp = w.agents.species_id[id as usize];
        agg.build(&w);

        w.codex.knowledge_by_species.insert(sp, KNOWLEDGE_RATCHET_MIN - 0.01);
        detect_knowledge_ratchet(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "below threshold: no event");

        w.codex.knowledge_by_species.insert(sp, KNOWLEDGE_RATCHET_MIN);
        detect_knowledge_ratchet(&mut w, &agg);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::KnowledgeRatchet).count(),
            1,
            "threshold crossed: fires exactly once"
        );

        // Latched: staying above threshold does not refire.
        detect_knowledge_ratchet(&mut w, &agg);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::KnowledgeRatchet).count(),
            1,
            "latched: no refire while sustained"
        );
    }

    #[test]
    fn flag_off_is_inert() {
        let mut w = World::new(3);
        w.knowledge_enabled = false;
        let id = w
            .spawn_agent(crate::prelude::Vec2::new(500.0, 500.0), crate::genome::Genome::neutral());
        let sp = w.agents.species_id[id as usize];
        w.codex.knowledge_by_species.insert(sp, 1.0);
        let mut agg = SpeciesAggTable::default();
        agg.build(&w);
        detect_knowledge_ratchet(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "flag off: never fires");
    }
}
