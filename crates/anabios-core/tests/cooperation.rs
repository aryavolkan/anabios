//! M15 mechanism tests: kin recognition, sharing, and the cooperation detectors.

use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::kin::kinship;

#[test]
fn kinship_high_for_siblings_low_for_unrelated() {
    // Siblings: share both parent lineages (2 and 3); near-identical genomes.
    let g = Genome::neutral();
    let sib = kinship(10, &[2, 3], &g, 11, &[2, 3], &g);
    // Unrelated: no shared parents; distant genomes.
    let mut far = Genome::neutral();
    far.set(GenomeSlot::Size, 1.0);
    far.set(GenomeSlot::DietCarnivory, 1.0);
    far.set(GenomeSlot::SpeedMax, 1.0);
    let unrel = kinship(10, &[2, 3], &g, 99, &[50, 51], &far);
    assert!(sib > 0.7, "siblings with identical genome are highly related ({sib})");
    assert!(unrel < sib, "unrelated distant-genome pair is less related ({unrel} < {sib})");
}

#[test]
fn kinship_parent_child_is_related() {
    let g = Genome::neutral();
    // Agent 5 is a parent of agent 12 (12's parents include lineage 5).
    let r = kinship(5, &[1, 2], &g, 12, &[5, 7], &g);
    assert!(r > 0.5, "parent-child relatedness ({r})");
}

use anabios_core::prelude_test::Vec2;
use anabios_core::program::{Node, Program};
use anabios_core::tick::step;
use anabios_core::world::World;

#[test]
fn share_transfers_energy_scaled_by_altruism() {
    let mut w = World::new(4);
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Altruism, 1.0);
    // High reproduction threshold so the adjacent pair does NOT mate this tick —
    // reproduction's energy cost would otherwise mask the share transfer.
    g.set(GenomeSlot::ReproductionThreshold, 1.0);
    let mut rg = Genome::neutral();
    rg.set(GenomeSlot::ReproductionThreshold, 1.0);
    let donor = w.spawn_agent(Vec2::new(500.0, 500.0), g);
    let recipient = w.spawn_agent(Vec2::new(501.0, 500.0), rg); // within SHARE_RANGE
    // Donor always shares (share_intent = 1.0 via Const + Share).
    w.agents.program[donor as usize] = Program::from_slice(&[Node::Const(1.0), Node::Share]);
    w.agents.program[recipient as usize] = Program::from_slice(&[Node::Idle]);
    let d0 = w.agents.energy[donor as usize];
    let r0 = w.agents.energy[recipient as usize];
    step(&mut w);
    assert!(w.agents.energy[donor as usize] < d0, "donor lost energy");
    assert!(w.agents.energy[recipient as usize] > r0, "recipient gained energy");
}

#[test]
fn zero_altruism_means_no_sharing() {
    let mut w = World::new(4);
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Altruism, 0.0);
    let donor = w.spawn_agent(Vec2::new(500.0, 500.0), g);
    let recipient = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
    w.agents.program[donor as usize] = Program::from_slice(&[Node::Const(1.0), Node::Share]);
    // Idle recipient so it neither grazes nor moves — isolates "no share in".
    w.agents.program[recipient as usize] = Program::from_slice(&[Node::Idle]);
    let r0 = w.agents.energy[recipient as usize];
    step(&mut w);
    // Recipient's only energy change is its own metabolism/grazing — no share in.
    // Assert it did not gain the share amount (energy did not increase from sharing).
    assert!(w.agents.energy[recipient as usize] <= r0 + 1e-3, "no altruism → no share");
}
