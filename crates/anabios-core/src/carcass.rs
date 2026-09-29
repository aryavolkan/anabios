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

/// Carcass seeking (chase layer, `World::chase_enabled`): a carnivore at or
/// above this carnivory that is below `CARCASS_SEEK_SATIETY` energy steers
/// toward the nearest carcass with flesh within `CARCASS_SEEK_REACH` and,
/// once within `SCAVENGE_RANGE`, stands to eat. Without it a predator left
/// its kill the tick it made it: measured on `predator-prey` (2026-09-29,
/// every realism knob off, 1500 ticks) the pursuers made 227 kills and let
/// 6346 of the 6070 flesh units those created rot (the difference is
/// natural deaths), sitting at a carcass on 10% of their ticks — and no
/// pursuer lineage ever fed itself.
pub const CARCASS_SEEK_CARNIVORY: f32 = 0.5;
/// Energy at and above which a carnivore leaves carcasses alone.
pub const CARCASS_SEEK_SATIETY: f32 = 2.0 * crate::agent::SPAWN_ENERGY;
/// How far a hungry carnivore looks for a carcass, in world units (a mid
/// perception radius; the water and mate pulls reach 96).
pub const CARCASS_SEEK_REACH: f32 = 48.0;
/// Length of the move intent toward the carcass; it replaces the program's
/// movement (the mate pull's 2.0 gain, so the gait reads a full intent).
pub const CARCASS_PULL: f32 = 2.0;

/// The unit direction (torus) from `pos` to the nearest carcass with flesh
/// within `reach`, and its distance; `None` when there is none. A linear
/// scan (carcasses number tens to hundreds, and the carcass hash's ring is
/// one cell wide) with strict `<` on the squared distance and the lowest
/// index on ties, so the choice is deterministic.
pub fn nearest_carcass(
    carcasses: &[Carcass],
    pos: Vec2,
    reach: f32,
    ws: f32,
) -> Option<(Vec2, f32)> {
    let mut best: Option<(usize, f32)> = None;
    for (ci, c) in carcasses.iter().enumerate() {
        if c.flesh <= 0.0 {
            continue;
        }
        let d2 = crate::spatial::torus_distance_sq(pos, c.pos, ws);
        if d2 <= reach * reach && best.is_none_or(|(_, b)| d2 < b) {
            best = Some((ci, d2));
        }
    }
    best.map(|(ci, d2)| {
        let d = crate::spatial::torus_delta(carcasses[ci].pos, pos, ws);
        (d.normalize_or_zero(), d2.sqrt())
    })
}

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
