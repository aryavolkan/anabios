//! Stage 1 of the cultural-inventions experiment: does an INVENTIVE culture
//! lineage (Inventiveness gene + Communicator → the invention ratchet + a
//! stacking tech-tree: domestication, writing, industry) robustly out-reproduce
//! a non-cultural CONTROL lineage in the ABUNDANT living biome — the condition
//! the plain foraging-skill benefit failed (see tests/living_sandbox.rs)?
//!
//! Unlike the living-sandbox harness, the two cohorts differ in the GENOME (the
//! Inventiveness gene), so speciation keeps them apart — no cohort-tally leak.
//!
//! Run: cargo test -p anabios-core --release --test cultural_inventions -- --ignored --nocapture
//! Env: CI_SEEDS / CI_TICKS override defaults; CI_HEADSTART=1 seeds culture at
//! full invention level (tests the ESTABLISHED benefit, not the ramp).
//!
//! FINDING (2026-07-18): the invention mechanism WORKS — the ratchet climbs to
//! full and the tech-tree fires (a clamp fix was needed; the level was
//! over-accumulating past 1.0, now bounded). But the result splits by metric:
//!
//! `inventive_culture_differential` (head-to-head, mixed, hard cap): culture
//! wins only ~1/3 of seeds — winner-take-all competitive exclusion. Even with
//! CI_HEADSTART (full benefit from tick 0) it is still ~1/3. This is a
//! methodological finding: final-cohort-size in a mixed population under a hard
//! population cap is a monopoly LOTTERY (whichever cohort wins the stochastic
//! founder-effect cap-race takes ~everything), which swamps any per-capita
//! benefit however strong or robust. The head-to-head metric cannot show the
//! differential — same obstruction seen in tests/living_sandbox.rs.
//!
//! `inventive_carrying_capacity` (each cohort run SEPARATELY — no competition):
//! populations in this harsh 2048 living+seasonal biome are bistable — they
//! boom to the cap (~8000) or bust to EXTINCTION. The inventive population
//! survives/establishes MORE reliably than control (e.g. 3/6 vs 2/6 seeds; in
//! the divergent seeds culture survives where control goes extinct 2:1, mean
//! log-ratio ~+1.5). So the robust invention benefit (steady domestication
//! food + industry upkeep discount + lower breed threshold) buys EXTINCTION
//! RESISTANCE / establishment robustness — a real fitness effect the saturating
//! foraging-skill benefit lacked — but it manifests as resilience, not
//! head-to-head dominance. Demonstrating the differential needs a
//! non-competitive metric; the winner-take-all head-to-head hides it.
//!
//! Next: more seeds to firm up the survival delta; a softer biome (fewer
//! boom/bust extinctions) or an early-phase growth-rate metric; and Stage 2
//! (gene sweep, tests/inventions_sweep.rs).
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anabios_core::world::World;

const SCENARIO: &str = include_str!("../../../scenarios/cultural-inventions.toml");
const CULTURE_FOUNDER: u32 = 1;
const CONTROL_FOUNDER: u32 = 2;

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Live members of `founder` and every species descended from it.
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

/// Mean invention level (meme channel 7) among the culture cohort — shows the
/// ratchet actually climbing.
fn mean_culture_invention(w: &World) -> f32 {
    use anabios_core::culture::INVENTION_CHANNEL;
    let (mut sum, mut n) = (0.0f32, 0u32);
    for id in w.agents.iter_alive() {
        let i = id as usize;
        if w.agents.species_id[i] == CULTURE_FOUNDER {
            sum += w.agents.meme_vector[i][INVENTION_CHANNEL];
            n += 1;
        }
    }
    if n > 0 {
        sum / n as f32
    } else {
        0.0
    }
}

fn run(seed: u64, ticks: u64) -> (u32, u32, f32) {
    use anabios_core::culture::INVENTION_CHANNEL;
    let mut sc = Scenario::parse_toml(SCENARIO).unwrap();
    sc.seed = seed;
    let mut w = sc.instantiate();
    // Optional head-start: the invention level ramps from 0, so cold-start gives
    // culture NO advantage during the early cap-race that decides the monopoly.
    // CI_HEADSTART=1 seeds the culture cohort at full invention level, testing
    // whether the ESTABLISHED benefit wins (Stage-1 "validate the benefit").
    if std::env::var("CI_HEADSTART").is_ok() {
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        for id in ids {
            let i = id as usize;
            if w.agents.species_id[i] == CULTURE_FOUNDER {
                w.agents.meme_vector[i][INVENTION_CHANNEL] = 1.0;
            }
        }
    }
    for _ in 0..ticks {
        step(&mut w);
    }
    (
        cohort_count(&w, CULTURE_FOUNDER),
        cohort_count(&w, CONTROL_FOUNDER),
        mean_culture_invention(&w),
    )
}

/// Run a SINGLE cohort alone (no direct competition → no winner-take-all), to
/// measure the invention benefit's effect on carrying capacity directly.
fn run_solo(seed: u64, ticks: u64, keep_archetype: &str) -> (u32, f32) {
    use anabios_core::culture::INVENTION_CHANNEL;
    let mut sc = Scenario::parse_toml(SCENARIO).unwrap();
    sc.seed = seed;
    sc.agents.retain(|a| a.archetype.as_deref() == Some(keep_archetype));
    let mut w = sc.instantiate();
    for _ in 0..ticks {
        step(&mut w);
    }
    let (mut sum, mut n) = (0.0f32, 0u32);
    for id in w.agents.iter_alive() {
        sum += w.agents.meme_vector[id as usize][INVENTION_CHANNEL];
        n += 1;
    }
    (w.agents.live_count(), if n > 0 { sum / n as f32 } else { 0.0 })
}

/// The clean benefit measurement: does an inventive population sustain a larger
/// equilibrium than a control population in the same biome, run separately (so
/// there is no winner-take-all monopoly lottery)?
#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn inventive_carrying_capacity() {
    let seeds = env_u64("CI_SEEDS", 6);
    let ticks = env_u64("CI_TICKS", 4000);
    eprintln!("cultural-inventions carrying capacity (solo runs): {seeds} seeds x {ticks} ticks");
    let mut culture_bigger = 0u32;
    let mut sum_lr = 0.0f64;
    for seed in 0..seeds {
        let (cu, inv) = run_solo(seed, ticks, "inventive_forager");
        let (co, _) = run_solo(seed, ticks, "asocial_forager");
        let ratio = (cu.max(1) as f64) / (co.max(1) as f64);
        if cu > co {
            culture_bigger += 1;
        }
        sum_lr += ratio.ln();
        eprintln!("seed{seed}: culture_solo={cu} control_solo={co} ratio={ratio:.2} mean_invention={inv:.2}");
    }
    eprintln!(
        "RESULT: inventive population is larger in {culture_bigger}/{seeds} seeds, mean log-ratio {:.3}",
        sum_lr / seeds as f64
    );
}

#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn inventive_culture_differential() {
    let seeds = env_u64("CI_SEEDS", 10);
    let ticks = env_u64("CI_TICKS", 6000);
    eprintln!(
        "cultural-inventions Stage 1: {seeds} seeds x {ticks} ticks (living biome, inventions ON)"
    );

    let mut culture_wins = 0u32;
    let mut sum_lr = 0.0f64;
    for seed in 0..seeds {
        let (cu, co, inv) = run(seed, ticks);
        let ratio = (cu.max(1) as f64) / (co.max(1) as f64);
        if cu > co {
            culture_wins += 1;
        }
        sum_lr += ratio.ln();
        eprintln!("seed{seed}: culture={cu} control={co} ratio={ratio:.2} mean_invention={inv:.2}");
    }
    let mean_lr = sum_lr / seeds as f64;
    eprintln!("RESULT: culture wins {culture_wins}/{seeds}, mean log-ratio {mean_lr:.3}");
    let bar = (seeds as f64 * 0.7).ceil() as u32;
    let met = culture_wins >= bar && mean_lr > 0.0;
    eprintln!(
        "VERDICT: Stage-1 bar (culture wins >={bar}/{seeds}, mean-lr>0) {}",
        if met { "MET — robust cumulative benefit fixes the differential" } else { "NOT met" }
    );
}
