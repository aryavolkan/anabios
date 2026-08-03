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

pub const STRATEGY_KINDS: [StrategyKind; 2] = [StrategyKind::Cultural, StrategyKind::Asocial];

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
