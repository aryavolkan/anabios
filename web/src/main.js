// anabios atlas — three.js frontend entry point.
//
// One render loop drives either a live wasm world (LiveSource) or a recorded
// replay (ReplaySource). Each frame: advance time by the chosen speed, read the
// flat buffers, push them through the layers, and refresh the HUD on a cadence.

import * as THREE from "three";
import { createStage } from "./scene.js";
import { Terrain } from "./terrain.js";
import { Agents, Segments, Villages, Hubs, EventFx, COLOR_MODES } from "./layers.js";
import { openLive, openReplay } from "./sources.js";
import { kindOf, KIND_CSS, TERRAIN, MOOD_COLORS, cssHex, hsv, speciesHue } from "./palette.js";
import { WORLD_FLAG } from "./sim.js";

const $ = (id) => document.getElementById(id);
const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const params = new URLSearchParams(location.search);

// ---------------------------------------------------------------------------
// Stage + layers (built once; the world is swapped underneath them)
// ---------------------------------------------------------------------------
const stage = createStage($("view"));
const layers = {
  agents: new Agents(),
  streaks: new Segments({ life: 14, sat: 0.75, additive: true, lift: 1.4 }),
  trades: new Segments({ life: 24, sat: 0.55, additive: false, lift: 0.9 }),
  villages: new Villages(),
  hubs: new Hubs(),
  fx: new EventFx(),
};
const world = new THREE.Group();
stage.scene.add(world, layers.agents.mesh, layers.agents.marker, layers.streaks.lines, layers.trades.lines,
  layers.villages.mesh, layers.hubs.mesh, layers.fx.group);
layers.fx.enabled = !reduceMotion;

const state = {
  source: null,
  terrain: null,
  manifest: { scenarios: [], replays: [] },
  speed: Number(params.get("speed")) || 1,          // ticks per 60 Hz frame
  paused: params.get("paused") === "1",
  colorMode: params.get("color") || "species",
  selected: -1,
  follow: false,
  lastColorTick: -1,
  lastStatsTick: -1,
  fps: 0,
  rate: 0,
  frames: 0,
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
    history.replaceState(null, "", `?scenario=${encodeURIComponent(entry.id)}${seed !== undefined && seed !== "" ? `&seed=${seed}` : ""}`);
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
    history.replaceState(null, "", `?replay=${encodeURIComponent(entry.id)}`);
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
  state.terrain = new Terrain(source.biomeRes, ws, source.seaLevel, source.elevation());
  state.terrain.updateColors(source.biomeRgba());
  world.add(state.terrain.group);
  stage.fit(ws);
  stage.frame();
  for (const l of [layers.agents, layers.villages, layers.fx]) l.setWorldSize(ws);
  layers.streaks.clear(); layers.trades.clear(); layers.villages.clear();
  layers.hubs.set(source.hubs(), ws, heightAt);
  state.selected = -1; state.follow = false; $("card").classList.remove("show");
  state.lastColorTick = -1; state.lastStatsTick = -1;
  $("codex").innerHTML = "";
  applyLayerToggles();
  const isLive = source.kind === "live";
  $("badge").classList.toggle("replay", !isLive);
  $("badge-text").textContent = isLive ? "live · wasm" : "recorded replay";
  $("desc").textContent = entry.description || (isLive ? "" : "A deterministic replay recorded by anabios-headless; the world is played back frame by frame.");
  $("seed").value = source.meta().seed;
  buildColorModes(source);
  refreshStats(true);
}

function heightAt(x, y) { return state.terrain ? state.terrain.heightAt(x, y) : 0; }

// ---------------------------------------------------------------------------
// Frame loop
// ---------------------------------------------------------------------------
let last = performance.now();
function loop(now) {
  requestAnimationFrame(loop);
  const dt = Math.min(0.1, (now - last) / 1000); last = now;
  const src = state.source;
  stage.tick(dt);
  if (state.terrain) state.terrain.water.update(reduceMotion ? 0 : now / 1000);
  layers.fx.update(now / 1000);

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
    if (stepped > 0 || fractional || state.sinceStep === 0) {
      const tick = src.tick;
      layers.agents.update(src.agents(), heightAt, src.kind === "live");
      if (stepped > 0 || src.kind === "replay") {
        layers.streaks.push(src.streaks(), tick);
        layers.trades.push(src.trades(), tick);
        layers.villages.update(src.sites(), tick, heightAt);
        for (const ev of src.events()) onEvent(ev, now / 1000);
      }
      if (Math.floor(tick / 10) !== Math.floor(state.lastColorTick / 10)) {
        state.terrain.updateColors(src.biomeRgba());
        state.lastColorTick = tick;
      }
      if (Math.floor(tick / 30) !== Math.floor(state.lastStatsTick / 30) || stepped === 0) {
        refreshStats(false);
        state.lastStatsTick = tick;
      }
      state.sinceStep = 1;
    }
    layers.streaks.update(src.tick, heightAt);
    layers.trades.update(src.tick, heightAt);
    if (state.follow && layers.agents.selectedPos) {
      const p = layers.agents.selectedPos, before = stage.controls.target.clone();
      stage.controls.target.lerp(p, 0.15);
      stage.camera.position.add(stage.controls.target.clone().sub(before)); // keep the camera offset
    }
    if (src.kind === "replay") $("progress-fill").style.width = `${(100 * src.tick / src.endTick).toFixed(2)}%`;
  }

  stage.renderer.render(stage.scene, stage.camera);

  state.frames++;
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
    `<tr data-sid="${s.id}" title="click: colour by species, fly to a member"><td><span class="sw" style="background:${cssHex(hsv(speciesHue(s.id), 0.6, 0.9))}"></span>${esc(s.name)}</td><td>${s.count.toLocaleString()}</td><td>${s.tech_era ? "era " + s.tech_era : ""}</td></tr>`);
  $("species").innerHTML = rows.join("");
  $("receipt").innerHTML = `<b>${esc(meta.scenario)}</b> · seed <b>${meta.seed}</b>` +
    (meta.fingerprint ? ` · fingerprint <b>${meta.fingerprint}</b>` : "") +
    (meta.state_hash ? `<br>state hash <b>${meta.state_hash}</b>` : "") +
    (src.kind === "live" ? `<br>${src.stepMs.toFixed(1)} ms per step batch · deterministic per seed` : `<br>${meta.ticks?.toLocaleString()} ticks · sampled every ${meta.sample} · stride ${meta.stride}`);
  if (state.selected >= 0 && (full || Math.floor(src.tick) % 15 === 0)) renderCard();
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
    default: L.innerHTML = `<div class="item">body colour from the genome's hue/sat/val slots; livestock bleached</div>`;
  }
}

function onEvent(ev, now) {
  const kind = kindOf(ev.type);
  layers.fx.spawn(ev, now, heightAt);
  const line = document.createElement("div");
  line.className = "codex-line";
  const who = ev.sid == null ? "" : ` — ${esc(state.source.labels.get(ev.sid) || "species " + ev.sid)}`;
  line.innerHTML = `<span class="tick">${Math.floor(ev.tick).toLocaleString()}</span> · <span class="sw" style="background:${KIND_CSS[kind]}"></span><span style="color:${KIND_CSS[kind]}">${esc(ev.type.replace(/_/g, " "))}</span><span class="tick">${who}</span>`;
  if (ev.x || ev.y) line.onclick = () => stage.flyTo(ev.x, heightAt(ev.x, ev.y), ev.y, state.source.worldSize * 0.18);
  const feed = $("codex");
  feed.prepend(line);
  requestAnimationFrame(() => line.classList.add("show"));
  while (feed.children.length > 7) feed.removeChild(feed.lastChild);
}

function renderCard() {
  const src = state.source; if (!src || state.selected < 0) return;
  const a = src.agent(state.selected);
  if (!a) { deselect(); toast(`agent ${state.selected} died`); return; }
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
    row("mood", a.mood + (a.asleep ? " (asleep)" : ""));
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
    const on = cb.checked;
    switch (cb.dataset.layer) {
      case "relief": state.terrain?.setRelief(on); break;
      case "water": if (state.terrain) state.terrain.water.mesh.visible = on; break;
      case "streaks": layers.streaks.lines.visible = on; break;
      case "trades": layers.trades.lines.visible = on; break;
      case "villages": layers.villages.mesh.visible = on; break;
      case "hubs": layers.hubs.mesh.visible = on; break;
      case "events": layers.fx.group.visible = on; layers.fx.enabled = on && !reduceMotion; break;
      case "wire": if (state.terrain) state.terrain.material.wireframe = on; break;
    }
  }
}

function setSpeed(s) {
  state.speed = s;
  for (const b of document.querySelectorAll(".speed")) b.classList.toggle("on", Number(b.dataset.speed) === s);
}

function setPaused(p) { state.paused = p; $("play").textContent = p ? "▶" : "❚❚"; }

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
  const hits = raycaster.intersectObject(layers.agents.mesh, false);
  if (hits.length && hits[0].instanceId !== undefined) select(layers.agents.ids[hits[0].instanceId]);
  else deselect();
});

window.addEventListener("keydown", (e) => {
  if (e.target.closest("input,select,textarea")) return;
  const speeds = [0.25, 1, 4, 16, 64];
  switch (e.code) {
    case "Space": e.preventDefault(); setPaused(!state.paused); break;
    case "KeyF": stage.frame(); break;
    case "KeyH": document.body.classList.toggle("hide-hud"); break;
    case "KeyC": { const opts = Array.from($("color-mode").options).map((o) => o.value); state.colorMode = opts[(opts.indexOf(state.colorMode) + 1) % opts.length]; $("color-mode").value = state.colorMode; layers.agents.mode = state.colorMode; renderLegend(); break; }
    case "KeyL": if (state.selected >= 0) { state.follow = !state.follow; $("follow").classList.toggle("on", state.follow); } break;
    case "Escape": deselect(); break;
    default: if (/^Digit[1-5]$/.test(e.code)) setSpeed(speeds[Number(e.code[5]) - 1]);
  }
});

$("play").onclick = () => setPaused(!state.paused);
$("frame").onclick = () => stage.frame();
for (const b of document.querySelectorAll(".speed")) b.onclick = () => setSpeed(Number(b.dataset.speed));
$("color-mode").onchange = (e) => { state.colorMode = e.target.value; layers.agents.mode = state.colorMode; renderLegend(); };
$("layers").addEventListener("change", applyLayerToggles);
$("rail-toggle").onclick = () => { const r = $("rail"); r.classList.toggle("collapsed"); $("rail-toggle").textContent = r.classList.contains("collapsed") ? "+" : "−"; };
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
  const s = state.manifest.scenarios.find((x) => x.id === opt.value);
  if (s) { $("seed").value = s.seed; $("desc").textContent = s.description; }
};
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
    if (rep) { sel.value = rep.id; await loadReplay(rep); return; }
  }
  const want = params.get("scenario") || "predator-prey";
  const entry = state.manifest.scenarios.find((s) => s.id === want) || state.manifest.scenarios[0];
  if (!entry) { fail("No scenarios staged", new Error("web/scenarios is empty")); return; }
  sel.value = entry.id;
  const seed = params.has("seed") ? Number(params.get("seed")) : undefined;
  $("seed").value = seed ?? entry.seed; $("desc").textContent = entry.description;
  await loadScenario(entry, seed);
}
/** Debug handle (devtools / the headless render check): `__atlas.state.source.meta()` etc. */
window.__atlas = { stage, layers, state, select, deselect };
boot();
