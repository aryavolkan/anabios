//! anabios-wasm — the WebAssembly bridge behind the three.js frontend (`web/`).
//!
//! Compiled with `--target wasm32-unknown-unknown` this crate becomes a plain
//! `.wasm` module with **no imports** and a hand-rolled C ABI: opaque `Sim`
//! handles, fixed-stride `f32`/`u8` buffers the JavaScript side views directly
//! in linear memory, and JSON strings for the sparse, irregular data (agent
//! inspector, species table, static catalogs). No `wasm-bindgen`: the surface
//! is small enough that a 200-line loader (`web/src/sim.js`) is simpler than a
//! generated glue layer, and the module stays buildable with nothing but
//! `rustup target add wasm32-unknown-unknown`.
//!
//! The same source builds natively as an `rlib`, which is how the workspace
//! gates (clippy, rustdoc, `cargo test --lib`) cover the pure builders in
//! [`view`].
//!
//! # Calling convention
//!
//! * Every export that reads the world takes a `*mut Sim` from [`sim_new`].
//! * Buffer exports come in pairs: `sim_x(sim) -> count` refreshes an internal
//!   buffer and returns the element count; `sim_x_ptr(sim) -> *const f32`
//!   points at it. Pointers are valid until the next call into the module (a
//!   refill may reallocate; `memory.grow` may move the whole heap), so JS must
//!   re-create its typed-array view after every call.
//! * Strings cross as `(ptr, len)` UTF-8. Inputs are allocated with
//!   [`anabios_alloc`], written by JS, and freed with [`anabios_free`].
//!   Outputs live in a per-`Sim` scratch `String` (`sim_json` → `sim_json_ptr`).
//! * Failures set a thread-local message readable via [`sim_error_ptr`] /
//!   [`sim_error_len`]. A Rust panic inside the module writes the panic
//!   message there too before trapping (`unreachable`), so the browser can
//!   show *why* the instance died; the instance must then be recreated.

#![allow(clippy::missing_safety_doc)] // Every export documents its contract in the module docs above.

pub mod view;

use std::cell::RefCell;
use std::collections::BTreeMap;

use anabios_core::scenario::Scenario;
use anabios_core::tick::step;
use anabios_core::World;

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn set_error(msg: impl Into<String>) {
    LAST_ERROR.with(|e| *e.borrow_mut() = msg.into());
}

/// Route panics into `LAST_ERROR` so the JS side can read the message after
/// the trap. Installed once, on the first `sim_new`.
fn install_panic_hook() {
    use std::sync::Once;
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let msg = match info.payload().downcast_ref::<&str>() {
                Some(s) => s.to_string(),
                None => match info.payload().downcast_ref::<String>() {
                    Some(s) => s.clone(),
                    None => "panic".to_string(),
                },
            };
            let loc = info
                .location()
                .map(|l| format!(" at {}:{}", l.file(), l.line()))
                .unwrap_or_default();
            set_error(format!("panic: {msg}{loc}"));
        }));
    });
}

/// One in-module simulation plus the reusable view buffers. Opaque to JS.
pub struct Sim {
    world: Option<World>,
    scenario_name: String,
    labels: BTreeMap<u32, String>,
    agents: Vec<f32>,
    biome_rgba: Vec<u8>,
    elevation: Vec<f32>,
    terrain: Vec<u8>,
    streaks: Vec<f32>,
    trades: Vec<f32>,
    sites: Vec<f32>,
    hubs: Vec<f32>,
    /// Codex events drained since the last `sim_events` read.
    pending_events: Vec<f32>,
    /// Snapshot of `pending_events` handed to JS by the last `sim_events`.
    events_out: Vec<f32>,
    json: String,
}

impl Sim {
    fn new() -> Self {
        Self {
            world: None,
            scenario_name: String::new(),
            labels: BTreeMap::new(),
            agents: Vec::new(),
            biome_rgba: Vec::new(),
            elevation: Vec::new(),
            terrain: Vec::new(),
            streaks: Vec::new(),
            trades: Vec::new(),
            sites: Vec::new(),
            hubs: Vec::new(),
            pending_events: Vec::new(),
            events_out: Vec::new(),
            json: String::new(),
        }
    }

    /// Parse and instantiate a scenario, optionally overriding its seed.
    /// Returns an error string on parse failure (the previous world, if any,
    /// is kept).
    pub fn load(&mut self, toml_text: &str, seed: Option<u64>) -> Result<(), String> {
        let mut s = Scenario::parse_toml(toml_text).map_err(|e| e.to_string())?;
        if let Some(seed) = seed {
            s.seed = seed;
        }
        self.labels = view::species_labels(&s);
        self.scenario_name = s.name.clone();
        self.world = Some(s.instantiate());
        self.pending_events.clear();
        self.events_out.clear();
        Ok(())
    }

    /// Advance `n` ticks, draining codex events into the pending buffer.
    pub fn step(&mut self, n: u32) {
        let Some(w) = self.world.as_mut() else { return };
        for _ in 0..n {
            step(w);
            for ev in w.codex.drain_events() {
                view::push_event(&ev, &mut self.pending_events);
            }
        }
    }

    /// Refresh the agent buffer; returns the alive count.
    pub fn agents(&mut self) -> usize {
        match self.world.as_ref() {
            Some(w) => view::fill_agents(w, &mut self.agents),
            None => 0,
        }
    }

    /// Hand the pending events to the caller and start a fresh batch.
    /// Returns the event count.
    pub fn take_events(&mut self) -> usize {
        std::mem::swap(&mut self.pending_events, &mut self.events_out);
        self.pending_events.clear();
        self.events_out.len() / view::EVENT_STRIDE
    }

    fn world(&self) -> Option<&World> {
        self.world.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Memory helpers
// ---------------------------------------------------------------------------

/// Allocate `len` bytes for the caller to fill (scenario text). Pair with
/// [`anabios_free`] using the same `len`.
#[unsafe(no_mangle)]
pub extern "C" fn anabios_alloc(len: u32) -> *mut u8 {
    let mut v: Vec<u8> = Vec::with_capacity(len as usize);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Free a buffer from [`anabios_alloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn anabios_free(ptr: *mut u8, len: u32) {
    if !ptr.is_null() {
        drop(unsafe { Vec::from_raw_parts(ptr, 0, len as usize) });
    }
}

/// Pointer to the last error / panic message (UTF-8, not NUL-terminated).
#[unsafe(no_mangle)]
pub extern "C" fn sim_error_ptr() -> *const u8 {
    LAST_ERROR.with(|e| e.borrow().as_ptr())
}

/// Byte length of the last error / panic message.
#[unsafe(no_mangle)]
pub extern "C" fn sim_error_len() -> u32 {
    LAST_ERROR.with(|e| e.borrow().len() as u32)
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// Create an empty simulation handle. Free with [`sim_free`].
#[unsafe(no_mangle)]
pub extern "C" fn sim_new() -> *mut Sim {
    install_panic_hook();
    Box::into_raw(Box::new(Sim::new()))
}

/// Destroy a handle from [`sim_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_free(sim: *mut Sim) {
    if !sim.is_null() {
        drop(unsafe { Box::from_raw(sim) });
    }
}

/// Load a TOML scenario from `(ptr, len)` UTF-8 bytes. When `override_seed`
/// is non-zero the seed is `seed_lo | seed_hi << 32` instead of the file's.
/// Returns 1 on success, 0 on failure (message via [`sim_error_ptr`]).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_load(
    sim: *mut Sim,
    ptr: *const u8,
    len: u32,
    seed_lo: u32,
    seed_hi: u32,
    override_seed: u32,
) -> u32 {
    let sim = unsafe { &mut *sim };
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => {
            set_error(format!("scenario text is not UTF-8: {e}"));
            return 0;
        }
    };
    let seed = (override_seed != 0).then_some((seed_lo as u64) | ((seed_hi as u64) << 32));
    match sim.load(text, seed) {
        Ok(()) => 1,
        Err(e) => {
            set_error(format!("scenario parse failed: {e}"));
            0
        }
    }
}

/// Advance the world by `n` ticks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_step(sim: *mut Sim, n: u32) {
    unsafe { &mut *sim }.step(n);
}

// ---------------------------------------------------------------------------
// Scalars
// ---------------------------------------------------------------------------

/// Current tick (as `f64`; ticks stay far below 2^53).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_tick(sim: *const Sim) -> f64 {
    unsafe { &*sim }.world().map(|w| w.tick as f64).unwrap_or(0.0)
}

/// Alive agent count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_alive(sim: *const Sim) -> u32 {
    unsafe { &*sim }.world().map(|w| w.agents.live_count()).unwrap_or(0)
}

/// World extent per axis (torus side).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_world_size(sim: *const Sim) -> f32 {
    unsafe { &*sim }.world().map(|w| w.world_size).unwrap_or(0.0)
}

/// Biome grid resolution per axis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_biome_res(sim: *const Sim) -> u32 {
    unsafe { &*sim }.world().map(|w| w.biome_res as u32).unwrap_or(0)
}

/// Viewer sea level in normalized elevation units (see [`view::sea_level`]).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_sea_level(sim: *const Sim) -> f32 {
    unsafe { &*sim }.world().map(view::sea_level).unwrap_or(0.0)
}

/// Enabled-subsystem bitmask (see the `FLAG_*` consts in [`view`]).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_flags(sim: *const Sim) -> u32 {
    unsafe { &*sim }.world().map(view::world_flags).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Buffers (count + pointer pairs)
// ---------------------------------------------------------------------------

/// Refresh the agent buffer ([`view::AGENT_STRIDE`] floats each). Returns count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_agents(sim: *mut Sim) -> u32 {
    unsafe { &mut *sim }.agents() as u32
}

/// Pointer to the agent buffer filled by [`sim_agents`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_agents_ptr(sim: *const Sim) -> *const f32 {
    unsafe { &*sim }.agents.as_ptr()
}

/// Refresh the biome RGBA8 buffer (`res² × 4` bytes). Returns the byte count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_biome_rgba(sim: *mut Sim) -> u32 {
    let sim = unsafe { &mut *sim };
    if let Some(w) = sim.world.as_ref() {
        view::fill_biome_rgba(w, &mut sim.biome_rgba);
    } else {
        sim.biome_rgba.clear();
    }
    sim.biome_rgba.len() as u32
}

/// Pointer to the biome RGBA8 buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_biome_rgba_ptr(sim: *const Sim) -> *const u8 {
    unsafe { &*sim }.biome_rgba.as_ptr()
}

/// Refresh the elevation buffer (`res²` floats in `[0,1]`). Returns count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_elevation(sim: *mut Sim) -> u32 {
    let sim = unsafe { &mut *sim };
    if let Some(w) = sim.world.as_ref() {
        view::fill_elevation(w, &mut sim.elevation);
    } else {
        sim.elevation.clear();
    }
    sim.elevation.len() as u32
}

/// Pointer to the elevation buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_elevation_ptr(sim: *const Sim) -> *const f32 {
    unsafe { &*sim }.elevation.as_ptr()
}

/// Refresh the terrain-id buffer (`res²` bytes). Returns count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_terrain(sim: *mut Sim) -> u32 {
    let sim = unsafe { &mut *sim };
    if let Some(w) = sim.world.as_ref() {
        view::fill_terrain(w, &mut sim.terrain);
    } else {
        sim.terrain.clear();
    }
    sim.terrain.len() as u32
}

/// Pointer to the terrain-id buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_terrain_ptr(sim: *const Sim) -> *const u8 {
    unsafe { &*sim }.terrain.as_ptr()
}

/// Refresh this tick's combat streaks ([`view::SEGMENT_STRIDE`] floats each).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_streaks(sim: *mut Sim) -> u32 {
    let sim = unsafe { &mut *sim };
    match sim.world.as_ref() {
        Some(w) => view::fill_segments(&w.combat_streaks, &mut sim.streaks) as u32,
        None => 0,
    }
}

/// Pointer to the combat-streak buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_streaks_ptr(sim: *const Sim) -> *const f32 {
    unsafe { &*sim }.streaks.as_ptr()
}

/// Refresh this tick's trade routes ([`view::SEGMENT_STRIDE`] floats each).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_trades(sim: *mut Sim) -> u32 {
    let sim = unsafe { &mut *sim };
    match sim.world.as_ref() {
        Some(w) => view::fill_segments(&w.trade_routes, &mut sim.trades) as u32,
        None => 0,
    }
}

/// Pointer to the trade-route buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_trades_ptr(sim: *const Sim) -> *const f32 {
    unsafe { &*sim }.trades.as_ptr()
}

/// Refresh the settlement-site buffer ([`view::SITE_STRIDE`] floats each).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_sites(sim: *mut Sim) -> u32 {
    let sim = unsafe { &mut *sim };
    match sim.world.as_ref() {
        Some(w) => view::fill_sites(w, &mut sim.sites) as u32,
        None => 0,
    }
}

/// Pointer to the settlement-site buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_sites_ptr(sim: *const Sim) -> *const f32 {
    unsafe { &*sim }.sites.as_ptr()
}

/// Refresh the trade-hub buffer ([`view::HUB_STRIDE`] floats each).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_hubs(sim: *mut Sim) -> u32 {
    let sim = unsafe { &mut *sim };
    match sim.world.as_ref() {
        Some(w) => view::fill_hubs(w, &mut sim.hubs) as u32,
        None => 0,
    }
}

/// Pointer to the trade-hub buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_hubs_ptr(sim: *const Sim) -> *const f32 {
    unsafe { &*sim }.hubs.as_ptr()
}

/// Hand over the codex events fired since the previous call
/// ([`view::EVENT_STRIDE`] floats each). Returns the event count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_events(sim: *mut Sim) -> u32 {
    unsafe { &mut *sim }.take_events() as u32
}

/// Pointer to the event buffer from the last [`sim_events`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_events_ptr(sim: *const Sim) -> *const f32 {
    unsafe { &*sim }.events_out.as_ptr()
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

/// `kind` selector for [`sim_json`]: the static catalog (event names,
/// invention tree, moods, genome slots, goods).
pub const JSON_CATALOG: u32 = 0;
/// `kind` selector for [`sim_json`]: per-species table + archetype labels.
pub const JSON_SPECIES: u32 = 1;
/// `kind` selector for [`sim_json`]: one agent's inspector view (`arg` = id).
pub const JSON_AGENT: u32 = 2;
/// `kind` selector for [`sim_json`]: run metadata + `state_hash` receipt.
pub const JSON_META: u32 = 3;

/// Build a JSON document into the handle's scratch string. Returns its byte
/// length; read it via [`sim_json_ptr`]. Unknown kinds yield `{}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_json(sim: *mut Sim, kind: u32, arg: u32) -> u32 {
    let sim = unsafe { &mut *sim };
    sim.json = match (kind, sim.world.as_ref()) {
        (JSON_CATALOG, _) => view::catalog_json(),
        (JSON_SPECIES, Some(w)) => view::species_json(w, &sim.labels),
        (JSON_AGENT, Some(w)) => view::agent_json(w, arg, &sim.labels),
        (JSON_META, Some(w)) => view::meta_json(w, &sim.scenario_name),
        _ => "{}".to_string(),
    };
    sim.json.len() as u32
}

/// Pointer to the JSON scratch string from the last [`sim_json`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sim_json_ptr(sim: *const Sim) -> *const u8 {
    unsafe { &*sim }.json.as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anabios_core::snapshot::state_hash;

    fn scenario(name: &str) -> String {
        let path = format!("{}/../../scenarios/{name}.toml", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(path).expect("scenario")
    }

    /// Driving the world through the bridge must reproduce the plain
    /// `step` loop's state hash — the same receipt the headless CLI prints.
    #[test]
    fn bridge_matches_plain_step_loop() {
        let text = scenario("predator-prey");
        let mut sim = Sim::new();
        sim.load(&text, Some(7)).unwrap();
        sim.step(300);
        let mut plain = Scenario::parse_toml(&text).unwrap();
        plain.seed = 7;
        let mut w = plain.instantiate();
        for _ in 0..300 {
            step(&mut w);
            for _ in w.codex.drain_events() {}
        }
        assert_eq!(state_hash(sim.world().unwrap()), state_hash(&w));
        assert_eq!(sim.world().unwrap().tick, 300);
    }

    #[test]
    fn events_drain_once() {
        let mut sim = Sim::new();
        sim.load(&scenario("predator-prey"), Some(0)).unwrap();
        sim.step(1500);
        let first = sim.take_events();
        assert!(first > 0, "predator-prey fires codex events within 1500 ticks");
        assert_eq!(sim.events_out.len(), first * view::EVENT_STRIDE);
        assert_eq!(sim.take_events(), 0);
    }

    #[test]
    fn load_rejects_bad_toml_and_keeps_old_world() {
        let mut sim = Sim::new();
        sim.load(&scenario("minimal"), None).unwrap();
        assert!(sim.load("name = ", None).is_err());
        assert!(sim.world().is_some());
    }

    #[test]
    fn c_abi_round_trip() {
        let text = scenario("minimal");
        let sim = sim_new();
        unsafe {
            let ptr = anabios_alloc(text.len() as u32);
            std::ptr::copy_nonoverlapping(text.as_ptr(), ptr, text.len());
            assert_eq!(sim_load(sim, ptr, text.len() as u32, 5, 0, 1), 1);
            anabios_free(ptr, text.len() as u32);
            assert_eq!(sim_seed_for_test(sim), 5);
            sim_step(sim, 10);
            assert_eq!(sim_tick(sim), 10.0);
            let n = sim_agents(sim);
            assert_eq!(n, sim_alive(sim));
            assert!(!sim_agents_ptr(sim).is_null());
            assert_eq!(sim_biome_res(sim), (*sim).world().unwrap().biome_res as u32);
            assert_eq!(sim_biome_rgba(sim), sim_biome_res(sim) * sim_biome_res(sim) * 4);
            let len = sim_json(sim, JSON_META, 0);
            let s = std::slice::from_raw_parts(sim_json_ptr(sim), len as usize);
            let v: serde_json::Value = serde_json::from_slice(s).unwrap();
            assert_eq!(v["tick"], 10);
            assert_eq!(v["seed"], 5);
            // A parse failure reports through the error channel.
            let bad = b"not = [toml";
            let ptr = anabios_alloc(bad.len() as u32);
            std::ptr::copy_nonoverlapping(bad.as_ptr(), ptr, bad.len());
            assert_eq!(sim_load(sim, ptr, bad.len() as u32, 0, 0, 0), 0);
            anabios_free(ptr, bad.len() as u32);
            let err = std::slice::from_raw_parts(sim_error_ptr(), sim_error_len() as usize);
            assert!(std::str::from_utf8(err).unwrap().contains("parse failed"));
            sim_free(sim);
        }
    }

    unsafe fn sim_seed_for_test(sim: *const Sim) -> u64 {
        unsafe { &*sim }.world().unwrap().seed
    }
}
