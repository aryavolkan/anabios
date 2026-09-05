//! Terrain-aware founder placement: `Placement::Habitat` and
//! `Placement::NearSpec`.
//!
//! The property under test is that these two placements re-derive their sites
//! from the *generated* world instead of trusting hardcoded coordinates. So
//! every assertion here is checked across a span of seeds: a placement that
//! only works on the seed it was scouted against is exactly the failure mode
//! these variants exist to remove.

use anabios_core::biome::TerrainType;
use anabios_core::needs::drinkable_cell;
use anabios_core::scenario::{Scenario, HABITAT_WATER_CELLS};
use anabios_core::snapshot::state_hash;
use anabios_core::spatial::torus_distance;
use anabios_core::world::World;
use glam::Vec2;

/// A watered, mountainous world small enough to generate quickly. The
/// river threshold is pinned to this `biome_res` on purpose — see
/// `examples/river_scaling.rs` for why the pair cannot be separated.
fn watered_world(seed: u64, extra: &str) -> String {
    format!(
        r#"
name = "habitat-test"
seed = {seed}
world_size = 2048.0
biome_res = 256
hash_res = 128
max_population = 400

[climate]
continentality = 0.85
mountain_uplift = 0.6
rain_shadow = 0.4
river_threshold = 50.0
sea_level = 0.45
{extra}
"#
    )
}

fn instantiate(toml: &str) -> World {
    Scenario::parse_toml(toml).expect("parse").instantiate()
}

/// World-unit distance from `pos` to the nearest drinkable cell, by scanning
/// the grid. Deliberately a brute-force independent reimplementation: it must
/// not share code with the BFS under test.
fn distance_to_water(world: &World, pos: Vec2) -> f32 {
    let biome = &world.biome;
    let mut best = f32::INFINITY;
    for row in 0..biome.res {
        for col in 0..biome.res {
            if !drinkable_cell(biome, col, row) {
                continue;
            }
            let center = Vec2::new(
                (col as f32 + 0.5) * biome.cell_size,
                (row as f32 + 0.5) * biome.cell_size,
            );
            best = best.min(torus_distance(center, pos, world.world_size));
        }
    }
    best
}

#[test]
fn habitat_places_every_grazer_on_forage_within_reach_of_water() {
    // The anchor is guaranteed watered and vegetated; agents then scatter up
    // to `radius` off it, so the per-agent bound is anchor reach + radius.
    let radius = 60.0f32;
    let max_water_dist = 40.0f32;
    for seed in 0..6u64 {
        let toml = watered_world(
            seed,
            &format!(
                r#"
[[agents]]
count = 40
archetype = "mammal_grazer"
placement = {{ kind = "habitat", herds = 3, radius = {radius}, max_water_dist = {max_water_dist} }}
"#
            ),
        );
        let w = instantiate(&toml);
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        assert_eq!(ids.len(), 40, "seed {seed}: all grazers spawn");
        for id in ids {
            let pos = w.agents.position[id as usize];
            let d = distance_to_water(&w, pos);
            assert!(
                d <= max_water_dist + radius,
                "seed {seed}: grazer {id} sited {d:.1} units from water, \
                 beyond the {max_water_dist} anchor reach + {radius} scatter"
            );
        }
    }
}

#[test]
fn habitat_anchors_sit_on_vegetated_ground_beside_water() {
    // Pin `radius = 0` so every agent lands exactly on its anchor: this is the
    // assertion about the *site choice*, with the scatter removed.
    for seed in 0..6u64 {
        let toml = watered_world(
            seed,
            r#"
[[agents]]
count = 12
archetype = "mammal_grazer"
placement = { kind = "habitat", herds = 4, radius = 0.0, max_water_dist = 32.0 }
"#,
        );
        let w = instantiate(&toml);
        for id in w.agents.iter_alive() {
            let pos = w.agents.position[id as usize];
            let cell = w.biome.sample(pos);
            assert!(
                cell.terrain.carrying_capacity() > 0.0,
                "seed {seed}: anchor on barren {:?} — a herd cannot graze there",
                cell.terrain
            );
            assert_ne!(cell.terrain, TerrainType::Water, "seed {seed}: anchor in open water");
            let d = distance_to_water(&w, pos);
            assert!(d <= 32.0, "seed {seed}: anchor {d:.1} units from water, asked for <= 32");
        }
    }
}

#[test]
fn habitat_default_water_reach_scales_to_the_field() {
    // `max_water_dist` absent => HABITAT_WATER_CELLS cells of the *actual*
    // field, not a constant in default-world units.
    let toml = watered_world(
        3,
        r#"
[[agents]]
count = 16
archetype = "mammal_grazer"
placement = { kind = "habitat", herds = 2, radius = 0.0 }
"#,
    );
    let w = instantiate(&toml);
    let reach = HABITAT_WATER_CELLS * w.biome.cell_size;
    for id in w.agents.iter_alive() {
        let d = distance_to_water(&w, w.agents.position[id as usize]);
        assert!(d <= reach, "default reach should be {reach:.1} units, got {d:.1}");
    }
}

#[test]
fn habitat_spreads_herds_apart() {
    // Four herds at radius 80 should not collapse onto one river bend. The
    // separation pass is best-effort (bounded retries), so assert the weaker
    // property that actually matters: the cohort occupies several distinct
    // regions rather than one.
    let toml = watered_world(
        11,
        r#"
[[agents]]
count = 80
archetype = "mammal_grazer"
placement = { kind = "habitat", herds = 4, radius = 80.0 }
"#,
    );
    let w = instantiate(&toml);
    let positions: Vec<Vec2> =
        w.agents.iter_alive().map(|id| w.agents.position[id as usize]).collect();
    let spread = positions
        .iter()
        .flat_map(|a| positions.iter().map(move |b| torus_distance(*a, *b, 2048.0)))
        .fold(0.0f32, f32::max);
    assert!(
        spread > 160.0,
        "four herds should span more than one 80-unit cluster, got {spread:.1}"
    );
}

#[test]
fn near_spec_puts_predators_on_top_of_the_herds() {
    let radius = 100.0f32;
    for seed in 0..6u64 {
        let toml = watered_world(
            seed,
            &format!(
                r#"
[[agents]]
count = 40
archetype = "mammal_grazer"
placement = {{ kind = "habitat", herds = 3, radius = 70.0 }}

[[agents]]
count = 8
archetype = "mammal_pursuer"
placement = {{ kind = "near_spec", spec = 0, radius = {radius} }}
"#
            ),
        );
        let w = instantiate(&toml);
        let ids: Vec<u32> = w.agents.iter_alive().collect();
        assert_eq!(ids.len(), 48, "seed {seed}: both specs spawn");
        // Species 1 is the grazer spec, species 2 the pursuers (species 0 is
        // reserved for archetype-free specs).
        let prey: Vec<Vec2> = ids
            .iter()
            .filter(|id| w.agents.species_id[**id as usize] == 1)
            .map(|id| w.agents.position[*id as usize])
            .collect();
        let predators: Vec<Vec2> = ids
            .iter()
            .filter(|id| w.agents.species_id[**id as usize] == 2)
            .map(|id| w.agents.position[*id as usize])
            .collect();
        assert_eq!(prey.len(), 40, "seed {seed}: grazer count");
        assert_eq!(predators.len(), 8, "seed {seed}: pursuer count");
        for p in &predators {
            let nearest = prey
                .iter()
                .map(|q| torus_distance(*p, *q, w.world_size))
                .fold(f32::INFINITY, f32::min);
            assert!(
                nearest <= radius,
                "seed {seed}: pursuer sited {nearest:.1} units from the nearest grazer, \
                 beyond its {radius} scatter — predators must start on the herds"
            );
        }
    }
}

#[test]
fn terrain_aware_placement_is_deterministic() {
    let toml = watered_world(
        5,
        r#"
[[agents]]
count = 30
archetype = "mammal_grazer"
placement = { kind = "habitat", herds = 3, radius = 60.0 }

[[agents]]
count = 6
archetype = "mammal_pursuer"
placement = { kind = "near_spec", spec = 0, radius = 90.0 }
"#,
    );
    let scenario = Scenario::parse_toml(&toml).expect("parse");
    let a = scenario.instantiate();
    let b = scenario.instantiate();
    assert_eq!(state_hash(&a), state_hash(&b), "two instantiations must agree");
}

#[test]
fn habitat_falls_back_to_uniform_in_a_world_with_no_forage() {
    // sea_level 0.99 drowns essentially everything: no vegetated cell can
    // anchor a herd. The run must still start rather than panic or hang.
    let toml = r#"
name = "drowned"
seed = 2
max_population = 100

[climate]
sea_level = 0.99

[[agents]]
count = 20
archetype = "mammal_grazer"
placement = { kind = "habitat", herds = 3, radius = 50.0 }
"#;
    let w = instantiate(toml);
    assert_eq!(w.agents.iter_alive().count(), 20, "all agents still spawn");
    for id in w.agents.iter_alive() {
        let p = w.agents.position[id as usize];
        assert!(
            p.x.is_finite() && p.y.is_finite(),
            "fallback must produce real coordinates, got {p:?}"
        );
    }
}
