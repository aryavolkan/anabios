//! Per-species durable tech memory (E14): knowledge rises while any live
//! member of a species holds Writing, and decays slowly otherwise. This
//! turns Writing from a one-off invention into a ratchet — a species that
//! loses its Writing holders (bottleneck, extinction of the holder lineage)
//! still retains a fading trace of accumulated knowledge, which can later
//! feed a re-discovery bonus into the invention step (integration point
//! documented, not necessarily wired in v1).
//!
//! Everything here is gated on `World::knowledge_enabled`: with the flag off
//! the tick stage early-returns with zero state change and zero RNG draws,
//! so every pre-E14 scenario stays byte-identical. `knowledge_enabled`
//! requires `inventions_enabled` (enforced at scenario-parse time), since
//! Writing must exist for there to be anything to track.

use crate::world::World;

/// Per-tick knowledge gain for a species with a live Writing-holder.
pub const KNOWLEDGE_GAIN: f32 = 0.002;
/// Per-tick multiplicative knowledge decay for a species with no
/// Writing-holder this tick.
pub const KNOWLEDGE_DECAY: f32 = 0.0005;
/// Knowledge accumulation ceiling.
pub const KNOWLEDGE_MAX: f32 = 1.0;
/// Threshold at which `KnowledgeRatchet` fires (once per species, latched via
/// `CodexState.knowledge_ratchet_fired`): a species crossing this level has
/// built durable, transmissible knowledge. See `codex::knowledge` for the v1
/// vs. deferred-causal-re-acquisition detector design note.
pub const KNOWLEDGE_RATCHET_MIN: f32 = 0.5;

/// Tick stage: rises `knowledge_by_species[sp]` for every species with a live
/// Writing-holder this tick (capped at `KNOWLEDGE_MAX`), decays it slowly for
/// every other species already tracked. RNG-free. No-op when the flag is off.
///
/// Note the intentional one-tick seeding delay: a species first seen holding
/// Writing is `or_insert(0.0)`-ed *after* the gain/decay loop above, so its
/// first tick with a Writing-holder seeds the entry at 0.0 and it only starts
/// accruing `KNOWLEDGE_GAIN` from the following tick. This is deterministic
/// and pinned by the `writer_species_knowledge_grows` test below — don't
/// "fix" the ordering, or the golden hashes move.
pub fn knowledge_step(world: &mut World) {
    if !world.knowledge_enabled {
        return;
    }
    // Species with a live Writing-holder this tick (BTreeSet for
    // deterministic iteration order below).
    let writing = crate::invention::bit(crate::invention::WRITING);
    let mut has_writer: std::collections::BTreeSet<u32> = Default::default();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        if crate::invention::held_mask(&world.agents.meme_vector[i]) & writing != 0 {
            has_writer.insert(world.agents.species_id[i]);
        }
    }
    let k = &mut world.codex.knowledge_by_species;
    for (sp, v) in k.iter_mut() {
        if has_writer.contains(sp) {
            *v = (*v + KNOWLEDGE_GAIN).min(KNOWLEDGE_MAX);
        } else {
            *v *= 1.0 - KNOWLEDGE_DECAY;
        }
    }
    for sp in has_writer {
        k.entry(sp).or_insert(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;

    /// A single Writing-holding agent of species 1 on a fresh world with the
    /// flag on (mirrors `domestication.rs`'s `herder_and_calf` helper).
    fn writer_world() -> (World, crate::agent::AgentId) {
        let mut w = World::new(5);
        w.inventions_enabled = true;
        w.knowledge_enabled = true;
        w.species_centroids.push(Genome::neutral());
        w.species_parents.push(Some(0));
        let writer = w.spawn_seeded(
            Vec2::new(500.0, 500.0),
            Genome::neutral(),
            1,
            crate::module::starter_kit(),
            crate::program::starter_grazer(),
        );
        w.agents.meme_vector[writer as usize]
            [crate::invention::channel(crate::invention::WRITING)] = 1.0;
        (w, writer)
    }

    #[test]
    fn writer_species_knowledge_grows() {
        let (mut w, writer) = writer_world();
        let sp = w.agents.species_id[writer as usize];
        // First tick: the species has no prior `knowledge_by_species` entry,
        // so it is only seeded at 0.0 this tick (the gain loop only touches
        // pre-existing entries; the writer-seed happens after it).
        knowledge_step(&mut w);
        let after_one = *w.codex.knowledge_by_species.get(&sp).unwrap();
        assert!((after_one - 0.0).abs() < 1e-6, "first tick seeds the entry at 0.0");
        // Subsequent ticks: the now-existing entry accumulates gain.
        knowledge_step(&mut w);
        let after_two = *w.codex.knowledge_by_species.get(&sp).unwrap();
        assert!((after_two - KNOWLEDGE_GAIN).abs() < 1e-6, "second tick gains");
        knowledge_step(&mut w);
        let after_three = *w.codex.knowledge_by_species.get(&sp).unwrap();
        assert!((after_three - 2.0 * KNOWLEDGE_GAIN).abs() < 1e-6, "third tick keeps accumulating");
    }

    #[test]
    fn non_writer_species_knowledge_decays() {
        let (mut w, writer) = writer_world();
        let sp = w.agents.species_id[writer as usize];
        w.codex.knowledge_by_species.insert(sp, 0.5);
        // Remove Writing so the species has no holder this tick.
        w.agents.meme_vector[writer as usize]
            [crate::invention::channel(crate::invention::WRITING)] = 0.0;
        knowledge_step(&mut w);
        let v = *w.codex.knowledge_by_species.get(&sp).unwrap();
        assert!((v - 0.5 * (1.0 - KNOWLEDGE_DECAY)).abs() < 1e-6, "no holder: decays");
    }

    #[test]
    fn flag_off_is_inert() {
        let (mut w, _writer) = writer_world();
        w.knowledge_enabled = false;
        let hash_before = crate::snapshot::state_hash(&w);
        knowledge_step(&mut w);
        assert!(w.codex.knowledge_by_species.is_empty(), "flag off: no state written");
        assert_eq!(
            crate::snapshot::state_hash(&w),
            hash_before,
            "flag off: zero state change, zero RNG draws"
        );
    }
}
