//! Kin recognition: a scalar relatedness in [0,1] blending shared ancestry
//! (parent-lineage overlap + parent/child links) with genome similarity.
//! Gates altruism on kin so cooperation is evolutionarily stable (§3.2, §4.3).

use crate::agent::LINEAGE_NONE;
use crate::genome::{Genome, GENOME_LEN};

/// √GENOME_LEN — the max possible L2 distance between two genomes whose slots
/// are all in [0,1]. Used to normalize genome distance into a [0,1] similarity.
pub const SQRT_GENOME_LEN: f32 = 7.071_068; // sqrt(50)

/// Relatedness of two agents in [0,1]: `0.5*ancestry + 0.5*genome_similarity`.
pub fn kinship(
    a_lineage: u64,
    a_parents: &[u64; 2],
    a_genome: &Genome,
    b_lineage: u64,
    b_parents: &[u64; 2],
    b_genome: &Genome,
) -> f32 {
    // Ancestry: shared (non-NONE) parents + parent/child link.
    let mut shared = 0u32;
    for pa in a_parents {
        if *pa != LINEAGE_NONE && b_parents.contains(pa) {
            shared += 1;
        }
    }
    let parent_child = a_parents.contains(&b_lineage) || b_parents.contains(&a_lineage);
    let ancestry = (shared as f32 * 0.25 + if parent_child { 0.5 } else { 0.0 }).min(1.0);

    // Genome similarity from normalized L2 distance.
    let genome_sim = (1.0 - a_genome.distance(b_genome) / SQRT_GENOME_LEN).clamp(0.0, 1.0);

    (0.5 * ancestry + 0.5 * genome_sim).clamp(0.0, 1.0)
}

// Suppress unused-import warning: GENOME_LEN is used as a compile-time
// documentation anchor (its value 50 drives SQRT_GENOME_LEN). Verify here.
const _: () = assert!(GENOME_LEN == 50);
