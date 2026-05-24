//! Online species clustering and phylogeny tracking.
//!
//! Runs every `SPECIES_STEP_INTERVAL` ticks. Algorithm:
//!
//! 1. Recompute each species' centroid as the mean of its alive members.
//!    Mark empty species (member count = 0) but keep their id slots intact.
//! 2. For each alive agent in id order:
//!    - Compute distance to its current species centroid.
//!    - If `> SPECIATION_THRESHOLD`, find the closest existing species
//!      (over all non-empty species).
//!      - If that closest species is also `> SPECIATION_THRESHOLD`, allocate
//!        a new species id whose centroid is this agent's genome and whose
//!        `species_parents[k] = Some(prior_id)`.
//!      - Otherwise reassign the agent to the closest species.
//! 3. Recompute centroids once more (since memberships changed in step 2).

use crate::genome::{Genome, GENOME_LEN};
use crate::world::World;

/// Run species clustering every N ticks. Triggered at ticks 0, N, 2N, ...
/// (the check is `tick % N == 0` *before* the tick counter is incremented),
/// so the first clustering pass executes during the first call to
/// `tick::step()`.
pub const SPECIES_STEP_INTERVAL: u64 = 200;

/// L2 distance threshold beyond which an agent's genome is considered
/// "different enough" from its species' centroid to trigger reassignment
/// or split-off.
pub const SPECIATION_THRESHOLD: f32 = 0.6;

pub fn species_step(world: &mut World) {
    recompute_centroids(world);

    // Snapshot alive ids to iterate deterministically.
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();

    for id in &alive_ids {
        let i = *id as usize;
        let g = world.agents.genome[i];
        let cur_species = world.agents.species_id[i] as usize;
        let d_own = world.species_centroids[cur_species].distance(&g);

        if d_own <= SPECIATION_THRESHOLD {
            continue;
        }

        // Find closest non-empty species across the table.
        let mut best_id: usize = cur_species;
        let mut best_d: f32 = d_own;
        for (sid, count) in world.species_member_counts.iter().enumerate() {
            if *count == 0 || sid == cur_species {
                continue;
            }
            let d = world.species_centroids[sid].distance(&g);
            if d < best_d {
                best_d = d;
                best_id = sid;
            }
        }

        if best_d <= SPECIATION_THRESHOLD {
            // Reassign to the existing closer species.
            world.remove_from_species(cur_species as u32);
            world.add_to_species(best_id as u32);
            world.agents.species_id[i] = best_id as u32;
        } else {
            // Allocate a new species with this agent's genome as centroid.
            let new_id = world.next_species_id;
            world.next_species_id =
                world.next_species_id.checked_add(1).expect("species id overflow");
            world.species_centroids.push(g);
            world.species_member_counts.push(0); // helper increments below
            world.species_parents.push(Some(cur_species as u32));
            world.remove_from_species(cur_species as u32);
            world.add_to_species(new_id);
            world.agents.species_id[i] = new_id;
        }
    }

    // Step 3: recompute centroids once more so they reflect new memberships.
    recompute_centroids(world);
}

fn recompute_centroids(world: &mut World) {
    let num_species = world.species_centroids.len();

    // Sum genome slots per species in deterministic agent id order.
    let mut sums: Vec<[f64; GENOME_LEN]> = vec![[0.0_f64; GENOME_LEN]; num_species];

    for id in world.agents.iter_alive() {
        let i = id as usize;
        let sid = world.agents.species_id[i] as usize;
        let g = &world.agents.genome[i].0;
        for k in 0..GENOME_LEN {
            sums[sid][k] += g[k] as f64;
        }
    }

    for (sid, sum) in sums.iter().enumerate().take(num_species) {
        let n = world.species_member_counts[sid];
        if n > 0 {
            let mut centroid = [0.0_f32; GENOME_LEN];
            let nf = n as f64;
            for k in 0..GENOME_LEN {
                centroid[k] = (sum[k] / nf) as f32;
            }
            world.species_centroids[sid] = Genome(centroid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::Vec2;

    #[test]
    fn empty_world_runs_without_panic() {
        let mut w = World::new(1);
        species_step(&mut w);
        assert_eq!(w.species_centroids.len(), 1);
    }

    #[test]
    fn homogeneous_population_stays_one_species() {
        let mut w = World::new(7);
        for _ in 0..50 {
            w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        }
        species_step(&mut w);
        assert_eq!(w.species_member_counts.len(), 1);
        assert_eq!(w.species_member_counts[0], 50);
    }

    #[test]
    fn divergent_genome_triggers_speciation() {
        let mut w = World::new(7);
        for _ in 0..20 {
            w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        }
        // Add one agent with a very different genome.
        let mut weird = Genome::neutral();
        for i in 0..GENOME_LEN {
            weird.0[i] = if i % 2 == 0 { 0.0 } else { 1.0 };
        }
        w.spawn_agent(Vec2::new(500.0, 500.0), weird);

        species_step(&mut w);
        // Should have produced one new species with the weird agent.
        assert!(
            w.species_member_counts.len() >= 2,
            "expected speciation: {:?}",
            w.species_member_counts
        );
        assert_eq!(w.species_parents[1], Some(0));
    }
}
