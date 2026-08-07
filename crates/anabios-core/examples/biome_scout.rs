//! View a world's climate-driven biomes and find cluster centers for each.
//!
//! Terrain is now a climate-driven Whittaker classification (elevation ×
//! temperature × moisture; see `biome.rs`), so subtropical Desert and an
//! equatorial Rainforest belt appear in EVERY seed by construction — seed
//! scouting is no longer needed to get "deserts and a tropical biome". This
//! example still ranks seeds by how much Desert + Rainforest they carry, then
//! renders the best one and reports the densest Desert and Rainforest patch
//! centers (handy as scenario cluster centers).
//!
//! Run: `cargo run -p anabios-core --example biome_scout`
//!      `cargo run -p anabios-core --example biome_scout -- 200`
//!   args: [seeds_to_scan]

use anabios_core::biome::{
    BiomeCell, BiomeField, ClimateParams, TerrainType, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT,
};
use std::io::Write;

/// Half-width (in cells) of the window used to find each biome's densest patch.
/// At the default 128-grid / 1024-world this is a ~64-unit radius, matching a
/// scenario's cluster radius.
const WINDOW: usize = 8;

struct Score {
    seed: u64,
    desert_frac: f32,
    tropical_frac: f32,
    /// Combined objective: reward worlds rich in BOTH biomes. The `min` term
    /// forces both to be present; the sum breaks ties toward generous coverage.
    objective: f32,
}

fn score_seed(seed: u64) -> Score {
    let field = BiomeField::generate(seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
    let total = field.cells.len() as f32;
    let mut desert = 0.0;
    let mut tropical = 0.0;
    for c in &field.cells {
        match c.terrain {
            TerrainType::Desert => desert += 1.0,
            TerrainType::Rainforest => tropical += 1.0,
            _ => {}
        }
    }
    let desert_frac = desert / total;
    let tropical_frac = tropical / total;
    let objective = desert_frac.min(tropical_frac) * 4.0 + (desert_frac + tropical_frac);
    Score { seed, desert_frac, tropical_frac, objective }
}

/// Find the cell whose surrounding `+/-window` block contains the most cells
/// matching `pred`. Returns `(col, row, hits)` of that densest window center.
fn densest_patch(
    field: &BiomeField,
    window: usize,
    pred: impl Fn(&BiomeCell) -> bool,
) -> (usize, usize, usize) {
    let res = field.res;
    let (mut best_col, mut best_row, mut best_hits) = (0, 0, 0);
    for row in 0..res {
        for col in 0..res {
            if !pred(field.at(col, row)) {
                continue;
            }
            let mut hits = 0;
            for dr in row.saturating_sub(window)..=(row + window).min(res - 1) {
                for dc in col.saturating_sub(window)..=(col + window).min(res - 1) {
                    if pred(field.at(dc, dr)) {
                        hits += 1;
                    }
                }
            }
            if hits > best_hits {
                best_hits = hits;
                best_col = col;
                best_row = row;
            }
        }
    }
    (best_col, best_row, best_hits)
}

fn terrain_rgb(t: TerrainType) -> (u8, u8, u8) {
    match t {
        TerrainType::Water => (23, 49, 112),
        TerrainType::Grass => (54, 112, 48),
        TerrainType::Forest => (18, 66, 28),
        TerrainType::Desert => (173, 148, 84),
        TerrainType::Rock => (107, 102, 115),
        TerrainType::Savanna => (184, 168, 92),
        TerrainType::Rainforest => (15, 87, 41),
        TerrainType::Taiga => (41, 87, 66),
        TerrainType::Tundra => (158, 168, 158),
    }
}

/// Write a P6 PPM: terrain color, dimmed/brightened by elevation, rivers blue.
fn write_ppm(path: &str, f: &BiomeField) -> std::io::Result<()> {
    let res = f.res;
    let mut buf = Vec::with_capacity(res * res * 3 + 32);
    write!(buf, "P6\n{res} {res}\n255\n")?;
    for c in &f.cells {
        let (mut r, mut g, mut b) = terrain_rgb(c.terrain);
        // Relief shading: scale by 0.6..1.15 with elevation.
        let s = 0.6 + 0.55 * c.elevation;
        r = (r as f32 * s).min(255.0) as u8;
        g = (g as f32 * s).min(255.0) as u8;
        b = (b as f32 * s).min(255.0) as u8;
        if c.river_flow > 0.0 {
            r = 40;
            g = 90;
            b = 200;
        }
        buf.extend_from_slice(&[r, g, b]);
    }
    std::fs::write(path, buf)
}

fn terrain_char(t: TerrainType) -> char {
    match t {
        TerrainType::Water => '~',
        TerrainType::Desert => '.',
        TerrainType::Grass => '"',
        TerrainType::Forest => 'f',
        TerrainType::Rock => '^',
        TerrainType::Savanna => 'S',
        TerrainType::Rainforest => 'T',
        TerrainType::Taiga => 't',
        TerrainType::Tundra => 'u',
    }
}

/// Render a coarse ASCII map by sampling the biome grid on a fixed canvas.
fn ascii_map(field: &BiomeField, cols: usize, rows: usize) {
    let res = field.res;
    for r in 0..rows {
        let mut line = String::with_capacity(cols);
        let row = r * res / rows;
        for c in 0..cols {
            let col = c * res / cols;
            line.push(terrain_char(field.at(col, row).terrain));
        }
        println!("{line}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--ppm") {
        let path = args
            .get(pos + 1)
            .filter(|a| !a.starts_with("--"))
            .cloned()
            .unwrap_or_else(|| "world.ppm".into());
        let seed: u64 = args
            .iter()
            .position(|a| a == "--seed")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(7);
        let climate = ClimateParams {
            continentality: 0.85,
            mountain_uplift: 0.6,
            rain_shadow: 0.4,
            river_threshold: 150.0,
            ..Default::default()
        };
        let field = BiomeField::generate_with(seed, 512, 4096.0, &climate);
        write_ppm(&path, &field).expect("write ppm");
        eprintln!("wrote {path}");
        return;
    }

    let scan: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(64);

    let mut scores: Vec<Score> = (0..scan.max(1)).map(score_seed).collect();
    scores.sort_by(|a, b| b.objective.partial_cmp(&a.objective).unwrap());

    println!("Scanned seeds 0..{scan} at {BIOME_RES_DEFAULT}x{BIOME_RES_DEFAULT}\n");
    println!("Top 10 worlds by Desert + Rainforest coverage:");
    println!("  {:>6}  {:>8}  {:>10}", "seed", "desert%", "tropical%");
    for s in scores.iter().take(10) {
        println!(
            "  {:>6}  {:>7.1}%  {:>9.1}%",
            s.seed,
            s.desert_frac * 100.0,
            s.tropical_frac * 100.0
        );
    }

    let best = &scores[0];
    println!(
        "\nBest seed = {}  (desert {:.1}%, tropical {:.1}%)\n",
        best.seed,
        best.desert_frac * 100.0,
        best.tropical_frac * 100.0,
    );

    let field = BiomeField::generate(best.seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
    let (dcol, drow, dhits) = densest_patch(&field, WINDOW, |c| c.terrain == TerrainType::Desert);
    let (tcol, trow, thits) =
        densest_patch(&field, WINDOW, |c| c.terrain == TerrainType::Rainforest);
    let center = |col: usize, row: usize| {
        ((col as f32 + 0.5) * field.cell_size, (row as f32 + 0.5) * field.cell_size)
    };
    let win_cells = ((2 * WINDOW + 1) * (2 * WINDOW + 1)) as f32;
    if dhits > 0 {
        let (cx, cy) = center(dcol, drow);
        println!(
            "Desert cluster   ~ ({cx:.0}, {cy:.0})  {:.0}% desert within +/-{WINDOW} cells  [desert cohort]",
            dhits as f32 / win_cells * 100.0
        );
    }
    if thits > 0 {
        let (cx, cy) = center(tcol, trow);
        println!(
            "Tropical cluster ~ ({cx:.0}, {cy:.0})  {:.0}% rainforest within +/-{WINDOW} cells  [tropical cohort]",
            thits as f32 / win_cells * 100.0
        );
    }

    println!(
        "\nLegend: ~ water  . desert  S savanna  \" grass  f forest  T rainforest  t taiga  u tundra  ^ rock\n"
    );
    ascii_map(&field, 96, 40);
}
