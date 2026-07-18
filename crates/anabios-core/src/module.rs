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

/// A carnivore starter kit: mobile, sighted, meat-eating, and armed. Used by
/// the `stalker`/`pack_hunter` scenario archetypes.
pub fn predator_kit() -> ModuleList {
    smallvec![
        Module::Locomotor { max_speed: 0.7, terrain_affinity: 0.5 },
        Module::Sensor { sensor_type: SensorType::Vision, radius: 0.8, acuity: 0.7 },
        Module::Mouth { bite_size: 0.6, diet_affinity: 1.0 },
        Module::Weapon { damage: 8.0, energy_cost: 1.0 },
    ]
}

/// A pheromone-marking herbivore: mobile, smells pheromones, grazes, and marks
/// territory on the Marker channel. Used by the `marker` scenario archetype.
pub fn marker_kit() -> ModuleList {
    smallvec![
        Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 },
        Module::Sensor { sensor_type: SensorType::Smell, radius: 0.7, acuity: 0.6 },
        Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 },
        Module::Pheromone { channel: PheromoneChannel::Marker, strength: 1.0, decay: 0.1 },
    ]
}

/// A meme-broadcasting herbivore: mobile, sighted, grazes, and communicates on
/// channel 0. Used by the `communicator` scenario archetype.
pub fn communicator_kit() -> ModuleList {
    smallvec![
        Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 },
        Module::Sensor { sensor_type: SensorType::Vision, radius: 0.6, acuity: 0.6 },
        Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 },
        Module::Communicator { range: 12.0, channel_id: 0 },
    ]
}

/// Gene-culture experiment: an omnivore hunter — grazes (fallback) AND can hunt
/// (Weapon + carnivore-capable Mouth) + communicates. `FAST` sets a high
/// Locomotor max_speed (the primal "speed gene"); the slow variant is identical
/// but slow. The hunt-technique meme's payoff is conditional on this gene.
pub fn fast_hunter_kit() -> ModuleList {
    smallvec![
        Module::Locomotor { max_speed: 0.95, terrain_affinity: 0.5 },
        Module::Sensor { sensor_type: SensorType::Vision, radius: 0.8, acuity: 0.7 },
        Module::Mouth { bite_size: 0.6, diet_affinity: 1.0 },
        Module::Weapon { damage: 8.0, energy_cost: 1.0 },
        Module::Communicator { range: 12.0, channel_id: 0 },
    ]
}

/// Slow variant of `fast_hunter_kit` — identical except a low Locomotor speed.
pub fn slow_hunter_kit() -> ModuleList {
    smallvec![
        Module::Locomotor { max_speed: 0.3, terrain_affinity: 0.5 },
        Module::Sensor { sensor_type: SensorType::Vision, radius: 0.8, acuity: 0.7 },
        Module::Mouth { bite_size: 0.6, diet_affinity: 1.0 },
        Module::Weapon { damage: 8.0, energy_cost: 1.0 },
        Module::Communicator { range: 12.0, channel_id: 0 },
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

/// Fold the extracted per-module parameter with `f32::max`, defaulting to 0.0
/// when no module contributes. Shared by the "strongest module wins" accessors.
fn max_param(modules: &ModuleList, extract: impl Fn(&Module) -> Option<f32>) -> f32 {
    modules.iter().filter_map(extract).fold(0.0_f32, f32::max)
}

/// Maximum perception radius across all Sensor modules. 0.0 if no Sensor.
#[inline]
pub fn effective_perception_radius(modules: &ModuleList) -> f32 {
    max_param(modules, |m| match m {
        Module::Sensor { radius, .. } => Some(*radius),
        _ => None,
    })
}

/// Maximum bite size across all Mouth modules. 0.0 if no Mouth.
#[inline]
pub fn effective_bite_size(modules: &ModuleList) -> f32 {
    max_param(modules, |m| match m {
        Module::Mouth { bite_size, .. } => Some(*bite_size),
        _ => None,
    })
}

/// Maximum diet affinity across all Mouth modules. 0.0 (pure herbivore)
/// if no Mouth, but action gating means feeding is skipped anyway.
#[inline]
pub fn effective_diet_carnivory(modules: &ModuleList) -> f32 {
    max_param(modules, |m| match m {
        Module::Mouth { diet_affinity, .. } => Some(*diet_affinity),
        _ => None,
    })
}

/// Damage + energy_cost of the highest-damage `Weapon`, or `None` if the
/// agent has no `Weapon` module (combat gating, design §3.5).
#[inline]
pub fn effective_weapon(modules: &ModuleList) -> Option<(f32, f32)> {
    modules
        .iter()
        .filter_map(|m| match m {
            Module::Weapon { damage, energy_cost } => Some((*damage, *energy_cost)),
            _ => None,
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
}

/// Max `Pheromone.strength`, or `0.0` if the agent has no `Pheromone` module.
#[inline]
pub fn effective_pheromone_strength(modules: &ModuleList) -> f32 {
    max_param(modules, |m| match m {
        Module::Pheromone { strength, .. } => Some(*strength),
        _ => None,
    })
}

/// `true` iff the agent has a `Sensor` module of type `Smell` (gates pheromone
/// perception, design §3.6).
#[inline]
pub fn has_smell(modules: &ModuleList) -> bool {
    modules.iter().any(|m| matches!(m, Module::Sensor { sensor_type: SensorType::Smell, .. }))
}

/// Max `Armor.protection`, or `0.0` if the agent has no `Armor` module.
#[inline]
pub fn effective_armor_protection(modules: &ModuleList) -> f32 {
    max_param(modules, |m| match m {
        Module::Armor { protection, .. } => Some(*protection),
        _ => None,
    })
}

/// Max `Communicator.range`, or `0.0` if the agent has no `Communicator`.
#[inline]
pub fn effective_communicator_range(modules: &ModuleList) -> f32 {
    max_param(modules, |m| match m {
        Module::Communicator { range, .. } => Some(*range),
        _ => None,
    })
}

/// Perturb every parameter of `module` with probability `MUTATE_PARAM_PROB`,
/// drawing perturbations from `N(0, PARAM_SIGMA)` and clamping back into
/// `[0, 1]`. Per-slot decisions consume the RNG in a fixed order so the
/// result is deterministic.
pub fn mutate_params(module: &mut Module, rng: &mut Rng) {
    fn perturb(v: &mut f32, rng: &mut Rng) {
        if rng.f32_unit() < MUTATE_PARAM_PROB {
            *v = (*v + rng.gaussian(0.0, PARAM_SIGMA)).clamp(0.0, 1.0);
        }
    }
    match module {
        Module::Locomotor { max_speed, terrain_affinity } => {
            perturb(max_speed, rng);
            perturb(terrain_affinity, rng);
        }
        Module::Sensor { sensor_type: _, radius, acuity } => {
            perturb(radius, rng);
            perturb(acuity, rng);
        }
        Module::Mouth { bite_size, diet_affinity } => {
            perturb(bite_size, rng);
            perturb(diet_affinity, rng);
        }
        Module::Weapon { damage, energy_cost } => {
            perturb(damage, rng);
            perturb(energy_cost, rng);
        }
        Module::Armor { protection, mass_penalty } => {
            perturb(protection, rng);
            perturb(mass_penalty, rng);
        }
        Module::Storage { capacity } => {
            perturb(capacity, rng);
        }
        Module::Communicator { range, channel_id: _ } => {
            perturb(range, rng);
        }
        Module::Pheromone { channel: _, strength, decay } => {
            perturb(strength, rng);
            perturb(decay, rng);
        }
        Module::Reproductive { viability, brood_size_bias } => {
            perturb(viability, rng);
            perturb(brood_size_bias, rng);
        }
    }
}

/// Apply structural mutations to `modules` in place. Each operator fires
/// independently with its own probability. The list is clamped to
/// `[0, MODULE_LIST_MAX]` items; if a delete would empty the list, it
/// skips to leave at least one module (so the agent is not entirely
/// vestigial — extinction by full module loss is unproductive noise).
pub fn structural_mutate(modules: &mut ModuleList, rng: &mut Rng) {
    // Add
    if modules.len() < MODULE_LIST_MAX && rng.f32_unit() < ADD_MODULE_PROB {
        modules.push(Module::random_any(rng));
    }
    // Duplicate
    if modules.len() < MODULE_LIST_MAX
        && !modules.is_empty()
        && rng.f32_unit() < DUPLICATE_MODULE_PROB
    {
        let pick = rng.index(modules.len());
        let copy = modules[pick];
        modules.push(copy);
    }
    // Replace
    if !modules.is_empty() && rng.f32_unit() < REPLACE_MODULE_PROB {
        let pick = rng.index(modules.len());
        modules[pick] = Module::random_any(rng);
    }
    // Delete (last, so we don't replace then immediately delete)
    if modules.len() > 1 && rng.f32_unit() < DELETE_MODULE_PROB {
        let pick = rng.index(modules.len());
        modules.remove(pick);
    }
}

/// Build a child's module list from two parents:
/// 1. For each slot index up to the longer parent's length, inherit from
///    parent A or parent B with equal probability (per-slot uniform
///    crossover). The shorter parent's slots beyond its length are skipped
///    (so the child's length lands between the two parents' lengths).
/// 2. Run `mutate_params` on every inherited module.
/// 3. Run `structural_mutate` once on the resulting list.
pub fn crossover_and_mutate(a: &ModuleList, b: &ModuleList, rng: &mut Rng) -> ModuleList {
    let max_len = a.len().max(b.len());
    let mut out = ModuleList::new();
    for i in 0..max_len {
        let from_a = rng.f32_unit() < 0.5;
        let chosen = if from_a {
            if i < a.len() {
                Some(&a[i])
            } else if i < b.len() {
                Some(&b[i])
            } else {
                None
            }
        } else if i < b.len() {
            Some(&b[i])
        } else if i < a.len() {
            Some(&a[i])
        } else {
            None
        };
        if let Some(m) = chosen {
            let mut copy = *m;
            mutate_params(&mut copy, rng);
            out.push(copy);
        }
    }
    structural_mutate(&mut out, rng);
    out
}

/// Deduct per-tick module upkeep from every alive agent. Modules cost
/// energy continuously regardless of whether they were used this tick;
/// agents with too many modules for their food intake go negative and
/// die in the subsequent `age_and_starve` stage.
pub fn upkeep_all(agents: &mut crate::agent::AgentBuffers) {
    let mut ids = std::mem::take(&mut agents.scratch_ids);
    ids.clear();
    ids.extend(agents.iter_alive());
    for &id in &ids {
        let i = id as usize;
        let cost = total_upkeep(&agents.modules[i]);
        agents.energy[i] -= cost;
    }
    agents.scratch_ids = ids;
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

    #[test]
    fn mutate_params_keeps_values_in_range() {
        let mut rng = Rng::from_seed(7);
        let mut m = Module::Locomotor { max_speed: 0.5, terrain_affinity: 0.5 };
        for _ in 0..200 {
            mutate_params(&mut m, &mut rng);
            if let Module::Locomotor { max_speed, terrain_affinity } = m {
                assert!((0.0..=1.0).contains(&max_speed));
                assert!((0.0..=1.0).contains(&terrain_affinity));
            }
        }
    }

    #[test]
    fn structural_mutate_never_empties_the_list() {
        let mut rng = Rng::from_seed(11);
        let mut k = starter_kit();
        for _ in 0..1000 {
            structural_mutate(&mut k, &mut rng);
            assert!(!k.is_empty());
            assert!(k.len() <= MODULE_LIST_MAX);
        }
    }

    #[test]
    fn crossover_with_identical_parents_yields_same_length_distribution() {
        let mut rng = Rng::from_seed(13);
        let p = starter_kit();
        let mut len_sum = 0;
        let n = 100;
        for _ in 0..n {
            let c = crossover_and_mutate(&p, &p, &mut rng);
            len_sum += c.len();
        }
        // With identical parents and small structural mutation rates,
        // child length should average close to parent length.
        let avg = len_sum as f32 / n as f32;
        let parent_len = p.len() as f32;
        assert!(
            (avg - parent_len).abs() < 1.5,
            "average child length {avg} differs significantly from parent {parent_len}",
        );
    }

    #[test]
    fn crossover_is_deterministic() {
        let p = starter_kit();
        let mut r1 = Rng::from_seed(99);
        let mut r2 = Rng::from_seed(99);
        let c1 = crossover_and_mutate(&p, &p, &mut r1);
        let c2 = crossover_and_mutate(&p, &p, &mut r2);
        assert_eq!(c1, c2);
    }

    #[test]
    fn upkeep_all_deducts_starter_kit_cost() {
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::world::World;
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let before = w.agents.energy[id as usize];
        upkeep_all(&mut w.agents);
        let after = w.agents.energy[id as usize];
        let expected_cost = total_upkeep(&w.agents.modules[id as usize]);
        assert!((before - after - expected_cost).abs() < 1e-5);
    }
}
