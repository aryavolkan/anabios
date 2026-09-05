//! Show how `river_threshold` interacts with `biome_res`.
//!
//! `carve_rivers` thresholds a raw flow accumulation counted in *upstream
//! cells*, so the same number means different things at different grid
//! resolutions: a threshold tuned at `biome_res = 512` erases the river
//! network entirely at 128. That is a real footgun for anyone copying a
//! `[climate]` block between scenarios of different scale, and it is why the
//! large-world scenarios pin threshold and resolution together.
//!
//! Run: `cargo run --release -p anabios-core --example river_scaling`

use anabios_core::biome::{BiomeField, ClimateParams, TerrainType};
use anabios_core::needs::RIVER_DRINK_MIN;

fn main() {
    println!("seed 7, continental climate — river cells are those agents can drink from");
    println!("{:>6}  {:>6}  {:>8}  {:>8}  {:>8}", "res", "thresh", "river", "river%", "forage%");
    for (res, world_size) in [(128usize, 1024.0f32), (256, 2048.0), (512, 4096.0)] {
        for threshold in [20.0f32, 50.0, 100.0, 150.0, 300.0] {
            let cfg = ClimateParams {
                continentality: 0.85,
                mountain_uplift: 0.6,
                rain_shadow: 0.4,
                river_threshold: threshold,
                sea_level: 0.45,
                ..Default::default()
            };
            let field = BiomeField::generate_with(7, res, world_size, &cfg);
            let cells = field.cells.len() as f32;
            let river = field.cells.iter().filter(|c| c.river_flow >= RIVER_DRINK_MIN).count();
            let forage = field.cells.iter().filter(|c| c.terrain.carrying_capacity() > 0.0).count();
            println!(
                "{res:>6}  {threshold:>6.0}  {river:>8}  {:>7.2}%  {:>7.1}%",
                river as f32 / cells * 100.0,
                forage as f32 / cells * 100.0
            );
        }
        println!();
    }
    // Sanity line: how much of the map is open ocean at this sea level.
    let cfg = ClimateParams { sea_level: 0.45, continentality: 0.85, ..Default::default() };
    let field = BiomeField::generate_with(7, 256, 2048.0, &cfg);
    let water = field.cells.iter().filter(|c| c.terrain == TerrainType::Water).count();
    println!(
        "open water at sea_level 0.45: {:.1}%",
        water as f32 / field.cells.len() as f32 * 100.0
    );
}
