//! Save→load→step round-trip hardening: every subsystem must survive
//! serialization bit-identically, and the world must step identically after a
//! reload (the `#[serde(skip)]`-accumulator footgun — a skipped field that
//! feeds future ticks is invisible to `state_hash` yet breaks replay).
//!
//! One test per world (each runs the full stack — `habitat-territories` less
//! sexual dimorphism, its one documented opt-out — so every default-on
//! subsystem is warmed across the twelve), warmed enough that the subsystems'
//! state is non-trivial before saving, each guarding that its world actually
//! enables the stack (so a scenario edit silently dropping a flag fails
//! loudly). The retired files whose configuration no world carries — the
//! experiment levers, and the few states no world reaches within its warm-up —
//! round-trip as inline fixtures below. These assert *self-consistency*, not
//! pinned values — no golden hashes. See `docs/determinism-contract.md` for the
//! skip rules this suite enforces.

use anabios_core::world::World;

mod common;
use common::assert_roundtrip as roundtrip;

macro_rules! roundtrip_tests {
    ($($name:ident: $file:literal, $warm:literal, $flag:expr, $flag_name:literal;)*) => {
        $(
            #[test]
            fn $name() {
                roundtrip(include_str!($file), $warm, $flag, $flag_name);
            }
        )*
    };
}

/// Every feature knob the scenario schema defaults on is on.
fn full_stack(w: &World) -> bool {
    w.sexual_dimorphism_enabled && full_stack_but_dimorphism(w)
}

/// Every default-on knob except sexual dimorphism — the one opt-out a curated
/// world carries (`habitat-territories`: female choice sterilizes its small
/// flyers; see that file's header).
fn full_stack_but_dimorphism(w: &World) -> bool {
    w.biome_adaptation
        && w.terrain_habitat
        && w.inventions_enabled
        && w.gene_tech_coupling
        && w.gene_requirements
        && w.cognition_enabled
        && w.affect_enabled
        && w.living_biome
        && w.season_period > 0
        && w.nutrient_variation
        && w.soil_fertility
        && w.resources_enabled
        && w.conserve_goods_on_death
        && w.disasters_enabled
        && w.war_enabled
        && w.settlement_enabled
        && w.domestication_enabled
        && w.knowledge_enabled
        && w.practices_enabled
        && w.basic_needs_enabled
        && w.mate_seeking_enabled
        && w.territory_enabled
        && w.gait_enabled
        && w.turning_enabled
        && w.gestation_enabled
        && w.disease_enabled
        && w.anthro_race_enabled
        && w.repro_biased_learning
        && w.growth_enabled
        && w.chase_enabled
}

// Warm-ups: a world keeps the longest warm-up of the retired rows it absorbed
// (so every absorbed subsystem is past its old detector windows), 120 when it
// absorbed none.
roundtrip_tests! {
    minimal_roundtrip:
        "../../../scenarios/minimal.toml", 120, full_stack, "minimal (full stack)";
    predator_prey_roundtrip:
        // Absorbed biome-adaptation (400) and foraging-selection (400).
        "../../../scenarios/predator-prey.toml", 400, full_stack, "predator-prey (full stack)";
    speciation_roundtrip:
        "../../../scenarios/speciation.toml", 120, full_stack, "speciation (full stack)";
    tribes_roundtrip:
        // Absorbed inventions (500), cognitive-coevolution (400), war (600),
        // dimorphism, domestication, disease, anthro-race (400), basic-needs
        // (600), knowledge-ratchet, tech-gene-coupling and the affect files —
        // affect-showcase's 800 carries the affect detectors past their windows.
        "../../../scenarios/tribes.toml", 800,
        |w: &World| full_stack(w) && !w.culture_roots.is_empty(),
        "tribes (full stack + culture_roots)";
    markets_roundtrip:
        // Absorbed settlement, biome-trade, geographic-trade, unilateral-trade (400).
        "../../../scenarios/markets.toml", 400, full_stack, "markets (full stack)";
    habitat_territories_roundtrip:
        // Warm past two species steps (ticks 0/200/400) so territory EMA state
        // is non-trivial when saved.
        "../../../scenarios/habitat-territories.toml", 420,
        |w: &World| full_stack_but_dimorphism(w) && !w.sexual_dimorphism_enabled,
        "habitat-territories (full stack but dimorphism)";
    grand_theater_roundtrip:
        // The strongest single guard: grand-theater warms every subsystem at
        // once, including both experiment levers it opts into.
        "../../../scenarios/grand-theater.toml", 300,
        |w: &World| full_stack(w) && w.env_period > 0 && w.climate_drift_rate > 0.0,
        "grand-theater (everything on)";
    out_of_africa_saga_roundtrip:
        "../../../scenarios/out-of-africa-saga.toml", 120, full_stack,
        "out-of-africa-saga (full stack)";
    out_of_africa_earth_roundtrip:
        "../../../scenarios/out-of-africa-earth.toml", 300,
        |w: &World| full_stack(w) && w.biome.res == 256,
        "out-of-africa-earth (from_earth field + full stack)";
    sandbox_roundtrip:
        // Absorbed living-sandbox-coevolution (400) and sandbox-large (300).
        "../../../scenarios/sandbox.toml", 400, full_stack, "sandbox (full stack)";
    riverlands_roundtrip:
        "../../../scenarios/riverlands.toml", 200,
        |w: &World| full_stack(w) && !w.lineage_caps.is_empty(),
        "riverlands (full stack + max_share)";
    huge_steppe_roundtrip:
        "../../../scenarios/huge-steppe.toml", 120,
        |w: &World| full_stack(w) && w.world_size == 8192.0,
        "huge-steppe (8192 world, full stack)";
}

/// One `#[test]` per inline fixture, so nextest can shard them: as two tests
/// the nine ran back to back, and the experiment group was the longest single
/// test in the debug suite.
macro_rules! fixture_roundtrip_tests {
    ($($name:ident: $src:expr, $warm:literal, $flag:expr, $what:literal;)*) => {
        $(
            #[test]
            fn $name() {
                super::roundtrip(&$src, $warm, $flag, $what);
            }
        )*
    };
}

/// The retired experiment files (`scenarios/experiments/`), kept verbatim as
/// inline fixtures: each carries a lever or a combination no world has.
/// Warm-ups are the retired rows' own.
mod experiment_fixtures {
    use super::common::fixtures::*;
    use anabios_core::world::World;

    fixture_roundtrip_tests! {
        // A default-size carrier scenario rather than vast-steppe: instantiating
        // the 2048^2 grid is the slowest thing in the suite (minutes in the
        // debug coverage shards) and the flag's persistence needs only one
        // full biome_step_interval=4 period (effective cadence 40 ticks).
        biome_step_interval_roundtrip:
            biome_step_interval_flag_off(), 120,
            |w: &World| w.biome_step_interval > 1, "biome_step_interval";
        dit_env_slow_roundtrip:
            dit_env_slow_flag_off(), 300, |w: &World| w.env_period > 0, "env_period";
        drifting_climate_roundtrip:
            drifting_climate_flag_off(), 400,
            |w: &World| w.climate_drift_rate > 0.0, "climate_drift_rate";
        gene_requirements_roundtrip:
            gene_requirements_flag_off(), 500, |w: &World| w.gene_requirements, "gene_requirements";
        o1_lever_practices_off_roundtrip:
            o1_lever_practices_off_flag_off(), 300,
            |w: &World| !w.practices_enabled && w.cognition_enabled,
            "practices_enabled(off variant)";
        o2_payoff_biased_learning_roundtrip:
            o2_payoff_biased_learning_flag_off(), 300,
            |w: &World| w.payoff_biased_learning, "payoff_biased_learning";
        o3_repro_biased_learning_roundtrip:
            o3_repro_biased_learning_flag_off(), 300,
            |w: &World| w.repro_biased_learning, "repro_biased_learning";
    }
}

/// Retired root files whose round-tripped state the absorbing world does not
/// reach: the `unilateral_trade` lever stays off in every world (`markets`
/// absorbed the file's founders), and `tribes` (which absorbed
/// knowledge-ratchet) holds no Writing within its warm-up, so its
/// `knowledge_by_species` is empty when saved. The knowledge fixture keeps the
/// pairing with `knowledge.rs::knowledge_accrues_per_species`, which pins its
/// non-triviality at this warm-up. Warm-ups are the retired rows' own.
mod retired_state_fixtures {
    use super::common::fixtures::*;
    use anabios_core::world::World;

    fixture_roundtrip_tests! {
        unilateral_trade_roundtrip:
            unilateral_trade_flag_off(), 400,
            |w: &World| w.unilateral_trade && w.conserve_goods_on_death,
            "unilateral_trade+conserve_goods_on_death";
        knowledge_ratchet_roundtrip:
            knowledge_ratchet_flag_off(), 300, |w: &World| w.knowledge_enabled, "knowledge_enabled";
    }
}

/// Gestation: the `gestation_left` countdown and the `pending_litter` store
/// (the litter's drawn genomes, modules, programs and sexes, plus the father
/// bookkeeping) are conception RNG output and must survive a snapshot — a
/// reloaded world that dropped them would deliver nothing where the live one
/// delivers a litter. Hand-built so the state is provably non-trivial when
/// saved: a well-fed herd on grass conceives within a few ticks, and the
/// save lands mid-term. The twelve worlds above round-trip it too (the knob
/// is on in all of them), but a cap-bound world's warm-up could in principle
/// end with no pregnancy in flight.
#[test]
fn gestation_roundtrip_with_litters_in_flight() {
    use anabios_core::biome::TerrainType;
    use anabios_core::genome::{Genome, GenomeSlot};
    use anabios_core::prelude_test::Vec2;
    use anabios_core::reproduce::GESTATION_TICKS;
    let mut w = World::new(11);
    w.gestation_enabled = true;
    w.territory_enabled = true;
    for c in w.biome.cells.iter_mut() {
        c.terrain = TerrainType::Grass;
        c.plant_biomass = 1.0;
    }
    let mut g = Genome::neutral();
    g.set(GenomeSlot::ReproductionThreshold, 0.3);
    g.set(GenomeSlot::Size, 0.8); // litters of two
    for k in 0..24 {
        let id =
            w.spawn_agent(Vec2::new(300.0 + (k % 6) as f32 * 1.2, 300.0 + (k / 6) as f32 * 1.2), g);
        w.agents.energy[id as usize] = anabios_core::agent::SPAWN_ENERGY * 2.0;
    }
    common::run(&mut w, GESTATION_TICKS as u64 / 2);
    let pregnant =
        w.agents.iter_alive().filter(|&id| w.agents.gestation_left[id as usize] > 0).count();
    assert!(pregnant > 0, "warm-up must leave litters in flight (got none)");
    assert!(
        w.agents.iter_alive().all(|id| {
            (w.agents.gestation_left[id as usize] > 0)
                == w.agents.pending_litter[id as usize].is_some()
        }),
        "a countdown and a stored litter go together"
    );
    common::assert_roundtrip_world(&mut w, "gestation_enabled with litters in flight");
    // And the reloaded world delivers them exactly when the live one does.
    let bytes = anabios_core::snapshot::save_to_bytes(&w).expect("save");
    let mut reloaded = anabios_core::snapshot::load_from_bytes(&bytes).expect("load");
    common::run(&mut w, GESTATION_TICKS as u64);
    common::run(&mut reloaded, GESTATION_TICKS as u64);
    assert_eq!(
        anabios_core::snapshot::state_hash(&w),
        anabios_core::snapshot::state_hash(&reloaded),
        "births after a reload must match the continuous run"
    );
}

/// Territory layer on a NON-default world extent. `collision_spatial` is
/// `#[serde(skip)]`, so a loaded world comes back with serde's `Default` hash
/// (1024-wide, res 64) — and a 256-wide world's collision grid ALSO resolves
/// to res 64, so only the extent check in `collision::rebuild_hash` can heal
/// it; a stale extent would bucket a seam-straddling pair in unrelated cells
/// and the resolve would diverge after the load. No shipped scenario has
/// this extent, so the world is hand-built here rather than in the table.
#[test]
fn territory_roundtrip_on_a_256_world_heals_the_collision_hash() {
    use anabios_core::biome::TerrainType;
    use anabios_core::genome::Genome;
    use anabios_core::prelude_test::Vec2;
    let mut w = World::with_dims(7, 256.0, 32, 16);
    w.territory_enabled = true;
    for c in w.biome.cells.iter_mut() {
        c.terrain = TerrainType::Grass;
    }
    // A pair straddling the x = 256 seam plus a small herd.
    w.spawn_agent(Vec2::new(255.8, 128.0), Genome::neutral());
    w.spawn_agent(Vec2::new(0.1, 128.0), Genome::neutral());
    for k in 0..20 {
        w.spawn_agent(Vec2::new(100.0 + k as f32 * 0.7, 100.0), Genome::neutral());
    }
    common::run(&mut w, 30);
    common::assert_roundtrip_world(&mut w, "territory_enabled on a 256-wide world");
}

/// Turning inertia: the `heading` column is path-dependent state (each tick
/// turns it from its previous value), so it must be serialized, not skipped —
/// a reloaded world that forgot its facings would steer differently on the
/// very next tick. Hand-built with fixed-intent programs and saved mid-turn,
/// so the column is provably non-trivial (no slot at the spawn constant) when
/// it round-trips; the twelve worlds above round-trip it under the full stack.
#[test]
fn heading_survives_a_roundtrip_and_the_reloaded_world_keeps_turning() {
    use anabios_core::genome::Genome;
    use anabios_core::heading::SPAWN_HEADING;
    use anabios_core::prelude_test::Vec2;
    use anabios_core::program::{Node, Program};
    let mut w = World::new(11);
    w.turning_enabled = true;
    for k in 0..12u32 {
        let id = w.spawn_agent(Vec2::new(300.0 + k as f32 * 4.0, 500.0), Genome::neutral());
        // Half want due west, half due north: every heading leaves +x.
        let (v, node) =
            if k % 2 == 0 { (-1.0, Node::MoveTowardX) } else { (1.0, Node::MoveTowardY) };
        w.agents.program[id as usize] = Program::from_slice(&[Node::Const(v), node]);
    }
    // Two ticks: 1.2 rad into a 1.57 / 3.14 rad turn — every facing mid-turn.
    common::run(&mut w, 2);
    assert!(
        w.agents.iter_alive().all(|id| w.agents.heading[id as usize] != SPAWN_HEADING),
        "warm-up must leave every heading off the spawn constant"
    );
    let bytes = anabios_core::snapshot::save_to_bytes(&w).expect("save");
    let reloaded = anabios_core::snapshot::load_from_bytes(&bytes).expect("load");
    assert_eq!(w.agents.heading, reloaded.agents.heading, "heading column must persist");
    common::assert_roundtrip_world(&mut w, "turning_enabled (heading column mid-turn)");
}
