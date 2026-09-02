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
/// Amplitude of the E10 secular climate-drift term added on top of the seasonal
/// optimum (`env_optimum_at`). The seasonal sweep spans `[0.25, 0.75]`; a `0.25`
/// amplitude lets a fully-drifted optimum reach the `[0, 1]` extremes (then
/// clamped), so the moving target can leave any camped genetic strategy behind.
/// Only active when a scenario sets `World.climate_drift_rate > 0`.
pub const CLIMATE_DRIFT_AMPLITUDE: f32 = 0.25;
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

/// Neighborhood radius (world units) scanned for terrain habitat selection.
pub const TERRAIN_HABITAT_REACH: f32 = 48.0;
/// Movement-bias strength pulling an agent toward its preferred terrain.
pub const TERRAIN_HABITAT_PULL: f32 = 1.0;

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
///
/// `drift_rate` (E10 drifting climate) adds a slow SECULAR term on top of the
/// seasonal sweep: `CLIMATE_DRIFT_AMPLITUDE * sin(drift_rate * tick)`, clamped
/// to `[0,1]`. Unlike the bounded seasonal cycle it never stationarizes — the
/// optimum's mean keeps wandering as the world ages, so selection pressure stays
/// non-stationary indefinitely. `drift_rate == 0.0` (every pre-E10 scenario)
/// short-circuits to the exact undrifted seasonal value, byte-identical to the
/// pre-drift codebase. Still pure (no RNG, no stored state).
pub fn env_optimum_at(tick: u64, period: u32, drift_rate: f32) -> f32 {
    let base = if period == ENV_STATIC_PERIOD {
        ENV_STATIC_OPTIMUM
    } else {
        let full = period as u64 * 2;
        let phase = (tick % full) as f32 / period as f32; // 0.0 .. 2.0
        let tri = if phase <= 1.0 { phase } else { 2.0 - phase }; // 0→1→0
        ENV_TECH_LOW + (ENV_TECH_HIGH - ENV_TECH_LOW) * tri
    };
    if drift_rate == 0.0 {
        // Determinism guarantee: with drift off, no float op touches `base`, so
        // the result is bit-identical to the pre-E10 function for every world.
        return base;
    }
    // Secular term computed in f64 (the phase `drift_rate * tick` grows without
    // bound) then folded to f32. Deterministic across platforms: the phase is a
    // fixed-order pure expression and the sine goes through the libm wrapper
    // (`f64::sin` is not correctly-rounded across host libms — see `mathf`).
    let secular =
        CLIMATE_DRIFT_AMPLITUDE * crate::mathf::sin64(drift_rate as f64 * tick as f64) as f32;
    (base + secular).clamp(0.0, 1.0)
}

/// Child meme = per-channel parent average plus small centered-uniform jitter.
/// Jitter uses a centered uniform draw scaled by `MEME_INHERIT_JITTER` (matches
/// the codebase's `perturb` style; determinism via the shared `rng`).
///
/// A channel is jittered only when its block is active, so widening the meme
/// vector to fit a new block never perturbs a scenario that has not opted into
/// it: base channels (`0..INVENTION_CHANNEL_BASE`) always jitter; invention
/// channels jitter iff `inventions_enabled`; practice channels jitter iff
/// `cognition_enabled`. An inactive block inherits the exact parent average with
/// no RNG draw, so the draw count (and thus every unrelated scenario's
/// trajectory) is byte-identical to before that block existed.
pub fn inherit_meme(
    a: &[f32; MEME_CHANNELS],
    b: &[f32; MEME_CHANNELS],
    rng: &mut Rng,
    inventions_enabled: bool,
    cognition_enabled: bool,
    fidelity: f32,
) -> [f32; MEME_CHANNELS] {
    let mut out = [0.0f32; MEME_CHANNELS];
    for ch in 0..MEME_CHANNELS {
        let avg = 0.5 * (a[ch] + b[ch]);
        let active = if crate::invention::is_invention_channel(ch) {
            inventions_enabled
        } else if crate::practice::is_practice_channel(ch) {
            cognition_enabled
        } else {
            true
        };
        // `fidelity` scales the jitter (E9 institutional memory: settled
        // cultures pass memes down more faithfully). 1.0 = baseline.
        out[ch] = if active {
            avg + (rng.f32_unit() - 0.5) * 2.0 * MEME_INHERIT_JITTER * fidelity
        } else {
            avg
        };
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
        transmit_to_receiver(world, id);
    }
    world.agents.scratch_ids = alive_ids;
}

/// One receiver's turn: gather the neighbour aggregates once, then run each
/// transmission rule over them. Rules are applied in a fixed order and each
/// owns a disjoint set of meme channels, so the sequence below is the whole
/// specification of what one Communicator learns this tick.
fn transmit_to_receiver(world: &mut World, id: u32) {
    let i = id as usize;
    if !module::has(&world.agents.modules[i], ModuleType::Communicator) {
        return;
    }
    let range = module::effective_communicator_range(&world.agents.modules[i])
        .min(world.spatial.perception_max_radius());
    if range <= 0.0 {
        return;
    }
    let pos = world.agents.position[i];
    // DIT env mode: the current optimum the neighbour scan matches techniques
    // against. Only meaningful when env mode is active.
    let env_on = world.env_period > 0;
    let opt = if env_on {
        env_optimum_at(world.tick, world.env_period, world.climate_drift_rate)
    } else {
        0.0
    };
    let mut scan = scan_neighbor_memes(world, id, pos, range, env_on, opt);
    // O2 payoff-biased learning (opt-in): re-target transmission before the
    // rules below consume the scan. Off ⇒ every target is exactly what the
    // payoff-blind scan produced (byte-identical).
    if world.payoff_biased_learning {
        apply_payoff_bias(world, &mut scan);
    }
    // O3 repro-biased learning (opt-in): decline practices whose local
    // holders demonstrably bury more infants. Runs after the energy-proxy
    // bias (both only ever zero practice targets, so order is benign).
    if world.repro_biased_learning {
        apply_repro_bias(&mut scan);
    }
    // Writing buff: the holder's meme copy rate and invention spread rate are
    // doubled (literacy accelerates all cultural transmission). The literacy
    // bonus scales with the holder's gene when `gene_tech_coupling` is on
    // (identity otherwise); computed once for both spread sites below.
    let self_mask = if world.inventions_enabled {
        crate::invention::held_mask(&world.agents.meme_vector[i])
    } else {
        0
    };
    let literacy = crate::invention::spread_multiplier_coupled(
        self_mask,
        &world.agents.genome[i],
        world.gene_tech_coupling,
    );

    transmit_broadcast_memes(world, i, &scan, MEME_COPY_RATE * literacy);
    transmit_skill(world, i, &scan);
    transmit_inventions(world, i, &scan, self_mask, literacy);
    transmit_practices(world, i, &scan);
    transmit_technique(world, i, &scan, env_on, opt);
}

/// E9 variant descent: adopt the teacher's meme variant on channel `ch` when
/// the receiver's freshly-learned level lands inside that variant's band. A
/// null variant (0) or an out-of-band value leaves the lineage untouched.
/// Call sites read the *post-write* level, so this must run after the copy.
fn adopt_teacher_variant(world: &mut World, i: usize, ch: usize, teacher_variant: u32) {
    if teacher_variant == 0 {
        return;
    }
    let new_val = world.agents.meme_vector[i][ch];
    let in_band = world
        .codex
        .meme_variants
        .get(&teacher_variant)
        .is_some_and(|v| v.band == crate::codex::traditions::band_of(new_val));
    if in_band {
        world.agents.meme_lineage[i][ch] = teacher_variant;
    }
}

/// O2 payoff-biased learning: re-point the scan's copy targets at evidence of
/// what actually pays off locally, before any rule consumes them.
fn apply_payoff_bias(world: &World, scan: &mut NeighborMemeScan) {
    // Model bias: copy from the highest-ENERGY Communicator neighbour rather
    // than the highest-trait-level one — the fittest available model tends to
    // carry fewer maladaptive practices.
    if let Some(m) = scan.best_model {
        scan.max_neighbour_skill = world.agents.meme_vector[m][SKILL_CHANNEL];
        scan.best_skill_neighbor = Some(m);
        if world.inventions_enabled {
            for k in 0..crate::invention::INVENTION_COUNT {
                let ch = crate::invention::channel(k);
                scan.max_neighbour_inv[k] = world.agents.meme_vector[m][ch];
                scan.max_neighbour_inv_variant[k] = world.agents.meme_lineage[m][ch];
            }
        }
        if world.cognition_enabled {
            for p in 0..crate::practice::PRACTICE_COUNT {
                let ch = crate::practice::channel(p);
                scan.max_neighbour_practice[p] = world.agents.meme_vector[m][ch];
                scan.max_neighbour_practice_variant[p] = world.agents.meme_lineage[m][ch];
            }
        }
    }
    // Content bias: decline a candidate trait when its local holders have lower
    // mean energy than its non-holders — the trait is empirically maladaptive
    // here, even if the chosen model carries it. Zeroing the copy target
    // declines it (targets start at 0.0 = "no neighbour holds it", and levels
    // are non-negative). Needs both groups present, else there is no local
    // evidence either way.
    for p in 0..crate::practice::PRACTICE_COUNT {
        let held = scan.holder_count[p];
        if held > 0 && held < scan.neighbour_count {
            let holder_mean = scan.holder_energy_sum[p] / held as f32;
            let non_mean = (scan.neighbour_energy_sum - scan.holder_energy_sum[p])
                / (scan.neighbour_count - held) as f32;
            if holder_mean < non_mean {
                scan.max_neighbour_practice[p] = 0.0;
            }
        }
    }
}

/// O3 repro-biased learning: content bias keyed on observed reproductive
/// success — the only fitness proxy that can see a stillbirth/cull cost
/// (energy cannot; O2b's measured failure). Scoped to practice channels
/// only, and deliberately WITHOUT model bias (O2b's model bias measurably
/// suppressed skill adoption). Decline a practice when, among Communicator
/// neighbours, holders' birth-failure fraction exceeds non-holders' — with
/// both groups present and at least one birth outcome observed in each,
/// else there is no local evidence either way.
fn apply_repro_bias(scan: &mut NeighborMemeScan) {
    for p in 0..crate::practice::PRACTICE_COUNT {
        let held = scan.holder_count_repro[p];
        if held == 0 || held >= scan.neighbour_count_repro {
            continue;
        }
        let h_ok = scan.holder_bok_sum[p];
        let h_fail = scan.holder_bfail_sum[p];
        let n_ok = scan.neighbour_bok_sum - h_ok;
        let n_fail = scan.neighbour_bfail_sum - h_fail;
        let h_births = h_ok + h_fail;
        let n_births = n_ok + n_fail;
        if h_births == 0 || n_births == 0 {
            continue;
        }
        let h_frac = h_fail as f32 / h_births as f32;
        let n_frac = n_fail as f32 / n_births as f32;
        if h_frac > n_frac {
            scan.max_neighbour_practice[p] = 0.0;
        }
    }
}

/// The generic meme lerp: drag each broadcast-carried channel toward the
/// neighbour mean.
///
/// The skill and technique channels carry cumulative learned values transmitted
/// by their own social-learning rules — they must NOT be dragged toward the
/// broadcast mean (which would pull them toward 0 and fight individual
/// learning). Invention and practice channels likewise have their own
/// copy-toward-best rules; they didn't exist before the tree, so excluding them
/// unconditionally is flag-off neutral.
fn transmit_broadcast_memes(world: &mut World, i: usize, scan: &NeighborMemeScan, rate: f32) {
    for ch in 0..MEME_CHANNELS {
        if ch == SKILL_CHANNEL
            || ch == TECH_CHANNEL
            || crate::invention::is_invention_channel(ch)
            || crate::practice::is_practice_channel(ch)
        {
            continue;
        }
        if scan.count[ch] > 0 {
            let received = scan.sum[ch] / scan.count[ch] as f32;
            let cur = world.agents.meme_vector[i][ch];
            world.agents.meme_vector[i][ch] = cur + rate * (received - cur);
        }
    }
}

/// Social learning of the foraging skill: copy toward the most-skilled
/// neighbour (you can learn a skill from an expert much faster than by
/// rediscovering it yourself).
fn transmit_skill(world: &mut World, i: usize, scan: &NeighborMemeScan) {
    let cur_skill = world.agents.meme_vector[i][SKILL_CHANNEL];
    if !(scan.count[0] > 0 && scan.max_neighbour_skill > cur_skill) {
        return;
    }
    world.agents.meme_vector[i][SKILL_CHANNEL] =
        cur_skill + SKILL_SOCIAL_RATE * (scan.max_neighbour_skill - cur_skill);
    if let Some(j) = scan.best_skill_neighbor {
        let tv = world.agents.meme_lineage[j][SKILL_CHANNEL];
        adopt_teacher_variant(world, i, SKILL_CHANNEL, tv);
    }
}

/// Invention spread: copy each adoptable invention toward the best-holding
/// neighbour's level (learn-from-the-expert, the same rule as the skill
/// channel). Every gate is prereq-, IQ-, material- and gene-checked against the
/// *receiver*, so the tree can't be skipped socially either.
fn transmit_inventions(
    world: &mut World,
    i: usize,
    scan: &NeighborMemeScan,
    self_mask: u32,
    literacy: f32,
) {
    if !world.inventions_enabled
        || !crate::invention::is_ape(&world.agents.genome[i], &world.agents.modules[i])
    {
        return;
    }
    let rate = crate::invention::INVENTION_SPREAD_RATE * literacy;
    for k in 0..crate::invention::INVENTION_COUNT {
        if !receiver_can_learn_invention(world, i, k, self_mask) {
            continue;
        }
        let target = scan.max_neighbour_inv[k];
        let ch = crate::invention::channel(k);
        let cur = world.agents.meme_vector[i][ch];
        if target > cur {
            world.agents.meme_vector[i][ch] = cur + rate * (target - cur);
            let new_val = world.agents.meme_vector[i][ch];
            // Completed the apprenticeship: crossing into functional adoption
            // consumes the material basket.
            if world.resources_enabled
                && cur < crate::invention::HELD_THRESHOLD
                && new_val >= crate::invention::HELD_THRESHOLD
            {
                crate::invention::consume_materials(&mut world.agents.inventory[i], k);
                world.codex.push_event(crate::codex::CodexEvent {
                    event_type: crate::codex::EventType::MaterialLearning,
                    tick: world.tick,
                    species_id: world.agents.species_id[i],
                    value: k as f32,
                    loc_x: world.agents.position[i].x,
                    loc_y: world.agents.position[i].y,
                });
            }
            adopt_teacher_variant(world, i, ch, scan.max_neighbour_inv_variant[k]);
        }
    }
}

/// The four gates an apprentice must clear before invention `k` can spread to
/// it: held foundations, cognition, materials, and genome. Each gate is inert
/// unless its feature flag is on.
fn receiver_can_learn_invention(world: &World, i: usize, k: usize, self_mask: u32) -> bool {
    crate::invention::INVENTIONS[k].prereqs & !self_mask == 0 // missing foundations
        && crate::invention::iq_permits(world.agents.iq[i], k, world.cognition_enabled)
        && crate::invention::materials_permit(
            &world.agents.inventory[i],
            k,
            world.resources_enabled,
        )
        && crate::invention::gene_permits(&world.agents.genome[i], k, world.gene_requirements)
}

/// Maladaptive practice spread: copy each practice toward the best-holding
/// neighbour (payoff-blind — the receiver has no idea it is harmful). IQ-gated
/// on the receiver, but the bar (`PRACTICE_IQ_REQ`) is deliberately low: only
/// the very lowest-cognition agents are spared, so maladaptive customs still
/// spread widely. The gate is loop-invariant (one receiver), so it is checked
/// once rather than per channel.
fn transmit_practices(world: &mut World, i: usize, scan: &NeighborMemeScan) {
    if !world.cognition_enabled
        || !crate::practice::iq_permits(world.agents.iq[i], world.cognition_enabled)
    {
        return;
    }
    for p in 0..crate::practice::PRACTICE_COUNT {
        let target = scan.max_neighbour_practice[p];
        let ch = crate::practice::channel(p);
        let cur = world.agents.meme_vector[i][ch];
        if target > cur {
            world.agents.meme_vector[i][ch] =
                cur + crate::practice::PRACTICE_SPREAD_RATE * (target - cur);
            adopt_teacher_variant(world, i, ch, scan.max_neighbour_practice_variant[p]);
        }
    }
}

/// DIT env mode: social learners copy the technique toward the best-matched
/// neighbour — but only if that neighbour is doing BETTER than they are
/// (payoff-biased imitation). Copying indiscriminately would drag a
/// well-adapted individual down toward its lagging neighbours; imitating only
/// your betters means social learning can help but never hurt.
fn transmit_technique(
    world: &mut World,
    i: usize,
    scan: &NeighborMemeScan,
    env_on: bool,
    opt: f32,
) {
    if !(env_on && world.agents.genome[i].get(GenomeSlot::SocialLearning) > 0.5) {
        return;
    }
    let Some(target_tech) = scan.best_tech else { return };
    let cur_tech = world.agents.meme_vector[i][TECH_CHANNEL];
    let own_err = (cur_tech - opt).abs();
    if scan.best_tech_err < own_err {
        world.agents.meme_vector[i][TECH_CHANNEL] =
            cur_tech + ENV_SOCIAL_RATE * (target_tech - cur_tech);
    }
}

/// Per-channel neighbour aggregates gathered in one spatial query, consumed by
/// the transmission rules in `culture_step`. Splitting the ~60-line scan closure
/// out of the driver keeps "gather" and "apply" as separate, readable phases.
struct NeighborMemeScan {
    /// Per-channel sum of neighbour broadcast intents.
    sum: [f32; MEME_CHANNELS],
    /// Per-channel count of Communicator neighbours (broadcasters).
    count: [u32; MEME_CHANNELS],
    /// Highest foraging-skill level seen among neighbours.
    max_neighbour_skill: f32,
    /// Index of the neighbour holding `max_neighbour_skill` (variant descent).
    best_skill_neighbor: Option<usize>,
    /// Best (highest) neighbour level per invention channel.
    max_neighbour_inv: [f32; crate::invention::INVENTION_COUNT],
    /// Variant id of the best-holding neighbour per invention channel.
    max_neighbour_inv_variant: [u32; crate::invention::INVENTION_COUNT],
    /// Best (highest) neighbour level per maladaptive-practice channel.
    max_neighbour_practice: [f32; crate::practice::PRACTICE_COUNT],
    /// Variant id of the best-holding neighbour per practice channel.
    max_neighbour_practice_variant: [u32; crate::practice::PRACTICE_COUNT],
    /// Technique of the neighbour best matching the DIT optimum (env mode only).
    best_tech: Option<f32>,
    /// `|tech - opt|` of `best_tech` (init `+inf`; env mode only).
    best_tech_err: f32,
    /// Highest-ENERGY Communicator neighbour — the model-bias copy source.
    /// Only populated when `world.payoff_biased_learning` is on.
    best_model: Option<usize>,
    /// Per practice channel: summed energy of holder neighbours
    /// (`practice::has`) and how many they are. Content-bias evidence;
    /// payoff-biased mode only.
    holder_energy_sum: [f32; crate::practice::PRACTICE_COUNT],
    holder_count: [u32; crate::practice::PRACTICE_COUNT],
    /// Total energy sum / count over all Communicator neighbours, so the
    /// content bias can derive non-holder means. Payoff-biased mode only.
    neighbour_energy_sum: f32,
    neighbour_count: u32,
    /// O3 repro-biased mode only: per practice channel, holders' summed
    /// birth outcomes (`births_ok`/`births_failed`) and holder count —
    /// aggregated independently of the payoff-mode fields so each flag's
    /// off-state stays byte-identical.
    holder_bok_sum: [u32; crate::practice::PRACTICE_COUNT],
    holder_bfail_sum: [u32; crate::practice::PRACTICE_COUNT],
    holder_count_repro: [u32; crate::practice::PRACTICE_COUNT],
    /// O3 repro-biased mode only: birth-outcome sums / count over ALL
    /// Communicator neighbours (non-holder stats derive by subtraction).
    neighbour_bok_sum: u32,
    neighbour_bfail_sum: u32,
    neighbour_count_repro: u32,
}

/// Scan the Communicator neighbours of agent `id` within `range`, aggregating
/// the per-channel values the transmission rules need. Pure read of `world`
/// (no mutation, no RNG) — identical iteration and comparison order to the
/// former inline closure, so the result is byte-for-byte the same.
fn scan_neighbor_memes(
    world: &World,
    id: u32,
    pos: crate::prelude::Vec2,
    range: f32,
    env_on: bool,
    opt: f32,
) -> NeighborMemeScan {
    let mut sum = [0.0f32; MEME_CHANNELS];
    let mut count = [0u32; MEME_CHANNELS];
    let mut max_neighbour_skill = 0.0f32;
    let mut max_neighbour_inv = [0.0f32; crate::invention::INVENTION_COUNT];
    let mut max_neighbour_practice = [0.0f32; crate::practice::PRACTICE_COUNT];
    let mut best_tech: Option<f32> = None;
    let mut best_tech_err = f32::INFINITY;
    let mut best_skill_neighbor: Option<usize> = None;
    let mut max_neighbour_inv_variant = [0u32; crate::invention::INVENTION_COUNT];
    let mut max_neighbour_practice_variant = [0u32; crate::practice::PRACTICE_COUNT];
    let payoff = world.payoff_biased_learning;
    let mut best_model: Option<usize> = None;
    let mut best_model_energy = f32::NEG_INFINITY;
    let mut holder_energy_sum = [0.0f32; crate::practice::PRACTICE_COUNT];
    let mut holder_count = [0u32; crate::practice::PRACTICE_COUNT];
    let mut neighbour_energy_sum = 0.0f32;
    let mut neighbour_count = 0u32;
    let repro = world.repro_biased_learning;
    let mut holder_bok_sum = [0u32; crate::practice::PRACTICE_COUNT];
    let mut holder_bfail_sum = [0u32; crate::practice::PRACTICE_COUNT];
    let mut holder_count_repro = [0u32; crate::practice::PRACTICE_COUNT];
    let mut neighbour_bok_sum = 0u32;
    let mut neighbour_bfail_sum = 0u32;
    let mut neighbour_count_repro = 0u32;
    world.spatial.query(pos, range, |oid| {
        if oid == id {
            return;
        }
        let j = oid as usize;
        if !module::has(&world.agents.modules[j], ModuleType::Communicator) {
            return;
        }
        if payoff {
            let e = world.agents.energy[j];
            neighbour_energy_sum += e;
            neighbour_count += 1;
            if e > best_model_energy {
                best_model_energy = e;
                best_model = Some(j);
            }
            for p in 0..crate::practice::PRACTICE_COUNT {
                if crate::practice::has(&world.agents.meme_vector[j], p) {
                    holder_energy_sum[p] += e;
                    holder_count[p] += 1;
                }
            }
        }
        if repro {
            let bok = world.agents.births_ok[j] as u32;
            let bfail = world.agents.births_failed[j] as u32;
            neighbour_bok_sum += bok;
            neighbour_bfail_sum += bfail;
            neighbour_count_repro += 1;
            for p in 0..crate::practice::PRACTICE_COUNT {
                if crate::practice::has(&world.agents.meme_vector[j], p) {
                    holder_bok_sum[p] += bok;
                    holder_bfail_sum[p] += bfail;
                    holder_count_repro[p] += 1;
                }
            }
        }
        for ch in 0..MEME_CHANNELS {
            sum[ch] += world.actions[j].broadcast_intent[ch];
            count[ch] += 1;
        }
        let j_skill = world.agents.meme_vector[j][SKILL_CHANNEL];
        if j_skill > max_neighbour_skill {
            max_neighbour_skill = j_skill;
            best_skill_neighbor = Some(j);
        }
        if world.inventions_enabled {
            for (k, best) in max_neighbour_inv.iter_mut().enumerate() {
                let ch = crate::invention::channel(k);
                let v = world.agents.meme_vector[j][ch];
                if v > *best {
                    *best = v;
                    // E9: remember whose variant the best level came from,
                    // so adoption can follow the variant socially.
                    max_neighbour_inv_variant[k] = world.agents.meme_lineage[j][ch];
                }
            }
        }
        if world.cognition_enabled {
            for (p, best) in max_neighbour_practice.iter_mut().enumerate() {
                let ch = crate::practice::channel(p);
                let v = world.agents.meme_vector[j][ch];
                if v > *best {
                    *best = v;
                    max_neighbour_practice_variant[p] = world.agents.meme_lineage[j][ch];
                }
            }
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
    NeighborMemeScan {
        sum,
        count,
        max_neighbour_skill,
        best_skill_neighbor,
        max_neighbour_inv,
        max_neighbour_inv_variant,
        max_neighbour_practice,
        max_neighbour_practice_variant,
        best_tech,
        best_tech_err,
        best_model,
        holder_energy_sum,
        holder_count,
        neighbour_energy_sum,
        neighbour_count,
        holder_bok_sum,
        holder_bfail_sum,
        holder_count_repro,
        neighbour_bok_sum,
        neighbour_bfail_sum,
        neighbour_count_repro,
    }
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

    /// The undrifted seasonal triangle, recomputed independently of the function
    /// under test — the reference for "exactly as before" (drift_rate == 0.0).
    fn seasonal_reference(tick: u64, period: u32) -> f32 {
        let full = period as u64 * 2;
        let phase = (tick % full) as f32 / period as f32;
        let tri = if phase <= 1.0 { phase } else { 2.0 - phase };
        super::ENV_TECH_LOW + (super::ENV_TECH_HIGH - super::ENV_TECH_LOW) * tri
    }

    #[test]
    fn drift_off_is_exactly_seasonal() {
        // (a) rate 0.0 → the optimum is bit-identical to the pure seasonal value
        // at every tick across several periods and phases.
        let period = 4000u32;
        for tick in [0u64, 1, 999, 2000, 4000, 6001, 123_457, 1_000_000] {
            let got = super::env_optimum_at(tick, period, 0.0);
            let want = seasonal_reference(tick, period);
            assert_eq!(got.to_bits(), want.to_bits(), "drift-off mismatch at tick {tick}");
        }
        // Static-optimum branch is likewise untouched with drift off.
        assert_eq!(
            super::env_optimum_at(500, super::ENV_STATIC_PERIOD, 0.0),
            super::ENV_STATIC_OPTIMUM
        );
    }

    #[test]
    fn drift_shifts_long_run_mean_monotonically() {
        // (b) rate > 0 → the optimum's long-run mean wanders. Average over whole
        // seasonal cycles (so the triangle contributes exactly 0.5, isolating the
        // secular term), across three successive early windows on the rising
        // quarter of the drift sine; the windowed mean must climb monotonically.
        let period = 4000u32;
        let window_ticks = (period as u64) * 2 * 4; // four full seasonal cycles
                                                    // Slow rate: rising quarter of the sine lasts ~PI/2 / rate ticks; keep the
                                                    // sampled windows well inside it so drift is strictly increasing.
        let rate = 1.0e-6f32;

        let window_mean = |start: u64| -> f64 {
            let mut sum = 0.0f64;
            for t in start..start + window_ticks {
                sum += super::env_optimum_at(t, period, rate) as f64;
            }
            sum / window_ticks as f64
        };

        let m0 = window_mean(0);
        let m1 = window_mean(200_000);
        let m2 = window_mean(400_000);

        // Drift is real and monotone-ish over this horizon.
        assert!(m1 > m0 + 1e-3, "expected rising mean: m0={m0}, m1={m1}");
        assert!(m2 > m1 + 1e-3, "expected rising mean: m1={m1}, m2={m2}");
        // And the total shift is a meaningful fraction of the drift amplitude,
        // not float noise — the optimum genuinely moved, not just wobbled.
        assert!((m2 - m0) > 0.05, "drift too weak to matter: {}", m2 - m0);
        // Sanity: the undrifted mean over whole cycles is exactly the 0.5 midpoint.
        let mut base_sum = 0.0f64;
        for t in 0..window_ticks {
            base_sum += super::env_optimum_at(t, period, 0.0) as f64;
        }
        assert!((base_sum / window_ticks as f64 - 0.5).abs() < 1e-6);
    }
}
