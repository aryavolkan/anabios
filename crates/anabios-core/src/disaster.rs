//! Natural disasters (E4): fire, drought, freeze on a deterministic Poisson
//! schedule. Disasters mutate the biome over their duration and leave
//! succession scars (`BiomeCell.succession`); they have no direct agent
//! effects in E4.
//!
//! Everything runs behind `World.disasters_enabled`. All randomness comes
//! from the single world RNG stream, drawn in this fixed order per spawned
//! disaster: interval, kind, epicenter col, epicenter row, severity.

use serde::{Deserialize, Serialize};

use crate::biome::{TerrainType, SUCCESSION_BARE};
use crate::rng::Rng;
use crate::world::World;

/// Mean ticks between disasters (Poisson).
pub const DISASTER_MEAN_INTERVAL: u64 = 800;
/// Ticks a fire takes to grow to its full radius.
pub const FIRE_DURATION: u64 = 120;
/// Ticks a drought persists.
pub const DROUGHT_DURATION: u64 = 400;
/// Ticks a freeze persists.
pub const FREEZE_DURATION: u64 = 200;
/// Fire radius at severity 1.0, in cells.
pub const FIRE_MAX_RADIUS: f32 = 24.0;
/// Drought disk radius at severity 1.0, in cells.
pub const DROUGHT_MAX_RADIUS: f32 = 32.0;
/// Freeze disk radius at severity 1.0, in cells.
pub const FREEZE_MAX_RADIUS: f32 = 24.0;
/// Per-tick biomass multiplier inside a drought disk at severity 1.0.
pub const DROUGHT_DECAY: f32 = 0.004;
/// Per-tick biomass multiplier inside a freeze disk at severity 1.0.
pub const FREEZE_DECAY: f32 = 0.03;
/// Max simultaneously active disasters (overflow delays the schedule).
pub const ACTIVE_CAP: usize = 4;
/// Max remembered disaster sites (for succession tracking); oldest dropped.
pub const SITE_CAP: usize = 8;
/// Max scorched cells tracked per site.
pub const SITE_CELL_CAP: usize = 200;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisasterKind {
    Fire = 0,
    Drought = 1,
    Freeze = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveDisaster {
    pub kind: DisasterKind,
    /// Epicenter cell `(col, row)`.
    pub epicenter: (u16, u16),
    pub severity: f32,
    pub start_tick: u64,
    pub duration: u64,
    /// Full-effect radius in cells at severity.
    pub radius: f32,
}

/// A past fire's scar, tracked until enough of it returns to Climax.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisasterSite {
    pub epicenter: (u16, u16),
    /// Scorched cell indices (row * res + col), capped at `SITE_CELL_CAP`.
    pub cells: Vec<u32>,
    pub succession_fired: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DisasterState {
    pub next_tick: u64,
    pub active: Vec<ActiveDisaster>,
    pub sites: Vec<DisasterSite>,
    /// Total disasters spawned over the run (observability).
    pub spawned: u32,
}

impl DisasterState {
    /// Schedule the first disaster. Draw order: interval.
    pub fn init(rng: &mut Rng) -> Self {
        Self { next_tick: draw_interval(rng, 0), ..Default::default() }
    }
}

/// Next Poisson interval: `mean × -ln(1-u)`, u ∈ (0,1].
fn draw_interval(rng: &mut Rng, from_tick: u64) -> u64 {
    let u = 1.0 - rng.f32_unit();
    let dt = (DISASTER_MEAN_INTERVAL as f32 * -u.ln()).max(1.0);
    from_tick + dt as u64
}

/// Spawn one disaster. Draw order: kind, col, row, severity.
fn draw_disaster(rng: &mut Rng, res: usize, tick: u64) -> ActiveDisaster {
    let kind = match rng.index(3) {
        0 => DisasterKind::Fire,
        1 => DisasterKind::Drought,
        _ => DisasterKind::Freeze,
    };
    let col = rng.index(res) as u16;
    let row = rng.index(res) as u16;
    let severity = 0.3 + 0.7 * rng.f32_unit();
    let (duration, radius) = match kind {
        DisasterKind::Fire => (FIRE_DURATION, severity * FIRE_MAX_RADIUS),
        DisasterKind::Drought => (DROUGHT_DURATION, severity * DROUGHT_MAX_RADIUS),
        DisasterKind::Freeze => (FREEZE_DURATION, severity * FREEZE_MAX_RADIUS),
    };
    ActiveDisaster { kind, epicenter: (col, row), severity, start_tick: tick, duration, radius }
}

/// Per-tick disaster stage. No-op unless `world.disasters_enabled`.
/// Inserted into the tick pipeline after pheromone decay.
pub fn disaster_step(world: &mut World) {
    if !world.disasters_enabled {
        return;
    }
    let tick = world.tick;
    let res = world.biome.res;

    // Spawn due disasters (schedule overflow while at the active cap just
    // shifts the schedule right).
    while tick >= world.disasters.next_tick {
        if world.disasters.active.len() >= ACTIVE_CAP {
            world.disasters.next_tick = tick + 1;
            break;
        }
        let d = draw_disaster(&mut world.rng, res, tick);
        #[cfg(debug_assertions)]
        if std::env::var("ANABIOS_DISASTER_DEBUG").is_ok() {
            eprintln!(
                "[disaster] t={} spawned {:?} severity={:.2} at {:?}",
                tick, d.kind, d.severity, d.epicenter
            );
        }
        world.disasters.active.push(d);
        world.disasters.spawned += 1;
        world.disasters.next_tick = draw_interval(&mut world.rng, tick);
    }

    // Propagate active disasters; collect expired fires as new sites.
    let mut expired: Vec<ActiveDisaster> = Vec::new();
    let mut keep: Vec<ActiveDisaster> = Vec::new();
    for d in std::mem::take(&mut world.disasters.active) {
        let done = apply_disaster(world, &d, tick);
        if done {
            expired.push(d);
        } else {
            keep.push(d);
        }
    }
    world.disasters.active = keep;

    for d in expired {
        #[cfg(debug_assertions)]
        if std::env::var("ANABIOS_DISASTER_DEBUG").is_ok() {
            eprintln!(
                "[disaster] t={} expired {:?} severity={:.2} radius={:.1}",
                tick, d.kind, d.severity, d.radius
            );
        }
        if d.kind != DisasterKind::Fire {
            continue;
        }
        // Register the burn scar for succession tracking.
        let mut cells = Vec::new();
        for row in 0..res {
            for col in 0..res {
                if in_disk(&world.biome, (col, row), d.epicenter, d.radius)
                    && world.biome.at(col, row).succession == SUCCESSION_BARE
                    && cells.len() < SITE_CELL_CAP
                {
                    cells.push(world.biome.cell_index(col, row) as u32);
                }
            }
        }
        if !cells.is_empty() {
            let mut sites = std::mem::take(&mut world.disasters.sites);
            if sites.len() >= SITE_CAP {
                sites.remove(0);
            }
            sites.push(DisasterSite { epicenter: d.epicenter, cells, succession_fired: false });
            world.disasters.sites = sites;
        }
    }
}

/// Apply one tick of a disaster's effects. Returns true when it has expired.
fn apply_disaster(world: &mut World, d: &ActiveDisaster, tick: u64) -> bool {
    let age = tick.saturating_sub(d.start_tick);
    if age >= d.duration {
        return true;
    }
    let res = world.biome.res;
    match d.kind {
        DisasterKind::Fire => {
            // Ring grows linearly to the full radius over the duration.
            let r = d.radius * (age + 1) as f32 / d.duration as f32;
            for row in 0..res {
                for col in 0..res {
                    if !in_disk(&world.biome, (col, row), d.epicenter, r) {
                        continue;
                    }
                    let cell = world.biome.at_mut(col, row);
                    if cell.terrain.carrying_capacity() > 0.0 {
                        cell.plant_biomass = 0.0;
                        cell.succession = SUCCESSION_BARE;
                        if cell.terrain == TerrainType::Forest {
                            cell.terrain = TerrainType::Grass;
                        }
                    }
                }
            }
        }
        DisasterKind::Drought => {
            let mult = 1.0 - d.severity * DROUGHT_DECAY;
            for row in 0..res {
                for col in 0..res {
                    if !in_disk(&world.biome, (col, row), d.epicenter, d.radius) {
                        continue;
                    }
                    let cell = world.biome.at_mut(col, row);
                    cell.plant_biomass *= mult;
                }
            }
        }
        DisasterKind::Freeze => {
            let mult = 1.0 - d.severity * FREEZE_DECAY;
            for row in 0..res {
                for col in 0..res {
                    if !in_disk(&world.biome, (col, row), d.epicenter, d.radius) {
                        continue;
                    }
                    let cell = world.biome.at_mut(col, row);
                    cell.plant_biomass *= mult;
                }
            }
        }
    }
    false
}

/// Torus-aware disk test on cell coordinates.
fn in_disk(
    biome: &crate::biome::BiomeField,
    cell: (usize, usize),
    center: (u16, u16),
    radius: f32,
) -> bool {
    let res = biome.res as f32;
    let mut dc = (cell.0 as f32 - center.0 as f32).abs();
    let mut dr = (cell.1 as f32 - center.1 as f32).abs();
    if dc > res * 0.5 {
        dc = res - dc;
    }
    if dr > res * 0.5 {
        dr = res - dr;
    }
    (dc * dc + dr * dr).sqrt() <= radius
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::SUCCESSION_CLIMAX;

    fn world_with_disasters(seed: u64) -> World {
        let mut w = World::new(seed);
        w.disasters_enabled = true;
        w.disasters = DisasterState::init(&mut w.rng);
        w
    }

    #[test]
    fn schedule_is_deterministic() {
        let a = world_with_disasters(42);
        let b = world_with_disasters(42);
        assert_eq!(a.disasters.next_tick, b.disasters.next_tick);
        assert!(a.disasters.next_tick > 0);
    }

    #[test]
    fn fire_scorches_and_converts_forest() {
        let mut w = world_with_disasters(42);
        // Plant a forest cell and a grass cell at known spots.
        let (col, row) = (10_usize, 10_usize);
        w.biome.at_mut(col, row).terrain = TerrainType::Forest;
        w.biome.at_mut(col, row).plant_biomass = 20.0;
        w.biome.at_mut(col + 1, row).terrain = TerrainType::Grass;
        w.biome.at_mut(col + 1, row).plant_biomass = 10.0;
        let d = ActiveDisaster {
            kind: DisasterKind::Fire,
            epicenter: (col as u16, row as u16),
            severity: 1.0,
            start_tick: 0,
            duration: FIRE_DURATION,
            radius: 4.0,
        };
        w.disasters.active.push(d);
        w.disasters.next_tick = u64::MAX; // pin the scheduler off for the test
                                          // Run the full duration plus the expiring tick.
        for t in 0..=FIRE_DURATION {
            w.tick = t;
            disaster_step(&mut w);
        }
        let c = w.biome.at(col, row);
        assert_eq!(c.plant_biomass, 0.0);
        assert_eq!(c.succession, SUCCESSION_BARE);
        assert_eq!(c.terrain, TerrainType::Grass, "forest scorches to grass");
        assert_eq!(w.biome.at(col + 1, row).succession, SUCCESSION_BARE);
        // Expired and registered as a succession site.
        assert!(w.disasters.active.is_empty());
        assert_eq!(w.disasters.sites.len(), 1);
        assert!(!w.disasters.sites[0].cells.is_empty());
    }

    #[test]
    fn drought_decays_biomass_only_in_disk() {
        let mut w = world_with_disasters(7);
        let (col, row) = (20_usize, 20_usize);
        w.biome.at_mut(col, row).plant_biomass = 10.0;
        w.biome.at_mut(90, 90).plant_biomass = 10.0;
        let d = ActiveDisaster {
            kind: DisasterKind::Drought,
            epicenter: (col as u16, row as u16),
            severity: 1.0,
            start_tick: 0,
            duration: DROUGHT_DURATION,
            radius: 3.0,
        };
        w.disasters.active.push(d);
        w.disasters.next_tick = u64::MAX;
        w.tick = 0;
        disaster_step(&mut w);
        assert!((w.biome.at(col, row).plant_biomass - 10.0 * (1.0 - DROUGHT_DECAY)).abs() < 1e-4);
        assert_eq!(w.biome.at(90, 90).plant_biomass, 10.0, "outside the disk untouched");
        // Drought leaves no succession site.
        w.tick = DROUGHT_DURATION;
        disaster_step(&mut w);
        assert!(w.disasters.sites.is_empty());
    }

    #[test]
    fn untouched_world_stays_climax() {
        let w = world_with_disasters(1);
        assert!(w.biome.cells.iter().all(|c| c.succession == SUCCESSION_CLIMAX));
    }
}
