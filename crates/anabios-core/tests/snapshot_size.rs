//! Phase-1 "scale fields": print the serialized snapshot size of the Huge
//! scale tier, so a human skimming test output (`--nocapture`) can see the
//! save-file cost of the new world-size budget. Not a pinned assertion — just
//! prints; see `docs/perf-notes.md` for the recorded numbers.

use anabios_core::scenario::Scenario;
use anabios_core::snapshot::save_to_bytes;

#[test]
fn huge_steppe_snapshot_bytes() {
    let toml = include_str!("../../../scenarios/huge-steppe.toml");
    let w = Scenario::parse_toml(toml).unwrap().instantiate();
    let bytes = save_to_bytes(&w).expect("save");
    eprintln!(
        "huge-steppe.toml snapshot: {} bytes ({:.2} MiB) at tick 0, {} agents, biome {}x{} cells",
        bytes.len(),
        bytes.len() as f64 / (1024.0 * 1024.0),
        w.agents.live_count(),
        w.biome.res,
        w.biome.res,
    );
    assert!(!bytes.is_empty());
}
