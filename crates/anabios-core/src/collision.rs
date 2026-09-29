//! Body collision (territory/habitat/collision layer). Each agent is a disc of
//! `body_radius` (from its Size gene; under `World::growth_enabled` scaled by
//! its growth body scale, so a juvenile is a smaller disc — see
//! `live_body_radius`); colliding pairs (see `Locomotion::collides_with`)
//! never pass through one another and end every tick apart, by:
//!
//! 1. a separation steering term added in `decide_all` (agents route around
//!    each other);
//! 2. a swept move (stage 4'', `sweep_moves`): every move is cut back to its
//!    first contact with another colliding body, relative motion through the
//!    tick, and continues by sliding along that body's surface — so two
//!    bodies never swap through each other between two rendered ticks and an
//!    agent walking into a crowd stops at its edge; and
//! 3. a post-move resolve (stage 4', `resolve_overlaps`): up to
//!    `RESOLVE_PASSES` Jacobi passes over a fine hash rebuilt before each
//!    one, each agent pushing itself out of its overlaps against a position
//!    snapshot and sliding along the coast when a push would leave its
//!    terrain, until a pass moves nobody more than `RESOLVE_SETTLE`.
//!
//! Births happen after all of that (stage 6), so a newborn is placed clear of
//! both parents' bodies (`offspring_position`) and, if that spot holds a
//! third body, at the nearest free spot (`settle_newborns`) instead of on its
//! parents' midpoint. The per-tick audit behind these (every curated world,
//! 2000 ticks, every colliding pair checked) is recorded in
//! `docs/scenarios.md`; before the swept move and the newborn placement it
//! found a stacked pair at every birth and tens of thousands of swap-throughs
//! per world, and crowds at trade hubs and watering holes the resolve could
//! not unpack (their pulls now have arrival zones: `hub::hub_pull`, the
//! water pull's `drinkable_near` gate).
//!
//! Every stage reads snapshots and writes only its own slot, so the result is
//! independent of rayon thread count. Gated on `World::territory_enabled`.

use crate::agent::AgentBuffers;
use crate::genome::{Genome, GenomeSlot};
use crate::habitat::Locomotion;
use crate::prelude::{wrap_torus, Vec2};
use crate::spatial::{torus_delta, UniformSpatialHash};
use crate::world::World;

/// Collision hash cell size (world units); the one-ring query guarantee
/// covers any reach up to this (the sweeps query smaller boxes, see
/// `STEER_QUERY_R` / `RESOLVE_QUERY_R`).
pub const COLLISION_CELL: f32 = 4.0;
/// Body radius at Size 0; `+ BODY_R_SIZE · Size` on top (`Size ∈ [0,1]`).
/// Invariant: `2 · (BODY_R_BASE + BODY_R_SIZE)` (the largest pair gap, 1.5)
/// stays below `reproduce::MATING_RANGE` and `interact::SHARE_RANGE` (2.0),
/// or the resolve would push mates and sharers out of contact; pinned by
/// `body_radius_spans_base_to_base_plus_size_term`.
pub const BODY_R_BASE: f32 = 0.4;
pub const BODY_R_SIZE: f32 = 0.35;
/// Steering starts at `STEER_MARGIN × (r_i + r_j)` — just before contact.
pub const STEER_MARGIN: f32 = 1.25;
/// Weight of the steering vector in `decide_all`.
pub const SEP_PULL: f32 = 2.0;
/// Upper bound on Jacobi passes per tick. Measured on the full-stack worlds
/// at 2000 ticks (minimal / predator-prey / habitat-territories): two passes
/// left 4–6% of agents visibly overlapping (closer than 90% of their gap)
/// and 1–13 pairs closer than half of it; eight passes with a rebuild and the
/// coastline slide below leave 0–3% visible and zero deep overlaps, for
/// ~20% more tick cost before the settle exit.
/// Raised from eight to thirty-two on 2026-09-28: a calm tick still exits
/// after one pass (the settle test below), only a real pile-up runs on, and
/// the pile-ups that remain once the hub and water pulls stop pressing agents
/// into a crowd they are already part of (`hub::hub_pull`, the water pull's
/// `drinkable_near` gate) are the ones that need the extra passes.
pub const RESOLVE_PASSES: usize = 32;
/// Rebuild the fine hash before every pass after the first, so each pass
/// queries fresh buckets (and the tight query radius) instead of the stale
/// ones the first build left.
pub const RESOLVE_REBUILD_EACH_PASS: bool = true;
/// When a push would leave the class's terrain, keep the axis component that
/// stays on it (a coastline slide, as `habitat::gate_move` does) instead of
/// dropping the push whole. Dropping it pinned shoreline crowds: roughly
/// 40% of the residual overlaps sat on coasts.
pub const RESOLVE_COAST_SLIDE: bool = true;
/// Settle threshold: the loop stops after a pass whose largest applied push
/// is below this many world units (5% of the smallest pair gap), so a tick
/// with no crowding pays for one pass and only a real pile-up runs all
/// `RESOLVE_PASSES`. A max over per-agent push lengths is order-independent,
/// so the exit is identical for any rayon thread count.
pub const RESOLVE_SETTLE: f32 = 0.04;
/// Largest per-pass push (keeps a pass inside the one-ring hash guarantee).
pub const MAX_PUSH: f32 = 1.0;
/// Contact surface of the swept move (`sweep_moves`) for a pair already
/// within `STEER_MARGIN` of its gap when the tick starts, as a fraction of
/// that gap: the resolve leaves herd neighbours at the gap, and a hard
/// surface there stopped every converging pair instantly — half of every
/// herd frozen (median step zero). Half the gap still rules out passing
/// through and stacking; the resolve then takes the shallow overlap back
/// out within the same tick. Set above the audit's "deep" line (half the
/// gap): at exactly half, pairs a crowd pressed against the surface ended
/// the tick at 0.42–0.46 of their gap.
pub const SWEEP_DEEP_FRAC: f32 = 0.6;
/// Slides per swept move: on contact the inward part of the velocity is
/// dropped and the move continues along the body's surface, up to this many
/// times, so a glancing contact does not stop the walk (stopping outright
/// cut a herd's median step from 2.3 units to 0.4).
pub const SWEEP_SLIDES: usize = 2;
/// Newborn settle (`settle_newborns`) search: rings of `NEWBORN_RING_STEP`
/// world units around the birth spot, `NEWBORN_RING_SAMPLES` candidates per
/// ring, out to `NEWBORN_RINGS` rings (twelve units — past the edge of the
/// densest crowd measured, the ~50 bodies within six units of a market on
/// grand-theater, where no spot within six units was free). A push-based
/// settle was tried first and failed exactly where it mattered: a child
/// dropped inside a crowd is pushed from every side and the pushes cancel,
/// the same interior cancellation that bounds the Jacobi resolve. Searching
/// for the nearest free spot has no such failure mode.
pub const NEWBORN_RINGS: usize = 24;
pub const NEWBORN_RING_STEP: f32 = 0.5;
pub const NEWBORN_RING_SAMPLES: usize = 12;
/// Hash query radius of the newborn settle's free-spot test: above the
/// largest gap (1.5); the hash is fresh (settled bodies do not move here)
/// and this tick's newborns are checked from a list instead.
pub const NEWBORN_QUERY_R: f32 = 1.75;
/// Hash query radius of the separation steer (`UniformSpatialHash::query_bbox`):
/// above its largest accept reach, `STEER_MARGIN · 2 · (BODY_R_BASE +
/// BODY_R_SIZE)` = 1.875, by a margin that dwarfs f32 seam rounding.
pub const STEER_QUERY_R: f32 = 2.0;
/// Hash query radii of the resolve passes: above the largest gap (1.5) for
/// the first pass, whose snapshot IS the hash, and above gap + `MAX_PUSH`
/// (2.5) for later passes, whose neighbours moved ≤ `MAX_PUSH` since the
/// hash was built. Both stay under the 4-unit cell the bbox query needs.
pub const RESOLVE_QUERY_R: [f32; 2] = [1.75, 2.75];

/// Fixed unit directions for exactly coincident pairs (no RNG).
const TIE_DIRS: [(f32, f32); 8] = [
    (1.0, 0.0),
    (0.707_106_77, 0.707_106_77),
    (0.0, 1.0),
    (-0.707_106_77, 0.707_106_77),
    (-1.0, 0.0),
    (-0.707_106_77, -0.707_106_77),
    (0.0, -1.0),
    (0.707_106_77, -0.707_106_77),
];

/// Adult body radius: the flag-off value, and the value every grown agent has.
#[inline]
pub fn body_radius(g: &Genome) -> f32 {
    BODY_R_BASE + BODY_R_SIZE * g.get(GenomeSlot::Size)
}

/// Where a newborn lands (territory layer on): on the perpendicular bisector
/// of its parents, offset from their midpoint just far enough that its body
/// clears both of theirs. `gap_a` / `gap_b` are the child-parent minimum
/// gaps (child radius + parent radius); the offset uses the larger, so the
/// child is at least its gap from each parent. The midpoint itself (the
/// flag-off placement) is at most `MATING_RANGE / 2` from either parent and
/// overlapped both by more than half a body for the one tick before the
/// next resolve pushed them apart — every birth was a stacked pair on
/// screen. Coincident parents (a zero-length axis) take a fixed +x normal;
/// `flip` mirrors the offset so consecutive litters alternate sides. No RNG.
pub fn offspring_position(
    a: Vec2,
    b: Vec2,
    gap_a: f32,
    gap_b: f32,
    flip: bool,
    world_size: f32,
) -> Vec2 {
    let d = torus_delta(b, a, world_size);
    let len = d.length();
    let mid = a + d * 0.5;
    let gap = gap_a.max(gap_b);
    let half = len * 0.5;
    let h = (gap * gap - half * half).max(0.0).sqrt();
    let n = if len > 1e-4 { Vec2::new(-d.y, d.x) / len } else { Vec2::new(1.0, 0.0) };
    let off = n * if flip { -h } else { h };
    wrap_torus(mid + off, Vec2::splat(world_size))
}

/// Body radius at a growth body scale (`growth::body_scale_of`): the adult
/// radius times `scale`. A scale of exactly `1.0` — every adult, and every
/// agent with growth off — gives exactly `body_radius(g)`.
#[inline]
pub fn body_radius_at(g: &Genome, scale: f32) -> f32 {
    body_radius(g) * scale
}

/// The body radius an agent `age` ticks old presents this tick: grown when
/// `growth_enabled`, the adult `body_radius(g)` otherwise.
#[inline]
pub fn live_body_radius(g: &Genome, age: u32, growth_enabled: bool) -> f32 {
    body_radius_at(g, crate::growth::body_scale_of(growth_enabled, age, g))
}

/// Unit vector pushing `i` away from `j`, given `d = pos_i − pos_j` (torus)
/// and its length. Coincident pairs get opposite fixed directions keyed on the
/// unordered id pair.
#[inline]
fn away_dir(i: u32, j: u32, d: Vec2, dist: f32) -> Vec2 {
    if dist > 1e-4 {
        return d / dist;
    }
    let (lo, hi) = if i < j { (i, j) } else { (j, i) };
    let (x, y) = TIE_DIRS[(lo.wrapping_mul(31).wrapping_add(hi) % 8) as usize];
    let dir = Vec2::new(x, y);
    if i < j {
        -dir
    } else {
        dir
    }
}

/// Grid resolution for a world of extent `ws`: cells of side `COLLISION_CELL`,
/// at least 3 wide, capped at 1024. The floor of 3 is the one-ring query's
/// minimum grid; below `ws = 3 · COLLISION_CELL` (12 units) the cell shrinks
/// under `COLLISION_CELL` and `UniformSpatialHash::query`'s radius guard
/// trips in debug builds — such a world is smaller than a handful of bodies
/// and is not supported by this layer. The flagship (1024-wide world) resolves
/// to 256, well under the cap and unaffected by it. Above the cap,
/// `cell_size = ws / res` grows past `COLLISION_CELL` instead of the grid
/// (and its `res²` buckets) growing without bound, which keeps rebuild and
/// memory cost bounded on huge worlds. Correctness holds either way: a
/// bigger `cell_size` only ever exceeds `COLLISION_CELL`, never falls below
/// it, so `UniformSpatialHash::query`'s one-ring scan (valid up to
/// `perception_max_radius() == cell_size`) still covers the `COLLISION_CELL`
/// reach `separation_steer`/`resolve_overlaps` query with, and the
/// `MAX_PUSH`-sized-move stale-bucket bound (see `resolve_overlaps`'s doc)
/// only gets more slack, never less.
pub(crate) fn collision_res(ws: f32) -> usize {
    ((ws / COLLISION_CELL) as usize).clamp(3, 1024)
}

/// (Re)size and rebuild `world.collision_spatial` from current positions.
/// Re-sizes whenever either the resolution OR the world extent no longer
/// matches the target — a snapshot load leaves `collision_spatial` at its
/// serde `Default` (`UniformSpatialHash::new()`, a 1024-wide/64-res hash),
/// not the `World::new` 3x3 placeholder, so `res` alone can spuriously match
/// (e.g. any `world_size` in `[256, 260)` also resolves to `res == 64`) while
/// the extent is still wrong: positions would then wrap at the stale extent
/// and same-cell neighbours near the true seam could land in unrelated cells.
pub fn rebuild_hash(world: &mut World) {
    let res = collision_res(world.world_size);
    let stale = world.collision_spatial.res() != res
        || world.collision_spatial.world_size() != world.world_size;
    if stale {
        world.collision_spatial = UniformSpatialHash::with_dims(world.world_size, res);
    }
    let agents = &world.agents;
    // Sparse: the grid (65k cells on the flagship, 1M at the cap) is far
    // larger than the population, and a dense rebuild's per-cell sweep would
    // dominate the flag-on cost on big worlds. Per-cell contents are the same.
    world.collision_spatial.rebuild_sparse(&agents.position, |i| agents.is_alive(i as u32));
}

/// Separation steering for agent `i`: Σ over colliding neighbours within
/// `STEER_MARGIN·(r_i+r_j)` of `away · (reach − d)/reach`. Reads the collision
/// hash built at stage 1 this tick. Radii are the live (grown) ones when
/// `growth_enabled`, the adult ones otherwise.
pub fn separation_steer(
    spatial: &UniformSpatialHash,
    agents: &AgentBuffers,
    i: usize,
    ws: f32,
    growth_enabled: bool,
) -> Vec2 {
    let pos = agents.position[i];
    let ri = live_body_radius(&agents.genome[i], agents.age[i], growth_enabled);
    let ci = Locomotion::of(&agents.genome[i]);
    let mut acc = Vec2::ZERO;
    spatial.query_bbox(pos, STEER_QUERY_R, |oid| {
        let j = oid as usize;
        if j == i || !ci.collides_with(Locomotion::of(&agents.genome[j])) {
            return;
        }
        let rj = live_body_radius(&agents.genome[j], agents.age[j], growth_enabled);
        let reach = (ri + rj) * STEER_MARGIN;
        let d = torus_delta(pos, agents.position[j], ws);
        let dist = d.length();
        if dist >= reach {
            return;
        }
        acc += away_dir(i as u32, oid, d, dist) * ((reach - dist) / reach);
    });
    acc
}

/// Stage 4': push overlapping colliding pairs apart to their minimum gap.
/// Rebuilds the fine hash from post-integrate positions, then runs up to
/// `RESOLVE_PASSES` Jacobi passes (rebuilding the hash before each one when
/// `RESOLVE_REBUILD_EACH_PASS`): each alive agent sums half of each overlap
/// along `away_dir` (read from the pass's snapshot), caps the push at
/// `MAX_PUSH`, and applies it if the destination is valid for its class —
/// else, with `RESOLVE_COAST_SLIDE`, the axis component that is. The loop
/// stops after a pass whose largest push is below `RESOLVE_SETTLE`. Without
/// a rebuild, positions move ≤ `MAX_PUSH` between passes, so a neighbour
/// within the max gap (1.5) is still within one hash cell (4.0) of the stale
/// bucket. No-op with the flag off. Gaps use the live (grown) radii when
/// `growth_enabled`, so juveniles pack closer.
pub fn resolve_overlaps(world: &mut World) {
    use rayon::prelude::*;
    if !world.territory_enabled {
        return;
    }
    rebuild_hash(world);
    let ws = world.world_size;
    let growth_enabled = world.growth_enabled;
    let cap = world.agents.capacity();
    for pass in 0..RESOLVE_PASSES {
        if pass > 0 && RESOLVE_REBUILD_EACH_PASS {
            rebuild_hash(world);
        }
        // Fresh buckets take the tight radius; stale ones (no rebuild since
        // the previous pass moved agents by up to MAX_PUSH) the wide one.
        let fresh = pass == 0 || RESOLVE_REBUILD_EACH_PASS;
        let query_r = if fresh { RESOLVE_QUERY_R[0] } else { RESOLVE_QUERY_R[1] };
        let mut snap = std::mem::take(&mut world.collision_scratch);
        snap.clear();
        snap.extend_from_slice(&world.agents.position[..cap]);
        let spatial = &world.collision_spatial;
        let biome = &world.biome;
        let AgentBuffers { position, genome, age, alive, .. } = &mut world.agents;
        let (genome, age, alive) = (&*genome, &*age, &*alive);
        let snap_ref = &snap;
        // Max over every agent's applied push length — order-independent, so
        // the settle exit below is identical for any rayon thread count.
        let largest_push = position[..cap]
            .par_iter_mut()
            .enumerate()
            .map(|(i, pos)| {
                if !alive[i] {
                    return 0.0f32;
                }
                let p = snap_ref[i];
                let ci = Locomotion::of(&genome[i]);
                let ri = live_body_radius(&genome[i], age[i], growth_enabled);
                let mut push = Vec2::ZERO;
                spatial.query_bbox(p, query_r, |oid| {
                    let j = oid as usize;
                    if j == i || !ci.collides_with(Locomotion::of(&genome[j])) {
                        return;
                    }
                    let gap = ri + live_body_radius(&genome[j], age[j], growth_enabled);
                    let d = torus_delta(p, snap_ref[j], ws);
                    let dist = d.length();
                    if dist >= gap {
                        return;
                    }
                    push += away_dir(i as u32, oid, d, dist) * ((gap - dist) * 0.5);
                });
                if push == Vec2::ZERO {
                    return 0.0;
                }
                let len = push.length();
                if len > MAX_PUSH {
                    push *= MAX_PUSH / len;
                }
                let target = wrap_torus(p + push, Vec2::splat(ws));
                if ci.can_occupy(biome.sample(target).terrain) {
                    *pos = target;
                    return push.length();
                }
                if RESOLVE_COAST_SLIDE {
                    for comp in [Vec2::new(push.x, 0.0), Vec2::new(0.0, push.y)] {
                        if comp == Vec2::ZERO {
                            continue;
                        }
                        let t = wrap_torus(p + comp, Vec2::splat(ws));
                        if ci.can_occupy(biome.sample(t).terrain) {
                            *pos = t;
                            return comp.length();
                        }
                    }
                }
                // A push into terrain the class can't occupy is dropped.
                0.0
            })
            .reduce(|| 0.0f32, f32::max);
        world.collision_scratch = snap;
        if largest_push < RESOLVE_SETTLE {
            break;
        }
    }
}

/// Stage 4'' (between integrate and the resolve): shorten every move to its
/// first contact. `integrate_all` applies each agent's full gated move; this
/// pass recovers the start of every move (`position − velocity`), solves,
/// for each colliding pair whose paths could touch this tick, the earliest
/// time `t ∈ [0, 1]` at which the two bodies (moving linearly through the
/// tick, relative motion) come within their gap, and moves each agent only
/// as far as its earliest contact. Bodies therefore never pass through one
/// another between two rendered ticks (the swap-through of two agents
/// stepping two body lengths in opposite directions was the most common
/// "collision" in every world), and an agent walking into a crowd stops at
/// its edge instead of stacking on the bodies inside — the density of a
/// crowd is then bounded by the bodies themselves, not by how many passes
/// the resolve can afford. A pair already overlapping at the start of the
/// tick (post-resolve residue) may move apart but not further in. The unused
/// part of the move's energy cost is refunded. Reads the stage-1 perception
/// hash (built from this tick's start positions), the applied velocities and
/// a position snapshot; writes only the agent's own slot, from a per-agent
/// contact time computed first — order-independent, so identical for any
/// thread count. RNG-free. No-op with the flag off.
pub fn sweep_moves(world: &mut World) {
    use rayon::prelude::*;
    if !world.territory_enabled {
        return;
    }
    let ws = world.world_size;
    let cap = world.agents.capacity();
    let size_v = Vec2::splat(ws);
    let max_gap = 2.0 * (BODY_R_BASE + BODY_R_SIZE);
    let spatial = &world.spatial;
    let biome = &world.biome;
    let query_cap = spatial.perception_max_radius();
    let (gait_enabled, growth_enabled) = (world.gait_enabled, world.growth_enabled);
    let desired = &world.desired_direction;
    let AgentBuffers { position, velocity, genome, age, alive, .. } = &world.agents;
    // Longest move this tick bounds how far any neighbour's start can be from
    // a contact: reach = own remaining move + that + the largest gap.
    let vmax = velocity[..cap]
        .par_iter()
        .enumerate()
        .map(|(i, v)| if alive[i] { v.length() } else { 0.0 })
        .reduce(|| 0.0f32, f32::max);
    let mut ends = std::mem::take(&mut world.collision_scratch);
    ends.clear();
    ends.extend_from_slice(&position[..cap]);
    ends.par_iter_mut().enumerate().for_each(|(i, end)| {
        if !alive[i] {
            return;
        }
        let v_full = velocity[i];
        if v_full.length_squared() <= 0.0 {
            return;
        }
        let start = wrap_torus(position[i] - v_full, size_v);
        let ci = Locomotion::of(&genome[i]);
        let ri = live_body_radius(&genome[i], age[i], growth_enabled);
        // Earliest contact of this agent, at `p` with velocity `v` at elapsed
        // tick fraction `tau`, over the remaining fraction: `(t, normal)`,
        // the normal pointing from the neighbour into this agent at contact.
        let earliest = |p: Vec2, v: Vec2, tau: f32, remaining: f32| -> Option<(f32, Vec2)> {
            let reach = (v.length() * remaining + vmax + max_gap).min(query_cap);
            let mut best: Option<(f32, Vec2)> = None;
            spatial.query(wrap_torus(p, size_v), reach, |oid| {
                let j = oid as usize;
                if j == i || !alive[j] || !ci.collides_with(Locomotion::of(&genome[j])) {
                    return;
                }
                let gap = ri + live_body_radius(&genome[j], age[j], growth_enabled);
                let vj = velocity[j];
                let start_j = wrap_torus(position[j] - vj, size_v);
                // The neighbour's position now (it moves linearly through the tick).
                let d0 = torus_delta(p, start_j, ws) - vj * tau;
                let dv = v - vj;
                let d0_sq = d0.dot(d0);
                // A pair already in contact range (herd neighbours sit at the
                // gap after the resolve, inside the steer margin) keeps its
                // freedom to jostle: its surface is half the gap — deep enough
                // to rule out passing through or stacking. Only a pair already
                // deeper than that may not move further in. A pair still
                // outside the margin meets a hard surface at the gap.
                let near = STEER_MARGIN * gap;
                let contact = if d0_sq >= near * near { gap } else { SWEEP_DEEP_FRAC * gap };
                let c = d0_sq - contact * contact;
                let b = 2.0 * d0.dot(dv);
                let a = dv.dot(dv);
                let tc = if c <= 0.0 {
                    if b < 0.0 {
                        0.0
                    } else {
                        return;
                    }
                } else {
                    if a <= 1e-12 {
                        return;
                    }
                    let disc = b * b - 4.0 * a * c;
                    if disc < 0.0 {
                        return;
                    }
                    (-b - disc.sqrt()) / (2.0 * a)
                };
                if !(0.0..remaining).contains(&tc) {
                    return;
                }
                if best.is_none_or(|(bt, _)| tc < bt) {
                    let at = d0 + dv * tc;
                    best = Some((tc, at.normalize_or_zero()));
                }
            });
            best
        };
        // Walk the move: advance to each contact, then slide along that
        // body's surface for the rest of the tick (up to `SWEEP_SLIDES`
        // times), gating every slid segment through the habitat rule.
        let mut p = start;
        let mut v = v_full;
        let mut tau = 0.0f32;
        let mut remaining = 1.0f32;
        for slide in 0..=SWEEP_SLIDES {
            let Some((tc, n)) = earliest(p, v, tau, remaining) else {
                p += v * remaining;
                break;
            };
            p += v * tc;
            tau += tc;
            remaining -= tc;
            if slide == SWEEP_SLIDES || remaining <= 1e-4 || n == Vec2::ZERO {
                break;
            }
            let inward = v.dot(n);
            if inward >= 0.0 {
                continue; // glancing: already moving off the surface
            }
            let slid = v - n * inward;
            if slid.length_squared() <= 1e-8 {
                break;
            }
            // The rest of the tick along the surface, habitat-gated.
            let allowed =
                crate::habitat::gate_move(biome, ci, wrap_torus(p, size_v), slid * remaining);
            if allowed == Vec2::ZERO {
                break;
            }
            v = allowed / remaining;
        }
        let end_p = wrap_torus(p, size_v);
        // A move cut short at a contact ends part-way along a path the
        // habitat gate validated only at half-cell samples, so the cut point
        // can clip the corner of a cell the class cannot occupy. Never end
        // there: stay at the tick's start instead (the resolve still runs).
        *end = if ci.can_occupy(biome.sample(end_p).terrain) { end_p } else { start };
    });
    let AgentBuffers { position, velocity, energy, genome, age, alive, .. } = &mut world.agents;
    let ends_ref = &ends;
    let (genome, age, alive) = (&*genome, &*age, &*alive);
    position[..cap]
        .par_iter_mut()
        .zip(velocity[..cap].par_iter_mut())
        .zip(energy[..cap].par_iter_mut())
        .enumerate()
        .for_each(|(i, ((pos, vel), en))| {
            if !alive[i] || vel.length_squared() <= 0.0 {
                return;
            }
            let end = ends_ref[i];
            if end == *pos {
                return;
            }
            let v = *vel;
            let start = wrap_torus(*pos - v, size_v);
            let nv = torus_delta(end, start, ws);
            *pos = end;
            *vel = nv;
            // Refund the cut part of the move at the rate `integrate_all`
            // charged it: per unit distance, size × growth body scale × the
            // gait's sprint factor (both exactly 1.0 with their knobs off).
            let size = genome[i].get(GenomeSlot::Size).max(0.1);
            let growth = crate::growth::body_scale_of(growth_enabled, age[i], &genome[i]);
            // `desired_direction` is sized to the capacity by the tick; a bare
            // stage call before any tick reads a full-intent fraction.
            let gait = if gait_enabled {
                crate::gait::move_cost_factor(desired.get(i).map_or(1.0, |d| d.length()))
            } else {
                1.0
            };
            *en += crate::integrate::MOVE_ENERGY_COST
                * (v.length() - nv.length()).max(0.0)
                * size
                * growth
                * gait;
        });
    world.collision_scratch = ends;
}

/// Stage 6 tail: move this tick's newborns off any body they landed on.
/// `offspring_position` clears the parents, but a herd is dense and the
/// bisector spot may hold a third agent (or another litter's child). Births
/// come after the stage-4' resolve, so without this the child sat stacked on
/// that body for the whole rendered tick. Rebuilds the fine hash once (the
/// settled population; this tick's newborns are skipped in it and tracked
/// from a list at their settled spots instead), then, in birth order, moves
/// each newborn that overlaps a colliding body to the nearest free spot: the
/// birth spot itself, else ring by ring (`NEWBORN_RINGS` × `NEWBORN_RING_STEP`,
/// `NEWBORN_RING_SAMPLES` fixed angles each, first hit wins) — a spot on
/// terrain its class can occupy where no colliding body is within their
/// gap. A newborn with no free spot within twelve units keeps its birth spot.
/// Only newborns move — the settled population is untouched, so the adult
/// trajectory is exactly what the stage-4' resolve left. Serial, RNG-free:
/// identical for any thread count. No-op with the flag off or no births.
pub fn settle_newborns(world: &mut World, newborns: &[u32]) {
    if !world.territory_enabled || newborns.is_empty() {
        return;
    }
    rebuild_hash(world);
    let ws = world.world_size;
    let mut is_newborn = std::mem::take(&mut world.newborn_mark);
    is_newborn.clear();
    is_newborn.resize(world.agents.capacity(), false);
    for &id in newborns {
        is_newborn[id as usize] = true;
    }
    // (position, radius, is_air) of every newborn settled so far this tick.
    let mut placed: Vec<(Vec2, f32, bool)> = Vec::with_capacity(newborns.len());
    for &id in newborns {
        let i = id as usize;
        if !world.agents.is_alive(id) {
            continue; // culled at birth (practice fitness costs)
        }
        let ci = Locomotion::of(&world.agents.genome[i]);
        let ri =
            live_body_radius(&world.agents.genome[i], world.agents.age[i], world.growth_enabled);
        let air = ci == Locomotion::Air;
        let origin = world.agents.position[i];
        let free = |c: Vec2| -> bool {
            if !ci.can_occupy(world.biome.sample(c).terrain) {
                return false;
            }
            let agents = &world.agents;
            let mut clear = true;
            world.collision_spatial.query_bbox(c, NEWBORN_QUERY_R, |oid| {
                let j = oid as usize;
                if !clear || is_newborn[j] || !agents.is_alive(oid) {
                    return;
                }
                if !ci.collides_with(Locomotion::of(&agents.genome[j])) {
                    return;
                }
                let gap =
                    ri + live_body_radius(&agents.genome[j], agents.age[j], world.growth_enabled);
                if torus_delta(c, agents.position[j], ws).length_squared() < gap * gap {
                    clear = false;
                }
            });
            clear
                && placed.iter().all(|&(p, r, a)| {
                    a != air || torus_delta(c, p, ws).length_squared() >= (ri + r) * (ri + r)
                })
        };
        let mut spot = None;
        if free(origin) {
            spot = Some(origin);
        } else {
            'rings: for k in 1..=NEWBORN_RINGS {
                let radius = k as f32 * NEWBORN_RING_STEP;
                for m in 0..NEWBORN_RING_SAMPLES {
                    // Alternate rings start half a step round so candidates
                    // do not line up on the same spokes.
                    let a = (m as f32 + if k % 2 == 0 { 0.5 } else { 0.0 })
                        * (std::f32::consts::TAU / NEWBORN_RING_SAMPLES as f32);
                    let c = wrap_torus(
                        origin + Vec2::new(radius * a.cos(), radius * a.sin()),
                        Vec2::splat(ws),
                    );
                    if free(c) {
                        spot = Some(c);
                        break 'rings;
                    }
                }
            }
        }
        let p = spot.unwrap_or(origin);
        world.agents.position[i] = p;
        placed.push((p, ri, air));
    }
    world.newborn_mark = is_newborn;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::{Genome, GenomeSlot};
    use crate::spatial::torus_distance;

    /// Build both hashes from the agents' current (start-of-tick) positions,
    /// then apply `moves` as `integrate_all` would (position += v, velocity
    /// = v) and run the sweep — the test's stand-in for stage 1 + stage 4.
    fn move_and_sweep(w: &mut World, moves: &[(u32, Vec2)]) {
        w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
        rebuild_hash(w);
        let ws = w.world_size;
        for &(id, v) in moves {
            let i = id as usize;
            w.agents.velocity[i] = v;
            w.agents.position[i] = wrap_torus(w.agents.position[i] + v, Vec2::splat(ws));
        }
        sweep_moves(w);
    }

    /// Two bodies walking straight at each other stop touching, each on its
    /// own side — never through one another.
    #[test]
    fn sweep_stops_head_on_movers_at_contact() {
        let mut w = flat_world();
        let ws = w.world_size;
        let g = Genome::neutral();
        let gap = 2.0 * body_radius(&g);
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), g);
        let b = w.spawn_agent(Vec2::new(306.0, 300.0), g);
        move_and_sweep(&mut w, &[(a, Vec2::new(4.0, 0.0)), (b, Vec2::new(-4.0, 0.0))]);
        let (pa, pb) = (w.agents.position[a as usize], w.agents.position[b as usize]);
        let d = torus_distance(pa, pb, ws);
        assert!(
            d >= gap - 1e-3 && d <= gap + 1e-2,
            "stopped at contact: d={d} gap={gap} {pa:?} {pb:?}"
        );
        assert!(pa.x < pb.x, "swapped through each other: {pa:?} {pb:?}");
        assert!(pa.x > 300.0 && pb.x < 306.0, "both moved toward the contact");
        // The applied velocity is the shortened move; energy refunded for the rest.
        assert!(w.agents.velocity[a as usize].x < 4.0);
    }

    /// A mover stops at a standing body; a crossing pair stops before their
    /// paths intersect; a pair moving apart is untouched.
    #[test]
    fn sweep_stops_at_standing_bodies_and_crossings_but_not_when_parting() {
        let g = Genome::neutral();
        let gap = 2.0 * body_radius(&g);
        // Into a standing body.
        let mut w = flat_world();
        let ws = w.world_size;
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), g);
        let b = w.spawn_agent(Vec2::new(303.0, 300.0), g);
        move_and_sweep(&mut w, &[(a, Vec2::new(4.0, 0.0))]);
        let d = torus_distance(w.agents.position[a as usize], w.agents.position[b as usize], ws);
        assert!(d >= gap - 1e-3 && d <= gap + 1e-2, "stopped at the standing body: {d}");
        assert_eq!(w.agents.position[b as usize], Vec2::new(303.0, 300.0));
        // Crossing paths.
        let mut w = flat_world();
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), g);
        let b = w.spawn_agent(Vec2::new(301.5, 298.5), g);
        move_and_sweep(&mut w, &[(a, Vec2::new(3.0, 0.0)), (b, Vec2::new(0.0, 3.0))]);
        let d = torus_distance(w.agents.position[a as usize], w.agents.position[b as usize], ws);
        assert!(d >= gap - 1e-3, "crossing pair overlaps: {d}");
        // Parting from an overlapping start: allowed in full.
        let mut w = flat_world();
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), g);
        let b = w.spawn_agent(Vec2::new(300.5, 300.0), g);
        move_and_sweep(&mut w, &[(a, Vec2::new(-2.0, 0.0)), (b, Vec2::new(2.0, 0.0))]);
        assert_eq!(w.agents.position[a as usize], Vec2::new(298.0, 300.0));
        assert_eq!(w.agents.position[b as usize], Vec2::new(302.5, 300.0));
        // Flag off: the sweep is a no-op.
        let mut w = World::new(2);
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), g);
        let b = w.spawn_agent(Vec2::new(303.0, 300.0), g);
        move_and_sweep(&mut w, &[(a, Vec2::new(4.0, 0.0))]);
        assert_eq!(w.agents.position[a as usize], Vec2::new(304.0, 300.0));
        let _ = b;
    }

    /// A newborn dropped onto a settled body is pushed clear of every
    /// colliding neighbour it overlaps, and the neighbours do not move.
    #[test]
    fn settle_newborns_clears_a_child_dropped_on_a_third_body() {
        let mut w = flat_world();
        let ws = w.world_size;
        let g = Genome::neutral();
        let r = body_radius(&g);
        // Two settled adults just touching along x, and a "newborn" spawned
        // right on top of the first one (and overlapping the second).
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), g);
        let b = w.spawn_agent(Vec2::new(300.0 + 2.0 * r, 300.0), g);
        let c = w.spawn_agent(Vec2::new(300.1, 300.05), g);
        let (pa, pb) = (w.agents.position[a as usize], w.agents.position[b as usize]);
        settle_newborns(&mut w, &[c]);
        let pc = w.agents.position[c as usize];
        let gap = 2.0 * r;
        assert!(torus_distance(pc, pa, ws) >= gap - 1e-4, "still on a: {pc:?}");
        assert!(torus_distance(pc, pb, ws) >= gap - 1e-4, "still on b: {pc:?}");
        assert_eq!(w.agents.position[a as usize], pa, "settled bodies never move");
        assert_eq!(w.agents.position[b as usize], pb, "settled bodies never move");
        // Flag off: a no-op.
        let mut w2 = World::new(2);
        let c2 = w2.spawn_agent(Vec2::new(300.1, 300.05), g);
        w2.spawn_agent(Vec2::new(300.0, 300.0), g);
        settle_newborns(&mut w2, &[c2]);
        assert_eq!(w2.agents.position[c2 as usize], Vec2::new(300.1, 300.05));
    }

    /// Two litters landing on the same spot settle against each other too:
    /// the second newborn sees the first at its settled position.
    #[test]
    fn settle_newborns_separates_sibling_newborns() {
        let mut w = flat_world();
        let ws = w.world_size;
        let g = Genome::neutral();
        let r = body_radius(&g);
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), g);
        let c1 = w.spawn_agent(Vec2::new(300.3, 300.0), g);
        let c2 = w.spawn_agent(Vec2::new(300.3, 300.01), g);
        settle_newborns(&mut w, &[c1, c2]);
        let (pa, p1, p2) = (
            w.agents.position[a as usize],
            w.agents.position[c1 as usize],
            w.agents.position[c2 as usize],
        );
        for (x, y, what) in [(p1, pa, "c1/a"), (p2, pa, "c2/a"), (p1, p2, "c1/c2")] {
            assert!(torus_distance(x, y, ws) >= 2.0 * r - 1e-4, "{what} overlap: {x:?} {y:?}");
        }
    }
    use crate::world::World;

    /// A newborn lands at least its child–parent gap from BOTH parents, on
    /// the torus, whether the parents touch, coincide, sit apart, straddle
    /// the seam or lie on a diagonal; `flip` mirrors it across the midpoint.
    #[test]
    fn offspring_position_clears_both_parents() {
        let ws = 1024.0;
        let (ga, gb) = (1.15, 1.2);
        let pairs = [
            ((300.0, 300.0), (300.5, 300.0)),  // parents overlapping
            ((300.0, 300.0), (300.0, 300.0)),  // coincident
            ((300.0, 300.0), (301.15, 300.0)), // just touching
            ((300.0, 300.0), (302.0, 300.0)),  // at MATING_RANGE
            ((1023.8, 300.0), (0.3, 300.0)),   // across the seam
            ((300.0, 300.0), (300.7, 300.9)),  // diagonal axis
        ];
        for ((ax, ay), (bx, by)) in pairs {
            let a = Vec2::new(ax, ay);
            let b = Vec2::new(bx, by);
            let mut sides = Vec::new();
            for flip in [false, true] {
                let c = offspring_position(a, b, ga, gb, flip, ws);
                assert!(c.x >= 0.0 && c.x < ws && c.y >= 0.0 && c.y < ws, "wrapped: {c:?}");
                let da = torus_distance(c, a, ws);
                let db = torus_distance(c, b, ws);
                assert!(
                    da >= ga - 1e-4,
                    "child {c:?} inside parent a's gap: {da} < {ga} ({a:?} {b:?})"
                );
                assert!(
                    db >= gb - 1e-4,
                    "child {c:?} inside parent b's gap: {db} < {gb} ({a:?} {b:?})"
                );
                // Just clear, not flung: the farther parent is within one extra body.
                assert!(da.max(db) <= gb + 1.0, "child flung away: {da} {db}");
                sides.push(c);
            }
            // The two flips mirror across the parents' midpoint.
            let mid = wrap_torus(a + torus_delta(b, a, ws) * 0.5, Vec2::splat(ws));
            let m2 =
                wrap_torus(sides[0] + torus_delta(sides[1], sides[0], ws) * 0.5, Vec2::splat(ws));
            assert!(
                torus_distance(mid, m2, ws) < 1e-3,
                "flips are not mirrored: {sides:?} mid {mid:?}"
            );
        }
    }

    /// Parents already farther apart than the gap: the midpoint is clear and
    /// is kept (no offset for no reason).
    #[test]
    fn offspring_position_keeps_a_clear_midpoint() {
        let ws = 1024.0;
        let a = Vec2::new(300.0, 300.0);
        let b = Vec2::new(303.0, 300.0);
        let c = offspring_position(a, b, 1.15, 1.15, false, ws);
        assert!((c.x - 301.5).abs() < 1e-5 && (c.y - 300.0).abs() < 1e-5, "{c:?}");
    }

    fn flat_world() -> World {
        let mut w = World::new(2);
        w.territory_enabled = true;
        for c in w.biome.cells.iter_mut() {
            c.terrain = crate::biome::TerrainType::Grass;
        }
        w
    }

    fn with_class(loco: f32) -> Genome {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Locomotion, loco);
        g
    }

    #[test]
    fn collision_res_caps_at_1024_with_cell_size_still_at_least_collision_cell() {
        // Flagship-sized world: well under the cap, untouched by it.
        assert_eq!(collision_res(1024.0), 256);
        // A huge world: capped at 1024 rather than growing without bound.
        assert_eq!(collision_res(16384.0), 1024);
        let cell_size = 16384.0 / collision_res(16384.0) as f32;
        assert!(
            cell_size >= COLLISION_CELL,
            "cell_size {cell_size} must stay >= COLLISION_CELL so the one-ring \
             query still covers the reach separation_steer/resolve_overlaps query with"
        );
        // Tiny world: floors at 3 (a one-ring query needs at least a 3x3 grid).
        assert_eq!(collision_res(1.0), 3);
    }

    #[test]
    fn body_radius_spans_base_to_base_plus_size_term() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Size, 0.0);
        assert_eq!(body_radius(&g), BODY_R_BASE);
        g.set(GenomeSlot::Size, 1.0);
        assert!((body_radius(&g) - (BODY_R_BASE + BODY_R_SIZE)).abs() < 1e-6);
        // The whole premise of the resolve: the largest pair gap must stay
        // below the mating / food-sharing contact ranges, or the min-gap push
        // would hold mates and sharers out of contact every tick.
        let contact = crate::reproduce::MATING_RANGE.min(crate::interact::SHARE_RANGE);
        let max_gap = 2.0 * (BODY_R_BASE + BODY_R_SIZE);
        assert!(2.0 * body_radius(&g) <= max_gap + 1e-6);
        assert!(
            max_gap < contact,
            "max pair gap {max_gap} must stay under contact range {contact}"
        );
    }

    #[test]
    fn stacked_agents_end_at_least_the_gap_apart() {
        let mut w = flat_world();
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let b = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let gap =
            body_radius(&w.agents.genome[a as usize]) + body_radius(&w.agents.genome[b as usize]);
        resolve_overlaps(&mut w);
        let d = crate::spatial::torus_distance(
            w.agents.position[a as usize],
            w.agents.position[b as usize],
            w.world_size,
        );
        assert!(d >= gap - 1e-4, "d={d} gap={gap}");
    }

    #[test]
    fn air_and_land_pass_through_each_other() {
        let mut w = flat_world();
        let land = w.spawn_agent(Vec2::new(300.0, 300.0), with_class(0.5));
        let bird = w.spawn_agent(Vec2::new(300.2, 300.0), with_class(0.9));
        resolve_overlaps(&mut w);
        assert_eq!(w.agents.position[land as usize], Vec2::new(300.0, 300.0));
        assert_eq!(w.agents.position[bird as usize], Vec2::new(300.2, 300.0));
    }

    #[test]
    fn a_push_never_crosses_into_invalid_habitat() {
        let mut w = flat_world();
        // Coast at x = 304 (cell col 38 of 8-unit cells on the 1024 world): water east of it.
        let res = w.biome.res;
        for row in 0..res {
            for col in 38..res {
                w.biome.at_mut(col, row).terrain = crate::biome::TerrainType::Water;
            }
        }
        let a = w.spawn_agent(Vec2::new(303.8, 300.0), Genome::neutral()); // land, at the shore
        let _b = w.spawn_agent(Vec2::new(303.4, 300.0), Genome::neutral()); // pushes a east
        resolve_overlaps(&mut w);
        let t = w.biome.sample(w.agents.position[a as usize]).terrain;
        assert_ne!(t, crate::biome::TerrainType::Water, "land agent pushed into the sea");
    }

    #[test]
    fn a_dense_pile_separates_to_within_a_tenth_of_its_gaps() {
        // Forty agents dropped inside a 3-unit square: the pass cap plus the
        // per-pass rebuild must spread them so no pair is closer than half
        // its gap and at most a couple sit closer than 90% of it.
        let mut w = flat_world();
        let mut ids = Vec::new();
        for k in 0..40u32 {
            let x = 300.0 + (k % 8) as f32 * 0.4;
            let y = 300.0 + (k / 8) as f32 * 0.6;
            ids.push(w.spawn_agent(Vec2::new(x, y), Genome::neutral()));
        }
        let count = |w: &World| {
            let (mut visible, mut deep) = (0, 0);
            for (i, &a) in ids.iter().enumerate() {
                for &b in &ids[i + 1..] {
                    let gap = body_radius(&w.agents.genome[a as usize])
                        + body_radius(&w.agents.genome[b as usize]);
                    let d = crate::spatial::torus_distance(
                        w.agents.position[a as usize],
                        w.agents.position[b as usize],
                        w.world_size,
                    );
                    if d < 0.9 * gap {
                        visible += 1;
                    }
                    if d < 0.5 * gap {
                        deep += 1;
                    }
                }
            }
            (visible, deep)
        };
        // One tick's resolve clears every deep overlap of the pile outright.
        resolve_overlaps(&mut w);
        let (visible1, deep1) = count(&w);
        assert_eq!(deep1, 0, "deep overlaps left after one resolve");
        assert!(visible1 < 40 * 39 / 2 / 4, "one resolve barely spread the pile: {visible1} pairs");
        // The sim resolves every tick; consecutive resolves never undo the
        // spread (with the 32-pass cap one resolve usually finishes it, so a
        // strict "keeps spreading" can no longer be asked for).
        resolve_overlaps(&mut w);
        resolve_overlaps(&mut w);
        let (visible3, deep3) = count(&w);
        assert_eq!(deep3, 0);
        assert!(
            visible3 <= visible1,
            "later resolves must not re-crowd the pile: {visible1} -> {visible3}"
        );
        // A handful of pairs (measured: 4 of 780) settle just under 90% of
        // their gap in the crowd equilibrium — a tenth of a unit, invisible.
        assert!(
            visible3 <= 6,
            "{visible3} pairs still closer than 90% of their gap after 3 resolves"
        );
    }

    #[test]
    fn a_seaward_push_slides_along_the_coast_instead_of_being_dropped() {
        let mut w = flat_world();
        // Water east of x = 304 (cell col 38), as above.
        let res = w.biome.res;
        for row in 0..res {
            for col in 38..res {
                w.biome.at_mut(col, row).terrain = crate::biome::TerrainType::Water;
            }
        }
        // `a` sits on the shore; `b` overlaps it from just south of due west,
        // so the away push on `a` points east with a small northward part:
        // its x part would enter the sea, its y part stays on land.
        let a = w.spawn_agent(Vec2::new(303.9, 300.0), Genome::neutral());
        let _b = w.spawn_agent(Vec2::new(303.4, 299.9), Genome::neutral());
        let before = w.agents.position[a as usize];
        resolve_overlaps(&mut w);
        let after = w.agents.position[a as usize];
        assert_ne!(after, before, "the push must not be dropped wholesale");
        assert!(after.y > before.y, "the y component slides north: {before:?} -> {after:?}");
        assert!(after.x < 304.0, "never into the sea: {after:?}");
        assert_ne!(w.biome.sample(after).terrain, crate::biome::TerrainType::Water);
        // And the pair is apart: the slide plus the neighbour's own retreat
        // reach the gap within the pass cap.
        let gap = 2.0 * body_radius(&Genome::neutral());
        let d = crate::spatial::torus_distance(after, w.agents.position[_b as usize], w.world_size);
        assert!(d >= 0.9 * gap, "still overlapping at the shore: d={d} gap={gap}");
    }

    #[test]
    fn resolve_is_a_noop_with_the_flag_off() {
        let mut w = flat_world();
        w.territory_enabled = false;
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let _b = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        resolve_overlaps(&mut w);
        assert_eq!(w.agents.position[a as usize], Vec2::new(300.0, 300.0));
    }

    #[test]
    fn steering_points_away_from_an_overlapping_neighbour() {
        let mut w = flat_world();
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let _b = w.spawn_agent(Vec2::new(300.5, 300.0), Genome::neutral());
        rebuild_hash(&mut w);
        let s = separation_steer(
            &w.collision_spatial,
            &w.agents,
            a as usize,
            w.world_size,
            w.growth_enabled,
        );
        assert!(s.x < 0.0 && s.y.abs() < 1e-6, "steer west, away from b: {s:?}");
    }

    /// Growth: `body_radius_at` scales the adult radius; with the flag off
    /// (scale exactly 1.0) it IS `body_radius`, whatever the age. With the
    /// flag on a newborn is `JUVENILE_BODY` of the adult and exactly the
    /// adult from maturity on.
    #[test]
    fn body_radius_at_scales_the_adult_value_and_is_exact_at_one() {
        use crate::growth::{body_scale_of, maturity_ticks, JUVENILE_BODY};
        let g = Genome::neutral();
        let adult = body_radius(&g);
        let m = maturity_ticks(crate::age::lifespan_of(&g));
        for age in [0, 7, m, 5000] {
            assert_eq!(body_radius_at(&g, body_scale_of(false, age, &g)), adult, "age {age}");
            assert_eq!(live_body_radius(&g, age, false), adult, "age {age}");
        }
        let newborn = live_body_radius(&g, 0, true);
        let expected = JUVENILE_BODY * adult;
        assert!((newborn - expected).abs() < 1e-6, "newborn {newborn} vs {expected}");
        assert!(live_body_radius(&g, m / 2, true) > newborn);
        assert!(live_body_radius(&g, m - 1, true) < adult);
        assert_eq!(live_body_radius(&g, m, true), adult);
        assert_eq!(live_body_radius(&g, m + 1000, true), adult);
    }

    /// Two coincident newborns resolve to their (smaller) juvenile gap —
    /// closer than two adults would sit — and, once grown, the same pair is
    /// pushed out to the adult gap.
    #[test]
    fn newborns_resolve_to_their_smaller_gap_and_adults_to_the_full_one() {
        use crate::growth::maturity_ticks;
        let mut w = flat_world();
        w.growth_enabled = true;
        let a = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let b = w.spawn_agent(Vec2::new(300.0, 300.0), Genome::neutral());
        let (ga, gb) = (w.agents.genome[a as usize], w.agents.genome[b as usize]);
        let adult_gap = body_radius(&ga) + body_radius(&gb);
        let gap = live_body_radius(&ga, 0, true) + live_body_radius(&gb, 0, true);
        assert!(gap < adult_gap);
        let dist = |w: &World| {
            crate::spatial::torus_distance(
                w.agents.position[a as usize],
                w.agents.position[b as usize],
                w.world_size,
            )
        };
        resolve_overlaps(&mut w);
        let d = dist(&w);
        assert!(d >= gap - 1e-4, "newborns still overlapping: d={d} gap={gap}");
        assert!(d < adult_gap, "newborns pack closer than adults: d={d} adult gap={adult_gap}");
        let m = maturity_ticks(crate::age::lifespan_of(&ga));
        w.agents.age[a as usize] = m;
        w.agents.age[b as usize] = m;
        resolve_overlaps(&mut w);
        let d = dist(&w);
        assert!(d >= adult_gap - 1e-4, "grown pair not at the adult gap: d={d} gap={adult_gap}");
    }

    #[test]
    fn rebuild_hash_heals_a_stale_extent_from_a_loaded_snapshot() {
        // A small, non-default world extent. `collision_res(256.0) == 64`,
        // which also happens to equal `UniformSpatialHash::new()`'s HASH_RES
        // default — so a resolution-only staleness check would miss this.
        let mut w = World::with_dims(1, 256.0, 32, 16);
        w.territory_enabled = true;
        for c in w.biome.cells.iter_mut() {
            c.terrain = crate::biome::TerrainType::Grass;
        }
        // Simulate a post-snapshot-load world: `collision_spatial` is
        // `#[serde(skip)]`, so it comes back as serde's `Default`
        // (`UniformSpatialHash::new()`, a 1024-wide/64-res hash) — not the
        // `World::new` 3x3 placeholder the naive res-only check assumed.
        w.collision_spatial = crate::spatial::UniformSpatialHash::default();

        // Two agents straddling the true wrap seam at 256: 0.3 apart on the
        // torus, but ~255.7 apart if a stale 1024-wide hash bucketed them.
        let a = w.spawn_agent(Vec2::new(255.8, 128.0), Genome::neutral());
        let b = w.spawn_agent(Vec2::new(0.1, 128.0), Genome::neutral());
        let gap =
            body_radius(&w.agents.genome[a as usize]) + body_radius(&w.agents.genome[b as usize]);

        resolve_overlaps(&mut w);

        assert_eq!(
            w.collision_spatial.world_size(),
            256.0,
            "rebuild_hash must re-extent the hash to the live world_size, not just match res"
        );
        let d = crate::spatial::torus_distance(
            w.agents.position[a as usize],
            w.agents.position[b as usize],
            w.world_size,
        );
        assert!(d >= gap - 1e-4, "seam pair not separated: d={d} gap={gap}");
    }
}
