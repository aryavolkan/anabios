//! Probe terrain composition around candidate cluster sites for a given seed.
//!
//! Run: `cargo run -p anabios-core --example site_probe -- <seed> <x> <y> <r> [<x> <y> <r> ...]`

use anabios_core::biome::{BiomeField, TerrainType, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT};
use glam::Vec2;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seed: u64 = args[0].parse().unwrap();
    let field = BiomeField::generate(seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);

    for site in args[1..].chunks(3) {
        let (cx, cy, r): (f32, f32, f32) =
            (site[0].parse().unwrap(), site[1].parse().unwrap(), site[2].parse().unwrap());
        let mut counts = [0usize; 9];
        let mut env_sum = 0.0f32;
        let mut n = 0usize;
        // Sample on a 4-unit grid over the disc (torus-aware).
        let steps = (r / 4.0) as i32;
        for dy in -steps..=steps {
            for dx in -steps..=steps {
                let (ox, oy) = (dx as f32 * 4.0, dy as f32 * 4.0);
                if ox * ox + oy * oy > r * r {
                    continue;
                }
                let pos = Vec2::new(
                    (cx + ox).rem_euclid(WORLD_SIZE_DEFAULT),
                    (cy + oy).rem_euclid(WORLD_SIZE_DEFAULT),
                );
                let cell = field.sample(pos);
                counts[cell.terrain as usize] += 1;
                env_sum += cell.env;
                n += 1;
            }
        }
        let pct = |t: TerrainType| counts[t as usize] as f32 / n as f32 * 100.0;
        println!(
            "({cx:>4.0},{cy:>4.0}) r={r:>3.0}: water {:>4.1}%  desert {:>4.1}%  grass {:>4.1}%  forest {:>4.1}%  rock {:>4.1}%  savanna {:>4.1}%  rainforest {:>4.1}%  taiga {:>4.1}%  tundra {:>4.1}%   env {:.2}",
            pct(TerrainType::Water),
            pct(TerrainType::Desert),
            pct(TerrainType::Grass),
            pct(TerrainType::Forest),
            pct(TerrainType::Rock),
            pct(TerrainType::Savanna),
            pct(TerrainType::Rainforest),
            pct(TerrainType::Taiga),
            pct(TerrainType::Tundra),
            env_sum / n as f32,
        );
    }
}
