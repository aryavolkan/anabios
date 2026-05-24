//! Interaction step. In M1 the only interaction is **feeding**: agents in a
//! cell with plant biomass and a herbivorous diet (low `DietCarnivory`)
//! graze. Combat and mating land in later milestones.

use crate::agent::AgentBuffers;
use crate::biome::BiomeField;
use crate::genome::GenomeSlot;

/// Maximum plant biomass an agent can eat per tick at `Size = 1.0`.
pub const BITE_MAX: f32 = 0.5;
/// Energy gained per biomass unit consumed.
pub const FOOD_ENERGY_PER_BIOMASS: f32 = 4.0;

pub fn interact_all(agents: &mut AgentBuffers, biome: &mut BiomeField) {
    let alive_ids: Vec<u32> = agents.iter_alive().collect();
    for id in alive_ids {
        let i = id as usize;

        // Action gating: no Mouth → can't eat.
        if !crate::module::has(&agents.modules[i], crate::module::ModuleType::Mouth) {
            continue;
        }

        let pos = agents.position[i];
        let bite_cap = crate::module::effective_bite_size(&agents.modules[i]);
        let diet_carn = crate::module::effective_diet_carnivory(&agents.modules[i]);
        let herbivory = (1.0 - diet_carn).clamp(0.0, 1.0);
        if herbivory <= 0.0 || bite_cap <= 0.0 {
            continue;
        }
        let size = agents.genome[i].get(GenomeSlot::Size).max(0.1);
        let desired_bite = BITE_MAX * size * bite_cap * herbivory;
        let taken = biome.graze(pos, desired_bite);
        if taken > 0.0 {
            agents.energy[i] += taken * FOOD_ENERGY_PER_BIOMASS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;
    use crate::genome::Genome;
    use crate::prelude::Vec2;
    use crate::world::World;

    fn find_grass_cell_center(w: &World) -> Vec2 {
        for row in 0..crate::biome::BIOME_RES {
            for col in 0..crate::biome::BIOME_RES {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    return Vec2::new(
                        (col as f32 + 0.5) * crate::biome::CELL_SIZE,
                        (row as f32 + 0.5) * crate::biome::CELL_SIZE,
                    );
                }
            }
        }
        panic!("no grass cell in biome");
    }

    #[test]
    fn herbivore_on_grass_gains_energy() {
        let mut w = World::new(11);
        let pos = find_grass_cell_center(&w);
        let mut genome = Genome::neutral();
        genome.set(GenomeSlot::DietCarnivory, 0.0);
        let id = w.spawn_agent(pos, genome);
        let energy_before = w.agents.energy[id as usize];
        let biomass_before = w.biome.sample(pos).plant_biomass;
        interact_all(&mut w.agents, &mut w.biome);
        let energy_after = w.agents.energy[id as usize];
        let biomass_after = w.biome.sample(pos).plant_biomass;
        assert!(energy_after > energy_before);
        assert!(biomass_after < biomass_before);
    }

    #[test]
    fn obligate_carnivore_does_not_eat_plants() {
        let mut w = World::new(11);
        let pos = find_grass_cell_center(&w);
        let id = w.spawn_agent(pos, Genome::neutral());
        // Replace Mouth with a pure carnivore.
        for m in w.agents.modules[id as usize].iter_mut() {
            if let crate::module::Module::Mouth { diet_affinity, .. } = m {
                *diet_affinity = 1.0;
            }
        }
        let energy_before = w.agents.energy[id as usize];
        let biomass_before = w.biome.sample(pos).plant_biomass;
        interact_all(&mut w.agents, &mut w.biome);
        assert_eq!(w.agents.energy[id as usize], energy_before);
        assert_eq!(w.biome.sample(pos).plant_biomass, biomass_before);
    }

    #[test]
    fn agent_without_mouth_does_not_eat() {
        let mut w = World::new(11);
        let pos = find_grass_cell_center(&w);
        let id = w.spawn_agent(pos, Genome::neutral());
        w.agents.modules[id as usize].retain(|m| !matches!(m, crate::module::Module::Mouth { .. }));
        let energy_before = w.agents.energy[id as usize];
        let biomass_before = w.biome.sample(pos).plant_biomass;
        interact_all(&mut w.agents, &mut w.biome);
        assert_eq!(w.agents.energy[id as usize], energy_before);
        assert_eq!(w.biome.sample(pos).plant_biomass, biomass_before);
    }

    #[test]
    fn two_agents_share_finite_biomass_deterministically() {
        let mut w = World::new(11);
        let pos = find_grass_cell_center(&w);
        // Drain to a small amount.
        let (col, row) = BiomeField::cell_coords(pos);
        w.biome.at_mut(col, row).plant_biomass = 0.3;
        let g = {
            let mut g = Genome::neutral();
            g.set(GenomeSlot::DietCarnivory, 0.0);
            g.set(GenomeSlot::Size, 1.0);
            g
        };
        let id0 = w.spawn_agent(pos, g);
        let id1 = w.spawn_agent(pos, g);
        interact_all(&mut w.agents, &mut w.biome);
        // First-in-id wins the larger share.
        assert!(w.agents.energy[id0 as usize] > w.agents.energy[id1 as usize]);
        assert!(w.biome.sample(pos).plant_biomass < 1e-5);
    }
}
