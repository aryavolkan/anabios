//! Foraging-selection observables. Pure functions of world state, zero RNG.
//! `forage_quality_gain` / `forage_fertility_gain` = (population-weighted mean
//! field value at live-agent positions) − (global mean field value over cells).
//! `> 0` and rising over generations ⇒ foragers concentrate in rich patches.

use crate::world::World;

fn global_mean(vals: impl Iterator<Item = f32>, n: usize) -> f32 {
    if n == 0 {
        return 0.0;
    }
    vals.sum::<f32>() / n as f32
}

/// Mean `nutrient_quality` under live agents minus the global cell mean.
pub fn forage_quality_gain(world: &World) -> f32 {
    let cells = &world.biome.cells;
    if cells.is_empty() {
        return 0.0;
    }
    let map_mean = global_mean(cells.iter().map(|c| c.nutrient_quality), cells.len());
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for id in world.agents.iter_alive() {
        sum += world.biome.sample(world.agents.position[id as usize]).nutrient_quality;
        n += 1;
    }
    if n == 0 {
        return 0.0;
    }
    sum / n as f32 - map_mean
}

/// Mean `fertility` under live agents minus the global cell mean.
pub fn forage_fertility_gain(world: &World) -> f32 {
    let cells = &world.biome.cells;
    if cells.is_empty() {
        return 0.0;
    }
    let map_mean = global_mean(cells.iter().map(|c| c.fertility), cells.len());
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for id in world.agents.iter_alive() {
        sum += world.biome.sample(world.agents.position[id as usize]).fertility;
        n += 1;
    }
    if n == 0 {
        return 0.0;
    }
    sum / n as f32 - map_mean
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Scenario;

    const MINIMAL: &str = include_str!("../../../scenarios/minimal.toml");

    #[test]
    fn uniform_field_gives_zero_gain() {
        let mut w = Scenario::parse_toml(MINIMAL).expect("parse").instantiate();
        for c in w.biome.cells.iter_mut() {
            c.nutrient_quality = 1.0;
        }
        assert_eq!(forage_quality_gain(&w), 0.0);
    }

    #[test]
    fn agents_on_rich_cells_give_positive_gain() {
        let mut w = Scenario::parse_toml(MINIMAL).expect("parse").instantiate();
        // Left half poor, right half rich (split on x).
        let half = w.biome.world_size / 2.0;
        for row in 0..w.biome.res {
            for col in 0..w.biome.res {
                let idx = row * w.biome.res + col;
                let x = col as f32 * w.biome.cell_size;
                w.biome.cells[idx].nutrient_quality = if x >= half { 1.4 } else { 0.6 };
            }
        }
        // Move every live agent onto the rich (right) half.
        let ids: Vec<_> = w.agents.iter_alive().collect();
        for id in ids {
            w.agents.position[id as usize].x = half + 1.0;
        }
        assert!(forage_quality_gain(&w) > 0.0);
    }
}
