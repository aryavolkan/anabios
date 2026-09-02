//! Tick orchestration: the master `step()` function that advances the whole
//! simulation by one tick.

use crate::age::age_and_starve;
use crate::behavior::decide;
use crate::integrate::integrate_all;
use crate::interact::interact_all;
use crate::prelude::Vec2;
use crate::sense::sense_all;
use crate::world::World;

/// How often (in ticks) the biome plant regrowth step runs.
pub const BIOME_STEP_INTERVAL: u64 = 10;

/// Advance the world by one tick.
pub fn step(world: &mut World) {
    world.resize_scratch();
    let cap = world.agents.capacity();

    // Stage 1: rebuild the spatial hash from current positions.
    world.spatial.rebuild(&world.agents.position, |i| world.agents.is_alive(i as u32));

    // Stage 2: sense. The culture-lineage mask (anthropogenic arms race) is
    // refreshed first — a no-op leaving an empty mask when the flag is off.
    world.refresh_culture_mask();
    sense_all(
        &world.agents,
        &world.biome,
        &world.pheromones,
        &world.spatial,
        &world.codex.hostility,
        &world.culture_mask,
        &mut world.sensors,
        world.world_size,
        world.gene_tech_coupling,
    );

    // Stage 2b: subcortical affect — update per-agent Panksepp activations from
    // this tick's physiology (SEEKING from the energy deficit in M-A). Runs
    // AFTER sense and BEFORE decide so the decision reads fresh affect. Strict
    // no-op + zero RNG when `affect_enabled` is false.
    crate::affect::develop_all(world);

    // Stage 3: decide.
    decide_all(world);

    // Stage 4: integrate (motion + per-tick metabolism).
    integrate_all(
        &mut world.agents,
        &world.desired_direction[..cap],
        world.world_size,
        world.sexual_dimorphism_enabled,
        world.gene_tech_coupling,
    );

    // Stage 4b: E6 ambush instrumentation — consecutive still ticks per
    // agent, read by `combat_pass` in the interact stage. Observability only.
    crate::codex::signatures::update_still_ticks(world);

    // Stage 4c: E8 anchor learning + market-field decay (opt-in; no-ops
    // when settlement/resources are disabled).
    crate::settlement::anchor_step(world);
    crate::settlement::market_decay_step(world);

    // Stage 5: interact (feeding, combat, predation).
    interact_all(world);

    // M3: module upkeep — every alive agent pays for its modules.
    crate::module::upkeep_all(&mut world.agents);

    // Stage 5b: cognitive development — juveniles fold this tick's nutrition
    // (post-feeding energy) + social enrichment (this tick's sensed crowding)
    // into their realized IQ. Runs before reproduce so newborns are processed
    // starting next tick. No-op when `cognition_enabled` is false.
    crate::iq::develop_all(world);

    // Stage 6: reproduce. Mutates the alive set; do not rely on `cap` after
    // this point.
    crate::reproduce::reproduce_all(world);

    // Stage 6b: culture — meme transmission between communicators (§3.7 step 7).
    crate::culture::culture_step(world);

    // Stage 6c: inventions — discovery rolls + per-holder upkeep/income/
    // stress/pollution (opt-in; no-op when `inventions_enabled` is false).
    crate::invention::invention_step(world);

    // Stage 6d: maladaptive-practice discovery — inventive agents can stumble
    // onto a harmful custom (opt-in; no-op when `cognition_enabled` is false).
    crate::practice::discover_step(world);

    // Stage 6e: husbandry — orphan sweep, milk yields, taming rolls (opt-in;
    // no-op and zero RNG draws when `domestication_enabled` is false).
    crate::domestication::husbandry_step(world);

    // Stage 6f: knowledge accumulation — per-species tech memory rises while
    // any member holds Writing, decays otherwise (opt-in; no-op and zero RNG
    // draws when `knowledge_enabled` is false).
    crate::knowledge::knowledge_step(world);

    // Stage 6g: disease — crowding-seeded SIS pathogen (spillover, proximity
    // spread, energy drain). Opt-in; no-op and zero RNG draws when
    // `disease_enabled` is false. Runs before age+starve so infection deaths
    // funnel through the existing carcass/starve path.
    crate::disease::disease_step(world);

    // Keep scratch sized to the post-reproduce capacity so end-of-tick detectors
    // (AlarmCall) that read actions/sensors/desired_direction see every agent —
    // reproduce (stage 6) can grow capacity past the top-of-tick resize.
    world.resize_scratch();

    // Stage 7: age + starve.
    age_and_starve(world);

    // Stage 7b: carcass aging + removal (design step 9 analogue).
    crate::carcass::carcass_step(world);

    // Stage 7c: conserve trade goods on death (opt-in; no-op + buffer drain
    // when off). Runs after all death stages so it captures this tick's deaths.
    crate::resource::conserve_goods_step(world);

    // Stage 8c: pheromone field decay (design §3.7 step 9).
    world.pheromones.decay_step();

    // Stage 8d: disasters — scheduler + propagation (opt-in; no-op and zero
    // RNG draws when `disasters_enabled` is false).
    crate::disaster::disaster_step(world);

    // Stage 8: periodic species clustering.
    if world.tick.is_multiple_of(crate::species::SPECIES_STEP_INTERVAL) {
        crate::species::species_step(world);
    }

    // Stage 9: codex detectors (extinction, population crash, etc.). Runs every
    // tick by default; `codex_interval > 1` samples it every N ticks instead
    // (opt-in throughput lever for large sweeps). `<= 1` keeps the every-tick
    // path — and avoids `is_multiple_of(0)` — so the default is bit-identical.
    if world.codex_interval <= 1 || world.tick.is_multiple_of(world.codex_interval) {
        crate::codex::observe_all(world);
    }

    // Stage 10: periodic biome regrowth (+ recolonization in a living biome).
    if world.tick.is_multiple_of(BIOME_STEP_INTERVAL) {
        let sf = world.soil_fertility;
        if world.living_biome {
            world.biome.recolonize_step(sf);
        }
        if world.season_period > 0 {
            let phase = crate::biome::season_phase(world.tick, world.season_period);
            world.biome.regrow_step_seasonal(phase, sf);
        } else {
            world.biome.regrow_step(sf);
        }
        // Stage 10b: resource node spawn/cleanup (opt-in; no-op when off).
        crate::resource::resource_step(world);
    }

    world.tick += 1;
}

fn decide_all(world: &mut World) {
    use rayon::prelude::*;
    // Each agent's action is a pure function of its own program/genome/sensors
    // plus the (read-only) biome, so the loop runs in parallel with
    // index-disjoint writes into `actions` / `desired_direction` — results are
    // bit-identical to the old serial ascending-id loop. `map_init` gives each
    // rayon worker one reusable eval stack (replacing the former shared
    // `eval_stack` scratch on World).
    let agents = &world.agents;
    let sensors = &world.sensors;
    let biome = &world.biome;
    let biome_adaptation = world.biome_adaptation;
    let terrain_habitat = world.terrain_habitat;
    let trade_hubs = &world.trade_hubs;
    let resources_enabled = world.resources_enabled;
    let settlement_enabled = world.settlement_enabled;
    let domestication_enabled = world.domestication_enabled;
    let affect_enabled = world.affect_enabled;
    let ws = world.world_size;
    let cap = world.agents.capacity();
    world
        .actions
        .par_iter_mut()
        .zip(world.desired_direction.par_iter_mut())
        .enumerate()
        .map_init(Vec::new, |stack, (i, (action_out, dir_out))| {
            if i >= cap || !agents.is_alive(i as u32) {
                // Dead/out-of-range slots get clean scratch, not stale:
                // `actions`/`desired_direction` are `#[serde(skip)]`, so a
                // stale dead-slot value survives a continuous run but reads
                // as default after a snapshot load — and a newborn reusing
                // the slot mid-tick (reproduce, stage 6) is then read at
                // observe time with divergent scratch (AlarmCall reads
                // `actions`, StructuredSignaling reads `desired_direction`,
                // the codex agg reads `sensors`). Same invariant as the
                // dead-slot zeroing in `sense_all` / `codex::signatures`.
                *action_out = crate::program::ActionRegister::default();
                *dir_out = Vec2::ZERO;
                return;
            }
            let mut action = decide(
                &agents.program[i],
                &agents.genome[i],
                &sensors[i],
                &agents.meme_vector[i],
                agents.energy[i],
                agents.age[i],
                agents.anchor[i],
                agents.position[i],
                ws,
                stack,
            );
            // Personality modulation of the raw action intents (Big Five).
            crate::personality::apply_personality(
                &mut action,
                &agents.genome[i],
                &sensors[i],
                agents.energy[i],
            );
            // Subcortical affect bias (SEEKING in M-A): steer/intensify movement
            // from the agent's current affect. Exact identity at neutral affect.
            crate::affect::apply_affect(
                &mut action,
                &agents.affect[i],
                &agents.genome[i],
                &sensors[i],
                agents.energy[i],
            );
            // Habitat selection (opt-in): bias movement toward the nearby cell whose
            // climate best matches this agent's EnvAffinity, so lineages sort into
            // their preferred zone. Gated on the flag so flag-off stays byte-identical.
            if biome_adaptation {
                let affinity = agents.genome[i].get(crate::genome::GenomeSlot::EnvAffinity);
                let pull = crate::biome::best_env_direction(
                    biome,
                    agents.position[i],
                    affinity,
                    crate::culture::HABITAT_REACH,
                );
                action.move_x += crate::culture::HABITAT_PULL * pull.x;
                action.move_y += crate::culture::HABITAT_PULL * pull.y;
            }
            // Terrain habitat selection (opt-in): bias movement toward the
            // nearest cell of this agent's TerrainAffinity-preferred terrain, so
            // species sort into biomes (and trade at borders). Gated so flag-off
            // stays byte-identical.
            if terrain_habitat {
                let aff = agents.genome[i].get(crate::genome::GenomeSlot::TerrainAffinity);
                let target = crate::resource::preferred_good(aff).home_terrain();
                let pull = crate::biome::best_terrain_direction(
                    biome,
                    agents.position[i],
                    target,
                    crate::culture::TERRAIN_HABITAT_REACH,
                );
                action.move_x += crate::culture::TERRAIN_HABITAT_PULL * pull.x;
                action.move_y += crate::culture::TERRAIN_HABITAT_PULL * pull.y;
            }
            // Trade-hub seeking (opt-in with the trade-goods subsystem): agents
            // with a real trade motive steer toward the nearest predetermined
            // hub so barter partners converge there. Motiveless agents forage
            // normally. Additive bias, normalized with the rest of the stack.
            // Skip the whole block on a hubless map (best_hub_direction would
            // just return ZERO): saves the has_trade_motive scan, byte-identical.
            if resources_enabled
                && !trade_hubs.is_empty()
                && crate::hub::has_trade_motive(&agents.inventory[i])
            {
                let pull = crate::hub::best_hub_direction(trade_hubs, agents.position[i], ws);
                action.move_x += crate::hub::HUB_PULL * pull.x;
                action.move_y += crate::hub::HUB_PULL * pull.y;
            }
            // Home-range anchoring (E8, opt-in): bias movement toward the
            // learned anchor, so species keep a place they return to. Gated
            // so flag-off stays byte-identical.
            if settlement_enabled {
                let pull = crate::settlement::anchor_pull_parts(
                    agents.anchor[i],
                    agents.position[i],
                    agents.genome[i].get(crate::genome::GenomeSlot::Territoriality),
                    ws,
                );
                action.move_x += pull.x;
                action.move_y += pull.y;
            }
            // Livestock pen override (E13, opt-in): a tamed animal with a
            // living owner ignores its program's movement entirely — it is
            // pulled back beyond the pen radius and stands to graze inside
            // it. Feeding/combat intents are untouched (grazing is
            // position-based). Gated so flag-off stays byte-identical.
            if domestication_enabled {
                let owner = agents.livestock_of[i];
                if owner != crate::agent::AGENT_NULL && agents.is_alive(owner) {
                    let pull = crate::domestication::pen_pull_parts(
                        agents.position[owner as usize],
                        agents.position[i],
                        ws,
                    );
                    action.move_x = pull.x;
                    action.move_y = pull.y;
                }
            }
            // Survival-reflex hijack (M-B, opt-in): under high threat-arousal the
            // Bracha reflex overwrites the movement + defensive intents chosen so
            // far. Gated so flag-off is byte-identical; zero RNG. Runs after every
            // movement bias, before desired_direction normalization.
            if affect_enabled {
                crate::affect::apply_hijack(
                    &mut action,
                    &agents.affect[i],
                    &agents.genome[i],
                    &sensors[i],
                    agents.energy[i],
                );
            }
            // Normalize the movement intent to a unit direction (identical to the
            // pre-M11 logic that lived inside `decide`). Guard against a non-finite
            // intent (an evolved program can overflow to `inf`; `inf/inf` would make
            // the direction `NaN` and corrupt the agent's position) — treat it as
            // no movement.
            let v = Vec2::new(action.move_x, action.move_y);
            let len = v.length();
            *dir_out = if len < 1e-4 || !v.is_finite() { Vec2::ZERO } else { v / len };
            *action_out = action;
        })
        .count();
}

#[cfg(test)]
mod tests {
    use crate::biome::TerrainType;
    use crate::genome::{Genome, GenomeSlot};
    use crate::prelude::Vec2;
    use crate::world::World;

    use super::decide_all;
    use super::step;

    #[test]
    fn empty_world_can_tick() {
        let mut w = World::new(1);
        for _ in 0..100 {
            step(&mut w);
        }
        assert_eq!(w.tick, 100);
    }

    #[test]
    fn agent_in_food_rich_world_survives_initial_ticks() {
        let mut w = World::new(13);
        // Find a grass cell to spawn near.
        let mut spawn = Vec2::ZERO;
        'outer: for row in 0..crate::biome::BIOME_RES {
            for col in 0..crate::biome::BIOME_RES {
                if w.biome.at(col, row).terrain == TerrainType::Grass {
                    spawn = Vec2::new(
                        (col as f32 + 0.5) * crate::biome::CELL_SIZE,
                        (row as f32 + 0.5) * crate::biome::CELL_SIZE,
                    );
                    break 'outer;
                }
            }
        }
        let mut g = Genome::neutral();
        g.set(GenomeSlot::DietCarnivory, 0.0);
        g.set(GenomeSlot::LifespanBias, 1.0);
        let id = w.spawn_agent(spawn, g);
        for _ in 0..200 {
            step(&mut w);
            if !w.agents.is_alive(id) {
                break;
            }
        }
        assert!(w.agents.is_alive(id), "well-fed agent on grass should survive 200 ticks");
    }

    #[test]
    fn starving_agent_dies() {
        let mut w = World::new(1);
        // Pin the agent on a barren cell with no speed so it can't graze its
        // way back to life, then drain its energy to a sliver.
        let mut spawn = Vec2::ZERO;
        'outer: for row in 0..crate::biome::BIOME_RES {
            for col in 0..crate::biome::BIOME_RES {
                if w.biome.at(col, row).plant_biomass <= 0.0 {
                    spawn = Vec2::new(
                        (col as f32 + 0.5) * crate::biome::CELL_SIZE,
                        (row as f32 + 0.5) * crate::biome::CELL_SIZE,
                    );
                    break 'outer;
                }
            }
        }
        let g = Genome::neutral();
        let id = w.spawn_agent(spawn, g);
        // Strip Locomotor so the agent can't graze its way back to life.
        w.agents.modules[id as usize]
            .retain(|m| !matches!(m, crate::module::Module::Locomotor { .. }));
        w.agents.energy[id as usize] = 0.5;
        for _ in 0..200 {
            step(&mut w);
            if !w.agents.is_alive(id) {
                break;
            }
        }
        assert!(!w.agents.is_alive(id));
    }

    #[test]
    fn dead_slots_get_clean_decide_scratch() {
        // Snapshot-restore invariant: after a tick, a dead slot's
        // actions/desired_direction are defaults (not the occupant's stale
        // values), so a newborn reusing the slot mid-tick is observed with
        // the same scratch a snapshot-loaded world would have.
        let mut w = World::new(1);
        let prog = crate::program::Program::from_slice(&[
            crate::program::Node::Const(1.0),
            crate::program::Node::FireWeapon,
        ]);
        let a = w.spawn_agent(crate::prelude::Vec2::new(400.0, 400.0), Genome::neutral());
        w.agents.program[a as usize] = prog;
        step(&mut w);
        assert!(w.actions[a as usize].fire_intent > 0.0, "scratch written while alive");

        w.agents.kill(a);
        step(&mut w);
        assert_eq!(w.actions[a as usize].fire_intent, 0.0, "dead slot action reset");
        assert_eq!(
            w.desired_direction[a as usize],
            crate::prelude::Vec2::ZERO,
            "dead slot direction reset"
        );
    }

    #[test]
    fn decide_populates_action_buffer_with_target() {
        use crate::program::{Node, Program, NO_TARGET};
        let mut w = World::new(1);
        // A program that always fires with intent 1.0.
        let prog = Program::from_slice(&[Node::Const(1.0), Node::FireWeapon]);
        let a = w.spawn_agent(Vec2::new(400.0, 400.0), Genome::neutral());
        let b = w.spawn_agent(Vec2::new(404.0, 400.0), Genome::neutral());
        w.agents.program[a as usize] = prog;
        // One tick runs sense -> decide; afterwards actions[a] reflects the program.
        step(&mut w);
        assert!(w.actions[a as usize].fire_intent > 0.0);
        // a's nearest neighbor is b, so target should be b (not NO_TARGET).
        assert_eq!(w.actions[a as usize].target_id, b);
        assert_ne!(w.actions[a as usize].target_id, NO_TARGET);
    }

    #[test]
    fn hijack_overrides_movement_in_decide_all_when_flag_on() {
        use crate::affect::FEAR;
        use crate::genome::Genome;
        use crate::prelude::Vec2;
        use crate::world::World;

        // A prey with high FEAR + a distant threatening other-species sensor. With
        // affect ON, the decide_all hijack must override its heading with Freeze
        // (zero); with affect OFF, only the ungated apply_affect flee bias applies,
        // leaving a non-zero heading.
        let build = |affect_on: bool| -> Vec2 {
            let mut w = World::new(9);
            w.affect_enabled = affect_on;
            let prey = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
            w.resize_scratch();
            {
                let s = &mut w.sensors[prey as usize];
                s.nearest_other_id = 42;
                s.nearest_other_dist = 150.0; // >= FREEZE_DIST(140) ⇒ Freeze
                s.nearest_other_dir = Vec2::new(1.0, 0.0);
                s.nearest_rel_size = 2.0;
            }
            w.agents.affect[prey as usize][FEAR] = 0.95; // arousal above hijack threshold
            decide_all(&mut w);
            w.desired_direction[prey as usize]
        };
        let off = build(false);
        let on = build(true);
        assert_ne!(on, off, "the gated hijack must change the frightened prey's heading");
        // Distant threat + high arousal ⇒ hijack Freeze: desired heading zeroed.
        // apply_affect (ungated, runs in BOTH builds) only ADDS a flee bias and
        // never zeroes movement, so a zero heading isolates the gated hijack.
        assert_eq!(
            on,
            Vec2::ZERO,
            "high-arousal distant threat ⇒ hijack Freeze (zero heading), got {on:?}"
        );
        assert_ne!(off, Vec2::ZERO, "flag off ⇒ no hijack; prey still has a (flee) heading");
    }
}
