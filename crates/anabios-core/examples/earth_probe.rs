//! THROWAWAY probe (sub-project B): diagnose why the real-Earth map collapses.
//! Prints the terrain histogram + mean carrying capacity for the real map vs.
//! the procedural map, and the terrain each founder geo-location sits on.
use anabios_core::biome::{BiomeField, EARTH_RES};
use glam::Vec2;

fn hist(f: &BiomeField, label: &str) {
    let mut counts = [0usize; 9];
    let mut cap = 0.0f64;
    for c in &f.cells {
        counts[c.terrain as usize] += 1;
        cap += c.terrain.carrying_capacity() as f64;
    }
    let n = f.cells.len() as f64;
    let names =
        ["Water", "Grass", "Forest", "Desert", "Rock", "Savanna", "Rainforest", "Taiga", "Tundra"];
    println!("--- {label}  ({} cells, res {}) ---", f.cells.len(), f.res);
    for i in 0..9 {
        println!("  {:10} {:6.1}%", names[i], 100.0 * counts[i] as f64 / n);
    }
    let land = n - counts[0] as f64;
    println!(
        "  mean cap / cell = {:.2} ; mean cap / LAND cell = {:.2}",
        cap / n,
        cap / land.max(1.0)
    );
}

fn founder(f: &BiomeField, name: &str, lat: f32, lon: f32) {
    let x = (lon + 180.0) / 360.0 * f.world_size;
    let y = (90.0 - lat) / 180.0 * f.world_size;
    let c = f.sample(Vec2::new(x, y));
    // local 5x5 mean capacity around the cell
    let cell_x = (x / f.cell_size) as i32;
    let cell_y = (y / f.cell_size) as i32;
    let mut local = 0.0f32;
    let mut nlocal = 0;
    for dy in -2..=2 {
        for dx in -2..=2 {
            let cx = (cell_x + dx).rem_euclid(f.res as i32) as usize;
            let cy = (cell_y + dy).rem_euclid(f.res as i32) as usize;
            local += f.cells[cy * f.res + cx].terrain.carrying_capacity();
            nlocal += 1;
        }
    }
    println!(
        "  {:22} lat{:>6.1} lon{:>6.1} -> {:?}  cap={:.1}  5x5_mean_cap={:.2}",
        name,
        lat,
        lon,
        c.terrain,
        c.terrain.carrying_capacity(),
        local / nlocal as f32
    );
}

/// Test the fix hypothesis: reconstruct approx real altitude from the current
/// normalized elevation (0 m->0.35, 8850 m->1.0), RE-normalize with a lower
/// ceiling so mountains exceed ROCK_LINE, re-classify, and report the new Rock%.
fn retune_rock(earth: &BiomeField, ceiling_m: f32) {
    use anabios_core::biome::{classify_with, TerrainType, SEA_LEVEL};
    let mut rock = 0usize;
    let mut cap = 0.0f64;
    let mut counts = [0usize; 9];
    for c in &earth.cells {
        // invert current normalization to approx metres (land only)
        let real_m = if c.elevation >= SEA_LEVEL {
            (c.elevation - SEA_LEVEL) / (1.0 - SEA_LEVEL) * 8850.0
        } else {
            -((SEA_LEVEL - c.elevation) / SEA_LEVEL) * 11000.0
        };
        // re-normalize with the lower ceiling
        let e = if real_m >= 0.0 {
            SEA_LEVEL + (real_m / ceiling_m) * (1.0 - SEA_LEVEL)
        } else {
            SEA_LEVEL * (1.0 + (real_m / 11000.0))
        }
        .clamp(0.0, 1.0);
        let t = classify_with(e, c.env, c.moisture, SEA_LEVEL);
        counts[t as usize] += 1;
        cap += t.carrying_capacity() as f64;
        if t == TerrainType::Rock {
            rock += 1;
        }
    }
    let n = earth.cells.len() as f64;
    println!(
        "  RETUNE ceiling={:.0}m -> Rock {:.1}%  mean_cap/cell {:.2}  (Taiga {:.1}%, Tundra {:.1}%)",
        ceiling_m,
        100.0 * rock as f64 / n,
        cap / n,
        100.0 * counts[7] as f64 / n,
        100.0 * counts[8] as f64 / n,
    );
}

fn main() {
    let earth = BiomeField::from_earth(EARTH_RES, 4096.0);
    let proc = BiomeField::generate(318, 256, 4096.0);
    hist(&earth, "REAL EARTH (from_earth)");
    hist(&proc, "PROCEDURAL (generate seed 318)");
    println!("\nFounder geo-locations on the REAL map:");
    // The scenario's actual placements.
    founder(&earth, "Cradle/foragers", 0.5, 37.0);
    founder(&earth, "Innovators (guess)", 0.5, 37.0);
    founder(&earth, "Sahel relay", 14.0, 10.0);
    founder(&earth, "Megafauna/herds", -2.5, 35.0);
    founder(&earth, "Rainforest commune", 1.0, 21.0);
    founder(&earth, "Cape", -33.0, 22.0);
    founder(&earth, "Eurasia NW", 50.0, 10.0);
    founder(&earth, "Eurasia Central", 45.0, 75.0);
    founder(&earth, "Eurasia NE", 55.0, 105.0);
    println!("\nFix hypothesis — does a lower elevation ceiling restore Rock/obsidian?");
    for ceiling in [4000.0, 3000.0, 2000.0] {
        retune_rock(&earth, ceiling);
    }
}
