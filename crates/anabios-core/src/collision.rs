//! Body collision (territory/habitat/collision layer). Each agent is a disc of
//! `body_radius` (from its Size gene); colliding pairs (see
//! `Locomotion::collides_with`) are kept at least `r_i + r_j` apart by:
//!
//! 1. a separation steering term added in `decide_all` (agents route around
//!    each other), and
//! 2. a hard post-integrate resolve (stage 4'): `RESOLVE_PASSES` Jacobi
//!    passes over a fine hash, each agent pushing itself out of its overlaps
//!    against a position snapshot, dropping any push into terrain its class
//!    can't occupy.
//!
//! Both read snapshots and write only their own slot, so the result is
//! independent of rayon thread count. Gated on `World::territory_enabled`.

use crate::agent::AgentBuffers;
use crate::genome::{Genome, GenomeSlot};
use crate::habitat::Locomotion;
use crate::prelude::{wrap_torus, Vec2};
use crate::spatial::{torus_delta, UniformSpatialHash};
use crate::world::World;

/// Collision hash cell size (world units); also the query reach.
pub const COLLISION_CELL: f32 = 4.0;
/// Body radius at Size 0; `+ BODY_R_SIZE · Size` on top (`Size ∈ [0,1]`).
pub const BODY_R_BASE: f32 = 0.4;
pub const BODY_R_SIZE: f32 = 0.35;
/// Steering starts at `STEER_MARGIN × (r_i + r_j)` — just before contact.
pub const STEER_MARGIN: f32 = 1.25;
/// Weight of the steering vector in `decide_all`.
pub const SEP_PULL: f32 = 2.0;
/// Jacobi passes per tick.
pub const RESOLVE_PASSES: usize = 2;
/// Largest per-pass push (keeps a pass inside the one-ring hash guarantee).
pub const MAX_PUSH: f32 = 1.0;

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

#[inline]
pub fn body_radius(g: &Genome) -> f32 {
    BODY_R_BASE + BODY_R_SIZE * g.get(GenomeSlot::Size)
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

fn collision_res(ws: f32) -> usize {
    ((ws / COLLISION_CELL) as usize).max(3)
}

/// (Re)size and rebuild `world.collision_spatial` from current positions.
pub fn rebuild_hash(world: &mut World) {
    let res = collision_res(world.world_size);
    if world.collision_spatial.res() != res {
        world.collision_spatial = UniformSpatialHash::with_dims(world.world_size, res);
    }
    let agents = &world.agents;
    world.collision_spatial.rebuild(&agents.position, |i| agents.is_alive(i as u32));
}

/// Separation steering for agent `i`: Σ over colliding neighbours within
/// `STEER_MARGIN·(r_i+r_j)` of `away · (reach − d)/reach`. Reads the collision
/// hash built at stage 1 this tick.
pub fn separation_steer(
    spatial: &UniformSpatialHash,
    agents: &AgentBuffers,
    i: usize,
    ws: f32,
) -> Vec2 {
    let pos = agents.position[i];
    let ri = body_radius(&agents.genome[i]);
    let ci = Locomotion::of(&agents.genome[i]);
    let mut acc = Vec2::ZERO;
    spatial.query(pos, COLLISION_CELL, |oid| {
        let j = oid as usize;
        if j == i || !ci.collides_with(Locomotion::of(&agents.genome[j])) {
            return;
        }
        let reach = (ri + body_radius(&agents.genome[j])) * STEER_MARGIN;
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
/// Rebuilds the fine hash from post-integrate positions, then runs
/// `RESOLVE_PASSES` Jacobi passes: each alive agent sums half of each overlap
/// along `away_dir` (read from the pass's snapshot), caps the push at
/// `MAX_PUSH`, and applies it only if the destination is valid for its class.
/// Between passes positions move ≤ `MAX_PUSH`, so a neighbour within the
/// max gap (1.5) is still within one hash cell (4.0) of the stale bucket.
/// No-op with the flag off.
pub fn resolve_overlaps(world: &mut World) {
    use rayon::prelude::*;
    if !world.territory_enabled {
        return;
    }
    rebuild_hash(world);
    let ws = world.world_size;
    let cap = world.agents.capacity();
    for _ in 0..RESOLVE_PASSES {
        let mut snap = std::mem::take(&mut world.collision_scratch);
        snap.clear();
        snap.extend_from_slice(&world.agents.position[..cap]);
        let spatial = &world.collision_spatial;
        let biome = &world.biome;
        let AgentBuffers { position, genome, alive, .. } = &mut world.agents;
        let (genome, alive) = (&*genome, &*alive);
        let snap_ref = &snap;
        position[..cap].par_iter_mut().enumerate().for_each(|(i, pos)| {
            if !alive[i] {
                return;
            }
            let p = snap_ref[i];
            let ci = Locomotion::of(&genome[i]);
            let ri = body_radius(&genome[i]);
            let mut push = Vec2::ZERO;
            spatial.query(p, COLLISION_CELL, |oid| {
                let j = oid as usize;
                if j == i || !ci.collides_with(Locomotion::of(&genome[j])) {
                    return;
                }
                let gap = ri + body_radius(&genome[j]);
                let d = torus_delta(p, snap_ref[j], ws);
                let dist = d.length();
                if dist >= gap {
                    return;
                }
                push += away_dir(i as u32, oid, d, dist) * ((gap - dist) * 0.5);
            });
            if push == Vec2::ZERO {
                return;
            }
            let len = push.length();
            if len > MAX_PUSH {
                push *= MAX_PUSH / len;
            }
            let target = wrap_torus(p + push, Vec2::splat(ws));
            if ci.can_occupy(biome.sample(target).terrain) {
                *pos = target;
            }
        });
        world.collision_scratch = snap;
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
    fn body_radius_spans_base_to_base_plus_size_term() {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Size, 0.0);
        assert_eq!(body_radius(&g), BODY_R_BASE);
        g.set(GenomeSlot::Size, 1.0);
        assert!((body_radius(&g) - (BODY_R_BASE + BODY_R_SIZE)).abs() < 1e-6);
        assert!(2.0 * body_radius(&g) <= 1.5 + 1e-6, "max pair gap stays under contact range 2.0");
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
        let s = separation_steer(&w.collision_spatial, &w.agents, a as usize, w.world_size);
        assert!(s.x < 0.0 && s.y.abs() < 1e-6, "steer west, away from b: {s:?}");
    }
}
