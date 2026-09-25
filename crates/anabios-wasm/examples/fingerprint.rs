//! Print the portable trajectory fingerprint of a scenario run, natively.
//!
//! The wasm smoke test (`web/test/wasm-smoke.mjs`) computes the same
//! `view::fingerprint` inside the browser/node module; equal output here and
//! there means the wasm build reproduces the native trajectory bit-for-bit.
//!
//! ```text
//! cargo run --release -p anabios-wasm --example fingerprint -- \
//!     scenarios/predator-prey.toml 300 7
//! ```
//! Arguments: `<scenario.toml> [ticks=300] [seed=<file's seed>]`.

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anabios_wasm::view::fingerprint;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: fingerprint <scenario.toml> [ticks] [seed]");
    let ticks: u64 = args.next().map(|t| t.parse().expect("ticks")).unwrap_or(300);
    let seed: Option<u64> = args.next().map(|s| s.parse().expect("seed"));
    let text = std::fs::read_to_string(&path).expect("reading scenario");
    let mut scenario = Scenario::parse_toml(&text).expect("parsing scenario");
    if let Some(seed) = seed {
        scenario.seed = seed;
    }
    let mut world = scenario.instantiate();
    for _ in 0..ticks {
        step(&mut world);
        for _ in world.codex.drain_events() {}
    }
    println!(
        "scenario={} seed={} ticks={} alive={} fingerprint=0x{:016x}",
        scenario.name,
        world.seed,
        world.tick,
        world.agents.live_count(),
        fingerprint(&world)
    );
}
