//! Varied nutrient value + soil fertility: field generation, inertness when
//! flagged off, and (later tasks) consumption behavior.

use anabios_core::biome::{
    BiomeField, TerrainType, FERTILITY_MAX, FERTILITY_MIN, NUTRIENT_QUALITY_MAX,
    NUTRIENT_QUALITY_MIN, SUCCESSION_CLIMAX,
};
use anabios_core::scenario::Scenario;
use anabios_core::tick::step;

const MINIMAL: &str = include_str!("../../../scenarios/minimal.toml");

#[test]
fn generated_fields_land_in_range() {
    let b = BiomeField::generate(0, 8, 1024.0);
    for cell in &b.cells {
        assert!(
            (NUTRIENT_QUALITY_MIN..=NUTRIENT_QUALITY_MAX).contains(&cell.nutrient_quality),
            "nutrient_quality {} out of range",
            cell.nutrient_quality
        );
        assert!(
            (FERTILITY_MIN..=FERTILITY_MAX).contains(&cell.fertility),
            "fertility {} out of range",
            cell.fertility
        );
    }
}

/// With both flags OFF (default), the nutrient_quality/fertility field VALUES
/// must not influence simulation dynamics: mutating them to extremes leaves the
/// biomass trajectory and agent energies bit-identical.
#[test]
fn fields_are_inert_when_flags_off() {
    let base = Scenario::parse_toml(MINIMAL).expect("parse");
    let mut a = base.clone().instantiate();
    let mut b = base.instantiate();
    assert!(!b.nutrient_variation && !b.soil_fertility);
    // Perturb every cell's new fields in world B only.
    for cell in b.biome.cells.iter_mut() {
        cell.nutrient_quality = 0.1;
        cell.fertility = 0.1;
    }
    for _ in 0..100 {
        step(&mut a);
        step(&mut b);
    }
    let biomass_a: Vec<f32> = a.biome.cells.iter().map(|c| c.plant_biomass).collect();
    let biomass_b: Vec<f32> = b.biome.cells.iter().map(|c| c.plant_biomass).collect();
    assert_eq!(biomass_a, biomass_b, "field values leaked into biomass dynamics");
    let energy_a: Vec<f32> = a.agents.iter_alive().map(|id| a.agents.energy[id as usize]).collect();
    let energy_b: Vec<f32> = b.agents.iter_alive().map(|id| b.agents.energy[id as usize]).collect();
    assert_eq!(energy_a, energy_b, "field values leaked into agent energy");
}

/// With soil_fertility ON, a high-fertility Grass cell reaches a higher standing
/// crop than a low-fertility one; Water stays barren regardless.
#[test]
fn fertility_scales_capacity_and_regrowth() {
    let mut b = BiomeField::generate(0, 8, 1024.0);
    // Cell 0: fertile grass. Cell 1: poor grass. Cell 2: water (barren).
    for (idx, (terr, fert)) in
        [(TerrainType::Grass, 1.5), (TerrainType::Grass, 0.5), (TerrainType::Water, 1.5)]
            .into_iter()
            .enumerate()
    {
        let c = &mut b.cells[idx];
        c.terrain = terr;
        c.fertility = fert;
        c.plant_biomass = if terr == TerrainType::Water { 0.0 } else { 1.0 };
        c.succession = SUCCESSION_CLIMAX;
        c.pollution = 0.0;
    }
    for _ in 0..2000 {
        b.regrow_step(true);
    }
    assert!(
        b.cells[0].plant_biomass > b.cells[1].plant_biomass,
        "fertile {} should exceed poor {}",
        b.cells[0].plant_biomass,
        b.cells[1].plant_biomass
    );
    // Fertile grass should exceed the flat carrying capacity (10.0) it would cap
    // at with fertility ignored.
    assert!(b.cells[0].plant_biomass > 10.0);
    assert_eq!(b.cells[2].plant_biomass, 0.0, "water stays barren");
}

/// With nutrient_variation ON, uniformly high-quality cells yield more total
/// forage energy than uniformly low-quality cells over the same run.
#[test]
fn nutrient_quality_scales_forage_energy() {
    let make = |q: f32| {
        let mut w = Scenario::parse_toml(MINIMAL).expect("parse").instantiate();
        w.nutrient_variation = true;
        for cell in w.biome.cells.iter_mut() {
            cell.nutrient_quality = q;
        }
        w
    };
    let mut hi = make(1.4);
    let mut lo = make(0.6);
    for _ in 0..20 {
        step(&mut hi);
        step(&mut lo);
    }
    let sum = |w: &anabios_core::world::World| -> f32 {
        w.agents.iter_alive().map(|id| w.agents.energy[id as usize]).sum()
    };
    assert!(
        sum(&hi) > sum(&lo),
        "high-quality total energy {} should exceed low {}",
        sum(&hi),
        sum(&lo)
    );
}
