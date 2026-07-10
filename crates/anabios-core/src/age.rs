//! Ageing and death of agents at the end of each tick.

use crate::genome::GenomeSlot;

/// Maximum lifespan in ticks at `LifespanBias = 1.0`.
pub const LIFESPAN_MAX_TICKS: u32 = 5_000;
/// Minimum lifespan in ticks at `LifespanBias = 0.0` (so newborns aren't
/// instantly senescent).
pub const LIFESPAN_MIN_TICKS: u32 = 500;

pub fn age_and_starve(world: &mut crate::world::World) {
    let alive_ids: Vec<u32> = world.agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;
        world.agents.age[i] = world.agents.age[i].saturating_add(1);

        let lifespan = lifespan_of(&world.agents.genome[i]);
        let died =
            if world.agents.energy[i] <= 0.0 { true } else { world.agents.age[i] >= lifespan };

        if died {
            let sid = world.agents.species_id[i];
            let size = world.agents.genome[i].get(GenomeSlot::Size).max(0.1);
            let pos = world.agents.position[i];
            world.carcasses.push(crate::carcass::Carcass {
                pos,
                flesh: crate::carcass::CARCASS_FLESH_PER_SIZE * size,
                age: 0,
                species_id: sid,
            });
            world.agents.kill(id);
            world.remove_from_species(sid);
        }
    }
}

/// Maximum tick age an agent of this genome can reach before dying of old age.
pub fn lifespan_of(genome: &crate::genome::Genome) -> u32 {
    let bias = genome.get(GenomeSlot::LifespanBias);
    let span = LIFESPAN_MIN_TICKS as f32 + (LIFESPAN_MAX_TICKS - LIFESPAN_MIN_TICKS) as f32 * bias;
    span as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::world::World;

    #[test]
    fn age_increments_each_call() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        age_and_starve(&mut w);
        age_and_starve(&mut w);
        age_and_starve(&mut w);
        assert_eq!(w.agents.age[id as usize], 3);
        assert!(w.agents.is_alive(id));
    }

    #[test]
    fn agent_with_zero_energy_dies() {
        let mut w = World::new(1);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w.agents.energy[id as usize] = 0.0;
        age_and_starve(&mut w);
        assert!(!w.agents.is_alive(id));
    }

    #[test]
    fn agent_dies_of_old_age_at_lifespan_bias_zero() {
        let mut w = World::new(1);
        let mut g = Genome::neutral();
        g.set(GenomeSlot::LifespanBias, 0.0);
        let id = w.spawn_agent(Vec2::new(500.0, 500.0), g);
        for _ in 0..LIFESPAN_MIN_TICKS as usize {
            age_and_starve(&mut w);
            if !w.agents.is_alive(id) {
                break;
            }
        }
        assert!(!w.agents.is_alive(id));
    }
}
