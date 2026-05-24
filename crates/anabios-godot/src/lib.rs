//! anabios-godot — Godot 4.6 extension binding for anabios-core.
//!
//! Exposes a single `Simulation` node class that GDScript can construct,
//! advance with `step_n()`, and query for per-tick agent buffers + codex
//! events. UI logic lives in GDScript; this crate is purely the bridge.

// gdext's #[godot_api] expands to glue that returns a large CallError
// enum variant. We can't change that, so silence the lint crate-wide.
#![allow(clippy::result_large_err)]

use godot::prelude::*;

struct AnabiosExtension;

#[gdextension]
unsafe impl ExtensionLibrary for AnabiosExtension {}

/// One in-process anabios simulation, owned by a Godot `Simulation` node.
#[derive(GodotClass)]
#[class(base = Node)]
pub struct Simulation {
    #[allow(dead_code)]
    base: Base<Node>,
    inner: Option<anabios_core::World>,
}

#[godot_api]
impl INode for Simulation {
    fn init(base: Base<Node>) -> Self {
        Self { base, inner: None }
    }
}

#[godot_api]
impl Simulation {
    /// Construct a new world from a seed. Idempotent — calling again resets.
    #[func]
    fn new_world(&mut self, seed: i64) {
        self.inner = Some(anabios_core::World::new(seed as u64));
    }

    /// Load a TOML scenario (passed as a Godot string). Returns true on
    /// success; false (and logs an error) on parse failure.
    #[func]
    fn load_scenario(&mut self, toml_text: GString) -> bool {
        let text = String::from(toml_text);
        match anabios_core::Scenario::parse_toml(&text) {
            Ok(s) => {
                self.inner = Some(s.instantiate());
                true
            }
            Err(e) => {
                godot_error!("scenario parse failed: {e}");
                false
            }
        }
    }

    /// Advance the simulation by N ticks.
    #[func]
    fn step_n(&mut self, n: i64) {
        if let Some(w) = self.inner.as_mut() {
            for _ in 0..n.max(0) {
                anabios_core::tick::step(w);
            }
        }
    }

    /// Current tick number.
    #[func]
    fn tick(&self) -> i64 {
        self.inner.as_ref().map(|w| w.tick as i64).unwrap_or(0)
    }

    /// Number of alive agents.
    #[func]
    fn alive_count(&self) -> i64 {
        self.inner.as_ref().map(|w| w.agents.live_count() as i64).unwrap_or(0)
    }

    /// World extent (a square). UI uses this to set camera limits.
    #[func]
    fn world_size(&self) -> f32 {
        anabios_core::biome::WORLD_SIZE
    }

    /// Return alive-agent positions as a Vector2 array, in ascending
    /// agent-id order. Dead agents are skipped.
    #[func]
    fn alive_positions(&self) -> PackedVector2Array {
        let mut out = PackedVector2Array::new();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                let p = w.agents.position[id as usize];
                out.push(Vector2::new(p.x, p.y));
            }
        }
        out
    }

    /// Return alive-agent colors derived from genome color slots, in the
    /// same order as `alive_positions`.
    #[func]
    fn alive_colors(&self) -> PackedColorArray {
        use anabios_core::genome::GenomeSlot;
        let mut out = PackedColorArray::new();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                let g = &w.agents.genome[id as usize];
                let h = g.get(GenomeSlot::ColorHue);
                let s = g.get(GenomeSlot::ColorSat).clamp(0.4, 1.0);
                let v = g.get(GenomeSlot::ColorVal).clamp(0.5, 1.0);
                out.push(hsv_to_color(h, s, v));
            }
        }
        out
    }

    /// Each agent's size in world units (used by MultiMesh scale).
    #[func]
    fn alive_sizes(&self) -> PackedFloat32Array {
        use anabios_core::genome::GenomeSlot;
        let mut out = PackedFloat32Array::new();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                let g = &w.agents.genome[id as usize];
                let s = 0.5 + 2.5 * g.get(GenomeSlot::Size);
                out.push(s);
            }
        }
        out
    }

    /// Look up one alive agent by id. Returns a Dictionary; empty if dead.
    #[func]
    fn get_agent_info(&self, id: i64) -> Dictionary {
        let mut d = Dictionary::new();
        let Some(w) = self.inner.as_ref() else { return d };
        let aid = id as u32;
        if !w.agents.is_alive(aid) {
            d.set("alive", false);
            return d;
        }
        let i = id as usize;
        let p = w.agents.position[i];
        d.set("id", id);
        d.set("alive", true);
        d.set("position", Vector2::new(p.x, p.y));
        d.set("energy", w.agents.energy[i]);
        d.set("age", w.agents.age[i] as i64);
        d.set("lineage_id", w.agents.lineage_id[i] as i64);
        d.set("species_id", w.agents.species_id[i] as i64);
        d.set("program_len", w.agents.program[i].len() as i64);
        d.set("module_count", w.agents.modules[i].len() as i64);
        let mut g = PackedFloat32Array::new();
        for v in w.agents.genome[i].0.iter() {
            g.push(*v);
        }
        d.set("genome", g);
        d
    }

    /// Drain the codex event buffer. Each event becomes a Dictionary:
    ///   { type: int (0=Extinction, 1=PopulationCrash, 2=SpeciationEvent),
    ///     tick: int, species_id: int, value: f32 }
    #[func]
    fn take_codex_events(&mut self) -> Array<Dictionary> {
        let mut out = Array::<Dictionary>::new();
        let Some(w) = self.inner.as_mut() else { return out };
        for ev in w.codex.drain_events() {
            let mut d = Dictionary::new();
            d.set("type", ev.event_type as u8 as i64);
            d.set("tick", ev.tick as i64);
            d.set("species_id", ev.species_id as i64);
            d.set("value", ev.value);
            out.push(&d);
        }
        out
    }

    /// Find the closest alive agent to a world position, within `radius`
    /// world units. Returns the agent id, or -1 if no agent in range.
    #[func]
    fn agent_near(&self, pos: Vector2, radius: f32) -> i64 {
        let Some(w) = self.inner.as_ref() else { return -1 };
        let p = glam::Vec2::new(pos.x, pos.y);
        let mut best_id: i64 = -1;
        let mut best_d = radius;
        for id in w.agents.iter_alive() {
            let q = w.agents.position[id as usize];
            let d = (q - p).length();
            if d < best_d {
                best_d = d;
                best_id = id as i64;
            }
        }
        best_id
    }
}

fn hsv_to_color(h: f32, s: f32, v: f32) -> Color {
    let h6 = (h.rem_euclid(1.0)) * 6.0;
    let i = h6.floor() as i32;
    let f = h6 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::from_rgb(r, g, b)
}
