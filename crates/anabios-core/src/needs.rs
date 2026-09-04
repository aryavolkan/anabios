//! Basic needs (thirst + sleep): the Layer-0 drive-vector widening reserved
//! by the affect spec (§3.2). Hunger already exists as `energy` (+ the affect
//! layer's `homeostatic_drive`); this module adds the two missing drives as
//! per-agent accumulators with consummatory behaviors:
//!
//! - **Thirst** rises each tick (faster while moving) and falls while the
//!   agent stands at a drinkable cell (Water terrain or a river cell).
//!   Dehydration never kills directly — it multiplies basal metabolic drain
//!   in `integrate_all`, so death arrives through the existing starvation
//!   path and the energy+biomass conservation invariant is untouched
//!   (drinking never *adds* energy). Thirsty agents get an additive
//!   water-seeking movement bias in `decide_all`.
//! - **Fatigue** rises with activity and forces sleep on a hysteresis
//!   (`SLEEP_ONSET`/`WAKE_AT`). While asleep: movement is suppressed and
//!   basal metabolism discounted (`integrate_all`), feeding is skipped
//!   (`feed_pass`), and fatigue recovers. The cost of sleep is lost foraging
//!   time; sleeping agents remain attackable.
//!
//! Everything is gated on `World::basic_needs_enabled`: with the flag off the
//! tick stage early-returns with zero state change and zero RNG draws, the
//! columns stay `0.0`/false, and every read-side hook is exact identity —
//! flag-off worlds are byte-identical (layout growth only).

use crate::agent::AgentBuffers;
use crate::biome::{BiomeField, TerrainType};
use crate::genome::GenomeSlot;
use crate::prelude::Vec2;
use crate::world::World;

/// Per-tick thirst gain while standing still, at neutral `ThirstTolerance`.
pub const THIRST_RATE_BASE: f32 = 0.0008;
/// Additional thirst gain per world-unit moved this tick.
pub const THIRST_RATE_MOVE: f32 = 0.0002;
/// Per-tick thirst loss while at a drinkable cell (~20 ticks to rehydrate).
pub const DRINK_RATE: f32 = 0.05;
/// `BiomeCell.river_flow` at/above this counts as drinkable (rivers exist
/// only when a scenario's `river_threshold > 0`; non-river cells hold 0.0).
pub const RIVER_DRINK_MIN: f32 = 0.05;
/// Dehydration severity: basal metabolism is multiplied by
/// `1 + DEHYDRATION_DRAIN × thirst²` — exactly 1.0 at thirst 0.
pub const DEHYDRATION_DRAIN: f32 = 3.0;
/// Thirst level above which the water-seeking movement bias engages.
pub const WATER_SEEK_MIN: f32 = 0.35;
/// Scan radius (world units) for `best_water_direction`.
pub const WATER_SEEK_REACH: f32 = 96.0;
/// Strength of the water-seeking movement bias (scaled by thirst).
pub const WATER_PULL: f32 = 2.0;
/// Per-tick fatigue gain while awake and still, at neutral `SleepNeed`.
pub const FATIGUE_RATE_BASE: f32 = 0.0015;
/// Additional fatigue gain per world-unit moved this tick.
pub const FATIGUE_RATE_MOVE: f32 = 0.0003;
/// Per-tick fatigue recovery while asleep (0.9 → 0.2 in ~70 ticks).
pub const FATIGUE_RECOVERY: f32 = 0.01;
/// Fatigue at/above which an awake agent falls asleep.
pub const SLEEP_ONSET: f32 = 0.9;
/// Fatigue at/below which a sleeping agent wakes (hysteresis).
pub const WAKE_AT: f32 = 0.2;
/// Basal-metabolism discount while asleep.
pub const SLEEP_METABOLISM_FACTOR: f32 = 0.6;

/// Read-side hook for `integrate_all`: dehydration multiplies basal drain.
/// Exactly `1.0` at `thirst == 0.0` (the flag-off state), so disabled worlds
/// stay byte-identical.
#[inline]
pub fn dehydration_metabolism_multiplier(thirst: f32) -> f32 {
    1.0 + DEHYDRATION_DRAIN * thirst * thirst
}

/// `true` iff the cell at (col, row) can be drunk from: open water, or a
/// river cell carved by the hydrology pass.
#[inline]
pub fn drinkable_cell(biome: &BiomeField, col: usize, row: usize) -> bool {
    let c = biome.at(col, row);
    c.terrain == TerrainType::Water || c.river_flow >= RIVER_DRINK_MIN
}

/// `true` iff the agent at `pos` can drink: its own cell or a 4-neighbour is
/// drinkable (shoreline drinking — agents need not stand in the water).
pub fn drinkable_near(biome: &BiomeField, pos: Vec2) -> bool {
    let (cx, cy) = biome.cell_coords(pos);
    const OFFS: [(i32, i32); 5] = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)];
    OFFS.iter().any(|&(dx, dy)| {
        let col = ((cx as i32 + dx).rem_euclid(biome.res as i32)) as usize;
        let row = ((cy as i32 + dy).rem_euclid(biome.res as i32)) as usize;
        drinkable_cell(biome, col, row)
    })
}

/// Unit direction (torus-wrapped) toward the nearest drinkable cell within
/// `radius`, or `Vec2::ZERO` when none is in reach (or the agent is already
/// on one). Deterministic: strict `<` keeps the earliest (lowest dy, dx)
/// candidate on ties, mirroring `biome::best_terrain_direction`.
pub fn best_water_direction(biome: &BiomeField, pos: Vec2, radius: f32) -> Vec2 {
    let cell_reach = (radius / biome.cell_size).ceil() as i32 + 1;
    let (cx, cy) = biome.cell_coords(pos);
    let mut best: Option<(f32, Vec2)> = None;
    for dy in -cell_reach..=cell_reach {
        for dx in -cell_reach..=cell_reach {
            let col = ((cx as i32 + dx).rem_euclid(biome.res as i32)) as usize;
            let row = ((cy as i32 + dy).rem_euclid(biome.res as i32)) as usize;
            if !drinkable_cell(biome, col, row) {
                continue;
            }
            let cell_center = Vec2::new(
                (col as f32 + 0.5) * biome.cell_size,
                (row as f32 + 0.5) * biome.cell_size,
            );
            let offset = crate::prelude::wrap_torus(
                cell_center - pos + Vec2::splat(biome.world_size * 0.5),
                Vec2::splat(biome.world_size),
            ) - Vec2::splat(biome.world_size * 0.5);
            let d2 = offset.length_squared();
            if d2 > radius * radius {
                continue;
            }
            if best.is_none_or(|(bd, _)| d2 < bd) {
                best = Some((d2, offset));
            }
        }
    }
    best.map(|(_, off)| off.normalize_or_zero()).unwrap_or(Vec2::ZERO)
}

/// Tick stage (directly after `integrate_all`, so `velocity` is this tick's):
/// accumulate thirst/fatigue per agent, drink at water, and run the sleep
/// hysteresis. Serial ascending-id loop over each agent's own columns —
/// deterministic, RNG-free. Strict no-op when the flag is off.
pub fn needs_step(world: &mut World) {
    if !world.basic_needs_enabled {
        return;
    }
    let biome = &world.biome;
    let AgentBuffers { position, velocity, genome, thirst, fatigue, asleep, alive, .. } =
        &mut world.agents;
    for i in alive.iter_ones() {
        let speed = velocity[i].length();
        if drinkable_near(biome, position[i]) {
            thirst[i] = (thirst[i] - DRINK_RATE).max(0.0);
        } else {
            let tol = genome[i].get(GenomeSlot::ThirstTolerance);
            let gain = (THIRST_RATE_BASE + THIRST_RATE_MOVE * speed) * (1.5 - tol);
            thirst[i] = (thirst[i] + gain).min(1.0);
        }
        if asleep[i] {
            fatigue[i] = (fatigue[i] - FATIGUE_RECOVERY).max(0.0);
            if fatigue[i] <= WAKE_AT {
                asleep.set(i, false);
            }
        } else {
            let need = genome[i].get(GenomeSlot::SleepNeed);
            let gain = (FATIGUE_RATE_BASE + FATIGUE_RATE_MOVE * speed) * (0.5 + need);
            fatigue[i] = (fatigue[i] + gain).min(1.0);
            if fatigue[i] >= SLEEP_ONSET {
                asleep.set(i, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;

    /// A world with the flag on and one agent at (500, 500); the agent's cell
    /// terrain is forced as requested so the test doesn't depend on worldgen.
    fn needs_world(terrain: TerrainType) -> (World, crate::agent::AgentId) {
        let mut w = World::new(7);
        w.basic_needs_enabled = true;
        let pos = Vec2::new(500.0, 500.0);
        let id = w.spawn_agent(pos, Genome::neutral());
        let (cx, cy) = w.biome.cell_coords(pos);
        // Force the agent's cell AND its 4-neighbours (drinkable_near checks
        // them) to the requested terrain.
        for (dx, dy) in [(0i32, 0i32), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let col = ((cx as i32 + dx).rem_euclid(w.biome.res as i32)) as usize;
            let row = ((cy as i32 + dy).rem_euclid(w.biome.res as i32)) as usize;
            w.biome.at_mut(col, row).terrain = terrain;
            w.biome.at_mut(col, row).river_flow = 0.0;
        }
        (w, id)
    }

    #[test]
    fn flag_off_is_inert() {
        let (mut w, id) = needs_world(TerrainType::Grass);
        w.basic_needs_enabled = false;
        let hash_before = crate::snapshot::state_hash(&w);
        needs_step(&mut w);
        assert_eq!(w.agents.thirst[id as usize], 0.0, "flag off: no thirst written");
        assert_eq!(
            crate::snapshot::state_hash(&w),
            hash_before,
            "flag off: zero state change, zero RNG draws"
        );
    }

    #[test]
    fn thirst_accumulates_on_dry_land_and_drinking_restores() {
        let (mut w, id) = needs_world(TerrainType::Grass);
        let i = id as usize;
        needs_step(&mut w);
        // Neutral ThirstTolerance (0.5) ⇒ rate scale exactly ×1.0; still ⇒ base rate.
        assert!(
            (w.agents.thirst[i] - THIRST_RATE_BASE).abs() < 1e-7,
            "one still tick gains the base rate, got {}",
            w.agents.thirst[i]
        );
        // Parch the agent, then stand it at water: thirst falls by DRINK_RATE.
        w.agents.thirst[i] = 0.8;
        let (cx, cy) = w.biome.cell_coords(w.agents.position[i]);
        w.biome.at_mut(cx, cy).terrain = TerrainType::Water;
        needs_step(&mut w);
        assert!(
            (w.agents.thirst[i] - (0.8 - DRINK_RATE)).abs() < 1e-6,
            "drinking reduces thirst by DRINK_RATE, got {}",
            w.agents.thirst[i]
        );
    }

    #[test]
    fn river_cells_are_drinkable() {
        let (mut w, id) = needs_world(TerrainType::Grass);
        let i = id as usize;
        w.agents.thirst[i] = 0.5;
        let (cx, cy) = w.biome.cell_coords(w.agents.position[i]);
        w.biome.at_mut(cx, cy).river_flow = RIVER_DRINK_MIN;
        needs_step(&mut w);
        assert!(w.agents.thirst[i] < 0.5, "a river cell quenches thirst");
    }

    #[test]
    fn movement_speeds_thirst_and_fatigue() {
        let (mut w, id) = needs_world(TerrainType::Grass);
        let i = id as usize;
        w.agents.velocity[i] = Vec2::new(4.0, 0.0);
        needs_step(&mut w);
        let expected_thirst = THIRST_RATE_BASE + THIRST_RATE_MOVE * 4.0;
        let expected_fatigue = FATIGUE_RATE_BASE + FATIGUE_RATE_MOVE * 4.0;
        assert!((w.agents.thirst[i] - expected_thirst).abs() < 1e-7);
        assert!((w.agents.fatigue[i] - expected_fatigue).abs() < 1e-7);
    }

    #[test]
    fn sleep_hysteresis_onset_and_wake() {
        let (mut w, id) = needs_world(TerrainType::Grass);
        let i = id as usize;
        // Push fatigue to the onset threshold: the agent falls asleep.
        w.agents.fatigue[i] = SLEEP_ONSET;
        needs_step(&mut w);
        assert!(w.agents.asleep[i], "fatigue at onset ⇒ asleep");
        // Asleep: fatigue recovers each tick; stays asleep above WAKE_AT.
        let before = w.agents.fatigue[i];
        needs_step(&mut w);
        assert!(w.agents.fatigue[i] < before, "asleep ⇒ fatigue recovers");
        assert!(w.agents.asleep[i], "still above WAKE_AT ⇒ still asleep");
        // Drop to the wake threshold: the agent wakes.
        w.agents.fatigue[i] = WAKE_AT + FATIGUE_RECOVERY * 0.5;
        needs_step(&mut w);
        assert!(!w.agents.asleep[i], "fatigue recovered to WAKE_AT ⇒ awake");
    }

    #[test]
    fn dehydration_multiplier_is_identity_at_zero_and_monotonic() {
        assert_eq!(dehydration_metabolism_multiplier(0.0), 1.0, "exact identity at 0");
        let m_half = dehydration_metabolism_multiplier(0.5);
        let m_full = dehydration_metabolism_multiplier(1.0);
        assert!(m_half > 1.0 && m_full > m_half, "monotonic in thirst");
        assert!((m_full - (1.0 + DEHYDRATION_DRAIN)).abs() < 1e-6);
    }

    #[test]
    fn best_water_direction_points_at_forced_water() {
        let (mut w, id) = needs_world(TerrainType::Grass);
        let pos = w.agents.position[id as usize];
        let (cx, cy) = w.biome.cell_coords(pos);
        // Dry out everything in reach, then place one water cell 5 columns east.
        let reach_cells = (WATER_SEEK_REACH / w.biome.cell_size).ceil() as i32 + 1;
        for dy in -reach_cells..=reach_cells {
            for dx in -reach_cells..=reach_cells {
                let col = ((cx as i32 + dx).rem_euclid(w.biome.res as i32)) as usize;
                let row = ((cy as i32 + dy).rem_euclid(w.biome.res as i32)) as usize;
                w.biome.at_mut(col, row).terrain = TerrainType::Grass;
                w.biome.at_mut(col, row).river_flow = 0.0;
            }
        }
        let wcol = ((cx as i32 + 5).rem_euclid(w.biome.res as i32)) as usize;
        w.biome.at_mut(wcol, cy).terrain = TerrainType::Water;
        let dir = best_water_direction(&w.biome, pos, WATER_SEEK_REACH);
        assert!(dir.x > 0.9, "direction points east toward the water, got {dir:?}");
        assert!(dir.y.abs() < 0.3, "mostly-horizontal pull, got {dir:?}");
    }
}
