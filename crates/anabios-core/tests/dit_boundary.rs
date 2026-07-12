//! DIT boundary suite — where gene-culture coevolution WORKS and where it FAILS.
//!
//! Companion to `gene_culture.rs`. Maps the dual-inheritance-theory boundary along
//! the canonical axes (Rogers / Boyd-Richerson): environmental change rate, the
//! pure-imitator paradox and its resolution, plus two ecological pre-conditions on
//! the existing C cumulative-skill mechanism (social structure, resource level).
//!
//! Method: the env axis is measured by TRACKING QUALITY — each strategy's run-time
//! -averaged technique-match against the (possibly moving) environmental optimum.
//! This isolates adaptation from the noisier population-share dynamics. Each test
//! asserts the PREDICTED DIRECTION across seeds, so "works/fails" is encoded, not
//! just printed. Analysis harnesses — run explicitly with:
//!   cargo test -p anabios-core --release --test dit_boundary -- --ignored --nocapture

use anabios_core::culture::{env_optimum_at, technique_match, ENV_STATIC_PERIOD, TECH_CHANNEL};
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::module::{starter_kit, Module, ModuleList};
use anabios_core::prelude_test::Vec2;
use anabios_core::program::starter_asocial_forager;
use anabios_core::tick::step;
use anabios_core::world::World;

const ENV_SLOW: u32 = 400; // slow triangle-wave sweep of the optimum
const ENV_FAST: u32 = 8; // sweep faster than the learn loop can follow
const ENV_TICKS: u32 = 1200; // measurement window (populations survive it)
const ENV_N: usize = 40; // founders per population

/// A forager strategy: whether it carries a `Communicator` module (needed only for
/// SOCIAL learning; individual learning is ungated), plus its DIT genome settings.
#[derive(Clone, Copy)]
struct Strat {
    comm: bool,
    il: f32,
    sl: f32,
    innate: f32,
}

// The four DIT strategies. Reproductive capability is identical across all of them
// (see `kit_for`); they differ only in the learning treatment.
const INNATE: Strat = Strat { comm: false, il: 0.0, sl: 0.0, innate: 0.5 };
const INDIVIDUAL: Strat = Strat { comm: false, il: 1.0, sl: 0.0, innate: 0.0 };
const IMITATOR: Strat = Strat { comm: true, il: 0.0, sl: 1.0, innate: 0.0 };
const CRITICAL: Strat = Strat { comm: true, il: 1.0, sl: 1.0, innate: 0.0 };

/// Build a forager kit that is IDENTICAL across strategies (Locomotor, Sensor,
/// Mouth, Reproductive — all can feed and breed) and adds a `Communicator` module
/// only when the strategy needs the social-learning channel. Essential: the stock
/// `communicator_kit()` swaps out Reproductive for Communicator, so using it would
/// cripple cultural lineages' reproduction and confound the comparison.
fn kit_for(comm: bool) -> ModuleList {
    let mut k = starter_kit();
    if comm {
        k.push(Module::Communicator { range: 12.0, channel_id: 0 });
    }
    k
}

/// Grow the per-species tables so `species` is a valid index (mirrors what
/// `Scenario::instantiate` does for archetype specs).
fn ensure_species(w: &mut World, species: u32) {
    while w.species_centroids.len() <= species as usize {
        w.species_centroids.push(Genome::neutral());
        w.species_parents.push(Some(0));
        w.species_member_counts.push(0);
    }
    if w.next_species_id <= species {
        w.next_species_id = species + 1;
    }
}

/// Spawn `n` agents of a strategy into `species`, clustered around a center. In env
/// mode every founder starts matched to the tick-0 optimum (the genetic strategy
/// pre-adapted; the cultural technique meme likewise seeded), avoiding a
/// pathological cold start.
fn spawn_lineage(w: &mut World, species: u32, n: usize, s: Strat, cx: f32, cy: f32, repro: f32) {
    ensure_species(w, species);
    let seed_opt = if w.env_period > 0 { Some(env_optimum_at(0, w.env_period)) } else { None };
    for k in 0..n {
        let ang = k as f32 * 0.7;
        let rad = 12.0 + (k % 11) as f32 * 7.0;
        let pos = Vec2::new(cx + rad * ang.cos(), cy + rad * ang.sin());
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, repro);
        g.set(GenomeSlot::IndividualLearning, s.il);
        g.set(GenomeSlot::SocialLearning, s.sl);
        g.set(GenomeSlot::InnateTechnique, seed_opt.unwrap_or(s.innate));
        let id = w.spawn_seeded(pos, g, species, kit_for(s.comm), starter_asocial_forager());
        if let (Some(opt), true) = (seed_opt, s.il > 0.5 || s.sl > 0.5) {
            w.agents.meme_vector[id as usize][TECH_CHANNEL] = opt;
        }
    }
}

/// The technique an agent currently forages with — `meme[TECH]` for cultural
/// agents, the innate genome slot for genetic agents (mirrors `feed_pass`).
fn tech_of(w: &World, i: usize) -> f32 {
    let il = w.agents.genome[i].get(GenomeSlot::IndividualLearning) > 0.5;
    let sl = w.agents.genome[i].get(GenomeSlot::SocialLearning) > 0.5;
    if il || sl {
        w.agents.meme_vector[i][TECH_CHANNEL]
    } else {
        w.agents.genome[i].get(GenomeSlot::InnateTechnique)
    }
}

/// Alive count of a species.
fn count(w: &World, species: u32) -> usize {
    w.agents.iter_alive().filter(|&id| w.agents.species_id[id as usize] == species).count()
}

/// Per-species instantaneous (sum of technique-match, member count) at this tick.
fn match_sum(w: &World, species: u32) -> (f64, u64) {
    let opt = env_optimum_at(w.tick, w.env_period);
    let mut sm = 0.0f64;
    let mut n = 0u64;
    for id in w.agents.iter_alive() {
        let i = id as usize;
        if w.agents.species_id[i] != species {
            continue;
        }
        sm += technique_match(tech_of(w, i), opt) as f64;
        n += 1;
    }
    (sm, n)
}

/// Run ONE population of a strategy under an env regime; return its run-time
/// -averaged per-capita technique-match — how well, on average, a member's
/// technique matched the optimum over the whole run. This is the tracking metric.
fn track_quality(seed: u64, env_period: u32, s: Strat) -> f64 {
    let mut w = World::new(seed);
    w.env_period = env_period;
    spawn_lineage(&mut w, 1, ENV_N, s, 512.0, 512.0, 0.3);
    let (mut sm, mut cn) = (0.0f64, 0u64);
    for _ in 0..ENV_TICKS {
        step(&mut w);
        let (s1, n1) = match_sum(&w, 1);
        sm += s1;
        cn += n1;
    }
    if cn > 0 {
        sm / cn as f64
    } else {
        0.0
    }
}

// ----------------------------------------------------------------------------
// Axis 1: environmental change rate (slow / fast / static)
// ----------------------------------------------------------------------------

/// SLOW/intermediate change ⇒ culture WORKS. The optimum sweeps slowly; a cultural
/// learner tracks it (learn + copy), while a fixed genetic strategy cannot follow a
/// moving target and matches only in passing. The tracker's mean match is far higher.
#[ignore = "experiment harness — run with --ignored --nocapture"]
#[test]
fn dit_env_slow_culture_tracks() {
    const SEEDS: u64 = 8;
    let mut culture_wins = 0;
    for seed in 0..SEEDS {
        let m_learn = track_quality(seed, ENV_SLOW, CRITICAL);
        let m_gene = track_quality(seed, ENV_SLOW, INNATE);
        if m_learn > m_gene + 0.2 {
            culture_wins += 1;
        }
        eprintln!("SLOW seed{seed}: critical match={m_learn:.2} | innate match={m_gene:.2}");
    }
    eprintln!(
        "SLOW RESULT: tracker clearly out-matched fixed genes in {culture_wins}/{SEEDS} seeds"
    );
    assert!(
        culture_wins * 2 > SEEDS,
        "under slow change the cultural tracker should track far better than a fixed genetic strategy"
    );
}

/// FAST change ⇒ culture FAILS. The optimum sweeps faster than the learn+copy loop
/// can follow, so the very same cultural strategy that tracks well under slow change
/// falls apart: its technique lags the optimum and its mean match collapses. The
/// adaptive benefit of culture evaporates once change outruns transmission.
#[ignore = "experiment harness — run with --ignored --nocapture"]
#[test]
fn dit_env_fast_culture_cannot_track() {
    const SEEDS: u64 = 8;
    let mut collapses = 0;
    for seed in 0..SEEDS {
        let m_fast = track_quality(seed, ENV_FAST, CRITICAL);
        let m_slow = track_quality(seed, ENV_SLOW, CRITICAL);
        if m_fast < 0.5 && m_fast < m_slow - 0.3 {
            collapses += 1;
        }
        eprintln!("FAST seed{seed}: critical match fast={m_fast:.2} | slow={m_slow:.2}");
    }
    eprintln!("FAST RESULT: cultural tracking collapsed under fast vs slow change in {collapses}/{SEEDS} seeds");
    assert!(
        collapses * 2 > SEEDS,
        "under fast change the same cultural strategy that tracks slowly should fail to track"
    );
}

/// STATIC env ⇒ culture is REDUNDANT. When the optimum never moves, the fixed
/// genetic strategy (pre-adapted) matches it perfectly at zero cost; the cultural
/// learner also matches it, but pays a perpetual learning cost for a technique
/// genes already encode. Both track well and neither has a tracking edge.
#[ignore = "experiment harness — run with --ignored --nocapture"]
#[test]
fn dit_env_static_culture_redundant() {
    const SEEDS: u64 = 8;
    let mut redundant = 0;
    for seed in 0..SEEDS {
        let m_learn = track_quality(seed, ENV_STATIC_PERIOD, CRITICAL);
        let m_gene = track_quality(seed, ENV_STATIC_PERIOD, INNATE);
        if m_gene > 0.5 && (m_gene - m_learn).abs() < 0.2 {
            redundant += 1;
        }
        eprintln!("STATIC seed{seed}: critical match={m_learn:.2} | innate match={m_gene:.2}");
    }
    eprintln!("STATIC RESULT: culture matched genes with no edge (redundant) in {redundant}/{SEEDS} seeds");
    assert!(
        redundant * 2 > SEEDS,
        "in a static world the fixed genetic strategy should match as well as culture (culture redundant)"
    );
}

// ----------------------------------------------------------------------------
// Axis 2: Rogers' pure-imitator paradox and its resolution
// ----------------------------------------------------------------------------

/// Rogers' paradox (core claim): imitation ALONE cannot bootstrap adaptation. A
/// pure-imitator population (copy only, never individually learn) has no source of
/// fresh information — everyone copies everyone — so once the optimum moves nobody
/// discovers the new value and the whole population goes stale. Its mean match is
/// far below an individual-learner population's. Culture without individual learning
/// is empty.
#[ignore = "experiment harness — run with --ignored --nocapture"]
#[test]
fn dit_rogers_imitation_alone_fails() {
    const SEEDS: u64 = 8;
    let mut learner_better = 0;
    for seed in 0..SEEDS {
        let m_im = track_quality(seed, ENV_SLOW, IMITATOR);
        let m_il = track_quality(seed, ENV_SLOW, INDIVIDUAL);
        if m_il > m_im + 0.2 {
            learner_better += 1;
        }
        eprintln!(
            "ROGERS seed{seed}: imitator-only match={m_im:.2} | individual-learner match={m_il:.2}"
        );
    }
    eprintln!("ROGERS RESULT: individual learning tracked, pure imitation did not, in {learner_better}/{SEEDS} seeds");
    assert!(
        learner_better * 2 > SEEDS,
        "pure imitation alone should fail to track (no fresh information to copy)"
    );
}

/// Resolution (Boyd-Richerson / Enquist): coupling imitation WITH individual
/// correction rescues it. The critical learner (copy + individually learn) tracks
/// as well as a pure individual learner AND vastly better than a pure imitator —
/// social learning adds value precisely when paired with a way to refresh stale info.
#[ignore = "experiment harness — run with --ignored --nocapture"]
#[test]
fn dit_rogers_critical_learning_resolves() {
    const SEEDS: u64 = 8;
    let mut resolved = 0;
    for seed in 0..SEEDS {
        let m_crit = track_quality(seed, ENV_SLOW, CRITICAL);
        let m_il = track_quality(seed, ENV_SLOW, INDIVIDUAL);
        let m_im = track_quality(seed, ENV_SLOW, IMITATOR);
        if m_crit > m_im + 0.2 && m_crit >= m_il - 0.1 {
            resolved += 1;
        }
        eprintln!(
            "RESOLVE seed{seed}: critical={m_crit:.2} | individual={m_il:.2} | imitator={m_im:.2}"
        );
    }
    eprintln!("RESOLVE RESULT: critical learning matched IL and beat pure imitation in {resolved}/{SEEDS} seeds");
    assert!(
        resolved * 2 > SEEDS,
        "critical learning (copy + individual correction) should track like an individual learner and beat pure imitation"
    );
}

// ----------------------------------------------------------------------------
// Axis 3: ecological pre-conditions on the C cumulative-skill mechanism (env_period = 0)
// ----------------------------------------------------------------------------

/// Run a Communicator C-skill lineage (species 1) vs an asocial control (species 2)
/// under the cumulative-skill mechanism (env_period = 0), placed either clustered
/// (social channel open) or dispersed (isolated). Returns the culture lineage's
/// share gain (end share − start share). Both lineages have identical kits except
/// the Communicator module, so the C skill is the only difference.
fn run_skill_pair(seed: u64, ticks: u32, clustered: bool) -> f64 {
    use anabios_core::biome::WORLD_SIZE;
    let mut w = World::new(seed);
    let n = 60usize;
    let skilled = Strat { comm: true, il: 0.0, sl: 0.0, innate: 0.0 };
    let control = Strat { comm: false, il: 0.0, sl: 0.0, innate: 0.0 };
    if clustered {
        spawn_lineage(&mut w, 1, n, skilled, 512.0, 512.0, 0.3);
        spawn_lineage(&mut w, 2, n, control, 512.0, 512.0, 0.3);
    } else {
        // Disperse both lineages across the whole world on interleaved grids so
        // Communicators rarely have a same-species neighbour to learn from.
        ensure_species(&mut w, 1);
        ensure_species(&mut w, 2);
        let cols = 16;
        let step_xy = WORLD_SIZE / cols as f32;
        for k in 0..n {
            let gx = (k % cols) as f32 * step_xy + 4.0;
            let gy = (k / cols) as f32 * step_xy + 4.0;
            let mut place = |sp: u32, s: Strat, off: f32| {
                let mut g = Genome::neutral();
                g.set(GenomeSlot::ReproductionThreshold, 0.3);
                w.spawn_seeded(
                    Vec2::new(gx + off, gy),
                    g,
                    sp,
                    kit_for(s.comm),
                    starter_asocial_forager(),
                );
            };
            place(1, skilled, 0.0);
            place(2, control, step_xy * 0.5);
        }
    }
    let start = count(&w, 1) as f64 / (count(&w, 1) + count(&w, 2)).max(1) as f64;
    for _ in 0..ticks {
        step(&mut w);
    }
    let end = count(&w, 1) as f64 / (count(&w, 1) + count(&w, 2)).max(1) as f64;
    end - start
}

/// Social structure gates cultural transmission: the C skill lineage gains far more
/// share when CLUSTERED (neighbours to copy from) than when DISPERSED (isolated,
/// only slow solo learning). Culture WORKS clustered, ~fails dispersed.
#[ignore = "experiment harness — run with --ignored --nocapture"]
#[test]
fn dit_social_clustered_vs_dispersed() {
    const SEEDS: u64 = 6;
    let mut clustered_bigger = 0;
    for seed in 0..SEEDS {
        let g_clustered = run_skill_pair(seed, 1500, true);
        let g_dispersed = run_skill_pair(seed, 1500, false);
        if g_clustered > g_dispersed {
            clustered_bigger += 1;
        }
        eprintln!(
            "SOCIAL seed{seed}: share-gain clustered={g_clustered:+.3} dispersed={g_dispersed:+.3}"
        );
    }
    eprintln!("SOCIAL RESULT: clustered advantage > dispersed in {clustered_bigger}/{SEEDS} seeds");
    assert!(
        clustered_bigger * 2 > SEEDS,
        "the social-learning advantage should be larger clustered than dispersed"
    );
}

/// Run the C-skill lineage vs asocial control at a given founding density (same
/// biome, so more founders ⇒ scarcer per-capita food). Returns the culture share gain.
fn run_density_pair(seed: u64, ticks: u32, per_side: usize) -> f64 {
    let mut w = World::new(seed);
    let skilled = Strat { comm: true, il: 0.0, sl: 0.0, innate: 0.0 };
    let control = Strat { comm: false, il: 0.0, sl: 0.0, innate: 0.0 };
    spawn_lineage(&mut w, 1, per_side, skilled, 512.0, 512.0, 0.3);
    spawn_lineage(&mut w, 2, per_side, control, 512.0, 512.0, 0.3);
    let s0 = count(&w, 1) as f64 / (count(&w, 1) + count(&w, 2)).max(1) as f64;
    for _ in 0..ticks {
        step(&mut w);
    }
    let s1 = count(&w, 1) as f64 / (count(&w, 1) + count(&w, 2)).max(1) as f64;
    s1 - s0
}

/// Resource level for the C skill: the cumulative foraging skill is a grazing
/// MULTIPLIER, so it converts into a decisive share advantage where there is
/// abundant food to exploit faster than rivals. Under chronic scarcity the biome is
/// grazed flat and there is little left to multiply. (This inverts the naive
/// "culture matters most under scarcity" guess — a measured property of a
/// multiplicative-payoff skill.)
#[ignore = "experiment harness — run with --ignored --nocapture"]
#[test]
fn dit_abundance_amplifies_skill() {
    const SEEDS: u64 = 6;
    let mut abundant_bigger = 0;
    for seed in 0..SEEDS {
        // Short runs so the share hasn't saturated (which would hide the rate gap).
        let g_scarce = run_density_pair(seed, 700, 120);
        let g_abundant = run_density_pair(seed, 700, 10);
        if g_abundant >= g_scarce {
            abundant_bigger += 1;
        }
        eprintln!("DENSITY seed{seed}: C-skill share-gain scarce={g_scarce:+.3} abundant={g_abundant:+.3}");
    }
    eprintln!("DENSITY RESULT: abundance advantage >= scarcity in {abundant_bigger}/{SEEDS} seeds");
    assert!(
        abundant_bigger * 2 > SEEDS,
        "the multiplicative C-skill edge should be at least as large under abundance as scarcity"
    );
}

// ----------------------------------------------------------------------------
// Axis 4: the Goldilocks rate, gene-culture coevolution, and adaptation timescale
// ----------------------------------------------------------------------------

/// The GOLDILOCKS result: culture's tracking advantage over a fixed genetic
/// strategy is NON-MONOTONIC in the rate of environmental change. It is ~zero when
/// the world is static (genes already match), largest at an intermediate rate
/// (only culture keeps up), and small again when change is too fast for culture to
/// track. The advantage peaks in the middle — the canonical inverted-U.
#[ignore = "experiment harness — run with --ignored --nocapture"]
#[test]
fn dit_intermediate_change_is_optimal() {
    const SEEDS: u64 = 6;
    let mut peaked = 0;
    for seed in 0..SEEDS {
        let adv = |p: u32| track_quality(seed, p, CRITICAL) - track_quality(seed, p, INNATE);
        let a_static = adv(ENV_STATIC_PERIOD);
        let a_mid = adv(ENV_SLOW);
        let a_fast = adv(ENV_FAST);
        if a_mid > a_static + 0.2 && a_mid > a_fast + 0.2 {
            peaked += 1;
        }
        eprintln!("GOLDILOCKS seed{seed}: culture advantage static={a_static:.2} mid={a_mid:.2} fast={a_fast:.2}");
    }
    eprintln!("GOLDILOCKS RESULT: culture's advantage peaked at the intermediate rate in {peaked}/{SEEDS} seeds");
    assert!(
        peaked * 2 > SEEDS,
        "culture's tracking advantage should peak at an intermediate change rate (inverted-U)"
    );
}

/// Fraction of the WHOLE alive population carrying the learning gene
/// (`IndividualLearning > 0.5`) — across all species, since selection may split
/// learners and non-learners into distinct species.
fn learner_fraction(w: &World) -> f64 {
    let mut n = 0usize;
    let mut learners = 0usize;
    for id in w.agents.iter_alive() {
        n += 1;
        if w.agents.genome[id as usize].get(GenomeSlot::IndividualLearning) > 0.5 {
            learners += 1;
        }
    }
    if n > 0 {
        learners as f64 / n as f64
    } else {
        0.0
    }
}

/// Outcome of one interbreeding gene-culture run.
struct Coevo {
    start_frac: f64,       // learner-gene frequency at founding
    early_frac: f64,       // learner-gene frequency early, before the population saturates
    end_frac: f64,         // learner-gene frequency at the end
    learner_match: f64,    // run-time-averaged technique-match of learners
    nonlearner_match: f64, // ... and of the fixed genetic foragers
}

/// Run one interbreeding population that starts half individual-learners (the
/// heritable `IndividualLearning` gene on) and half fixed genetic foragers under a
/// changing environment. The learning gene is inherited allele-like via crossover,
/// so its frequency tracks selection. Records how well each group tracks the
/// optimum and how the learner-gene frequency moves early vs late.
fn run_coevolution(seed: u64, ticks: u32) -> Coevo {
    let mut w = World::new(seed);
    w.env_period = ENV_SLOW;
    ensure_species(&mut w, 1);
    let seed_opt = env_optimum_at(0, ENV_SLOW);
    let n = 80usize;
    for k in 0..n {
        let ang = k as f32 * 0.7;
        let rad = 12.0 + (k % 11) as f32 * 7.0;
        let pos = Vec2::new(512.0 + rad * ang.cos(), 512.0 + rad * ang.sin());
        let learner = k % 2 == 0;
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.3);
        g.set(GenomeSlot::IndividualLearning, if learner { 1.0 } else { 0.0 });
        g.set(GenomeSlot::InnateTechnique, seed_opt); // non-learners pre-adapted to tick-0 opt
        let id = w.spawn_seeded(pos, g, 1, starter_kit(), starter_asocial_forager());
        if learner {
            w.agents.meme_vector[id as usize][TECH_CHANNEL] = seed_opt;
        }
    }
    let start_frac = learner_fraction(&w);
    let mut early_frac = start_frac;
    let (mut lsum, mut lcnt, mut nsum, mut ncnt) = (0.0f64, 0u64, 0.0f64, 0u64);
    for t in 0..ticks {
        step(&mut w);
        let opt = env_optimum_at(w.tick, w.env_period);
        for id in w.agents.iter_alive() {
            let i = id as usize;
            let m = technique_match(tech_of(&w, i), opt) as f64;
            if w.agents.genome[i].get(GenomeSlot::IndividualLearning) > 0.5 {
                lsum += m;
                lcnt += 1;
            } else {
                nsum += m;
                ncnt += 1;
            }
        }
        if t + 1 == 600 {
            early_frac = learner_fraction(&w);
        }
    }
    Coevo {
        start_frac,
        early_frac,
        end_frac: learner_fraction(&w),
        learner_match: if lcnt > 0 { lsum / lcnt as f64 } else { 0.0 },
        nonlearner_match: if ncnt > 0 { nsum / ncnt as f64 } else { 0.0 },
    }
}

/// Gene-culture COEVOLUTION — the honest dissociation. Starting from a 50/50 mix of
/// individual-learners and fixed genetic foragers under a changing environment, the
/// learners are behaviourally the clear winners: they track the moving optimum
/// nearly perfectly while the fixed strategy stays badly mismatched, and they
/// initially boom to a large majority. YET the learning GENE does not fix — it is
/// steadily purged as the population saturates. The env bonus is a grazing
/// MULTIPLIER, worthless at carrying capacity (depleted biome, nothing to multiply)
/// while the learning cost persists, so behaviourally-adaptive culture fails to
/// translate into durable gene selection. This is *why* the first-principles test
/// (experiment B) came back negative, and it matches the abundance/scarcity axis.
#[ignore = "experiment harness — run with --ignored --nocapture"]
#[test]
fn dit_coevolution_tracking_does_not_fix_the_gene() {
    const SEEDS: u64 = 6;
    let mut adaptive_but_purged = 0;
    for seed in 0..SEEDS {
        let c = run_coevolution(seed, 4000);
        // (1) culture is behaviourally adaptive: learners track the moving optimum
        //     far better than the fixed genetic strategy.
        let behaviourally_adaptive = c.learner_match > c.nonlearner_match + 0.3;
        // (2) yet the learning GENE is not selected up — it declines by the end.
        let gene_purged = c.end_frac < c.start_frac;
        if behaviourally_adaptive && gene_purged {
            adaptive_but_purged += 1;
        }
        eprintln!(
            "COEVO seed{seed}: learner-match={:.2} non-match={:.2} | learner-freq start={:.2} early={:.2} end={:.2}",
            c.learner_match, c.nonlearner_match, c.start_frac, c.early_frac, c.end_frac
        );
    }
    eprintln!("COEVO RESULT: culture was behaviourally adaptive yet its gene was purged in {adaptive_but_purged}/{SEEDS} seeds");
    assert!(
        adaptive_but_purged * 2 > SEEDS,
        "learners should track far better and win early, yet the learning gene should still be purged at carrying capacity"
    );
}

/// Two-timescale adaptation: culture responds FASTER than genes. A population
/// seeded MIS-matched to a static optimum recovers within a lifetime if it can
/// learn (technique converges by learning), but a fixed genetic population can
/// only recover by mutating + selecting its innate technique over generations.
/// Returns the tick at which mean match first passes 0.6 (or `ticks` if never).
fn recovery_tick(seed: u64, learner: bool, ticks: u32) -> u32 {
    let mut w = World::new(seed);
    w.env_period = ENV_STATIC_PERIOD; // fixed optimum = ENV_STATIC_OPTIMUM (0.75)
    ensure_species(&mut w, 1);
    let wrong = 0.2_f32; // seed far from the 0.75 optimum → everyone starts mismatched
    for k in 0..40usize {
        let ang = k as f32 * 0.7;
        let rad = 12.0 + (k % 11) as f32 * 7.0;
        let pos = Vec2::new(512.0 + rad * ang.cos(), 512.0 + rad * ang.sin());
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.3);
        // Give the genetic population a mutation rate so it CAN evolve (slowly);
        // the learner instead adapts within life via the IndividualLearning gene.
        if learner {
            g.set(GenomeSlot::IndividualLearning, 1.0);
        } else {
            g.set(GenomeSlot::MutationRate, 0.5);
        }
        g.set(GenomeSlot::InnateTechnique, wrong);
        let id = w.spawn_seeded(pos, g, 1, starter_kit(), starter_asocial_forager());
        if learner {
            w.agents.meme_vector[id as usize][TECH_CHANNEL] = wrong;
        }
    }
    for t in 0..ticks {
        step(&mut w);
        let (sm, n) = match_sum(&w, 1);
        if n > 0 && sm / n as f64 > 0.6 {
            return t + 1;
        }
    }
    ticks
}

/// Culture adapts on a faster timescale than genes: from the same mismatched
/// start, a learning population recovers a good technique-match far sooner than a
/// genetic-only population that must evolve its innate technique.
#[ignore = "experiment harness — run with --ignored --nocapture"]
#[test]
fn dit_culture_adapts_faster_than_genes() {
    const SEEDS: u64 = 6;
    const TICKS: u32 = 2500;
    let mut faster = 0;
    for seed in 0..SEEDS {
        let t_culture = recovery_tick(seed, true, TICKS);
        let t_genes = recovery_tick(seed, false, TICKS);
        if (t_culture as f64) < 0.5 * t_genes as f64 {
            faster += 1;
        }
        eprintln!("TIMESCALE seed{seed}: recovery ticks culture={t_culture} genes={t_genes}");
    }
    eprintln!(
        "TIMESCALE RESULT: culture recovered in <half the time of genes in {faster}/{SEEDS} seeds"
    );
    assert!(
        faster * 2 > SEEDS,
        "cultural adaptation should recover a good match far faster than genetic evolution"
    );
}
