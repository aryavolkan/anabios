//! Reproduction stage.
//!
//! Two same-species agents in close proximity (≤ `MATING_RANGE`) with energy
//! above `reproduction_threshold * SPAWN_ENERGY * 1.5` may produce one
//! offspring per tick. Each parent pays `PARENT_ENERGY_COST_FRAC *
//! SPAWN_ENERGY` energy; the offspring is seeded with `SPAWN_ENERGY` energy.
//! The fraction is tuned to be **energy-conserving** within the family-pair
//! exchange (parents collectively pay exactly the offspring's spawn energy).
//!
//! Reproduction is hard-capped at `World::max_population` (default
//! `MAX_POPULATION` = 10_000, scenario-overridable) to prevent runaway
//! growth in over-fertile scenarios; this is a coarse backstop, not a
//! carrying-capacity model.

use crate::agent::{AgentBuffers, SPAWN_ENERGY};
use crate::genome::{Genome, GenomeSlot};
use crate::prelude::Vec2;
use crate::spatial::{torus_distance, UniformSpatialHash};
use crate::world::World;

/// Maximum distance between two parents at the moment of mating, in world units.
pub const MATING_RANGE: f32 = 2.0;

/// Fraction of `SPAWN_ENERGY` each parent pays to produce an offspring.
/// 0.5 means parents collectively pay `SPAWN_ENERGY` total (energy-conserving).
pub const PARENT_ENERGY_COST_FRAC: f32 = 0.5;

/// Default hard upper bound on alive agents. Reproduction skips at/above the
/// cap. The live value is `World::max_population` (per-world overridable);
/// this constant is the design's 10k-agent budget (design §8; the
/// `tick_bench` 10k case seeds founders directly to exercise that scale).
pub const MAX_POPULATION: u32 = 10_000;

/// Run the reproduce stage. Each alive agent at most mates once per tick.
/// Order: ascending agent id. Each agent A checks its same-cell neighbours
/// in ascending id order and mates with the first eligible B such that
/// `B.id > A.id`; this avoids double-counting and keeps the algorithm
/// deterministic.
pub fn reproduce_all(world: &mut World) {
    // Pull scratch buffer length up to current capacity.
    if world.reproduced_this_tick.len() < world.agents.capacity() {
        world.reproduced_this_tick.resize(world.agents.capacity(), false);
    }
    world.reproduced_this_tick.fill(false);

    // Snapshot the alive ids to a local vec; reproduction mutates the
    // alive set via spawn() and we don't want to iterate over newborns
    // this tick.
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());

    for &a_id in &alive_ids {
        if world.agents.live_count() >= world.max_population {
            // Backstop: stop producing offspring above the cap. Iteration
            // order is deterministic (ascending id), so the cutoff is too.
            break;
        }
        let i = a_id as usize;
        if world.reproduced_this_tick[i] {
            continue;
        }
        if !is_eligible(&world.agents, a_id) {
            continue;
        }
        if world.resources_enabled && !has_dowry(&world.agents, a_id) {
            continue;
        }

        let a_pos = world.agents.position[i];
        let a_species = world.agents.species_id[i];
        let a_genome = world.agents.genome[i];
        let a_lineage = world.agents.lineage_id[i];

        // Find an eligible mate with a strictly higher id. An inbreeding-practice
        // holder (cognition on) seeks the genetically-nearest partner instead.
        let kin_seeking = world.cognition_enabled
            && crate::practice::has(&world.agents.meme_vector[i], crate::practice::INBREEDING);
        let mate = find_mate(
            &world.spatial,
            &world.agents,
            &world.reproduced_this_tick,
            a_id,
            a_pos,
            a_species,
            world.world_size,
            kin_seeking,
            &a_genome,
        );
        let Some(b_id) = mate else { continue };
        if world.resources_enabled && !has_dowry(&world.agents, b_id) {
            continue;
        }

        let j = b_id as usize;
        let b_pos = world.agents.position[j];
        let b_genome = world.agents.genome[j];
        let b_lineage = world.agents.lineage_id[j];

        // Pay energy from both parents.
        let cost = SPAWN_ENERGY * PARENT_ENERGY_COST_FRAC;
        world.agents.energy[i] -= cost;
        world.agents.energy[j] -= cost;

        // Consume dowry (trade goods) from both parents when resources are enabled.
        if world.resources_enabled {
            for g in crate::resource::Good::ALL {
                world.agents.inventory[i][g.index()] -= crate::resource::DOWRY_REQ;
                world.agents.inventory[j][g.index()] -= crate::resource::DOWRY_REQ;
            }
            world.codex.push_event(crate::codex::CodexEvent {
                event_type: crate::codex::EventType::DowryBirth,
                tick: world.tick,
                species_id: a_species,
                value: 0.0,
                loc_x: world.agents.position[i].x,
                loc_y: world.agents.position[i].y,
            });
        }

        // Build child genome: crossover + mutate. Nuclear Power debuff:
        // radiation scales the child's mutation sigma when either parent
        // holds it (draw count unchanged — only magnitudes).
        let mut child_genome = Genome::crossover(&a_genome, &b_genome, &mut world.rng);
        let sigma_mult = crate::invention::mutation_multiplier(
            crate::invention::held_mask(&world.agents.meme_vector[i]),
            crate::invention::held_mask(&world.agents.meme_vector[j]),
        );
        child_genome.mutate_in_place_scaled(&mut world.rng, sigma_mult);

        // Mark both parents as reproduced this tick before spawning so the
        // newborn's slot (which gets a fresh bitvec bit) isn't accidentally
        // touched.
        world.reproduced_this_tick.set(i, true);
        world.reproduced_this_tick.set(j, true);

        // Spawn at midpoint of parents on the torus (account for wrap).
        let child_pos = midpoint_torus(a_pos, b_pos, world.world_size);

        let a_modules = world.agents.modules[i].clone();
        let b_modules = world.agents.modules[j].clone();
        let child_modules =
            crate::module::crossover_and_mutate(&a_modules, &b_modules, &mut world.rng);

        let a_program = world.agents.program[i].clone();
        let b_program = world.agents.program[j].clone();
        let child_program = crate::program::crossover_and_mutate(
            &a_program,
            &b_program,
            &mut world.rng,
            world.war_enabled,
            world.settlement_enabled,
        );

        let lineage = world.next_lineage();
        let child_id = world.agents.spawn(
            child_pos,
            child_genome,
            lineage,
            [a_lineage, b_lineage],
            a_species,
            child_modules,
            child_program,
        );
        world.add_to_species(a_species);

        // Anchor inheritance (E8): child anchor = parent-anchor midpoint +
        // drift, ONLY when settlement is enabled. Gated so flag-off draws
        // zero extra RNG (baseline streams unchanged).
        if world.settlement_enabled {
            let ws = world.world_size;
            let aa = world.agents.anchor[i];
            let ba = world.agents.anchor[j];
            // Torus-safe midpoint: walk from A halfway toward B.
            let mut dx = ba.x - aa.x;
            let mut dy = ba.y - aa.y;
            if dx > ws * 0.5 {
                dx -= ws;
            } else if dx < -ws * 0.5 {
                dx += ws;
            }
            if dy > ws * 0.5 {
                dy -= ws;
            } else if dy < -ws * 0.5 {
                dy += ws;
            }
            let jx = world.rng.gaussian(0.0, crate::codex::ANCHOR_DRIFT_SIGMA);
            let jy = world.rng.gaussian(0.0, crate::codex::ANCHOR_DRIFT_SIGMA);
            world.agents.anchor[child_id as usize] = crate::prelude::Vec2::new(
                (aa.x + dx * 0.5 + jx).rem_euclid(ws),
                (aa.y + dy * 0.5 + jy).rem_euclid(ws),
            );
        }

        // Ensure the bitvec covers the new slot, mark the child as
        // "reproduced this tick" so they cannot immediately mate again.
        if world.reproduced_this_tick.len() <= child_id as usize {
            world.reproduced_this_tick.resize(child_id as usize + 1, false);
        }
        world.reproduced_this_tick.set(child_id as usize, true);

        // Meme inheritance: child = parent average + jitter, ONLY if the child
        // has a Communicator module. This gates RNG draws so that non-communicator
        // lineages (e.g. minimal.toml) draw zero meme RNG, keeping the golden
        // hash stream unchanged.
        if crate::module::has(
            &world.agents.modules[child_id as usize],
            crate::module::ModuleType::Communicator,
        ) {
            let a_meme = world.agents.meme_vector[i];
            let b_meme = world.agents.meme_vector[j];
            let inventions_enabled = world.inventions_enabled;
            let cognition_enabled = world.cognition_enabled;
            // E9 institutional memory: when BOTH parents belong to a
            // settlement-latched species, inheritance jitter shrinks —
            // settled cultures pass memes down more faithfully.
            let a_species = world.agents.species_id[i];
            let b_species = world.agents.species_id[j];
            let settled = world.codex.settlement_active.contains(&a_species)
                && world.codex.settlement_active.contains(&b_species);
            let fidelity = if settled { crate::codex::SETTLED_FIDELITY } else { 1.0 };
            world.agents.meme_vector[child_id as usize] = crate::culture::inherit_meme(
                &a_meme,
                &b_meme,
                &mut world.rng,
                inventions_enabled,
                cognition_enabled,
                fidelity,
            );
            // E9 lineage: the newborn's per-channel variants descend from
            // its parents' variants (band-matched) or are freshly minted.
            crate::codex::traditions::assign_birth_variants(
                world,
                child_id as usize,
                i,
                j,
            );
        }

        // Maladaptive-practice fitness costs (cognition-gated). A parent's held
        // practice damages the offspring's reproductive/genetic fitness. Read
        // the holdings first (releasing the meme-vector borrow) so the energy /
        // kill mutations below don't alias it.
        if world.cognition_enabled {
            use crate::practice::{self, CHILD_SACRIFICE, INBREEDING};
            let inbred = practice::has(&world.agents.meme_vector[i], INBREEDING)
                || practice::has(&world.agents.meme_vector[j], INBREEDING);
            let sacrifices = practice::has(&world.agents.meme_vector[i], CHILD_SACRIFICE)
                || practice::has(&world.agents.meme_vector[j], CHILD_SACRIFICE);
            // Inbreeding depression: a kin-mating custom expresses recessive
            // genetic load — the closer the parents, the frailer the child
            // (energy) and the likelier it is stillborn (viability). Paired with
            // the kin-seeking mate bias in `find_mate`, close pairings are common,
            // so this bites at the population level.
            let closeness =
                if inbred { practice::inbreeding_closeness(&a_genome, &b_genome) } else { 0.0 };
            if inbred {
                world.agents.energy[child_id as usize] *=
                    1.0 - practice::INBREEDING_DEPRESSION * closeness;
            }
            // Two independent lethal rolls; a child removed by either is removed
            // exactly once. `&&` short-circuits, so a non-inbreeding /
            // non-sacrificing birth draws no RNG (keeping unrelated scenarios'
            // streams unchanged).
            let stillborn =
                inbred && world.rng.f32_unit() < practice::INBREEDING_STILLBIRTH * closeness;
            let sacrificed = sacrifices && world.rng.f32_unit() < practice::CHILD_SACRIFICE_CULL;
            if stillborn || sacrificed {
                world.agents.kill(child_id);
                world.remove_from_species(a_species);
            }
        }
    }
    world.agents.scratch_ids = alive_ids;
}

fn is_eligible(agents: &AgentBuffers, id: u32) -> bool {
    let i = id as usize;
    if !agents.is_alive(id) {
        return false;
    }
    // Action gating: must have Reproductive module to mate.
    if !crate::module::has(&agents.modules[i], crate::module::ModuleType::Reproductive) {
        return false;
    }
    // Conscientiousness raises the effective breeding threshold.
    let threshold = SPAWN_ENERGY
        * agents.genome[i].get(GenomeSlot::ReproductionThreshold)
        * 1.5
        * crate::personality::personality_reproduction_factor(&agents.genome[i]);
    agents.energy[i] >= threshold
}

/// True iff this agent holds at least `DOWRY_REQ` of every good — the basket
/// required to reproduce when the trade economy is active.
fn has_dowry(agents: &AgentBuffers, id: u32) -> bool {
    let inv = &agents.inventory[id as usize];
    crate::resource::Good::ALL.iter().all(|g| inv[g.index()] >= crate::resource::DOWRY_REQ)
}

#[allow(clippy::too_many_arguments)]
fn find_mate(
    spatial: &UniformSpatialHash,
    agents: &AgentBuffers,
    reproduced: &bitvec::vec::BitVec,
    a_id: u32,
    a_pos: Vec2,
    a_species: u32,
    world_size: f32,
    kin_seeking: bool,
    a_genome: &Genome,
) -> Option<u32> {
    let mut best: Option<u32> = None;
    // Genome distance of `best` — only consulted when `kin_seeking`.
    let mut best_gd = f32::INFINITY;
    spatial.query(a_pos, MATING_RANGE, |other_id| {
        if other_id <= a_id {
            return;
        }
        let j = other_id as usize;
        if reproduced[j] {
            return;
        }
        if !is_eligible(agents, other_id) {
            return;
        }
        if agents.species_id[j] != a_species {
            return;
        }
        let d = torus_distance(a_pos, agents.position[j], world_size);
        if d > MATING_RANGE {
            return;
        }
        if kin_seeking {
            // Inbreeding custom: prefer the genetically-NEAREST eligible mate
            // (min genome distance, tie-break lowest id). This raises the
            // frequency and severity of close-kin pairings, so inbreeding
            // depression actually bites. Order-independent (a min with a
            // deterministic tie-break), so it stays deterministic regardless of
            // bucket traversal order.
            let gd = a_genome.distance(&agents.genome[j]);
            match best {
                None => {
                    best = Some(other_id);
                    best_gd = gd;
                }
                Some(cur) if gd < best_gd || (gd == best_gd && other_id < cur) => {
                    best = Some(other_id);
                    best_gd = gd;
                }
                _ => {}
            }
        } else {
            // Default: the lowest-id eligible mate. The spatial query already
            // visits cells in a fixed order and within each cell ids are
            // scattered in ascending-id order, so this is robust to any future
            // change in bucket traversal.
            match best {
                None => best = Some(other_id),
                Some(cur) if other_id < cur => best = Some(other_id),
                _ => {}
            }
        }
    });
    best
}

fn midpoint_torus(a: Vec2, b: Vec2, world_size: f32) -> Vec2 {
    let mut dx = b.x - a.x;
    let mut dy = b.y - a.y;
    if dx > world_size * 0.5 {
        dx -= world_size;
    } else if dx < -world_size * 0.5 {
        dx += world_size;
    }
    if dy > world_size * 0.5 {
        dy -= world_size;
    } else if dy < -world_size * 0.5 {
        dy += world_size;
    }
    let mid_x = (a.x + dx * 0.5).rem_euclid(world_size);
    let mid_y = (a.y + dy * 0.5).rem_euclid(world_size);
    Vec2::new(mid_x, mid_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::TerrainType;
    use crate::genome::Genome;
    use crate::world::World;

    fn find_grass_cell_center(w: &World) -> Vec2 {
        let res = w.biome.res;
        let cell_size = w.biome.cell_size;
        for row in 0..res {
            for col in 0..res {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    return Vec2::new(
                        (col as f32 + 0.5) * cell_size,
                        (row as f32 + 0.5) * cell_size,
                    );
                }
            }
        }
        panic!("no grass cell in biome");
    }

    fn fertile_genome() -> Genome {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.4);
        g.set(GenomeSlot::Size, 0.4);
        g.set(GenomeSlot::BasalMetabolism, 0.4);
        g
    }

    #[test]
    fn two_adjacent_well_fed_agents_produce_offspring() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());

        // Give both ample energy.
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;

        // Build the spatial hash so find_mate can see them.
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();

        assert_eq!(after, before + 1, "expected exactly one offspring");
        // Each parent paid energy.
        assert!(w.agents.energy[id0 as usize] < SPAWN_ENERGY * 2.0);
        assert!(w.agents.energy[id1 as usize] < SPAWN_ENERGY * 2.0);
    }

    #[test]
    fn cross_species_pair_does_not_mate() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        // Force different species.
        w.agents.species_id[id1 as usize] = 1;
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;

        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();
        assert_eq!(after, before, "different species must not produce offspring");
    }

    #[test]
    fn population_cap_blocks_reproduction() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        // At the cap: no offspring.
        w.max_population = 2;
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), 2, "at cap: no offspring");

        // One slot free: exactly one offspring, then the cap bites again.
        w.max_population = 3;
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), 3, "one free slot: exactly one offspring");
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), 3, "cap holds on the next pass too");
    }

    #[test]
    fn inbreeding_meme_depresses_surviving_offspring_energy() {
        use crate::practice;
        // The newborn's starting energy, if the pair produced a surviving child
        // (an inbred child may be stillborn — a separate roll, tested below).
        let birth_energy = |inbreeding: bool, seed: u64| -> Option<f32> {
            let mut w = World::new(seed);
            w.cognition_enabled = true;
            let pos = find_grass_cell_center(&w);
            let a = w.spawn_agent(pos, fertile_genome());
            let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
            // Identical genomes → closeness 1 → maximal depression.
            w.agents.genome[b as usize] = w.agents.genome[a as usize];
            w.agents.energy[a as usize] = SPAWN_ENERGY * 2.0;
            w.agents.energy[b as usize] = SPAWN_ENERGY * 2.0;
            if inbreeding {
                w.agents.meme_vector[a as usize][practice::channel(practice::INBREEDING)] = 1.0;
            }
            w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
            reproduce_all(&mut w);
            (w.agents.live_count() == 3).then(|| w.agents.energy[2])
        };
        let control = birth_energy(false, 71).expect("control child always survives");
        assert!((control - SPAWN_ENERGY).abs() < 1e-4, "control child starts at spawn energy");
        // Among newborns that survive the stillbirth roll, starting energy is halved.
        let inbred = (0..64u64)
            .find_map(|s| birth_energy(true, s))
            .expect("some inbred child survives the stillbirth roll");
        assert!(
            (inbred - SPAWN_ENERGY * (1.0 - practice::INBREEDING_DEPRESSION)).abs() < 1e-3,
            "identical-parent inbreeding halves a surviving child's starting energy: {inbred}"
        );
    }

    #[test]
    fn inbreeding_stillbirth_culls_some_close_kin_offspring() {
        use crate::practice;
        // The viability half of inbreeding depression: over many seeds a fraction
        // of closeness-1 inbred newborns are stillborn, while a control rears all.
        let survivors = |inbreeding: bool| -> u32 {
            let mut count = 0;
            for seed in 0..80u64 {
                let mut w = World::new(2000 + seed);
                w.cognition_enabled = true;
                let pos = find_grass_cell_center(&w);
                let a = w.spawn_agent(pos, fertile_genome());
                let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
                w.agents.genome[b as usize] = w.agents.genome[a as usize];
                w.agents.energy[a as usize] = SPAWN_ENERGY * 2.0;
                w.agents.energy[b as usize] = SPAWN_ENERGY * 2.0;
                if inbreeding {
                    w.agents.meme_vector[a as usize][practice::channel(practice::INBREEDING)] = 1.0;
                }
                w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
                reproduce_all(&mut w);
                if w.agents.live_count() == 3 {
                    count += 1;
                }
            }
            count
        };
        let control = survivors(false);
        let inbred = survivors(true);
        assert_eq!(control, 80, "no practice → every close-kin pair rears its child");
        assert!(inbred < control, "inbreeding stillbirth culls some newborns: {inbred}/80");
        // ~INBREEDING_STILLBIRTH (0.45) at closeness 1 → roughly 44 of 80 survive.
        assert!((30..=60).contains(&inbred), "stillbirth rate in the expected band: {inbred}/80");
    }

    #[test]
    fn child_sacrifice_culls_about_half_of_newborns() {
        use crate::practice;
        let survivors = |sacrifice: bool| -> u32 {
            let mut count = 0;
            for seed in 0..80u64 {
                let mut w = World::new(1000 + seed);
                w.cognition_enabled = true;
                let pos = find_grass_cell_center(&w);
                let a = w.spawn_agent(pos, fertile_genome());
                let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
                w.agents.energy[a as usize] = SPAWN_ENERGY * 2.0;
                w.agents.energy[b as usize] = SPAWN_ENERGY * 2.0;
                if sacrifice {
                    w.agents.meme_vector[a as usize]
                        [practice::channel(practice::CHILD_SACRIFICE)] = 1.0;
                }
                w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
                reproduce_all(&mut w);
                if w.agents.live_count() == 3 {
                    count += 1; // the newborn survived
                }
            }
            count
        };
        let control = survivors(false);
        let sacrificed = survivors(true);
        assert_eq!(control, 80, "no practice → every pair rears its child");
        assert!(sacrificed < control, "child sacrifice culls some: {sacrificed}/80");
        assert!((20..=60).contains(&sacrificed), "roughly half survive: {sacrificed}/80");
    }

    #[test]
    fn species_count_stays_consistent_after_a_cull() {
        use crate::practice;
        // A child culled mid-`reproduce_all` (stillbirth / child-sacrifice) does
        // `kill` + `remove_from_species`, undoing the `add_to_species` from its
        // spawn. Verify `species_member_counts` still equals the true alive count
        // per species after such a birth — on both the culled and non-culled path.
        let alive_in_species = |w: &World, sid: u32| -> u32 {
            w.agents.iter_alive().filter(|&id| w.agents.species_id[id as usize] == sid).count()
                as u32
        };
        let mut saw_cull = false;
        for seed in 0..64u64 {
            let mut w = World::new(seed);
            w.cognition_enabled = true;
            let pos = find_grass_cell_center(&w);
            let a = w.spawn_agent(pos, fertile_genome());
            let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
            w.agents.energy[a as usize] = SPAWN_ENERGY * 2.0;
            w.agents.energy[b as usize] = SPAWN_ENERGY * 2.0;
            w.agents.meme_vector[a as usize][practice::channel(practice::CHILD_SACRIFICE)] = 1.0;
            w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
            reproduce_all(&mut w);
            let sid = w.agents.species_id[a as usize];
            assert_eq!(
                w.species_member_counts[sid as usize],
                alive_in_species(&w, sid),
                "species_member_counts must match the true alive count (seed {seed})"
            );
            if w.agents.live_count() == 2 {
                saw_cull = true; // confirmed the cull path was exercised
            }
        }
        assert!(saw_cull, "no seed produced a cull in 64 tries");
    }

    #[test]
    fn low_energy_pair_does_not_mate() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        // Below threshold.
        w.agents.energy[id0 as usize] = 1.0;
        w.agents.energy[id1 as usize] = 1.0;

        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();
        assert_eq!(after, before, "low-energy agents must not mate");
    }

    #[test]
    fn offspring_inherits_parent_lineages() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        let lin0 = w.agents.lineage_id[id0 as usize];
        let lin1 = w.agents.lineage_id[id1 as usize];

        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        reproduce_all(&mut w);

        // The newborn is the only agent with non-zero parent ids.
        let mut found = false;
        for id in w.agents.iter_alive() {
            let p = w.agents.parent_ids[id as usize];
            if p != [crate::agent::LINEAGE_NONE; 2] {
                assert_eq!(
                    {
                        let mut s = p;
                        s.sort();
                        s
                    },
                    {
                        let mut s = [lin0, lin1];
                        s.sort();
                        s
                    }
                );
                found = true;
            }
        }
        assert!(found, "offspring with parent ids not found");
    }

    #[test]
    fn agent_without_reproductive_does_not_mate() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());

        // Strip Reproductive from id0 only.
        w.agents.modules[id0 as usize]
            .retain(|m| !matches!(m, crate::module::Module::Reproductive { .. }));

        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        let after = w.agents.live_count();
        assert_eq!(after, before, "missing Reproductive must block mating");
    }

    #[test]
    fn dowry_blocks_then_permits_reproduction() {
        use crate::resource::{Good, DOWRY_REQ};
        let mut w = World::new(13);
        w.resources_enabled = true;
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        // No goods yet → no offspring despite ample energy.
        let before = w.agents.live_count();
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), before, "no dowry: no offspring");

        // Give both parents a full basket, then it must produce one offspring.
        for id in [id0, id1] {
            for g in Good::ALL {
                w.agents.inventory[id as usize][g.index()] = DOWRY_REQ;
            }
        }
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), before + 1, "full dowry: one offspring");
        // Dowry consumed from both parents.
        for id in [id0, id1] {
            for g in Good::ALL {
                assert_eq!(w.agents.inventory[id as usize][g.index()], 0.0, "dowry spent");
            }
        }
    }

    #[test]
    fn dowry_gate_is_inert_when_resources_disabled() {
        // With resources off, reproduction ignores inventory entirely (byte-identical path).
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let before = w.agents.live_count();
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), before + 1, "flag off: dowry not required");
    }

    #[test]
    fn dowry_birth_emits_event() {
        use crate::codex::EventType;
        use crate::resource::{Good, DOWRY_REQ};
        let mut w = World::new(13);
        w.resources_enabled = true;
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        for id in [id0, id1] {
            for g in Good::ALL {
                w.agents.inventory[id as usize][g.index()] = DOWRY_REQ;
            }
        }
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        reproduce_all(&mut w);
        assert!(
            w.codex.events.iter().any(|e| e.event_type == EventType::DowryBirth),
            "a DowryBirth event was recorded"
        );
    }
}
