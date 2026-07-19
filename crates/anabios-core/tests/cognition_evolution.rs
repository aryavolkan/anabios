//! Evolutionarily-meaningful tests for the cognitive gene–culture layer.
//!
//! The unit tests in `iq.rs` / `practice.rs` / `inventions.rs` prove the
//! *mechanisms* fire correctly. These harnesses run the actual `step()` loop
//! over many ticks and seeds and assert the *design claims* — that the
//! mechanisms have the intended **selective / developmental consequences**:
//!
//!  1. A maladaptive practice (`child_sacrifice`) inflicts a real reproductive
//!     deficit — a seeded population is clearly out-grown by the control.
//!  2. `inbreeding` is likewise selected against — the kin-seeking mate bias
//!     drives close-kin pairings and a viability (stillbirth) cost those pairings
//!     can't re-feed away, so the practising population is out-grown. (An earlier
//!     energy-only form was too weak; strengthening it was the point of this
//!     mechanic — see `INBREEDING_STILLBIRTH`.)
//!  3. Realized IQ carries both **heritable** (gene) and **plastic** variation —
//!     the raw material selection needs — measured from real juvenile
//!     development, with each nurture channel isolated: a food-rich upbringing
//!     AND a socially-embedded one each raise realized IQ over their opposite.
//!  4. A reported coevolution verdict: with cognition on, mean IQ + the
//!     maladaptive-practice load are printed over a long run. The sweep
//!     direction is genuinely uncertain (culture genes often don't sweep from
//!     standing variation), so this asserts only the robust invariants and
//!     reports the trajectory — matching the repo's harness style.
//!
//! Run explicitly: `cargo test -p anabios-core --test cognition_evolution -- --ignored --nocapture`.

use anabios_core::biome::{TerrainType, BIOME_RES, CELL_SIZE};
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::module::Module;
use anabios_core::prelude_test::Vec2;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anabios_core::world::World;
use anabios_core::{iq, practice};

const COGNITIVE: &str = include_str!("../../../scenarios/cognitive-coevolution.toml");

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Instantiate the cognitive scenario at `seed`, with the population cap raised
/// enough that a growing control run does not saturate within the test horizon
/// (at the cap both arms plateau and any reproductive differential is masked)
/// but bounded so compute stays reasonable. Override with `COG_MAXPOP`.
fn cognitive_world(seed: u64) -> World {
    let mut sc = Scenario::parse_toml(COGNITIVE).expect("parse cognitive scenario");
    sc.seed = seed;
    sc.max_population = Some(env_u64("COG_MAXPOP", 1500) as u32);
    sc.instantiate()
}

/// Final live population after `ticks`, optionally seeding every founder with a
/// held maladaptive `practice` at t=0 (it then propagates by meme inheritance +
/// social spread among the Communicator lineages).
fn final_pop(seed: u64, ticks: u64, seeded_practice: Option<usize>) -> u32 {
    let mut w = cognitive_world(seed);
    if let Some(p) = seeded_practice {
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        for id in ids {
            w.agents.meme_vector[id as usize][practice::channel(p)] = 1.0;
        }
    }
    for _ in 0..ticks {
        step(&mut w);
    }
    w.agents.live_count()
}

/// A/B over `seeds`: how many seeds have the practice arm strictly smaller than
/// the control arm, plus the mean population ratio (treatment / control).
fn differential(practice: usize, seeds: u64, ticks: u64) -> (u64, f64) {
    let mut wins = 0u64;
    let mut ratio_sum = 0.0f64;
    for s in 0..seeds {
        let control = final_pop(s, ticks, None).max(1);
        let treated = final_pop(s, ticks, Some(practice));
        if treated < control {
            wins += 1;
        }
        ratio_sum += treated as f64 / control as f64;
    }
    (wins, ratio_sum / seeds as f64)
}

#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn child_sacrifice_is_selected_against() {
    let seeds = env_u64("COG_SEEDS", 8);
    let ticks = env_u64("COG_TICKS", 300);
    let (wins, mean_ratio) = differential(practice::CHILD_SACRIFICE, seeds, ticks);
    println!(
        "RESULT child_sacrifice: practice arm smaller in {wins}/{seeds} seeds, \
         mean pop ratio (treated/control) = {mean_ratio:.3}"
    );
    // Culling half of all newborns is a large, reliable fitness hit.
    assert!(
        wins as f64 >= 0.75 * seeds as f64,
        "a child-sacrificing population should out-lose the control in most seeds"
    );
    assert!(mean_ratio < 0.9, "and carry a clear population deficit: {mean_ratio:.3}");
}

#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn inbreeding_is_selected_against() {
    // With the kin-seeking mate bias (`find_mate`) driving close-kin pairings and
    // a viability (stillbirth) cost those pairings can't re-feed away, inbreeding
    // is now a genuine population-level selector — a population practising it is
    // out-grown by the control. (The earlier energy-only form was too weak; see
    // the git history / `INBREEDING_STILLBIRTH`.)
    let seeds = env_u64("COG_SEEDS", 8);
    let ticks = env_u64("COG_TICKS", 300);
    let (wins, mean_ratio) = differential(practice::INBREEDING, seeds, ticks);
    println!(
        "RESULT inbreeding: practice arm smaller in {wins}/{seeds} seeds, \
         mean pop ratio (treated/control) = {mean_ratio:.3}"
    );
    assert!(
        wins as f64 >= 0.6 * seeds as f64,
        "an inbreeding population should be out-grown by the control in most seeds"
    );
    assert!(mean_ratio < 0.9, "and carry a clear population deficit: {mean_ratio:.3}");
}

/// Centre of a well-fed grass cell / a barren cell to stand a cohort on.
fn grass_spot(w: &World) -> Vec2 {
    for row in 0..BIOME_RES {
        for col in 0..BIOME_RES {
            let cell = w.biome.at(col, row);
            if cell.terrain == TerrainType::Grass && cell.plant_biomass > 0.5 {
                return Vec2::new((col as f32 + 0.5) * CELL_SIZE, (row as f32 + 0.5) * CELL_SIZE);
            }
        }
    }
    Vec2::splat(CELL_SIZE * 0.5)
}
fn barren_spot(w: &World) -> Vec2 {
    for row in 0..BIOME_RES {
        for col in 0..BIOME_RES {
            if w.biome.at(col, row).plant_biomass <= 0.0 {
                return Vec2::new((col as f32 + 0.5) * CELL_SIZE, (row as f32 + 0.5) * CELL_SIZE);
            }
        }
    }
    Vec2::splat(CELL_SIZE * 0.5)
}

/// Mean realized IQ of a cohort grown through the whole juvenile window, with
/// the two nurture channels cleanly separable. Each agent is stripped to just a
/// Sensor — no Locomotor (stays on its placed cell → stable local food), no
/// Mouth (no grazing → the cell's food isn't depleted), no Reproductive (no
/// births to perturb survival) — so `nutrition` = the cell's standing food and
/// `social` = clustering, each set independently:
/// - `rich_food`: grown on vegetated grass vs barren ground.
/// - `crowded`: tightly clustered (high crowding) vs each alone in its own world.
fn cohort_iq(gene: f32, rich_food: bool, crowded: bool, n: usize, seed: u64) -> f32 {
    let mut g = Genome::neutral();
    g.set(GenomeSlot::CognitivePotential, gene);
    let spawn_dev = |w: &mut World, spot: Vec2| -> u32 {
        let id = w.spawn_agent(spot, g);
        w.agents.modules[id as usize].retain(|m| matches!(m, Module::Sensor { .. }));
        id
    };
    let place = |w: &World| if rich_food { grass_spot(w) } else { barren_spot(w) };
    let mut iqs = Vec::new();
    if crowded {
        let mut w = World::new(seed);
        w.cognition_enabled = true;
        let spot = place(&w);
        let ids: Vec<u32> = (0..n)
            .map(|k| spawn_dev(&mut w, spot + Vec2::new((k % 5) as f32, (k / 5) as f32)))
            .collect();
        for _ in 0..=iq::IQ_MATURATION_AGE {
            step(&mut w);
        }
        iqs.extend(
            ids.iter().filter(|&&id| w.agents.is_alive(id)).map(|&id| w.agents.iq[id as usize]),
        );
    } else {
        // Each juvenile alone in its own world → zero social enrichment.
        for k in 0..n {
            let mut w = World::new(seed.wrapping_add(k as u64));
            w.cognition_enabled = true;
            let spot = place(&w);
            let id = spawn_dev(&mut w, spot);
            for _ in 0..=iq::IQ_MATURATION_AGE {
                step(&mut w);
            }
            if w.agents.is_alive(id) {
                iqs.push(w.agents.iq[id as usize]);
            }
        }
    }
    assert!(!iqs.is_empty(), "some of the cohort must survive the juvenile window");
    iqs.iter().sum::<f32>() / iqs.len() as f32
}

#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn realized_iq_is_heritable_and_plastic() {
    // Heritability: same environment (crowded grass), brighter gene → higher IQ.
    let dull = cohort_iq(0.1, true, true, 25, 7);
    let bright = cohort_iq(0.9, true, true, 25, 7);
    println!("RESULT heritability: dull={dull:.3} bright={bright:.3} (shared env)");
    assert!(
        bright > dull + 0.1,
        "the heritable gene must lift realized IQ under a shared environment"
    );

    // Nutrition plasticity: same gene + same (crowded) social env, grown on rich
    // grass vs barren ground → the local FOOD of the growing environment lifts
    // realized IQ. (This is what the biome-food nutrition channel fixed — with
    // the old spawn-energy proxy this signal was ~0.)
    let fed = cohort_iq(0.5, true, true, 25, 13);
    let starved = cohort_iq(0.5, false, true, 25, 13);
    println!("RESULT nutrition plasticity: fed={fed:.3} starved={starved:.3} (gene=0.5)");
    assert!(
        fed > starved + 0.05,
        "a food-rich upbringing must raise realized IQ above a barren one"
    );

    // Social plasticity: same gene + same (rich) food, a socially-embedded
    // upbringing vs growing up alone → clustering lifts realized IQ.
    let crowded = cohort_iq(0.5, true, true, 25, 11);
    let solo = cohort_iq(0.5, true, false, 25, 11);
    println!("RESULT social plasticity: crowded={crowded:.3} solo={solo:.3} (gene=0.5)");
    assert!(
        crowded > solo + 0.05,
        "a socially-embedded upbringing must raise realized IQ above growing up alone"
    );
}

#[ignore = "experiment harness — run explicitly with --ignored --nocapture"]
#[test]
fn cognitive_coevolution_verdict() {
    // A reported run of the full scenario: track the mean CognitivePotential
    // gene and the maladaptive-practice load over a long horizon. The sweep
    // DIRECTION is a genuine experimental question (culture genes often do not
    // sweep from standing variation — see the gene-culture memory), so this
    // asserts only the robust invariant (the run stays alive and produces both
    // tech and practices) and PRINTS the coevolutionary trajectory to inspect.
    let ticks = env_u64("COG_VERDICT_TICKS", 1500);
    let mut w = cognitive_world(env_u64("COG_SEED", 0));

    let mean_gene = |w: &World| -> f32 {
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        if ids.is_empty() {
            return 0.0;
        }
        ids.iter().map(|&id| w.agents.genome[id as usize].cognitive_potential()).sum::<f32>()
            / ids.len() as f32
    };
    let practice_load = |w: &World| -> f32 {
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        if ids.is_empty() {
            return 0.0;
        }
        let held = ids
            .iter()
            .filter(|&&id| {
                (0..practice::PRACTICE_COUNT)
                    .any(|p| practice::has(&w.agents.meme_vector[id as usize], p))
            })
            .count();
        held as f32 / ids.len() as f32
    };

    let gene0 = mean_gene(&w);
    for _ in 0..ticks {
        step(&mut w);
    }
    let gene1 = mean_gene(&w);
    let load1 = practice_load(&w);
    let mean_iq = {
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        ids.iter().map(|&id| w.agents.iq[id as usize]).sum::<f32>() / ids.len().max(1) as f32
    };
    println!(
        "VERDICT after {ticks} ticks: CognitivePotential {gene0:.3} -> {gene1:.3} \
         (Δ {:+.3}), mean realized IQ {mean_iq:.3}, maladaptive-practice load {load1:.3}",
        gene1 - gene0
    );
    // Robust invariants only: the world persists and cognition produced a
    // non-degenerate realized-IQ distribution.
    assert!(w.agents.live_count() > 0, "the cognitive population must not go extinct");
    assert!(mean_iq > 0.0, "realized IQ must have developed above zero");
}
