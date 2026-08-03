//! O1 diagnostic instrument: per-strategy aggregates read from a `&World`.
//!
//! Headless-only and read-only — never mutates sim state, so it adds zero
//! determinism/golden risk (mirrors `soak.rs`'s telemetry stance). "Strategy"
//! is keyed on Communicator-module presence: culture-capable cognition vs the
//! asocial baseline. That key is heritable and survives species reclustering,
//! unlike `species_id`.

use anabios_core::culture::SKILL_CHANNEL;
use anabios_core::invention::{held_mask, tech_era};
use anabios_core::module::{self, ModuleType};
use anabios_core::world::World;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StrategyKind {
    Cultural,
    Asocial,
}

pub fn strategy_label(k: StrategyKind) -> &'static str {
    match k {
        StrategyKind::Cultural => "cultural",
        StrategyKind::Asocial => "asocial",
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StrategyStat {
    pub kind: StrategyKind,
    pub count: u32,
    pub mean_energy: f64,
    pub mean_skill: f64,
    pub mean_iq: f64,
    pub mean_era: f64,
    pub max_era: u8,
}

impl StrategyStat {
    fn empty(kind: StrategyKind) -> Self {
        StrategyStat {
            kind,
            count: 0,
            mean_energy: 0.0,
            mean_skill: 0.0,
            mean_iq: 0.0,
            mean_era: 0.0,
            max_era: 0,
        }
    }
}

// Running accumulator, folded into a StrategyStat at the end.
#[derive(Default)]
struct Acc {
    count: u64,
    energy: f64,
    skill: f64,
    iq: f64,
    era_sum: f64,
    max_era: u8,
}

impl Acc {
    fn finish(self, kind: StrategyKind) -> StrategyStat {
        if self.count == 0 {
            return StrategyStat::empty(kind);
        }
        let n = self.count as f64;
        StrategyStat {
            kind,
            count: self.count as u32,
            mean_energy: self.energy / n,
            mean_skill: self.skill / n,
            mean_iq: self.iq / n,
            mean_era: self.era_sum / n,
            max_era: self.max_era,
        }
    }
}

/// One measurement window for invasion analysis: the mutant strategy's count
/// and the total live population at `tick`.
#[derive(Clone, Copy, Debug)]
pub struct InvasionWindow {
    pub mutant_n: u32,
    pub total_n: u32,
}

/// Invasion fitness: the mutant's mean per-window log-growth rate while rare.
///
/// Averages `ln(n[k+1]/n[k])` over consecutive window pairs where window `k` is
/// below `rare_frac_max` frequency and both counts are positive. Pairs touching
/// a zero count are skipped (an `ln(0)` would be `-inf`). Returns `None` when no
/// qualifying rare pair exists. Positive ⇒ the rare strategy can invade.
pub fn invasion_fitness(windows: &[InvasionWindow], rare_frac_max: f64) -> Option<f64> {
    let mut sum = 0.0_f64;
    let mut pairs = 0_u64;
    for pair in windows.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a.total_n == 0 {
            continue;
        }
        let freq = a.mutant_n as f64 / a.total_n as f64;
        if freq > rare_frac_max {
            continue;
        }
        if a.mutant_n == 0 || b.mutant_n == 0 {
            continue;
        }
        sum += (b.mutant_n as f64 / a.mutant_n as f64).ln();
        pairs += 1;
    }
    if pairs == 0 {
        None
    } else {
        Some(sum / pairs as f64)
    }
}

/// Per-strategy aggregate over the currently-alive agents. Index 0 = Cultural,
/// index 1 = Asocial. Read-only; safe to call every window during a run.
pub fn sample_strategies(world: &World) -> [StrategyStat; 2] {
    let mut cultural = Acc::default();
    let mut asocial = Acc::default();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let is_cultural = module::has(&world.agents.modules[i], ModuleType::Communicator);
        let acc = if is_cultural { &mut cultural } else { &mut asocial };
        acc.count += 1;
        acc.energy += world.agents.energy[i] as f64;
        acc.skill += world.agents.meme_vector[i][SKILL_CHANNEL] as f64;
        acc.iq += world.agents.iq[i] as f64;
        let era = tech_era(held_mask(&world.agents.meme_vector[i]));
        acc.era_sum += era as f64;
        acc.max_era = acc.max_era.max(era);
    }
    [cultural.finish(StrategyKind::Cultural), asocial.finish(StrategyKind::Asocial)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use anabios_core::scenario::Scenario;

    // 3 asocial foragers + 2 communicators (culture-capable). At tick 0 the
    // sampler must bucket them purely by Communicator-module presence.
    const MIX: &str = "\
name = \"t\"
seed = 1
[[agents]]
count = 3
archetype = \"asocial_forager\"
placement = { kind = \"uniform\" }
[[agents]]
count = 2
archetype = \"communicator\"
placement = { kind = \"uniform\" }
";

    #[test]
    fn sample_buckets_by_communicator_module() {
        let world = Scenario::parse_toml(MIX).unwrap().instantiate();
        let stats = sample_strategies(&world);
        assert_eq!(stats[0].kind, StrategyKind::Cultural);
        assert_eq!(stats[1].kind, StrategyKind::Asocial);
        assert_eq!(stats[0].count, 2, "two communicator agents are Cultural");
        assert_eq!(stats[1].count, 3, "three asocial foragers are Asocial");
    }

    #[test]
    fn empty_strategy_has_zero_means_not_nan() {
        // All-asocial world: the Cultural bucket is empty and must read 0.0,
        // never NaN (a NaN would poison the CSV and the invasion math).
        let only_asocial = "\
name = \"t\"
seed = 1
[[agents]]
count = 4
archetype = \"asocial_forager\"
placement = { kind = \"uniform\" }
";
        let world = Scenario::parse_toml(only_asocial).unwrap().instantiate();
        let stats = sample_strategies(&world);
        assert_eq!(stats[0].count, 0);
        assert_eq!(stats[0].mean_energy, 0.0);
        assert_eq!(stats[0].mean_skill, 0.0);
        assert_eq!(stats[0].max_era, 0);
    }
}

#[cfg(test)]
mod invasion_tests {
    use super::*;

    fn w(mutant_n: u32, total_n: u32) -> InvasionWindow {
        InvasionWindow { mutant_n, total_n }
    }

    #[test]
    fn rare_mutant_that_grows_has_positive_invasion_fitness() {
        // Mutant stays under 10% of the population but its count climbs.
        let windows = [w(10, 1000), w(20, 1000), w(40, 1000)];
        let r = invasion_fitness(&windows, 0.10).unwrap();
        assert!(r > 0.0, "growing rare mutant must invade, got {r}");
    }

    #[test]
    fn rare_mutant_that_shrinks_is_excluded() {
        let windows = [w(40, 1000), w(20, 1000), w(10, 1000)];
        let r = invasion_fitness(&windows, 0.10).unwrap();
        assert!(r < 0.0, "shrinking rare mutant is excluded, got {r}");
    }

    #[test]
    fn windows_above_rare_threshold_are_ignored() {
        // Every window is >10% frequency → no qualifying rare pair → None.
        let windows = [w(500, 1000), w(600, 1000)];
        assert!(invasion_fitness(&windows, 0.10).is_none());
    }

    #[test]
    fn extinction_pair_is_skipped_not_neg_infinity() {
        // mutant_n[k+1] == 0 would make ln(0) = -inf; that pair must be skipped.
        // The only surviving valid pair here is (10 -> 20): positive.
        let windows = [w(10, 1000), w(20, 1000), w(0, 1000)];
        let r = invasion_fitness(&windows, 0.10).unwrap();
        assert!(r.is_finite() && r > 0.0, "got {r}");
    }
}
