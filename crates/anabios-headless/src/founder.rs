//! Lineage-locked founder tags for O2 invasion analysis.
//!
//! The raw cultural/asocial key (Communicator-module presence) is re-read every
//! tick and the module is recombined/mutated at every birth, so it drifts — O1
//! showed this confounds the "who is cultural" readout. This tracker fixes each
//! lineage's tag at t0 (by initial module presence) and inherits it through
//! `parent_ids`, so it reports which founding population an agent DESCENDS from,
//! immune to later module mutation. Headless-only; reads `&World`, never mutates.

use std::collections::HashMap;

use anabios_core::agent::{LineageId, LINEAGE_NONE};
use anabios_core::module::{self, ModuleType};
use anabios_core::world::World;

use crate::ledger::{sample_strategies_by, StrategyKind, StrategyStat};

fn module_kind(world: &World, i: usize) -> StrategyKind {
    if module::has(&world.agents.modules[i], ModuleType::Communicator) {
        StrategyKind::Cultural
    } else {
        StrategyKind::Asocial
    }
}

/// Maps each lineage id to the founding strategy it descends from.
pub struct FounderTracker {
    tag: HashMap<LineageId, StrategyKind>,
}

/// Seed founder tags from the initial population's module presence. Call once on
/// the freshly-instantiated world (before any `step`).
pub fn init(world: &World) -> FounderTracker {
    let mut tag = HashMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        tag.insert(world.agents.lineage_id[i], module_kind(world, i));
    }
    FounderTracker { tag }
}

impl FounderTracker {
    /// Tag any newly-appeared lineage with its mother's founder tag. Call once
    /// per tick: a birth's mother was tagged on a prior tick and lineage ids are
    /// never reused, so the map only grows and every new lineage finds its
    /// parent. Fallback (untracked mother / founder with no parent): current
    /// module presence.
    pub fn observe(&mut self, world: &World) {
        for id in world.agents.iter_alive() {
            let i = id as usize;
            let lid = world.agents.lineage_id[i];
            if self.tag.contains_key(&lid) {
                continue;
            }
            let mother = world.agents.parent_ids[i][0];
            let kind = if mother != LINEAGE_NONE {
                self.tag.get(&mother).copied()
            } else {
                None
            }
            .unwrap_or_else(|| module_kind(world, i));
            self.tag.insert(lid, kind);
        }
    }

    /// The lineage-locked tag for alive agent slot `i` (falls back to module
    /// presence for any untracked lineage — shouldn't happen if `observe` ran).
    pub fn kind_of(&self, world: &World, i: usize) -> StrategyKind {
        self.tag
            .get(&world.agents.lineage_id[i])
            .copied()
            .unwrap_or_else(|| module_kind(world, i))
    }
}

/// Per-strategy aggregate keyed on the lineage-locked founder tag instead of the
/// per-tick module readout. Index 0 = Cultural-descended, 1 = Asocial-descended.
pub fn sample_by_founder(world: &World, tracker: &FounderTracker) -> [StrategyStat; 2] {
    sample_strategies_by(world, |w, i| tracker.kind_of(w, i))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anabios_core::scenario::Scenario;
    use anabios_core::tick::step;

    const MIX: &str = "\
name = \"t\"
seed = 3
[[agents]]
count = 4
archetype = \"asocial_forager\"
placement = { kind = \"uniform\" }
[[agents]]
count = 2
archetype = \"communicator\"
placement = { kind = \"uniform\" }
";

    #[test]
    fn init_tags_founders_by_module_presence() {
        let world = Scenario::parse_toml(MIX).unwrap().instantiate();
        let t = init(&world);
        let stats = sample_by_founder(&world, &t);
        assert_eq!(stats[0].count, 2, "two communicator founders → Cultural-descended");
        assert_eq!(stats[1].count, 4, "four asocial founders → Asocial-descended");
    }

    #[test]
    fn descendants_keep_founder_tag_even_if_module_mutates() {
        // All-asocial-founder world. Run it; every descendant MUST remain
        // Asocial-descended by founder tag, regardless of any Communicator
        // module a birth mutates in. If module mutation ever produces a
        // Communicator (module readout disagrees), that disagreement proves the
        // lineage-locked tag is doing its job.
        const ASOCIAL: &str = "\
name = \"t\"
seed = 7
[[agents]]
count = 40
archetype = \"asocial_forager\"
placement = { kind = \"uniform\" }
";
        let mut world = Scenario::parse_toml(ASOCIAL).unwrap().instantiate();
        let mut t = init(&world);
        for _ in 0..1500 {
            step(&mut world);
            t.observe(&world);
        }
        let by_founder = sample_by_founder(&world, &t);
        let by_module = crate::ledger::sample_strategies(&world);
        // Invariant: no lineage descends from a Cultural founder here.
        assert_eq!(by_founder[0].count, 0, "no Cultural-descended lineages exist");
        // If the module readout found any Communicator (mutation), the two tags
        // must disagree — which is the whole point of the lineage-locked tag.
        if by_module[0].count > 0 {
            assert_ne!(
                by_module[0].count, by_founder[0].count,
                "module tag drifted via mutation; founder tag held"
            );
        }
    }
}
