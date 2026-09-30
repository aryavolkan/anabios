// anabios atlas — three.js frontend entry point.
//
// One render loop drives either a live wasm world (LiveSource) or a recorded
// replay (ReplaySource). Each frame: advance time by the chosen speed, read the
// flat buffers, push them through the layers, and refresh the HUD on a cadence.

import * as THREE from "three";
import { createStage } from "./scene.js";
import { Terrain } from "./terrain.js";
import { Agents, Segments, Villages, Hubs, EventFx, COLOR_MODES, Birds } from "./layers.js";
import { Particles, KIND } from "./particles.js";
import { openLive, openReplay } from "./sources.js";
import { kindOf, KIND_CSS, KIND_COLOR, TERRAIN, MOOD_COLORS, cssHex, hsv, speciesHue } from "./palette.js";
import { WORLD_FLAG } from "./sim.js";

const $ = (id) => document.getElementById(id);
const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const params = new URLSearchParams(location.search);

// ---------------------------------------------------------------------------
// Stage + layers (built once; the world is swapped underneath them)
// ---------------------------------------------------------------------------
const stage = createStage($("view"));
/** What the HUD covers, for `stage.frame()`: the view rail on the left (unless
 *  collapsed, or the window is too narrow to give up its width — then the
 *  plate may pass under it) and the transport buttons along the bottom. The
 *  corner text (brand, readout, codex) is left to overlap the plate. */
stage.frameInsets = () => {
  const m = 16;
  if (document.body.classList.contains("hide-hud")) return { top: m, right: m, bottom: m, left: m };
  const rail = $("rail"), railRight = rail.getBoundingClientRect().right + m;
  const left = !rail.classList.contains("collapsed") && railRight <= window.innerWidth * 0.4 ? railRight : m;
  return { top: m, right: m, bottom: window.innerHeight - document.querySelector(".transport").getBoundingClientRect().top + m, left };
};
/** The narrow (≤860px) layout stacks the codex feed and the agent card on the transport,
 *  whose height depends on how its buttons and hint wrap: publish it for the stylesheet. */
new ResizeObserver(([e]) => document.documentElement.style.setProperty("--transport-h", `${e.target.offsetHeight}px`)).observe(document.querySelector(".transport"));
const layers = {
  agents: new Agents(),
  streaks: new Segments({ life: 14, sat: 0.75, additive: true, lift: 1.4 }),
  trades: new Segments({ life: 24, sat: 0.55, additive: false, lift: 0.9 }),
  villages: new Villages(),
  hubs: new Hubs(),
  fx: new EventFx(),
  particles: new Particles(),
  birds: new Birds(),
};
const world = new THREE.Group();
stage.scene.add(world, ...layers.agents.meshes, layers.agents.marker, layers.streaks.lines, layers.trades.lines,
  layers.villages.mesh, layers.hubs.mesh, layers.fx.group, layers.particles.group, layers.birds.mesh);
layers.fx.enabled = !reduceMotion;
layers.particles.enabled = !reduceMotion;

const state = {
  source: null,
  terrain: null,
  manifest: { scenarios: [], replays: [] },
  speed: Number(params.get("speed")) || 1,          // ticks per 60 Hz frame
  paused: params.get("paused") === "1",
  colorMode: params.get("color") || "species",
  /** Day cycle: on for live viewing, off under capture unless `&day=1` (gallery stills stay at noon). */
  dayCycle: params.has("day") ? params.get("day") === "1" : !params.has("capture"),
  DAY_TICKS: 1500,
  /** Event tour (V): fly to fresh codex events while slowly orbiting. */
  tour: false,
  lastFlyAt: 0,
  /** Capture harness (see web/scripts/capture.mjs): `?tick=N&cam=…&inspect=…`. */
  shot: {
    tick: params.has("tick") ? Math.max(0, Number(params.get("tick"))) : null,
    cam: params.get("cam"),
    inspect: params.get("inspect"),
    stage: 0,       // 0 idle · 1 waiting for the first frame at the target tick · 2 camera/inspect applied
  },
  ready: false,     // true once the requested tick, camera and selection are all on screen
  fastForwarding: false,
  lastEventLoc: null,
  selected: -1,
  follow: false,
  lastColorTick: -1,
  replayTick: -1,   // last replay tick drawn: a smaller one means the replay looped back
  lastStatsTick: -1,
  lastCardTick: -1, // src.tick at the last agent-card rebuild (see refreshStats)
  animTick: 0,      // src.tick at the last forest-transition step
  hubsNeedIds: false,   // market stalls were placed before the terrain ids existed
  fps: 0,
  rate: 0,
  frames: 0,
  frameNo: 0,      // monotonic frame counter (`frames` resets every FPS window)
  ticksWindow: 0,
  windowStart: performance.now(),
  sinceStep: 0,
};

// ---------------------------------------------------------------------------
// World lifecycle
// ---------------------------------------------------------------------------
async function loadScenario(entry, seed) {
  cover("Instantiating the world…", `${entry.name} — parsing the scenario and generating terrain in the wasm core.`);
  try {
    const text = await (await fetch(entry.file)).text();
    const src = await openLive("wasm/anabios.wasm", text, seed);
    attach(src, entry);
    document.body.classList.remove("replay");
    if (!params.has("capture")) history.replaceState(null, "", `?scenario=${encodeURIComponent(entry.id)}${seed !== undefined && seed !== "" ? `&seed=${seed}` : ""}`);
    await prepareShot();
  } catch (e) {
    fail("Could not start the live world", e);
    return;
  }
  cover(null);
}

async function loadReplay(entry) {
  cover("Loading the recording…", entry.name);
  try {
    const src = await openReplay(entry.file);
    attach(src, entry);
    document.body.classList.add("replay");
    if (!params.has("capture")) history.replaceState(null, "", `?replay=${encodeURIComponent(entry.id)}`);
    await prepareShot();
  } catch (e) {
    fail("Could not load the replay", e);
    return;
  }
  cover(null);
}

function attach(source, entry) {
  if (state.source) { state.source.dispose(); state.source = null; }
  if (state.terrain) { world.remove(state.terrain.group); state.terrain.dispose(); }
  state.source = source;
  const ws = source.worldSize;
  state.terrain = new Terrain(source.biomeRes, ws, source.seaLevel, source.elevation(), source.terrain());
  state.terrain.updateColors(source.biomeRgba());
  world.add(state.terrain.group);
  stage.fit(ws);
  stage.frame();
  for (const l of [layers.agents, layers.villages, layers.fx, layers.particles]) l.setWorldSize(ws, state.terrain.cell);
  layers.streaks.clear(); layers.trades.clear(); layers.villages.clear(); layers.particles.clear();
  layers.villages.isWater = isWater;
  layers.hubs.set(source.hubs(), state.terrain.cell, heightAt, isWater, ws);
  state.hubsNeedIds = !state.terrain.terrainIds;   // placed blind: redo once the ids are classified
  layers.birds.setWorld(ws, source.biomeRes, state.terrain.cell, heightAt);
  layers.agents.reset();
  state.selected = -1; state.follow = false; $("card").classList.remove("show");
  state.lastColorTick = -1; state.lastStatsTick = -1; state.lastCardTick = -1; state.replayTick = -1;
  $("codex").innerHTML = "";
  recentFx.clear(); recentLines.clear();
  applyLayerToggles();
  const isLive = source.kind === "live";
  $("badge").classList.toggle("replay", !isLive);
  $("badge-text").textContent = isLive ? "live · wasm" : "recorded replay";
  $("desc").textContent = entry.description || (isLive ? "" : "A deterministic replay recorded by anabios-headless; the world is played back frame by frame.");
  $("seed").value = source.meta().seed;
  setSeedControls(isLive);
  buildColorModes(source);
  refreshStats(true);
}

/** A recorded replay plays back one fixed seed: `Load` ignores the seed box
 *  for it and `attach` writes the recording's seed back, so `Random seed`
 *  would only flash a number and restart the replay at tick 0. Both controls
 *  are disabled while a replay entry is selected (the CSS dims them). */
function setSeedControls(live) {
  $("seed").disabled = !live;
  $("reseed").disabled = !live;
}

function heightAt(x, y) { return state.terrain ? state.terrain.heightAt(x, y) : 0; }
function isWater(x, y) { return state.terrain ? state.terrain.isWater(x, y) : false; }

/** Village update + the forest clearings under the huts (trees stand back from a village while its layer is on). */
function updateVillages(src, tick, instant = false) {
  layers.villages.update(src.sites(), tick, heightAt, 4, instant);
  // A time jump draws its villages grown: their clearings are made at once too.
  syncClearings(false, instant || layers.villages.jumped);
}
/** `force`: the layer was toggled (trees ease back in, or out of the huts' way). */
function syncClearings(force = false, snap = false) {
  if (!state.terrain) return;
  if (!layers.villages.mesh.visible) { if (force) state.terrain.forest.setClearings([]); return; }
  if (force) layers.villages.clearingKey = "";
  const circles = layers.villages.clearings();
  if (circles) state.terrain.forest.setClearings(circles, snap);
}

const nextFrame = () => new Promise((r) => requestAnimationFrame(r));

/**
 * Capture harness: `?tick=N` fast-forwards the fresh world to exactly tick N
 * (a replay seeks there, clamped short of its end) and pauses there unless
 * `&paused=0`; then `?cam=fit | event | x,y,zoom[,polar]`
 * frames it and `?inspect=<id> | sp<species>` pins an agent. The loop flips
 * `state.ready` once all of that is on screen; `web/scripts/capture.mjs`
 * waits for it. `zoom` follows the Godot viewer's convention (screen pixels
 * per world unit at a 1280 px wide viewport), so `gallery/README.md` env
 * values map 1:1.
 */
async function prepareShot() {
  const src = state.source, shot = state.shot;
  state.ready = false;
  if (shot.tick === null && !shot.cam && !shot.inspect) { state.ready = true; return; }
  if (shot.tick !== null) {
    if (src.kind === "replay") {
      src.seek(shot.tick);
      if (params.get("paused") !== "0") setPaused(true);
    } else {
      setPaused(true);
      const fxWas = layers.fx.enabled;
      layers.fx.enabled = false;
      cover("Fast-forwarding…", `${src.meta().scenario} → tick ${shot.tick.toLocaleString()}`);
      state.fastForwarding = true;
      let lastPaint = performance.now();
      while (src.tick < shot.tick) {
        const remaining = shot.tick - src.tick;
        // The last 40 ticks step one at a time so combat/trade trails and
        // villages accumulate exactly as they would in live play.
        const batch = remaining > 40 ? Math.min(50, remaining - 40) : 1;
        src.advance(batch);
        const tick = src.tick;
        if (remaining <= 40) {
          layers.streaks.push(src.streaks(), tick);
          layers.trades.push(src.trades(), tick);
          layers.villages.classify(src.agents());
          updateVillages(src, tick, true);
        }
        for (const ev of src.events()) onEvent(ev, performance.now() / 1000);
        if (performance.now() - lastPaint > 250) {
          $("cover-body").textContent = `tick ${Math.floor(tick).toLocaleString()} / ${shot.tick.toLocaleString()} · ${src.sim.alive().toLocaleString()} alive`;
          await nextFrame();
          lastPaint = performance.now();
        }
      }
      state.fastForwarding = false; layers.agents.reset();
      layers.fx.enabled = fxWas;
      if (params.get("paused") === "0") setPaused(false);
    }
  }
  if (params.get("hud") === "0") document.body.classList.add("hide-hud");
  state.sinceStep = 0;          // force one full layer update at this tick
  state.lastStatsTick = -1; state.lastCardTick = -1;
  shot.stage = 1;
}

/** Apply `?cam=` once the world is on screen (needs terrain heights + last event). */
function applyShotCamera(spec) {
  const src = state.source;
  if (!spec || spec === "fit") { stage.frame(); return; }
  if (spec === "event") {
    const p = state.lastEventLoc || { x: src.worldSize / 2, y: src.worldSize / 2 };
    stage.lookAt(p.x, heightAt(p.x, p.y), p.y, stage.distanceForWidth(1280 / 2), 0.8);
    return;
  }
  // `site,<zoom>` aims at the largest settlement on screen, `hub,<zoom>` at
  // the busiest-looking market (the hub nearest the world centre): the Godot
  // gallery frames these by torus-wrapped coordinates the atlas can't wrap.
  if (spec.startsWith("site") || spec.startsWith("hub")) {
    const zoom = Number(spec.split(",")[1]) || 4;
    let best = null;
    if (spec.startsWith("site")) {
      for (const v of layers.villages.sites.values()) if (!best || v.n > best.n) best = v;
    } else {
      // Picked by the sim's hub position, aimed at where its stall is drawn
      // (a hub in a lake is drawn on the nearest shore).
      const h = src.hubs(), c = src.worldSize / 2;
      for (let k = 0; k < h.count; k++) {
        const hx = h.data[k * 3], hy = h.data[k * 3 + 1], d = Math.hypot(hx - c, hy - c);
        const at = layers.hubs.spots[k] || { x: hx, y: hy };
        if (!best || d < best.d) best = { x: at.x, y: at.y, d };
      }
    }
    if (!best) { stage.frame(); return; }
    stage.lookAt(best.x, heightAt(best.x, best.y), best.y, stage.distanceForWidth(1280 / zoom), zoom >= 3 ? 0.8 : 0.95);
    return;
  }
  let [x, y, zoom = 2, polar] = spec.split(",").map(Number);
  // Gallery coordinates may sit past the seam (the Godot viewer wraps the
  // ground); fold them back onto the plate.
  const ws = src.worldSize;
  x = ((x % ws) + ws) % ws;
  y = ((y % ws) + ws) % ws;
  const worldWidth = 1280 / Math.max(0.05, zoom);
  stage.lookAt(x, heightAt(x, y), y, stage.distanceForWidth(worldWidth), polar || (zoom >= 3 ? 0.8 : 0.95));
}

/** Resolve `?inspect=` against the agents now on screen. */
function applyShotInspect(spec) {
  if (!spec) return;
  const a = state.source.agents();
  const m = /^sp(\d+)$/.exec(spec);
  if (m) {
    const sid = Number(m[1]);
    for (let k = 0; k < a.count; k++) if (a.data[k * a.stride + 11] === sid) { select(a.data[k * a.stride]); return; }
  } else {
    const id = Number(spec);
    for (let k = 0; k < a.count; k++) if (a.data[k * a.stride] === id) { select(id); return; }
  }
  if (a.count) select(a.data[0]);   // requested agent is dead: pin the lowest live slot instead
}

// ---------------------------------------------------------------------------
// Frame loop
// ---------------------------------------------------------------------------
let last = performance.now();
function loop(now) {
  requestAnimationFrame(loop);
  if (state.fastForwarding) { last = now; return; }   // the harness owns the frame budget while it steps
  const dt = Math.min(0.1, (now - last) / 1000); last = now;
  const src = state.source;
  stage.tick(dt);
  const clock = reduceMotion ? 0 : now / 1000;
  if (state.terrain) { state.terrain.water.update(clock); state.terrain.forest.setTime(clock); state.terrain.setTime(clock); }
  layers.agents.setTime(clock);
  layers.birds.update(clock);
  layers.fx.update(now / 1000);
  const viewportHeightPx = $("view").clientHeight;
  layers.particles.update(reduceMotion ? 0 : dt, viewportHeightPx / (2 * Math.tan((stage.camera.fov * Math.PI) / 360)));
  // Computed once per frame (not per agent) and threaded through the update
  // path below rather than having layers.js reach into the camera directly.
  const unitsPerPixel = stage.unitsPerPixel(viewportHeightPx);
  if (layers.agents.marker.visible) {
    const pulse = 0.5 + 0.5 * Math.sin(clock * 5);
    // Sized to the drawn figure, not the world's base scale: close in,
    // bodyScale shrinks a figure to its physical body (~1.5-2 units) and a
    // baseScale ring stayed 12-15 units across, a hoop ~7x the figure. At
    // 0.5 x the drawn length the ring (inner/outer radius 1.25/1.55 at scale
    // 1) hugs the figure at ~1.6-2.4x its length, and the pixel floor keeps
    // it >= ~24 px across (inner edge clear of a LEGIBLE_PX figure) however
    // small the figure is on screen.
    const extent = layers.agents.selectedExtent;
    const ringScale = extent > 0 ? Math.max(extent * 0.5, 6 * unitsPerPixel) : layers.agents.baseScale;
    layers.agents.marker.scale.setScalar(ringScale * (1.3 + 0.25 * pulse));
    layers.agents.marker.material.opacity = 0.55 + 0.4 * pulse;
  }

  if (src) {
    let stepped = 0;
    if (!state.paused) {
      // ticks/frame is defined at 60 Hz; scale by the real frame time but never
      // ask for more than 2× the nominal batch so a slow world can't spiral.
      const want = Math.min(state.speed * 2, state.speed * 60 * dt);
      stepped = src.advance(want);
      state.ticksWindow += stepped;
    }
    const fractional = src.kind === "replay" || state.speed < 1;
    // Figure size depends on the zoom (`bodyScale`), so a paused world that is
    // zoomed into must still re-seat its figures — or a close-up kept the
    // far-away readable size and bodies piled through each other.
    const rezoomed = Math.abs(unitsPerPixel / (state.agentsUpp || unitsPerPixel) - 1) > 0.03;
    if (stepped > 0 || fractional || state.sinceStep === 0) {
      const tick = src.tick;
      // A replay loops back to tick 0 at its end: trails stamped near the end
      // would sit in the "future" and stay lit at full strength for the whole
      // first stretch, and every figure would read as a death plus a birth.
      if (src.kind === "replay" && tick < state.replayTick) { layers.streaks.clear(); layers.trades.clear(); layers.agents.reset(); }
      state.replayTick = tick;
      const agents = src.agents();
      layers.agents.update(agents, heightAt, src.kind === "live", unitsPerPixel);
      state.agentsUpp = unitsPerPixel;
      layers.villages.classify(agents);   // before the next wasm call can detach the view
      if (stepped > 0 || src.kind === "replay") {
        const streaks = src.streaks();
        layers.streaks.push(streaks, tick);
        // Impact sparks at the struck end of each volley.
        for (let k = 0, n = Math.min(streaks.count, 150); k < n; k++) {
          const o = k * 5, x2 = streaks.data[o + 2], y2 = streaks.data[o + 3];
          layers.particles.spawn(KIND.SPARK, x2, heightAt(x2, y2) + layers.agents.baseScale * 0.5, y2, 3);
        }
        // A glimmer where an agent was born, a grey puff where one died (skipped
        // at the fastest speeds, where whole generations pass between frames).
        if (state.speed <= 16) {
          const born = layers.agents.born, died = layers.agents.died, lift = layers.agents.baseScale * 0.6;
          for (let k = 0, n = Math.min(born.length, 240); k < n; k += 2) layers.particles.spawn(KIND.MOTE, born[k], heightAt(born[k], born[k + 1]) + lift, born[k + 1], 3, 0xfff2c8);
          for (let k = 0, n = Math.min(died.length, 400); k < n; k += 2) layers.particles.spawn(KIND.SMOKE, died[k], heightAt(died[k], died[k + 1]) + lift, died[k + 1], 2);
        }
        layers.trades.push(src.trades(), tick);
        updateVillages(src, tick);
        for (const ev of src.events()) onEvent(ev, now / 1000);
      }
      if (Math.floor(tick / 10) !== Math.floor(state.lastColorTick / 10)) {
        // Refreshes are ≤ 128 ticks apart in play (64×); a wider gap is a
        // fast-forward or seek, where scarred trees take their size at once.
        const gap = tick - state.lastColorTick;
        state.terrain.updateColors(src.biomeRgba(), gap < 0 || gap > 300);
        state.lastColorTick = tick;
        if (state.hubsNeedIds && state.terrain.terrainIds) { layers.hubs.layout(); state.hubsNeedIds = false; }
      }
      if (Math.floor(tick / 30) !== Math.floor(state.lastStatsTick / 30) || stepped === 0) {
        refreshStats(false);
        state.lastStatsTick = tick;
      }
      state.sinceStep = 1;
      if (state.shot.stage === 1) {
        applyShotCamera(state.shot.cam);
        applyShotInspect(state.shot.inspect);
        state.shot.stage = 2;
        state.shot.appliedAt = state.frameNo;
      }
    } else if (rezoomed) {
      layers.agents.update(src.agents(), heightAt, src.kind === "live", unitsPerPixel);
      state.agentsUpp = unitsPerPixel;
    }
    // One frame has been rendered with the camera and selection applied. Not
    // an `else` of the update branch above: with `&paused=0` the world steps
    // every frame, and that branch would starve this one forever.
    if (state.shot.stage === 2 && state.shot.appliedAt !== state.frameNo) {
      state.shot.stage = 0;
      state.ready = true;
    }
    layers.streaks.update(src.tick, heightAt);
    layers.trades.update(src.tick, heightAt);
    // Hearth smoke drifts up from every settled village while the world runs.
    if (!state.paused && layers.villages.mesh.visible) {
      for (const v of layers.villages.centers()) {
        if (Math.random() < dt * 1.8) layers.particles.spawn(KIND.SMOKE, v.x, (v.base ?? heightAt(v.x, v.y)) + layers.villages.scale * 1.25, v.y, 1);
      }
    }
    if (state.follow && layers.agents.selectedPos) {
      const p = layers.agents.selectedPos, before = stage.controls.target.clone();
      stage.controls.target.lerp(p, 0.15);
      stage.camera.position.add(stage.controls.target.clone().sub(before)); // keep the camera offset
    }
    if (src.kind === "replay") $("progress-fill").style.width = `${(100 * src.tick / src.endTick).toFixed(2)}%`;
    if (state.dayCycle) {
      stage.setDaylight((src.tick % state.DAY_TICKS) / state.DAY_TICKS);
      state.terrain.water.uniforms.uSun.value.copy(stage.sunDir);
    }
  }

  // Trees easing into or out of a clearing (or a scar): paced by the ticks
  // this frame advanced, within a wall-time band (see terrain.js TRANSITION_TICKS).
  if (state.terrain) {
    const t = src ? src.tick : 0, dTicks = Math.max(0, t - state.animTick);
    state.animTick = t;
    state.terrain.forest.update(reduceMotion ? Infinity : dt, dTicks);
  }

  stage.render();

  state.frames++;
  state.frameNo++;
  const span = now - state.windowStart;
  if (span >= 500) {
    state.fps = state.frames * 1000 / span;
    state.rate = state.ticksWindow * 1000 / span;
    state.frames = 0; state.ticksWindow = 0; state.windowStart = now;
    $("stat-fps").textContent = state.fps.toFixed(0);
    $("stat-rate").textContent = state.paused ? "—" : state.rate.toFixed(0);
  }
}

// ---------------------------------------------------------------------------
// HUD
// ---------------------------------------------------------------------------
function refreshStats(full) {
  const src = state.source; if (!src) return;
  const species = src.species();
  $("stat-tick").textContent = Math.floor(src.tick).toLocaleString();
  const meta = src.meta();
  $("stat-alive").textContent = meta.alive.toLocaleString();
  $("stat-species").textContent = species.length;
  $("stat-era").textContent = species.reduce((m, s) => Math.max(m, s.tech_era || 0), 0);
  const rows = species.slice().sort((a, b) => b.count - a.count).slice(0, 14).map((s) =>
    `<tr data-sid="${s.id}" title="click: fly to a member"><td><span class="sw" style="background:${cssHex(hsv(speciesHue(s.id), 0.6, 0.9))}"></span>${esc(s.name)}</td><td>${s.count.toLocaleString()}</td><td>${s.tech_era ? "era " + s.tech_era : ""}</td></tr>`);
  $("species").innerHTML = rows.join("");
  $("receipt").innerHTML = `<b>${esc(meta.scenario)}</b> · seed <b>${meta.seed}</b>` +
    (meta.fingerprint ? ` · fingerprint <b>${meta.fingerprint}</b>` : "") +
    (meta.state_hash ? `<br>state hash <b>${meta.state_hash}</b>` : "") +
    (src.kind === "live" ? `<br>${src.stepMs.toFixed(1)} ms per step batch · deterministic per seed` : `<br>${meta.ticks?.toLocaleString()} ticks · sampled every ${meta.sample} · stride ${meta.stride}`);
  // The card rebuilds when the tick has crossed a 15-tick boundary since its
  // last rebuild, not when the tick sampled here happens to be a multiple of
  // 15: at 4×–64× the step batch is a fractional accumulator, so that sample
  // lands on a multiple only by chance and the card froze for hundreds to
  // thousands of ticks. Crossing still caps it at one rebuild per 15 ticks
  // at fractional speeds and in replays, where stats refresh every frame.
  if (state.selected >= 0 && (full || Math.floor(src.tick / 15) !== Math.floor(state.lastCardTick / 15))) {
    renderCard();
    state.lastCardTick = src.tick;
  }
}

function buildColorModes(source) {
  const flags = source.flags, live = source.kind === "live";
  const available = {
    species: true, diet: true,
    dialect: live, energy: live,
    mood: live && (flags & WORLD_FLAG.AFFECT) !== 0,
    arousal: live && (flags & WORLD_FLAG.AFFECT) !== 0,
    infection: live && (flags & WORLD_FLAG.DISEASE) !== 0,
  };
  const sel = $("color-mode");
  sel.innerHTML = COLOR_MODES.filter((m) => available[m]).map((m) => `<option value="${m}">${m}</option>`).join("");
  if (!available[state.colorMode]) state.colorMode = "species";
  sel.value = state.colorMode;
  layers.agents.mode = state.colorMode;
  renderLegend();
}

function renderLegend() {
  const m = state.colorMode, L = $("legend");
  const grad = (a, b, la, lb) => `<div class="item"><span>${la}</span><span class="grad" style="background:linear-gradient(90deg,${a},${b})"></span><span>${lb}</span></div>`;
  switch (m) {
    case "diet": L.innerHTML = grad("#7fbf5a", "#d64a3a", "grazer", "predator"); break;
    case "energy": L.innerHTML = grad("#3a2a22", "#f2d27a", "starving", "sated"); break;
    case "arousal": L.innerHTML = grad("#4a6b8a", "#ff5a2a", "calm", "fear / rage / panic"); break;
    case "infection": L.innerHTML = grad("#b7ac98", "#77d64a", "healthy", "infected"); break;
    case "dialect": L.innerHTML = `<div class="item">hue = weighted meme vector; distinct colours are distinct dialects</div>`; break;
    case "mood": {
      const names = state.source?.catalog?.moods || ["content", "seek food", "seek water", "sleep", "flee", "fight", "seek mate", "mate"];
      L.innerHTML = names.map((n, i) => `<div class="item"><span class="sw" style="background:${cssHex(MOOD_COLORS[i])}"></span>${n}</div>`).join("");
      break;
    }
    default: L.innerHTML = `<div class="item">species colour (as in the species list), shaded per individual by the genome's hue/sat/val; livestock bleached</div>`;
  }
}

/**
 * Several detectors re-fire every tick while their condition holds (a mass
 * fright or a dehydration wave is dozens of events a second at 1×, up to ~50
 * in one tick). Each one used to spawn its own ring, light pillar and burst
 * of additive particles on the same spot — stacking into a white blow-out
 * under bloom — and its own feed line, so the feed churned faster than a line
 * could fade in and read as empty. Now a repeat within `REPEAT_TICKS` of the
 * last one of its kind (or within `REPEAT_SECS` of wall time, for a slow
 * machine at 64× where one frame spans more ticks than that) is folded in:
 * the feed keeps one line per event type, bumping a `×n` count and the
 * species tally, and a species' effects re-fire at most every
 * `REPEAT_FX_SECS`, with at most `FX_PER_FRAME` bursts a frame. A repeat also
 * moves its line to the newest end of the feed (first child; the feed is
 * `column-reverse`, so that is the bottom row): the line shows the latest
 * tick, and left where it was created it sat among older lines, so the feed's
 * ticks went up and down the list instead of reading chronologically.
 */
const REPEAT_TICKS = 120, REPEAT_SECS = 1.5, REPEAT_FX_SECS = 2.5, FX_PER_FRAME = 8;
const recentFx = new Map();     // `${type}/${sid}` → {tick, fxAt}
const recentLines = new Map();  // type → {tick, count, sids, line}
let fxFrame = -1, fxThisFrame = 0;

/** The entry for `key` if its last event is recent (and not in the future: a replay seek starts over), else a fresh one. */
function recent(map, key, tick, now, fresh) {
  const prev = map.get(key);
  const repeat = !!prev && tick >= prev.tick && (tick - prev.tick <= REPEAT_TICKS || now - prev.at <= REPEAT_SECS);
  const entry = repeat ? prev : fresh();
  entry.tick = tick; entry.at = now;
  map.set(key, entry);
  return [entry, repeat];
}

function onEvent(ev, now) {
  const kind = kindOf(ev.type);
  const [fx, repeat] = recent(recentFx, `${ev.type}/${ev.sid}`, ev.tick, now, () => ({ fxAt: -Infinity }));
  if (ev.x || ev.y) {
    state.lastEventLoc = { x: ev.x, y: ev.y };
    if (!repeat && state.tour && now - state.lastFlyAt > 3 && !state.fastForwarding) {
      state.lastFlyAt = now;
      stage.flyTo(ev.x, heightAt(ev.x, ev.y), ev.y, state.source.worldSize * (kind === "war" ? 0.12 : 0.18));
    }
  }
  if (fxFrame !== state.frameNo) { fxFrame = state.frameNo; fxThisFrame = 0; }
  if ((ev.x || ev.y) && layers.fx.enabled && now - fx.fxAt >= REPEAT_FX_SECS && fxThisFrame < FX_PER_FRAME) {
    fx.fxAt = now;
    fxThisFrame++;
    layers.fx.spawn(ev, now, heightAt);
    const h = heightAt(ev.x, ev.y) + 0.6;
    if (kind === "fire") layers.particles.spawn(KIND.EMBER, ev.x, h, ev.y, 16);
    else if (kind === "war") layers.particles.spawn(KIND.SPARK, ev.x, h, ev.y, 22);
    else layers.particles.spawn(KIND.MOTE, ev.x, h, ev.y, 10, KIND_COLOR[kind]);
  }
  const feed = $("codex");
  const [entry] = recent(recentLines, ev.type, ev.tick, now, () => ({ count: 0, sids: new Set(), line: null }));
  if (!entry.line || entry.line.parentNode !== feed) {
    const line = document.createElement("div");
    line.className = "codex-line";
    line.innerHTML = `<span class="tick"></span> · <span class="sw" style="background:${KIND_CSS[kind]}"></span><span style="color:${KIND_CSS[kind]}">${esc(ev.type.replace(/_/g, " "))}</span><span class="rep"></span><span class="tick who"></span>`;
    feed.prepend(line);
    requestAnimationFrame(() => line.classList.add("show"));
    while (feed.children.length > 7) feed.removeChild(feed.lastChild);
    entry.line = line; entry.count = 0; entry.sids.clear();
  }
  entry.count++;
  if (ev.sid != null) entry.sids.add(ev.sid);
  const line = entry.line;
  // A move, not a re-insert: the line keeps its `.show` class, so it does not
  // fade in again, and the feed's length (the 7-line trim) is unchanged.
  if (feed.firstChild !== line) feed.prepend(line);
  line.firstChild.textContent = Math.floor(ev.tick).toLocaleString();
  line.querySelector(".rep").textContent = entry.count > 1 ? ` ×${entry.count}` : "";
  const who = entry.sids.size > 1 ? `${entry.sids.size} species` : ev.sid == null ? "" : state.source.labels.get(ev.sid) || `species ${ev.sid}`;
  line.querySelector(".who").textContent = who ? ` — ${who}` : "";
  if (ev.x || ev.y) line.onclick = () => stage.flyTo(ev.x, heightAt(ev.x, ev.y), ev.y, state.source.worldSize * 0.18);
}

function renderCard() {
  const src = state.source; if (!src || state.selected < 0) return;
  const a = src.agent(state.selected);
  if (!a) { toast(`agent ${state.selected} died`); deselect(); return; }   // toast first: deselect clears the id
  $("card-title").textContent = `${a.species} · #${a.id}`;
  const rows = [];
  const row = (k, v) => rows.push(`<dt>${k}</dt><dd>${v}</dd>`);
  if (a.replay) {
    row("diet", `${(a.diet_carnivory * 100).toFixed(0)}% carnivore`);
    row("position", `${a.x.toFixed(0)}, ${a.y.toFixed(0)}`);
    rows.push(`<dt></dt><dd style="color:var(--muted)">recorded replays carry position, species and diet only</dd>`);
  } else {
    row("energy", `${a.energy.toFixed(1)}<div class="bar"><i style="width:${Math.min(100, a.energy / 1.5).toFixed(0)}%"></i></div>`);
    row("age", a.age.toLocaleString());
    row("diet", `${(a.diet_carnivory * 100).toFixed(0)}% carnivore`);
    row("size", a.size.toFixed(2));
    // The sleep mood already says it: "sleep (asleep)" was redundant. The
    // flag still shows when a sleeper's arbiter has moved on to another mood.
    row("mood", a.mood + (a.asleep && a.mood !== "sleep" ? " (asleep)" : ""));
    if (a.iq > 0) row("iq", a.iq.toFixed(2));
    if (a.infection > 0) row("infection", a.infection.toFixed(2));
    if (a.arousal > 0) row("arousal", a.arousal.toFixed(2));
    if (a.livestock_of >= 0) row("livestock of", `#${a.livestock_of}`);
    row("learning", [a.indiv_learn ? "individual" : "", a.social_learn ? "social" : ""].filter(Boolean).join(" + ") || "innate");
    row("skill / tech", `${a.skill.toFixed(2)} / ${a.technique.toFixed(2)}`);
    row("program", `${a.program_len} nodes`);
    row("body", `<div class="chips">${a.modules.map((m) => `<span class="chip">${esc(m)}</span>`).join("")}</div>`);
    if (a.inventions.length) row(`tech · era ${a.tech_era}`, `<div class="chips">${a.inventions.map((i) => `<span class="chip">${esc(i.name)}</span>`).join("")}</div>`);
  }
  $("card-body").innerHTML = `<dl>${rows.join("")}</dl>`;
  $("card").classList.add("show");
  $("follow").classList.toggle("on", state.follow);
}

function select(id) { state.selected = id; layers.agents.selected = id; renderCard(); }
function deselect() { state.selected = -1; layers.agents.selected = -1; state.follow = false; $("card").classList.remove("show"); }

function applyLayerToggles() {
  for (const cb of document.querySelectorAll("#layers input")) {
    if (cb.dataset.layer === "day" && !state.dayInit) { cb.checked = state.dayCycle; state.dayInit = true; }
    if (cb.dataset.layer === "events" && !state.eventsInit) { cb.checked = params.get("events") === "1"; state.eventsInit = true; }
    const on = cb.checked;
    switch (cb.dataset.layer) {
      case "relief":
        if (!state.terrain) break;
        state.terrain.setRelief(on);
        // Buildings are seated when placed: re-seat them on the reshaped ground.
        layers.hubs.layout();   // re-seat on the new heights (and re-check the shore)
        layers.villages.layout(heightAt);
        break;
      case "water": if (state.terrain) state.terrain.water.mesh.visible = on && state.terrain.reliefOn; break;
      case "forest": if (state.terrain) state.terrain.forest.group.visible = on; break;
      case "shadows": stage.renderer.shadowMap.enabled = on; stage.scene.traverse((o) => { if (o.material) o.material.needsUpdate = true; }); break;
      case "streaks": layers.streaks.lines.visible = on; break;
      case "trades": layers.trades.lines.visible = on; break;
      case "villages": layers.villages.mesh.visible = on; syncClearings(true); break;
      case "hubs": layers.hubs.mesh.visible = on; break;
      // Event markers (a ring, a light pillar and a burst per codex event) are
      // opt-in: with dozens of events a minute they crowded the map, and the
      // feed already lists every one (click a line to fly there).
      case "events": layers.fx.group.visible = on; layers.fx.enabled = on && !reduceMotion; break;
      case "sparks": layers.particles.group.visible = on; layers.particles.enabled = on && !reduceMotion; break;
      case "wire": if (state.terrain) state.terrain.material.wireframe = on; break;
      case "bloom": stage.post = on; break;
      case "clouds": if (state.terrain) state.terrain.uniforms.uCloud.value = on ? 1 : 0; break;
      case "birds": layers.birds.mesh.visible = on; break;
      case "day": state.dayCycle = on; if (!on) { stage.setDaylight(0); state.terrain?.water.uniforms.uSun.value.copy(stage.sunDir); } break;
    }
  }
}

function setSpeed(s) {
  state.speed = s;
  for (const b of document.querySelectorAll(".speed")) b.classList.toggle("on", Number(b.dataset.speed) === s);
}

function setPaused(p) {
  state.paused = p; $("play").textContent = p ? "▶" : "❚❚";
  // A paused world skips the frame loop's stats refresh, so bring the card up
  // to the paused tick now (and report a death in the ticks since its last
  // rebuild: toast, deselect, release the follow camera) instead of leaving
  // it frozen on stale values.
  if (p && state.selected >= 0) { renderCard(); state.lastCardTick = state.source ? state.source.tick : -1; }
}
/** Event tour: the camera drifts in a slow orbit and cuts to each fresh codex event. */
function setTour(on) {
  state.tour = on;
  stage.controls.autoRotate = on;
  stage.controls.autoRotateSpeed = 0.35;
  $("tour").classList.toggle("on", on);
  if (on && state.lastEventLoc) { const p = state.lastEventLoc; stage.flyTo(p.x, heightAt(p.x, p.y), p.y, state.source.worldSize * 0.18); }
}

function cover(title, body, code) {
  const c = $("cover");
  if (!title) { c.classList.remove("show"); return; }
  $("cover-title").textContent = title; $("cover-body").textContent = body || "";
  const pre = $("cover-code"); pre.style.display = code ? "block" : "none"; pre.textContent = code || "";
  c.classList.add("show");
}
function fail(title, e) {
  console.error(e);
  cover(title, `${e.message}. The page needs the built assets next to it — from the repository root run:`,
    "scripts/web.sh build     # wasm core → web/wasm, three.js → web/vendor, scenarios → web/scenarios\nscripts/web.sh serve     # then open http://127.0.0.1:8080/");
}
let toastTimer = 0;
function toast(msg) { const t = $("toast"); t.textContent = msg; t.classList.add("show"); clearTimeout(toastTimer); toastTimer = setTimeout(() => t.classList.remove("show"), 3500); }
function esc(s) { return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;"); }

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------
const raycaster = new THREE.Raycaster();
let downAt = null;
const canvas = $("view");
canvas.addEventListener("pointerdown", (e) => { downAt = [e.clientX, e.clientY]; });
canvas.addEventListener("pointerup", (e) => {
  if (!downAt || Math.hypot(e.clientX - downAt[0], e.clientY - downAt[1]) > 4 || e.button !== 0) return;
  const r = canvas.getBoundingClientRect();
  const ndc = new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
  raycaster.setFromCamera(ndc, stage.camera);
  const hits = raycaster.intersectObjects(layers.agents.meshes, false);
  if (hits.length && hits[0].instanceId !== undefined) select(hits[0].object.userData.ids[hits[0].instanceId]);
  else deselect();
});

window.addEventListener("keydown", (e) => {
  if (e.target.closest("input,select,textarea")) return;
  const speeds = [0.25, 1, 4, 16, 64];
  switch (e.code) {
    case "Space": e.preventDefault(); setPaused(!state.paused); break;
    case "KeyF": stage.frame(); break;
    case "KeyH": document.body.classList.toggle("hide-hud"); stage.refit(); break;
    case "KeyC": { const opts = Array.from($("color-mode").options).map((o) => o.value); state.colorMode = opts[(opts.indexOf(state.colorMode) + 1) % opts.length]; $("color-mode").value = state.colorMode; layers.agents.mode = state.colorMode; renderLegend(); break; }
    case "KeyL": if (state.selected >= 0) { state.follow = !state.follow; $("follow").classList.toggle("on", state.follow); } break;
    case "KeyV": setTour(!state.tour); break;
    case "Escape": deselect(); break;
    default: if (/^Digit[1-5]$/.test(e.code)) setSpeed(speeds[Number(e.code[5]) - 1]);
  }
});

$("play").onclick = () => setPaused(!state.paused);
$("frame").onclick = () => stage.frame();
$("tour").onclick = () => setTour(!state.tour);
for (const b of document.querySelectorAll(".speed")) b.onclick = () => setSpeed(Number(b.dataset.speed));
$("color-mode").onchange = (e) => { state.colorMode = e.target.value; layers.agents.mode = state.colorMode; renderLegend(); };
$("layers").addEventListener("change", applyLayerToggles);
$("rail-toggle").onclick = () => { const r = $("rail"); r.classList.toggle("collapsed"); $("rail-toggle").textContent = r.classList.contains("collapsed") ? "+" : "−"; stage.refit(); };
$("card-close").onclick = deselect;
$("follow").onclick = () => { state.follow = !state.follow; $("follow").classList.toggle("on", state.follow); };
$("card-fly").onclick = () => { const p = layers.agents.selectedPos; if (p) stage.flyTo(p.x, p.y, p.z, state.source.worldSize * 0.1); };
$("species").addEventListener("click", (e) => {
  const tr = e.target.closest("tr"); if (!tr || !state.source) return;
  const sid = Number(tr.dataset.sid);
  const a = state.source.agents();
  for (let k = 0; k < a.count; k++) {
    const o = k * a.stride;
    if (a.data[o + 11] === sid) { const x = a.data[o + 1], y = a.data[o + 2]; stage.flyTo(x, heightAt(x, y), y, state.source.worldSize * 0.15); break; }
  }
});
$("load").onclick = () => {
  const opt = $("scenario").selectedOptions[0]; if (!opt) return;
  const seed = $("seed").value;
  if (opt.dataset.kind === "replay") loadReplay(state.manifest.replays.find((r) => r.id === opt.value));
  else loadScenario(state.manifest.scenarios.find((s) => s.id === opt.value), seed === "" ? undefined : Number(seed));
};
$("reseed").onclick = () => { $("seed").value = Math.floor(Math.random() * 1e6); $("load").click(); };
$("scenario").onchange = () => {
  const opt = $("scenario").selectedOptions[0];
  // A replay and a live scenario can share an id (`out-of-africa-saga` does),
  // so the option's kind decides which manifest to read.
  setSeedControls(opt.dataset.kind !== "replay");   // before Load, so a replay pick greys the seed at once
  if (opt.dataset.kind === "replay") {
    const r = state.manifest.replays.find((x) => x.id === opt.value);
    if (r) $("desc").textContent = r.description || r.name;
    return;
  }
  const s = state.manifest.scenarios.find((x) => x.id === opt.value);
  if (s) { $("seed").value = s.seed; $("desc").textContent = s.description; }
};
/** Select the dropdown entry for `id` of the given kind: `sel.value = id`
 *  alone would pick the first option with that value, the live one, even
 *  when the replay of the same name is meant. */
function selectEntry(sel, kind, id) {
  const o = Array.from(sel.options).find((x) => x.dataset.kind === kind && x.value === id);
  if (o) o.selected = true;
}
$("terrain-legend").innerHTML = TERRAIN.map(([n, c]) => `<div class="item"><span class="sw" style="background:${c}"></span>${n}</div>`).join("");

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------
async function boot() {
  setSpeed(state.speed); setPaused(state.paused);
  layers.agents.mode = state.colorMode;
  try {
    const r = await fetch("scenarios/index.json");
    if (!r.ok) throw new Error(`scenarios/index.json: HTTP ${r.status}`);
    state.manifest = await r.json();
  } catch (e) { fail("No scenario manifest", e); requestAnimationFrame(loop); return; }
  const sel = $("scenario");
  const groups = [["live · wasm", state.manifest.scenarios, "live"], ["recorded", state.manifest.replays, "replay"]];
  for (const [label, items, kind] of groups) {
    if (!items.length) continue;
    const g = document.createElement("optgroup"); g.label = label;
    for (const it of items) { const o = document.createElement("option"); o.value = it.id; o.textContent = kind === "live" ? `${it.id}  (${it.agents} founders${it.world_size !== 1024 ? `, ${it.world_size}²` : ""})` : it.name; o.dataset.kind = kind; g.appendChild(o); }
    sel.appendChild(g);
  }
  requestAnimationFrame(loop);
  const replayId = params.get("replay");
  if (replayId) {
    const rep = state.manifest.replays.find((x) => x.id === replayId);
    if (rep) { selectEntry(sel, "replay", rep.id); await loadReplay(rep); return; }
  }
  const want = params.get("scenario") || "predator-prey";
  const entry = state.manifest.scenarios.find((s) => s.id === want) || state.manifest.scenarios[0];
  if (!entry) { fail("No scenarios staged", new Error("web/scenarios is empty")); return; }
  selectEntry(sel, "live", entry.id);
  const seed = params.has("seed") ? Number(params.get("seed")) : undefined;
  $("seed").value = seed ?? entry.seed; $("desc").textContent = entry.description;
  await loadScenario(entry, seed);
}
/** Debug handle (devtools / the headless render check): `__atlas.state.source.meta()` etc. */
window.__atlas = { stage, layers, state, select, deselect };
boot();
