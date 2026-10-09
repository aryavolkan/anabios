//! Cognition-threshold instrument: how realized IQ, measured against the
//! invention and practice gates, bears on evolutionary fitness.
//!
//! Read-only on the sim (it steps the world and reads columns; it never
//! writes agent state), so it adds no determinism risk. One run yields four
//! long-format CSVs, every row keyed by `seed` and `scale`:
//!
//! - `series.csv` — per window and founder tier: alive, cumulative births and
//!   deaths, mean realized IQ and heritable potential, mean energy and tech
//!   era, the share of the tier above each gate, and discoveries so far.
//! - `fitness.csv` — lifetime reproductive success by realized-IQ bin: every
//!   agent whose IQ had crystallized (age at death ≥ `IQ_MATURATION_AGE`)
//!   and that died inside the run contributes one lifetime, with its
//!   offspring count (`births_ok`), lifespan and highest era held.
//! - `tiers.csv` — the same lifetime table folded by founder tier, plus the
//!   tier's final count and birth and death totals.
//! - `gates.csv` — one row per run: the gate ladder in force and the whole
//!   population's final count, births, mean IQ, mean era and discoveries.
//!
//! A tier is a founding spec's lineage: at tick 0 every founder's species id
//! names its tier, and every later agent takes the tier of its first parent
//! (`parent_ids[0]`), so speciation splinters stay with their founders.
//!
//! `--gate-scales` multiplies the whole ladder (`World::iq_req_by_era` and
//! `World::practice_iq_req`) by each factor and runs the scenario once per
//! factor, so the fitness curve against gate height comes out of one call.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use anabios_core::agent::{LineageId, LINEAGE_NONE};
use anabios_core::codex::EventType;
use anabios_core::genome::GenomeSlot;
use anabios_core::invention::{held_mask, tech_era};
use anabios_core::iq::IQ_MATURATION_AGE;
use anabios_core::practice;
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anabios_core::world::World;
use anyhow::{Context, Result};
use rayon::prelude::*;

/// Realized-IQ bin width for `fitness.csv`.
pub const IQ_BIN: f32 = 0.05;
const IQ_BINS: usize = 20;

/// One completed lifetime, read from the slot the tick after the agent died.
#[derive(Clone, Copy, Debug)]
struct Lifetime {
    tier: u32,
    iq: f32,
    age: u32,
    offspring: u16,
    max_era: u8,
    held_practice: bool,
}

/// Running per-tier tallies that outlive any one window.
#[derive(Default, Clone, Debug)]
struct TierTally {
    founder_potential: f32,
    founders: u32,
    births: u64,
    deaths: u64,
    discoveries: u64,
}

/// One window row of `series.csv`.
#[derive(Clone, Debug)]
struct SeriesRow {
    tick: u64,
    tier: u32,
    founder_potential: f32,
    alive: u32,
    births_cum: u64,
    deaths_cum: u64,
    mean_iq: f64,
    mean_potential: f64,
    mean_energy: f64,
    mean_era: f64,
    /// Share of the tier's alive agents at or above the practice gate and
    /// each era gate, in gate order.
    gate_share: [f64; 5],
    discoveries_cum: u64,
}

/// The whole-run summary (`gates.csv`).
#[derive(Clone, Debug)]
struct RunSummary {
    seed: u64,
    scale: f32,
    era_req: [f32; 4],
    practice_req: f32,
    final_alive: u32,
    births_total: u64,
    deaths_total: u64,
    lifetimes: u64,
    mean_iq: f64,
    mean_potential: f64,
    mean_era: f64,
    share_era2_plus: f64,
    discoveries: u64,
}

struct RunOutput {
    summary: RunSummary,
    series: Vec<SeriesRow>,
    lifetimes: Vec<Lifetime>,
    tiers: BTreeMap<u32, TierTally>,
    final_alive_by_tier: BTreeMap<u32, u32>,
}

/// Per-slot shadow of the columns a death reads back, taken after every tick.
struct Shadow {
    alive: Vec<bool>,
    lineage: Vec<LineageId>,
    tier: Vec<u32>,
    iq: Vec<f32>,
    age: Vec<u32>,
    offspring: Vec<u16>,
    max_era: Vec<u8>,
    held_practice: Vec<bool>,
}

impl Shadow {
    fn with_capacity(cap: usize) -> Self {
        Shadow {
            alive: vec![false; cap],
            lineage: vec![LINEAGE_NONE; cap],
            tier: vec![u32::MAX; cap],
            iq: vec![0.0; cap],
            age: vec![0; cap],
            offspring: vec![0; cap],
            max_era: vec![0; cap],
            held_practice: vec![false; cap],
        }
    }

    fn grow_to(&mut self, cap: usize) {
        if self.alive.len() < cap {
            self.alive.resize(cap, false);
            self.lineage.resize(cap, LINEAGE_NONE);
            self.tier.resize(cap, u32::MAX);
            self.iq.resize(cap, 0.0);
            self.age.resize(cap, 0);
            self.offspring.resize(cap, 0);
            self.max_era.resize(cap, 0);
            self.held_practice.resize(cap, false);
        }
    }
}

fn holds_any_practice(meme: &[f32; anabios_core::program::MEME_CHANNELS]) -> bool {
    (0..practice::PRACTICE_COUNT).any(|p| practice::has(meme, p))
}

/// Realized-IQ bin index for `fitness.csv` (`1.0` lands in the last bin).
pub fn iq_bin(iq: f32) -> usize {
    ((iq / IQ_BIN).floor() as usize).min(IQ_BINS - 1)
}

/// Scale the world's gate ladder in place; a factor of 0 opens every gate.
fn scale_gates(world: &mut World, scale: f32) {
    for req in world.iq_req_by_era.iter_mut() {
        *req = (*req * scale).min(1.0);
    }
    world.practice_iq_req = (world.practice_iq_req * scale).min(1.0);
}

/// Run one seed at one gate scale and collect the instrument's readings.
fn simulate(scenario: &Scenario, seed: u64, scale: f32, ticks: u64, window: u64) -> RunOutput {
    let mut sc = scenario.clone();
    sc.seed = seed;
    let mut world = sc.instantiate();
    scale_gates(&mut world, scale);

    // Tier = founding species id; every founder carries one at tick 0.
    let mut tier_of_lineage: HashMap<LineageId, u32> = HashMap::new();
    let mut tiers: BTreeMap<u32, TierTally> = BTreeMap::new();
    {
        let mut pot_sum: BTreeMap<u32, f64> = BTreeMap::new();
        for id in world.agents.iter_alive() {
            let i = id as usize;
            let tier = world.agents.species_id[i];
            tier_of_lineage.insert(world.agents.lineage_id[i], tier);
            let t = tiers.entry(tier).or_default();
            t.founders += 1;
            *pot_sum.entry(tier).or_default() +=
                world.agents.genome[i].get(GenomeSlot::CognitivePotential) as f64;
        }
        for (tier, t) in tiers.iter_mut() {
            t.founder_potential = (pot_sum[tier] / t.founders.max(1) as f64) as f32;
        }
    }

    let mut shadow = Shadow::with_capacity(world.agents.capacity());
    let mut lifetimes: Vec<Lifetime> = Vec::new();
    let mut series: Vec<SeriesRow> = Vec::new();
    let mut seen_events: HashSet<(u64, u8, u32, u32)> = HashSet::new();

    // Seed the shadow from tick 0 so founders are not counted as births.
    refresh_shadow(&world, &mut shadow, &mut tier_of_lineage, &mut tiers, &mut lifetimes, true);
    series.extend(window_rows(&world, &shadow, &tiers));

    for _ in 0..ticks {
        step(&mut world);
        refresh_shadow(
            &world,
            &mut shadow,
            &mut tier_of_lineage,
            &mut tiers,
            &mut lifetimes,
            false,
        );
        count_discoveries(&world, &shadow, &mut tiers, &mut seen_events);
        if window > 0 && world.tick.is_multiple_of(window) {
            series.extend(window_rows(&world, &shadow, &tiers));
        }
    }
    if window == 0 || !world.tick.is_multiple_of(window) {
        series.extend(window_rows(&world, &shadow, &tiers));
    }

    // Whole-run summary.
    let mut alive = 0u32;
    let (mut iq_sum, mut pot_sum, mut era_sum, mut era2) = (0.0f64, 0.0f64, 0.0f64, 0u32);
    let mut final_alive_by_tier: BTreeMap<u32, u32> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        alive += 1;
        iq_sum += world.agents.iq[i] as f64;
        pot_sum += world.agents.genome[i].get(GenomeSlot::CognitivePotential) as f64;
        let era = tech_era(held_mask(&world.agents.meme_vector[i]));
        era_sum += era as f64;
        era2 += u32::from(era >= 2);
        *final_alive_by_tier.entry(shadow.tier[i]).or_default() += 1;
    }
    let n = alive.max(1) as f64;
    let summary = RunSummary {
        seed,
        scale,
        era_req: world.iq_req_by_era,
        practice_req: world.practice_iq_req,
        final_alive: alive,
        births_total: tiers.values().map(|t| t.births).sum(),
        deaths_total: tiers.values().map(|t| t.deaths).sum(),
        lifetimes: lifetimes.len() as u64,
        mean_iq: iq_sum / n,
        mean_potential: pot_sum / n,
        mean_era: era_sum / n,
        share_era2_plus: era2 as f64 / n,
        discoveries: tiers.values().map(|t| t.discoveries).sum(),
    };
    RunOutput { summary, series, lifetimes, tiers, final_alive_by_tier }
}

/// Compare the live columns against the shadow: a slot that was alive and is
/// now dead or re-occupied (its lineage id moved on) closes a lifetime; a
/// slot newly alive is a birth, labelled with its first parent's tier. Then
/// copy the live columns into the shadow for the next tick.
fn refresh_shadow(
    world: &World,
    shadow: &mut Shadow,
    tier_of_lineage: &mut HashMap<LineageId, u32>,
    tiers: &mut BTreeMap<u32, TierTally>,
    lifetimes: &mut Vec<Lifetime>,
    founding: bool,
) {
    let cap = world.agents.capacity();
    shadow.grow_to(cap);
    let a = &world.agents;
    for i in 0..cap {
        let alive_now = a.alive[i];
        let same_occupant = alive_now && shadow.alive[i] && a.lineage_id[i] == shadow.lineage[i];
        if shadow.alive[i] && !same_occupant {
            // The previous occupant died (its slot is empty or reused).
            let tier = shadow.tier[i];
            if let Some(t) = tiers.get_mut(&tier) {
                t.deaths += 1;
            }
            if shadow.age[i] >= IQ_MATURATION_AGE {
                lifetimes.push(Lifetime {
                    tier,
                    iq: shadow.iq[i],
                    age: shadow.age[i],
                    offspring: shadow.offspring[i],
                    max_era: shadow.max_era[i],
                    held_practice: shadow.held_practice[i],
                });
            }
        }
        if alive_now && !same_occupant {
            // A new occupant: a founder at tick 0, else a birth.
            let lineage = a.lineage_id[i];
            let tier = match tier_of_lineage.get(&lineage) {
                Some(&t) => t,
                None => {
                    let [p0, p1] = a.parent_ids[i];
                    let t = tier_of_lineage
                        .get(&p0)
                        .or_else(|| tier_of_lineage.get(&p1))
                        .copied()
                        .unwrap_or(u32::MAX);
                    tier_of_lineage.insert(lineage, t);
                    t
                }
            };
            shadow.tier[i] = tier;
            if !founding {
                tiers.entry(tier).or_default().births += 1;
            }
        }
        shadow.alive[i] = alive_now;
        if alive_now {
            shadow.lineage[i] = a.lineage_id[i];
            shadow.iq[i] = a.iq[i];
            shadow.age[i] = a.age[i];
            shadow.offspring[i] = a.births_ok[i];
            shadow.max_era[i] = tech_era(held_mask(&a.meme_vector[i]));
            shadow.held_practice[i] = holds_any_practice(&a.meme_vector[i]);
        }
    }
}

/// Credit this tick's `InventionDiscovered` events to the discoverer's tier
/// (the first alive agent of the event's species names it). Events are
/// read in place, never drained, so the codex ring buffer is untouched.
fn count_discoveries(
    world: &World,
    shadow: &Shadow,
    tiers: &mut BTreeMap<u32, TierTally>,
    seen: &mut HashSet<(u64, u8, u32, u32)>,
) {
    for ev in world.codex.events.iter().rev() {
        if ev.tick + 1 < world.tick {
            break; // older than this step; the buffer is in push order
        }
        if ev.event_type != EventType::InventionDiscovered {
            continue;
        }
        let key = (ev.tick, ev.event_type as u8, ev.species_id, ev.value.to_bits());
        if !seen.insert(key) {
            continue;
        }
        let tier = world
            .agents
            .iter_alive()
            .find(|&id| world.agents.species_id[id as usize] == ev.species_id)
            .map(|id| shadow.tier[id as usize])
            .unwrap_or(u32::MAX);
        tiers.entry(tier).or_default().discoveries += 1;
    }
}

/// One `series.csv` row per tier for the current tick.
fn window_rows(world: &World, shadow: &Shadow, tiers: &BTreeMap<u32, TierTally>) -> Vec<SeriesRow> {
    #[derive(Default)]
    struct Acc {
        n: u32,
        iq: f64,
        pot: f64,
        energy: f64,
        era: f64,
        gates: [u32; 5],
    }
    let gates = [
        world.practice_iq_req,
        world.iq_req_by_era[0],
        world.iq_req_by_era[1],
        world.iq_req_by_era[2],
        world.iq_req_by_era[3],
    ];
    let mut acc: BTreeMap<u32, Acc> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let a = acc.entry(shadow.tier[i]).or_default();
        a.n += 1;
        let iq = world.agents.iq[i];
        a.iq += iq as f64;
        a.pot += world.agents.genome[i].get(GenomeSlot::CognitivePotential) as f64;
        a.energy += world.agents.energy[i] as f64;
        a.era += tech_era(held_mask(&world.agents.meme_vector[i])) as f64;
        for (g, &req) in gates.iter().enumerate() {
            a.gates[g] += u32::from(iq >= req);
        }
    }
    tiers
        .iter()
        .map(|(&tier, t)| {
            let a = acc.get(&tier);
            let n = a.map_or(0, |a| a.n);
            let d = n.max(1) as f64;
            let mean = |f: fn(&Acc) -> f64| a.map_or(0.0, |a| f(a) / d);
            let mut gate_share = [0.0f64; 5];
            if let Some(a) = a {
                for (share, &count) in gate_share.iter_mut().zip(a.gates.iter()) {
                    *share = count as f64 / d;
                }
            }
            SeriesRow {
                tick: world.tick,
                tier,
                founder_potential: t.founder_potential,
                alive: n,
                births_cum: t.births,
                deaths_cum: t.deaths,
                mean_iq: mean(|a| a.iq),
                mean_potential: mean(|a| a.pot),
                mean_energy: mean(|a| a.energy),
                mean_era: mean(|a| a.era),
                gate_share,
                discoveries_cum: t.discoveries,
            }
        })
        .collect()
}

/// Fold lifetimes into realized-IQ bins: `(bin, n, lifespan_sum,
/// offspring_sum, era_sum, practice_count)`.
fn fitness_bins(lifetimes: &[Lifetime]) -> Vec<(usize, u64, f64, f64, f64, u64)> {
    let mut bins = vec![(0usize, 0u64, 0.0f64, 0.0f64, 0.0f64, 0u64); IQ_BINS];
    for (b, row) in bins.iter_mut().enumerate() {
        row.0 = b;
    }
    for l in lifetimes {
        let row = &mut bins[iq_bin(l.iq)];
        row.1 += 1;
        row.2 += l.age as f64;
        row.3 += l.offspring as f64;
        row.4 += l.max_era as f64;
        row.5 += u64::from(l.held_practice);
    }
    bins
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    scenario_path: PathBuf,
    ticks: u64,
    seeds: u64,
    seed: Option<u64>,
    window: u64,
    out_dir: PathBuf,
    gate_scales: Vec<f32>,
    threads: Option<usize>,
) -> Result<()> {
    if let Some(n) = threads {
        rayon::ThreadPoolBuilder::new().num_threads(n).build_global().ok();
    }
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating output dir {}", out_dir.display()))?;
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario {}", scenario_path.display()))?;
    let scenario = Scenario::parse_toml(&text)?;
    let base_seed = seed.unwrap_or(scenario.seed);
    let scales = if gate_scales.is_empty() { vec![1.0f32] } else { gate_scales };
    for &s in &scales {
        anyhow::ensure!(s.is_finite() && s >= 0.0, "gate scale must be finite and >= 0: {s}");
    }

    let jobs: Vec<(u64, f32)> = scales
        .iter()
        .flat_map(|&scale| (0..seeds.max(1)).map(move |k| (base_seed + k, scale)))
        .collect();
    eprintln!(
        "[cognition] {} run(s): seeds {}..{} × gate scales {:?}, {ticks} ticks, window {window}",
        jobs.len(),
        base_seed,
        base_seed + seeds.max(1) - 1,
        scales
    );
    let mut outputs: Vec<RunOutput> = jobs
        .par_iter()
        .map(|&(seed, scale)| {
            let out = simulate(&scenario, seed, scale, ticks, window);
            eprintln!(
                "[cognition] seed={seed} scale={scale:.2} alive={} births={} lifetimes={} \
                 mean_iq={:.3} mean_era={:.2} discoveries={}",
                out.summary.final_alive,
                out.summary.births_total,
                out.summary.lifetimes,
                out.summary.mean_iq,
                out.summary.mean_era,
                out.summary.discoveries
            );
            out
        })
        .collect();
    outputs.sort_by(|a, b| {
        a.summary.scale.total_cmp(&b.summary.scale).then(a.summary.seed.cmp(&b.summary.seed))
    });

    write_series(&out_dir.join("series.csv"), &outputs)?;
    write_fitness(&out_dir.join("fitness.csv"), &outputs)?;
    write_tiers(&out_dir.join("tiers.csv"), &outputs)?;
    write_gates(&out_dir.join("gates.csv"), &outputs)?;
    println!(
        "wrote {}/{{series,fitness,tiers,gates}}.csv ({} runs)",
        out_dir.display(),
        outputs.len()
    );
    Ok(())
}

fn write_series(path: &PathBuf, outputs: &[RunOutput]) -> Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    writeln!(
        w,
        "seed,scale,tick,tier,founder_potential,alive,births_cum,deaths_cum,mean_iq,\
         mean_potential,mean_energy,mean_era,share_practice_gate,share_era1,share_era2,\
         share_era3,share_era4,discoveries_cum"
    )?;
    for o in outputs {
        for r in &o.series {
            writeln!(
                w,
                "{},{:.3},{},{},{:.3},{},{},{},{:.4},{:.4},{:.2},{:.3},{:.4},{:.4},{:.4},{:.4},{:.4},{}",
                o.summary.seed,
                o.summary.scale,
                r.tick,
                r.tier,
                r.founder_potential,
                r.alive,
                r.births_cum,
                r.deaths_cum,
                r.mean_iq,
                r.mean_potential,
                r.mean_energy,
                r.mean_era,
                r.gate_share[0],
                r.gate_share[1],
                r.gate_share[2],
                r.gate_share[3],
                r.gate_share[4],
                r.discoveries_cum
            )?;
        }
    }
    Ok(())
}

fn write_fitness(path: &PathBuf, outputs: &[RunOutput]) -> Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    writeln!(
        w,
        "seed,scale,iq_lo,iq_hi,lifetimes,mean_lifespan,mean_offspring,\
         offspring_per_1k_ticks,mean_max_era,share_practice"
    )?;
    for o in outputs {
        for (b, n, age, off, era, prac) in fitness_bins(&o.lifetimes) {
            let d = n.max(1) as f64;
            let per_1k = if age > 0.0 { off / age * 1000.0 } else { 0.0 };
            writeln!(
                w,
                "{},{:.3},{:.2},{:.2},{},{:.1},{:.4},{:.4},{:.3},{:.4}",
                o.summary.seed,
                o.summary.scale,
                b as f32 * IQ_BIN,
                (b + 1) as f32 * IQ_BIN,
                n,
                age / d,
                off / d,
                per_1k,
                era / d,
                prac as f64 / d
            )?;
        }
    }
    Ok(())
}

fn write_tiers(path: &PathBuf, outputs: &[RunOutput]) -> Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    writeln!(
        w,
        "seed,scale,tier,founder_potential,founders,final_alive,births_cum,deaths_cum,\
         discoveries,lifetimes,mean_iq_at_death,mean_lifespan,mean_offspring,\
         offspring_per_1k_ticks,mean_max_era,share_practice"
    )?;
    for o in outputs {
        for (&tier, t) in &o.tiers {
            let (mut n, mut iq, mut age, mut off, mut era, mut prac) =
                (0u64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0u64);
            for l in o.lifetimes.iter().filter(|l| l.tier == tier) {
                n += 1;
                iq += l.iq as f64;
                age += l.age as f64;
                off += l.offspring as f64;
                era += l.max_era as f64;
                prac += u64::from(l.held_practice);
            }
            let d = n.max(1) as f64;
            let per_1k = if age > 0.0 { off / age * 1000.0 } else { 0.0 };
            writeln!(
                w,
                "{},{:.3},{},{:.3},{},{},{},{},{},{},{:.4},{:.1},{:.4},{:.4},{:.3},{:.4}",
                o.summary.seed,
                o.summary.scale,
                tier,
                t.founder_potential,
                t.founders,
                o.final_alive_by_tier.get(&tier).copied().unwrap_or(0),
                t.births,
                t.deaths,
                t.discoveries,
                n,
                iq / d,
                age / d,
                off / d,
                per_1k,
                era / d,
                prac as f64 / d
            )?;
        }
    }
    Ok(())
}

fn write_gates(path: &PathBuf, outputs: &[RunOutput]) -> Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    writeln!(
        w,
        "seed,scale,era_req_1,era_req_2,era_req_3,era_req_4,practice_req,final_alive,\
         births_total,deaths_total,lifetimes,mean_iq,mean_potential,mean_era,\
         share_era2_plus,discoveries"
    )?;
    for o in outputs {
        let s = &o.summary;
        writeln!(
            w,
            "{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{},{:.4},{:.4},{:.3},{:.4},{}",
            s.seed,
            s.scale,
            s.era_req[0],
            s.era_req[1],
            s.era_req[2],
            s.era_req[3],
            s.practice_req,
            s.final_alive,
            s.births_total,
            s.deaths_total,
            s.lifetimes,
            s.mean_iq,
            s.mean_potential,
            s.mean_era,
            s.share_era2_plus,
            s.discoveries
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iq_bins_cover_the_unit_interval_with_one_at_the_top() {
        assert_eq!(iq_bin(0.0), 0);
        assert_eq!(iq_bin(0.049), 0);
        assert_eq!(iq_bin(0.05), 1);
        assert_eq!(iq_bin(0.999), 19);
        assert_eq!(iq_bin(1.0), 19, "1.0 lands in the last bin, not a 21st");
    }

    #[test]
    fn gate_scale_moves_the_whole_ladder_and_caps_at_one() {
        let mut w = World::new(1);
        scale_gates(&mut w, 2.0);
        assert_eq!(w.iq_req_by_era, [0.3, 0.7, 1.0, 1.0]);
        assert!((w.practice_iq_req - 0.2).abs() < 1e-6);
        let mut open = World::new(1);
        scale_gates(&mut open, 0.0);
        assert_eq!(open.iq_req_by_era, [0.0; 4]);
        assert_eq!(open.practice_iq_req, 0.0);
    }

    /// Two tiers of two founders each: the shadow labels founders by species
    /// and does not count them as births; a reused slot is a death of the
    /// previous occupant.
    #[test]
    fn shadow_labels_founders_and_closes_a_lifetime_on_reuse() {
        const TWO_TIERS: &str = "\
name = \"t\"
seed = 1
[[agents]]
count = 2
archetype = \"innovator\"
placement = { kind = \"uniform\" }
[agents.traits]
cognitive_potential = 0.1
[[agents]]
count = 2
archetype = \"innovator\"
placement = { kind = \"uniform\" }
[agents.traits]
cognitive_potential = 0.9
";
        let mut world = Scenario::parse_toml(TWO_TIERS).unwrap().instantiate();
        let mut shadow = Shadow::with_capacity(world.agents.capacity());
        let mut tier_of = HashMap::new();
        let mut tiers: BTreeMap<u32, TierTally> = BTreeMap::new();
        for id in world.agents.iter_alive() {
            let i = id as usize;
            tier_of.insert(world.agents.lineage_id[i], world.agents.species_id[i]);
            tiers.entry(world.agents.species_id[i]).or_default().founders += 1;
        }
        let mut lifetimes = Vec::new();
        refresh_shadow(&world, &mut shadow, &mut tier_of, &mut tiers, &mut lifetimes, true);
        assert_eq!(tiers.len(), 2, "two founding species ⇒ two tiers");
        assert!(tiers.values().all(|t| t.founders == 2 && t.births == 0 && t.deaths == 0));

        // Age one founder past maturation, then kill it: its lifetime closes.
        let victim = world.agents.iter_alive().next().unwrap();
        world.agents.age[victim as usize] = IQ_MATURATION_AGE + 5;
        world.agents.iq[victim as usize] = 0.42;
        refresh_shadow(&world, &mut shadow, &mut tier_of, &mut tiers, &mut lifetimes, false);
        let victim_tier = shadow.tier[victim as usize];
        world.agents.kill(victim);
        refresh_shadow(&world, &mut shadow, &mut tier_of, &mut tiers, &mut lifetimes, false);
        assert_eq!(lifetimes.len(), 1);
        assert_eq!(lifetimes[0].tier, victim_tier);
        assert!((lifetimes[0].iq - 0.42).abs() < 1e-6);
        assert_eq!(tiers[&victim_tier].deaths, 1);
        assert_eq!(tiers.values().map(|t| t.births).sum::<u64>(), 0);
    }
}
