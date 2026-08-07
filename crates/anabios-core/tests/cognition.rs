//! End-to-end determinism for the flag-ON cognitive gene–culture scenario.
//! `determinism.rs` only locks the flag-OFF minimal scenario and `inventions.rs`
//! the inventions demo; this pins the cognitive layer's actual behavior (IQ
//! development, IQ-gated acquisition, practice discovery/spread, reproductive
//! effects) so it cannot drift silently.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;

const SCENARIO: &str = include_str!("../../../scenarios/cognitive-coevolution.toml");

#[test]
fn cognitive_scenario_parses_with_both_flags() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse cognitive scenario");
    assert!(s.cognition_enabled);
    assert!(s.inventions_enabled);
}

#[test]
fn cognitive_scenario_is_self_consistent() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse cognitive scenario");
    let run = |ticks: u64| {
        let mut w = s.instantiate();
        for _ in 0..ticks {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(300), run(300), "same seed + flags on → bit-identical");
}

/// Pinned golden for the flag-ON cognitive scenario. Regenerate deliberately
/// with `UPDATE_HASHES=1` (prints new values) whenever a cognitive change is
/// intentional.
// Refreshed 2026-07-19: inbreeding strengthened into a real selector — kin-
// seeking mate bias (`find_mate`) + a viability (stillbirth) cost on close-kin
// offspring. A behavior change (not layout): an inbreeding holder reproduces
// after tick 0, so ticks 100/300 moved while tick 0 held. Flag-off (minimal /
// inventions) goldens are unaffected — the mechanic is cognition-gated.
// Refreshed 2026-07-19 (2): IQ nutrition channel now samples LOCAL BIOME FOOD
// (plant_biomass / carrying_capacity) instead of the spawn-energy-buffered
// energy level, so the "growing environment" actually shapes realized IQ.
// Behavior change: juvenile IQ development differs from tick 1 on, so ticks
// 100/300 moved (tick 0 holds — no development yet). Cognition-gated, so
// minimal / inventions are unaffected.
// Refreshed 2026-07-19 (merge): merged with the biome-trade-goods + geographic
// trade routes branch (FORMAT_VERSION 8). Those features are opt-in and off in
// this scenario, so cognition behavior/trajectory is unchanged — the moved
// hashes are pure serialized-layout growth from the merged-in fields.
// Refreshed 2026-07-19 (weapons): Spines/Jaws module types join the
// structural-mutation pool (random_any 9 → 11 types) — trajectory drift only.
// Refreshed 2026-07-23 (E3): CodexState cycle/plateau/cascade scratch
// (FORMAT_VERSION 8→9) — layout growth only, behavior unchanged.
// Refreshed 2026-07-23 (E4): BiomeCell.succession + World.disasters (FORMAT_VERSION
// 9→10) — layout growth only, flag off.
// Refreshed 2026-07-23 (E5): genome moments + trait-detector scratch
// (FORMAT_VERSION 10→11) — layout growth only.
// Refreshed 2026-07-23 (E6): CombatHit context + signature scratch
// (FORMAT_VERSION 11→12) — observability only.
// Refreshed 2026-07-23 (E7): hostility records + SenseHostility behind
// war_enabled (FORMAT_VERSION 12→13) — layout growth only, flag off.
// Refreshed 2026-07-23 (E8): anchors + harvest exp + market field
// (FORMAT_VERSION 13→14) — layout growth only, flags off.
// Refreshed 2026-07-23 (E6+E7 merge): merged e7 in — carries the e6
// still_ticks/prev_desired_direction serialization (FORMAT_VERSION 14→15),
// layout growth only.
const COGNITIVE_GOLDEN: &[(u64, u64)] =
    // Refreshed 2026-07-23 (E9): meme-variant registry + meme_lineage
    // (FORMAT_VERSION 15→16) — layout growth only, fidelity gated off here.
    // Refreshed 2026-07-24 (E10): World.climate_drift_rate (FORMAT_VERSION 16→17),
    // drift 0.0 here — layout growth only, behavior byte-identical.
    // Refreshed 2026-07-24 (E11): maladapt scratch + MaladaptationLag
    // (FORMAT_VERSION 17→18), env_period == 0 here — layout growth only.
    // Refreshed 2026-07-25 (merge of TG1 + nutrient/fertility, FORMAT_VERSION 18→19):
    // World.gene_tech_coupling + BiomeCell.{nutrient_quality,fertility} +
    // World.{nutrient_variation,soil_fertility}. All flags off here, so cognition
    // behavior is byte-identical — pure serialized-layout growth.
    // Refreshed 2026-07-27 (E12 sexual dimorphism, FORMAT_VERSION 19→20):
    // AgentBuffers.sex + World.sexual_dimorphism_enabled + codex dimorphism
    // latches. Flag off here — zero extra draws, identity factors; pure
    // serialized-layout growth.
    // Refreshed 2026-07-27 (climate worldgen merged onto E13, FORMAT_VERSION 22):
    // new Whittaker terrain reshapes the agents' environment — a genuine
    // trajectory change — on top of E13's livestock_of / domestication_enabled
    // layout. Regenerated from the merged code.
    // Refreshed 2026-07-31 (invention requirements + full affinities,
    // FORMAT_VERSION 22→23): World.gene_requirements added (off here); all
    // inventions carry affinities and every buff site has a coupled variant,
    // but gene_tech_coupling is off in this scenario so behavior is
    // byte-identical — pure serialized-layout growth.
    // Refreshed 2026-08-02 (supply-side trade fix, FORMAT_VERSION 23→24):
    // World.conserve_goods_on_death added (flag off here) — layout growth
    // only, trajectory byte-identical.
    // Refreshed 2026-08-03 (maladaptive-practices toggle, FORMAT_VERSION 24→25):
    // World.practices_enabled added, defaulting true — practices still run in
    // this cognition-on scenario, so behavior is byte-identical; only the
    // serialized layout grew.
    // Refreshed 2026-08-03 (main knowledge layout, FORMAT_VERSION →26):
    // World.knowledge_enabled + CodexState knowledge fields, off in this scenario
    // ⇒ knowledge_step early-returns, trajectory byte-identical, layout growth only.
    // Refreshed 2026-08-03 (M-A+M-B affect layer, FORMAT_VERSION 26→27): affect
    // column + flag + temperament + EventType::MassFright, all off/gated here ⇒
    // byte-identical atop the v26 knowledge layout; regenerated on the merged tree.
    // Refreshed 2026-08-04 (affect M-D, FORMAT_VERSION 27→28): added
    // AgentBuffers.affect_prev_crowding serialized column. affect_enabled off in
    // this scenario ⇒ develop_all no-op, column stays 0.0 — trajectory
    // byte-identical, only the serialized layout grew.
    // Refreshed 2026-08-04 (M-F affect observability, FORMAT_VERSION 28→29): new
    // CodexState affect-detector fields ({frenzy_active, rage_streak,
    // rage_active, fear_count_history, cascade_active, grief_active}); affect_enabled
    // off in this scenario ⇒ detectors never fire — layout growth only.
    &[(0, 0x17059ba9dea8529a), (100, 0x8f5ba1f0dc208a63), (300, 0x67032f0377569454)];

#[test]
fn cognitive_scenario_matches_golden_hashes() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse cognitive scenario");
    let mut w = s.instantiate();
    let max_tick = COGNITIVE_GOLDEN.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut idx = 0;
    let mut observed: Vec<(u64, u64)> = Vec::new();
    while w.tick <= max_tick {
        while idx < COGNITIVE_GOLDEN.len() && COGNITIVE_GOLDEN[idx].0 == w.tick {
            observed.push((w.tick, state_hash(&w)));
            idx += 1;
        }
        if w.tick == max_tick {
            break;
        }
        step(&mut w);
    }
    if std::env::var("UPDATE_HASHES").is_ok() {
        println!("// regenerated cognitive hashes:");
        for (t, h) in &observed {
            println!("    ({t}, 0x{h:016x}),");
        }
        return;
    }
    for ((exp_tick, exp_hash), (got_tick, got_hash)) in COGNITIVE_GOLDEN.iter().zip(&observed) {
        assert_eq!(exp_tick, got_tick, "tick mismatch");
        assert_eq!(
            *exp_hash, *got_hash,
            "cognitive hash drift at tick {exp_tick}: expected 0x{exp_hash:016x}, got 0x{got_hash:016x}.\n\
             If intentional, rerun with UPDATE_HASHES=1 and copy the printed values.",
        );
    }
}

/// The demo's promise: with cognition on, both beneficial tech and maladaptive
/// practices appear in the codex event stream within a few hundred ticks.
#[test]
fn cognitive_scenario_produces_invention_and_practice_events() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse cognitive scenario");
    let mut w = s.instantiate();
    let mut saw_invention = false;
    let mut saw_practice = false;
    for _ in 0..5000 {
        step(&mut w);
        for ev in w.codex.drain_events() {
            match ev.event_type {
                EventType::InventionDiscovered | EventType::InventionAdopted => {
                    saw_invention = true
                }
                EventType::PracticeDiscovered | EventType::PracticeAdopted => saw_practice = true,
                _ => {}
            }
        }
        if saw_invention && saw_practice {
            break;
        }
    }
    assert!(saw_invention, "cognitive scenario should climb the tech tree");
    assert!(saw_practice, "cognitive scenario should surface a maladaptive practice");
}
