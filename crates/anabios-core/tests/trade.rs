//! Integration tests for the biome trade-goods economy.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;

const TRADE: &str = include_str!("../../../scenarios/biome-trade.toml");

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
