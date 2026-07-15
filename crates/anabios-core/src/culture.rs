//! Culture: per-agent meme vectors transmitted between Communicator-equipped
//! neighbors with imperfect copy (design §3.1, §3.7 step 7, §4.4). Meme ops are
//! gated on the `Communicator` module.

use crate::genome::GenomeSlot;
use crate::module::{self, ModuleType};
use crate::program::MEME_CHANNELS;
use crate::rng::Rng;
use crate::spatial::PERCEPTION_MAX_RADIUS;
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
            .min(PERCEPTION_MAX_RADIUS);
        if range <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let mut sum = [0.0f32; MEME_CHANNELS];
        let mut count = [0u32; MEME_CHANNELS];
        // Social learning: track the most-skilled Communicator neighbour.
        let mut max_neighbour_skill = 0.0f32;
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
            // The skill and technique channels carry cumulative learned values
            // transmitted by their own social-learning rules below — they must NOT
            // be dragged toward the broadcast mean by the generic meme lerp (which
            // would pull them toward 0 and fight individual learning).
            if ch == SKILL_CHANNEL || ch == TECH_CHANNEL {
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
