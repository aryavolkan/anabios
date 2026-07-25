// TG1 contract guard: a holder with a high affinity gene must out-buff a
// low-gene holder when gene_tech_coupling is on, and get the identical buff
// when it's off. Exercises the public coupled-multiplier path (Farming↔
// Conscientiousness as the exemplar).
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::invention::{self, affinity_gene, bit, FARMING};

#[test]
fn coupling_creates_a_buff_differential_only_when_on() {
    let mask = bit(FARMING);
    let mut hi = Genome::neutral();
    hi.set(GenomeSlot::Conscientiousness, 1.0);
    let mut lo = Genome::neutral();
    lo.set(GenomeSlot::Conscientiousness, 0.0);

    // OFF: no differential.
    let off_hi = invention::graze_multiplier_coupled(mask, affinity_gene(&hi, FARMING), false);
    let off_lo = invention::graze_multiplier_coupled(mask, affinity_gene(&lo, FARMING), false);
    assert_eq!(off_hi, off_lo);

    // ON: the high-gene holder gets the larger buff.
    let on_hi = invention::graze_multiplier_coupled(mask, affinity_gene(&hi, FARMING), true);
    let on_lo = invention::graze_multiplier_coupled(mask, affinity_gene(&lo, FARMING), true);
    assert!(on_hi > on_lo, "coupling must create a selection differential");
}
