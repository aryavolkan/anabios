//! Integration tests for the biome trade-goods economy.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;

const TRADE: &str = include_str!("../../../scenarios/biome-trade.toml");
const GEO: &str = include_str!("../../../scenarios/geographic-trade.toml");

/// The trade scenario is deterministic: two independent runs match at tick 300.
#[test]
fn trade_scenario_is_deterministic() {
    let run = || {
        let mut w = Scenario::parse_toml(TRADE).expect("parse").instantiate();
        for _ in 0..300 {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(), run(), "trade scenario must replay identically");
}

/// The economy actually turns over: cross-species trades and dowry births occur.
#[test]
fn trade_scenario_produces_trades_and_dowry_births() {
    let mut w = Scenario::parse_toml(TRADE).expect("parse").instantiate();
    let mut saw_trade = false;
    let mut saw_dowry = false;
    for _ in 0..600 {
        step(&mut w);
        for e in w.codex.events.iter() {
            match e.event_type {
                EventType::ResourceTraded => saw_trade = true,
                EventType::DowryBirth => saw_dowry = true,
                _ => {}
            }
        }
        if saw_trade && saw_dowry {
            break;
        }
    }
    assert!(saw_trade, "expected at least one cross-species trade");
    assert!(saw_dowry, "expected at least one dowry-gated birth");
}

/// Fraction of alive agents currently standing on THEIR OWN preferred
/// terrain (per `TerrainAffinity` -> `preferred_good` -> `home_terrain`).
fn sorted_fraction(w: &anabios_core::world::World) -> f32 {
    let mut on = 0u32;
    let mut n = 0u32;
    for id in w.agents.iter_alive() {
        let i = id as usize;
        let aff = w.agents.genome[i].get(anabios_core::genome::GenomeSlot::TerrainAffinity);
        let target = anabios_core::resource::preferred_good(aff).home_terrain();
        if w.biome.sample(w.agents.position[i]).terrain == target {
            on += 1;
        }
        n += 1;
    }
    if n == 0 {
        0.0
    } else {
        on as f32 / n as f32
    }
}

/// The geographic-trade scenario is deterministic: two independent runs
/// match at tick 300.
#[test]
fn geographic_trade_scenario_is_deterministic() {
    let run = || {
        let mut w = Scenario::parse_toml(GEO).expect("parse").instantiate();
        for _ in 0..300 {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(), run(), "geographic-trade scenario must replay identically");
}

/// The geographic-trade economy turns over: cross-species trades AND
/// same-species dowry-gated births both occur.
#[test]
fn geographic_trade_produces_trades_and_dowry_births() {
    let mut w = Scenario::parse_toml(GEO).expect("parse").instantiate();
    let mut saw_trade = false;
    let mut saw_dowry = false;
    for _ in 0..800 {
        step(&mut w);
        for e in w.codex.events.iter() {
            match e.event_type {
                EventType::ResourceTraded => saw_trade = true,
                EventType::DowryBirth => saw_dowry = true,
                _ => {}
            }
        }
        if saw_trade && saw_dowry {
            break;
        }
    }
    assert!(saw_trade, "expected at least one cross-species trade");
    assert!(saw_dowry, "expected at least one dowry-gated birth");
}

/// Geographic sorting actually happens: the fraction of agents standing on
/// their own preferred terrain increases from tick 0 to ~tick 400. This is
/// the ROBUST metric (whole-population, not per-species) called out in the
/// task brief — it proves the `terrain_habitat` cline forms without
/// requiring perfect sorting or fighting the Rock-terrain scarcity problem.
#[test]
fn geographic_trade_sorts_by_terrain() {
    let mut w = Scenario::parse_toml(GEO).expect("parse").instantiate();
    let sorted_before = sorted_fraction(&w);
    for _ in 0..400 {
        step(&mut w);
    }
    let sorted_after = sorted_fraction(&w);
    assert!(
        sorted_after > sorted_before + 0.05,
        "expected sorted fraction to increase: before={sorted_before}, after={sorted_after}"
    );
}

/// Regression guard: a resources-OFF scenario is unaffected by the feature.
/// (minimal.toml never enables resources; its golden hashes live in
/// determinism.rs. This asserts the flag genuinely defaults off end-to-end.)
#[test]
fn minimal_scenario_keeps_resources_off() {
    let minimal = include_str!("../../../scenarios/minimal.toml");
    let w = Scenario::parse_toml(minimal).expect("parse").instantiate();
    assert!(!w.resources_enabled);
    assert!(w.resources.is_empty());
}

/// Trade is ongoing, not a one-off: the per-tick trade-route buffer (which
/// also feeds the viewer) keeps recording fresh trades late into the run.
/// The codex `ResourceTraded` event is latched on the first trade, so event
/// counts alone cannot prove turnover — this asserts the underlying swap
/// flow stays alive across the whole run.
#[test]
fn geographic_trade_turnover_is_ongoing() {
    let mut w = Scenario::parse_toml(GEO).expect("parse").instantiate();
    let mut early = 0usize; // ticks 0..400
    let mut late = 0usize; // ticks 400..800
    for t in 0..800 {
        step(&mut w);
        if t < 400 {
            early += w.trade_routes.len();
        } else {
            late += w.trade_routes.len();
        }
    }
    assert!(early > 0, "expected trades in ticks 0..400, got {early}");
    assert!(late > early / 4, "expected trade to stay alive late: early={early}, late={late}");
}
