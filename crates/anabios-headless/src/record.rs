//! Record a scenario deterministically into a compact replay stream for the
//! web showcase player (`showcase/`).
//!
//! The recorder runs the same `step` loop as every other subcommand, sampling
//! per-agent draw data every `sample` ticks and draining codex events every
//! tick. Reading the agent columns is pure and RNG-free, so recording does not
//! perturb the run — the final `state_hash` matches a plain `run` of the same
//! scenario/seed/ticks. The per-agent draw logic mirrors the Godot bridge
//! (`crates/anabios-godot/src/lib.rs`) so the web player shows the same world.
//!
//! Output is a single `.js` file assigning `window.ANABIOS_REPLAY = {…}` (so the
//! player loads it via `<script>` with no fetch/CORS), or raw `.json` when the
//! `--out` path ends in `.json`. Coordinates are kept in world units and
//! rounded to integers; diet is quantised to 0..=255. Agent slot ids are
//! included per frame so the player can interpolate positions between samples.

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::PathBuf;

use anabios_core::codex::CodexEvent;
use anabios_core::module::effective_diet_carnivory;
use anabios_core::scenario::Scenario;
use anabios_core::snapshot::state_hash;
use anabios_core::tick::step;
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
    frames: Vec<Frame>,
    events: Vec<Event>,
}

pub fn run(
    scenario_path: PathBuf,
    ticks: u64,
    seed: Option<u64>,
    sample: u64,
    max_agents: usize,
    max_events: usize,
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

    println!(
        "recording scenario={} seed={} ticks={} sample={} agents≈{}/{} (stride {})",
        scenario.name,
        world.seed,
        ticks,
        sample,
        initial_live / stride,
        initial_live,
        stride
    );

    let mut frames: Vec<Frame> = Vec::with_capacity((ticks / sample) as usize + 1);
    let mut events: Vec<Event> = Vec::new();

    // Frame 0 (initial placement) then one every `sample` ticks.
    capture_frame(&world, stride, &mut frames);
    for _ in 0..ticks {
        step(&mut world);
        for ev in world.codex.drain_events() {
            events.push(flatten_event(&ev));
        }
        if world.tick.is_multiple_of(sample) {
            capture_frame(&world, stride, &mut frames);
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
        frames,
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
        "wrote {} frames, {} events → {} ({} KiB) state_hash={}",
        replay.meta.frame_count,
        replay.meta.event_count,
        out.display(),
        payload.len() / 1024,
        replay.meta.state_hash
    );
    Ok(())
}

/// Sample every `stride`-th alive agent's draw data into a new frame.
fn capture_frame(world: &anabios_core::World, stride: usize, frames: &mut Vec<Frame>) {
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
    frames.push(Frame { t: world.tick, id, x, y, sp, d });
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
