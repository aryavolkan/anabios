//! The cognition threshold knobs (`iq_req_by_era`, `practice_iq_req`) and the
//! world built to graph them, `scenarios/cognition-threshold.toml`.
//!
//! The unit tests in `iq.rs` / `invention` / `practice.rs` pin the gate
//! arithmetic against the default ladder. These tests pin the knobs: a
//! scenario moves the ladder, the world reads the moved ladder (not the
//! constants), bad values are refused at parse, and the six-rung scenario is
//! built the way its header says.

use anabios_core::genome::GenomeSlot;
use anabios_core::invention::{self, IQ_REQ_BY_ERA};
use anabios_core::module::{self, ModuleType};
use anabios_core::practice::PRACTICE_IQ_REQ;
use anabios_core::scenario::{Scenario, ScenarioError};
use anabios_core::tick::step;
use anabios_core::world::World;

const SCENARIO: &str = include_str!("../../../scenarios/cognition-threshold.toml");

fn tiny(extra: &str) -> String {
    format!(
        "name = \"t\"\nseed = 1\n{extra}\n[[agents]]\ncount = 2\narchetype = \"innovator\"\n\
         placement = {{ kind = \"uniform\" }}\n"
    )
}

#[test]
fn default_world_reads_the_constant_ladder() {
    let w = World::new(1);
    assert_eq!(w.iq_req_by_era, IQ_REQ_BY_ERA);
    assert_eq!(w.practice_iq_req, PRACTICE_IQ_REQ);
    let w = Scenario::parse_toml(&tiny("")).unwrap().instantiate();
    assert_eq!(w.iq_req_by_era, IQ_REQ_BY_ERA, "absent knob = engine default");
    assert_eq!(w.practice_iq_req, PRACTICE_IQ_REQ);
}

#[test]
fn scenario_moves_the_ladder_and_the_gates_read_the_moved_one() {
    let toml = tiny("iq_req_by_era = [0.1, 0.2, 0.3, 0.4]\npractice_iq_req = 0.05");
    let w = Scenario::parse_toml(&toml).unwrap().instantiate();
    assert_eq!(w.iq_req_by_era, [0.1, 0.2, 0.3, 0.4]);
    assert_eq!(w.practice_iq_req, 0.05);
    // Nuclear Power is era 4: 0.4 clears the moved gate, where the constant
    // ladder (0.75) would refuse it.
    assert!(w.invention_iq_permits(0.4, invention::NUCLEAR_POWER));
    assert!(!invention::iq_permits(0.4, invention::NUCLEAR_POWER, true));
    assert!(!w.invention_iq_permits(0.39, invention::NUCLEAR_POWER));
    assert!(w.practice_iq_permits(0.05));
    assert!(!w.practice_iq_permits(0.04));
    // Cognition off: both gates are open whatever the ladder says.
    let mut off = w;
    off.cognition_enabled = false;
    assert!(off.invention_iq_permits(0.0, invention::NUCLEAR_POWER));
    assert!(off.practice_iq_permits(0.0));
}

#[test]
fn a_gate_outside_the_unit_interval_is_refused_at_parse() {
    for bad in ["iq_req_by_era = [0.1, 0.2, 0.3, 1.5]", "practice_iq_req = -0.1"] {
        match Scenario::parse_toml(&tiny(bad)) {
            Err(ScenarioError::InvalidIqReq(_)) => {}
            other => panic!("{bad}: expected InvalidIqReq, got {other:?}"),
        }
    }
    // A fully open ladder is legal: it is the gate-sweep's zero point.
    Scenario::parse_toml(&tiny("iq_req_by_era = [0.0, 0.0, 0.0, 0.0]\npractice_iq_req = 0.0"))
        .expect("an open ladder parses");
}

#[test]
fn the_six_rungs_are_distinct_cultured_breeding_species_on_an_ascending_ladder() {
    let w = Scenario::parse_toml(SCENARIO).unwrap().instantiate();
    assert!(w.cognition_enabled && w.inventions_enabled && w.practices_enabled);
    assert!(!w.resources_enabled && !w.gene_requirements && !w.gene_tech_coupling);
    assert_eq!(w.iq_req_by_era, IQ_REQ_BY_ERA, "the world keeps the default ladder");

    // One species per rung (ids 1..=6), 40 founders each, with the rung's
    // gene pinned and Stone Tools + Fire held.
    let mut per_species = std::collections::BTreeMap::<u32, (u32, f32)>::new();
    for id in w.agents.iter_alive() {
        let i = id as usize;
        let mods = &w.agents.modules[i];
        assert!(module::has(mods, ModuleType::Communicator), "agent {id}: culture-capable");
        assert!(module::has(mods, ModuleType::Reproductive), "agent {id}: breeds");
        let mask = invention::held_mask(&w.agents.meme_vector[i]);
        assert_eq!(
            mask & (invention::bit(invention::STONE_TOOLS) | invention::bit(invention::FIRE)),
            invention::bit(invention::STONE_TOOLS) | invention::bit(invention::FIRE),
            "agent {id}: seeded with Stone Tools and Fire"
        );
        let gene = w.agents.genome[i].get(GenomeSlot::CognitivePotential);
        let e = per_species.entry(w.agents.species_id[i]).or_insert((0, gene));
        e.0 += 1;
        assert_eq!(e.1, gene, "agent {id}: one gene value per rung");
    }
    let rungs: Vec<(u32, (u32, f32))> = per_species.into_iter().collect();
    assert_eq!(rungs.len(), 6, "six rungs");
    let genes: Vec<f32> = rungs.iter().map(|(_, (_, g))| *g).collect();
    assert_eq!(genes, vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]);
    assert!(rungs.iter().all(|(_, (n, _))| *n == 40), "40 founders per rung: {rungs:?}");
    assert_eq!(rungs.iter().map(|(sid, _)| *sid).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6]);
}

/// After the juvenile window the rungs' realized IQs must come out in gene
/// order — the ladder the scenario exists to graph — and every rung must
/// still be alive.
#[test]
fn realized_iq_follows_the_rungs_after_maturation() {
    let mut w = Scenario::parse_toml(SCENARIO).unwrap().instantiate();
    w.max_population = 500;
    for _ in 0..anabios_core::iq::IQ_MATURATION_AGE + 20 {
        step(&mut w);
    }
    let mut sum = [0.0f32; 7];
    let mut n = [0u32; 7];
    for id in w.agents.iter_alive() {
        let i = id as usize;
        let sid = w.agents.species_id[i] as usize;
        if (1..=6).contains(&sid) {
            sum[sid] += w.agents.iq[i];
            n[sid] += 1;
        }
    }
    let means: Vec<f32> = (1..=6).map(|s| sum[s] / n[s].max(1) as f32).collect();
    assert!(n[1..].iter().all(|&c| c > 0), "every rung alive: {n:?}");
    assert!(means.windows(2).all(|p| p[0] < p[1]), "ascending realized IQ: {means:?}");
    assert!(means[0] < IQ_REQ_BY_ERA[1], "the gene-0 rung sits below the era-2 gate: {means:?}");
    assert!(means[5] > IQ_REQ_BY_ERA[2], "the gene-1 rung clears the era-3 gate: {means:?}");
}
