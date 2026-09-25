// Loader for the `anabios-wasm` module (crates/anabios-wasm): a thin class
// over the hand-rolled C ABI. Runs unchanged in the browser and in node (the
// smoke test), so it never touches `fetch` or the DOM itself — callers hand it
// a compiled module or raw bytes.
//
// Buffer views returned here alias wasm linear memory and are valid only
// until the next call into the module (a refill may reallocate, and
// `memory.grow` detaches every existing ArrayBuffer view). Copy what you keep.

/** Column indices inside one agent row (see `view::AGENT_STRIDE` in Rust). */
export const AGENT = Object.freeze({
  ID: 0, X: 1, Y: 2, ROT: 3, SIZE: 4, DIET: 5, HUE: 6, SAT: 7, VAL: 8,
  DIALECT_HUE: 9, ENERGY: 10, SPECIES: 11, MOOD: 12, FLAGS: 13, AROUSAL: 14, INFECTION: 15,
});
/** Bits of an agent row's FLAGS column. */
export const AGENT_FLAG = Object.freeze({ LIVESTOCK: 1, ASLEEP: 2, MALE: 4 });
/** Bits of `flags()` — which opt-in subsystems the loaded world runs. */
export const WORLD_FLAG = Object.freeze({
  INVENTIONS: 1 << 0, AFFECT: 1 << 1, DISEASE: 1 << 2, DOMESTICATION: 1 << 3,
  RESOURCES: 1 << 4, DIMORPHISM: 1 << 5, BASIC_NEEDS: 1 << 6, COGNITION: 1 << 7,
  SETTLEMENT: 1 << 8, WAR: 1 << 9,
});
/** Column indices inside one codex event row. */
export const EVENT = Object.freeze({ TYPE: 0, TICK: 1, SPECIES: 2, VALUE: 3, X: 4, Y: 5 });

const JSON_CATALOG = 0, JSON_SPECIES = 1, JSON_AGENT = 2, JSON_META = 3;

const enc = new TextEncoder();
const dec = new TextDecoder();

export class AnabiosSim {
  /** Instantiate from raw `.wasm` bytes (node, or a pre-fetched ArrayBuffer). */
  static async fromBytes(bytes) {
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return new AnabiosSim(instance);
  }

  /** Instantiate from a URL (browser). Streams when the server sets the wasm MIME type. */
  static async fromUrl(url) {
    const resp = await fetch(url);
    if (!resp.ok) throw new Error(`fetch ${url}: HTTP ${resp.status}`);
    let result;
    if (WebAssembly.instantiateStreaming && resp.headers.get("content-type")?.includes("application/wasm")) {
      result = await WebAssembly.instantiateStreaming(resp, {});
    } else {
      result = await WebAssembly.instantiate(await resp.arrayBuffer(), {});
    }
    return new AnabiosSim(result.instance);
  }

  constructor(instance) {
    this.ex = instance.exports;
    this.h = this.ex.sim_new();
    /** Static catalog: event names, invention tree, moods, genome slots, strides. */
    this.catalog = JSON.parse(this._json(JSON_CATALOG, 0));
    this.stride = {
      agent: this.catalog.agent_stride,
      segment: this.catalog.segment_stride,
      site: this.catalog.site_stride,
      hub: this.catalog.hub_stride,
      event: this.catalog.event_stride,
    };
  }

  // -- memory views (always re-created: the buffer may have been detached) --
  _f32(ptr, n) { return new Float32Array(this.ex.memory.buffer, ptr, n); }
  _u8(ptr, n) { return new Uint8Array(this.ex.memory.buffer, ptr, n); }
  _str(ptr, len) { return dec.decode(new Uint8Array(this.ex.memory.buffer, ptr, len)); }

  /** Last error or panic message recorded by the module ("" when none). */
  error() { return this._str(this.ex.sim_error_ptr(), this.ex.sim_error_len()); }

  /** Run `fn`, converting a wasm trap into an Error carrying the panic message. */
  _guard(fn) {
    try {
      return fn();
    } catch (e) {
      const msg = this.error();
      throw new Error(msg ? `${msg} (${e.message})` : e.message);
    }
  }

  /**
   * Parse + instantiate a TOML scenario. `seed` (number or BigInt) overrides
   * the file's seed when given. Throws with the parser's message on failure.
   */
  load(tomlText, seed) {
    const bytes = enc.encode(tomlText);
    const ptr = this.ex.anabios_alloc(bytes.length);
    this._u8(ptr, bytes.length).set(bytes);
    let lo = 0, hi = 0, override = 0;
    if (seed !== undefined && seed !== null && seed !== "") {
      const s = BigInt(seed);
      lo = Number(s & 0xffffffffn);
      hi = Number((s >> 32n) & 0xffffffffn);
      override = 1;
    }
    const ok = this._guard(() => this.ex.sim_load(this.h, ptr, bytes.length, lo, hi, override));
    this.ex.anabios_free(ptr, bytes.length);
    if (!ok) throw new Error(this.error() || "scenario load failed");
  }

  /** Advance the world `n` ticks. */
  step(n) { this._guard(() => this.ex.sim_step(this.h, n)); }

  tick() { return this.ex.sim_tick(this.h); }
  alive() { return this.ex.sim_alive(this.h); }
  worldSize() { return this.ex.sim_world_size(this.h); }
  biomeRes() { return this.ex.sim_biome_res(this.h); }
  seaLevel() { return this.ex.sim_sea_level(this.h); }
  flags() { return this.ex.sim_flags(this.h); }

  /** `{count, data}` — `data` is a Float32Array of `count * stride.agent`. */
  agents() {
    const count = this.ex.sim_agents(this.h);
    return { count, data: this._f32(this.ex.sim_agents_ptr(this.h), count * this.stride.agent) };
  }
  /** RGBA8 bytes, `res*res*4`, row-major (row = y). Elevation rides in alpha. */
  biomeRgba() {
    const n = this.ex.sim_biome_rgba(this.h);
    return this._u8(this.ex.sim_biome_rgba_ptr(this.h), n);
  }
  /** Elevation floats in [0,1], `res*res`, row-major. */
  elevation() {
    const n = this.ex.sim_elevation(this.h);
    return this._f32(this.ex.sim_elevation_ptr(this.h), n);
  }
  /** TerrainType ids (0 water … 8 tundra), `res*res`. */
  terrain() {
    const n = this.ex.sim_terrain(this.h);
    return this._u8(this.ex.sim_terrain_ptr(this.h), n);
  }
  /** This tick's combat streaks: `{count, data}` rows of `[x1,y1,x2,y2,hue]`. */
  streaks() {
    const count = this.ex.sim_streaks(this.h);
    return { count, data: this._f32(this.ex.sim_streaks_ptr(this.h), count * this.stride.segment) };
  }
  /** This tick's trade routes, same layout as `streaks()`. */
  trades() {
    const count = this.ex.sim_trades(this.h);
    return { count, data: this._f32(this.ex.sim_trades_ptr(this.h), count * this.stride.segment) };
  }
  /** Settlement sites: rows of `[species_id, x, y, members]`. */
  sites() {
    const count = this.ex.sim_sites(this.h);
    return { count, data: this._f32(this.ex.sim_sites_ptr(this.h), count * this.stride.site) };
  }
  /** Fixed trade hubs: rows of `[x, y, goods_mask]`. */
  hubs() {
    const count = this.ex.sim_hubs(this.h);
    return { count, data: this._f32(this.ex.sim_hubs_ptr(this.h), count * this.stride.hub) };
  }
  /** Codex events fired since the previous call: rows of `[type, tick, sid, value, x, y]`. */
  events() {
    const count = this.ex.sim_events(this.h);
    return { count, data: this._f32(this.ex.sim_events_ptr(this.h), count * this.stride.event) };
  }

  _json(kind, arg) {
    const len = this._guard(() => this.ex.sim_json(this.h, kind, arg));
    return this._str(this.ex.sim_json_ptr(this.h), len);
  }
  /** `{species: [{id,name,count,mean_energy,tech_era,adopted}], labels}` */
  species() { return JSON.parse(this._json(JSON_SPECIES, 0)); }
  /** Inspector view of one agent, or `null` when it is dead. */
  agent(id) {
    const v = JSON.parse(this._json(JSON_AGENT, id >>> 0));
    return v.id === undefined ? null : v;
  }
  /** Run metadata incl. the `state_hash` determinism receipt. */
  meta() { return JSON.parse(this._json(JSON_META, 0)); }

  /** Release the handle. The instance can't be reused afterwards. */
  free() {
    if (this.h) this.ex.sim_free(this.h);
    this.h = 0;
  }
}
