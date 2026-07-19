//! Cultural invention tree: a cumulative technology layer riding ON the meme
//! substrate (design §4.4 extension). Inventions ARE memes: each invention
//! owns one meme channel (`INVENTION_CHANNEL_BASE + id`), holding a
//! continuous adoption level in `[0,1]` — discovered by individual
//! Communicator agents (innovation roll gated on Openness + learned foraging
//! skill), spread between Communicator neighbours inside `culture_step`
//! (copy-toward-best, like the skill channel), vertically inherited by
//! `inherit_meme`, and sensed by programs via `SenseMeme`. A level at or
//! above `HELD_THRESHOLD` means the agent functionally "holds" the invention
//! and its buffs AND debuffs apply (read by `interact`, `integrate`, `age`,
//! `reproduce`, `sense`, module upkeep, and biome pollution). A species
//! "adopts" an invention when ≥ half its members hold it (codex
//! `InventionAdopted`).
//!
//! The whole mechanism is gated on `World::inventions_enabled`; with the flag
//! off every invention channel stays 0.0, every multiplier below is exactly
//! 1.0, and no RNG draws are consumed, so baseline scenarios stay unchanged.

use crate::module::{self, ModuleType};
use crate::program::MEME_CHANNELS;
use crate::world::World;

/// Number of inventions in the tree.
pub const INVENTION_COUNT: usize = 10;

/// First meme channel owned by the invention tree. Channels below this keep
/// their pre-existing meanings (alarm, dialects, cooperation norm, hunt
/// technique, skill, DIT technique); channels `INVENTION_CHANNEL_BASE ..
/// MEME_CHANNELS` are the invention tree.
pub const INVENTION_CHANNEL_BASE: usize = 8;

/// Compile-time layout check: the tree must fit in the meme vector.
const _: () = assert!(INVENTION_CHANNEL_BASE + INVENTION_COUNT <= MEME_CHANNELS);

pub const STONE_TOOLS: usize = 0;
pub const FIRE: usize = 1;
pub const FARMING: usize = 2;
pub const METALWORKING: usize = 3;
pub const WRITING: usize = 4;
pub const MEDICINE: usize = 5;
pub const HUSBANDRY: usize = 6;
pub const MACHINERY: usize = 7;
pub const ELECTRICITY: usize = 8;
pub const NUCLEAR_POWER: usize = 9;

/// Adoption level at/above which an invention is functionally held (buffs and
/// debuffs apply, prereqs count as satisfied, codex counts it).
pub const HELD_THRESHOLD: f32 = 0.5;

/// Static per-invention metadata. Effect magnitudes live in the constants
/// below (kept separate so the table stays display-friendly for the headless
/// demo and the Godot inspector).
pub struct Invention {
    /// Display name ("Stone Tools").
    pub name: &'static str,
    /// Machine key ("stone_tools") — used in coevo series keys / JSONL.
    pub key: &'static str,
    /// Era 1..=4; harder to discover in later eras and used for the tech-era
    /// display.
    pub era: u8,
    /// Bitmask of invention ids that must be held (level ≥ `HELD_THRESHOLD`)
    /// before this one can be discovered or copied.
    pub prereqs: u32,
    /// One-line upside summary (UI).
    pub buff: &'static str,
    /// One-line downside summary (UI).
    pub debuff: &'static str,
}

#[inline]
pub const fn bit(inv: usize) -> u32 {
    1u32 << inv
}

pub const INVENTIONS: [Invention; INVENTION_COUNT] = [
    Invention {
        name: "Stone Tools",
        key: "stone_tools",
        era: 1,
        prereqs: 0,
        buff: "+25% graze bite",
        debuff: "none",
    },
    Invention {
        name: "Fire",
        key: "fire",
        era: 1,
        prereqs: bit(STONE_TOOLS),
        buff: "+40% energy per biomass",
        debuff: "+10% metabolism",
    },
    Invention {
        name: "Farming",
        key: "farming",
        era: 2,
        prereqs: bit(FIRE),
        buff: "+60% graze yield",
        debuff: "crowding stress",
    },
    Invention {
        name: "Metalworking",
        key: "metalworking",
        era: 2,
        prereqs: bit(FIRE),
        buff: "+50% weapon damage",
        debuff: "+10% module upkeep",
    },
    Invention {
        name: "Writing",
        key: "writing",
        era: 3,
        prereqs: bit(FARMING),
        buff: "2x meme + invention spread",
        debuff: "small upkeep",
    },
    Invention {
        name: "Medicine",
        key: "medicine",
        era: 3,
        prereqs: bit(WRITING),
        buff: "+50% lifespan",
        debuff: "small upkeep",
    },
    Invention {
        name: "Husbandry",
        key: "husbandry",
        era: 3,
        prereqs: bit(FARMING),
        buff: "+40% scavenge energy",
        debuff: "+8% metabolism",
    },
    Invention {
        name: "Machinery",
        key: "machinery",
        era: 4,
        prereqs: bit(METALWORKING) | bit(WRITING),
        buff: "+25% speed & bite",
        debuff: "pollutes local biome",
    },
    Invention {
        name: "Electricity",
        key: "electricity",
        era: 4,
        prereqs: bit(MACHINERY),
        buff: "+30% perception, 1.5x discovery",
        debuff: "upkeep",
    },
    Invention {
        name: "Nuclear Power",
        key: "nuclear_power",
        era: 4,
        prereqs: bit(ELECTRICITY),
        buff: "flat energy income",
        debuff: "1.5x child mutation + upkeep",
    },
];

// --- Effect magnitudes -----------------------------------------------------

/// Stone Tools: graze-bite bonus.
pub const STONE_TOOLS_BITE: f32 = 0.25;
/// Fire: energy-per-biomass bonus; extra basal metabolism fraction.
pub const FIRE_ENERGY: f32 = 0.40;
pub const FIRE_METABOLISM: f32 = 0.10;
/// Farming: graze-bite bonus; energy drained per tick per crowding neighbour
/// above the free allowance (sedentary density stress).
pub const FARMING_BITE: f32 = 0.60;
pub const FARMING_CROWDING_FREE: u32 = 8;
pub const FARMING_STRESS_PER_NEIGHBOR: f32 = 0.002;
/// Metalworking: weapon-damage bonus; extra module upkeep fraction.
pub const METALWORKING_DAMAGE: f32 = 0.50;
pub const METALWORKING_UPKEEP: f32 = 0.10;
/// Writing: multiplier on meme copy rate and invention spread rate; small
/// flat per-tick upkeep.
pub const WRITING_SPREAD_MULT: f32 = 2.0;
pub const WRITING_UPKEEP: f32 = 0.003;
/// Medicine: lifespan bonus; small flat per-tick upkeep.
pub const MEDICINE_LIFESPAN: f32 = 0.50;
pub const MEDICINE_UPKEEP: f32 = 0.003;
/// Husbandry: scavenge-energy bonus; extra basal metabolism fraction.
pub const HUSBANDRY_SCAVENGE: f32 = 0.40;
pub const HUSBANDRY_METABOLISM: f32 = 0.08;
/// Machinery: speed + graze-bite bonuses; pollution deposited into the local
/// biome cell per tick (regrowth penalty, decays per biome step).
pub const MACHINERY_SPEED: f32 = 0.25;
pub const MACHINERY_BITE: f32 = 0.25;
pub const MACHINERY_POLLUTION_DEPOSIT: f32 = 0.002;
/// Electricity: perception-radius bonus; discovery-rate multiplier; upkeep.
pub const ELECTRICITY_PERCEPTION: f32 = 0.30;
pub const ELECTRICITY_DISCOVERY: f32 = 1.5;
pub const ELECTRICITY_UPKEEP: f32 = 0.005;
/// Nuclear Power: flat per-tick energy income; child mutation-sigma
/// multiplier (radiation); heavy flat upkeep.
pub const NUCLEAR_INCOME: f32 = 0.06;
pub const NUCLEAR_MUTATION: f32 = 1.5;
pub const NUCLEAR_UPKEEP: f32 = 0.012;

/// Biome pollution: per-cell cap, regrowth-penalty cap, and per-biome-step
/// decay. Regrowth is multiplied by `1 - min(pollution, POLLUTION_MAX_EFFECT)`.
pub const POLLUTION_CAP: f32 = 0.8;
pub const POLLUTION_MAX_EFFECT: f32 = 0.7;
pub const POLLUTION_DECAY: f32 = 0.95;

// --- Discovery / spread tuning ----------------------------------------------

/// Base per-agent per-tick discovery probability at Openness = 1, skill = 1,
/// era 1 (scaled down by era and by the agent's traits/skill).
pub const BASE_DISCOVERY: f32 = 3e-5;
/// Hard cap on the summed per-tick discovery probability (all candidates).
pub const DISCOVERY_CAP: f32 = 0.05;
/// Spread: per-tick lerp rate toward the best-holding neighbour's level
/// (the skill channel's `SKILL_SOCIAL_RATE` analogue).
pub const INVENTION_SPREAD_RATE: f32 = 0.03;
/// Knowledge atrophy: per-tick decay of an invention level whose prereqs the
/// agent does NOT hold (foundations lost → the dependent tech fades).
pub const ATROPHY_RATE: f32 = 0.001;

/// The meme channel carrying invention `inv`'s adoption level.
#[inline]
pub const fn channel(inv: usize) -> usize {
    INVENTION_CHANNEL_BASE + inv
}

/// `true` iff the channel is owned by the invention tree (used to exclude
/// invention channels from the generic broadcast-mean meme lerp, which would
/// otherwise fight the copy-toward-best spread dynamic).
#[inline]
pub const fn is_invention_channel(ch: usize) -> bool {
    ch >= INVENTION_CHANNEL_BASE && ch < INVENTION_CHANNEL_BASE + INVENTION_COUNT
}

/// Adoption level of invention `inv` in a meme vector.
#[inline]
pub fn level(meme: &[f32; MEME_CHANNELS], inv: usize) -> f32 {
    meme[channel(inv)]
}

/// `true` iff the meme vector functionally holds invention `inv`.
#[inline]
pub fn has(meme: &[f32; MEME_CHANNELS], inv: usize) -> bool {
    level(meme, inv) >= HELD_THRESHOLD
}

/// Compact bitmask view of the held inventions in a meme vector — the form
/// prereq checks, the codex aggregator, and effect sites consume.
pub fn held_mask(meme: &[f32; MEME_CHANNELS]) -> u32 {
    let mut mask = 0u32;
    for k in 0..INVENTION_COUNT {
        if has(meme, k) {
            mask |= bit(k);
        }
    }
    mask
}

/// Call `f(k)` for each set bit index in `mask`, ascending.
pub fn for_each_set_bit(mask: u32, mut f: impl FnMut(usize)) {
    let mut m = mask;
    while m != 0 {
        let k = m.trailing_zeros() as usize;
        f(k);
        m &= m - 1;
    }
}

/// Inventions the holder of `mask` could work on next: not yet held, with all
/// prereqs satisfied. Visits ids ascending (era order).
fn candidates(mask: u32, mut f: impl FnMut(usize)) {
    for (k, inv) in INVENTIONS.iter().enumerate() {
        if mask & bit(k) != 0 {
            continue;
        }
        if inv.prereqs & !mask == 0 {
            f(k);
        }
    }
}

/// Highest era held in the mask (0 = pre-invention). For display.
pub fn tech_era(mask: u32) -> u8 {
    let mut era = 0u8;
    for_each_set_bit(mask, |k| era = era.max(INVENTIONS[k].era));
    era
}

// --- Cognitive (IQ) acquisition gate (Phase 2) ------------------------------

/// Realized-IQ required to acquire an invention, indexed by `era - 1`. Era-1
/// tech is nearly free to learn; era-4 tech demands high cognition. Only
/// consulted when `World::cognition_enabled` is true (see `iq_permits`).
pub const IQ_REQ_BY_ERA: [f32; 4] = [0.15, 0.35, 0.55, 0.75];

/// Realized-IQ threshold to discover or copy invention `k` (scales with era).
#[inline]
pub fn iq_req(k: usize) -> f32 {
    IQ_REQ_BY_ERA[(INVENTIONS[k].era - 1) as usize]
}

/// Whether an agent with realized `iq` may acquire invention `k`. With
/// `cognition_enabled` false the IQ gate is off (always permitted), so
/// non-cognition scenarios keep their exact behavior; otherwise the agent
/// needs `iq >= iq_req(k)`. `Openness` still governs discovery *rate*; this is
/// the hard capability *ceiling*.
#[inline]
pub fn iq_permits(iq: f32, k: usize, cognition_enabled: bool) -> bool {
    !cognition_enabled || iq >= iq_req(k)
}

// --- Multipliers read by effect sites (identity at mask = 0) ----------------

/// `1.0` if `mask` holds invention `inv`, else `0.0`. The branchless
/// multiplier form (`CONST * held_f32(..)`) keeps every effect site a straight
/// fused-multiply-add with no data-dependent branch, and reads far better than
/// the raw `(mask & bit(inv) != 0) as u8 as f32` cast it replaces. Bit-for-bit
/// identical to that cast (a `bool` is 0/1 as `u8`, exactly `0.0`/`1.0` as
/// `f32`).
#[inline]
pub fn held_f32(mask: u32, inv: usize) -> f32 {
    (mask & bit(inv) != 0) as u8 as f32
}

/// Graze-bite multiplier (Stone Tools, Farming, Machinery) — `interact::feed_pass`.
#[inline]
pub fn graze_multiplier(mask: u32) -> f32 {
    1.0 + STONE_TOOLS_BITE * held_f32(mask, STONE_TOOLS)
        + FARMING_BITE * held_f32(mask, FARMING)
        + MACHINERY_BITE * held_f32(mask, MACHINERY)
}

/// Energy-per-biomass multiplier (Fire) — `interact::feed_pass` payout.
#[inline]
pub fn food_energy_multiplier(mask: u32) -> f32 {
    1.0 + FIRE_ENERGY * held_f32(mask, FIRE)
}

/// Weapon-damage multiplier (Metalworking) — `interact::combat_pass`.
#[inline]
pub fn weapon_multiplier(mask: u32) -> f32 {
    1.0 + METALWORKING_DAMAGE * held_f32(mask, METALWORKING)
}

/// Scavenge-energy multiplier (Husbandry) — `interact::scavenge_pass` payout.
#[inline]
pub fn scavenge_multiplier(mask: u32) -> f32 {
    1.0 + HUSBANDRY_SCAVENGE * held_f32(mask, HUSBANDRY)
}

/// Locomotor speed multiplier (Machinery) — `integrate::integrate_all`.
#[inline]
pub fn speed_multiplier(mask: u32) -> f32 {
    1.0 + MACHINERY_SPEED * held_f32(mask, MACHINERY)
}

/// Basal-metabolism multiplier (Fire, Husbandry) — `integrate::integrate_all`.
#[inline]
pub fn metabolism_multiplier(mask: u32) -> f32 {
    1.0 + FIRE_METABOLISM * held_f32(mask, FIRE) + HUSBANDRY_METABOLISM * held_f32(mask, HUSBANDRY)
}

/// Module-upkeep multiplier (Metalworking) — `module::upkeep_all`.
#[inline]
pub fn module_upkeep_multiplier(mask: u32) -> f32 {
    1.0 + METALWORKING_UPKEEP * held_f32(mask, METALWORKING)
}

/// Lifespan multiplier (Medicine) — `age::age_and_starve`.
#[inline]
pub fn lifespan_multiplier(mask: u32) -> f32 {
    1.0 + MEDICINE_LIFESPAN * held_f32(mask, MEDICINE)
}

/// Child mutation-sigma multiplier (Nuclear Power, either parent) —
/// `reproduce::reproduce_all`.
#[inline]
pub fn mutation_multiplier(parent_a: u32, parent_b: u32) -> f32 {
    if (parent_a | parent_b) & bit(NUCLEAR_POWER) != 0 {
        NUCLEAR_MUTATION
    } else {
        1.0
    }
}

/// Perception-radius multiplier (Electricity) — `sense::sense_one`.
#[inline]
pub fn perception_multiplier(mask: u32) -> f32 {
    1.0 + ELECTRICITY_PERCEPTION * held_f32(mask, ELECTRICITY)
}

/// Meme-copy / invention-spread multiplier (Writing) — `culture::culture_step`.
#[inline]
pub fn spread_multiplier(mask: u32) -> f32 {
    if mask & bit(WRITING) != 0 {
        WRITING_SPREAD_MULT
    } else {
        1.0
    }
}

/// Discovery-rate multiplier (Electricity) — discovery roll below.
#[inline]
pub fn discovery_multiplier(mask: u32) -> f32 {
    if mask & bit(ELECTRICITY) != 0 {
        ELECTRICITY_DISCOVERY
    } else {
        1.0
    }
}

/// Per-tick flat upkeep minus income from held inventions (Writing, Medicine,
/// Electricity, Nuclear upkeep; Nuclear income). Positive = net drain.
pub fn flat_upkeep(mask: u32) -> f32 {
    let mut cost = 0.0;
    cost += WRITING_UPKEEP * held_f32(mask, WRITING);
    cost += MEDICINE_UPKEEP * held_f32(mask, MEDICINE);
    cost += ELECTRICITY_UPKEEP * held_f32(mask, ELECTRICITY);
    cost += NUCLEAR_UPKEEP * held_f32(mask, NUCLEAR_POWER);
    cost - NUCLEAR_INCOME * held_f32(mask, NUCLEAR_POWER)
}

/// Per-tick energy drain from Farming crowding stress, given this tick's
/// crowding neighbour count.
pub fn crowding_stress(mask: u32, crowding: u32) -> f32 {
    if mask & bit(FARMING) == 0 {
        return 0.0;
    }
    let extra = crowding.saturating_sub(FARMING_CROWDING_FREE) as f32;
    extra * FARMING_STRESS_PER_NEIGHBOR
}

/// Per-tick invention stage, run after `culture_step` (tick stage 6c):
/// innovation rolls for Communicator agents, then per-holder upkeep/income,
/// Farming crowding stress, Machinery pollution deposits, and knowledge
/// atrophy for inventions whose prereqs the agent has lost. Gated on
/// `World::inventions_enabled` — with the flag off this consumes no RNG and
/// touches no state.
pub fn invention_step(world: &mut World) {
    if !world.inventions_enabled {
        return;
    }
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());
    for &id in &alive_ids {
        let i = id as usize;
        let mut mask = held_mask(&world.agents.meme_vector[i]);

        // --- Innovation: one roll per Communicator with open candidates. ---
        if module::has(&world.agents.modules[i], ModuleType::Communicator) {
            let openness = world.agents.genome[i].get(crate::genome::GenomeSlot::Openness);
            let skill = world.agents.meme_vector[i][crate::culture::SKILL_CHANNEL].clamp(0.0, 1.0);
            let disc_mult = discovery_multiplier(mask);
            // Cognitive gate: an agent can only discover a trait its realized IQ
            // clears (no-op when cognition is disabled). Filtering the candidate
            // here keeps it out of both the summed probability and the weighted
            // pick below (its `probs` entry stays 0).
            let cognition = world.cognition_enabled;
            let agent_iq = world.agents.iq[i];
            let mut total = 0.0f32;
            let mut probs = [0.0f32; INVENTION_COUNT];
            candidates(mask, |k| {
                if !iq_permits(agent_iq, k, cognition) {
                    return;
                }
                let p = (BASE_DISCOVERY * openness * (0.3 + skill) * disc_mult
                    / INVENTIONS[k].era as f32)
                    .min(DISCOVERY_CAP);
                probs[k] = p;
                total += p;
            });
            if total > 0.0 {
                let total = total.min(DISCOVERY_CAP);
                let r = world.rng.f32_unit();
                if r < total {
                    // Weighted pick over candidates with the same draw. `probs[k]`
                    // is 0.0 for every non-candidate, so this plain ascending scan
                    // accumulates exactly what a second `candidates()` walk would —
                    // one traversal instead of two, and no prereq re-check.
                    let mut acc = 0.0f32;
                    let mut picked = usize::MAX;
                    for (k, &p) in probs.iter().enumerate() {
                        acc += p;
                        if r < acc {
                            picked = k;
                            break;
                        }
                    }
                    if picked != usize::MAX {
                        // Breakthrough: the channel jumps straight to full
                        // adoption; neighbours now copy toward it socially.
                        world.agents.meme_vector[i][channel(picked)] = 1.0;
                        mask |= bit(picked);
                    }
                }
            }
        }

        if mask == 0 {
            continue;
        }
        // --- Per-holder per-tick effects. ---
        world.agents.energy[i] -= flat_upkeep(mask);
        // Per-agent sensor bounds check: `invention_step` (stage 6c) runs before
        // the second `resize_scratch`, so on a tick where reproduce grew capacity
        // the sensors buffer is still sized to the top-of-tick population. Guard
        // per agent — not globally — so established Farming holders keep paying
        // crowding stress during growth ticks; only the just-born agents beyond
        // the buffer (no valid sensor reading yet) are skipped.
        if i < world.sensors.len() {
            world.agents.energy[i] -= crowding_stress(mask, world.sensors[i].crowding);
        }
        if mask & bit(MACHINERY) != 0 {
            let (col, row) = world.biome.cell_coords(world.agents.position[i]);
            let cell = world.biome.at_mut(col, row);
            cell.pollution = (cell.pollution + MACHINERY_POLLUTION_DEPOSIT).min(POLLUTION_CAP);
        }
        // --- Knowledge atrophy: an invention whose foundations the agent no
        // longer holds decays away (levels only — `has` drops out as the
        // level crosses the threshold). Prereq-free techs never atrophy.
        let meme = &mut world.agents.meme_vector[i];
        for k in 0..INVENTION_COUNT {
            let lvl = meme[channel(k)];
            if lvl > 0.0 && INVENTIONS[k].prereqs & !mask != 0 {
                meme[channel(k)] = (lvl - ATROPHY_RATE).max(0.0);
            }
        }
    }
    world.agents.scratch_ids = alive_ids;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prereq_chain_shape() {
        assert_eq!(INVENTIONS[STONE_TOOLS].prereqs, 0);
        for (k, inv) in INVENTIONS.iter().enumerate() {
            // Prereqs never reference self or later inventions (era order).
            assert_eq!(inv.prereqs & bit(k), 0, "{} prereqs include itself", inv.name);
            for_each_set_bit(inv.prereqs, |p| assert!(p < k, "{} prereq out of order", inv.name));
        }
    }

    #[test]
    fn candidates_respect_prereqs() {
        let mut got = Vec::new();
        candidates(0, |k| got.push(k));
        assert_eq!(got, vec![STONE_TOOLS]);
        got.clear();
        candidates(bit(STONE_TOOLS), |k| got.push(k));
        assert_eq!(got, vec![FIRE]);
        got.clear();
        candidates(bit(STONE_TOOLS) | bit(FIRE), |k| got.push(k));
        assert_eq!(got, vec![FARMING, METALWORKING]);
        got.clear();
        // Machinery needs BOTH metalworking and writing.
        candidates(bit(STONE_TOOLS) | bit(FIRE) | bit(METALWORKING), |k| got.push(k));
        assert_eq!(got, vec![FARMING]);
    }

    #[test]
    fn held_mask_thresholds() {
        let mut meme = [0.0f32; MEME_CHANNELS];
        assert_eq!(held_mask(&meme), 0);
        meme[channel(STONE_TOOLS)] = HELD_THRESHOLD - 0.01;
        assert_eq!(held_mask(&meme), 0, "just below threshold: not held");
        meme[channel(STONE_TOOLS)] = HELD_THRESHOLD;
        assert_eq!(held_mask(&meme), bit(STONE_TOOLS));
        meme[channel(FIRE)] = 1.0;
        assert_eq!(held_mask(&meme), bit(STONE_TOOLS) | bit(FIRE));
    }

    #[test]
    fn multipliers_are_identity_at_zero_mask() {
        assert_eq!(graze_multiplier(0), 1.0);
        assert_eq!(food_energy_multiplier(0), 1.0);
        assert_eq!(weapon_multiplier(0), 1.0);
        assert_eq!(scavenge_multiplier(0), 1.0);
        assert_eq!(speed_multiplier(0), 1.0);
        assert_eq!(metabolism_multiplier(0), 1.0);
        assert_eq!(module_upkeep_multiplier(0), 1.0);
        assert_eq!(lifespan_multiplier(0), 1.0);
        assert_eq!(mutation_multiplier(0, 0), 1.0);
        assert_eq!(perception_multiplier(0), 1.0);
        assert_eq!(spread_multiplier(0), 1.0);
        assert_eq!(flat_upkeep(0), 0.0);
        assert_eq!(crowding_stress(0, 100), 0.0);
    }

    #[test]
    fn tech_era_tracks_highest_held() {
        assert_eq!(tech_era(0), 0);
        assert_eq!(tech_era(bit(FIRE)), 1);
        assert_eq!(tech_era(bit(FARMING) | bit(WRITING)), 3);
        assert_eq!(tech_era(bit(NUCLEAR_POWER)), 4);
    }

    #[test]
    fn held_f32_is_exact_zero_or_one() {
        assert_eq!(held_f32(0, STONE_TOOLS), 0.0);
        assert_eq!(held_f32(bit(STONE_TOOLS), STONE_TOOLS), 1.0);
        // Unrelated bits set → still 0 for the queried invention.
        assert_eq!(held_f32(bit(FIRE) | bit(FARMING), STONE_TOOLS), 0.0);
    }

    #[test]
    fn for_each_set_bit_visits_ascending() {
        let mut got = Vec::new();
        for_each_set_bit(bit(NUCLEAR_POWER) | bit(STONE_TOOLS) | bit(WRITING), |k| got.push(k));
        assert_eq!(got, vec![STONE_TOOLS, WRITING, NUCLEAR_POWER]);
        // Empty mask visits nothing.
        got.clear();
        for_each_set_bit(0, |k| got.push(k));
        assert!(got.is_empty());
    }

    #[test]
    fn is_invention_channel_covers_exactly_the_tree() {
        assert!(!is_invention_channel(INVENTION_CHANNEL_BASE - 1));
        assert!(is_invention_channel(INVENTION_CHANNEL_BASE));
        assert!(is_invention_channel(channel(NUCLEAR_POWER)));
        // The last invention channel is the top of the tree block; the practice
        // channels above it (`PRACTICE_CHANNEL_BASE..`) are NOT invention channels.
        assert_eq!(channel(NUCLEAR_POWER), INVENTION_CHANNEL_BASE + INVENTION_COUNT - 1);
        assert!(!is_invention_channel(INVENTION_CHANNEL_BASE + INVENTION_COUNT));
        assert!(!is_invention_channel(MEME_CHANNELS));
    }

    #[test]
    fn graze_multiplier_stacks_all_three_bonuses() {
        assert_eq!(graze_multiplier(bit(STONE_TOOLS)), 1.0 + STONE_TOOLS_BITE);
        assert_eq!(graze_multiplier(bit(FARMING)), 1.0 + FARMING_BITE);
        assert_eq!(graze_multiplier(bit(MACHINERY)), 1.0 + MACHINERY_BITE);
        let all = bit(STONE_TOOLS) | bit(FARMING) | bit(MACHINERY);
        assert_eq!(graze_multiplier(all), 1.0 + STONE_TOOLS_BITE + FARMING_BITE + MACHINERY_BITE);
    }

    #[test]
    fn metabolism_multiplier_stacks_fire_and_husbandry() {
        assert_eq!(metabolism_multiplier(bit(FIRE)), 1.0 + FIRE_METABOLISM);
        assert_eq!(metabolism_multiplier(bit(HUSBANDRY)), 1.0 + HUSBANDRY_METABOLISM);
        assert_eq!(
            metabolism_multiplier(bit(FIRE) | bit(HUSBANDRY)),
            1.0 + FIRE_METABOLISM + HUSBANDRY_METABOLISM
        );
    }

    #[test]
    fn single_bit_multipliers_apply_their_magnitude() {
        assert_eq!(food_energy_multiplier(bit(FIRE)), 1.0 + FIRE_ENERGY);
        assert_eq!(weapon_multiplier(bit(METALWORKING)), 1.0 + METALWORKING_DAMAGE);
        assert_eq!(scavenge_multiplier(bit(HUSBANDRY)), 1.0 + HUSBANDRY_SCAVENGE);
        assert_eq!(speed_multiplier(bit(MACHINERY)), 1.0 + MACHINERY_SPEED);
        assert_eq!(module_upkeep_multiplier(bit(METALWORKING)), 1.0 + METALWORKING_UPKEEP);
        assert_eq!(lifespan_multiplier(bit(MEDICINE)), 1.0 + MEDICINE_LIFESPAN);
        assert_eq!(perception_multiplier(bit(ELECTRICITY)), 1.0 + ELECTRICITY_PERCEPTION);
    }

    #[test]
    fn writing_and_electricity_gate_their_rate_multipliers() {
        // Off by default, on only for the exact holder.
        assert_eq!(spread_multiplier(bit(FARMING)), 1.0);
        assert_eq!(spread_multiplier(bit(WRITING)), WRITING_SPREAD_MULT);
        assert_eq!(discovery_multiplier(bit(MACHINERY)), 1.0);
        assert_eq!(discovery_multiplier(bit(ELECTRICITY)), ELECTRICITY_DISCOVERY);
    }

    #[test]
    fn mutation_multiplier_triggers_on_either_parent() {
        let nuke = bit(NUCLEAR_POWER);
        assert_eq!(mutation_multiplier(0, 0), 1.0);
        assert_eq!(mutation_multiplier(nuke, 0), NUCLEAR_MUTATION);
        assert_eq!(mutation_multiplier(0, nuke), NUCLEAR_MUTATION);
        assert_eq!(mutation_multiplier(nuke, nuke), NUCLEAR_MUTATION);
        // A non-Nuclear invention on both parents does not radiate.
        assert_eq!(mutation_multiplier(bit(MEDICINE), bit(WRITING)), 1.0);
    }

    #[test]
    fn crowding_stress_only_bites_farmers_above_the_free_allowance() {
        // No Farming → no stress regardless of density.
        assert_eq!(crowding_stress(bit(FIRE), 1000), 0.0);
        let farm = bit(FARMING);
        // At or below the free allowance → no stress.
        assert_eq!(crowding_stress(farm, 0), 0.0);
        assert_eq!(crowding_stress(farm, FARMING_CROWDING_FREE), 0.0);
        // Above → linear in the excess.
        let excess = 5;
        assert_eq!(
            crowding_stress(farm, FARMING_CROWDING_FREE + excess),
            excess as f32 * FARMING_STRESS_PER_NEIGHBOR
        );
    }

    #[test]
    fn flat_upkeep_nets_income_against_costs() {
        // Writing alone: pure cost.
        assert_eq!(flat_upkeep(bit(WRITING)), WRITING_UPKEEP);
        // Nuclear alone: income minus its own upkeep (design intends net income).
        let nuke_only = NUCLEAR_UPKEEP - NUCLEAR_INCOME;
        assert!((flat_upkeep(bit(NUCLEAR_POWER)) - nuke_only).abs() < 1e-7);
        assert!(nuke_only < 0.0, "Nuclear should be net energy income when held alone");
        // Full late-game stack: every upkeep plus Nuclear income.
        let full = bit(WRITING) | bit(MEDICINE) | bit(ELECTRICITY) | bit(NUCLEAR_POWER);
        let expected =
            WRITING_UPKEEP + MEDICINE_UPKEEP + ELECTRICITY_UPKEEP + NUCLEAR_UPKEEP - NUCLEAR_INCOME;
        assert!((flat_upkeep(full) - expected).abs() < 1e-7);
    }
}
