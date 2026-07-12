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
