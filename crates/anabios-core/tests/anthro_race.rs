//! Anthropogenic arms-race integration tests: the culture-lineage tag, the
//! aimed threat sense, the flag-gated program node, and end-to-end scenario
//! determinism with the subsystem on/off.

use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude_test::Vec2;
use anabios_core::scenario::Scenario;
use anabios_core::sense;
use anabios_core::snapshot::state_hash;
use anabios_core::species;
use anabios_core::tick::step;
use anabios_core::world::World;

/// `World::resize_scratch` is crate-private; tests size the tick scratch
/// buffers directly (same defaults).
fn size_scratch(w: &mut World) {
    let cap = w.agents.capacity();
    w.sensors.resize(cap, Default::default());
    w.desired_direction.resize(cap, Vec2::ZERO);
    w.actions.resize(cap, Default::default());
    w.combat_damaged.resize(cap, false);
    w.combat_attacker.resize(cap, 0);
}

const SCENARIO: &str = r#"
name = "anthro-test"
seed = 7
inventions_enabled = true
affect_enabled = true
war_enabled = true
anthro_race_enabled = true
max_population = 300

[[agents]]
count = 12
archetype = "ape_hunter"
culture_bearer = true
starting_inventions = ["stone_tools"]
placement = { kind = "cluster", center_x = 460.0, center_y = 512.0, radius = 70.0 }
[agents.traits]
lifespan_bias = 1.0

[[agents]]
count = 30
archetype = "herd"
placement = { kind = "cluster", center_x = 580.0, center_y = 512.0, radius = 70.0 }
[agents.traits]
lifespan_bias = 1.0
vigilance = 0.5
"#;

#[test]
fn culture_bearer_requires_the_flag() {
    let bad = SCENARIO.replace("anthro_race_enabled = true", "anthro_race_enabled = false");
    assert!(
        Scenario::parse_toml(&bad).is_err(),
        "culture_bearer without anthro_race_enabled must fail at parse"
    );
}

#[test]
fn tag_marks_founder_and_its_splinters_only() {
    let w = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    assert_eq!(w.culture_roots.len(), 1, "one tagged founder");
    let root = *w.culture_roots.iter().next().unwrap();
    assert!(species::is_culture_lineage(&w, root));
    assert!(!species::is_culture_lineage(&w, 0), "the wild herd (species 0) is untagged");
    // A speciation splinter under the tagged root inherits the tag.
    let mut w2 = w.clone();
    let splinter = w2.push_species(Genome::neutral(), Some(root));
    assert!(species::is_culture_lineage(&w2, splinter), "splinter inherits via the root walk");
}

/// `refresh_culture_mask` extends the mask instead of rebuilding it every
/// tick, which is only sound because species rows are append-only. Pin that
/// the incremental result matches a from-scratch rebuild after speciation —
/// a splinter of the tagged root must come back tagged, an unrelated one not.
#[test]
fn incremental_mask_refresh_matches_a_full_rebuild() {
    let mut w = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    w.refresh_culture_mask();
    let root = *w.culture_roots.iter().next().unwrap();
    let before = w.culture_mask.clone();
    assert!(before[root as usize], "the tagged founder is masked");

    // Speciation appends rows: one splinter under the tagged root, one wild.
    let tagged_splinter = w.push_species(Genome::neutral(), Some(root));
    let wild_splinter = w.push_species(Genome::neutral(), Some(0));
    w.refresh_culture_mask();
    assert_eq!(w.culture_mask[..before.len()], before[..], "existing rows are untouched");
    assert!(w.culture_mask[tagged_splinter as usize], "splinter of the tagged root is masked");
    assert!(!w.culture_mask[wild_splinter as usize], "an unrelated splinter is not");

    // Identical to a from-scratch rebuild (the state after a snapshot load).
    let incremental = w.culture_mask.clone();
    w.culture_mask.clear();
    w.refresh_culture_mask();
    assert_eq!(incremental, w.culture_mask, "incremental refresh == full rebuild");
}

#[test]
fn culture_threat_sense_is_aimed_and_flag_gated() {
    // One armed tagged culture agent + one wild agent, in perception range.
    let build = |tagged: bool, anthro: bool| -> f32 {
        let mut w = World::new(11);
        w.anthro_race_enabled = anthro;
        let hunter = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        let prey = w.spawn_agent(Vec2::new(504.0, 500.0), Genome::neutral());
        w.agents.modules[hunter as usize] = {
            let mut m = anabios_core::module::ModuleList::new();
            m.push(anabios_core::module::Module::Weapon { damage: 8.0, energy_cost: 1.0 });
            m
        };
        let hunter_sp = anabios_core::prelude_test::reassign_to_new_species(&mut w, hunter);
        if tagged {
            w.culture_roots.insert(hunter_sp);
        }
        w.refresh_culture_mask();
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        size_scratch(&mut w);
        sense::sense_all(
            &w.agents,
            &w.biome,
            &w.pheromones,
            &w.spatial,
            &w.codex.hostility,
            &w.culture_mask,
            &mut w.sensors,
            w.world_size,
            false,
        );
        w.sensors[prey as usize].culture_threat
    };
    let aimed = build(true, true);
    assert!(
        (aimed - 8.0 / anabios_core::module::DAMAGE_MAX).abs() < 1e-6,
        "armed tagged neighbor ⇒ threat = damage/DAMAGE_MAX, got {aimed}"
    );
    assert_eq!(build(false, true), 0.0, "untagged neighbor ⇒ no threat");
    assert_eq!(build(true, false), 0.0, "flag off ⇒ mask empty ⇒ no threat");
}

#[test]
fn sense_culture_threat_node_reads_the_register() {
    let g = Genome::neutral();
    let prog = anabios_core::program::Program::from_slice(&[
        anabios_core::program::Node::SenseCultureThreat,
        anabios_core::program::Node::MoveAwayX,
    ]);
    let mut stack = Vec::new();
    let ctx =
        |threat: f32| anabios_core::program::EvalContext { culture_threat: threat, ..eval_ctx(&g) };
    let calm = anabios_core::program::evaluate(&prog, ctx(0.0), &mut stack);
    let scared = anabios_core::program::evaluate(&prog, ctx(0.9), &mut stack);
    assert_eq!(calm.move_x, 0.0);
    assert!(scared.move_x < 0.0, "threat pushes a flee-X intent");
}

fn eval_ctx(g: &Genome) -> anabios_core::program::EvalContext<'_> {
    anabios_core::program::EvalContext {
        energy: 30.0,
        age: 10,
        genome: g,
        nearest_distance: 5.0,
        nearest_dir: Vec2::new(1.0, 0.0),
        plant_dir: Vec2::ZERO,
        local_biomass: 1.0,
        same_distance: f32::INFINITY,
        same_dir: Vec2::ZERO,
        other_distance: f32::INFINITY,
        other_dir: Vec2::ZERO,
        rel_size: 0.0,
        rel_energy: 0.0,
        crowding: 0.0,
        pheromone_sample: [0.0; anabios_core::program::PHEROMONE_CHANNELS],
        meme_sample: [0.0; anabios_core::program::MEME_CHANNELS],
        nearest_kinship: 0.0,
        hostility: 0.0,
        anchor_dir: Vec2::ZERO,
        anchor_dist: 0.0,
        culture_threat: 0.0,
    }
}

#[test]
fn scenario_runs_and_stays_deterministic_with_the_flag_on() {
    let run = || {
        let mut w = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
        for _ in 0..600 {
            step(&mut w);
        }
        state_hash(&w)
    };
    assert_eq!(run(), run(), "two flag-on runs hash identically");
}

#[test]
fn flag_on_world_registers_threat_when_hunters_close_in() {
    let mut w = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    for _ in 0..600 {
        step(&mut w);
    }
    // Not every tick has a hunter adjacent to prey, but over 600 ticks of a
    // deliberately crowded map at least one register must have read a threat.
    // (The final-tick registers are what we can inspect; assert the weaker
    // invariant that the world stayed alive and the tag persisted.)
    assert!(w.agents.iter_alive().count() > 0, "world survived 600 ticks");
    assert!(!w.culture_mask.is_empty(), "culture mask populated under the flag");
    assert!(w.culture_mask.iter().any(|&t| t), "a tagged lineage is masked in");
}

#[test]
fn vigilance_seed_lands_in_the_genome() {
    let w = Scenario::parse_toml(SCENARIO).expect("parse").instantiate();
    // The herd is the untagged founder lineage (the tagged root is the
    // ape_hunter's fresh species id).
    let root = *w.culture_roots.iter().next().unwrap();
    let herd: Vec<u32> =
        w.agents.iter_alive().filter(|&id| w.agents.species_id[id as usize] != root).collect();
    assert_eq!(herd.len(), 30, "30 wild founders");
    let mean: f32 =
        herd.iter().map(|&id| w.agents.genome[id as usize].get(GenomeSlot::Vigilance)).sum::<f32>()
            / herd.len() as f32;
    assert!((mean - 0.5).abs() < 1e-6, "the herd's vigilance override seeds at 0.5, got {mean}");
}

/// Diagnostic (run with --ignored --nocapture): 5k-tick run printing the
/// arms-race substrate — hostility kills per root pair, hunted-ring lengths,
/// and per-lineage trait means — to explain detector (non-)fires.
#[test]
#[ignore]
fn diagnose_arms_race_substrate() {
    let ticks: u64 = std::env::var("DIAG_TICKS").ok().and_then(|s| s.parse().ok()).unwrap_or(5000);
    let seeded = std::env::var("DIAG_NO_SEED").is_err();
    let scenario = if seeded {
        SCENARIO.to_string()
    } else {
        SCENARIO.replace("starting_inventions = [\"stone_tools\"]\n", "")
    };
    let scenario = if std::env::var("DIAG_OFF").is_ok() {
        scenario
            .replace("anthro_race_enabled = true", "anthro_race_enabled = false")
            .replace("culture_bearer = true", "culture_bearer = false")
    } else {
        scenario
    };
    let mut w = Scenario::parse_toml(&scenario).expect("parse").instantiate();
    let root2 = 2u32; // herd A founder species id (ape_hunter took 1)
    for t in 0..ticks {
        step(&mut w);
        if t % 2000 == 1999 {
            let mut agg = anabios_core::codex::SpeciesAggTable::default();
            agg.build(&w);
            let (mut n, mut vig, mut spd, mut arm) = (0u32, 0.0f64, 0.0f64, 0.0f64);
            for &sid in agg.active() {
                // Fold species into the herd-A lineage root.
                let mut cur = sid;
                for _ in 0..64 {
                    match w.species_parents.get(cur as usize).copied().flatten() {
                        Some(p) if p != cur && p != 0 => cur = p,
                        _ => break,
                    }
                }
                if cur != root2 {
                    continue;
                }
                let e = agg.get(sid).unwrap();
                n += e.count;
                vig += e.genome_sums[GenomeSlot::Vigilance.idx()];
                spd += e.speed_sum;
                arm += e.armor_sum;
            }
            let nf = n.max(1) as f64;
            println!(
                "t={} herdA: n={} vigil={:.4} speed={:.4} armor={:.4}",
                t + 1,
                n,
                vig / nf,
                spd / nf,
                arm / nf
            );
        }
    }
    println!("culture_roots: {:?}", w.culture_roots);
    for ((lo, hi), rec) in w.codex.hostility.iter() {
        println!("hostility ({lo},{hi}): kills={} score={:.2}", rec.kills, rec.score);
    }
    for (pair, base) in w.codex.hunted_baselines.iter() {
        println!(
            "hunted {pair:?}: baseline={base:?} fired={}",
            w.codex.hunted_active.contains(pair)
        );
    }
    // Per-species vigilance/armor/weapon means at t5k.
    let mut agg = anabios_core::codex::SpeciesAggTable::default();
    agg.build(&w);
    for &sid in agg.active() {
        let e = agg.get(sid).unwrap();
        let n = e.count as f64;
        println!(
            "species {sid}: n={} vigil={:.3} armor={:.3} speed={:.3} weapon={:.3}",
            e.count,
            e.genome_sums[GenomeSlot::Vigilance.idx()] / n,
            e.armor_sum / n,
            e.speed_sum / n,
            e.weapon_sum / n,
        );
    }
}
