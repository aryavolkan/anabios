use anabios_core::biome::{BiomeField, TerrainType, EARTH_RES};

/// from_earth builds a full 256x256 field with a plausible land fraction and
/// real coastlines: central Africa is land, the mid-Pacific is open water.
#[test]
fn from_earth_has_real_coastlines() {
    let f = BiomeField::from_earth(EARTH_RES, 4096.0);
    assert_eq!(f.cells.len(), EARTH_RES * EARTH_RES);
    let land = f.cells.iter().filter(|c| c.terrain != TerrainType::Water).count();
    let frac = land as f32 / f.cells.len() as f32;
    assert!((0.15..0.45).contains(&frac), "implausible land fraction {frac}");

    // Equirectangular: x = (lon+180)/360*W, y = (90-lat)/180*W ; cell = pos/cell_size.
    let cell = |lat: f32, lon: f32| {
        let x = (lon + 180.0) / 360.0 * f.world_size;
        let y = (90.0 - lat) / 180.0 * f.world_size;
        let col = (x / f.cell_size) as usize;
        let row = (y / f.cell_size) as usize;
        f.cells[row * f.res + col].terrain
    };
    assert_ne!(cell(0.0, 20.0), TerrainType::Water, "central Africa should be land");
    assert_eq!(cell(0.0, -150.0), TerrainType::Water, "mid-Pacific should be water");
}

/// from_earth is a pure function of the embedded assets: identical every call.
#[test]
fn from_earth_is_deterministic() {
    let a = BiomeField::from_earth(EARTH_RES, 4096.0);
    let b = BiomeField::from_earth(EARTH_RES, 4096.0);
    assert!(a.cells.iter().zip(&b.cells).all(|(x, y)| x.terrain == y.terrain
        && x.elevation == y.elevation
        && x.env == y.env
        && x.moisture == y.moisture));
}
