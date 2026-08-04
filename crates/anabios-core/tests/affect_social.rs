//! M-D flag-on behavior: CARE (kin provision) + PANIC/GRIEF (isolation distress).

use anabios_core::affect::{CARE, PANIC};
use anabios_core::culture::{ALARM_MEME_CHANNEL, MEME_BROADCAST_THRESHOLD};
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude_test::Vec2;
use anabios_core::scenario::Scenario;
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
