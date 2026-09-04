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

/// Multiplier on `ReproductionThreshold × SPAWN_ENERGY` that sets the mating
/// energy gate (see `is_eligible`). Named so the affect layer's LUST trigger
/// (`affect::trigger_lust`) reads the same reference the gate uses.
pub const REPRO_ENERGY_MULT: f32 = 1.5;

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
            world.sexual_dimorphism_enabled,
        );
        let Some(b_id) = mate else { continue };

        let j = b_id as usize;
        let b_pos = world.agents.position[j];
        let b_genome = world.agents.genome[j];
        let b_lineage = world.agents.lineage_id[j];

        // Pay energy from both parents.
        let cost = SPAWN_ENERGY * PARENT_ENERGY_COST_FRAC;
        world.agents.energy[i] -= cost;
        world.agents.energy[j] -= cost;

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

        // `crossover_and_mutate` only reads the parents, so borrow their module
        // lists / programs in place instead of cloning them: `world.agents.*` and
        // `world.rng` are disjoint fields, so the shared reads coexist with the
        // `&mut rng`. Removes up to four heap allocations per birth; RNG order and
        // inputs are unchanged, so the result is bit-identical.
        let child_modules = crate::module::crossover_and_mutate(
            &world.agents.modules[i],
            &world.agents.modules[j],
            &mut world.rng,
        );

        let child_program = crate::program::crossover_and_mutate(
            &world.agents.program[i],
            &world.agents.program[j],
            &mut world.rng,
            world.war_enabled,
            world.settlement_enabled,
            world.anthro_race_enabled,
        );

        let lineage = world.next_lineage();
        // E12: child sex is a fresh 50/50 draw when dimorphism is active.
        // Gated on the flag so flag-off births draw zero extra RNG and the
        // pre-E12 stream is unchanged.
        let child_sex = world.sexual_dimorphism_enabled && world.rng.f32_unit() < 0.5;
        let child_id = world.agents.spawn(
            child_pos,
            child_genome,
            lineage,
            [a_lineage, b_lineage],
            a_species,
            child_modules,
            child_program,
            child_sex,
        );
        world.add_to_species(a_species);

        // Born domesticated (E13): a child of two livestock of the SAME
        // living owner is born tamed. No RNG; gated on the flag.
        if world.domestication_enabled {
            let pa = world.agents.livestock_of[i];
            if pa != crate::agent::AGENT_NULL
                && pa == world.agents.livestock_of[j]
                && world.agents.is_alive(pa)
            {
                world.agents.livestock_of[child_id as usize] = pa;
                // Arm the kill-time release (see domestication::husbandry_step).
                world.agents.track_livestock = true;
            }
        }

        // Anchor inheritance (E8): child anchor = parent-anchor midpoint +
        // drift, ONLY when settlement is enabled. Gated so flag-off draws
        // zero extra RNG (baseline streams unchanged).
        if world.settlement_enabled {
            let ws = world.world_size;
            let aa = world.agents.anchor[i];
            let ba = world.agents.anchor[j];
            // Torus-safe midpoint: walk from A halfway toward B.
            let d = crate::spatial::torus_delta(ba, aa, ws);
            let jx = world.rng.gaussian(0.0, crate::codex::ANCHOR_DRIFT_SIGMA);
            let jy = world.rng.gaussian(0.0, crate::codex::ANCHOR_DRIFT_SIGMA);
            world.agents.anchor[child_id as usize] = crate::prelude::Vec2::new(
                (aa.x + d.x * 0.5 + jx).rem_euclid(ws),
                (aa.y + d.y * 0.5 + jy).rem_euclid(ws),
            );
        }

        // Ensure the bitvec covers the new slot, mark the child as
        // "reproduced this tick" so they cannot immediately mate again.
        if world.reproduced_this_tick.len() <= child_id as usize {
            world.reproduced_this_tick.resize(child_id as usize + 1, false);
        }
        world.reproduced_this_tick.set(child_id as usize, true);

        // Meme inheritance (Communicator-gated; may draw meme RNG).
        inherit_child_meme(world, child_id, i, j);

        // Maladaptive-practice fitness costs (cognition-gated; may cull the child).
        let child_lost =
            apply_practice_fitness_costs(world, child_id, i, j, &a_genome, &b_genome, a_species);
        // O3 repro-biased learning: record the birth outcome on both parents.
        // Flag-gated so flag-off worlds keep all-zero counters (byte-identical
        // serialized state modulo the layout growth).
        if world.repro_biased_learning {
            if child_lost {
                world.agents.births_failed[i] = world.agents.births_failed[i].saturating_add(1);
                world.agents.births_failed[j] = world.agents.births_failed[j].saturating_add(1);
            } else {
                world.agents.births_ok[i] = world.agents.births_ok[i].saturating_add(1);
                world.agents.births_ok[j] = world.agents.births_ok[j].saturating_add(1);
            }
        }
    }
    world.agents.scratch_ids = alive_ids;
}

/// Meme inheritance: child meme = parent average + jitter, ONLY when the child
/// carries a Communicator module. Gating the whole block on the module keeps
/// non-communicator lineages (e.g. minimal.toml) drawing zero meme RNG, so the
/// golden hash stream is unchanged for them.
fn inherit_child_meme(world: &mut World, child_id: u32, i: usize, j: usize) {
    if !crate::module::has(
        &world.agents.modules[child_id as usize],
        crate::module::ModuleType::Communicator,
    ) {
        return;
    }
    let a_meme = world.agents.meme_vector[i];
    let b_meme = world.agents.meme_vector[j];
    let inventions_enabled = world.inventions_enabled;
    let cognition_enabled = world.cognition_enabled;
    // E9 institutional memory: when BOTH parents belong to a settlement-latched
    // species, inheritance jitter shrinks — settled cultures pass memes down
    // more faithfully.
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
    // Apes-only inventions: a non-ape child inherits no invention channels
    // (practice/base channels inherit normally). Runs AFTER inherit_meme so the
    // jitter draw count is unchanged — this only overwrites stored values.
    // `enforce_ape_only` already early-returns for apes and when the flag is
    // off, so it is safe (and avoids a split-borrow) to call unconditionally.
    let ci = child_id as usize;
    crate::invention::enforce_ape_only(
        &mut world.agents.meme_vector[ci],
        &world.agents.genome[ci],
        &world.agents.modules[ci],
        world.inventions_enabled,
    );
    // O3 repro-biased learning: vertical content bias — a child of a family
    // whose observed births fail at least as often as they succeed declines
    // the inherited practice channels (the custom dies with the grieving
    // family). Zero-only and AFTER inherit_meme, so RNG draw counts are
    // unchanged; flag off ⇒ byte-identical inheritance.
    if world.repro_biased_learning {
        let fail = world.agents.births_failed[i] as u32 + world.agents.births_failed[j] as u32;
        let ok = world.agents.births_ok[i] as u32 + world.agents.births_ok[j] as u32;
        if fail > 0 && fail >= ok {
            for p in 0..crate::practice::PRACTICE_COUNT {
                world.agents.meme_vector[ci][crate::practice::channel(p)] = 0.0;
            }
        }
    }
    // E9 lineage: the newborn's per-channel variants descend from its parents'
    // variants (band-matched) or are freshly minted.
    crate::codex::traditions::assign_birth_variants(world, child_id as usize, i, j);
}

/// Maladaptive-practice fitness costs (cognition-gated). A parent's held
/// practice damages the offspring's reproductive/genetic fitness. `a_genome`/
/// `b_genome` are the parents' pre-spawn genome snapshots; `a_species` is the
/// child's species (for the removal bookkeeping on a cull). Returns `true`
/// when the child was removed (stillborn or sacrificed) — the birth-outcome
/// signal for O3 repro-biased learning.
fn apply_practice_fitness_costs(
    world: &mut World,
    child_id: u32,
    i: usize,
    j: usize,
    a_genome: &Genome,
    b_genome: &Genome,
    a_species: crate::agent::SpeciesId,
) -> bool {
    if !world.cognition_enabled {
        return false;
    }
    use crate::practice::{self, CHILD_SACRIFICE, INBREEDING};
    let inbred = practice::has(&world.agents.meme_vector[i], INBREEDING)
        || practice::has(&world.agents.meme_vector[j], INBREEDING);
    let sacrifices = practice::has(&world.agents.meme_vector[i], CHILD_SACRIFICE)
        || practice::has(&world.agents.meme_vector[j], CHILD_SACRIFICE);
    // Inbreeding depression: a kin-mating custom expresses recessive genetic
    // load — the closer the parents, the frailer the child (energy) and the
    // likelier it is stillborn (viability). Paired with the kin-seeking mate
    // bias in `find_mate`, close pairings are common, so this bites at the
    // population level.
    let closeness = if inbred { practice::inbreeding_closeness(a_genome, b_genome) } else { 0.0 };
    if inbred {
        world.agents.energy[child_id as usize] *= 1.0 - practice::INBREEDING_DEPRESSION * closeness;
    }
    // Two independent lethal rolls; a child removed by either is removed exactly
    // once. `&&` short-circuits, so a non-inbreeding / non-sacrificing birth
    // draws no RNG (keeping unrelated scenarios' streams unchanged).
    let stillborn = inbred && world.rng.f32_unit() < practice::INBREEDING_STILLBIRTH * closeness;
    let sacrificed = sacrifices && world.rng.f32_unit() < practice::CHILD_SACRIFICE_CULL;
    if stillborn || sacrificed {
        world.agents.kill(child_id);
        world.remove_from_species(a_species);
    }
    stillborn || sacrificed
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
    // Basic needs: sleepers neither seek nor accept mates — lost mating time
    // is part of the cost of sleep (design doc §3). All-false when the flag
    // is off, so flag-off worlds are untouched.
    if agents.asleep[i] {
        return false;
    }
    // Conscientiousness raises the effective breeding threshold.
    let threshold = SPAWN_ENERGY
        * agents.genome[i].get(GenomeSlot::ReproductionThreshold)
        * REPRO_ENERGY_MULT
        * crate::personality::personality_reproduction_factor(&agents.genome[i])
        * crate::affect::affect_reproduction_factor(&agents.affect[i]);
    agents.energy[i] >= threshold
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
    dimorphism: bool,
) -> Option<u32> {
    let mut best: Option<u32> = None;
    // Genome distance of `best` — only consulted when `kin_seeking`.
    let mut best_gd = f32::INFINITY;
    let a_sex = agents.sex.get(a_id as usize).map(|b| *b).unwrap_or(false);
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
        if dimorphism {
            // E12: mating requires opposite sexes, and the female partner
            // (whichever side of the pair she is) applies her choosiness bar
            // to the male's display quality. Deterministic — no RNG here.
            let b_sex = agents.sex.get(j).map(|b| *b).unwrap_or(false);
            if a_sex == b_sex {
                return;
            }
            let (female, male) = if a_sex == crate::dimorphism::FEMALE {
                (a_genome, &agents.genome[j])
            } else {
                (&agents.genome[j], a_genome)
            };
            if !crate::dimorphism::female_accepts(female, male) {
                return;
            }
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
    let d = crate::spatial::torus_delta(b, a, world_size);
    let mid_x = (a.x + d.x * 0.5).rem_euclid(world_size);
    let mid_y = (a.y + d.y * 0.5).rem_euclid(world_size);
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

    /// Basic needs: a sleeping agent is not mate-eligible — lost mating time
    /// is part of the cost of sleep. Wake it and the same pair reproduces.
    #[test]
    fn sleeping_agents_do_not_mate() {
        let mut w = World::new(1000);
        w.basic_needs_enabled = true;
        let pos = find_grass_cell_center(&w);
        let a = w.spawn_agent(pos, fertile_genome());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        w.agents.energy[a as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[b as usize] = SPAWN_ENERGY * 2.0;
        w.agents.asleep.set(a as usize, true);
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), 2, "sleeper must not mate (either side)");

        w.agents.asleep.set(a as usize, false);
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), 3, "awake pair reproduces");
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
    fn repro_bias_vertical_filter_blocks_practice_inheritance() {
        use crate::practice;
        // A Communicator pair holding Child Sacrifice, whose observed births
        // have all failed. Returns the surviving child's inherited practice
        // level (None when the sacrifice roll culled the child at this seed).
        let child_practice = |seed: u64, flag: bool| -> Option<f32> {
            let mut w = World::new(seed);
            w.cognition_enabled = true;
            w.repro_biased_learning = flag;
            let pos = find_grass_cell_center(&w);
            let a = w.spawn_agent(pos, fertile_genome());
            let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
            for id in [a, b] {
                let i = id as usize;
                w.agents.modules[i]
                    .push(crate::module::Module::Communicator { range: 8.0, channel_id: 0 });
                w.agents.energy[i] = SPAWN_ENERGY * 2.0;
                w.agents.meme_vector[i][practice::channel(practice::CHILD_SACRIFICE)] = 1.0;
                w.agents.births_failed[i] = 3; // the family buries its children
            }
            w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
            reproduce_all(&mut w);
            (w.agents.live_count() == 3)
                .then(|| w.agents.meme_vector[2][practice::channel(practice::CHILD_SACRIFICE)])
        };
        // Find a seed where the flag-off child survives, carries a
        // Communicator, and inherits the practice — the control.
        let seed = (0..64u64)
            .find(|&s| child_practice(s, false).is_some_and(|v| v > 0.5))
            .expect("some seed yields a surviving communicator child inheriting the practice");
        // Same seed, flag on: identical RNG stream (the filter draws none), so
        // the same child is born — but declines the family's fatal custom.
        assert_eq!(
            child_practice(seed, true),
            Some(0.0),
            "vertical filter zeroes the inherited practice channel"
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
    fn non_ape_child_inherits_no_inventions() {
        use crate::invention::{self, channel};
        let mut w = World::new(31);
        w.inventions_enabled = true;
        let pos = find_grass_cell_center(&w);
        // Two parents holding Stone Tools.
        let a = w.spawn_agent(pos, fertile_genome());
        let b = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        // Give both parents a Communicator so the child gets one too, and Stone Tools.
        for &p in &[a, b] {
            w.agents.modules[p as usize]
                .push(crate::module::Module::Communicator { range: 10.0, channel_id: 0 });
            w.agents.meme_vector[p as usize][channel(invention::STONE_TOOLS)] = 1.0;
        }
        // Spawn a child slot and make it a NON-ape (herbivore Mouth), Communicator.
        let child = w.spawn_agent(pos, fertile_genome());
        let mut kit = crate::module::ModuleList::new();
        kit.push(crate::module::Module::Mouth { bite_size: 0.6, diet_affinity: 0.0 }); // non-ape
        kit.push(crate::module::Module::Communicator { range: 10.0, channel_id: 0 });
        w.agents.modules[child as usize] = kit;

        inherit_child_meme(&mut w, child, a as usize, b as usize);

        assert_eq!(
            w.agents.meme_vector[child as usize][channel(invention::STONE_TOOLS)],
            0.0,
            "a non-ape child must inherit no inventions"
        );
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
    fn opposite_sexes_mate_when_dimorphism_on() {
        let mut w = World::new(13);
        w.sexual_dimorphism_enabled = true;
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        w.agents.sex.set(id0 as usize, crate::dimorphism::FEMALE);
        w.agents.sex.set(id1 as usize, crate::dimorphism::MALE);
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), before + 1, "female + male must mate");
    }

    #[test]
    fn same_sex_pair_does_not_mate_when_dimorphism_on() {
        for sex in [crate::dimorphism::FEMALE, crate::dimorphism::MALE] {
            let mut w = World::new(13);
            w.sexual_dimorphism_enabled = true;
            let pos = find_grass_cell_center(&w);
            let id0 = w.spawn_agent(pos, fertile_genome());
            let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
            w.agents.sex.set(id0 as usize, sex);
            w.agents.sex.set(id1 as usize, sex);
            w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
            w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
            w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

            let before = w.agents.live_count();
            reproduce_all(&mut w);
            assert_eq!(w.agents.live_count(), before, "same-sex pair must not mate");
        }
    }

    #[test]
    fn choosy_female_rejects_low_quality_male() {
        // fertile_genome size 0.4 → male display quality 0.4×1.3 = 0.52.
        // Choosiness 0.9 → bar 0.72: rejected. Choosiness 0.5 → bar 0.4: ok.
        let mates_with = |choosiness: f32| -> bool {
            let mut w = World::new(13);
            w.sexual_dimorphism_enabled = true;
            let pos = find_grass_cell_center(&w);
            let mut fg = fertile_genome();
            fg.set(GenomeSlot::MateChoosiness, choosiness);
            let id0 = w.spawn_agent(pos, fg);
            let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fg);
            w.agents.sex.set(id0 as usize, crate::dimorphism::FEMALE);
            w.agents.sex.set(id1 as usize, crate::dimorphism::MALE);
            w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
            w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
            w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
            let before = w.agents.live_count();
            reproduce_all(&mut w);
            w.agents.live_count() == before + 1
        };
        assert!(mates_with(0.5), "neutral choosiness accepts the neutral male");
        assert!(!mates_with(0.9), "choosy female rejects the low-quality male");
    }

    #[test]
    fn flag_off_leaves_all_agents_female_and_mating_sexless() {
        let mut w = World::new(13);
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        assert!(!w.agents.sex[id0 as usize] && !w.agents.sex[id1 as usize]);
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        let before = w.agents.live_count();
        reproduce_all(&mut w);
        assert_eq!(w.agents.live_count(), before + 1, "flag off: sex bits unread");
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
    fn resources_do_not_gate_reproduction() {
        // The trade economy funds MEME learning, not births: with resources
        // on, empty inventories must NOT block mating, and reproduction must
        // not touch inventory.
        let mut w = World::new(13);
        w.resources_enabled = true;
        let pos = find_grass_cell_center(&w);
        let id0 = w.spawn_agent(pos, fertile_genome());
        let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
        w.agents.energy[id0 as usize] = SPAWN_ENERGY * 2.0;
        w.agents.energy[id1 as usize] = SPAWN_ENERGY * 2.0;
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));

        let before = w.agents.live_count();
        reproduce_all(&mut w);
        assert_eq!(
            w.agents.live_count(),
            before + 1,
            "empty inventory must not block reproduction"
        );
        for id in [id0, id1] {
            assert_eq!(
                w.agents.inventory[id as usize],
                [0.0; crate::resource::GOOD_COUNT],
                "reproduction must not consume goods"
            );
        }
    }

    #[test]
    fn reproduction_is_unaffected_when_resources_disabled() {
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
        assert_eq!(w.agents.live_count(), before + 1, "flag off: reproduction unaffected");
    }

    #[test]
    fn high_lust_lets_a_below_threshold_pair_mate() {
        // Energy set between the LUST-lowered gate and the neutral gate: they mate
        // only when LUST is high.
        let mates = |lust: f32| -> bool {
            let mut w = World::new(13);
            let pos = find_grass_cell_center(&w);
            let id0 = w.spawn_agent(pos, fertile_genome());
            let id1 = w.spawn_agent(Vec2::new(pos.x + 0.5, pos.y), fertile_genome());
            // fertile_genome: ReproductionThreshold 0.4 → neutral gate =
            // SPAWN_ENERGY*0.4*1.5 = 0.6*SPAWN_ENERGY. LUST=1 gate = 0.7×that.
            let gate = SPAWN_ENERGY * 0.4 * crate::reproduce::REPRO_ENERGY_MULT;
            let e = gate * 0.85; // below neutral gate, above the LUST-lowered gate
            w.agents.energy[id0 as usize] = e;
            w.agents.energy[id1 as usize] = e;
            w.agents.affect[id0 as usize][crate::affect::LUST] = lust;
            w.agents.affect[id1 as usize][crate::affect::LUST] = lust;
            w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
            let before = w.agents.live_count();
            reproduce_all(&mut w);
            w.agents.live_count() == before + 1
        };
        assert!(!mates(0.0), "neutral LUST: below-gate pair must not mate");
        assert!(mates(1.0), "high LUST lowers the gate enough to mate");
    }
}
