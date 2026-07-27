//! Scout for a world seed that contains both arid deserts and a warm tropical
//! zone, then render it as an ASCII map.
//!
//! Terrain (Water/Desert/Grass/Forest/Rock) is a deterministic function of a
//! world's seed alone — there is no per-scenario knob for biome placement — so
//! "generate a world with deserts and a tropical biome" reduces to searching
//! seeds for one whose map has substantial Desert terrain AND a tropical band:
//! Forest terrain (the wettest, most productive terrain) sitting in a warm,
//! high-`env` climate cell. This example scores every seed in a range on both
//! and prints the winner.
//!
//! Run: `cargo run -p anabios-core --example biome_scout`
//!      `cargo run -p anabios-core --example biome_scout -- 200 0.5`
//!   args: [seeds_to_scan] [tropical_env_threshold]

use anabios_core::biome::{BiomeField, TerrainType, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT};

/// A cell counts as "tropical" when it is Forest terrain (wet, high carrying
/// capacity) sitting in a warm climate — `env` at or above this threshold.
const DEFAULT_TROPICAL_ENV: f32 = 0.55;

/// Half-width (in cells) of the window used to find each biome's densest patch.
/// At the default 128-grid / 1024-world this is a ~64-unit radius, matching the
/// scenario's cluster radius.
const WINDOW: usize = 8;

struct Score {
    seed: u64,
    desert_frac: f32,
    tropical_frac: f32,
    forest_frac: f32,
    /// Combined objective: reward worlds that are rich in BOTH biomes. The
    /// `min` term forces both to be present; the sum breaks ties toward
    /// generous coverage.
    objective: f32,
}

fn score_seed(seed: u64, tropical_env: f32) -> Score {
    let field = BiomeField::generate(seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
    let total = field.cells.len() as f32;
    let mut desert = 0.0;
    let mut forest = 0.0;
    let mut tropical = 0.0;
    for c in &field.cells {
        match c.terrain {
            TerrainType::Desert => desert += 1.0,
            TerrainType::Forest => {
                forest += 1.0;
                if c.env >= tropical_env {
                    tropical += 1.0;
                }
            }
            _ => {}
        }
    }
    let desert_frac = desert / total;
    let tropical_frac = tropical / total;
    let forest_frac = forest / total;
    let objective = desert_frac.min(tropical_frac) * 4.0 + (desert_frac + tropical_frac);
    Score { seed, desert_frac, tropical_frac, forest_frac, objective }
}

/// Find the cell whose surrounding `+/-window` block contains the most cells
/// matching `pred`. Returns `(col, row, hits)` of that densest window center.
fn densest_patch(
    field: &BiomeField,
    window: usize,
    pred: impl Fn(&anabios_core::biome::BiomeCell) -> bool,
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

/// Render a coarse ASCII map by sampling the biome grid on a fixed character
/// canvas. `~` water, `.` desert, `"` grass, `T` tropical forest, `f` other
/// forest, `^` rock.
fn ascii_map(seed: u64, tropical_env: f32, cols: usize, rows: usize) {
    let field = BiomeField::generate(seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
    let res = field.res;
    for r in 0..rows {
        let mut line = String::with_capacity(cols);
        let row = r * res / rows;
        for c in 0..cols {
            let col = c * res / cols;
            let cell = field.at(col, row);
            let ch = match cell.terrain {
                TerrainType::Water => '~',
                TerrainType::Desert => '.',
                TerrainType::Grass => '"',
                TerrainType::Forest => {
                    if cell.env >= tropical_env {
                        'T'
                    } else {
                        'f'
                    }
                }
                TerrainType::Rock => '^',
            };
            line.push(ch);
        }
        println!("{line}");
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let scan: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(256);
    let tropical_env: f32 =
        args.next().and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_TROPICAL_ENV);

    let mut scores: Vec<Score> = (0..scan).map(|s| score_seed(s, tropical_env)).collect();
    scores.sort_by(|a, b| b.objective.partial_cmp(&a.objective).unwrap());

    println!(
        "Scanned seeds 0..{scan} at {BIOME_RES_DEFAULT}x{BIOME_RES_DEFAULT}, tropical = Forest with env >= {tropical_env:.2}\n"
    );
    println!("Top 10 worlds (desert + tropical):");
    println!("  {:>6}  {:>8}  {:>10}  {:>8}", "seed", "desert%", "tropical%", "forest%");
    for s in scores.iter().take(10) {
        println!(
            "  {:>6}  {:>7.1}%  {:>9.1}%  {:>7.1}%",
            s.seed,
            s.desert_frac * 100.0,
            s.tropical_frac * 100.0,
            s.forest_frac * 100.0,
        );
    }

    let best = &scores[0];
    println!(
        "\nBest seed = {}  (desert {:.1}%, tropical {:.1}%, forest {:.1}%)\n",
        best.seed,
        best.desert_frac * 100.0,
        best.tropical_frac * 100.0,
        best.forest_frac * 100.0,
    );

    // Cluster centers (world coordinates) for each biome. The arithmetic mean
    // of scattered cells lands in a gap between patches, so instead pick the
    // cell whose surrounding window is densest in the target biome — a point
    // that actually sits inside a real patch.
    let field = BiomeField::generate(best.seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
    let (dcol, drow, dhits) = densest_patch(&field, WINDOW, |c| c.terrain == TerrainType::Desert);
    let (tcol, trow, thits) = densest_patch(&field, WINDOW, |c| {
        c.terrain == TerrainType::Forest && c.env >= tropical_env
    });
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
            "Tropical cluster ~ ({cx:.0}, {cy:.0})  {:.0}% tropical within +/-{WINDOW} cells  [tropical cohort]",
            thits as f32 / win_cells * 100.0
        );
    }

    println!("\nLegend: ~ water   . desert   \" grass   T tropical forest   f forest   ^ rock\n");
    ascii_map(best.seed, tropical_env, 96, 40);
}
