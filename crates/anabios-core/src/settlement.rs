//! Settlements & economy substrate (E8): home-range anchoring, the market
//! density field, and harvest experience. Everything is gated: anchoring
//! behind `World::settlement_enabled`, the market/experience effects behind
//! `World::resources_enabled` (they piggyback the trade economy).

use crate::codex::{
    ANCHOR_LEARN_RATE, EXP_CAP, HARVEST_EXP_RATE, MARKET_DECAY, MARKET_DEPOSIT, SPECIALIZATION_GAIN,
};
use crate::genome::GenomeSlot;
use crate::prelude::Vec2;
use crate::world::World;

/// Per-tick anchor learning (EMA toward current position, scaled by the
/// agent's Territoriality drive). Torus-aware. Inserted after integrate.
pub fn anchor_step(world: &mut World) {
    if !world.settlement_enabled {
        return;
    }
    let ws = world.world_size;
    let mut ids = std::mem::take(&mut world.agents.scratch_ids);
    ids.clear();
    ids.extend(world.agents.iter_alive());
    for &id in &ids {
        let i = id as usize;
        let terr = world.agents.genome[i].get(GenomeSlot::Territoriality);
        let rate = ANCHOR_LEARN_RATE * terr;
        if rate <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let a = &mut world.agents.anchor[i];
        let mut dx = pos.x - a.x;
        let mut dy = pos.y - a.y;
        if dx > ws * 0.5 {
            dx -= ws;
        } else if dx < -ws * 0.5 {
            dx += ws;
        }
        if dy > ws * 0.5 {
            dy -= ws;
        } else if dy < -ws * 0.5 {
            dy += ws;
        }
        a.x = (a.x + dx * rate).rem_euclid(ws);
        a.y = (a.y + dy * rate).rem_euclid(ws);
    }
    world.agents.scratch_ids = ids;
}

/// Torus shortest-path delta from `pos` to `anchor`.
fn anchor_delta(anchor: Vec2, pos: Vec2, ws: f32) -> Vec2 {
    let mut dx = anchor.x - pos.x;
    let mut dy = anchor.y - pos.y;
    if dx > ws * 0.5 {
        dx -= ws;
    } else if dx < -ws * 0.5 {
        dx += ws;
    }
    if dy > ws * 0.5 {
        dy -= ws;
    } else if dy < -ws * 0.5 {
        dy += ws;
    }
    Vec2::new(dx, dy)
}

/// Homing pull parts: `Territoriality × ANCHOR_PULL × unit(anchor − pos)`.
/// Applied in `decide_all` behind the flag (rayon closure — no World).
pub fn anchor_pull_parts(anchor: Vec2, pos: Vec2, terr: f32, ws: f32) -> Vec2 {
    let d = anchor_delta(anchor, pos, ws);
    let len = d.length();
    if len < 1e-6 {
        return Vec2::ZERO;
    }
    (d / len) * (terr * crate::codex::ANCHOR_PULL)
}

/// Unit direction toward home + distance (for SenseAnchor nodes).
pub fn anchor_sense_parts(anchor: Vec2, pos: Vec2, ws: f32) -> (Vec2, f32) {
    let d = anchor_delta(anchor, pos, ws);
    let len = d.length();
    if len < 1e-6 {
        (Vec2::ZERO, 0.0)
    } else {
        (d / len, len)
    }
}

/// Deposit one successful swap at the initiator's cell (called from
/// `trade_pass`). No-op when `resources_enabled` is off.
pub fn market_deposit(world: &mut World, pos: Vec2) {
    if !world.resources_enabled || world.market_field.is_empty() {
        return;
    }
    let (col, row) = world.biome.cell_coords(pos);
    let idx = world.biome.cell_index(col, row);
    world.market_field[idx] += MARKET_DEPOSIT;
}

/// Per-tick market-field decay. Inserted into the tick pipeline.
pub fn market_decay_step(world: &mut World) {
    if !world.resources_enabled || world.market_field.is_empty() {
        return;
    }
    for v in world.market_field.iter_mut() {
        *v *= MARKET_DECAY;
        if *v < 1e-4 {
            *v = 0.0;
        }
    }
}

/// Experience-adjusted harvest amount: `base × (1 + min(exp, CAP) × GAIN)`.
pub fn experienced_harvest(base: f32, exp: f32) -> f32 {
    base * (1.0 + exp.min(EXP_CAP) * SPECIALIZATION_GAIN)
}

/// Record one harvest of good `k` (called from the harvest pass).
pub fn gain_harvest_exp(world: &mut World, i: usize, k: usize) {
    if !world.resources_enabled {
        return;
    }
    world.agents.harvest_exp[i][k] += HARVEST_EXP_RATE;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;

    #[test]
    fn anchor_learns_toward_position() {
        let mut w = World::new(1);
        w.settlement_enabled = true;
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let i = id as usize;
        // Walk the anchor's EMA target away: teleport the agent east.
        w.agents.position[i] = Vec2::new(600.0, 500.0);
        let before = w.agents.anchor[i].x;
        anchor_step(&mut w);
        let after = w.agents.anchor[i].x;
        assert!(after > before, "anchor drifts toward the agent");
        // Rate check: neutral Territoriality (0.5) × LEARN_RATE × 100 units.
        assert!((after - before - ANCHOR_LEARN_RATE * 0.5 * 100.0).abs() < 1e-3);
    }

    #[test]
    fn anchor_inert_when_disabled() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let i = id as usize;
        w.agents.position[i] = Vec2::new(600.0, 500.0);
        anchor_step(&mut w);
        assert_eq!(w.agents.anchor[i].x, 500.0);
    }

    #[test]
    fn market_field_deposits_and_decays() {
        let mut w = World::new(2);
        w.resources_enabled = true;
        w.market_field = vec![0.0; w.biome.cells.len()];
        market_deposit(&mut w, Vec2::new(500.0, 500.0));
        let (c, r) = w.biome.cell_coords(Vec2::new(500.0, 500.0));
        let idx = w.biome.cell_index(c, r);
        assert_eq!(w.market_field[idx], MARKET_DEPOSIT);
        market_decay_step(&mut w);
        assert!((w.market_field[idx] - MARKET_DEPOSIT * MARKET_DECAY).abs() < 1e-6);
    }

    #[test]
    fn experience_boosts_harvest() {
        assert_eq!(experienced_harvest(1.0, 0.0), 1.0);
        assert!((experienced_harvest(1.0, 5.0) - 1.5).abs() < 1e-6);
        // Cap: exp 100 behaves like EXP_CAP.
        assert!(
            (experienced_harvest(1.0, 100.0) - (1.0 + EXP_CAP * SPECIALIZATION_GAIN)).abs() < 1e-4
        );
    }
}
