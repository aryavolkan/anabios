//! Scout for a world seed with "Out of Africa" geography under the
//! climate-driven worldgen: a habitable equatorial/tropical homeland, a
//! water/rock barrier band separating it from a large temperate landmass,
//! and at least one narrow land bridge between them.
//!
//! Water and Rock are the only zero-capacity terrains and the only terrains
//! the CorridorUse codex detector counts as corridor barriers
//! (codex/population.rs), so the exodus reads best when the band is built
//! from them. Desert/Tundra stay habitable-but-harsh crossing terrain.
//!
//! Run: `cargo run -p anabios-core --example africa_scout -- [seeds_to_scan]`

use anabios_core::biome::{BiomeField, TerrainType, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT};

// Barrier-band height in cells. 12 cells x 8 world units = a ~96-unit-wide
// strait/isthmus region to push through.
const BAND_H: usize = 12;

fn is_barrier(t: TerrainType) -> bool {
    matches!(t, TerrainType::Water | TerrainType::Rock)
}

fn is_habitable(t: TerrainType) -> bool {
    matches!(
        t,
        TerrainType::Grass
            | TerrainType::Forest
            | TerrainType::Savanna
            | TerrainType::Rainforest
            | TerrainType::Taiga
    )
}

pub fn terrain_char(t: TerrainType) -> char {
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

struct Score {
    seed: u64,
    band_row: usize,
    barrier_frac: f32,
    bridges: usize,
    north_hab: f32,
    south_hab: f32,
    objective: f32,
}

fn score_seed(seed: u64) -> Score {
    let field = BiomeField::generate(seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
    let res = field.res;
    let mut best = Score {
        seed,
        band_row: 0,
        barrier_frac: 0.0,
        bridges: 0,
        north_hab: 0.0,
        south_hab: 0.0,
        objective: 0.0,
    };
    // Slide the candidate band over the map (rows measured as the band's top
    // edge), skipping the polar margins.
    for top in res / 8..(7 * res / 8).saturating_sub(BAND_H) {
        let bot = top + BAND_H;
        let mut barrier = 0usize;
        let mut bridges = 0usize;
        for col in 0..res {
            let mut col_barrier = 0usize;
            for row in top..bot {
                if is_barrier(field.at(col, row).terrain) {
                    col_barrier += 1;
                }
            }
            barrier += col_barrier;
            if col_barrier == 0 {
                bridges += 1;
            }
        }
        let band_cells = (res * BAND_H) as f32;
        let barrier_frac = barrier as f32 / band_cells;

        let mut north = 0usize;
        let mut south = 0usize;
        for row in 0..top {
            for col in 0..res {
                if is_habitable(field.at(col, row).terrain) {
                    north += 1;
                }
            }
        }
        for row in bot..res {
            for col in 0..res {
                if is_habitable(field.at(col, row).terrain) {
                    south += 1;
                }
            }
        }
        let north_hab = north as f32 / (res * top.max(1)) as f32;
        let south_hab = south as f32 / (res * (res - bot).max(1)) as f32;

        // Reward: strong barrier, habitable BOTH sides, few-but-nonzero land
        // bridges (1..6 columns of unbroken crossing). Zero bridges risks an
        // uncrossable ocean; many bridges means no drama.
        let bridge_score = if bridges == 0 {
            0.2
        } else if bridges <= 6 {
            1.0
        } else {
            (6.0 / bridges as f32).max(0.2)
        };
        let objective = barrier_frac * north_hab.min(south_hab) * 4.0 * bridge_score;
        if objective > best.objective {
            best = Score {
                seed,
                band_row: top,
                barrier_frac,
                bridges,
                north_hab,
                south_hab,
                objective,
            };
        }
    }
    best
}

fn ascii_map(seed: u64, band_row: usize, cols: usize, rows: usize) {
    let field = BiomeField::generate(seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
    let res = field.res;
    let band_lo = band_row * rows / res;
    let band_hi = (band_row + BAND_H) * rows / res;
    for r in 0..rows {
        let mut line = String::with_capacity(cols);
        let row = r * res / rows;
        for c in 0..cols {
            let col = c * res / cols;
            line.push(terrain_char(field.at(col, row).terrain));
        }
        let marker = if r >= band_lo && r < band_hi { "  <== band" } else { "" };
        println!("{line}{marker}");
    }
}

fn main() {
    let scan: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(400);

    let mut scores: Vec<Score> = (0..scan).map(score_seed).collect();
    scores.sort_by(|a, b| b.objective.partial_cmp(&a.objective).unwrap());

    println!(
        "Scanned seeds 0..{scan} at {BIOME_RES_DEFAULT}x{BIOME_RES_DEFAULT} \
         (band = {BAND_H} cells of water/rock)\n"
    );
    println!(
        "  {:>6}  {:>7}  {:>8}  {:>7}  {:>7}  {:>7}  {:>6}",
        "seed", "band_y", "barrier%", "bridges", "north%", "south%", "score"
    );
    for s in scores.iter().take(10) {
        let band_y =
            (s.band_row + BAND_H / 2) as f32 * (WORLD_SIZE_DEFAULT / BIOME_RES_DEFAULT as f32);
        println!(
            "  {:>6}  {:>7.0}  {:>7.1}%  {:>7}  {:>6.1}%  {:>6.1}%  {:>6.3}",
            s.seed,
            band_y,
            s.barrier_frac * 100.0,
            s.bridges,
            s.north_hab * 100.0,
            s.south_hab * 100.0,
            s.objective,
        );
    }

    println!("\nLegend: ~ water   . desert   \" grass   f forest   T rainforest   S savanna   t taiga   u tundra   ^ rock");
    for s in scores.iter().take(5) {
        println!("\n=== seed {} (band at row {}) ===", s.seed, s.band_row);
        ascii_map(s.seed, s.band_row, 96, 40);
    }

    // Report the exact bridge columns (contiguous runs of zero-barrier
    // columns through the band) for a chosen seed (arg 2; default = the
    // winner) — the crossing points a scenario should aim its migration
    // corridors at.
    let report = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(scores[0].seed);
    let s = score_seed(report);
    let best = &s;
    let field = BiomeField::generate(best.seed, BIOME_RES_DEFAULT, WORLD_SIZE_DEFAULT);
    let res = field.res;
    let cell = WORLD_SIZE_DEFAULT / res as f32;
    println!(
        "\nBest seed {} bridges (band y {}..{}):",
        best.seed,
        best.band_row as f32 * cell,
        (best.band_row + BAND_H) as f32 * cell
    );
    let mut run_start: Option<usize> = None;
    for col in 0..=res {
        let clear = col < res
            && (best.band_row..best.band_row + BAND_H)
                .all(|row| !is_barrier(field.at(col, row).terrain));
        match (run_start, clear) {
            (None, true) => run_start = Some(col),
            (Some(s), false) => {
                println!(
                    "  x {:.0}..{:.0}  ({} cells)",
                    s as f32 * cell,
                    col as f32 * cell,
                    col - s
                );
                run_start = None;
            }
            _ => {}
        }
    }
}
