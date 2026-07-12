//! Culture: per-agent meme vectors transmitted between Communicator-equipped
//! neighbors with imperfect copy (design §3.1, §3.7 step 7, §4.4). Meme ops are
//! gated on the `Communicator` module.

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
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
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
        });
        for ch in 0..MEME_CHANNELS {
            // The skill channel is transmitted by social learning (below), not by
            // the broadcast-mean lerp.
            if ch == SKILL_CHANNEL {
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
    }
}
