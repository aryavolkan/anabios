//! Carcasses: dead-but-edible flesh left by killed/starved agents. Carnivore
//! Mouth modules scavenge them (see `interact::scavenge_pass`). Flesh energy is
//! proportional to body size, not the (depleted) metabolic energy at death —
//! agents die at energy ≤ 0, so flesh must come from body mass to close the
//! trophic loop.

use serde::{Deserialize, Serialize};

use crate::prelude::Vec2;
use crate::world::World;

/// Flesh energy per unit of `GenomeSlot::Size` a fresh carcass carries.
/// (Balance value; tuning deferred to M16.)
pub const CARCASS_FLESH_PER_SIZE: f32 = 20.0;
/// Ticks after which a carcass is removed even if not fully scavenged.
pub const CARCASS_DECAY_TICKS: u32 = 100;
/// Max distance (world units) a carnivore can reach a carcass. Mirrors
/// `interact::COMBAT_RANGE`.
pub const SCAVENGE_RANGE: f32 = 2.0;
/// Max flesh a Mouth can take from a carcass in one tick (before scaling).
pub const SCAVENGE_MAX: f32 = 0.5;
/// Energy yielded per unit of flesh scavenged (mirrors FOOD_ENERGY_PER_BIOMASS).
pub const FLESH_ENERGY_PER_UNIT: f32 = 4.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Carcass {
    pub pos: Vec2,
    pub flesh: f32,
    pub age: u32,
    pub species_id: u32,
}

/// Age every carcass by one tick and drop the depleted/expired ones.
/// `retain` preserves order → deterministic.
pub fn carcass_step(world: &mut World) {
    for c in world.carcasses.iter_mut() {
        c.age = c.age.saturating_add(1);
    }
    world.carcasses.retain(|c| c.flesh > 0.0 && c.age < CARCASS_DECAY_TICKS);
}
