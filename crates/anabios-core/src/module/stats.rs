//! Read-only module-list stat accessors (`effective_*`, `has_smell`, and the
//! `WeaponStats` view). Split out of `module/mod.rs`; re-exported via
//! `pub use stats::*`, so `crate::module::effective_*` / `WeaponStats` paths
//! are unchanged.

use super::*;

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

/// The strongest weapon an agent carries: damage, per-shot energy cost, and
/// effective reach (world units), resolved across all weapon module types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponStats {
    pub damage: f32,
    pub energy_cost: f32,
    pub range: f32,
}

/// Stats of the highest-damage weapon module (`Weapon`, `Spines`, or
/// `Jaws`), or `None` if the agent is unarmed (combat gating, design §3.5).
#[inline]
pub fn effective_weapon(modules: &ModuleList) -> Option<WeaponStats> {
    modules
        .iter()
        .filter_map(|m| match m {
            Module::Weapon { damage, energy_cost } => Some(WeaponStats {
                damage: *damage,
                energy_cost: *energy_cost,
                range: WEAPON_RANGE,
            }),
            Module::Spines { damage, energy_cost, range } => Some(WeaponStats {
                damage: *damage,
                energy_cost: *energy_cost,
                range: effective_spines_range(*range),
            }),
            Module::Jaws { damage, energy_cost } => {
                Some(WeaponStats { damage: *damage, energy_cost: *energy_cost, range: JAWS_RANGE })
            }
            _ => None,
        })
        .max_by(|a, b| a.damage.partial_cmp(&b.damage).unwrap_or(std::cmp::Ordering::Equal))
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
