//! Collision audit: run a scenario and, after every tick, check every
//! colliding pair (see `Locomotion::collides_with`) of alive agents against
//! the gap the collision layer enforces (the sum of their live body radii).
//! Reports, over the whole run:
//!
//! * end-of-tick pair distances below the gap, bucketed by `d / gap`
//!   (`< 0.5` deep, `0.5–0.7`, `0.7–0.9`, `0.9–1.0`), with how many of the
//!   offending pairs sit on a shoreline (either body within one biome cell
//!   of terrain its class cannot occupy) or involve an agent born this tick;
//! * swap-throughs: pairs whose straight-line relative motion through the
//!   tick passed closer than half their gap while both endpoints were not
//!   that close (one body passing through the other between two frames);
//! * airborne agents standing on a ground body (`Air` over `Land`/`Water`,
//!   closer than their radii sum) — not a collision the engine forbids, but
//!   what a viewer drawing both on the ground shows as one;
//! * the worst pairs seen, for reproduction.
//!
//! Reads the world only; it never changes the trajectory.

use std::path::PathBuf;

use anabios_core::collision::live_body_radius;
use anabios_core::habitat::Locomotion;
use anabios_core::scenario::Scenario;
use anabios_core::spatial::{torus_delta, UniformSpatialHash};
use anabios_core::tick::step;
use anabios_core::world::World;
use anyhow::{Context, Result};

use glam::Vec2;

/// Hash cell for the audit's own hash: above the largest gap plus the
/// longest plausible per-tick move on either side.
const AUDIT_CELL: f32 = 8.0;

#[derive(Default, Clone, Copy, Debug)]
struct Buckets {
    deep: u64,
    b50_70: u64,
    b70_90: u64,
    b90_95: u64,
    b95_100: u64,
}

impl Buckets {
    fn add(&mut self, ratio: f32) {
        if ratio < 0.5 {
            self.deep += 1;
        } else if ratio < 0.7 {
            self.b50_70 += 1;
        } else if ratio < 0.9 {
            self.b70_90 += 1;
        } else if ratio < 0.95 {
            self.b90_95 += 1;
        } else {
            self.b95_100 += 1;
        }
    }
    fn total(&self) -> u64 {
        self.deep + self.b50_70 + self.b70_90 + self.b90_95 + self.b95_100
    }
    fn merge(&mut self, o: &Buckets) {
        self.deep += o.deep;
        self.b50_70 += o.b50_70;
        self.b70_90 += o.b70_90;
        self.b90_95 += o.b90_95;
        self.b95_100 += o.b95_100;
    }
    fn line(&self) -> String {
        format!(
            "deep(<0.5)={} 0.5-0.7={} 0.7-0.9={} 0.9-0.95={} 0.95-1.0={}",
            self.deep, self.b50_70, self.b70_90, self.b90_95, self.b95_100
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct Worst {
    tick: u64,
    a: u32,
    b: u32,
    ratio: f32,
    pos: Vec2,
    shore: bool,
    newborn: bool,
    classes: (Locomotion, Locomotion),
}

/// Whether `pos`'s cell or any of its eight neighbours is terrain `class`
/// cannot occupy — the agent stands on a shoreline (or a habitat edge).
fn on_shore(world: &World, pos: Vec2, class: Locomotion) -> bool {
    if class == Locomotion::Air {
        return false;
    }
    let biome = &world.biome;
    let (cx, cy) = biome.cell_coords(pos);
    let res = biome.res as i32;
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            let col = (cx as i32 + dx).rem_euclid(res) as usize;
            let row = (cy as i32 + dy).rem_euclid(res) as usize;
            if !class.can_occupy(biome.at(col, row).terrain) {
                return true;
            }
        }
    }
    false
}

/// Closest approach of `d0 + (d1 - d0) t` for `t ∈ [0, 1]`.
fn closest_approach(d0: Vec2, d1: Vec2) -> f32 {
    let dv = d1 - d0;
    let a = dv.dot(dv);
    if a <= 1e-12 {
        return d0.length();
    }
    let t = (-d0.dot(dv) / a).clamp(0.0, 1.0);
    (d0 + dv * t).length()
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    scenario_path: PathBuf,
    ticks: u64,
    seed: Option<u64>,
    report_every: u64,
    worst_n: usize,
    extra_resolve: bool,
) -> Result<()> {
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario file {}", scenario_path.display()))?;
    let mut scenario = Scenario::parse_toml(&text)?;
    if let Some(s) = seed {
        scenario.seed = s;
    }
    let mut world = scenario.instantiate();
    let ws = world.world_size;
    let res = ((ws / AUDIT_CELL) as usize).clamp(3, 2048);
    let mut hash = UniformSpatialHash::with_dims(ws, res);
    let cell = ws / res as f32;
    println!(
        "audit scenario={} seed={} ticks={} initial_agents={} territory_enabled={} growth_enabled={}",
        scenario.name,
        world.seed,
        ticks,
        world.agents.live_count(),
        world.territory_enabled,
        world.growth_enabled
    );

    let mut prev_pos: Vec<Option<Vec2>> = Vec::new();
    let mut end = Buckets::default();
    let mut end_shore = Buckets::default();
    let mut end_newborn = Buckets::default();
    let mut end_cross_class = Buckets::default(); // Land–Water pairs
    let mut swaps = 0u64; // chord closest approach under half the gap
    let mut swaps_deep = 0u64; // ... under a quarter: a pass straight through
    let mut steps: Vec<f32> = Vec::new(); // this tick's move lengths (movers only)
    let mut step_sum = 0f64;
    let mut step_n = 0u64;
    let mut step_hist = [0u64; 6]; // <0.05, <0.5, <1, <2, <4, >=4
                                   // The stage-4' resolve's own displacement: end position minus the swept
                                   // end (`prev + velocity`, the move the sweep left). Bucketed by length.
    let mut resolve_hist = [0u64; 5]; // 0, <0.1, <0.5, <1, >=1
    let mut resolve_max = 0f32;
    // Swap-throughs attributed: the swept chord alone tunnels / only the
    // full chord (sweep end → resolve end) does.
    let mut swaps_sweep = 0u64;
    // Pair distances right after the swept move, before the resolve
    // (`prev + velocity`): what the sweep alone leaves.
    let mut post_sweep = Buckets::default();
    let mut air_over_ground = 0u64;
    let mut pair_checks = 0u64;
    let mut agent_ticks = 0u64;
    let mut ticks_with_visible = 0u64; // any pair under 0.9
    let mut ticks_with_deep = 0u64;
    let mut worst: Vec<Worst> = Vec::new();
    let mut window = Buckets::default();
    let mut window_swaps = 0u64;

    let mut extra_before = 0u64;
    let mut extra_after = 0u64;
    let mut extra_after_pinned = 0u64;
    // Tick 0 (founders as instantiated) is audited too.
    for t in 0..=ticks {
        if t > 0 {
            step(&mut world);
            if extra_resolve {
                // Probe: does one more resolve (another 32 passes) clear the
                // pairs under 0.9 of their gap? Those it clears were a pass
                // budget problem; those it leaves are pinned (terrain or a
                // crowd whose pushes cancel).
                let count = |world: &World| -> (u64, u64) {
                    let agents = &world.agents;
                    let growth = world.growth_enabled;
                    let mut h = UniformSpatialHash::with_dims(ws, res);
                    h.rebuild_sparse(&agents.position, |i| agents.is_alive(i as u32));
                    let (mut n, mut shore_n) = (0u64, 0u64);
                    for id in agents.iter_alive() {
                        let i = id as usize;
                        let pi = agents.position[i];
                        let ci = Locomotion::of(&agents.genome[i]);
                        let ri = live_body_radius(&agents.genome[i], agents.age[i], growth);
                        h.query_bbox(pi, 1.75, |oid| {
                            if oid <= id {
                                return;
                            }
                            let j = oid as usize;
                            let cj = Locomotion::of(&agents.genome[j]);
                            if !ci.collides_with(cj) {
                                return;
                            }
                            let gap =
                                ri + live_body_radius(&agents.genome[j], agents.age[j], growth);
                            let d = torus_delta(pi, agents.position[j], ws).length();
                            if d < 0.9 * gap {
                                n += 1;
                                if on_shore(world, pi, ci)
                                    || on_shore(world, agents.position[j], cj)
                                {
                                    shore_n += 1;
                                }
                            }
                        });
                    }
                    (n, shore_n)
                };
                let (b, _) = count(&world);
                if b > 0 {
                    anabios_core::collision::resolve_overlaps(&mut world);
                    let (a, sh) = count(&world);
                    extra_before += b;
                    extra_after += a;
                    extra_after_pinned += sh;
                }
            }
        }
        let agents = &world.agents;
        let cap = agents.capacity();
        prev_pos.resize(cap, None);
        hash.rebuild_sparse(&agents.position, |i| agents.is_alive(i as u32));
        let growth = world.growth_enabled;
        let mut tick_b = Buckets::default();
        let mut tick_swaps = 0u64;
        let mut tick_swaps_deep = 0u64;
        steps.clear();
        for id in agents.iter_alive() {
            agent_ticks += 1;
            let i = id as usize;
            let pi = agents.position[i];
            let ci = Locomotion::of(&agents.genome[i]);
            let ri = live_body_radius(&agents.genome[i], agents.age[i], growth);
            let vi = prev_pos[i].map(|p| torus_delta(pi, p, ws));
            if let Some(v) = vi {
                let swept_end = prev_pos[i].unwrap() + agents.velocity[i];
                let r = torus_delta(pi, swept_end, ws).length();
                resolve_max = resolve_max.max(r);
                let k = if r <= 1e-6 {
                    0
                } else if r < 0.1 {
                    1
                } else if r < 0.5 {
                    2
                } else if r < 1.0 {
                    3
                } else {
                    4
                };
                resolve_hist[k] += 1;
                let l = v.length();
                steps.push(l);
                step_sum += l as f64;
                step_n += 1;
                let k = if l < 0.05 {
                    0
                } else if l < 0.5 {
                    1
                } else if l < 1.0 {
                    2
                } else if l < 2.0 {
                    3
                } else if l < 4.0 {
                    4
                } else {
                    5
                };
                step_hist[k] += 1;
            }
            // Reach: the gap plus both moves (a swap-through needs the pair
            // to have been within reach at either end of the tick).
            let reach = (1.5f32 + 2.0 * vi.map_or(0.0, |v: Vec2| v.length()) + 4.0).min(cell * 3.0);
            hash.query_wide(pi, reach, |oid| {
                if oid <= id || !agents.is_alive(oid) {
                    return;
                }
                let j = oid as usize;
                let cj = Locomotion::of(&agents.genome[j]);
                let pj = agents.position[j];
                let d1 = torus_delta(pi, pj, ws);
                let gap = ri + live_body_radius(&agents.genome[j], agents.age[j], growth);
                if !ci.collides_with(cj) {
                    // Air over a ground body (or the reverse): drawn stacked
                    // by a viewer that puts both on the ground.
                    if d1.length() < gap {
                        air_over_ground += 1;
                    }
                    return;
                }
                pair_checks += 1;
                let dist = d1.length();
                let ratio = dist / gap;
                let born = prev_pos[i].is_none() || prev_pos[j].is_none();
                if dist < gap {
                    tick_b.add(ratio);
                    let shore = on_shore(&world, pi, ci) || on_shore(&world, pj, cj);
                    if shore {
                        end_shore.add(ratio);
                    }
                    if born {
                        end_newborn.add(ratio);
                    }
                    if ci != cj {
                        end_cross_class.add(ratio);
                    }
                    if ratio < 0.9 {
                        let w = Worst {
                            tick: t,
                            a: id,
                            b: oid,
                            ratio,
                            pos: pi,
                            shore,
                            newborn: born,
                            classes: (ci, cj),
                        };
                        if worst.len() < worst_n {
                            worst.push(w);
                        } else if let Some((k, _)) = worst
                            .iter()
                            .enumerate()
                            .max_by(|a, b| a.1.ratio.total_cmp(&b.1.ratio))
                        {
                            if ratio < worst[k].ratio {
                                worst[k] = w;
                            }
                        }
                    }
                }
                // Swap-through: both alive last tick, the straight relative
                // path passed closer than half the gap, neither endpoint did.
                if let (Some(pi0), Some(pj0)) = (prev_pos[i], prev_pos[j]) {
                    let d0 = torus_delta(pi0, pj0, ws);
                    let ds_len = torus_delta(pi0 + agents.velocity[i], pj0 + agents.velocity[j], ws).length();
                    if ds_len < gap {
                        post_sweep.add(ds_len / gap);
                    }
                    let ca = closest_approach(d0, d1);
                    if ca < 0.5 * gap && d0.length() >= 0.5 * gap && dist >= 0.5 * gap {
                        tick_swaps += 1;
                        if ca < 0.25 * gap {
                            tick_swaps_deep += 1;
                        }
                        // Was it the swept move itself (before the resolve)?
                        let si = pi0 + agents.velocity[i];
                        let sj = pj0 + agents.velocity[j];
                        let ds = torus_delta(si, sj, ws);
                        if closest_approach(d0, ds) < 0.5 * gap {
                            swaps_sweep += 1;
                        }
                    }
                }
            });
        }
        end.merge(&tick_b);
        swaps += tick_swaps;
        swaps_deep += tick_swaps_deep;
        window.merge(&tick_b);
        window_swaps += tick_swaps;
        if tick_b.deep + tick_b.b50_70 + tick_b.b70_90 > 0 {
            ticks_with_visible += 1;
        }
        if tick_b.deep > 0 {
            ticks_with_deep += 1;
        }
        if report_every > 0 && t > 0 && t % report_every == 0 {
            steps.sort_by(f32::total_cmp);
            let median = steps.get(steps.len() / 2).copied().unwrap_or(0.0);
            println!(
                "  t={t} alive={} window: {} swaps={} median_step={median:.2}",
                agents.live_count(),
                window.line(),
                window_swaps
            );
            window = Buckets::default();
            window_swaps = 0;
        }
        // Snapshot this tick's positions for the next tick's swap check.
        for slot in prev_pos.iter_mut() {
            *slot = None;
        }
        for id in agents.iter_alive() {
            prev_pos[id as usize] = Some(agents.position[id as usize]);
        }
    }
    println!(
        "result scenario={} seed={} ticks={ticks} agent_ticks={agent_ticks} pair_checks={pair_checks}",
        scenario.name, world.seed
    );
    println!("  end-of-tick pairs under gap: total={} {}", end.total(), end.line());
    println!("    of which on a shoreline: {}", end_shore.line());
    println!("    of which with a newborn: {}", end_newborn.line());
    println!("    of which Land-Water pairs: {}", end_cross_class.line());
    println!(
        "  post-sweep, pre-resolve pairs under gap: total={} {}",
        post_sweep.total(),
        post_sweep.line()
    );
    println!(
        "  moves: mean_step={:.3} hist(<0.05,<0.5,<1,<2,<4,>=4)={:?} over {step_n} agent-ticks",
        if step_n > 0 { step_sum / step_n as f64 } else { 0.0 },
        step_hist
    );
    println!(
        "  ticks with any pair under 0.9 gap: {ticks_with_visible}/{}  ticks with a deep pair: {ticks_with_deep}",
        ticks + 1
    );
    println!(
        "  swap-throughs: relative chord within half a gap (endpoints not): {swaps}; within a quarter: {swaps_deep}; already on the swept chord (before the resolve): {swaps_sweep}"
    );
    if extra_resolve {
        println!(
            "  extra-resolve probe: pairs under 0.9 gap before={extra_before} after another resolve={extra_after} (of which on a shoreline: {extra_after_pinned})"
        );
    }
    println!(
        "  resolve displacement per agent-tick: hist(0,<0.1,<0.5,<1,>=1)={:?} max={resolve_max:.2}",
        resolve_hist
    );
    println!("  air-over-ground stacks (Air within radii sum of a ground body): {air_over_ground}");
    worst.sort_by(|a, b| a.ratio.total_cmp(&b.ratio));
    for w in &worst {
        println!(
            "  worst: t={} ids=({},{}) d/gap={:.3} at ({:.1},{:.1}) classes={:?} shore={} newborn={}",
            w.tick, w.a, w.b, w.ratio, w.pos.x, w.pos.y, w.classes, w.shore, w.newborn
        );
    }
    Ok(())
}
