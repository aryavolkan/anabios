//! Modular morphology (M3).
//!
//! Each agent carries a `SmallVec<[Module; 8]>` (typically 3-12 modules)
//! that define what it can do. Module presence gates actions in the tick
//! pipeline:
//!
//! - No `Locomotor`  → cannot move
//! - No `Sensor`     → cannot perceive plants or neighbours
//! - No `Mouth`      → cannot feed
//! - No `Reproductive` → cannot mate
//!
//! Other module types (`Weapon`, `Armor`, `Storage`, `Communicator`,
//! `Pheromone`) are part of the M3 substrate but their gameplay effects
//! land in later milestones (combat in M4, pheromones in a later
//! milestone). They still pay upkeep when present.
//!
//! All parameters are `f32` in `[0, 1]` and are perturbed by Gaussian
//! mutation during reproduction. Whole-module mutation (add, delete,
//! duplicate, replace) is also applied with low probability.

use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};

use crate::rng::Rng;

/// Maximum number of modules per agent. The `SmallVec` inline storage is
/// also 8; agents with > 8 modules spill to the heap.
pub const MODULE_INLINE_CAPACITY: usize = 8;
pub const MODULE_LIST_MAX: usize = 16;

/// Per-module per-tick upkeep cost at parameter value 1.0. Actual cost
/// scales linearly with the dominant parameter of the module.
pub const UPKEEP_BASE: f32 = 0.005;

/// Module-list inheritance probabilities applied during reproduction.
pub const MUTATE_PARAM_PROB: f32 = 0.5;
pub const ADD_MODULE_PROB: f32 = 0.02;
pub const DELETE_MODULE_PROB: f32 = 0.02;
pub const DUPLICATE_MODULE_PROB: f32 = 0.02;
pub const REPLACE_MODULE_PROB: f32 = 0.01;

/// Gaussian sigma when perturbing a single module parameter.
pub const PARAM_SIGMA: f32 = 0.05;

/// Sensor channel type. Vision sees plants and other agents; smell, heat,
/// and sound are reserved for later milestones and have no effect in M3.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SensorType {
    Vision = 0,
    Smell = 1,
    Heat = 2,
    Sound = 3,
}

/// Pheromone channel id. Multiple channels coexist; M3 does not yet read
/// pheromones in any tick stage (no field present in `World`), so the
/// channel value is currently inert metadata. Reserved for later.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PheromoneChannel {
    Alarm = 0,
    Mate = 1,
    Trail = 2,
    Marker = 3,
}

/// One body module.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Module {
    /// Enables motion. `max_speed` scales the agent's velocity cap.
    /// `terrain_affinity` is reserved for M4+ (will gate land vs water
    /// crossing); currently inert.
    Locomotor { max_speed: f32, terrain_affinity: f32 },
    /// Enables one channel of perception. `radius` and `acuity` shape
    /// what the agent can sense.
    Sensor { sensor_type: SensorType, radius: f32, acuity: f32 },
    /// Enables feeding. `bite_size` caps biomass per bite; `diet_affinity`
    /// = 0 → pure herbivore, 1 → pure carnivore (carnivory has no effect
    /// in M3 since combat is M4).
    Mouth { bite_size: f32, diet_affinity: f32 },
    /// Inflicts damage on contact. No gameplay effect in M3 (combat is
    /// later); pays upkeep.
    Weapon { damage: f32, energy_cost: f32 },
    /// Reduces damage. No gameplay effect in M3; pays upkeep.
    Armor { protection: f32, mass_penalty: f32 },
    /// Increases the agent's effective energy capacity. No gameplay
    /// effect in M3 (no overflow check yet); pays upkeep.
    Storage { capacity: f32 },
    /// Emits/receives meme signals. No gameplay effect in M3; pays upkeep.
    Communicator { range: f32, channel_id: u8 },
    /// Leaves chemical marks on the biome. No gameplay effect in M3 (no
    /// pheromone field yet); pays upkeep.
    Pheromone { channel: PheromoneChannel, strength: f32, decay: f32 },
    /// Required for reproduction. `viability` modulates the mating energy
    /// cost; `brood_size_bias` is reserved for M5.
    Reproductive { viability: f32, brood_size_bias: f32 },
}

/// Discriminant tag — useful when generating a random module or checking
/// "do I have any module of type X".
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleType {
    Locomotor = 0,
    Sensor = 1,
    Mouth = 2,
    Weapon = 3,
    Armor = 4,
    Storage = 5,
    Communicator = 6,
    Pheromone = 7,
    Reproductive = 8,
}

impl Module {
    /// Tag-only view of this module's type.
    #[inline]
    pub fn module_type(&self) -> ModuleType {
        match self {
            Module::Locomotor { .. } => ModuleType::Locomotor,
            Module::Sensor { .. } => ModuleType::Sensor,
            Module::Mouth { .. } => ModuleType::Mouth,
            Module::Weapon { .. } => ModuleType::Weapon,
            Module::Armor { .. } => ModuleType::Armor,
            Module::Storage { .. } => ModuleType::Storage,
            Module::Communicator { .. } => ModuleType::Communicator,
            Module::Pheromone { .. } => ModuleType::Pheromone,
            Module::Reproductive { .. } => ModuleType::Reproductive,
        }
    }

    /// Per-tick upkeep cost in energy units. Scales with the module's
    /// dominant parameter so a high-capacity organ costs more than a
    /// vestigial one.
    pub fn upkeep(&self) -> f32 {
        let factor = match self {
            Module::Locomotor { max_speed, .. } => *max_speed,
            Module::Sensor { radius, acuity, .. } => 0.5 * (radius + acuity),
            Module::Mouth { bite_size, .. } => *bite_size,
            Module::Weapon { damage, .. } => *damage,
            Module::Armor { protection, mass_penalty } => 0.5 * (protection + mass_penalty),
            Module::Storage { capacity } => *capacity,
            Module::Communicator { range, .. } => *range,
            Module::Pheromone { strength, .. } => *strength,
            Module::Reproductive { viability, .. } => *viability,
        };
        UPKEEP_BASE * factor.clamp(0.05, 1.0)
    }

    /// Construct a random module of the given type with uniform parameters.
    pub fn random_of_type(module_type: ModuleType, rng: &mut Rng) -> Module {
        let p = |rng: &mut Rng| rng.f32_unit();
        match module_type {
            ModuleType::Locomotor => {
                Module::Locomotor { max_speed: p(rng), terrain_affinity: p(rng) }
            }
            ModuleType::Sensor => Module::Sensor {
                sensor_type: match (rng.f32_unit() * 4.0) as u8 {
                    0 => SensorType::Vision,
                    1 => SensorType::Smell,
                    2 => SensorType::Heat,
                    _ => SensorType::Sound,
                },
                radius: p(rng),
                acuity: p(rng),
            },
            ModuleType::Mouth => Module::Mouth { bite_size: p(rng), diet_affinity: p(rng) },
            ModuleType::Weapon => Module::Weapon { damage: p(rng), energy_cost: p(rng) },
            ModuleType::Armor => Module::Armor { protection: p(rng), mass_penalty: p(rng) },
            ModuleType::Storage => Module::Storage { capacity: p(rng) },
            ModuleType::Communicator => {
                Module::Communicator { range: p(rng), channel_id: (rng.f32_unit() * 4.0) as u8 }
            }
            ModuleType::Pheromone => Module::Pheromone {
                channel: match (rng.f32_unit() * 4.0) as u8 {
                    0 => PheromoneChannel::Alarm,
                    1 => PheromoneChannel::Mate,
                    2 => PheromoneChannel::Trail,
                    _ => PheromoneChannel::Marker,
                },
                strength: p(rng),
                decay: p(rng),
            },
            ModuleType::Reproductive => {
                Module::Reproductive { viability: p(rng), brood_size_bias: p(rng) }
            }
        }
    }

    /// Construct a random module of any type. Used by the structural
    /// "add" and "replace" mutation operators.
    pub fn random_any(rng: &mut Rng) -> Module {
        let t = match (rng.f32_unit() * 9.0) as u8 {
            0 => ModuleType::Locomotor,
            1 => ModuleType::Sensor,
            2 => ModuleType::Mouth,
            3 => ModuleType::Weapon,
            4 => ModuleType::Armor,
            5 => ModuleType::Storage,
            6 => ModuleType::Communicator,
            7 => ModuleType::Pheromone,
            _ => ModuleType::Reproductive,
        };
        Module::random_of_type(t, rng)
    }
}

/// Variable-length module list owned by an agent.
pub type ModuleList = SmallVec<[Module; MODULE_INLINE_CAPACITY]>;

/// The default 4-module kit assigned to every founder spawned via
/// `World::spawn_agent`. All four are at parameter value 0.6 (above the
/// upkeep dead-band, below max).
pub fn starter_kit() -> ModuleList {
    smallvec![
        Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 },
        Module::Sensor { sensor_type: SensorType::Vision, radius: 0.6, acuity: 0.6 },
        Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 },
        Module::Reproductive { viability: 0.6, brood_size_bias: 0.5 },
    ]
}

/// `true` iff the list contains at least one module of the given type.
#[inline]
pub fn has(modules: &ModuleList, module_type: ModuleType) -> bool {
    modules.iter().any(|m| m.module_type() == module_type)
}

/// Total per-tick upkeep cost.
#[inline]
pub fn total_upkeep(modules: &ModuleList) -> f32 {
    modules.iter().map(|m| m.upkeep()).sum()
}

/// Sum the `max_speed` of every Locomotor in the list. Used by the
/// integrate stage; 0.0 if no Locomotor is present (agent can't move).
#[inline]
pub fn effective_speed_max(modules: &ModuleList) -> f32 {
    modules
        .iter()
        .filter_map(|m| match m {
            Module::Locomotor { max_speed, .. } => Some(*max_speed),
            _ => None,
        })
        .sum()
}

/// Maximum perception radius across all Sensor modules. 0.0 if no Sensor.
#[inline]
pub fn effective_perception_radius(modules: &ModuleList) -> f32 {
    modules
        .iter()
        .filter_map(|m| match m {
            Module::Sensor { radius, .. } => Some(*radius),
            _ => None,
        })
        .fold(0.0_f32, f32::max)
}

/// Maximum bite size across all Mouth modules. 0.0 if no Mouth.
#[inline]
pub fn effective_bite_size(modules: &ModuleList) -> f32 {
    modules
        .iter()
        .filter_map(|m| match m {
            Module::Mouth { bite_size, .. } => Some(*bite_size),
            _ => None,
        })
        .fold(0.0_f32, f32::max)
}

/// Maximum diet affinity across all Mouth modules. 0.0 (pure herbivore)
/// if no Mouth, but action gating means feeding is skipped anyway.
#[inline]
pub fn effective_diet_carnivory(modules: &ModuleList) -> f32 {
    modules
        .iter()
        .filter_map(|m| match m {
            Module::Mouth { diet_affinity, .. } => Some(*diet_affinity),
            _ => None,
        })
        .fold(0.0_f32, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_kit_has_required_modules() {
        let k = starter_kit();
        assert!(has(&k, ModuleType::Locomotor));
        assert!(has(&k, ModuleType::Sensor));
        assert!(has(&k, ModuleType::Mouth));
        assert!(has(&k, ModuleType::Reproductive));
    }

    #[test]
    fn upkeep_is_proportional_to_dominant_param() {
        let small = Module::Locomotor { max_speed: 0.1, terrain_affinity: 0.5 };
        let big = Module::Locomotor { max_speed: 1.0, terrain_affinity: 0.5 };
        assert!(big.upkeep() > small.upkeep());
    }

    #[test]
    fn random_module_is_deterministic() {
        let mut a = Rng::from_seed(42);
        let mut b = Rng::from_seed(42);
        for _ in 0..20 {
            assert_eq!(Module::random_any(&mut a), Module::random_any(&mut b));
        }
    }

    #[test]
    fn module_type_matches_variant() {
        for t in [
            ModuleType::Locomotor,
            ModuleType::Sensor,
            ModuleType::Mouth,
            ModuleType::Weapon,
            ModuleType::Armor,
            ModuleType::Storage,
            ModuleType::Communicator,
            ModuleType::Pheromone,
            ModuleType::Reproductive,
        ] {
            let mut rng = Rng::from_seed(1);
            let m = Module::random_of_type(t, &mut rng);
            assert_eq!(m.module_type(), t);
        }
    }
}
