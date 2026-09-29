//! End-to-end determinism for the cognitive gene–culture layer on `tribes`
//! (which absorbed `cognitive-coevolution.toml`; the full stack keeps both
//! `cognition_enabled` and `inventions_enabled` on). This pins the cognitive
//! layer's actual behavior (IQ development, IQ-gated acquisition, practice
//! discovery/spread, reproductive effects) so it cannot drift silently.

use anabios_core::codex::EventType;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;

mod common;

const SCENARIO: &str = include_str!("../../../scenarios/tribes.toml");

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
    // Refreshed 2026-08-07 (O2 payoff-biased learning, FORMAT_VERSION 29→30):
    // World.payoff_biased_learning layout growth only (off here).
    // Refreshed 2026-08-11 (apes-only inventions): culture cohort (innovator/
    // traditionalist) reclassed omnivore (ape) so it can carry the tech tree;
    // the diet change shifts feeding ecology and the invention-race trajectory.
    // Regenerated on the gated tree.
    // Refreshed 2026-08-19 (anthropogenic arms race, FORMAT_VERSION →34):
    // World.{anthro_race_enabled,culture_roots} + CodexState hunted fields —
    // layout growth only (off here).
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
    // AgentBuffers.mood column. affect_enabled is off here ⇒ the column stays
    // CONTENT and apply_mood is exact identity — layout growth only,
    // trajectory byte-identical.
    // Refreshed 2026-09-05 (sparse-lineage breeding, FORMAT_VERSION 38→39):
    // World.lineage_caps + World.mate_seeking_enabled. Both absent/off here ⇒
    // layout growth only, trajectory byte-identical.
    // Refreshed 2026-09-05 (tuned PERCEPTION_ENERGY_COST to 0.005).
    // Refreshed 2026-09-07 (military branch, FORMAT_VERSION 39→40): the meme
    // vector widened and four inventions were appended, growing the discovery
    // probability table layout for every scenario. inventions_enabled is on
    // here, so the wider candidate pool genuinely reshapes discovery/copy
    // draws — a real trajectory change, not pure layout growth.
    // Refreshed 2026-09-10 (X1 invention expansion, FORMAT_VERSION 40→41):
    // meme channels widened 24->32 and INVENTION_COUNT grew 14->20;
    // inventions_enabled is on here so the wider candidate pool again
    // reshapes discovery/copy draws — a real trajectory change.
    // Refreshed 2026-09-11 (X2 Wells+Vaccination, FORMAT_VERSION 41→42):
    // invention count 20->22 shifted the discovery table and practice
    // channels; inventions_enabled is on here so the wider candidate pool
    // again reshapes discovery/copy draws.
    // Refreshed 2026-09-12 (pixel-world-at-scale Phase 1, FORMAT_VERSION
    // 42→43): World.biome_step_interval added (default/absent 1) — layout
    // growth only, trajectory byte-identical.
    // Refreshed 2026-09-25 (territory/habitat/collision layer, FORMAT_VERSION
    // 43→44): added World.territory_enabled + World.species_territories
    // (empty with the flag off). Layout growth only — trajectory proven
    // unchanged by tests/determinism.rs::*_trajectory_unchanged_by_territory_substrate.
    // Re-pinned 2026-09-26 (scenario consolidation): `cognitive-coevolution.toml`
    // was retired into `tribes.toml`, and the scenario schema now defaults every
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
    &[(0, 0xc16a8d23cf4b8035), (100, 0xb954801033fad3d5), (300, 0x20881a45a9ef0158)];

#[test]
#[ignore = "golden validation is off (2026-09-29): run with `-- --ignored golden_hashes trajectory_is_pinned` to compare against the pins, UPDATE_HASHES=1 to re-pin"]
fn cognitive_scenario_matches_golden_hashes() {
    common::assert_golden("cognitive", SCENARIO, COGNITIVE_GOLDEN);
}

/// The demo's promise: with cognition on, both beneficial tech and maladaptive
/// practices appear in the codex event stream within a few hundred ticks.
/// Only inventions nobody held at t0 count: `tribes` seeds Stone Tools, whose
/// Discovered / Adopted latches fire on the seeding at tick 0 (the first
/// climbed invention, Fire, arrives at tick 947 on this seed).
#[test]
fn cognitive_scenario_produces_invention_and_practice_events() {
    let s = Scenario::parse_toml(SCENARIO).expect("parse cognitive scenario");
    // No population cap here: the loop stops at the first climbed invention,
    // and a lower cap only delays it (tick 1159 under a 500 cap set after
    // `instantiate`, 2310 under one set before it, 947 uncapped).
    let mut w = s.instantiate();
    // Growth (2026-09-29) is off here: `tribes` seeds its founders at age 0
    // with `lifespan_bias = 1.0`, so under the knob nobody breeds before tick
    // 750 and the innovator lineage stays at its 30 founders for that long
    // (about 80-110 afterwards, against 90-175 from tick 250 without it);
    // discovery is a 3e-5-per-agent-tick draw, so the first climbed
    // invention moves past the 5000-tick window. The claim under test is
    // the cognitive layer's, and the knob's own tests pin growth.
    w.growth_enabled = false;
    let seeded = common::inventions_held(&w);
    let mut saw_invention = false;
    let mut saw_practice = false;
    for _ in 0..5000 {
        step(&mut w);
        for ev in w.codex.drain_events() {
            match ev.event_type {
                EventType::InventionDiscovered | EventType::InventionAdopted
                    if !seeded.contains(&(ev.value as usize)) =>
                {
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

/// Realized IQ actually develops above zero in the cognitive scenario. This is
/// the non-triviality precondition that keeps `tribes_roundtrip` (in
/// `save_load_roundtrip.rs`) honest — a round-trip over an all-zero IQ column
/// would pass vacuously.
#[test]
fn realized_iq_develops_above_zero() {
    let w = common::world_after(SCENARIO, 150);
    assert!(
        w.agents.iter_alive().any(|id| w.agents.iq[id as usize] > 0.0),
        "realized IQ should have developed above zero",
    );
}
