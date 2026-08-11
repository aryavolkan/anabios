//! `#[serde(skip)]` audit guard: every skipped cache that must be re-derived
//! at load time is actually re-derived. `state_hash` hashes exactly the
//! serialized fields, so a skipped field that feeds future ticks breaks
//! replay invisibly (the v13 `still_ticks` footgun, and the spatial-hash dims
//! bug found by `save_load_roundtrip` — see `docs/determinism-contract.md`).

use anabios_core::scenario::Scenario;
use anabios_core::snapshot::{load_from_bytes, save_to_bytes, state_hash};
use anabios_core::tick::step;

/// A pheromone-active + domestication scenario, warmed so both documented
/// re-derivation caches are non-default: after load, `track_livestock` must
/// be re-derived from the persisted flag (not left false) and the pheromone
/// nonzero cache must match a fresh recompute (decay would no-op otherwise).
#[test]
fn load_rederives_skipped_caches() {
    let mut w = Scenario::parse_toml(include_str!("../../../scenarios/domestication.toml"))
        .unwrap()
        .instantiate();
    for _ in 0..300 {
        step(&mut w);
    }
    let reloaded = load_from_bytes(&save_to_bytes(&w).unwrap()).unwrap();
    // track_livestock must be re-derived from the persisted flag, not left false.
    assert_eq!(reloaded.agents.track_livestock, reloaded.domestication_enabled);
    // pheromone nonzero cache must match a fresh recompute (decay would no-op
    // otherwise) — identity of the full serialized state covers it.
    assert_eq!(state_hash(&w), state_hash(&reloaded));
}

/// The serde-skipped spatial hashes reset to `Default` (1024/64) on load;
/// `load_from_bytes` must re-derive them from the persisted world dims, or a
/// custom-dims world reloads with the wrong `cell_size` (clamping perception
/// radii wrong) and wrong torus extent — the bug `season_period_roundtrip`
/// and `living_biome_roundtrip` surfaced. Pin the re-derivation directly.
#[test]
fn load_rederives_spatial_hash_dims() {
    let mut w = Scenario::parse_toml(include_str!("../../../scenarios/sandbox-large.toml"))
        .unwrap()
        .instantiate();
    assert_eq!(w.world_size, 2048.0, "scenario pins non-default dims");
    for _ in 0..10 {
        step(&mut w);
    }
    let reloaded = load_from_bytes(&save_to_bytes(&w).unwrap()).unwrap();
    let want = w.spatial.perception_max_radius();
    for (name, got) in [
        ("spatial", reloaded.spatial.perception_max_radius()),
        ("carcass_spatial", reloaded.carcass_spatial.perception_max_radius()),
        ("resource_spatial", reloaded.resource_spatial.perception_max_radius()),
    ] {
        assert_eq!(got, want, "{name} reloaded with default dims (1024/64)");
    }
}
