//! anabios-godot — Godot 4.7 extension binding for anabios-core.
//!
//! Exposes a single `Simulation` node class that GDScript can construct,
//! advance with `step_n()`, and query for per-tick agent buffers + codex
//! events. UI logic lives in GDScript; this crate is purely the bridge.

// gdext's #[godot_api] expands to glue that returns a large CallError
// enum variant. We can't change that, so silence the lint crate-wide.
#![allow(clippy::result_large_err)]

use godot::builtin::VarDictionary;
use godot::prelude::*;

mod coevo;

struct AnabiosExtension;

#[gdextension]
unsafe impl ExtensionLibrary for AnabiosExtension {}

/// One per-tick co-evolution metric snapshot. Plain data, lives OUTSIDE
/// `World` (on the `Simulation` node) so determinism is untouched.
#[derive(Clone, Copy)]
struct CoevoSample {
    tick: f32,
    communicator_frac: f32,
    mean_social_learning: f32,
    mean_individual_learning: f32,
    genetic_diversity: f32,
    mean_skill: f32,
    mean_tech_match: f32,
    meme_divergence: f32,
    live_count: f32,
    species_count: f32,
    env_optimum: f32,
    /// World adoption fraction per invention (share of alive agents at or
    /// above the held threshold; all zero when the tree is inactive).
    inv_adopt_frac: [f32; anabios_core::invention::INVENTION_COUNT],
}

/// Soft cap on retained samples (~tens of KB each thousand ticks). Past this we
/// stop appending and log once, rather than grow without bound.
const COEVO_HISTORY_CAP: usize = 200_000;

/// A codex event retained for the timeline. Plain data on the `Simulation`
/// node (outside `World`).
#[derive(Clone, Copy)]
struct StoredEvent {
    event_type: i64,
    tick: i64,
    species_id: i64,
    value: f32,
    loc_x: f32,
    loc_y: f32,
}

/// Reusable buffers for `sample_into` — avoids 5 Vec + 1 BTreeSet
/// allocations per sampled tick in the render loop.
#[derive(Default)]
struct SampleScratch {
    memes: Vec<[f32; anabios_core::program::MEME_CHANNELS]>,
    genomes: Vec<anabios_core::genome::Genome>,
    species: Vec<u32>,
    xs: Vec<f32>,
    comm: Vec<bool>,
    species_set: std::collections::BTreeSet<u32>,
}

/// One in-process anabios simulation, owned by a Godot `Simulation` node.
#[derive(GodotClass)]
#[class(base = Node)]
pub struct Simulation {
    #[allow(dead_code)]
    base: Base<Node>,
    inner: Option<anabios_core::World>,
    history: Vec<CoevoSample>,
    history_capped_logged: bool,
    events: Vec<StoredEvent>,
    sample_scratch: SampleScratch,
}

#[godot_api]
impl INode for Simulation {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            inner: None,
            history: Vec::new(),
            history_capped_logged: false,
            events: Vec::new(),
            sample_scratch: SampleScratch::default(),
        }
    }
}

#[godot_api]
impl Simulation {
    /// Construct a new world from a seed. Idempotent — calling again resets.
    #[func]
    fn new_world(&mut self, seed: i64) {
        self.inner = Some(anabios_core::World::new(seed as u64));
        self.reset_history();
    }

    /// Load a TOML scenario (passed as a Godot string). Returns true on
    /// success; false (and logs an error) on parse failure.
    #[func]
    fn load_scenario(&mut self, toml_text: GString) -> bool {
        let text = String::from(toml_text);
        match anabios_core::Scenario::parse_toml(&text) {
            Ok(s) => {
                self.inner = Some(s.instantiate());
                self.reset_history();
                true
            }
            Err(e) => {
                godot_error!("scenario parse failed: {e}");
                false
            }
        }
    }

    /// Load a TOML scenario but override its seed. Returns true on success.
    #[func]
    fn load_scenario_with_seed(&mut self, toml_text: GString, seed: i64) -> bool {
        let text = String::from(toml_text);
        match anabios_core::Scenario::parse_toml(&text) {
            Ok(mut s) => {
                s.seed = seed as u64;
                self.inner = Some(s.instantiate());
                self.reset_history();
                true
            }
            Err(e) => {
                godot_error!("scenario parse failed: {e}");
                false
            }
        }
    }

    /// Advance the simulation by N ticks, sampling co-evolution metrics into
    /// the view-only history buffer after each tick.
    #[func]
    fn step_n(&mut self, n: i64) {
        for _ in 0..n.max(0) {
            let Some(w) = self.inner.as_mut() else { return };
            anabios_core::tick::step(w);
            // Persist codex events for the timeline (single drain site). Draining
            // is an output read; it does not feed back into `step`, so per-tick
            // vs per-frame draining leaves World evolution identical.
            for ev in w.codex.drain_events() {
                if self.events.len() < COEVO_HISTORY_CAP {
                    self.events.push(StoredEvent {
                        event_type: ev.event_type as u8 as i64,
                        tick: ev.tick as i64,
                        species_id: ev.species_id as i64,
                        value: ev.value,
                        loc_x: ev.loc_x,
                        loc_y: ev.loc_y,
                    });
                }
            }
            if self.history.len() < COEVO_HISTORY_CAP {
                let s = sample_into(self.inner.as_ref().unwrap(), &mut self.sample_scratch);
                self.history.push(s);
            } else if !self.history_capped_logged {
                godot_warn!("coevo history hit {COEVO_HISTORY_CAP} samples; no longer recording");
                self.history_capped_logged = true;
            }
        }
    }

    /// Clear all view-only recording buffers (called on world (re)load).
    fn reset_history(&mut self) {
        self.history.clear();
        self.history_capped_logged = false;
        self.events.clear();
    }

    /// Current-tick co-evolution scalars (see plan/spec for key meanings).
    /// All frequencies/means in `[0,1]`; `env_optimum` is `-1.0` when inactive.
    #[func]
    fn coevo_metrics(&mut self) -> VarDictionary {
        match self.inner.as_ref() {
            Some(w) => {
                let s = sample_into(w, &mut self.sample_scratch);
                sample_to_dict(&s)
            }
            None => VarDictionary::new(),
        }
    }

    /// Number of recorded per-tick samples.
    #[func]
    fn coevo_history_len(&self) -> i64 {
        self.history.len() as i64
    }

    /// Full history of one series, oldest-first. Unknown key returns empty.
    /// Invention adoption series are `inv_<key>_frac` (e.g. `inv_farming_frac`).
    #[func]
    fn coevo_series(&self, key: GString) -> PackedFloat32Array {
        let mut out = PackedFloat32Array::new();
        let key_str = String::from(key);
        if let Some(inv_key) = key_str.strip_prefix("inv_").and_then(|k| k.strip_suffix("_frac")) {
            let Some(idx) =
                anabios_core::invention::INVENTIONS.iter().position(|i| i.key == inv_key)
            else {
                return out;
            };
            for s in &self.history {
                out.push(s.inv_adopt_frac[idx]);
            }
            return out;
        }
        let pick: fn(&CoevoSample) -> f32 = match key_str.as_str() {
            "tick" => |s| s.tick,
            "communicator_frac" => |s| s.communicator_frac,
            "mean_social_learning" => |s| s.mean_social_learning,
            "mean_individual_learning" => |s| s.mean_individual_learning,
            "genetic_diversity" => |s| s.genetic_diversity,
            "mean_skill" => |s| s.mean_skill,
            "mean_tech_match" => |s| s.mean_tech_match,
            "meme_divergence" => |s| s.meme_divergence,
            "live_count" => |s| s.live_count,
            "species_count" => |s| s.species_count,
            "env_optimum" => |s| s.env_optimum,
            _ => return out,
        };
        for s in &self.history {
            out.push(pick(s));
        }
        out
    }

    /// One historical sample as a Dictionary (for the scrub readout).
    /// Out-of-range index returns empty.
    #[func]
    fn coevo_sample_at(&self, index: i64) -> VarDictionary {
        match (index >= 0).then(|| self.history.get(index as usize)).flatten() {
            Some(s) => sample_to_dict(s),
            None => VarDictionary::new(),
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
        self.inner.as_ref().map(|w| w.world_size).unwrap_or(anabios_core::biome::WORLD_SIZE_DEFAULT)
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

    /// Carnivory diet score per alive agent (0 herbivore .. 1 carnivore),
    /// same order as `alive_positions`.
    #[func]
    fn alive_diet(&self) -> PackedFloat32Array {
        let mut out = PackedFloat32Array::new();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                out.push(anabios_core::module::effective_diet_carnivory(
                    &w.agents.modules[id as usize],
                ));
            }
        }
        out
    }

    /// Dialect hue per alive agent in `[0,1)`, same order as `alive_positions`.
    #[func]
    fn alive_dialect_hue(&self) -> PackedFloat32Array {
        let mut out = PackedFloat32Array::new();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                out.push(dialect_hue(&w.agents.meme_vector[id as usize]));
            }
        }
        out
    }

    /// Energy per alive agent, same order as `alive_positions`.
    #[func]
    fn alive_energy(&self) -> PackedFloat32Array {
        let mut out = PackedFloat32Array::new();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                out.push(w.agents.energy[id as usize]);
            }
        }
        out
    }

    /// Look up one alive agent by id. Returns a Dictionary; empty if dead.
    #[func]
    fn get_agent_info(&self, id: i64) -> VarDictionary {
        let mut d = VarDictionary::new();
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
        d.set("genome", &g);
        d
    }

    /// Full inspector view of one alive agent. Superset of `get_agent_info`,
    /// adding diet, learned skill/technique, learning flags, dialect hue, and
    /// module names.
    #[func]
    fn agent_detail(&self, id: i64) -> VarDictionary {
        use anabios_core::culture::{SKILL_CHANNEL, TECH_CHANNEL};
        use anabios_core::genome::GenomeSlot;
        let mut d = self.get_agent_info(id);
        let Some(w) = self.inner.as_ref() else { return d };
        let aid = id as u32;
        if !w.agents.is_alive(aid) {
            return d;
        }
        let i = id as usize;
        let g = &w.agents.genome[i];
        let meme = &w.agents.meme_vector[i];
        d.set(
            "diet_carnivory",
            anabios_core::module::effective_diet_carnivory(&w.agents.modules[i]),
        );
        d.set("skill", meme[SKILL_CHANNEL]);
        d.set("technique", meme[TECH_CHANNEL]);
        d.set("indiv_learn", g.get(GenomeSlot::IndividualLearning) > 0.5);
        d.set("social_learn", g.get(GenomeSlot::SocialLearning) > 0.5);
        d.set("dialect_hue", dialect_hue(meme));
        let mut names = PackedStringArray::new();
        for m in w.agents.modules[i].iter() {
            names.push(&format!("{:?}", m.module_type()));
        }
        d.set("module_names", &names);
        // Invention tree: held techs, their levels, and the tech era.
        let mask = anabios_core::invention::held_mask(meme);
        let mut held = PackedStringArray::new();
        let mut levels = VarDictionary::new();
        anabios_core::invention::for_each_set_bit(mask, |k| {
            let inv = &anabios_core::invention::INVENTIONS[k];
            held.push(inv.name);
            levels.set(inv.key, anabios_core::invention::level(meme, k));
        });
        d.set("inventions", &held);
        d.set("invention_levels", &levels);
        d.set("tech_era", anabios_core::invention::tech_era(mask) as i64);
        d
    }

    /// The full invention tree metadata (static): one Dictionary per
    /// invention — `{ key, name, era, prereqs: PackedStringArray of keys,
    /// buff, debuff }`. Lets GDScript render the tree without duplicating it.
    #[func]
    fn invention_catalog(&self) -> Array<VarDictionary> {
        let mut out = Array::<VarDictionary>::new();
        for inv in &anabios_core::invention::INVENTIONS {
            let mut d = VarDictionary::new();
            d.set("key", inv.key);
            d.set("name", inv.name);
            d.set("era", inv.era as i64);
            let mut prereqs = PackedStringArray::new();
            anabios_core::invention::for_each_set_bit(inv.prereqs, |p| {
                prereqs.push(anabios_core::invention::INVENTIONS[p].key);
            });
            d.set("prereqs", &prereqs);
            d.set("buff", inv.buff);
            d.set("debuff", inv.debuff);
            out.push(&d);
        }
        out
    }

    /// Total codex events recorded so far.
    #[func]
    fn codex_event_count(&self) -> i64 {
        self.events.len() as i64
    }

    /// Codex events at log index `>= cursor`, non-draining. Callers track their
    /// own cursor (use the returned `index + 1`, or `codex_event_count`). Each
    /// event becomes a Dictionary:
    ///   { index: int, type: int, tick: int, species_id: int,
    ///     value: f32, loc: Vector2 }
    #[func]
    fn codex_events_since(&self, cursor: i64) -> Array<VarDictionary> {
        let mut out = Array::<VarDictionary>::new();
        let start = cursor.max(0) as usize;
        for (idx, ev) in self.events.iter().enumerate().skip(start) {
            let mut d = VarDictionary::new();
            d.set("index", idx as i64);
            d.set("type", ev.event_type);
            d.set("tick", ev.tick);
            d.set("species_id", ev.species_id);
            d.set("value", ev.value);
            d.set("loc", Vector2::new(ev.loc_x, ev.loc_y));
            out.push(&d);
        }
        out
    }

    /// Biome grid resolution per axis (cells = res²).
    #[func]
    fn biome_resolution(&self) -> i64 {
        self.inner
            .as_ref()
            .map(|w| w.biome_res as i64)
            .unwrap_or(anabios_core::biome::BIOME_RES_DEFAULT as i64)
    }

    /// One color per biome cell, row-major (`row * RES + col`). Terrain
    /// type sets the base hue; live plant biomass brightens grass/forest
    /// cells. Returns `RES²` colors, or empty if no world is loaded.
    #[func]
    fn biome_colors(&self) -> PackedColorArray {
        use anabios_core::biome::TerrainType;
        let mut out = PackedColorArray::new();
        let Some(w) = self.inner.as_ref() else { return out };
        for cell in w.biome.cells.iter() {
            let base = match cell.terrain {
                TerrainType::Water => Color::from_rgb(0.12, 0.22, 0.42),
                TerrainType::Grass => Color::from_rgb(0.20, 0.42, 0.18),
                TerrainType::Forest => Color::from_rgb(0.10, 0.30, 0.12),
                TerrainType::Desert => Color::from_rgb(0.62, 0.56, 0.34),
                TerrainType::Rock => Color::from_rgb(0.36, 0.36, 0.40),
            };
            let cap = cell.terrain.carrying_capacity();
            let frac = if cap > 0.0 { (cell.plant_biomass / cap).clamp(0.0, 1.0) } else { 0.0 };
            let lit = base.lerp(Color::from_rgb(0.40, 0.85, 0.35), (frac * 0.6) as f64);
            out.push(lit);
        }
        out
    }

    /// Number of pheromone channels (for the overlay cycling loop).
    #[func]
    fn pheromone_channel_count(&self) -> i64 {
        anabios_core::program::PHEROMONE_CHANNELS as i64
    }

    /// One color per pheromone cell on `channel`, row-major (`row * RES + col`),
    /// as a dark-to-hot ramp with alpha proportional to intensity. Returns
    /// `RES²` colors, or empty if no world is loaded or the channel is invalid.
    #[func]
    fn pheromone_colors(&self, channel: i64) -> PackedColorArray {
        let mut out = PackedColorArray::new();
        let Some(w) = self.inner.as_ref() else { return out };
        let ch = channel as usize;
        if ch >= anabios_core::program::PHEROMONE_CHANNELS {
            return out;
        }
        for cell in w.pheromones.cells.iter() {
            let t = phero_intensity(cell[ch]);
            let c = Color::from_rgb(0.05 + 0.95 * t, 0.10 * t, 0.30 * (1.0 - t));
            out.push(Color { a: t, ..c });
        }
        out
    }

    /// True iff the DIT environment mechanism is active (`env_period > 0`).
    #[func]
    fn env_active(&self) -> bool {
        self.inner.as_ref().map(|w| w.env_period > 0).unwrap_or(false)
    }

    /// Current global optimal technique in `[0,1]`, or `-1.0` when the env
    /// mechanism is inactive.
    #[func]
    fn env_optimum(&self) -> f32 {
        match self.inner.as_ref() {
            Some(w) if w.env_period > 0 => {
                anabios_core::culture::env_optimum_at(w.tick, w.env_period)
            }
            _ => -1.0,
        }
    }

    /// Per-live-species aggregate stats. `mean_technique_match` is the mean of
    /// `technique_match(meme[TECH_CHANNEL], optimum)` when the env mechanism is
    /// active, else `0.0`. `adopted_inventions` lists the keys of inventions at
    /// ≥50% adoption inside the species, `tech_era` the highest era among them.
    /// Each entry is
    /// `{ species_id, count, mean_energy, mean_technique_match,
    ///    adopted_inventions, tech_era }`.
    #[func]
    fn species_stats(&self) -> Array<VarDictionary> {
        use anabios_core::culture::{env_optimum_at, technique_match, TECH_CHANNEL};
        use anabios_core::invention::{for_each_set_bit, held_mask, INVENTIONS, INVENTION_COUNT};
        use std::collections::BTreeMap;
        let mut out = Array::<VarDictionary>::new();
        let Some(w) = self.inner.as_ref() else { return out };
        let active = w.env_period > 0;
        let opt = if active { env_optimum_at(w.tick, w.env_period) } else { 0.0 };
        // Aggregate over live agents, keyed by species_id (BTreeMap keeps the
        // output stable and ascending).
        let mut count: BTreeMap<u32, i64> = BTreeMap::new();
        let mut energy: BTreeMap<u32, f32> = BTreeMap::new();
        let mut matchsum: BTreeMap<u32, f32> = BTreeMap::new();
        let mut inv_counts: BTreeMap<u32, [u32; INVENTION_COUNT]> = BTreeMap::new();
        for id in w.agents.iter_alive() {
            let i = id as usize;
            let sp = w.agents.species_id[i];
            *count.entry(sp).or_insert(0) += 1;
            *energy.entry(sp).or_insert(0.0) += w.agents.energy[i];
            if active {
                let tech = w.agents.meme_vector[i][TECH_CHANNEL];
                *matchsum.entry(sp).or_insert(0.0) += technique_match(tech, opt);
            }
            if w.inventions_enabled {
                let counts = inv_counts.entry(sp).or_insert([0; INVENTION_COUNT]);
                for_each_set_bit(held_mask(&w.agents.meme_vector[i]), |k| counts[k] += 1);
            }
        }
        for (sp, n) in count.iter() {
            let mut d = VarDictionary::new();
            let nf = *n as f32;
            d.set("species_id", *sp as i64);
            d.set("count", *n);
            d.set("mean_energy", energy[sp] / nf);
            d.set("mean_technique_match", if active { matchsum[sp] / nf } else { 0.0 });
            let mut adopted = PackedStringArray::new();
            let mut mask = 0u32;
            if let Some(counts) = inv_counts.get(sp) {
                for (k, &holders) in counts.iter().enumerate() {
                    if holders as f32 / nf >= 0.5 {
                        adopted.push(INVENTIONS[k].key);
                        mask |= anabios_core::invention::bit(k);
                    }
                }
            }
            d.set("adopted_inventions", &adopted);
            d.set("tech_era", anabios_core::invention::tech_era(mask) as i64);
            out.push(&d);
        }
        out
    }

    /// Whether the loaded world has the invention tree active.
    #[func]
    fn inventions_enabled(&self) -> bool {
        self.inner.as_ref().map(|w| w.inventions_enabled).unwrap_or(false)
    }

    /// One entry per carcass currently in the world:
    /// `{ pos, flesh, age, species_id }`.
    #[func]
    fn carcass_data(&self) -> Array<VarDictionary> {
        let mut out = Array::<VarDictionary>::new();
        let Some(w) = self.inner.as_ref() else { return out };
        for c in w.carcasses.iter() {
            let mut d = VarDictionary::new();
            d.set("pos", Vector2::new(c.pos.x, c.pos.y));
            d.set("flesh", c.flesh);
            d.set("age", c.age as i64);
            d.set("species_id", c.species_id as i64);
            out.push(&d);
        }
        out
    }

    /// World positions of alive agents that took combat damage on the most
    /// recent tick (the flag is reset at the start of the next combat pass).
    #[func]
    fn combat_flashes(&self) -> PackedVector2Array {
        let mut out = PackedVector2Array::new();
        let Some(w) = self.inner.as_ref() else { return out };
        for id in w.agents.iter_alive() {
            let i = id as usize;
            if w.combat_damaged.get(i).copied().unwrap_or(false) {
                let p = w.agents.position[i];
                out.push(Vector2::new(p.x, p.y));
            }
        }
        out
    }

    /// Body rotation (radians) per alive agent, from velocity direction.
    /// Non-moving agents keep rotation 0. Same order as `alive_positions`.
    #[func]
    fn alive_rotations(&self) -> PackedFloat32Array {
        let mut out = PackedFloat32Array::new();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                let v = w.agents.velocity[id as usize];
                let r = if v.length_squared() > 1e-6 { v.y.atan2(v.x) } else { 0.0 };
                out.push(r);
            }
        }
        out
    }

    /// Number of module types (for the GDScript layer loop).
    #[func]
    fn module_type_count(&self) -> i64 {
        9
    }

    /// All module glyphs in ONE alive pass, bucketed by module type. Returns an
    /// array of length `module_type_count()` (9); entry `t` is a
    /// `PackedVector2Array` of world positions for every module of type `t`.
    /// Read-only view — replaces nine separate `module_glyphs(t)` passes.
    #[func]
    fn module_glyphs_all(&self) -> Array<PackedVector2Array> {
        use anabios_core::genome::GenomeSlot;
        let type_count = 9usize; // matches module_type_count()
        let mut buckets: Vec<PackedVector2Array> =
            (0..type_count).map(|_| PackedVector2Array::new()).collect();
        if let Some(w) = self.inner.as_ref() {
            for id in w.agents.iter_alive() {
                let i = id as usize;
                let pos = w.agents.position[i];
                let size = 0.5 + 2.5 * w.agents.genome[i].get(GenomeSlot::Size);
                let radius = size * 0.7;
                for (slot, m) in w.agents.modules[i].iter().enumerate() {
                    let t = m.module_type() as usize;
                    if t >= type_count {
                        continue;
                    }
                    let angle = (slot as f32) * std::f32::consts::TAU / 8.0;
                    let gx = pos.x + radius * math_cos(angle);
                    let gy = pos.y + radius * math_sin(angle);
                    buckets[t].push(Vector2::new(gx, gy));
                }
            }
        }
        let mut out: Array<PackedVector2Array> = Array::new();
        for b in &buckets {
            out.push(b);
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

#[inline]
fn math_cos(x: f32) -> f32 {
    libm::cosf(x)
}

#[inline]
fn math_sin(x: f32) -> f32 {
    libm::sinf(x)
}

/// Map a raw pheromone concentration to a saturating intensity in `[0,1]`
/// (deposits are unbounded and decay is slow, so a plain clamp would wash out).
fn phero_intensity(v: f32) -> f32 {
    let x = v.max(0.0);
    1.0 - (-x).exp()
}

/// Project a meme vector onto a stable hue in `[0,1)` so divergent dialects
/// render as distinct body colors. The per-channel weights are normalized to
/// sum to 1, so the hue is a weighted average of the meme values (bounded and
/// not dominated by any single high-index channel) wrapped into `[0,1)`.
fn dialect_hue(meme: &[f32]) -> f32 {
    let mut acc = 0.0_f32;
    let mut wsum = 0.0_f32;
    for (k, v) in meme.iter().enumerate() {
        let w = 0.37 + 0.11 * k as f32;
        acc += v * w;
        wsum += w;
    }
    if wsum <= 0.0 {
        return 0.0;
    }
    (acc / wsum).rem_euclid(1.0)
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

/// Compute a `CoevoSample` from a read-only world, filling `scratch` with
/// compact live-only slices (cleared + reused across ticks), then delegating
/// to the pure `coevo` helpers.
fn sample_into(w: &anabios_core::World, scratch: &mut SampleScratch) -> CoevoSample {
    use anabios_core::culture::{env_optimum_at, SKILL_CHANNEL};
    use anabios_core::genome::GenomeSlot;
    use anabios_core::module::{self, ModuleType};

    scratch.memes.clear();
    scratch.genomes.clear();
    scratch.species.clear();
    scratch.xs.clear();
    scratch.comm.clear();
    scratch.species_set.clear();
    for id in w.agents.iter_alive() {
        let i = id as usize;
        scratch.memes.push(w.agents.meme_vector[i]);
        scratch.genomes.push(w.agents.genome[i]);
        scratch.species.push(w.agents.species_id[i]);
        scratch.xs.push(w.agents.position[i].x);
        scratch.comm.push(module::has(&w.agents.modules[i], ModuleType::Communicator));
        scratch.species_set.insert(w.agents.species_id[i]);
    }
    let (memes, genomes, species, xs, comm) =
        (&scratch.memes, &scratch.genomes, &scratch.species, &scratch.xs, &scratch.comm);
    let active = w.env_period > 0;
    let opt = if active { env_optimum_at(w.tick, w.env_period) } else { 0.0 };
    let mut inv_adopt_frac = [0.0f32; anabios_core::invention::INVENTION_COUNT];
    if w.inventions_enabled && !memes.is_empty() {
        for m in memes {
            anabios_core::invention::for_each_set_bit(anabios_core::invention::held_mask(m), |k| {
                inv_adopt_frac[k] += 1.0
            });
        }
        for f in inv_adopt_frac.iter_mut() {
            *f /= memes.len() as f32;
        }
    }
    CoevoSample {
        tick: w.tick as f32,
        communicator_frac: coevo::frac_true(comm),
        mean_social_learning: coevo::mean_slot(genomes, GenomeSlot::SocialLearning),
        mean_individual_learning: coevo::mean_slot(genomes, GenomeSlot::IndividualLearning),
        genetic_diversity: coevo::genetic_diversity(genomes),
        mean_skill: coevo::mean_channel_over(memes, comm, SKILL_CHANNEL),
        mean_tech_match: if active { coevo::mean_tech_match(memes, comm, opt) } else { 0.0 },
        meme_divergence: coevo::species_max_meme_divergence(memes, species, xs, comm),
        live_count: memes.len() as f32,
        species_count: scratch.species_set.len() as f32,
        env_optimum: if active { opt } else { -1.0 },
        inv_adopt_frac,
    }
}

/// Serialize a `CoevoSample` into a Godot Dictionary (shared by the live-metrics
/// and scrub-readout exports).
fn sample_to_dict(s: &CoevoSample) -> VarDictionary {
    let mut d = VarDictionary::new();
    d.set("tick", s.tick as i64);
    d.set("communicator_frac", s.communicator_frac);
    d.set("mean_social_learning", s.mean_social_learning);
    d.set("mean_individual_learning", s.mean_individual_learning);
    d.set("genetic_diversity", s.genetic_diversity);
    d.set("mean_skill", s.mean_skill);
    d.set("mean_tech_match", s.mean_tech_match);
    d.set("meme_divergence", s.meme_divergence);
    d.set("live_count", s.live_count);
    d.set("species_count", s.species_count);
    d.set("env_optimum", s.env_optimum);
    let mut inv = VarDictionary::new();
    for (k, f) in s.inv_adopt_frac.iter().enumerate() {
        inv.set(anabios_core::invention::INVENTIONS[k].key, *f);
    }
    d.set("inv_adopt_frac", &inv);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_now_is_bounded_and_nonneg() {
        // Instantiate the shipped minimal scenario and step a few ticks.
        let toml = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scenarios/minimal.toml"
        ))
        .expect("read minimal.toml");
        let mut w = anabios_core::Scenario::parse_toml(&toml).unwrap().instantiate();
        for _ in 0..25 {
            anabios_core::tick::step(&mut w);
        }
        let s = super::sample_into(&w, &mut super::SampleScratch::default());
        for v in [
            s.communicator_frac,
            s.mean_social_learning,
            s.mean_individual_learning,
            s.mean_skill,
            s.mean_tech_match,
        ] {
            assert!((0.0..=1.0).contains(&v), "expected [0,1], got {v}");
        }
        assert!(s.meme_divergence >= 0.0);
        assert!(s.genetic_diversity >= 0.0);
        assert!(s.live_count >= 0.0);
    }

    #[test]
    fn phero_intensity_saturates_monotonically() {
        assert_eq!(phero_intensity(0.0), 0.0);
        assert!(phero_intensity(1.0) > phero_intensity(0.1));
        assert!(phero_intensity(100.0) <= 1.0);
        assert!(phero_intensity(-5.0) == 0.0);
    }

    #[test]
    fn dialect_hue_is_bounded_and_varies() {
        let a = [0.0_f32; 8];
        let b = [0.9_f32, 0.1, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert!((0.0..1.0).contains(&dialect_hue(&a)));
        assert!((0.0..1.0).contains(&dialect_hue(&b)));
        assert!(dialect_hue(&a) != dialect_hue(&b));
    }
}
