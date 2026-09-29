//! M-E flag-ON end-to-end: PLAY trigger + social-approach bias + IQ-enrichment
//! coupling. Runs on `tribes` (which absorbed `affect-play.toml`), where the
//! full stack keeps both `affect_enabled` and `cognition_enabled` on so all
//! three PLAY touchpoints are exercised. Models `cognition.rs`.

use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;

mod common;

const SCENARIO: &str = include_str!("../../../scenarios/tribes.toml");

#[test]
fn affect_play_scenario_parses_with_both_flags() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse affect-play scenario");
    assert!(s.affect_enabled, "affect layer must be on");
    assert!(s.cognition_enabled, "cognition must be on");
}

#[test]
fn affect_play_scenario_is_self_consistent() {
    let run = || {
        let mut w = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
        for _ in 0..200 {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(), run(), "same seed + flags on → bit-identical");
}

/// Flag-ON trajectory pin for M-E PLAY + enrichment (affect_enabled +
/// cognition_enabled). Regenerate deliberately with `UPDATE_HASHES=1` when the
/// PLAY behaviour changes on purpose.
// Refreshed 2026-08-07 (M-F observability, FORMAT_VERSION 28→29): new serialized
// CodexState detector fields grow the state-hash layout (detectors run flag-on),
// so all ticks move.
// Refreshed 2026-08-07 (O2 payoff-biased learning, FORMAT_VERSION 29→30):
// World.payoff_biased_learning layout growth only (off here).
// Refreshed 2026-08-19 (anthropogenic arms race, FORMAT_VERSION →34):
// World.{anthro_race_enabled,culture_roots} + CodexState hunted fields —
// layout growth only (off here).
const PLAY_GOLDEN: &[(u64, u64)] =
    // Refreshed 2026-09-02 (basic needs, FORMAT_VERSION 34→35): thirst/
    // fatigue/asleep columns + basic_needs_enabled + EventType::Dehydration.
    // Flag off here ⇒ layout growth only, trajectory byte-identical.
    // Refreshed 2026-09-04 (merge of main incl. repro_biased_learning #145,
    // FORMAT_VERSION 35→36): births_ok/births_failed + thirst/fatigue/asleep
    // columns now both serialized. All flags off here ⇒ layout growth only,
    // trajectory byte-identical.
    // Refreshed 2026-09-04 (disease merge, FORMAT_VERSION 36→37): infection
    // column + epidemic_latched + the two disease events. Flag off here ⇒
    // layout growth only, trajectory byte-identical.
    // Refreshed 2026-09-04 (mood arbiter, FORMAT_VERSION 37→38):
    // AgentBuffers.mood column. affect_enabled is ON here, so the mood layer
    // genuinely arbitrates — a real flag-on trajectory change layered on the
    // layout growth.
    // Refreshed 2026-09-05 (sparse-lineage breeding, FORMAT_VERSION 38→39):
    // World.lineage_caps + World.mate_seeking_enabled. Both absent/off here ⇒
    // layout growth only, trajectory byte-identical.
    // Refreshed 2026-09-05 (tuned PERCEPTION_ENERGY_COST to 0.005 so the
    // radius-scaled IQ cost is strong enough to matter but not strong enough
    // to drown the PLAY enrichment signal).
    // Refreshed 2026-09-05 (affective temperament unified onto the Big Five):
    // boldness/aggressiveness/nurturance/sociality/reactivity are now derived
    // from Neuroticism/Agreeableness/Extraversion instead of dedicated genome
    // slots, and archetypes DO set those OCEAN slots — so temperament now
    // actually varies by archetype. Behaviour-only change: tick 0 is
    // byte-identical, later ticks move.
    // Refreshed 2026-09-07 (military branch, FORMAT_VERSION 39→40): the meme
    // vector widened and four inventions were appended. inventions_enabled is
    // off here (cognition_enabled alone is on) ⇒ the new candidates are never
    // consulted — layout growth only, trajectory byte-identical.
    // Refreshed 2026-09-10 (X1 invention expansion, FORMAT_VERSION 40→41):
    // meme channels widened 24->32 and INVENTION_COUNT grew 14->20.
    // inventions_enabled is off here ⇒ layout growth only, trajectory
    // byte-identical.
    // Refreshed 2026-09-11 (X2 Wells+Vaccination, FORMAT_VERSION 41→42):
    // invention count 20->22 shifted the discovery table and practice
    // channels. inventions_enabled is off here ⇒ layout growth only; tick 0
    // held, ticks 100/200 moved.
    // Refreshed 2026-09-12 (pixel-world-at-scale Phase 1, FORMAT_VERSION
    // 42→43): World.biome_step_interval added (default/absent 1) — layout
    // growth only, trajectory byte-identical.
    // Refreshed 2026-09-25 (territory/habitat/collision layer, FORMAT_VERSION
    // 43→44): added World.territory_enabled + World.species_territories
    // (empty with the flag off). Layout growth only — trajectory proven
    // unchanged by tests/determinism.rs::*_trajectory_unchanged_by_territory_substrate.
    // Re-pinned 2026-09-26 (scenario consolidation): `affect-play.toml` was
    // retired into `tribes.toml`, and the scenario schema now defaults every
    // feature on; the flag-off engine is pinned separately by the
    // `*_trajectory_is_pinned` guards, which did not move.
    // Re-pinned 2026-09-27: the collision resolve now runs up to eight Jacobi
    // passes with a per-pass hash rebuild, a coastline slide and a settle exit
    // (was two fixed passes); every full-stack trajectory moves from tick 1.
    // Re-pinned 2026-09-28: a newborn is placed clear of both parents' bodies
    // (collision layer on) instead of on their midpoint; every full-stack
    // trajectory moves from its first birth.
    // Refreshed 2026-09-29: the collision audit (swept moves, newborn placement,
    // the hub and water pulls), the realism layers (gait, growth, turning
    // inertia, gestation, the chase and its carcass economy) and capacity from
    // food (no lineage shares, non-binding caps) — every full-stack trajectory
    // moved from tick 1; see docs/scenarios.md.
    // Re-pinned 2026-09-29 on the merge of the projectile ladder (FORMAT_VERSION
    // 45→46): Throwing Stones appended (id 22), Hafted Spears re-rooted onto it,
    // MEME_CHANNELS 32→33 — one lane per agent moves every layout hash, and
    // with `inventions_enabled` on each Communicator birth jitters one more
    // lane and the discovery table gains an era-1 candidate. Ignored by
    // default (golden validation is off); regenerated on the merged tree with
    // `UPDATE_HASHES=1 … -- --ignored golden_hashes trajectory_is_pinned`.
    &[(0, 0xc16a8d23cf4b8035), (100, 0xb954801033fad3d5), (200, 0x67151efc2614c840)];

#[test]
#[ignore = "golden validation is off (2026-09-29): run with `-- --ignored golden_hashes trajectory_is_pinned` to compare against the pins, UPDATE_HASHES=1 to re-pin"]
fn affect_play_matches_golden_hashes() {
    common::assert_golden("affect-play", SCENARIO, PLAY_GOLDEN);
}
