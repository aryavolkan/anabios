//! E6 named-behavior instrumentation and detectors: ambush, tool use,
//! flight (fast barrier crossing), and structured signaling.
//!
//! `update_still_ticks` runs between integrate and interact (tick stage 4b)
//! so `combat_pass` can tag each hit with fire-time behavioral context.

use super::*;
use crate::biome::TerrainType;

/// Consecutive ticks each agent has been below the still-speed threshold.
/// Observability only — does not feed back into the sim.
pub(crate) fn update_still_ticks(world: &mut World) {
    let cap = world.agents.capacity();
    if world.still_ticks.len() < cap {
        world.still_ticks.resize(cap, 0);
    }
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let module_speed = crate::module::effective_speed_max(&world.agents.modules[i]);
        let speed = world.agents.velocity[i].length();
        if module_speed > 0.0
            && speed < STILL_SPEED_FRAC * crate::integrate::SPEED_MAX_CAP * module_speed
        {
            world.still_ticks[i] = world.still_ticks[i].saturating_add(1);
        } else {
            world.still_ticks[i] = 0;
        }
    }
}

pub(super) fn detect_ambush_and_tool(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    // Prune the rolling log and rebuild per-species counters.
    while let Some(front) = world.codex.sig_hit_log.front() {
        if tick.saturating_sub(front.tick) >= SIG_HIT_WINDOW {
            world.codex.sig_hit_log.pop_front();
        } else {
            break;
        }
    }
    let mut total: BTreeMap<u32, u32> = BTreeMap::new();
    let mut ambush: BTreeMap<u32, u32> = BTreeMap::new();
    let mut tool: BTreeMap<u32, u32> = BTreeMap::new();
    for hit in world.codex.sig_hit_log.iter() {
        *total.entry(hit.species).or_insert(0) += 1;
        if hit.ambush {
            *ambush.entry(hit.species).or_insert(0) += 1;
        }
        if hit.tool_boosted {
            *tool.entry(hit.species).or_insert(0) += 1;
        }
    }

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, n) in total {
        let a = ambush.get(&sid).copied().unwrap_or(0);
        let t = tool.get(&sid).copied().unwrap_or(0);
        let ambush_share = a as f32 / n as f32;
        let tool_share = t as f32 / n as f32;

        let ambush_fired = n >= AMBUSH_MIN_HITS && ambush_share >= AMBUSH_MIN_SHARE;
        if let Some(ev) =
            edge_trigger_species(&mut world.codex.ambush_active, sid, ambush_fired, || {
                let (lx, ly) = centroid_of(agg, sid);
                CodexEvent {
                    event_type: EventType::EvolvedAmbush,
                    tick,
                    species_id: sid,
                    value: ambush_share,
                    loc_x: lx,
                    loc_y: ly,
                }
            })
        {
            to_push.push(ev);
        }

        let tool_fired = if world.inventions_enabled {
            // Adoption put to work: ≥30% Metalworking adoption in the
            // species AND at least one invention-boosted hit in the window.
            let adopted = agg.get(sid).map(|e| {
                e.invention_counts[crate::invention::METALWORKING] as f32 / e.count.max(1) as f32
                    >= TOOL_ADOPTION_SHARE
            });
            t >= 1 && adopted == Some(true)
        } else {
            false
        };
        if let Some(ev) =
            edge_trigger_species(&mut world.codex.tool_active, sid, tool_fired, || {
                let (lx, ly) = centroid_of(agg, sid);
                CodexEvent {
                    event_type: EventType::EvolvedTool,
                    tick,
                    species_id: sid,
                    value: tool_share,
                    loc_x: lx,
                    loc_y: ly,
                }
            })
        {
            to_push.push(ev);
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

pub(super) fn detect_flight(world: &mut World, agg: &SpeciesAggTable) {
    let tick = world.tick;
    // Count this tick's fast barrier-crossers per species.
    let cap = world.agents.capacity();
    if world.agents.velocity.len() < cap {
        return; // standalone observe_all outside the tick
    }
    let mut crossings: BTreeMap<u32, u32> = BTreeMap::new();
    for id in world.agents.iter_alive() {
        let i = id as usize;
        let module_speed = crate::module::effective_speed_max(&world.agents.modules[i]);
        if module_speed <= 0.0 {
            continue;
        }
        let speed = world.agents.velocity[i].length();
        if speed < FLIGHT_SPEED_FRAC * crate::integrate::SPEED_MAX_CAP * module_speed {
            continue;
        }
        let (col, row) = world.biome.cell_coords(world.agents.position[i]);
        match world.biome.at(col, row).terrain {
            TerrainType::Water | TerrainType::Rock => {
                *crossings.entry(world.agents.species_id[i]).or_insert(0) += 1;
            }
            _ => {}
        }
    }
    // Append to per-species logs, prune, evaluate.
    for (sid, n) in crossings {
        let buf = world.codex.flight_log.entry(sid).or_default();
        buf.push_back((tick, n));
        while buf.front().map(|(t, _)| tick.saturating_sub(*t) >= SIG_HIT_WINDOW).unwrap_or(false) {
            buf.pop_front();
        }
    }
    let mut to_push: Vec<CodexEvent> = Vec::new();
    // Evolutionary significance: the species must be meaningfully faster
    // than the world mean (relative adaptation), not just fast in absolute
    // terms — water-crossing at speed everyone can match is not flight.
    let mut world_speed_sum = 0.0_f64;
    let mut world_count = 0_u32;
    for &sid in agg.active() {
        let e = agg.get(sid).expect("active species has an entry");
        world_speed_sum += e.speed_sum;
        world_count += e.count;
    }
    let world_mean_speed = if world_count > 0 { world_speed_sum / world_count as f64 } else { 0.0 };
    for (sid, buf) in world.codex.flight_log.iter() {
        let total: u32 = buf.iter().map(|(_, n)| n).sum();
        // Sustained behavior: every quarter of the window must show
        // crossings, so a single burst doesn't qualify.
        let oldest = buf.front().map(|(t, _)| *t).unwrap_or(tick);
        let mut quarters = [0_u32; 4];
        for (t, n) in buf.iter() {
            let q = ((t.saturating_sub(oldest)) * 4 / SIG_HIT_WINDOW).min(3) as usize;
            quarters[q] += n;
        }
        let sustained = buf.len() as u64 >= SIG_HIT_WINDOW / 4
            && quarters.iter().all(|&q| q >= FLIGHT_MIN_PER_QUARTER);
        let species_mean_speed =
            agg.get(*sid).map(|e| e.speed_sum / e.count.max(1) as f64).unwrap_or(0.0);
        let faster = world_mean_speed > 0.0
            && species_mean_speed >= FLIGHT_RELATIVE_SPEED * world_mean_speed;
        let fired = total >= FLIGHT_MIN_CROSSINGS && sustained && faster;
        // Lineage-root latch: one flight event per lineage — speciation
        // splinters of a fast lineage don't each get their own.
        let root = lineage_root(world, *sid);
        if fired && !world.codex.flight_active.contains(&root) {
            world.codex.flight_active.insert(root);
            let (lx, ly) = centroid_of(agg, *sid);
            to_push.push(CodexEvent {
                event_type: EventType::EvolvedFlight,
                tick,
                species_id: *sid,
                value: total as f32,
                loc_x: lx,
                loc_y: ly,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

/// Walk `species_parents` to the lineage root (cycle-guarded).
fn lineage_root(world: &World, sid: u32) -> u32 {
    let mut cur = sid;
    for _ in 0..64 {
        match world.species_parents.get(cur as usize).copied().flatten() {
            Some(p) if p != cur => cur = p,
            _ => break,
        }
    }
    cur
}

/// Structured signaling: a Communicator broadcast (any meme channel) after
/// which ≥ `SIGNAL_CONVERGE_MIN` same-species receivers steer toward the
/// caller. Mirrors the AlarmCall machinery with convergence instead of
/// fleeing. Cumulative per species; fires once per species.
pub(super) fn detect_structured_signaling(world: &mut World) {
    let cap = world.agents.capacity();
    if world.actions.len() < cap || world.sensors.len() < cap || world.desired_direction.len() < cap
    {
        return;
    }
    let tick = world.tick;
    let mut alive_ids = std::mem::take(&mut world.agents.scratch_ids);
    alive_ids.clear();
    alive_ids.extend(world.agents.iter_alive());

    let mut responses: Vec<(u32, (f32, f32))> = Vec::new();
    for &id in &alive_ids {
        let i = id as usize;
        if !crate::module::has(&world.agents.modules[i], crate::module::ModuleType::Communicator) {
            continue;
        }
        // Broadcasting on ANY meme channel above threshold.
        let broadcasting = world.actions[i]
            .broadcast_intent
            .iter()
            .any(|&b| b > crate::culture::MEME_BROADCAST_THRESHOLD);
        if !broadcasting {
            continue;
        }
        let range = crate::module::effective_communicator_range(&world.agents.modules[i])
            .min(world.spatial.perception_max_radius());
        if range <= 0.0 {
            continue;
        }
        let pos = world.agents.position[i];
        let caller_species = world.agents.species_id[i];
        let mut converging: u32 = 0;
        world.spatial.query(pos, range, |oid| {
            if oid == id {
                return;
            }
            let j = oid as usize;
            if world.agents.species_id[j] != caller_species {
                return;
            }
            // A signal response is a receiver STEERING toward the caller:
            // aligned now AND materially better aligned than last tick (a
            // spawn-tick coincidence is not a response).
            let ws = world.world_size;
            let mut dx = pos.x - world.agents.position[j].x;
            let mut dy = pos.y - world.agents.position[j].y;
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
            let len = (dx * dx + dy * dy).sqrt().max(1e-6);
            let (ux, uy) = (dx / len, dy / len);
            let desired = world.desired_direction[j];
            let align_now = desired.x * ux + desired.y * uy;
            let prev =
                world.prev_desired_direction.get(j).copied().unwrap_or(crate::prelude::Vec2::ZERO);
            let align_prev = prev.x * ux + prev.y * uy;
            if align_now > 0.5 && align_now - align_prev >= 0.5 {
                converging += 1;
            }
        });
        if converging >= SIGNAL_CONVERGE_MIN {
            // Rate limit: one response per species per SIGNAL_WINDOW ticks.
            let last = world.codex.signal_last_response.get(&caller_species).copied().unwrap_or(0);
            if tick.saturating_sub(last) >= SIGNAL_WINDOW {
                world.codex.signal_last_response.insert(caller_species, tick);
                responses.push((caller_species, (pos.x, pos.y)));
            }
        }
    }
    world.agents.scratch_ids = alive_ids;
    // Snapshot desired directions for next tick's steering-change compare.
    if world.prev_desired_direction.len() < cap {
        world.prev_desired_direction.resize(cap, crate::prelude::Vec2::ZERO);
    }
    world.prev_desired_direction[..cap].copy_from_slice(&world.desired_direction[..cap]);

    let mut to_push: Vec<CodexEvent> = Vec::new();
    for (sid, (lx, ly)) in responses {
        let n = world.codex.signal_responses.entry(sid).or_insert(0);
        *n += 1;
        let count = *n;
        if count >= SIGNAL_MIN_RESPONSES && world.codex.signal_active.insert(sid) {
            to_push.push(CodexEvent {
                event_type: EventType::StructuredSignaling,
                tick,
                species_id: sid,
                value: count as f32,
                loc_x: lx,
                loc_y: ly,
            });
        }
    }
    for ev in to_push {
        world.codex.push_event(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::prelude::Vec2;

    fn world_with_agent() -> World {
        let mut w = World::new(1);
        let _ = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
        w
    }

    fn agg_for(sid: u32, count: u32) -> SpeciesAggTable {
        let mut agg = SpeciesAggTable::default();
        let e = SpeciesAgg {
            count,
            sum_x: 500.0 * count as f64,
            sum_y: 500.0 * count as f64,
            ..Default::default()
        };
        if sid as usize >= agg.entries.len() {
            agg.entries.resize(sid as usize + 1, SpeciesAgg::default());
        }
        agg.entries[sid as usize] = e;
        agg.active.push(sid);
        agg
    }

    fn stuff_hits(w: &mut World, sid: u32, n: u32, ambush: bool, tool: bool) {
        for _ in 0..n {
            w.codex.sig_hit_log.push_back(SigHit {
                tick: w.tick,
                species: sid,
                ambush,
                tool_boosted: tool,
            });
        }
    }

    #[test]
    fn ambush_share_fires_and_latches() {
        let mut w = world_with_agent();
        let agg = agg_for(0, 20);
        stuff_hits(&mut w, 0, 6, true, false);
        stuff_hits(&mut w, 0, 4, false, false); // 60% ambush share
        detect_ambush_and_tool(&mut w, &agg);
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::EvolvedAmbush));
        // Latched.
        detect_ambush_and_tool(&mut w, &agg);
        assert_eq!(
            w.codex.events.iter().filter(|e| e.event_type == EventType::EvolvedAmbush).count(),
            1
        );
    }

    #[test]
    fn mobile_shooter_is_not_an_ambusher() {
        let mut w = world_with_agent();
        let agg = agg_for(0, 20);
        stuff_hits(&mut w, 0, 12, false, false); // 0% ambush share
        detect_ambush_and_tool(&mut w, &agg);
        assert!(w.codex.events.is_empty());
    }

    #[test]
    fn metalworking_hunter_fires_tool_use() {
        let mut w = world_with_agent();
        w.inventions_enabled = true;
        let mut agg = agg_for(0, 20);
        // Full Metalworking adoption in the species entry.
        agg.entries[0].invention_counts[crate::invention::METALWORKING] = 20;
        // One boosted hit among many plain ones — adoption put to work.
        stuff_hits(&mut w, 0, 1, false, true);
        stuff_hits(&mut w, 0, 9, false, false);
        detect_ambush_and_tool(&mut w, &agg);
        assert!(w.codex.events.iter().any(|e| e.event_type == EventType::EvolvedTool));
    }

    #[test]
    fn unboosted_hits_are_not_tool_use() {
        let mut w = world_with_agent();
        w.inventions_enabled = true;
        let mut agg = agg_for(0, 20);
        agg.entries[0].invention_counts[crate::invention::METALWORKING] = 20;
        stuff_hits(&mut w, 0, 12, false, false);
        detect_ambush_and_tool(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "adoption without boosted hits is not tool use");
    }

    #[test]
    fn boosted_hits_without_adoption_are_not_tool_use() {
        let mut w = world_with_agent();
        w.inventions_enabled = true;
        let agg = agg_for(0, 20); // no adoption
        stuff_hits(&mut w, 0, 12, false, true);
        detect_ambush_and_tool(&mut w, &agg);
        assert!(w.codex.events.is_empty(), "one boosted outlier is not a tool-using species");
    }

    #[test]
    fn still_ticks_accumulate_and_reset() {
        let mut w = world_with_agent();
        let id = w.agents.iter_alive().next().unwrap();
        let i = id as usize;
        w.still_ticks.resize(w.agents.capacity(), 0);
        // Agent at rest (zero velocity).
        update_still_ticks(&mut w);
        update_still_ticks(&mut w);
        assert_eq!(w.still_ticks[i], 2);
        // Agent moving fast resets.
        w.agents.velocity[i] = Vec2::new(4.0, 0.0);
        update_still_ticks(&mut w);
        assert_eq!(w.still_ticks[i], 0);
    }
}
