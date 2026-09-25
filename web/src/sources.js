// Two data sources behind one interface, so the scene never knows whether it
// is drawing a live wasm world or a recorded replay:
//
//   source.kind            'live' | 'replay'
//   source.worldSize, .biomeRes, .seaLevel, .flags, .labels (sid → name)
//   source.tick            current (possibly fractional) tick
//   source.endTick         replay length, or Infinity
//   source.advance(ticks)  move time forward; returns whole ticks actually stepped
//   source.agents()        {count, data, stride}   rows per AGENT columns (sim.js)
//   source.biomeRgba()     Uint8Array res²×4 (elevation in alpha) or null
//   source.elevation()     Float32Array res² or null (static)
//   source.terrain()       Uint8Array res² TerrainType ids or null (static)
//   source.streaks() / .trades()   {count, data} rows [x1,y1,x2,y2,hue]
//   source.sites()         {count, data} rows [sid,x,y,members]
//   source.hubs()          {count, data} rows [x,y,goods_mask]
//   source.events()        [{type, tick, sid, value, x, y}] since the last call
//   source.species()       [{id,name,count,tech_era,adopted}]
//   source.agent(id)       inspector object or null
//   source.meta()          {scenario, seed, tick, alive, state_hash, fingerprint, …}
//   source.dispose()

import { AnabiosSim, AGENT, EVENT } from "./sim.js";
import { speciesHue } from "./palette.js";

const EMPTY = { count: 0, data: new Float32Array(0) };

// ---------------------------------------------------------------------------
// Live: the wasm core
// ---------------------------------------------------------------------------
export class LiveSource {
  /** @param {AnabiosSim} sim  an instantiated module with a loaded scenario */
  constructor(sim) {
    this.kind = "live";
    this.sim = sim;
    this.worldSize = sim.worldSize();
    this.biomeRes = sim.biomeRes();
    this.seaLevel = sim.seaLevel();
    this.flags = sim.flags();
    this.labels = new Map(Object.entries(sim.species().labels).map(([k, v]) => [Number(k), v]));
    this.catalog = sim.catalog;
    this.endTick = Infinity;
    this._acc = 0;
    this.stepMs = 0;
  }
  get tick() { return this.sim.tick() + this._acc; }
  /** Accumulates fractional ticks; steps the core once a whole tick is due. */
  advance(ticks) {
    this._acc += ticks;
    const whole = Math.floor(this._acc);
    if (whole > 0) {
      const t0 = performance.now();
      this.sim.step(whole);
      this.stepMs = performance.now() - t0;
      this._acc -= whole;
    }
    return whole;
  }
  agents() { const a = this.sim.agents(); return { ...a, stride: this.sim.stride.agent }; }
  biomeRgba() { return this.sim.biomeRgba(); }
  /** Static after worldgen, so hand out a copy: a raw view would be detached
   *  by the next call that grows wasm memory. */
  elevation() { return this.sim.elevation().slice(); }
  /** TerrainType id per cell (static). */
  terrain() { return this.sim.terrain().slice(); }
  streaks() { return this.sim.streaks(); }
  trades() { return this.sim.trades(); }
  sites() { return this.sim.sites(); }
  hubs() { return this.sim.hubs(); }
  events() {
    const { count, data } = this.sim.events();
    const s = this.sim.stride.event, names = this.catalog.events, out = [];
    for (let k = 0; k < count; k++) {
      const r = data.subarray(k * s, (k + 1) * s);
      out.push({
        type: names[r[EVENT.TYPE]] ?? `event_${r[EVENT.TYPE]}`, tick: r[EVENT.TICK],
        sid: r[EVENT.SPECIES] < 0 ? null : r[EVENT.SPECIES], value: r[EVENT.VALUE], x: r[EVENT.X], y: r[EVENT.Y],
      });
    }
    return out;
  }
  species() { return this.sim.species().species; }
  agent(id) { return this.sim.agent(id); }
  meta() { return this.sim.meta(); }
  dispose() { this.sim.free(); }
}

/** Fetch + instantiate the module and load a scenario. */
export async function openLive(wasmUrl, tomlText, seed) {
  const sim = await AnabiosSim.fromUrl(wasmUrl);
  sim.load(tomlText, seed);
  return new LiveSource(sim);
}

// ---------------------------------------------------------------------------
// Replay: the `anabios-headless record` format (showcase/replay.js)
// ---------------------------------------------------------------------------
const STRIDE = 16;

export class ReplaySource {
  constructor(R) {
    if (!R || !R.frames || !R.frames.length) throw new Error("replay has no frames");
    this.kind = "replay";
    this.R = R;
    this.worldSize = R.meta.world_size || 1024;
    this.biomeRes = R.biome?.res || 0;
    this.seaLevel = 0.35;
    this.flags = 0;
    this.labels = new Map(Object.entries(R.species || {}).map(([k, v]) => [Number(k), v]));
    this.endTick = R.meta.ticks;
    this.tick = 0;
    this.frames = R.frames;
    this.events_ = R.events.slice().sort((a, b) => a.t - b.t);
    this.evPtr = 0;
    this._idMaps = new Map();
    this._grids = (R.biome?.grids || []).map((g) => ({ t: g.t, rgb: Uint8Array.from(atob(g.b64), (c) => c.charCodeAt(0)) }));
    this._rgba = this.biomeRes ? new Uint8Array(this.biomeRes * this.biomeRes * 4) : null;
    this._elev = decodeElevation(R.biome, this.biomeRes);
    this._buf = new Float32Array(0);
    this.stepMs = 0;
  }
  advance(ticks) {
    const before = Math.floor(this.tick);
    this.tick = Math.min(this.endTick, this.tick + ticks);
    if (this.tick >= this.endTick) { this.tick = 0; this.evPtr = 0; }
    return Math.floor(this.tick) - before;
  }
  seek(tick) { this.tick = Math.max(0, Math.min(this.endTick, tick)); this.evPtr = this.events_.findIndex((e) => e.t >= this.tick); if (this.evPtr < 0) this.evPtr = this.events_.length; }

  _frameIndex(tick) {
    const f = this.frames;
    let lo = 0, hi = f.length - 1, ans = 0;
    while (lo <= hi) { const m = (lo + hi) >> 1; if (f[m].t <= tick) { ans = m; lo = m + 1; } else hi = m - 1; }
    return ans;
  }
  _idMap(fi) {
    let m = this._idMaps.get(fi);
    if (m) return m;
    const f = this.frames[fi]; m = new Map();
    for (let k = 0; k < f.id.length; k++) m.set(f.id[k], k);
    this._idMaps.set(fi, m);
    if (this._idMaps.size > 8) this._idMaps.delete(this._idMaps.keys().next().value);
    return m;
  }
  /** Interpolate the bracketing frames by agent id into AGENT-layout rows. */
  agents() {
    const ai = this._frameIndex(this.tick), bi = Math.min(this.frames.length - 1, ai + 1);
    const A = this.frames[ai], B = this.frames[bi];
    const f = Math.min(1, Math.max(0, (this.tick - A.t) / ((B.t - A.t) || 1)));
    const bmap = this._idMap(bi), W = this.worldSize;
    if (this._buf.length < A.id.length * STRIDE) this._buf = new Float32Array(A.id.length * STRIDE);
    const out = this._buf;
    let n = 0;
    for (let k = 0; k < A.id.length; k++) {
      const id = A.id[k], bk = bmap.get(id);
      if (bk === undefined) continue; // died before B: skip (the ghost is a 2D-player flourish)
      const ax = A.x[k], ay = A.y[k], bx = B.x[bk], by = B.y[bk];
      let x = ax, y = ay, rot = 0;
      if (Math.abs(bx - ax) < W * 0.4 && Math.abs(by - ay) < W * 0.4) {
        x = ax + (bx - ax) * f; y = ay + (by - ay) * f;
        if (bx !== ax || by !== ay) rot = Math.atan2(by - ay, bx - ax);
      }
      const sp = A.sp[k], diet = A.d[k] / 255;
      const o = n * STRIDE;
      out[o + AGENT.ID] = id; out[o + AGENT.X] = x; out[o + AGENT.Y] = y; out[o + AGENT.ROT] = rot;
      out[o + AGENT.SIZE] = 1.0 + diet * 0.6; out[o + AGENT.DIET] = diet;
      out[o + AGENT.HUE] = speciesHue(sp); out[o + AGENT.SAT] = 0.55 + diet * 0.3; out[o + AGENT.VAL] = 0.85;
      out[o + AGENT.DIALECT_HUE] = 0; out[o + AGENT.ENERGY] = 0; out[o + AGENT.SPECIES] = sp;
      out[o + AGENT.MOOD] = 0; out[o + AGENT.FLAGS] = 0; out[o + AGENT.AROUSAL] = 0; out[o + AGENT.INFECTION] = 0;
      n++;
    }
    this._lastFrame = A; this._lastFrac = f; this._lastIndex = ai;
    return { count: n, data: out, stride: STRIDE };
  }
  /** Cross-fade the bracketing biome keyframes into RGBA (alpha = elevation when known). */
  biomeRgba() {
    if (!this._rgba || !this._grids.length) return null;
    let i = 0;
    while (i < this._grids.length - 1 && this._grids[i + 1].t <= this.tick) i++;
    const A = this._grids[i], B = this._grids[i + 1];
    const f = B ? Math.min(1, Math.max(0, (this.tick - A.t) / ((B.t - A.t) || 1))) : 0;
    const out = this._rgba, n = this.biomeRes * this.biomeRes;
    for (let k = 0; k < n; k++) {
      const a = k * 3, o = k * 4;
      if (B) {
        out[o] = A.rgb[a] + (B.rgb[a] - A.rgb[a]) * f;
        out[o + 1] = A.rgb[a + 1] + (B.rgb[a + 1] - A.rgb[a + 1]) * f;
        out[o + 2] = A.rgb[a + 2] + (B.rgb[a + 2] - A.rgb[a + 2]) * f;
      } else { out[o] = A.rgb[a]; out[o + 1] = A.rgb[a + 1]; out[o + 2] = A.rgb[a + 2]; }
      out[o + 3] = this._elev ? this._elev[k] * 255 : 255;
    }
    return out;
  }
  elevation() { return this._elev; }
  /** Recordings carry colours only; the terrain classifies them itself. */
  terrain() { return null; }
  _segments(arr) {
    if (!arr || !arr.length || this._lastFrac > 0.5) return EMPTY; // per-tick streaks fade within the interval
    const n = arr.length >> 2, data = new Float32Array(n * 5);
    for (let i = 0; i < n; i++) {
      data[i * 5] = arr[i * 4]; data[i * 5 + 1] = arr[i * 4 + 1]; data[i * 5 + 2] = arr[i * 4 + 2]; data[i * 5 + 3] = arr[i * 4 + 3];
      data[i * 5 + 4] = 0.08;
    }
    return { count: n, data };
  }
  streaks() { return this._segments(this._lastFrame?.st); }
  trades() { const s = this._segments(this._lastFrame?.tr); for (let i = 0; i < s.count; i++) s.data[i * 5 + 4] = 0.58; return s; }
  sites() {
    const sf = this.R.sites?.[this._lastIndex ?? 0];
    if (!sf || !sf.sid.length) return EMPTY;
    const data = new Float32Array(sf.sid.length * 4);
    for (let k = 0; k < sf.sid.length; k++) { data[k * 4] = sf.sid[k]; data[k * 4 + 1] = sf.x[k]; data[k * 4 + 2] = sf.y[k]; data[k * 4 + 3] = sf.n[k]; }
    return { count: sf.sid.length, data };
  }
  hubs() { return EMPTY; }
  events() {
    const out = [];
    while (this.evPtr < this.events_.length && this.events_[this.evPtr].t <= this.tick) {
      const e = this.events_[this.evPtr++];
      out.push({ type: e.type, tick: e.t, sid: e.sid ?? null, value: e.v, x: e.x, y: e.y });
    }
    return out;
  }
  species() {
    const A = this._lastFrame || this.frames[0], counts = new Map();
    for (const sp of A.sp) counts.set(sp, (counts.get(sp) || 0) + 1);
    const stride = Math.max(1, this.R.meta.stride || 1);
    return [...counts].sort((a, b) => a[0] - b[0]).map(([id, c]) => ({
      id, name: this.labels.get(id) || `species ${id}`, count: c * stride, tech_era: 0, adopted: [],
    }));
  }
  agent(id) {
    const A = this._lastFrame; if (!A) return null;
    const k = A.id.indexOf(id); if (k < 0) return null;
    const sid = A.sp[k];
    return { id, x: A.x[k], y: A.y[k], species_id: sid, species: this.labels.get(sid) || `species ${sid}`, diet_carnivory: A.d[k] / 255, replay: true };
  }
  meta() {
    const m = this.R.meta;
    return { scenario: m.scenario, seed: m.seed, tick: Math.floor(this.tick), alive: (this._lastFrame?.id.length || 0) * Math.max(1, m.stride || 1), state_hash: m.state_hash, ticks: m.ticks, sample: m.sample, stride: m.stride };
  }
  dispose() {}
}

/** Recorder may carry `elev` (base64 u8 per cell, res²) — absent in older files. */
function decodeElevation(biome, res) {
  if (!biome?.elev || !res) return null;
  const bytes = Uint8Array.from(atob(biome.elev), (c) => c.charCodeAt(0));
  if (bytes.length !== res * res) return null;
  return Float32Array.from(bytes, (b) => b / 255);
}

/** Load a replay: a `.js` file assigning `window.ANABIOS_REPLAY`, or raw `.json`. */
export async function openReplay(url) {
  if (url.endsWith(".json")) {
    const r = await fetch(url);
    if (!r.ok) throw new Error(`fetch ${url}: HTTP ${r.status}`);
    return new ReplaySource(await r.json());
  }
  await new Promise((ok, fail) => {
    const s = document.createElement("script");
    s.src = url; s.onload = ok; s.onerror = () => fail(new Error(`could not load ${url}`));
    document.head.appendChild(s);
  });
  const R = window.ANABIOS_REPLAY;
  if (!R) throw new Error(`${url} did not define window.ANABIOS_REPLAY`);
  return new ReplaySource(R);
}
