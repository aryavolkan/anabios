//! Culture: per-agent meme vectors transmitted between Communicator-equipped
//! neighbors with imperfect copy (design §3.1, §3.7 step 7, §4.4). Meme ops are
//! gated on the `Communicator` module.

use crate::genome::GenomeSlot;
use crate::module::{self, ModuleType};
use crate::program::MEME_CHANNELS;
use crate::rng::Rng;
use crate::world::World;

/// Fraction each receiver moves its meme toward the neighbor mean per tick
/// (the "imperfect copy" — < 1.0 means partial adoption).
pub const MEME_COPY_RATE: f32 = 0.05;
/// `broadcast_intent[ch]` above this counts as an active broadcast this tick.
pub const MEME_BROADCAST_THRESHOLD: f32 = 0.5;
/// Half-range of the centered-uniform per-channel jitter added to an inherited
/// meme vector (jitter is drawn from `[-MEME_INHERIT_JITTER, +MEME_INHERIT_JITTER]`).
pub const MEME_INHERIT_JITTER: f32 = 0.05;
/// The meme channel used for alarm calls (AlarmCall detector).
pub const ALARM_MEME_CHANNEL: usize = 0;

// --- Cumulative cultural skill (experiment C: gene-culture coevolution) ---
// A "foraging skill" that raises feeding efficiency. It is LEARNED within a
// lifetime by feeding (experience, not genetics) and can be SOCIALLY COPIED from
// a more-skilled neighbour far faster than grinding it alone. Both learning and
// the feeding bonus are gated on the `Communicator` module — culture-capable
// cognition — so it gives culture an adaptive niche genes cannot fill, and
// leaves non-Communicator baselines (minimal.toml) unchanged.
/// Meme channel holding the learned foraging skill in `[0,1]`.
pub const SKILL_CHANNEL: usize = 5;
/// Per-successful-feed increment toward mastery (asymptotic, learning-by-doing).
pub const SKILL_LEARN_RATE: f32 = 0.03;
/// Fraction a Communicator moves its skill toward a more-skilled neighbour's
/// skill each tick (social learning — much faster than solo learning).
pub const SKILL_SOCIAL_RATE: f32 = 0.15;
/// Extra feeding multiplier at full skill: bite *= 1 + SKILL_BONUS * skill.
pub const SKILL_BONUS: f32 = 2.5;

// --- Mutation-gated cultural inventions (ratchet: invent slowly solo, copy
// fast socially) ---
// A cumulative "invention level" that only an Inventiveness-gened Communicator
// can push forward through its own foraging (slow), but that ANY inventive
// Communicator can pick up fast from a more-advanced neighbour (social copy).
// Gated end-to-end on `World.cultural_inventions` so it is fully inert
// (byte-identical) unless a scenario opts in.
/// Meme channel carrying the cumulative cultural INVENTION LEVEL in `[0,1]`.
pub const INVENTION_CHANNEL: usize = 7;
/// Slow solo progress per successful foraging tick (invent-from-scratch rate).
pub const INVENT_RATE: f32 = 0.01;
/// Fast copy rate toward the best inventive Communicator neighbour's level.
pub const INVENT_SOCIAL_RATE: f32 = 0.15;
/// `GenomeSlot::Inventiveness` above this gene value makes an agent capable of
/// (solo) invention. `Genome::neutral()`'s 0.5 is NOT inventive (strict `>`).
pub const INVENTIVE_THRESHOLD: f32 = 0.5;

/// Whether this genome's `Inventiveness` gene clears the inventive threshold.
pub fn is_inventive(g: &crate::genome::Genome) -> bool {
    g.get(crate::genome::GenomeSlot::Inventiveness) > INVENTIVE_THRESHOLD
}

/// Read the cumulative invention level out of a meme vector.
pub fn invention_level(meme: &[f32; MEME_CHANNELS]) -> f32 {
    meme[INVENTION_CHANNEL]
}

// --- Named tech-tree: cumulative robust benefits unlocked at invention-level
// tiers (Task 2.1). Each tier compounds on the ratchet above: Domestication
// unlocks first (lowest bar), then Writing, then the Industrial Revolution
// (requires the full ratchet). All three are gated end-to-end through
// `invention_active` below, so flag-off / non-inventive / non-Communicator
// agents are completely unaffected (golden-neutral).
/// Invention-level tier at which Domestication (steady food income) unlocks.
pub const DOMESTICATION_THRESHOLD: f32 = 0.34;
/// Flat additive energy gained per foraging tick once Domestication is active.
pub const DOMESTICATION_ENERGY: f32 = 0.15;
/// Invention-level tier at which Writing (faster cultural transmission) unlocks.
pub const WRITING_THRESHOLD: f32 = 0.67;
/// Extra invention-copy rate added to `INVENT_SOCIAL_RATE` for a copier that
/// has reached the Writing tier.
pub const WRITING_COPY_BONUS: f32 = 0.20;
/// Invention-level tier at which the Industrial Revolution (metabolic and
/// reproductive efficiency) unlocks — the top of the ratchet.
pub const INDUSTRY_THRESHOLD: f32 = 1.0;
/// Per-tick module-upkeep discount once the Industry tier is active.
pub const INDUSTRY_UPKEEP_DISCOUNT: f32 = 0.004;
/// Reduction to the effective `ReproductionThreshold` gene once the Industry
/// tier is active.
pub const INDUSTRY_REPRO_DISCOUNT: f32 = 0.05;

/// Does an agent currently benefit from the invention tier unlocked at `threshold`?
pub fn invention_active(
    flag: bool,
    g: &crate::genome::Genome,
    meme: &[f32; MEME_CHANNELS],
    has_comm: bool,
    threshold: f32,
) -> bool {
    flag && has_comm && is_inventive(g) && invention_level(meme) >= threshold
}

// --- DIT environmental-variability technique (experiment) ---
// A culturally/genetically-carried foraging *technique* matched against a
// possibly-shifting environmental optimum. Inert unless `World.env_period > 0`.
/// Meme channel holding the culturally-transmitted foraging *technique* in `[0,1]`
/// (distinct from SKILL_CHANNEL). Used only when `World.env_period > 0`.
pub const TECH_CHANNEL: usize = 6;
/// Sentinel env_period meaning "mechanism active but the optimum never shifts" (static test).
pub const ENV_STATIC_PERIOD: u32 = u32::MAX;
/// The two technique optima the environment alternates between.
pub const ENV_TECH_LOW: f32 = 0.25;
pub const ENV_TECH_HIGH: f32 = 0.75;
/// The static-env optimum (what the genetic strategy can evolve toward).
pub const ENV_STATIC_OPTIMUM: f32 = 0.75;
/// Extra feeding multiplier when technique matches the optimum: bite *= 1 + ENV_BONUS*match.
/// Deliberately modest: a large bonus causes competitive exclusion (the matched
/// strategy grazes the biome flat, starving mismatched agents before they can
/// re-learn), which traps trackers mid-transition. A gentle edge lets mismatched
/// agents still feed and learn their way back, so tracking can actually pay off.
pub const ENV_BONUS: f32 = 0.5;
/// Per-successful-feed step of individual technique learning toward the current
/// optimum. Fast enough that a tracker re-matches within a few feeds after the
/// optimum shifts (so the transition is a brief dip, not a starvation event).
pub const ENV_LEARN_RATE: f32 = 0.15;
/// Flat energy cost paid each foraging tick an agent individually learns (now
/// per-tick, so kept small — real enough to make learning a cost the fixed
/// genetic strategy avoids, but survivable).
pub const ENV_LEARN_COST: f32 = 0.005;
/// Fraction a social learner moves its technique toward the best-matched neighbour's technique per tick.
pub const ENV_SOCIAL_RATE: f32 = 0.20;
/// Half-width of the technique-match kernel. A technique more than this far from
/// the current optimum earns NO bonus — so a fixed "generalist" technique midway
/// between two alternating optima (0.5 apart) gets nothing, not partial credit.
/// This is what makes environmental variability actually punish the genetic hedge
/// and reward tracking (the canonical DIT payoff).
pub const ENV_TOLERANCE: f32 = 0.2;

/// Foraging-efficiency factor in `[0,1]`: a triangular kernel, 1.0 at a perfect
/// technique match and falling linearly to 0.0 at `ENV_TOLERANCE` away. Used by
/// both the feeding bonus and the experiment harnesses so "match" == fitness.
pub fn technique_match(tech: f32, opt: f32) -> f32 {
    (1.0 - (tech - opt).abs() / ENV_TOLERANCE).clamp(0.0, 1.0)
}

/// Feeding bonus multiplier for a perfect biome-climate affinity match (spatial
/// genetic analog of the DIT technique bonus).
pub const ENV_AFFINITY_BONUS: f32 = 1.0;
/// Affinity distance beyond which the biome-adaptation bonus is zero.
pub const ENV_AFFINITY_TOLERANCE: f32 = 0.25;

/// Triangular match kernel for genetic biome-climate adaptation: 1.0 at a
/// perfect match, linearly to 0.0 at `ENV_AFFINITY_TOLERANCE` apart. Both args
/// in `[0,1]`.
pub fn env_affinity_match(affinity: f32, env: f32) -> f32 {
    (1.0 - (affinity - env).abs() / ENV_AFFINITY_TOLERANCE).clamp(0.0, 1.0)
}

/// Habitat-selection reach (world units): how far an agent scans for a cell
/// whose climate better matches its `EnvAffinity`. Feeding-bonus adaptation
/// alone yields panmixia (agents roam past climate features and adapt to the
/// global mean); this movement pull lets lineages sort into their preferred
/// zone, producing the spatial structure a cline needs.
pub const HABITAT_REACH: f32 = 48.0;
/// Weight of the affinity-matching pull added to an agent's movement intent
/// before normalization. Comparable to the program/personality move magnitudes
/// so climate-seeking meaningfully biases direction without fully overriding
/// the evolved behavior.
pub const HABITAT_PULL: f32 = 1.0;

/// The globally-optimal foraging technique at a given tick, in `[0,1]`. Pure (no RNG,
/// no stored state) so it needs no tick hook and stays perfectly deterministic.
/// `period == 0` should never reach here (callers gate on env_period > 0).
/// `period == ENV_STATIC_PERIOD` → a fixed optimum (static environment).
///
/// Otherwise the optimum SWEEPS continuously as a triangle wave between LOW and
/// HIGH, completing a full LOW→HIGH→LOW cycle every `2 * period` ticks. A moving
/// optimum (rather than two discrete states) denies a fixed genetic strategy any
/// permanent "refuge" optimum to camp — which is what lets a tracker actually win
/// under slow change and lose under fast change (the canonical DIT boundary).
pub fn env_optimum_at(tick: u64, period: u32) -> f32 {
    if period == ENV_STATIC_PERIOD {
        return ENV_STATIC_OPTIMUM;
    }
    let full = period as u64 * 2;
    let phase = (tick % full) as f32 / period as f32; // 0.0 .. 2.0
    let tri = if phase <= 1.0 { phase } else { 2.0 - phase }; // 0→1→0
    ENV_TECH_LOW + (ENV_TECH_HIGH - ENV_TECH_LOW) * tri
}

/// Child meme = per-channel parent average plus small centered-uniform jitter.
/// Jitter uses a centered uniform draw scaled by `MEME_INHERIT_JITTER` (matches
/// the codebase's `perturb` style; determinism via the shared `rng`).
/// Draw count is exactly `MEME_CHANNELS` per inheriting birth.
pub fn inherit_meme(
    a: &[f32; MEME_CHANNELS],
    b: &[f32; MEME_CHANNELS],
    rng: &mut Rng,
) -> [f32; MEME_CHANNELS] {
    let mut out = [0.0f32; MEME_CHANNELS];
    for ch in 0..MEME_CHANNELS {
        let jitter = (rng.f32_unit() - 0.5) * 2.0 * MEME_INHERIT_JITTER;
        out[ch] = 0.5 * (a[ch] + b[ch]) + jitter;
    }
    out
}

/// Transmit memes between Communicator neighbors: each receiver lerps its meme
/// vector toward the mean of nearby communicators' broadcasts. Deterministic
/// (no RNG); iterates alive ids ascending. The received value comes from
/// `broadcast_intent` (fixed this tick), so in-place updates don't interfere.
pub fn culture_step(world: &mut World) {
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());
    for &id in &alive_ids {
        let i = id as usize;
        if !module::has(&world.agents.modules[i], ModuleType::Communicator) {
            continue;
        }
        let range = module::effective_communicator_range(&world.agents.modules[i])
            .min(world.spatial.perception_max_radius());
        if range <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let mut sum = [0.0f32; MEME_CHANNELS];
        let mut count = [0u32; MEME_CHANNELS];
        // Social learning: track the most-skilled Communicator neighbour.
        let mut max_neighbour_skill = 0.0f32;
        // Cultural-inventions ratchet: track the highest invention level among
        // inventive Communicator neighbours (only meaningful when the flag is
        // on, but harmless to compute otherwise since it stays 0).
        let inventions_on = world.cultural_inventions;
        let mut best_neighbour_invention = 0.0f32;
        // DIT env mode: the current optimum, and the neighbour whose technique
        // best matches it (minimizes |tech - opt|). Only computed when active.
        let env_on = world.env_period > 0;
        let opt = if env_on { env_optimum_at(world.tick, world.env_period) } else { 0.0 };
        let mut best_tech: Option<f32> = None;
        let mut best_tech_err = f32::INFINITY;
        world.spatial.query(pos, range, |oid| {
            if oid == id {
                return;
            }
            let j = oid as usize;
            if !module::has(&world.agents.modules[j], ModuleType::Communicator) {
                return;
            }
            for ch in 0..MEME_CHANNELS {
                sum[ch] += world.actions[j].broadcast_intent[ch];
                count[ch] += 1;
            }
            max_neighbour_skill =
                max_neighbour_skill.max(world.agents.meme_vector[j][SKILL_CHANNEL]);
            if inventions_on && is_inventive(&world.agents.genome[j]) {
                best_neighbour_invention =
                    best_neighbour_invention.max(world.agents.meme_vector[j][INVENTION_CHANNEL]);
            }
            if env_on {
                let tech = world.agents.meme_vector[j][TECH_CHANNEL];
                let err = (tech - opt).abs();
                if err < best_tech_err {
                    best_tech_err = err;
                    best_tech = Some(tech);
                }
            }
        });
        for ch in 0..MEME_CHANNELS {
            // The skill, technique, and invention channels carry cumulative
            // learned values transmitted by their own social-learning rules
            // below — they must NOT be dragged toward the broadcast mean by
            // the generic meme lerp (which would pull them toward 0 and fight
            // individual learning). The invention exclusion is itself gated on
            // `cultural_inventions`: unlike SKILL/TECH, channel 7 can still be
            // touched by ordinary structural-mutation Communicators when the
            // flag is off (e.g. minimal.toml), so unconditionally excluding it
            // would change pre-existing (flag-off) behaviour and move the
            // golden hashes. Gating keeps flag-off byte-identical.
            if ch == SKILL_CHANNEL
                || ch == TECH_CHANNEL
                || (ch == INVENTION_CHANNEL && inventions_on)
            {
                continue;
            }
            if count[ch] > 0 {
                let received = sum[ch] / count[ch] as f32;
                let cur = world.agents.meme_vector[i][ch];
                world.agents.meme_vector[i][ch] = cur + MEME_COPY_RATE * (received - cur);
            }
        }
        // Social learning of the foraging skill: copy toward the most-skilled
        // neighbour (you can learn a skill from an expert much faster than by
        // rediscovering it yourself).
        let cur_skill = world.agents.meme_vector[i][SKILL_CHANNEL];
        if count[0] > 0 && max_neighbour_skill > cur_skill {
            world.agents.meme_vector[i][SKILL_CHANNEL] =
                cur_skill + SKILL_SOCIAL_RATE * (max_neighbour_skill - cur_skill);
        }
        // Cultural-inventions ratchet: an inventive Communicator copies fast
        // from the best-inventing Communicator neighbour (much faster than its
        // own slow solo invention in `feed_pass`) — the "ratchet" that lets a
        // rare breakthrough spread through the population instead of staying
        // locked to the inventor who found it.
        if inventions_on && is_inventive(&world.agents.genome[i]) {
            let cur = world.agents.meme_vector[i][INVENTION_CHANNEL];
            if best_neighbour_invention > cur {
                // Writing tier (Task 2.1): a copier who has already reached the
                // Writing invention level transmits/absorbs faster — the copy
                // rate itself compounds on the ratchet. `has_comm` is always
                // `true` here: this loop already `continue`d past every agent
                // lacking a Communicator module above.
                let rate = if invention_active(
                    inventions_on,
                    &world.agents.genome[i],
                    &world.agents.meme_vector[i],
                    true,
                    WRITING_THRESHOLD,
                ) {
                    INVENT_SOCIAL_RATE + WRITING_COPY_BONUS
                } else {
                    INVENT_SOCIAL_RATE
                };
                world.agents.meme_vector[i][INVENTION_CHANNEL] =
                    (cur + rate * (best_neighbour_invention - cur)).clamp(0.0, 1.0);
            }
        }
        // DIT env mode: social learners copy the technique toward the best-matched
        // neighbour — but only if that neighbour is doing BETTER than they are
        // (payoff-biased imitation). Copying indiscriminately would drag a
        // well-adapted individual down toward its lagging neighbours; imitating
        // only your betters means social learning can help but never hurt.
        if env_on && world.agents.genome[i].get(GenomeSlot::SocialLearning) > 0.5 {
            if let Some(target_tech) = best_tech {
                let cur_tech = world.agents.meme_vector[i][TECH_CHANNEL];
                let own_err = (cur_tech - opt).abs();
                if best_tech_err < own_err {
                    world.agents.meme_vector[i][TECH_CHANNEL] =
                        cur_tech + ENV_SOCIAL_RATE * (target_tech - cur_tech);
                }
            }
        }
    }
    world.agents.scratch_ids = alive_ids;
}

#[cfg(test)]
mod tests {
    #[test]
    fn env_affinity_match_peaks_and_falls_off() {
        assert!((super::env_affinity_match(0.5, 0.5) - 1.0).abs() < 1e-6);
        assert_eq!(super::env_affinity_match(0.0, 1.0), 0.0); // > tolerance apart
        let m = super::env_affinity_match(0.5, 0.6);
        assert!(m > 0.0 && m < 1.0);
        for (a, e) in [(0.2, 0.9), (1.0, 0.0), (0.5, 0.5)] {
            let v = super::env_affinity_match(a, e);
            assert!((0.0..=1.0).contains(&v));
        }
    }
}
