#!/usr/bin/env node
// Unit test for terrain.js's colour → terrain logic, fed the real bytes of
// crates/anabios-core/src/biome.rs `cell_color` (re-derived below, quantised
// the way the wasm view and the replay recorder do):
//
// - classifyTerrain (recorded replays have no id grid): every terrain at zero,
//   half and full biomass classifies as itself — a replay opens at carrying
//   capacity, where nearest-base once drew every forest as grassland and lush
//   desert as savanna; and, when the wasm module is built, the live
//   out-of-africa-saga colour grid classifies to the sim's own ids.
// - scarredBare (Forest.refresh): healthy savanna is not bare (the old
//   browner-than-green test flagged it all and shrank every savanna tree and
//   tuft); a burn scar (SUCCESSION_BARE) is bare on every planted terrain,
//   including the green-leaning burnt forest / grass / taiga / rainforest the
//   old test missed.
//
//   node web/test/classify-terrain.mjs   (live part needs scripts/web.sh wasm)

import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { classifyTerrain, scarredBare, Forest, T } from "../src/terrain.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");
const NAMES = ["water", "grass", "forest", "desert", "rock", "savanna", "rainforest", "taiga", "tundra"];

function check(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

// biome.rs `cell_color`, re-derived independently of terrain.js's tables.
const lerp = (a, b, t) => a.map((v, i) => v + (b[i] - v) * t);
const BASE = {
  [T.WATER]: [0.09, 0.19, 0.44], [T.GRASS]: [0.21, 0.44, 0.19], [T.FOREST]: [0.07, 0.26, 0.11],
  [T.DESERT]: [0.68, 0.58, 0.33], [T.ROCK]: [0.42, 0.40, 0.45], [T.SAVANNA]: [0.72, 0.66, 0.36],
  [T.RAINFOREST]: [0.06, 0.34, 0.16], [T.TAIGA]: [0.16, 0.34, 0.26], [T.TUNDRA]: [0.62, 0.66, 0.62],
};
const LUSH = { [T.GRASS]: [[0.42, 0.80, 0.33], 0.55], [T.FOREST]: [[0.20, 0.55, 0.24], 0.55], [T.DESERT]: [[0.86, 0.78, 0.52], 0.45] };
/** Linear RGB of a cell: terrain, biomass fraction, succession ("bare"/"pioneer"/null), pollution fraction. */
function cellColor(t, frac, succession = null, pol = 0) {
  let c = LUSH[t] ? lerp(BASE[t], LUSH[t][0], frac * LUSH[t][1]) : BASE[t];
  if (succession === "bare") c = lerp(c, [0.36, 0.24, 0.13], 0.65);
  if (succession === "pioneer") c = lerp(c, [0.55, 0.82, 0.30], 0.45);
  if (pol > 0) c = lerp(c, [0.32, 0.28, 0.24], pol * 0.55);
  return c;
}
/** view.rs `cell_view_rgba8` quantisation (truncating). */
const u8 = (c) => c.map((v) => Math.floor(Math.min(1, Math.max(0, v)) * 255));
const grid = (cols) => { const a = new Uint8Array(cols.length * 4); cols.forEach((c, k) => a.set([...c, 255], k * 4)); return a; };

// classifyTerrain: each terrain at 0, ½ and full biomass is itself.
for (let t = 0; t < 9; t++) {
  for (const frac of [0, 0.5, 1]) {
    const c = u8(cellColor(t, frac));
    const got = classifyTerrain(grid([c]), 1)[0];
    check(got === t, `${NAMES[t]} at biomass ${frac} (${c}) classifies as ${NAMES[t]}, not ${NAMES[got]}`);
  }
}

// scarredBare: healthy ground stands, a burn scar or a heavy smudge is bare.
const PLANTED = [T.GRASS, T.FOREST, T.SAVANNA, T.RAINFOREST, T.TAIGA, T.TUNDRA, T.DESERT];
for (const t of PLANTED) {
  for (const frac of [0, 0.5, 1]) {
    const ok = u8(cellColor(t, frac)), burnt = u8(cellColor(t, frac, "bare"));
    check(!scarredBare(...ok, t), `healthy ${NAMES[t]} at biomass ${frac} (${ok}) is not bare`);
    check(scarredBare(...burnt, t), `burnt ${NAMES[t]} at biomass ${frac} (${burnt}) is bare`);
    const pioneer = u8(cellColor(t, frac, "pioneer"));
    check(!scarredBare(...pioneer, t), `pioneer regrowth on ${NAMES[t]} (${pioneer}) is not bare`);
    check(!scarredBare(...u8(cellColor(t, frac, null, 0.3)), t), `lightly polluted ${NAMES[t]} is not bare`);
    check(scarredBare(...u8(cellColor(t, frac, null, 1)), t), `${NAMES[t]} smudged at the pollution cap is bare`);
  }
}
// The old browner-than-green test's fixture scar still reads bare on forest.
check(scarredBare(150, 100, 40, T.FOREST), "the synthetic (150,100,40) forest scar is bare");

// Forest.refresh end to end: a savanna plain keeps its full-size tufts and
// trees; burnt forest shrinks.
{
  const res = 8, cell = 8, half = (res * res) / 2;
  const ids = new Uint8Array(res * res).fill(T.SAVANNA).fill(T.FOREST, half);
  const terrain = { res, cell, terrainIds: ids, heightAt: () => 0, uniforms: { uCloud: { value: 1 } } };
  const f = new Forest(terrain, 4000);
  const cols = [...ids].map((t, k) => u8(cellColor(t, 1, k >= half + res ? "bare" : null)));
  f.refresh(grid(cols));
  const sav = f.items.filter((it) => ids[it.cell] === T.SAVANNA && it.kind !== "rock");
  const burnt = f.items.filter((it) => it.cell >= half + res && it.kind !== "rock");
  const green = f.items.filter((it) => it.cell >= half && it.cell < half + res && it.kind !== "rock");
  check(sav.length > 50 && sav.every((it) => !it.bare && it.sc === it.base), `savanna planting stands full size (${sav.length} items)`);
  check(burnt.length > 0 && burnt.every((it) => it.bare && it.sc < it.base * 0.5), `burnt forest shrinks (${burnt.length} items)`);
  check(green.length > 0 && green.every((it) => !it.bare && it.sc === it.base), "unburnt forest stands full size");
}

// The live saga: its colour grid classifies back to the sim's own ids (river
// cells are Water ids and their tint lands on water).
const wasmPath = resolve(root, "web/wasm/anabios.wasm");
let live = "skipped (no web/wasm/anabios.wasm; run scripts/web.sh wasm)";
if (existsSync(wasmPath)) {
  const { AnabiosSim } = await import("../src/sim.js");
  const sim = await AnabiosSim.fromBytes(readFileSync(wasmPath));
  sim.load(readFileSync(resolve(root, "scenarios/out-of-africa-saga.toml"), "utf8"), 318);
  const res = sim.biomeRes(), ids = sim.terrain(), got = classifyTerrain(sim.biomeRgba(), res);
  const miss = new Map();
  for (let k = 0; k < res * res; k++) {
    if (got[k] !== ids[k]) { const key = `${NAMES[ids[k]]}->${NAMES[got[k]]}`; miss.set(key, (miss.get(key) || 0) + 1); }
  }
  const n = [...miss.values()].reduce((a, b) => a + b, 0);
  check(n === 0, `live saga colours classify to the sim's ids (${n} of ${res * res} differ: ${[...miss].map(([k, v]) => `${k} ${v}`).join(", ")})`);
  live = `live saga ${res}² classified exactly`;
}

console.log(`classify-terrain: lush/bare/pioneer/smudge palette ok, savanna stands, burns shrink; ${live}`);
