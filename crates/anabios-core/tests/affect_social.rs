//! M-D flag-on behavior: CARE (kin provision) + PANIC/GRIEF (isolation distress).

use anabios_core::affect::{CARE, PANIC};
use anabios_core::culture::{ALARM_MEME_CHANNEL, MEME_BROADCAST_THRESHOLD};
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude_test::Vec2;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;
use anabios_core::world::World;

const AFFECT_SOCIAL: &str = include_str!("../../../scenarios/affect-social.toml");

#[test]
fn affect_social_scenario_enables_the_layer() {
    let w = Scenario::parse_toml(AFFECT_SOCIAL).expect("parse affect-social").instantiate();
    assert!(w.affect_enabled, "scenario must turn the affect layer on");
}

#[test]
fn isolated_social_agent_broadcasts_distress() {
    // A lone, strongly social agent (high Sociality) should raise an alarm
    // broadcast once PANIC has accrued over a few ticks. NOTE: a *neutral*
    // Sociality genome caps panic_trigger's `social` gain at 0.5 (see
    // affect::panic_trigger), so the isolation-only asymptote sits exactly at
    // `MEME_BROADCAST_THRESHOLD` (0.5) and can never cross it however long the
    // warm-up runs; a positive-Sociality genome (this test's "social" in its
    // name) raises the gain above 0.5 so the leaky integrator does cross the
    // threshold within a handful of ticks.
    let mut w = World::new(11);
    w.affect_enabled = true;
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Sociality, 1.0); // sociality() == +1.0 → panic gain 1.0
    let id = w.spawn_agent(Vec2::new(500.0, 500.0), g);
    for _ in 0..10 {
        step(&mut w);
    }
    assert!(w.agents.affect[id as usize][PANIC] > 0.0, "PANIC should have accrued");
    assert!(
        w.actions[id as usize].broadcast_intent[ALARM_MEME_CHANNEL] > MEME_BROADCAST_THRESHOLD,
        "isolated social agent should broadcast alarm above threshold"
    );
}

#[test]
fn kin_cluster_raises_care_and_sharing() {
    let mut w = World::new(12);
    w.affect_enabled = true;
    let a = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
    let _b = w.spawn_agent(Vec2::new(303.0, 300.0), Genome::neutral());
    let _c = w.spawn_agent(Vec2::new(300.0, 303.0), Genome::neutral());
    for _ in 0..5 {
        step(&mut w);
    }
    assert!(w.agents.affect[a as usize][CARE] > 0.0, "kin cluster should raise CARE");
    assert!(
        w.actions[a as usize].share_intent > 0.0,
        "CARE should push share_intent above zero for a clustered agent"
    );
}

/// Flag-ON trajectory pin for the affect layer (CARE + PANIC + PLAY live).
/// Regenerate with `UPDATE_HASHES=1` when the affect behaviour changes.
// Refreshed 2026-08-04 (M-E PLAY): juvenile members near same-species peers accrue
// PLAY (movement approach-bias) — flag-on behavior change, no layout growth.
// Refreshed 2026-08-07 (M-F observability, FORMAT_VERSION 28→29): new serialized
// CodexState detector fields grow the state-hash layout (detectors run flag-on),
// so all ticks move.
// Refreshed 2026-08-07 (O2 payoff-biased learning, FORMAT_VERSION 29→30):
// World.payoff_biased_learning layout growth only (off here).
const AFFECT_GOLDEN: &[(u64, u64)] =
    &[(0, 0x749d13d012077099), (100, 0x92bd3ffaf6d9e8ea), (300, 0x58a93278e94cad08)];

#[test]
fn affect_social_matches_golden_hashes() {
    let mut w = Scenario::parse_toml(AFFECT_SOCIAL).expect("parse affect-social").instantiate();
    let max_tick = AFFECT_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < AFFECT_GOLDEN.len() && AFFECT_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        step(&mut w);
    }
    if std::env::var("UPDATE_HASHES").is_ok() {
        for (t, h) in &observed {
            println!("({t}, {h:#018x}),");
        }
    }
    assert_eq!(observed, AFFECT_GOLDEN.to_vec(), "affect flag-on trajectory changed");
}
