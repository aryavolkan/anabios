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
//!
//! When `World::resources_enabled` is ALSO on, each invention carries a
//! material basket (`Invention::materials`): the learner must hold the goods
//! to make any progress (discovery rolls skip it, social copying stalls) and
//! pays the basket on completed acquisition, recorded as a codex
//! `MaterialLearning` event. The economy thereby funds culture, not births —
//! reproduction never touches trade goods.

use crate::genome::{Genome, GenomeSlot};
use crate::module::{self, ModuleType};
use crate::program::MEME_CHANNELS;
use crate::world::World;

mod params;
pub use params::*;

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

/// Couples an invention to a genome slot. When `World::gene_tech_coupling` is
/// on, holding the invention scales its buff by the holder's slot value, so
/// adoption exerts directional selection on that gene (the tech→gene arm), and
/// per-candidate discovery is reweighted by the same gene (the gene→tech arm).
#[derive(Clone, Copy)]
pub struct GeneAffinity {
    /// The genome slot this invention selects for.
    pub slot: GenomeSlot,
    /// Fraction of the buff scaled by `(gene - 0.5)`. `|coeff| < 2` keeps the
    /// buff strictly positive across `gene ∈ [0,1]`.
    pub coeff: f32,
}

/// Hard genetic prerequisite: the learner's value at `slot` must be ≥ `min`
/// to acquire the invention — discovery rolls exclude it and social copying
/// skips it while the genome falls short (the same hard-ceiling shape as the
/// IQ gate, but heritable: culture now waits on the genome, not just the
/// phenotype). Only consulted when `World::gene_requirements` is true.
#[derive(Clone, Copy)]
pub struct GeneReq {
    /// The genome slot that gates acquisition.
    pub slot: GenomeSlot,
    /// Minimum slot value in `[0,1]`.
    pub min: f32,
}

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
    /// Material basket (units per `resource::Good`, indexed by `Good::index`)
    /// the learner must HOLD to make any progress on this invention —
    /// discovery rolls exclude it and social copying skips it while the
    /// basket is unaffordable — and PAYS on completed acquisition (a
    /// discovery breakthrough, or a social copy crossing `HELD_THRESHOLD`).
    /// All-zero = free to learn. Only consulted when
    /// `World::resources_enabled`; with the flag off every invention is free
    /// and scenarios without the economy are byte-identical to before.
    pub materials: [f32; crate::resource::GOOD_COUNT],
    /// One-line upside summary (UI).
    pub buff: &'static str,
    /// One-line downside summary (UI).
    pub debuff: &'static str,
    /// Optional genome-slot coupling (gene↔tech coevolution). `None` = the
    /// buff is genome-independent (behaves as if coupling were off).
    pub affinity: Option<GeneAffinity>,
    /// Optional hard genetic prerequisite (min slot value to acquire). `None`
    /// = no genetic gate. Only consulted when `World::gene_requirements`.
    pub gene_req: Option<GeneReq>,
}

#[inline]
pub const fn bit(inv: usize) -> u32 {
    1u32 << inv
}

/// Resolve a scenario invention name — its machine `key`, e.g. `"stone_tools"`,
/// `"husbandry"`, `"nuclear_power"` — to its invention id. Case-insensitive;
/// surrounding whitespace ignored. Returns `None` for an unknown name.
pub fn id_from_name(name: &str) -> Option<usize> {
    let want = name.trim().to_ascii_lowercase();
    INVENTIONS.iter().position(|inv| inv.key == want)
}

/// The holder's value of invention `inv`'s affinity gene, or `0.5` (the neutral
/// point → identity in `coupled_held`) when the invention has no affinity.
#[inline]
pub fn affinity_gene(genome: &Genome, inv: usize) -> f32 {
    match INVENTIONS[inv].affinity {
        Some(a) => genome.get(a.slot),
        None => 0.5,
    }
}

pub const INVENTIONS: [Invention; INVENTION_COUNT] = [
    Invention {
        name: "Stone Tools",
        key: "stone_tools",
        era: 1,
        prereqs: 0,
        // Knapping stone.
        materials: [0.0, 2.0, 0.0, 0.0],
        buff: "+25% graze bite",
        debuff: "none",
        // Knapping is learned-by-doing: the bite buff scales with
        // IndividualLearning, tying the first technology to the DIT
        // individual-learning arm.
        affinity: Some(GeneAffinity { slot: GenomeSlot::IndividualLearning, coeff: 0.8 }),
        // Era-1 tech is nearly free to learn — no genetic gate.
        gene_req: None,
    },
    Invention {
        name: "Fire",
        key: "fire",
        era: 1,
        prereqs: bit(STONE_TOOLS),
        // Hearth stones + fuelwood.
        materials: [1.0, 0.0, 2.0, 0.0],
        buff: "+40% energy per biomass",
        debuff: "+10% metabolism",
        // Bold experimenters harness fire: its energy buff scales with Openness,
        // which also drives discovery — a clean innovation feedback loop.
        affinity: Some(GeneAffinity { slot: GenomeSlot::Openness, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::Openness, min: 0.30 }),
    },
    Invention {
        name: "Farming",
        key: "farming",
        era: 2,
        prereqs: bit(FIRE),
        // Seed grain + preservation salt.
        materials: [1.0, 0.0, 0.0, 2.0],
        buff: "+60% graze yield",
        debuff: "crowding stress",
        // Sedentary farming rewards prudent planners: its yield scales with
        // Conscientiousness.
        affinity: Some(GeneAffinity { slot: GenomeSlot::Conscientiousness, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::Conscientiousness, min: 0.35 }),
    },
    Invention {
        name: "Metalworking",
        key: "metalworking",
        era: 2,
        prereqs: bit(FIRE),
        // Ore + flux + forge fuel.
        materials: [1.0, 2.0, 1.0, 0.0],
        buff: "+50% weapon damage",
        debuff: "+10% module upkeep",
        // Better weapons pay off most for lineages that hold and defend ground:
        // the damage buff scales with Territoriality (wiring the previously
        // inert slot into the coevolution loop).
        affinity: Some(GeneAffinity { slot: GenomeSlot::Territoriality, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::Territoriality, min: 0.40 }),
    },
    Invention {
        name: "Writing",
        key: "writing",
        era: 3,
        prereqs: bit(FARMING),
        // Writing medium + pigment + binder.
        materials: [0.0, 0.0, 2.0, 1.0],
        buff: "2x meme + invention spread",
        debuff: "small upkeep",
        // Literacy rewards communicators: the spread buff scales with the
        // (previously inert) CommunicationStrength slot.
        affinity: Some(GeneAffinity { slot: GenomeSlot::CommunicationStrength, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::CommunicationStrength, min: 0.45 }),
    },
    Invention {
        name: "Medicine",
        key: "medicine",
        era: 3,
        prereqs: bit(WRITING),
        // Mineral salts + herbs + resin.
        materials: [2.0, 0.0, 1.0, 2.0],
        buff: "+50% lifespan",
        debuff: "small upkeep",
        // Medicine's lifespan buff rewards the cognitive lineage that could
        // reach era-3 tech: scales with CognitivePotential.
        affinity: Some(GeneAffinity { slot: GenomeSlot::CognitivePotential, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::CognitivePotential, min: 0.50 }),
    },
    Invention {
        name: "Husbandry",
        key: "husbandry",
        era: 3,
        prereqs: bit(FARMING),
        // Fodder + salt licks for the penned herd.
        materials: [2.0, 0.0, 0.0, 2.0],
        buff: "+40% scavenge energy",
        debuff: "+8% metabolism",
        // Livestock tolerate only patient keepers: the scavenge buff scales
        // with Agreeableness.
        affinity: Some(GeneAffinity { slot: GenomeSlot::Agreeableness, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::Agreeableness, min: 0.40 }),
    },
    Invention {
        name: "Machinery",
        key: "machinery",
        era: 4,
        prereqs: bit(METALWORKING) | bit(WRITING),
        // Worked metal + mineral parts + lubricant.
        materials: [2.0, 2.0, 1.0, 0.0],
        buff: "+25% speed & bite",
        debuff: "pollutes local biome",
        // Machines reward routine exploiters over restless explorers: the
        // speed/bite buff scales with ExploreVsExploit (wiring the previously
        // inert slot).
        affinity: Some(GeneAffinity { slot: GenomeSlot::ExploreVsExploit, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::CognitivePotential, min: 0.45 }),
    },
    Invention {
        name: "Electricity",
        key: "electricity",
        era: 4,
        prereqs: bit(MACHINERY),
        // Conductors + magnets + insulation.
        materials: [1.0, 2.0, 2.0, 0.0],
        buff: "+30% perception, 1.5x discovery",
        debuff: "upkeep",
        // Electrification is a leap of abstraction: its perception buff scales
        // with Openness (the TG1 roadmap's proposed pairing).
        affinity: Some(GeneAffinity { slot: GenomeSlot::Openness, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::Openness, min: 0.55 }),
    },
    Invention {
        name: "Nuclear Power",
        key: "nuclear_power",
        era: 4,
        prereqs: bit(ELECTRICITY),
        // The full industrial supply chain: a heavy draw on everything.
        materials: [2.0, 1.0, 1.0, 2.0],
        buff: "flat energy income",
        debuff: "1.5x child mutation + upkeep",
        // Living with mutagenic power selects lineages tuned for it: the
        // energy income scales with MutationRate.
        affinity: Some(GeneAffinity { slot: GenomeSlot::MutationRate, coeff: 0.8 }),
        gene_req: Some(GeneReq { slot: GenomeSlot::CognitivePotential, min: 0.65 }),
    },
];

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

/// Minimum genome `Size` for the viewer's "large" bucket. Equals the viewer's
/// `SIZE_SPLIT` (1.25 world-units) under `size = 0.5 + 2.5 × Size`.
pub const APE_SIZE_MIN: f32 = 0.30;
/// Omnivore band lower bound (inclusive) — the viewer's `HERB_MAX`.
pub const APE_DIET_LO: f32 = 0.34;
/// Omnivore band upper bound (exclusive) — the viewer's `CARN_MIN`.
pub const APE_DIET_HI: f32 = 0.66;

/// True when the agent is the viewer's PRIMATE archetype (omnivore + large):
/// the only archetype permitted to acquire cultural inventions. Practices are
/// unaffected — any animal can hold and spread those.
pub fn is_ape(genome: &Genome, modules: &module::ModuleList) -> bool {
    let diet = module::effective_diet_carnivory(modules);
    genome.get(GenomeSlot::Size) >= APE_SIZE_MIN && diet >= APE_DIET_LO && diet < APE_DIET_HI
}

/// Zero every invention channel of `meme` when the agent is not an ape (no-op
/// when `inventions_enabled` is false, so flag-off scenarios are byte-identical).
/// Practice and base channels are never touched. Consumes no RNG.
pub fn enforce_ape_only(
    meme: &mut [f32; MEME_CHANNELS],
    genome: &Genome,
    modules: &module::ModuleList,
    inventions_enabled: bool,
) {
    if !inventions_enabled || is_ape(genome, modules) {
        return;
    }
    for k in 0..INVENTION_COUNT {
        meme[channel(k)] = 0.0;
    }
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

/// Whether `genome` clears invention `k`'s hard genetic prerequisite
/// (`Invention::gene_req`). With `gene_requirements` false the gate is off
/// (always permitted), so baseline scenarios keep their exact behavior;
/// otherwise the learner needs `genome[slot] >= min` to discover OR copy the
/// invention — culture waits on the genome.
#[inline]
pub fn gene_permits(genome: &Genome, k: usize, gene_requirements: bool) -> bool {
    match (gene_requirements, INVENTIONS[k].gene_req) {
        (true, Some(req)) => genome.get(req.slot) >= req.min,
        _ => true,
    }
}

// --- Material (trade-goods) learning cost ------------------------------------

/// Whether `inventory` covers invention `k`'s material basket. With
/// `resources_enabled` false the material gate is off (always affordable), so
/// economy-free scenarios keep their exact behavior; otherwise the learner
/// must hold every required good before discovery rolls include `k` or social
/// copying makes progress on it.
#[inline]
pub fn materials_permit(
    inventory: &[f32; crate::resource::GOOD_COUNT],
    k: usize,
    resources_enabled: bool,
) -> bool {
    !resources_enabled
        || (0..crate::resource::GOOD_COUNT).all(|g| inventory[g] >= INVENTIONS[k].materials[g])
}

/// Deduct invention `k`'s material basket from `inventory`. Call only after
/// `materials_permit` passed, so no slot can go negative.
pub fn consume_materials(inventory: &mut [f32; crate::resource::GOOD_COUNT], k: usize) {
    for (slot, cost) in inventory.iter_mut().zip(INVENTIONS[k].materials.iter()) {
        *slot -= cost;
    }
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

/// Held-weight for `inv` inside a buff multiplier. With `coupling` off, or the
/// invention unheld, or no affinity, this is exactly `held_f32(mask, inv)`
/// (`0.0`/`1.0`) — so the base multipliers stay bit-identical. With coupling on
/// and `inv` held with an affinity, it is `1.0 + coeff*(gene-0.5)`, scaling that
/// invention's buff term by the holder's gene (the tech→gene selection arm).
#[inline]
pub fn coupled_held(mask: u32, inv: usize, gene: f32, coupling: bool) -> f32 {
    let h = held_f32(mask, inv);
    if !coupling || h == 0.0 {
        return h;
    }
    match INVENTIONS[inv].affinity {
        Some(a) => h * (1.0 + a.coeff * (gene - 0.5)),
        None => h,
    }
}

/// `coupled_held` reading the affinity gene straight from `genome` — the form
/// effect sites use, so one argument list serves every invention.
#[inline]
pub fn coupled_held_genome(mask: u32, inv: usize, genome: &Genome, coupling: bool) -> f32 {
    coupled_held(mask, inv, affinity_gene(genome, inv), coupling)
}

/// Graze-bite multiplier (Stone Tools, Farming, Machinery) — `interact::feed_pass`.
/// With `coupling` on, each term scales with its invention's affinity gene.
#[inline]
pub fn graze_multiplier_coupled(mask: u32, genome: &Genome, coupling: bool) -> f32 {
    1.0 + STONE_TOOLS_BITE * coupled_held_genome(mask, STONE_TOOLS, genome, coupling)
        + FARMING_BITE * coupled_held_genome(mask, FARMING, genome, coupling)
        + MACHINERY_BITE * coupled_held_genome(mask, MACHINERY, genome, coupling)
}

/// Graze-bite multiplier with coupling off (genome-independent, as before).
/// Test-only oracle: production uses `graze_multiplier_coupled`, which equals
/// this exactly at `coupling = false`.
#[cfg(test)]
#[inline]
pub fn graze_multiplier(mask: u32) -> f32 {
    1.0 + STONE_TOOLS_BITE * held_f32(mask, STONE_TOOLS)
        + FARMING_BITE * held_f32(mask, FARMING)
        + MACHINERY_BITE * held_f32(mask, MACHINERY)
}

/// Energy-per-biomass multiplier (Fire) — `interact::feed_pass` payout.
#[inline]
pub fn food_energy_multiplier_coupled(mask: u32, genome: &Genome, coupling: bool) -> f32 {
    1.0 + FIRE_ENERGY * coupled_held_genome(mask, FIRE, genome, coupling)
}

/// Energy-per-biomass multiplier with coupling off. Test-only oracle for
/// `food_energy_multiplier_coupled` at `coupling = false`.
#[cfg(test)]
#[inline]
pub fn food_energy_multiplier(mask: u32) -> f32 {
    1.0 + FIRE_ENERGY * held_f32(mask, FIRE)
}

/// Weapon-damage multiplier (Metalworking) — `interact::combat_pass`.
#[inline]
pub fn weapon_multiplier_coupled(mask: u32, genome: &Genome, coupling: bool) -> f32 {
    1.0 + METALWORKING_DAMAGE * coupled_held_genome(mask, METALWORKING, genome, coupling)
}

/// Weapon-damage multiplier with coupling off. Test-only oracle for
/// `weapon_multiplier_coupled` at `coupling = false`.
#[cfg(test)]
#[inline]
pub fn weapon_multiplier(mask: u32) -> f32 {
    1.0 + METALWORKING_DAMAGE * held_f32(mask, METALWORKING)
}

/// Scavenge-energy multiplier (Husbandry) — `interact::scavenge_pass` payout.
#[inline]
pub fn scavenge_multiplier_coupled(mask: u32, genome: &Genome, coupling: bool) -> f32 {
    1.0 + HUSBANDRY_SCAVENGE * coupled_held_genome(mask, HUSBANDRY, genome, coupling)
}

/// Scavenge-energy multiplier with coupling off. Test-only oracle for
/// `scavenge_multiplier_coupled` at `coupling = false`.
#[cfg(test)]
#[inline]
pub fn scavenge_multiplier(mask: u32) -> f32 {
    1.0 + HUSBANDRY_SCAVENGE * held_f32(mask, HUSBANDRY)
}

/// Locomotor speed multiplier (Machinery) — `integrate::integrate_all`.
#[inline]
pub fn speed_multiplier_coupled(mask: u32, genome: &Genome, coupling: bool) -> f32 {
    1.0 + MACHINERY_SPEED * coupled_held_genome(mask, MACHINERY, genome, coupling)
}

/// Locomotor speed multiplier with coupling off. Test-only oracle for
/// `speed_multiplier_coupled` at `coupling = false`.
#[cfg(test)]
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
pub fn lifespan_multiplier_coupled(mask: u32, genome: &Genome, coupling: bool) -> f32 {
    1.0 + MEDICINE_LIFESPAN * coupled_held_genome(mask, MEDICINE, genome, coupling)
}

/// Lifespan multiplier with coupling off. Test-only oracle for
/// `lifespan_multiplier_coupled` at `coupling = false`.
#[cfg(test)]
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
pub fn perception_multiplier_coupled(mask: u32, genome: &Genome, coupling: bool) -> f32 {
    1.0 + ELECTRICITY_PERCEPTION * coupled_held_genome(mask, ELECTRICITY, genome, coupling)
}

/// Perception-radius multiplier with coupling off. Test-only oracle for
/// `perception_multiplier_coupled` at `coupling = false`.
#[cfg(test)]
#[inline]
pub fn perception_multiplier(mask: u32) -> f32 {
    1.0 + ELECTRICITY_PERCEPTION * held_f32(mask, ELECTRICITY)
}

/// Meme-copy / invention-spread multiplier (Writing) — `culture::culture_step`.
/// With `coupling` on, the literacy bonus above `1.0` scales with the holder's
/// CommunicationStrength gene.
#[inline]
pub fn spread_multiplier_coupled(mask: u32, genome: &Genome, coupling: bool) -> f32 {
    if mask & bit(WRITING) == 0 {
        return 1.0;
    }
    let bonus = WRITING_SPREAD_MULT - 1.0;
    let scale = match (coupling, INVENTIONS[WRITING].affinity) {
        (true, Some(a)) => 1.0 + a.coeff * (genome.get(a.slot) - 0.5),
        _ => 1.0,
    };
    1.0 + bonus * scale
}

/// Meme-copy / invention-spread multiplier with coupling off. Test-only oracle
/// for `spread_multiplier_coupled` at `coupling = false`.
#[cfg(test)]
#[inline]
pub fn spread_multiplier(mask: u32) -> f32 {
    if mask & bit(WRITING) == 0 {
        return 1.0;
    }
    WRITING_SPREAD_MULT
}

/// Multiplier on invention `inv`'s per-tick discovery probability from the
/// holder's affinity gene (the gene→tech arm). `1.0` when coupling is off, the
/// invention has no affinity, or the gene sits at the `0.5` neutral point;
/// ranges `[0.5, 1.5]`. Reweights the existing single RNG draw's probability
/// table — adds no draw, so flag-off behavior is unchanged.
#[inline]
pub fn discovery_affinity_weight(genome: &Genome, inv: usize, coupling: bool) -> f32 {
    match (coupling, INVENTIONS[inv].affinity) {
        (true, Some(a)) => 0.5 + genome.get(a.slot),
        _ => 1.0,
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
/// Electricity, Nuclear upkeep; Nuclear income). Positive = net drain. Test-only
/// oracle for `flat_upkeep_coupled` at `coupling = false`.
#[cfg(test)]
pub fn flat_upkeep(mask: u32) -> f32 {
    let mut cost = 0.0;
    cost += WRITING_UPKEEP * held_f32(mask, WRITING);
    cost += MEDICINE_UPKEEP * held_f32(mask, MEDICINE);
    cost += ELECTRICITY_UPKEEP * held_f32(mask, ELECTRICITY);
    cost += NUCLEAR_UPKEEP * held_f32(mask, NUCLEAR_POWER);
    cost - NUCLEAR_INCOME * held_f32(mask, NUCLEAR_POWER)
}

/// `flat_upkeep` with the Nuclear income term scaled by the holder's
/// MutationRate affinity gene when `coupling` is on (the tech→gene arm for
/// Nuclear Power). Bit-identical to `flat_upkeep` with coupling off.
pub fn flat_upkeep_coupled(mask: u32, genome: &Genome, coupling: bool) -> f32 {
    let mut cost = 0.0;
    cost += WRITING_UPKEEP * held_f32(mask, WRITING);
    cost += MEDICINE_UPKEEP * held_f32(mask, MEDICINE);
    cost += ELECTRICITY_UPKEEP * held_f32(mask, ELECTRICITY);
    cost += NUCLEAR_UPKEEP * held_f32(mask, NUCLEAR_POWER);
    cost - NUCLEAR_INCOME * coupled_held_genome(mask, NUCLEAR_POWER, genome, coupling)
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
        if module::has(&world.agents.modules[i], ModuleType::Communicator)
            && is_ape(&world.agents.genome[i], &world.agents.modules[i])
        {
            let openness = world.agents.genome[i].get(crate::genome::GenomeSlot::Openness);
            let skill = world.agents.meme_vector[i][crate::culture::SKILL_CHANNEL].clamp(0.0, 1.0);
            let disc_mult = discovery_multiplier(mask);
            // Cognitive gate: an agent can only discover a trait its realized IQ
            // clears (no-op when cognition is disabled). Filtering the candidate
            // here keeps it out of both the summed probability and the weighted
            // pick below (its `probs` entry stays 0).
            let cognition = world.cognition_enabled;
            let agent_iq = world.agents.iq[i];
            // Material gate: with the economy on, a discovery also needs its
            // trade-goods basket in hand (no-op when resources are disabled).
            let resources = world.resources_enabled;
            // Gene→tech arm: a lineage rich in a tech's affinity gene discovers
            // that tech faster (identity when coupling is off — the weight is
            // 1.0, so the probability table and its single RNG draw are
            // unchanged). Borrow ends when the closure returns, before the roll.
            let coupling = world.gene_tech_coupling;
            // Genetic prerequisite gate: with gene_requirements on, a lineage
            // whose genome falls short of an invention's GeneReq can neither
            // discover it here nor copy it socially (culture.rs) — identity
            // when the flag is off.
            let gene_reqs = world.gene_requirements;
            let genome = &world.agents.genome[i];
            let inventory = world.agents.inventory[i];
            let mut total = 0.0f32;
            let mut probs = [0.0f32; INVENTION_COUNT];
            candidates(mask, |k| {
                if !iq_permits(agent_iq, k, cognition) {
                    return;
                }
                if !materials_permit(&inventory, k, resources) {
                    return;
                }
                if !gene_permits(genome, k, gene_reqs) {
                    return;
                }
                let p = (BASE_DISCOVERY * openness * (0.3 + skill) * disc_mult
                    / INVENTIONS[k].era as f32
                    * discovery_affinity_weight(genome, k, coupling))
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
                        // The discoverer pays the material basket (economy on).
                        if resources {
                            consume_materials(&mut world.agents.inventory[i], picked);
                            world.codex.push_event(crate::codex::CodexEvent {
                                event_type: crate::codex::EventType::MaterialLearning,
                                tick: world.tick,
                                species_id: world.agents.species_id[i],
                                value: picked as f32,
                                loc_x: world.agents.position[i].x,
                                loc_y: world.agents.position[i].y,
                            });
                        }
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
        world.agents.energy[i] -=
            flat_upkeep_coupled(mask, &world.agents.genome[i], world.gene_tech_coupling);
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

    #[cfg(test)]
    fn ape_modules(diet: f32) -> crate::module::ModuleList {
        let mut m = crate::module::ModuleList::new();
        m.push(crate::module::Module::Mouth { bite_size: 0.6, diet_affinity: diet });
        m
    }

    #[test]
    fn is_ape_matches_viewer_primate_band() {
        use crate::genome::{Genome, GenomeSlot};
        let mut large = Genome::neutral();
        large.set(GenomeSlot::Size, 0.5); // -> world 1.75, large
        let mut small = Genome::neutral();
        small.set(GenomeSlot::Size, 0.25); // -> world 1.125, below SIZE_SPLIT

        // omnivore + large = ape
        assert!(is_ape(&large, &ape_modules(0.5)));
        // omnivore boundary: 0.34 in-band, 0.66 out (half-open, matches CARN_MIN)
        assert!(is_ape(&large, &ape_modules(APE_DIET_LO)));
        assert!(!is_ape(&large, &ape_modules(APE_DIET_HI)));
        // herbivore or carnivore large = not ape (Deer / Wolf)
        assert!(!is_ape(&large, &ape_modules(0.0)));
        assert!(!is_ape(&large, &ape_modules(0.9)));
        // omnivore but small = not ape (Boar)
        assert!(!is_ape(&small, &ape_modules(0.5)));
        // size boundary: exactly APE_SIZE_MIN is large
        let mut edge = Genome::neutral();
        edge.set(GenomeSlot::Size, APE_SIZE_MIN);
        assert!(is_ape(&edge, &ape_modules(0.5)));
    }

    #[test]
    fn enforce_ape_only_strips_inventions_from_non_apes() {
        use crate::genome::{Genome, GenomeSlot};
        let mut meme = [0.0f32; crate::program::MEME_CHANNELS];
        meme[channel(STONE_TOOLS)] = 1.0;
        meme[channel(FIRE)] = 0.7;
        // practice channel stand-in: a non-invention channel must be preserved
        let practice_ch = crate::practice::channel(crate::practice::CHILD_SACRIFICE);
        meme[practice_ch] = 1.0;

        let mut g = Genome::neutral();
        g.set(GenomeSlot::Size, 0.5);
        let herb = ape_modules(0.0); // non-ape

        // flag off: no-op even for a non-ape
        let mut off = meme;
        enforce_ape_only(&mut off, &g, &herb, false);
        assert_eq!(off, meme);

        // flag on, non-ape: invention channels zeroed, practice untouched
        let mut on = meme;
        enforce_ape_only(&mut on, &g, &herb, true);
        assert_eq!(on[channel(STONE_TOOLS)], 0.0);
        assert_eq!(on[channel(FIRE)], 0.0);
        assert_eq!(on[practice_ch], 1.0);

        // flag on, ape: unchanged
        let mut ape = meme;
        enforce_ape_only(&mut ape, &g, &ape_modules(0.5), true);
        assert_eq!(ape, meme);
    }

    #[test]
    fn id_from_name_resolves_keys_case_insensitively() {
        assert_eq!(id_from_name("stone_tools"), Some(STONE_TOOLS));
        assert_eq!(id_from_name("Husbandry"), Some(HUSBANDRY));
        assert_eq!(id_from_name("  writing  "), Some(WRITING));
        assert_eq!(id_from_name("wheel"), None);
        assert_eq!(id_from_name(""), None);
    }

    #[test]
    fn affinity_table_is_well_formed() {
        use crate::genome::{Genome, GenomeSlot};
        // Every invention carries an affinity — the full tree feeds the
        // coevolution loop. Spot-check the pairings.
        for inv in INVENTIONS.iter() {
            assert!(inv.affinity.is_some(), "{} has no affinity", inv.name);
        }
        assert_eq!(INVENTIONS[FIRE].affinity.unwrap().slot as usize, GenomeSlot::Openness as usize);
        assert_eq!(
            INVENTIONS[FARMING].affinity.unwrap().slot as usize,
            GenomeSlot::Conscientiousness as usize
        );
        assert_eq!(
            INVENTIONS[MEDICINE].affinity.unwrap().slot as usize,
            GenomeSlot::CognitivePotential as usize
        );
        assert_eq!(
            INVENTIONS[WRITING].affinity.unwrap().slot as usize,
            GenomeSlot::CommunicationStrength as usize
        );
        assert_eq!(
            INVENTIONS[STONE_TOOLS].affinity.unwrap().slot as usize,
            GenomeSlot::IndividualLearning as usize
        );
        // Coeffs keep the buff strictly positive across gene ∈ [0,1]:
        // 1 + coeff*(gene-0.5) > 0  <=>  |coeff| < 2.
        for inv in INVENTIONS.iter() {
            if let Some(a) = inv.affinity {
                assert!(a.coeff.abs() < 2.0, "{} coeff too large", inv.name);
            }
        }
        // affinity_gene returns the slot value for the invention's affinity.
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Openness, 0.9);
        g.set(GenomeSlot::IndividualLearning, 0.2);
        assert!((affinity_gene(&g, FIRE) - 0.9).abs() < 1e-6);
        assert!((affinity_gene(&g, STONE_TOOLS) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn gene_req_table_is_well_formed() {
        use crate::genome::{Genome, GenomeSlot};
        // Era-1 entry tech is genetically free; every gate sits inside (0,1).
        assert!(INVENTIONS[STONE_TOOLS].gene_req.is_none());
        for inv in INVENTIONS.iter() {
            if let Some(req) = inv.gene_req {
                assert!(req.min > 0.0 && req.min < 1.0, "{} gate out of range", inv.name);
            }
        }
        // Gates rise with era (max gate per era is non-decreasing).
        let mut prev = 0.0f32;
        for era in 1..=4u8 {
            let max_gate = INVENTIONS
                .iter()
                .filter(|inv| inv.era == era)
                .filter_map(|inv| inv.gene_req)
                .map(|req| req.min)
                .fold(0.0f32, f32::max);
            assert!(max_gate >= prev, "era {era} gates regressed");
            prev = max_gate;
        }
        // gene_permits: identity when the flag is off, threshold when on.
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Openness, 0.9);
        assert!(gene_permits(&g, FIRE, false));
        assert!(gene_permits(&g, FIRE, true));
        g.set(GenomeSlot::Openness, 0.1);
        assert!(gene_permits(&g, FIRE, false), "flag off = always permitted");
        assert!(!gene_permits(&g, FIRE, true), "below the gate: blocked");
        // Gate-free inventions are always permitted.
        assert!(gene_permits(&g, STONE_TOOLS, true));
        // Exact threshold passes (>= semantics).
        let req = INVENTIONS[FIRE].gene_req.unwrap();
        g.set(req.slot, req.min);
        assert!(gene_permits(&g, FIRE, true));
    }

    #[test]
    fn coupling_is_identity_when_off_and_monotonic_when_on() {
        use crate::genome::{Genome, GenomeSlot};
        let farm = bit(FARMING);
        let neutral = Genome::neutral();
        let mut lo = Genome::neutral();
        lo.set(GenomeSlot::Conscientiousness, 0.0);
        let mut hi = Genome::neutral();
        hi.set(GenomeSlot::Conscientiousness, 1.0);
        // OFF: coupled == base, regardless of gene.
        assert_eq!(graze_multiplier_coupled(farm, &lo, false), graze_multiplier(farm));
        assert_eq!(graze_multiplier_coupled(farm, &hi, false), graze_multiplier(farm));
        // ON, gene = 0.5: neutral, equals base.
        assert!(
            (graze_multiplier_coupled(farm, &neutral, true) - graze_multiplier(farm)).abs() < 1e-6
        );
        // ON: strictly increasing in the gene; the Farming term is what moves.
        let lo_m = graze_multiplier_coupled(farm, &lo, true);
        let hi_m = graze_multiplier_coupled(farm, &hi, true);
        assert!(hi_m > graze_multiplier(farm) && graze_multiplier(farm) > lo_m);
        assert!(lo_m > 0.0, "buff must stay positive");
        // Unheld invention: gene has no effect (coupled_held returns 0).
        assert_eq!(graze_multiplier_coupled(0, &hi, true), graze_multiplier(0));
        // Stone Tools and Machinery terms couple too (IndividualLearning /
        // ExploreVsExploit): a neutral genome is identity, a raised slot lifts
        // the buff.
        let st = bit(STONE_TOOLS);
        let mut learner = Genome::neutral();
        learner.set(GenomeSlot::IndividualLearning, 1.0);
        assert!((graze_multiplier_coupled(st, &neutral, true) - graze_multiplier(st)).abs() < 1e-6);
        assert!(graze_multiplier_coupled(st, &learner, true) > graze_multiplier(st));
        let mach = bit(MACHINERY);
        let mut exploiter = Genome::neutral();
        exploiter.set(GenomeSlot::ExploreVsExploit, 1.0);
        assert!(graze_multiplier_coupled(mach, &exploiter, true) > graze_multiplier(mach));
        assert!(speed_multiplier_coupled(mach, &exploiter, true) > speed_multiplier(mach));
        assert_eq!(speed_multiplier_coupled(mach, &exploiter, false), speed_multiplier(mach));
        // Metalworking weapon + Husbandry scavenge + Electricity perception
        // couple through their affinity slots.
        let mw = bit(METALWORKING);
        let mut territorial = Genome::neutral();
        territorial.set(GenomeSlot::Territoriality, 1.0);
        assert!(weapon_multiplier_coupled(mw, &territorial, true) > weapon_multiplier(mw));
        assert_eq!(weapon_multiplier_coupled(mw, &territorial, false), weapon_multiplier(mw));
        let hus = bit(HUSBANDRY);
        let mut agreeable = Genome::neutral();
        agreeable.set(GenomeSlot::Agreeableness, 1.0);
        assert!(scavenge_multiplier_coupled(hus, &agreeable, true) > scavenge_multiplier(hus));
        let elec = bit(ELECTRICITY);
        let mut open = Genome::neutral();
        open.set(GenomeSlot::Openness, 1.0);
        assert!(perception_multiplier_coupled(elec, &open, true) > perception_multiplier(elec));
        // Writing spread scales the same way.
        let w = bit(WRITING);
        let mut comms = Genome::neutral();
        comms.set(GenomeSlot::CommunicationStrength, 1.0);
        assert_eq!(spread_multiplier_coupled(w, &neutral, true), spread_multiplier(w));
        assert!(spread_multiplier_coupled(w, &comms, true) > spread_multiplier(w));
        assert_eq!(spread_multiplier_coupled(w, &comms, false), spread_multiplier(w));
        // Fire energy + Medicine lifespan coupled variants are identity when off.
        assert_eq!(
            food_energy_multiplier_coupled(bit(FIRE), &open, false),
            food_energy_multiplier(bit(FIRE))
        );
        assert_eq!(
            lifespan_multiplier_coupled(bit(MEDICINE), &open, false),
            lifespan_multiplier(bit(MEDICINE))
        );
        // Nuclear income couples through MutationRate.
        let nuke = bit(NUCLEAR_POWER);
        let mut mutator = Genome::neutral();
        mutator.set(GenomeSlot::MutationRate, 1.0);
        assert!(flat_upkeep_coupled(nuke, &mutator, true) < flat_upkeep(nuke));
        assert_eq!(flat_upkeep_coupled(nuke, &mutator, false), flat_upkeep(nuke));
        assert!((flat_upkeep_coupled(nuke, &neutral, true) - flat_upkeep(nuke)).abs() < 1e-6);
    }

    #[test]
    fn discovery_affinity_weight_is_neutral_off_and_scales_on() {
        use crate::genome::{Genome, GenomeSlot};
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Openness, 1.0);
        // OFF -> 1.0 always.
        assert_eq!(discovery_affinity_weight(&g, FIRE, false), 1.0);
        // ON, high affinity gene: > 1.0.
        assert!(discovery_affinity_weight(&g, FIRE, true) > 1.0);
        // Neutral gene -> 1.0 (keeps near-identity so tuning stays legible),
        // for every invention in the tree.
        for k in 0..INVENTION_COUNT {
            assert_eq!(discovery_affinity_weight(&Genome::neutral(), k, true), 1.0);
        }
        // Low gene damps discovery below 1.0.
        let mut lo = Genome::neutral();
        lo.set(GenomeSlot::Openness, 0.0);
        assert!(discovery_affinity_weight(&lo, FIRE, true) < 1.0);
        // Each invention reads its OWN affinity slot (Stone Tools ↔
        // IndividualLearning here), not the global Openness.
        let mut learner = Genome::neutral();
        learner.set(GenomeSlot::IndividualLearning, 1.0);
        assert!(discovery_affinity_weight(&learner, STONE_TOOLS, true) > 1.0);
        assert_eq!(discovery_affinity_weight(&learner, STONE_TOOLS, false), 1.0);
    }

    #[test]
    fn material_baskets_fit_the_economy() {
        // Per-good costs stay at/below the trade stock target (so a fully
        // stocked agent can afford any single tech), and the whole basket
        // fits inside base carrying capacity.
        for inv in INVENTIONS.iter() {
            let total: f32 = inv.materials.iter().sum();
            for &cost in inv.materials.iter() {
                assert!(
                    cost <= crate::resource::STOCK_TARGET + 1e-6,
                    "{} per-good cost {cost} exceeds STOCK_TARGET",
                    inv.name
                );
            }
            assert!(
                total <= crate::resource::INVENTORY_BASE_CAP + 1e-6,
                "{} basket total {total} exceeds base carrying capacity",
                inv.name
            );
        }
        // Era-scaled: baskets grow (non-strictly) with era on average.
        for era in 1..4u8 {
            let avg = |e: u8| {
                let v: Vec<f32> = INVENTIONS
                    .iter()
                    .filter(|inv| inv.era == e)
                    .map(|inv| inv.materials.iter().sum())
                    .collect();
                v.iter().sum::<f32>() / v.len() as f32
            };
            assert!(avg(era + 1) >= avg(era), "era {era} baskets got richer later");
        }
    }

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
