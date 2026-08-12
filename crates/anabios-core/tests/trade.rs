//! Integration tests for the biome trade-goods economy.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;

const TRADE: &str = include_str!("../../../scenarios/biome-trade.toml");
const GEO: &str = include_str!("../../../scenarios/geographic-trade.toml");
const UNI: &str = include_str!("../../../scenarios/unilateral-trade.toml");

/// The `unilateral_trade` flag parses from TOML and wires through to `World`.
#[test]
fn unilateral_trade_flag_parses_and_wires() {
    let toml = "name = \"t\"\nseed = 1\nworld_size = 64\nresources_enabled = true\nunilateral_trade = true\n[[agents]]\narchetype = \"grazer\"\ncount = 4\n";
    let w = Scenario::parse_toml(toml).unwrap().instantiate();
    assert!(w.unilateral_trade);
    // And the scenario file carries both freeze-fix flags.
    let w = Scenario::parse_toml(UNI).expect("parse unilateral-trade").instantiate();
    assert!(w.unilateral_trade && w.conserve_goods_on_death);
}

/// The unilateral-exchange scenario is deterministic (both new flags on).
#[test]
fn unilateral_trade_scenario_is_deterministic() {
    let run = || {
        let mut w = Scenario::parse_toml(UNI).expect("parse").instantiate();
        for _ in 0..300 {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(), run(), "unilateral-trade scenario must replay identically");
}

/// The unilateral-exchange economy turns over: cross-species trades occur.
/// (The freeze-escape evidence — nonzero trade past the baseline ~t10k freeze
/// — is a long-horizon property measured in release sweeps; see
/// `docs/superpowers/specs/2026-08-02-trade-freeze-diagnosis.md`. At this
/// horizon the baseline scenario also trades, so this only guards that the
/// new code path doesn't break the economy.)
#[test]
fn unilateral_trade_scenario_produces_trades() {
    let mut w = Scenario::parse_toml(UNI).expect("parse").instantiate();
    let mut saw_trade = false;
    for _ in 0..600 {
        step(&mut w);
        if w.codex.events.iter().any(|e| e.event_type == EventType::ResourceTraded) {
            saw_trade = true;
            break;
        }
    }
    assert!(saw_trade, "expected at least one cross-species trade");
}

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

/// The economy actually turns over: cross-species trades occur, and — with
/// reproduction no longer goods-gated — the population grows past its
/// founding stock.
#[test]
fn trade_scenario_produces_trades_and_population_growth() {
    let mut w = Scenario::parse_toml(TRADE).expect("parse").instantiate();
    let initial = w.agents.live_count();
    let mut saw_trade = false;
    for _ in 0..600 {
        step(&mut w);
        for e in w.codex.events.iter() {
            if e.event_type == EventType::ResourceTraded {
                saw_trade = true;
            }
        }
        if saw_trade && w.agents.live_count() > initial {
            break;
        }
    }
    assert!(saw_trade, "expected at least one cross-species trade");
    assert!(
        w.agents.live_count() > initial,
        "expected ungated births to grow the population: {initial} -> {}",
        w.agents.live_count()
    );
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

/// The geographic-trade economy turns over: cross-species trades occur AND
/// the population grows past its founding stock (reproduction is no longer
/// goods-gated; goods fund invention learning instead).
#[test]
fn geographic_trade_produces_trades_and_population_growth() {
    let mut w = Scenario::parse_toml(GEO).expect("parse").instantiate();
    let initial = w.agents.live_count();
    let mut saw_trade = false;
    for _ in 0..800 {
        step(&mut w);
        for e in w.codex.events.iter() {
            if e.event_type == EventType::ResourceTraded {
                saw_trade = true;
            }
        }
        if saw_trade && w.agents.live_count() > initial {
            break;
        }
    }
    assert!(saw_trade, "expected at least one cross-species trade");
    assert!(
        w.agents.live_count() > initial,
        "expected ungated births to grow the population: {initial} -> {}",
        w.agents.live_count()
    );
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
    // The cumulative counter (HUD observability) must equal the summed
    // per-tick buffer — both count exactly one record per swap.
    assert_eq!(w.total_trades, (early + late) as u64, "total_trades must track every swap");
}

/// The `conserve_goods_on_death` flag parses from TOML and wires through to
/// `World` (no behavior yet — Task 1 only adds the flag).
#[test]
fn conserve_goods_on_death_flag_parses_and_wires() {
    let toml = "name = \"t\"\nseed = 1\nworld_size = 64\nresources_enabled = true\nconserve_goods_on_death = true\n[[agents]]\narchetype = \"grazer\"\ncount = 4\n";
    let w = anabios_core::scenario::Scenario::parse_toml(toml).unwrap().instantiate();
    assert!(w.conserve_goods_on_death);
}

/// Killing an agent that holds goods snapshots them into `deaths_scratch`;
/// `conserve_goods_step` redistributes the snapshot to the nearest living
/// agent and drains the buffer.
#[test]
fn conserve_goods_step_moves_dead_inventory_to_living() {
    use anabios_core::genome::Genome;
    use anabios_core::prelude_test::Vec2;
    let toml = "name = \"c\"\nseed = 1\nworld_size = 64\nresources_enabled = true\nconserve_goods_on_death = true\n";
    let mut w = anabios_core::scenario::Scenario::parse_toml(toml).unwrap().instantiate();
    // Two agents a few units apart; A holds goods, B is the nearest (only) living neighbour.
    let a = w.spawn_agent(Vec2::new(10.0, 10.0), Genome::neutral());
    let b = w.spawn_agent(Vec2::new(12.0, 10.0), Genome::neutral());
    w.agents.inventory[a as usize] = [3.0, 1.0, 0.0, 2.0];
    let before_b: f32 = w.agents.inventory[b as usize].iter().sum();
    // Kill A; its goods must land on B after the conservation stage.
    w.agents.kill(a);
    anabios_core::resource::conserve_goods_step(&mut w);
    let after_b: f32 = w.agents.inventory[b as usize].iter().sum();
    assert!(
        (after_b - before_b - 6.0).abs() < 1e-4,
        "B should gain A's 6 units, got {after_b} from {before_b}"
    );
    assert_eq!(w.agents.inventory[b as usize], [3.0, 1.0, 0.0, 2.0]);
    // Buffer is drained.
    assert!(w.agents.deaths_scratch.is_empty());
}

/// A trade-motivated agent gets steered toward the nearest hub inside
/// `decide_all`. This isolates the new additive bias from every other
/// movement force: `decide()` itself never reads inventory, so two
/// otherwise-identical single-agent worlds (same seed, same start position,
/// same program/genome/sensors) diverge in their first-tick displacement
/// only through the hub-seeking block, which is gated on `has_trade_motive`.
/// A population-level "clustering increases over time" version of this test
/// was tried first and discarded: it already passed with no hub bias wired
/// in at all (other forces + natural wandering were enough to keep the
/// near-hub fraction from decreasing over 800 ticks), so it could not
/// distinguish "feature present" from "feature absent" — not usable for TDD.
#[test]
fn motivated_agent_gets_extra_pull_toward_hub() {
    use anabios_core::biome::WORLD_SIZE_DEFAULT;
    use anabios_core::genome::Genome;
    use anabios_core::hub::{best_hub_direction, TradeHub};
    use anabios_core::prelude_test::Vec2;
    use anabios_core::resource::{GOOD_COUNT, STOCK_TARGET, TRADE_UNIT};
    use anabios_core::world::World;

    let start = Vec2::new(5.0, 5.0);
    let hub_pos = Vec2::new(50.0, 50.0);
    let hub = || TradeHub { pos: hub_pos, cell: 0, goods: vec![] };

    let run = |motivated: bool| {
        let mut w = World::new(7);
        w.resources_enabled = true;
        w.trade_hubs = vec![hub()];
        let a = w.spawn_agent(start, Genome::neutral());
        w.agents.inventory[a as usize] = if motivated {
            let mut inv = [STOCK_TARGET; GOOD_COUNT];
            inv[0] = STOCK_TARGET + TRADE_UNIT * 2.0; // surplus -> has_trade_motive
            inv
        } else {
            [STOCK_TARGET; GOOD_COUNT] // balanced -> no motive
        };
        step(&mut w);
        w.agents.position[a as usize]
    };

    let pos_unmotivated = run(false);
    let pos_motivated = run(true);
    let dir = best_hub_direction(&[hub()], start, WORLD_SIZE_DEFAULT);
    assert!(dir.length() > 0.5, "hub steering must produce a real direction");
    let delta = pos_motivated - pos_unmotivated;
    assert!(
        delta.dot(dir) > 1e-4,
        "trade-motivated agent should be pulled toward the hub relative to an \
         unmotivated one: delta={delta:?}, dir={dir:?}"
    );
}
