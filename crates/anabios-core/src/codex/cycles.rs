//! Codex population-dynamics detectors (E3): cycles, boom-and-bust,
//! carrying-capacity plateaus, and the ordered trophic cascade.
//!
//! Cycle analysis runs on `cycle_history` (400-tick window), deliberately
//! separate from the crash detector's 200-tick `pop_history` so existing
//! detector semantics are untouched. Cycle/plateau checks are amortized
//! (every `CYCLE_CHECK_INTERVAL` ticks); the cascade state machine is
//! world-scalar and runs per tick.

use super::*;

/// Cycle-analysis result: mean full period (ticks) and peak/trough ratio.
struct CycleStats {
    period: f32,
    ratio: f32,
}

/// Zero-crossing analysis of a detrended population window. Returns cycle
/// stats when the series oscillates regularly: ≥4 sign changes with all
/// intervals in `[CYCLE_PERIOD_MIN, CYCLE_PERIOD_MAX]`, low interval jitter
/// (CV < 0.5), and peak deviation ≥ `CYCLE_MIN_AMPLITUDE` of the mean.
fn analyze_cycle(buf: &VecDeque<u32>) -> Option<CycleStats> {
    if buf.len() < CYCLE_WINDOW {
        return None;
    }
    let n = buf.len() as f64;
    let mean = buf.iter().map(|&c| c as f64).sum::<f64>() / n;
    if mean < 1.0 {
        return None;
    }

    let mut crossings: Vec<u64> = Vec::new();
    let mut prev_sign: i8 = 0;
    let mut peak_dev: f64 = 0.0;
    for (i, &c) in buf.iter().enumerate() {
        let d = c as f64 - mean;
        peak_dev = peak_dev.max(d.abs());
        let s: i8 = if d > 0.0 {
            1
        } else if d < 0.0 {
            -1
        } else {
            0
        };
        if s == 0 {
            continue;
        }
        if prev_sign != 0 && s != prev_sign {
            crossings.push(i as u64);
        }
        prev_sign = s;
    }
    if crossings.len() < 4 {
        return None;
    }
    let intervals: Vec<u64> = crossings.windows(2).map(|w| w[1] - w[0]).collect();
    if intervals.iter().any(|&iv| !(CYCLE_PERIOD_MIN..=CYCLE_PERIOD_MAX).contains(&iv)) {
        return None;
    }
    let im = intervals.iter().map(|&iv| iv as f64).sum::<f64>() / intervals.len() as f64;
    let ivar = intervals
        .iter()
        .map(|&iv| {
            let d = iv as f64 - im;
            d * d
        })
        .sum::<f64>()
        / intervals.len() as f64;
    if im <= 0.0 || ivar.sqrt() / im >= 0.5 {
        return None;
    }
    if peak_dev < (CYCLE_MIN_AMPLITUDE as f64) * mean {
        return None;
    }

    let peak = *buf.iter().max().unwrap_or(&0);
    let trough = *buf.iter().min().unwrap_or(&0);
    let ratio = peak as f32 / trough.max(1) as f32;
    // Crossings arrive twice per full oscillation.
    Some(CycleStats { period: (2.0 * im) as f32, ratio })
}

/// Plateau check: coefficient of variation over a full window. Returns
/// `(mean, cv)`; callers apply the latch thresholds.
fn plateau_cv(buf: &VecDeque<u32>) -> Option<(f64, f64)> {
    if buf.len() < CYCLE_WINDOW {
        return None;
    }
    let n = buf.len() as f64;
    let mean = buf.iter().map(|&c| c as f64).sum::<f64>() / n;
    if mean < CARRYING_MIN_POP as f64 {
        return None;
    }
    let var = buf
        .iter()
        .map(|&c| {
            let d = c as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n;
    Some((mean, var.sqrt() / mean))
}

pub(super) fn update_cycle_history(world: &mut World, agg: &SpeciesAggTable) {
    for sid in 0..world.species_member_counts.len() {
        let count = world.species_member_counts[sid];
        let buf = world.codex.cycle_history.entry(sid as u32).or_default();
        if buf.len() == CYCLE_WINDOW {
            buf.pop_front();
        }
        buf.push_back(count);
    }
    // Guild/world series: the trophic guild is the ecologically meaningful
    // oscillator — per-species lines churn under 200-tick reclustering.
    let (mut herb, mut carn, mut total) = (0_u32, 0_u32, 0_u32);
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        let mean_carnivory = e.diet_sum / e.count.max(1) as f64;
        if mean_carnivory >= 0.5 {
            carn += e.count;
        } else {
            herb += e.count;
        }
        total += e.count;
    }
    for (buf, v) in [
        (&mut world.codex.herb_cycle_history, herb),
        (&mut world.codex.carn_cycle_history, carn),
        (&mut world.codex.total_cycle_history, total),
    ] {
        if buf.len() == CYCLE_WINDOW {
            buf.pop_front();
        }
        buf.push_back(v);
    }
}

/// Guild series identity for event attribution: the largest member species
/// at fire time (loc = its centroid), so the event is inspectable even
/// though the oscillator is the guild.
fn guild_representative(agg: &SpeciesAggTable, guild: u8) -> (u32, (f32, f32)) {
    let mut best: Option<(u32, u32)> = None; // (sid, count)
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        let mean_carnivory = e.diet_sum / e.count.max(1) as f64;
        let in_guild = match guild {
            0 => mean_carnivory < 0.5,
            1 => mean_carnivory >= 0.5,
            _ => true,
        };
        if in_guild && best.is_none_or(|(_, c)| e.count > c) {
            best = Some((sid, e.count));
        }
    }
    match best {
        Some((sid, _)) => (sid, centroid_of(agg, sid)),
        None => (0, (0.0, 0.0)),
    }
}

fn detect_guild_cycles(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    let series: [(u8, VecDeque<u32>); 3] = [
        (0, world.codex.herb_cycle_history.clone()),
        (1, world.codex.carn_cycle_history.clone()),
        (2, world.codex.total_cycle_history.clone()),
    ];
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (guild, buf) in series {
        let stats = analyze_cycle(&buf);
        let (period, ratio, cycling) = match stats {
            Some(s) => (s.period, s.ratio, true),
            None => (0.0, 0.0, false),
        };
        if let Some(ev) =
            edge_trigger_species_u8(&mut world.codex.guild_cycle_active, guild, cycling, || {
                let (sid, (lx, ly)) = guild_representative(agg, guild);
                CodexEvent {
                    event_type: EventType::PopulationCycleDetected,
                    tick,
                    species_id: sid,
                    value: period,
                    loc_x: lx,
                    loc_y: ly,
                }
            })
        {
            to_push.push(ev);
        }
        let boom = cycling && ratio >= BOOM_AMPLITUDE;
        if let Some(ev) =
            edge_trigger_species_u8(&mut world.codex.guild_boom_active, guild, boom, || {
                let (sid, (lx, ly)) = guild_representative(agg, guild);
                CodexEvent {
                    event_type: EventType::BoomAndBust,
                    tick,
                    species_id: sid,
                    value: ratio,
                    loc_x: lx,
                    loc_y: ly,
                }
            })
        {
            to_push.push(ev);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// `edge_trigger_species` for u8-keyed guild latches.
fn edge_trigger_species_u8(
    active: &mut BTreeSet<u8>,
    key: u8,
    fired: bool,
    make: impl FnOnce() -> CodexEvent,
) -> Option<CodexEvent> {
    if fired {
        if active.insert(key) {
            return Some(make());
        }
    } else {
        active.remove(&key);
    }
    None
}

pub(super) fn detect_cycles(world: &mut World, agg: &SpeciesAggTable) {
    if !world.tick.is_multiple_of(CYCLE_CHECK_INTERVAL) {
        return;
    }
    detect_guild_cycles(world, agg);
    let tick = world.tick;
    // Decide first (immutable borrow of histories), apply after.
    let mut decisions: Vec<(u32, Option<CycleStats>)> =
        world.codex.cycle_history.iter().map(|(sid, buf)| (*sid, analyze_cycle(buf))).collect();

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, stats) in decisions.drain(..) {
        let (period, ratio, cycling) = match stats {
            Some(s) => (s.period, s.ratio, true),
            None => (0.0, 0.0, false),
        };
        if let Some(ev) = edge_trigger_species(&mut world.codex.cycle_active, sid, cycling, || {
            let (lx, ly) = centroid_of(agg, sid);
            CodexEvent {
                event_type: EventType::PopulationCycleDetected,
                tick,
                species_id: sid,
                value: period,
                loc_x: lx,
                loc_y: ly,
            }
        }) {
            to_push.push(ev);
        }
        let boom = cycling && ratio >= BOOM_AMPLITUDE;
        if let Some(ev) = edge_trigger_species(&mut world.codex.boom_active, sid, boom, || {
            let (lx, ly) = centroid_of(agg, sid);
            CodexEvent {
                event_type: EventType::BoomAndBust,
                tick,
                species_id: sid,
                value: ratio,
                loc_x: lx,
                loc_y: ly,
            }
        }) {
            to_push.push(ev);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

pub(super) fn detect_carrying_capacity(world: &mut World, agg: &SpeciesAggTable) {
    if !world.tick.is_multiple_of(CYCLE_CHECK_INTERVAL) {
        return;
    }
    detect_guild_carrying(world, agg);
    let tick = world.tick;
    let mut decisions: Vec<(u32, Option<(f64, f64)>)> =
        world.codex.cycle_history.iter().map(|(sid, buf)| (*sid, plateau_cv(buf))).collect();

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, plateau) in decisions.drain(..) {
        // Hysteresis: fire below CARRYING_MAX_CV, re-arm only above 2× that.
        let (fire, rearm, mean) = match plateau {
            Some((m, cv)) => (cv < CARRYING_MAX_CV as f64, cv > 2.0 * CARRYING_MAX_CV as f64, m),
            None => (false, true, 0.0),
        };
        let active = world.codex.carrying_active.contains(&sid);
        if fire && !active {
            world.codex.carrying_active.insert(sid);
            let (lx, ly) = centroid_of(agg, sid);
            to_push.push(CodexEvent {
                event_type: EventType::CarryingCapacityReached,
                tick,
                species_id: sid,
                value: mean as f32,
                loc_x: lx,
                loc_y: ly,
            });
        } else if rearm && active {
            world.codex.carrying_active.remove(&sid);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Guild/world plateaus: the ecological carrying-capacity signal (e.g. the
/// total population pinned at the world cap with collapsed variance).
fn detect_guild_carrying(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    let series: [(u8, VecDeque<u32>); 3] = [
        (0, world.codex.herb_cycle_history.clone()),
        (1, world.codex.carn_cycle_history.clone()),
        (2, world.codex.total_cycle_history.clone()),
    ];
    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (guild, buf) in series {
        let (fire, rearm, mean) = match plateau_cv(&buf) {
            Some((m, cv)) => (cv < CARRYING_MAX_CV as f64, cv > 2.0 * CARRYING_MAX_CV as f64, m),
            None => (false, true, 0.0),
        };
        let active = world.codex.guild_carrying_active.contains(&guild);
        if fire && !active {
            world.codex.guild_carrying_active.insert(guild);
            let (sid, (lx, ly)) = guild_representative(agg, guild);
            to_push.push(CodexEvent {
                event_type: EventType::CarryingCapacityReached,
                tick,
                species_id: sid,
                value: mean as f32,
                loc_x: lx,
                loc_y: ly,
            });
        } else if rearm && active {
            world.codex.guild_carrying_active.remove(&guild);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Ordered cascade: carnivore crash → herbivore boom → plant crash. The
/// staged machine with lag timeouts is what separates a true cascade from
/// three independent fluctuations.
pub(super) fn detect_trophic_cascade(world: &mut World, agg: &SpeciesAggTable) {
    let mut carn: u32 = 0;
    let mut herb: u32 = 0;
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        let mean_carnivory = e.diet_sum / e.count.max(1) as f64;
        if mean_carnivory >= 0.5 {
            carn += e.count;
        } else {
            herb += e.count;
        }
    }
    let plant = world.plant_biomass_total();
    let tick = world.tick;

    {
        let hist = &mut world.codex.cascade_carn_history;
        if hist.len() == CASCADE_WINDOW {
            hist.pop_front();
        }
        hist.push_back(carn);
    }

    let stage = world.codex.cascade_stage;
    match stage {
        0 => {
            let peak = *world.codex.cascade_carn_history.iter().max().unwrap_or(&0);
            let crashed = peak >= CASCADE_MIN_PREDATORS
                && (carn as f32) <= (1.0 - CASCADE_CRASH_FRAC) * peak as f32;
            if crashed {
                world.codex.cascade_stage = 1;
                world.codex.cascade_stage_tick = tick;
                world.codex.cascade_carn_peak = peak;
                world.codex.cascade_herb_entry = herb;
                world.codex.cascade_plant_entry = plant;
            }
        }
        1 => {
            if tick.saturating_sub(world.codex.cascade_stage_tick) > CASCADE_LAG {
                world.codex.cascade_stage = 0;
            } else if (herb as f32)
                >= (1.0 + CASCADE_HERB_RISE) * world.codex.cascade_herb_entry as f32
            {
                world.codex.cascade_stage = 2;
                world.codex.cascade_stage_tick = tick;
                // The plant reference resets at boom confirmation: the claim
                // is "plants crash once the released herbivores graze them
                // down", measured from the release point.
                world.codex.cascade_plant_entry = plant;
            }
        }
        _ => {
            if tick.saturating_sub(world.codex.cascade_stage_tick) > CASCADE_PLANT_LAG {
                world.codex.cascade_stage = 0;
            } else if (plant) <= (1.0 - CASCADE_PLANT_DROP) * world.codex.cascade_plant_entry {
                let peak = world.codex.cascade_carn_peak;
                let drop = 1.0 - (carn as f32 / peak.max(1) as f32);
                world.codex.cascade_stage = 0;
                world.codex.push_event(CodexEvent {
                    event_type: EventType::TrophicCascade,
                    tick,
                    species_id: 0,
                    value: drop,
                    loc_x: 0.0,
                    loc_y: 0.0,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use std::collections::VecDeque;

    fn world_with_agents(n: u32) -> World {
        let mut w = World::new(1);
        for _ in 0..n {
            let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        }
        w
    }

    fn sine_window(mean: f32, amp: f32, period: f32) -> VecDeque<u32> {
        (0..CYCLE_WINDOW)
            .map(|t| {
                (mean + amp * (t as f32 * std::f32::consts::TAU / period).sin()).round().max(0.0)
                    as u32
            })
            .collect()
    }

    fn ramp_window(from: u32, to: u32) -> VecDeque<u32> {
        (0..CYCLE_WINDOW)
            .map(|t| from + (to - from) * t as u32 / (CYCLE_WINDOW as u32 - 1))
            .collect()
    }

    fn flat_window(v: u32) -> VecDeque<u32> {
        std::iter::repeat_n(v, CYCLE_WINDOW).collect()
    }

    fn agg_with(sid: u32, count: u32, diet_sum: f64) -> SpeciesAggTable {
        let mut agg = SpeciesAggTable::default();
        let e = SpeciesAgg {
            count,
            sum_x: 500.0 * count as f64,
            sum_y: 500.0 * count as f64,
            diet_sum,
            ..Default::default()
        };
        if sid as usize >= agg.entries.len() {
            agg.entries.resize(sid as usize + 1, SpeciesAgg::default());
        }
        agg.entries[sid as usize] = e;
        agg.active.push(sid);
        agg
    }

    #[test]
    fn sine_population_fires_cycle_exactly_once() {
        let mut w = world_with_agents(4);
        w.codex.cycle_history.insert(0, sine_window(100.0, 60.0, 120.0));
        let agg = agg_with(0, 100, 0.0);
        detect_cycles(&mut w, &agg);
        let fired: Vec<_> = w
            .codex
            .events
            .iter()
            .filter(|e| e.event_type == EventType::PopulationCycleDetected)
            .collect();
        assert_eq!(fired.len(), 1, "sine window fires the cycle detector once");
        // Period ~120 ticks; allow analysis slack.
        assert!((fired[0].value - 120.0).abs() < 20.0, "period ≈ 120, got {}", fired[0].value);
        // Latched: a second observation does not re-fire.
        detect_cycles(&mut w, &agg);
        assert_eq!(
            w.codex
                .events
                .iter()
                .filter(|e| e.event_type == EventType::PopulationCycleDetected)
                .count(),
            1
        );
    }

    #[test]
    fn ramp_population_does_not_fire_cycle() {
        let mut w = world_with_agents(4);
        w.codex.cycle_history.insert(0, ramp_window(20, 400));
        let agg = agg_with(0, 400, 0.0);
        detect_cycles(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "monotonic ramp must not cycle: {:?}", w.codex.events);
    }

    #[test]
    fn flat_population_does_not_fire_cycle() {
        let mut w = world_with_agents(4);
        w.codex.cycle_history.insert(0, flat_window(100));
        let agg = agg_with(0, 100, 0.0);
        detect_cycles(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "flat line has zero amplitude");
    }

    #[test]
    fn deep_sine_fires_boom_and_bust() {
        let mut w = world_with_agents(4);
        // Peak 160, trough 40 → ratio 4.0 ≥ BOOM_AMPLITUDE.
        w.codex.cycle_history.insert(0, sine_window(100.0, 60.0, 120.0));
        let agg = agg_with(0, 100, 0.0);
        detect_cycles(&mut w, &agg);
        let booms: Vec<_> =
            w.codex.events.iter().filter(|e| e.event_type == EventType::BoomAndBust).collect();
        assert_eq!(booms.len(), 1, "deep sine fires BoomAndBust");
        assert!(booms[0].value >= BOOM_AMPLITUDE, "ratio reported: {}", booms[0].value);
    }

    #[test]
    fn shallow_sine_cycles_without_boom() {
        let mut w = world_with_agents(4);
        // Peak 130, trough 70 → ratio ~1.86 < BOOM_AMPLITUDE.
        w.codex.cycle_history.insert(0, sine_window(100.0, 30.0, 120.0));
        let agg = agg_with(0, 100, 0.0);
        detect_cycles(&mut w, &agg);
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::PopulationCycleDetected));
        assert!(!w.codex.events.iter().any(|e| e.event_type == EventType::BoomAndBust));
    }

    #[test]
    fn plateau_fires_carrying_capacity() {
        let mut w = world_with_agents(4);
        w.codex.cycle_history.insert(0, flat_window(150));
        let agg = agg_with(0, 150, 0.0);
        detect_carrying_capacity(&mut w, &agg);
        let fired: Vec<_> = w
            .codex
            .events
            .iter()
            .filter(|e| e.event_type == EventType::CarryingCapacityReached)
            .collect();
        assert_eq!(fired.len(), 1);
        assert!((fired[0].value - 150.0).abs() < 1.0);
    }

    #[test]
    fn noisy_plateau_does_not_fire_carrying_capacity() {
        let mut w = world_with_agents(4);
        // Alternating 100/150 → CV ~0.2, above the plateau threshold.
        let buf: VecDeque<u32> =
            (0..CYCLE_WINDOW).map(|t| if t % 2 == 0 { 100 } else { 150 }).collect();
        w.codex.cycle_history.insert(0, buf);
        let agg = agg_with(0, 125, 0.0);
        detect_carrying_capacity(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "noisy plateau must not fire");
    }

    /// Drive the cascade machine through a scripted sequence of
    /// (carnivore, herbivore, plant) observations.
    fn drive_cascade(w: &mut World, script: &[(u32, u32, f32)]) {
        for (i, &(carn, herb, plant)) in script.iter().enumerate() {
            w.tick = i as u64;
            // One carnivore species (diet 1.0) and one herbivore (diet 0.0);
            // counts scaled by the script.
            let mut agg = SpeciesAggTable::default();
            let c = SpeciesAgg {
                count: carn.max(1),
                diet_sum: carn.max(1) as f64, // mean carnivory 1.0
                ..Default::default()
            };
            let h = SpeciesAgg { count: herb.max(1), diet_sum: 0.0, ..Default::default() };
            agg.entries = vec![c, h];
            agg.active = vec![0, 1];
            // Zero carn count means extinction of the predator species: model
            // by dropping the carnivore entry instead of a zero-count row.
            if carn == 0 {
                agg.entries[0] = SpeciesAgg::default();
                agg.active = vec![1];
            }
            let per_cell = plant / w.biome.cells.len() as f32;
            for cell in w.biome.cells.iter_mut() {
                cell.plant_biomass = per_cell;
            }
            detect_trophic_cascade(w, &agg);
        }
    }

    #[test]
    fn ordered_crash_boom_drop_fires_cascade() {
        let mut w = world_with_agents(4);
        let mut script = Vec::new();
        // Establish a carnivore peak (fills the 150-tick window).
        script.extend(std::iter::repeat_n((20, 100, 1000.0), CASCADE_WINDOW));
        // Stage 1: carnivore crash (20 → 5 = 75% drop).
        script.push((5, 100, 1000.0));
        // Stage 2: herbivore boom (+50%).
        script.push((5, 150, 1000.0));
        // Fire: plant crash (-50%).
        script.push((5, 150, 500.0));
        drive_cascade(&mut w, &script);
        let fired: Vec<_> =
            w.codex.events.iter().filter(|e| e.event_type == EventType::TrophicCascade).collect();
        assert_eq!(fired.len(), 1, "ordered cascade fires exactly once");
        assert!((fired[0].value - 0.75).abs() < 0.01, "drop fraction: {}", fired[0].value);
    }

    #[test]
    fn out_of_order_fluctuations_do_not_fire_cascade() {
        let mut w = world_with_agents(4);
        let mut script = Vec::new();
        script.extend(std::iter::repeat_n((20, 100, 1000.0), CASCADE_WINDOW));
        // Plant crash FIRST (out of order) — machine is still armed; the
        // subsequent carnivore crash may open a candidate…
        script.push((20, 100, 500.0));
        script.push((5, 100, 500.0));
        // …but plants recover instead of crashing after the herbivore boom.
        script.push((5, 150, 1000.0));
        script.extend(std::iter::repeat_n((5, 150, 1000.0), 10));
        drive_cascade(&mut w, &script);
        assert!(
            !w.codex.events.iter().any(|e| e.event_type == EventType::TrophicCascade),
            "plant crash before the predator crash must not complete a cascade"
        );
    }

    #[test]
    fn cascade_candidate_times_out() {
        let mut w = world_with_agents(4);
        let mut script = Vec::new();
        script.extend(std::iter::repeat_n((20, 100, 1000.0), CASCADE_WINDOW));
        script.push((5, 100, 1000.0)); // crash opens stage 1 at tick 150
                                       // No herbivore boom: the candidate times out at +CASCADE_LAG. The
                                       // still-depressed carnivore count legitimately re-opens fresh
                                       // candidates while the 150-tick peak window still holds the old
                                       // peak (so until ~tick 300); the boom must therefore arrive a full
                                       // lag after the last possible re-open, when the window peak has
                                       // decayed to the depressed level (too low to crash from).
        script.extend(std::iter::repeat_n((5, 100, 1000.0), 460));
        // Boom arrives with the machine armed and a low carnivore peak; it
        // is never consumed, and the plant drop cannot complete a cascade.
        script.push((5, 150, 1000.0));
        script.push((5, 150, 500.0));
        drive_cascade(&mut w, &script);
        assert!(w.codex.events.is_empty(), "expired candidate must not fire");
    }
}
