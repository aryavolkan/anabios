//! Body collision (territory/habitat/collision layer). Each agent is a disc of
//! `body_radius` (from its Size gene; under `World::growth_enabled` scaled by
//! its growth body scale, so a juvenile is a smaller disc — see
//! `live_body_radius`); colliding pairs (see `Locomotion::collides_with`) are
//! kept apart, not guaranteed to never overlap, by:
//!
//! 1. a separation steering term added in `decide_all` (agents route around
//!    each other), and
//! 2. a post-integrate resolve (stage 4'): up to `RESOLVE_PASSES` Jacobi
//!    passes over a fine hash rebuilt before each one, each agent pushing
//!    itself out of its overlaps against a position snapshot and sliding
//!    along the coast when a push would leave its terrain, until a pass
//!    moves nobody more than `RESOLVE_SETTLE`. Measured at 2000 ticks on the
//!    full-stack worlds: zero pairs closer than half their gap and 0–3% of
//!    agents closer than 90% of it (a tenth of a unit); the rest of the
//!    "touching" pairs sit within that last 10% (see
//!    `territory_measurement_probe`'s `deep_overlaps`/`shallow_overlaps`).
//!
//! Both read snapshots and write only their own slot, so the result is
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
pub const RESOLVE_PASSES: usize = 8;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::{Genome, GenomeSlot};
    use crate::world::World;

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
        // The sim resolves every tick; a few consecutive resolves finish the job.
        resolve_overlaps(&mut w);
        resolve_overlaps(&mut w);
        let (visible3, deep3) = count(&w);
        assert_eq!(deep3, 0);
        assert!(visible3 < visible1, "later resolves must keep spreading the pile");
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
