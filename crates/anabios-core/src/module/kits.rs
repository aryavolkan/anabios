//! Founder module kits — the fixed `ModuleList` presets scenarios spawn
//! archetypes from. Split out of `module/mod.rs`; re-exported via
//! `pub use kits::*`, so `crate::module::<name>_kit` paths are unchanged.

use super::*;

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

/// A ranged-hunter kit: keen eyes, a carnivore mouth, and a `Spines` volley
/// weapon that kills from beyond contact range. Speed 0.65 outspeeds grazers
/// (0.6) and bruisers (0.5) so the kiting program can hold its standoff ring,
/// while stalkers (0.7) still run it down — a rock-paper-scissors balance.
/// Includes `Reproductive` so the lineage can establish and evolve (unlike
/// the founder predator kits). Used by the `spiner` scenario archetype.
pub fn spiner_kit() -> ModuleList {
    smallvec![
        Module::Locomotor { max_speed: 0.65, terrain_affinity: 0.5 },
        Module::Sensor { sensor_type: SensorType::Vision, radius: 0.9, acuity: 0.75 },
        Module::Mouth { bite_size: 0.5, diet_affinity: 1.0 },
        Module::Spines { damage: 4.0, energy_cost: 1.5, range: 0.8 },
        Module::Reproductive { viability: 0.6, brood_size_bias: 0.5 },
    ]
}

/// A heavy-assault kit: slow, armored, and equipped with `Jaws` — the
/// hardest-hitting but shortest-reaching weapon in the arsenal. Includes
/// `Reproductive` so the lineage can establish and evolve. Used by the
/// `bruiser` scenario archetype.
pub fn bruiser_kit() -> ModuleList {
    smallvec![
        Module::Locomotor { max_speed: 0.5, terrain_affinity: 0.5 },
        Module::Sensor { sensor_type: SensorType::Vision, radius: 0.7, acuity: 0.6 },
        Module::Mouth { bite_size: 0.7, diet_affinity: 1.0 },
        Module::Jaws { damage: 14.0, energy_cost: 2.0 },
        Module::Armor { protection: 1.0, mass_penalty: 0.2 },
        Module::Reproductive { viability: 0.6, brood_size_bias: 0.5 },
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
