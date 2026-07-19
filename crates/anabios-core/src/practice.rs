//! Maladaptive cultural practices — the antagonist half of the cognitive
//! gene–culture system. Practices are memes: they ride their own meme-channel
//! block above the invention tree, are discovered by inventive agents, and
//! spread by the same payoff-blind copy-toward-best rule as inventions. But
//! unlike inventions they carry **no buff** — only a reproductive/genetic
//! fitness cost paid in `reproduce`. Because social copying is payoff-blind, a
//! harmful practice can invade a neighbourhood before selection punishes its
//! carriers; the tug-of-war between that spread and the fitness cost is the
//! gene↔culture conflict.
//!
//! The whole mechanism is gated on `World::cognition_enabled`: with the flag
//! off every practice channel stays 0.0, no RNG is drawn, and no reproductive
//! effect applies, so non-cognition scenarios are byte-identical (modulo the
//! one-time meme-vector layout growth).

use crate::genome::{Genome, GenomeSlot};
use crate::invention::{DISCOVERY_CAP, HELD_THRESHOLD, INVENTION_CHANNEL_BASE, INVENTION_COUNT};
use crate::module::{self, ModuleType};
use crate::program::MEME_CHANNELS;
use crate::world::World;

/// Number of maladaptive practices.
pub const PRACTICE_COUNT: usize = 2;

/// First meme channel owned by the practice block — directly above the
/// invention tree. Channels `PRACTICE_CHANNEL_BASE .. MEME_CHANNELS` are
/// practices.
pub const PRACTICE_CHANNEL_BASE: usize = INVENTION_CHANNEL_BASE + INVENTION_COUNT;

/// Compile-time layout check: the practice block must fit in the meme vector.
const _: () = assert!(PRACTICE_CHANNEL_BASE + PRACTICE_COUNT <= MEME_CHANNELS);

/// Inbreeding: a kin-mating custom that expresses genetic load — close-kin
/// pairings yield frail offspring (inbreeding depression).
pub const INBREEDING: usize = 0;
/// Child sacrifice: holders cull a fraction of their own newborns.
pub const CHILD_SACRIFICE: usize = 1;

/// Static per-practice metadata (display / codex / demo).
pub struct Practice {
    pub name: &'static str,
    pub key: &'static str,
    pub debuff: &'static str,
}

pub const PRACTICES: [Practice; PRACTICE_COUNT] = [
    Practice { name: "Inbreeding", key: "inbreeding", debuff: "inbreeding depression" },
    Practice { name: "Child Sacrifice", key: "child_sacrifice", debuff: "culls newborns" },
];

// --- Tuning ------------------------------------------------------------------

/// Realized-IQ needed to acquire (discover or copy) any practice. Low — anyone
/// can catch a bad habit, which is what makes low-cognition lineages vulnerable
/// to maladaptive culture without access to the compensating high-era tech.
pub const PRACTICE_IQ_REQ: f32 = 0.10;
/// Base per-Communicator per-tick discovery probability (per open candidate),
/// scaled by Openness. Same order as an invention era-1 roll.
pub const PRACTICE_BASE_DISCOVERY: f32 = 3e-5;
/// Per-tick lerp rate toward the best-holding neighbour's level (mirrors the
/// invention spread rate).
pub const PRACTICE_SPREAD_RATE: f32 = 0.03;

/// Child sacrifice: probability a holder's newborn is culled at birth.
pub const CHILD_SACRIFICE_CULL: f32 = 0.5;
/// Inbreeding depression: offspring energy is scaled by `1 - INBREEDING_DEPRESSION
/// * closeness`, where closeness rises from 0 to 1 as the parents' genome
/// distance falls from `INBREEDING_DIST` to 0.
pub const INBREEDING_DEPRESSION: f32 = 0.5;
/// Genome distance below which inbreeding depression starts to bite.
pub const INBREEDING_DIST: f32 = 0.15;

// --- Channel helpers ---------------------------------------------------------

/// The meme channel carrying practice `p`'s adoption level.
#[inline]
pub const fn channel(p: usize) -> usize {
    PRACTICE_CHANNEL_BASE + p
}

/// `true` iff the channel is owned by the practice block (excluded from the
/// generic broadcast-mean meme lerp and from MemeSweep, like invention channels).
#[inline]
pub const fn is_practice_channel(ch: usize) -> bool {
    ch >= PRACTICE_CHANNEL_BASE && ch < PRACTICE_CHANNEL_BASE + PRACTICE_COUNT
}

/// Adoption level of practice `p` in a meme vector.
#[inline]
pub fn level(meme: &[f32; MEME_CHANNELS], p: usize) -> f32 {
    meme[channel(p)]
}

/// `true` iff the meme vector functionally holds practice `p`.
#[inline]
pub fn has(meme: &[f32; MEME_CHANNELS], p: usize) -> bool {
    level(meme, p) >= HELD_THRESHOLD
}

/// Whether an agent with realized `iq` may acquire a practice (no-op gate when
/// cognition is disabled — but callers already gate the whole mechanism on it).
#[inline]
pub fn iq_permits(iq: f32, cognition_enabled: bool) -> bool {
    !cognition_enabled || iq >= PRACTICE_IQ_REQ
}

/// Inbreeding-depression closeness in `[0,1]`: 1 when the parents are
/// genetically identical, falling linearly to 0 at `INBREEDING_DIST` apart.
pub fn inbreeding_closeness(a: &Genome, b: &Genome) -> f32 {
    (1.0 - a.distance(b) / INBREEDING_DIST).clamp(0.0, 1.0)
}

// --- Discovery ---------------------------------------------------------------

/// Per-tick practice-discovery stage (tick stage after `invention_step`, gated
/// on `cognition_enabled`). Each Communicator whose realized IQ clears
/// `PRACTICE_IQ_REQ` rolls once to invent a not-yet-held practice — an inventive
/// lineage can stumble onto a bad custom as readily as a good tech. No-op /
/// zero RNG when the flag is off.
pub fn discover_step(world: &mut World) {
    if !world.cognition_enabled {
        return;
    }
    let mut ids = std::mem::take(&mut world.agents.scratch_ids);
    ids.clear();
    ids.extend(world.agents.iter_alive());
    for &id in &ids {
        let i = id as usize;
        if !module::has(&world.agents.modules[i], ModuleType::Communicator) {
            continue;
        }
        if world.agents.iq[i] < PRACTICE_IQ_REQ {
            continue;
        }
        let openness = world.agents.genome[i].get(GenomeSlot::Openness);
        let mut total = 0.0f32;
        let mut probs = [0.0f32; PRACTICE_COUNT];
        for p in 0..PRACTICE_COUNT {
            if has(&world.agents.meme_vector[i], p) {
                continue;
            }
            let pr = (PRACTICE_BASE_DISCOVERY * openness).min(DISCOVERY_CAP);
            probs[p] = pr;
            total += pr;
        }
        if total <= 0.0 {
            continue;
        }
        let total = total.min(DISCOVERY_CAP);
        let r = world.rng.f32_unit();
        if r < total {
            let mut acc = 0.0f32;
            let mut picked = usize::MAX;
            for (p, &pr) in probs.iter().enumerate() {
                acc += pr;
                if r < acc {
                    picked = p;
                    break;
                }
            }
            if picked != usize::MAX {
                world.agents.meme_vector[i][channel(picked)] = 1.0;
            }
        }
    }
    world.agents.scratch_ids = ids;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn practice_channels_sit_above_the_invention_block() {
        assert_eq!(PRACTICE_CHANNEL_BASE, INVENTION_CHANNEL_BASE + INVENTION_COUNT);
        assert!(!is_practice_channel(PRACTICE_CHANNEL_BASE - 1));
        assert!(is_practice_channel(channel(INBREEDING)));
        assert!(is_practice_channel(channel(CHILD_SACRIFICE)));
        assert!(!is_practice_channel(MEME_CHANNELS));
        // The two blocks are disjoint.
        for p in 0..PRACTICE_COUNT {
            assert!(!crate::invention::is_invention_channel(channel(p)));
        }
    }

    #[test]
    fn iq_gate_is_off_when_cognition_disabled() {
        assert!(iq_permits(0.0, false));
        assert!(!iq_permits(PRACTICE_IQ_REQ - 0.01, true));
        assert!(iq_permits(PRACTICE_IQ_REQ, true));
    }

    #[test]
    fn inbreeding_closeness_peaks_for_identical_parents() {
        let a = Genome::neutral();
        assert_eq!(inbreeding_closeness(&a, &a), 1.0, "identical parents = max depression");
        let mut b = Genome::neutral();
        b.set(GenomeSlot::Size, 1.0); // push distance past INBREEDING_DIST
        assert_eq!(inbreeding_closeness(&a, &b), 0.0, "distant parents = no depression");
    }
}
