//! Experiment: does a Communicator/skill CULTURE lineage out-reproduce a
//! non-cultural CONTROL lineage in a large LIVING sandbox — and is the
//! advantage stronger with the living biome ON than OFF?
//!
//! Run: cargo test -p anabios-core --release --test living_sandbox -- --ignored --nocapture
//! Env knobs: LSB_SEEDS / LSB_TICKS / LSB_MAXPOP override the defaults for sweeps.
//!
//! FINDING (2026-07-18, hypothesis-INVERTING — the spec expected the living
//! biome to rescue the culture advantage; it does the opposite):
//! - The living biome dramatically raises carrying capacity (living pops fill
//!   the cap; the STATIC biome collapses to near-zero at 2048) — the renewal +
//!   seasonality mechanisms work.
//! - Under UNIFORM placement the differential is winner-take-all competitive
//!   exclusion: whichever cohort's skill positive-feedback (skill -> more graze
//!   -> more offspring -> more skilled foragers) fires first monopolizes the
//!   biome. Bistable on founder-effect noise (~coin flip across seeds).
//! - Separating the cohorts spatially removes the monopoly but reveals the
//!   mechanism: in the ABUNDANT living biome the CONTROL wins, while in the
//!   SCARCE static biome the CULTURE wins. The skill multiplier saturates when
//!   food is plentiful (everyone eats their fill), so it stops paying — yet the
//!   Communicator module's upkeep cost persists, making culture a net loser
//!   exactly where the biome is richest. Under scarcity the multiplier is
//!   decisive, so culture wins.
//! - Conclusion: the culture foraging-skill benefit is SCARCITY-dependent; a
//!   renewing/abundant biome does NOT produce a robust culture lineage win.
//!   This mirrors the prior DIT-boundary result that the cultural benefit is
//!   density-dependent and evaporates where selection is strongest. Next levers
//!   (future work): a non-saturating cultural benefit, lower Communicator
//!   upkeep, a scarcity-tuned biome, or the deferred genetic-assimilation
//!   (Baldwin) channel. This harness REPORTS the effect; it does not assert a
//!   pass, because forcing one by parameter-hunting would be p-hacking.
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anabios_core::world::World;

const SCENARIO: &str = include_str!("../../../scenarios/living-sandbox-coevolution.toml");
const CULTURE_FOUNDER: u32 = 1;
const CONTROL_FOUNDER: u32 = 2;

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Live members of `founder` and every species descended from it (species may
/// split off child species over a long run; tally the whole ancestry subtree).
fn cohort_count(w: &World, founder: u32) -> u32 {
    let mut total = 0u32;
    for sid in 0..w.species_member_counts.len() as u32 {
        let mut cur = Some(sid);
        let mut chained = false;
        let mut guard = 0;
        while let Some(c) = cur {
            if c == founder {
                chained = true;
                break;
            }
            cur = w.species_parents.get(c as usize).copied().flatten();
            guard += 1;
            if guard > 4096 {
                break;
            }
        }
        if chained {
            total += w.species_member_counts[sid as usize];
        }
    }
    total
}

fn run(seed: u64, ticks: u64, living: bool) -> (u32, u32) {
    let mut sc = Scenario::parse_toml(SCENARIO).unwrap();
    sc.seed = seed;
    sc.living_biome = living;
    if !living {
        sc.season_period = 0;
    }
    // Tuning override: raise the population cap so the BIOME (not the cap)
    // limits growth — at a hard cap the two cohorts hit winner-take-all
    // competitive exclusion driven by founder-effect noise, masking the
    // skill-efficiency advantage.
    if let Ok(v) = std::env::var("LSB_MAXPOP") {
        if let Ok(cap) = v.parse::<u32>() {
            sc.max_population = Some(cap);
        }
    }
    let mut w = sc.instantiate();
    for _ in 0..ticks {
        step(&mut w);
    }
    (cohort_count(&w, CULTURE_FOUNDER), cohort_count(&w, CONTROL_FOUNDER))
}

#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn culture_lineage_differential() {
    let seeds = env_u64("LSB_SEEDS", 10);
    let ticks = env_u64("LSB_TICKS", 6000);
    eprintln!("living-sandbox differential: {seeds} seeds x {ticks} ticks (living ON vs OFF)");

    let mut culture_wins_living = 0u32;
    let mut culture_wins_off = 0u32;
    let mut sum_lr_living = 0.0f64;
    let mut sum_lr_off = 0.0f64;
    for seed in 0..seeds {
        let (cu, co) = run(seed, ticks, true);
        let (cu0, co0) = run(seed, ticks, false);
        let ratio = (cu.max(1) as f64) / (co.max(1) as f64);
        let ratio0 = (cu0.max(1) as f64) / (co0.max(1) as f64);
        if cu > co {
            culture_wins_living += 1;
        }
        if cu0 > co0 {
            culture_wins_off += 1;
        }
        sum_lr_living += ratio.ln();
        sum_lr_off += ratio0.ln();
        eprintln!(
            "seed{seed}: LIVING culture={cu} control={co} ratio={ratio:.2} | \
             OFF culture={cu0} control={co0} ratio={ratio0:.2}"
        );
    }
    let mean_lr_living = sum_lr_living / seeds as f64;
    let mean_lr_off = sum_lr_off / seeds as f64;
    eprintln!(
        "RESULT: LIVING culture wins {culture_wins_living}/{seeds}, mean log-ratio {mean_lr_living:.3} | \
         OFF culture wins {culture_wins_off}/{seeds}, mean log-ratio {mean_lr_off:.3}"
    );
    eprintln!(
        "CONTRAST (living advantage over static): win-delta {}, mean-log-ratio delta {:.3}",
        culture_wins_living as i32 - culture_wins_off as i32,
        mean_lr_living - mean_lr_off
    );

    // Spec §1 success bar (for reference): culture out-reproduces control in
    // >= 70% of seeds with a positive mean log-ratio in the LIVING biome, and
    // stronger living-ON than OFF. This is a REPORTING harness — it prints the
    // verdict rather than asserting, because the finding is that this bar is NOT
    // met (see the module-level FINDING). The harness always completes so it can
    // be run repeatedly for sweeps without a red test masking the numbers.
    let bar = (seeds as f64 * 0.7).ceil() as u32;
    let met = culture_wins_living >= bar && mean_lr_living > 0.0 && mean_lr_living > mean_lr_off;
    eprintln!(
        "VERDICT: spec bar (culture wins >={bar}/{seeds} living, mean-lr>0, living>off) {}",
        if met {
            "MET"
        } else {
            "NOT met (see module FINDING: culture benefit is scarcity-dependent)"
        }
    );
}
