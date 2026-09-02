//! Record a scenario deterministically into a compact replay stream for the
//! web showcase player (`showcase/`).
//!
//! The recorder runs the same `step` loop as every other subcommand, sampling
//! per-agent draw data every `sample` ticks and draining codex events every
//! tick. Reading the agent/biome columns is pure and RNG-free, so recording
//! does not perturb the run — the final `state_hash` matches a plain `run` of
//! the same scenario/seed/ticks. The per-agent, per-species, and biome draw
//! logic mirrors the Godot bridge (`crates/anabios-godot/src/lib.rs`) so the
//! web player shows the same world the live viewer does.
//!
//! Output is a single `.js` file assigning `window.ANABIOS_REPLAY = {…}` (so the
//! player loads it via `<script>` with no fetch/CORS), or raw `.json` when the
//! `--out` path ends in `.json`.

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::PathBuf;

use anabios_core::codex::CodexEvent;
use anabios_core::module::effective_diet_carnivory;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;
use anabios_core::World;
use anyhow::{Context, Result};
use serde::Serialize;

use crate::score::event_name;

/// One sampled frame: parallel arrays indexed by agent, all same length.
#[derive(Serialize)]
struct Frame {
    /// Tick at which this frame was sampled.
    t: u64,
    /// Agent slot ids (stable across frames while an agent lives).
    id: Vec<u32>,
    /// Position x in world units, rounded.
    x: Vec<i32>,
    /// Position y in world units, rounded.
    y: Vec<i32>,
    /// Species id (numeric cluster id).
    sp: Vec<u32>,
    /// Diet carnivory quantised to 0 (herbivore) ..= 255 (carnivore).
    d: Vec<u8>,
    /// Combat streaks this tick, flattened [x1,y1,x2,y2, ...] (attacker→target).
    /// Absent when no combat happened at the sampled tick.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    st: Vec<i32>,
    /// Trade routes this tick, flattened [x1,y1,x2,y2, ...] (trader→partner).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tr: Vec<i32>,
}

/// Settlement sites at one tick: parallel arrays per settled species — the
/// codex latch's species, their anchor centroids, and anchored member counts.
/// Captured on the same cadence as `frames` (aligned by index) so the web
/// player can draw the same hut villages the Godot viewer does.
#[derive(Serialize)]
struct SitesFrame {
    t: u64,
    sid: Vec<u32>,
    x: Vec<i32>,
    y: Vec<i32>,
    n: Vec<u32>,
}

/// One codex event, flattened for the player's ticker.
#[derive(Serialize)]
struct Event {
    /// Tick the event fired.
    t: u64,
    /// Stable snake_case event-type name (matches the sweep corpus).
    #[serde(rename = "type")]
    kind: &'static str,
    /// Species id, or `null` for global events (`u32::MAX` in core).
    sid: Option<u32>,
    /// Event payload value; meaning depends on the type.
    v: f32,
    /// World-space location (rounded), when the event carries one.
    x: i32,
    y: i32,
}

/// A downsampled biome colour map at one tick, RGB bytes base64-encoded.
#[derive(Serialize)]
struct BiomeGrid {
    t: u64,
    b64: String,
}

/// The biome layer: a sequence of colour grids over time (the map lushes,
/// pollutes, and scars as the run proceeds).
#[derive(Serialize)]
struct BiomeSeq {
    res: usize,
    grids: Vec<BiomeGrid>,
}

#[derive(Serialize)]
struct Meta {
    scenario: String,
    seed: u64,
    ticks: u64,
    sample: u64,
    world_size: f32,
    /// Agent subsampling stride; multiply shown agents by this for the true count.
    stride: usize,
    frame_count: usize,
    event_count: usize,
    state_hash: String,
}

#[derive(Serialize)]
struct Replay {
    meta: Meta,
    /// Known species id → archetype display name (splinters are absent).
    species: BTreeMap<u32, String>,
    biome: BiomeSeq,
    frames: Vec<Frame>,
    /// Settlement sites, index-aligned with `frames`.
    sites: Vec<SitesFrame>,
    events: Vec<Event>,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    scenario_path: PathBuf,
    ticks: u64,
    seed: Option<u64>,
    sample: u64,
    max_agents: usize,
    max_events: usize,
    biome_res: usize,
    biome_frames: usize,
    out: PathBuf,
) -> Result<()> {
    let sample = sample.max(1);
    let text = std::fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading scenario file {}", scenario_path.display()))?;
    let mut scenario = Scenario::parse_toml(&text)?;
    if let Some(s) = seed {
        scenario.seed = s;
    }

    // Species id → archetype label, assigned in declaration order exactly as
    // `Scenario::instantiate` reserves species ids (see demo.rs::CultureMap).
    let mut species: BTreeMap<u32, String> = BTreeMap::new();
    let mut next_sid = 1u32;
    for spec in &scenario.agents {
        if let Some(name) = &spec.archetype {
            species.insert(next_sid, name.clone());
            next_sid += 1;
        }
    }

    let mut world = scenario.instantiate();
    let world_size = world.world_size;

    // Keep every `stride`-th agent slot so the frame count stays near
    // `max_agents`. A fixed stride keeps agent identity stable across frames
    // (a slot is always in or always out), which lets the player interpolate.
    let initial_live = world.agents.live_count() as usize;
    let stride = if max_agents == 0 { 1 } else { initial_live.div_ceil(max_agents).max(1) };

    // Biome grids at an even cadence over the run.
    let biome_every =
        if biome_frames == 0 { u64::MAX } else { (ticks / biome_frames as u64).max(1) };

    println!(
        "recording scenario={} seed={} ticks={} sample={} agents≈{}/{} (stride {}) biome={}²×{}",
        scenario.name,
        world.seed,
        ticks,
        sample,
        initial_live / stride,
        initial_live,
        stride,
        biome_res,
        biome_frames,
    );

    let mut frames: Vec<Frame> = Vec::with_capacity((ticks / sample) as usize + 1);
    let mut sites: Vec<SitesFrame> = Vec::with_capacity((ticks / sample) as usize + 1);
    let mut events: Vec<Event> = Vec::new();
    let mut biome_grids: Vec<BiomeGrid> = Vec::new();

    // Frame 0 + biome 0 (initial state), then on cadence.
    capture_frame(&world, stride, &mut frames);
    capture_sites(&world, &mut sites);
    if biome_frames > 0 {
        biome_grids.push(capture_biome(&world, biome_res));
    }
    for _ in 0..ticks {
        step(&mut world);
        for ev in world.codex.drain_events() {
            events.push(flatten_event(&ev));
        }
        if world.tick.is_multiple_of(sample) {
            capture_frame(&world, stride, &mut frames);
            capture_sites(&world, &mut sites);
        }
        if biome_frames > 0 && world.tick.is_multiple_of(biome_every) {
            biome_grids.push(capture_biome(&world, biome_res));
        }
    }

    let events = thin_events(events, max_events);
    let hash = state_hash(&world);
    let replay = Replay {
        meta: Meta {
            scenario: scenario.name.clone(),
            seed: world.seed,
            ticks: world.tick,
            sample,
            world_size,
            stride,
            frame_count: frames.len(),
            event_count: events.len(),
            state_hash: format!("0x{hash:016x}"),
        },
        species,
        biome: BiomeSeq { res: biome_res, grids: biome_grids },
        frames,
        sites,
        events,
    };

    let json = serde_json::to_string(&replay).context("serialising replay")?;
    let is_json = out.extension().and_then(|e| e.to_str()) == Some("json");
    let payload = if is_json { json } else { format!("window.ANABIOS_REPLAY = {json};\n") };
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    let mut f = std::fs::File::create(&out)
        .with_context(|| format!("creating replay file {}", out.display()))?;
    f.write_all(payload.as_bytes()).context("writing replay")?;
    f.flush().context("flushing replay")?;

    println!(
        "wrote {} frames, {} events, {} biome grids → {} ({} KiB) state_hash={}",
        replay.meta.frame_count,
        replay.meta.event_count,
        replay.biome.grids.len(),
        out.display(),
        payload.len() / 1024,
        replay.meta.state_hash
    );
    Ok(())
}

/// Sample every `stride`-th alive agent's draw data into a new frame, plus
/// this tick's combat streaks and trade routes (scratch buffers, read-only)
/// so the web player can draw volleys, lanes, and action poses like the
/// Godot viewer does.
fn capture_frame(world: &World, stride: usize, frames: &mut Vec<Frame>) {
    let cap = world.agents.live_count() as usize / stride + 1;
    let mut id = Vec::with_capacity(cap);
    let mut x = Vec::with_capacity(cap);
    let mut y = Vec::with_capacity(cap);
    let mut sp = Vec::with_capacity(cap);
    let mut d = Vec::with_capacity(cap);
    for aid in world.agents.iter_alive() {
        if !(aid as usize).is_multiple_of(stride) {
            continue;
        }
        let i = aid as usize;
        let p = world.agents.position[i];
        id.push(aid);
        x.push(p.x.round() as i32);
        y.push(p.y.round() as i32);
        sp.push(world.agents.species_id[i]);
        let carn = effective_diet_carnivory(&world.agents.modules[i]).clamp(0.0, 1.0);
        d.push((carn * 255.0).round() as u8);
    }
    let mut st = Vec::with_capacity(world.combat_streaks.len() * 4);
    for (from, to, _) in &world.combat_streaks {
        st.push(from.x.round() as i32);
        st.push(from.y.round() as i32);
        st.push(to.x.round() as i32);
        st.push(to.y.round() as i32);
    }
    let mut tr = Vec::with_capacity(world.trade_routes.len() * 4);
    for (from, to, _) in &world.trade_routes {
        tr.push(from.x.round() as i32);
        tr.push(from.y.round() as i32);
        tr.push(to.x.round() as i32);
        tr.push(to.y.round() as i32);
    }
    frames.push(Frame { t: world.tick, id, x, y, sp, d, st, tr });
}

/// Snapshot the codex settlement latch: for each species with a formed
/// settlement, the centroid of its alive members' learned home anchors and
/// the anchored member count. Pure read — mirrors `settlement_sites` in the
/// Godot bridge (`crates/anabios-godot/src/lib.rs`).
fn capture_sites(world: &World, out: &mut Vec<SitesFrame>) {
    let mut f =
        SitesFrame { t: world.tick, sid: Vec::new(), x: Vec::new(), y: Vec::new(), n: Vec::new() };
    if !world.codex.settlement_active.is_empty() {
        let mut acc: BTreeMap<u32, (f64, f64, u32)> = BTreeMap::new();
        for aid in world.agents.iter_alive() {
            let sid = world.agents.species_id[aid as usize];
            if !world.codex.settlement_active.contains(&sid) {
                continue;
            }
            let a = world.agents.anchor[aid as usize];
            let e = acc.entry(sid).or_insert((0.0, 0.0, 0));
            e.0 += a.x as f64;
            e.1 += a.y as f64;
            e.2 += 1;
        }
        for (sid, (sx, sy, n)) in acc {
            if n == 0 {
                continue;
            }
            f.sid.push(sid);
            f.x.push((sx / n as f64).round() as i32);
            f.y.push((sy / n as f64).round() as i32);
            f.n.push(n);
        }
    }
    out.push(f);
}

/// Downsample the biome colour field to `out_res`² RGB cells (block-averaged),
/// base64-encoded. Colour mapping ported from the Godot bridge `biome_colors`.
fn capture_biome(world: &World, out_res: usize) -> BiomeGrid {
    let src_res = world.biome.res;
    let out_res = out_res.max(1);
    let mut bytes = Vec::with_capacity(out_res * out_res * 3);
    for oy in 0..out_res {
        let y0 = oy * src_res / out_res;
        let y1 = ((oy + 1) * src_res / out_res).max(y0 + 1).min(src_res);
        for ox in 0..out_res {
            let x0 = ox * src_res / out_res;
            let x1 = ((ox + 1) * src_res / out_res).max(x0 + 1).min(src_res);
            let (mut r, mut g, mut b, mut n) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for row in y0..y1 {
                for col in x0..x1 {
                    let c = anabios_core::biome::cell_color(world.biome.at(col, row));
                    r += c[0];
                    g += c[1];
                    b += c[2];
                    n += 1.0;
                }
            }
            let inv = if n > 0.0 { 1.0 / n } else { 0.0 };
            bytes.push((r * inv * 255.0).round().clamp(0.0, 255.0) as u8);
            bytes.push((g * inv * 255.0).round().clamp(0.0, 255.0) as u8);
            bytes.push((b * inv * 255.0).round().clamp(0.0, 255.0) as u8);
        }
    }
    BiomeGrid { t: world.tick, b64: base64_encode(&bytes) }
}

/// Thin the event stream to at most `max_events`, always keeping the first
/// occurrence of each event type (the "first emergence" milestones the
/// showcase is built around) and uniformly sampling the remainder. Chronology
/// is preserved. `max_events == 0` keeps everything.
fn thin_events(events: Vec<Event>, max_events: usize) -> Vec<Event> {
    if max_events == 0 || events.len() <= max_events {
        return events;
    }
    let mut seen: HashSet<&'static str> = HashSet::new();
    let mut keep = vec![false; events.len()];
    let mut firsts = 0usize;
    for (i, e) in events.iter().enumerate() {
        if seen.insert(e.kind) {
            keep[i] = true;
            firsts += 1;
        }
    }
    let others: Vec<usize> = (0..events.len()).filter(|&i| !keep[i]).collect();
    let budget = max_events.saturating_sub(firsts);
    if budget > 0 && !others.is_empty() {
        let step = others.len().div_ceil(budget).max(1);
        for (n, &i) in others.iter().enumerate() {
            if n.is_multiple_of(step) {
                keep[i] = true;
            }
        }
    }
    events.into_iter().enumerate().filter(|(i, _)| keep[*i]).map(|(_, e)| e).collect()
}

fn flatten_event(ev: &CodexEvent) -> Event {
    let sid = if ev.species_id == u32::MAX { None } else { Some(ev.species_id) };
    Event {
        t: ev.tick,
        kind: event_name(ev.event_type),
        sid,
        v: ev.value,
        x: ev.loc_x.round() as i32,
        y: ev.loc_y.round() as i32,
    }
}

/// Minimal standard-alphabet base64 (no padding omitted), dependency-free.
fn base64_encode(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_world() -> World {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/predator-prey.toml");
        let text = std::fs::read_to_string(path).expect("scenario");
        let mut s = Scenario::parse_toml(&text).expect("parse");
        s.seed = 7;
        s.instantiate()
    }

    /// Capturing frames + biome grids + sites is pure: a run that also captures
    /// must end with the same state hash as one that only steps and drains.
    #[test]
    fn capture_is_side_effect_free() {
        let mut recorded = minimal_world();
        let mut plain = recorded.clone();
        let mut frames = Vec::new();
        let mut sites = Vec::new();
        for _ in 0..300 {
            step(&mut recorded);
            for _ in recorded.codex.drain_events() {}
            capture_frame(&recorded, 3, &mut frames);
            capture_sites(&recorded, &mut sites);
            let _ = capture_biome(&recorded, 48);

            step(&mut plain);
            for _ in plain.codex.drain_events() {}
        }
        assert_eq!(state_hash(&recorded), state_hash(&plain));
    }

    /// Every captured frame has equal-length parallel arrays and in-range values.
    #[test]
    fn frames_are_well_formed() {
        let mut world = minimal_world();
        let mut frames = Vec::new();
        capture_frame(&world, 2, &mut frames);
        for _ in 0..200 {
            step(&mut world);
            for _ in world.codex.drain_events() {}
            capture_frame(&world, 2, &mut frames);
        }
        let ws = world.world_size.ceil() as i32;
        for f in &frames {
            let n = f.id.len();
            assert_eq!(f.x.len(), n);
            assert_eq!(f.y.len(), n);
            assert_eq!(f.sp.len(), n);
            assert_eq!(f.d.len(), n);
            for &x in &f.x {
                assert!((0..=ws).contains(&x), "x {x} out of 0..={ws}");
            }
            for &y in &f.y {
                assert!((0..=ws).contains(&y), "y {y} out of 0..={ws}");
            }
        }
    }

    /// A biome grid decodes to exactly out_res² RGB triples.
    #[test]
    fn biome_grid_has_expected_size() {
        let world = minimal_world();
        let g = capture_biome(&world, 40);
        // base64 of 40*40*3 = 4800 bytes → 6400 chars, no padding needed (4800%3==0).
        assert_eq!(g.b64.len(), 4800usize.div_ceil(3) * 4);
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    /// Thinning keeps every event type's first occurrence, stays within the cap
    /// (when kinds ≤ cap), and preserves chronological order.
    #[test]
    fn thin_events_keeps_firsts_within_cap() {
        let kinds = ["a", "b", "c"];
        let events: Vec<Event> = (0..30)
            .map(|t| Event { t, kind: kinds[t as usize % 3], sid: None, v: 0.0, x: 0, y: 0 })
            .collect();
        let out = thin_events(events, 10);
        assert!(out.len() <= 10, "exceeded cap: {}", out.len());
        for k in kinds {
            assert!(out.iter().any(|e| e.kind == k), "dropped first of {k}");
        }
        assert!(out.windows(2).all(|w| w[0].t <= w[1].t), "not chronological");
    }
}
