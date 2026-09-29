#!/usr/bin/env node
// Unit test for market-stall placement (layers.js hubSpot / Hubs): a sim hub
// whose stall footprint touches water is drawn on the nearest dry spot within
// reach, a hub already on dry ground is never moved, a hub with no dry ground
// near floats on the water plane, and the stalls are re-placed once the
// terrain ids arrive.
//
//   node web/test/hubs.mjs

import { hubSpot, Hubs, STALL_FOOTPRINT } from "../src/layers.js";

function check(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

const cell = 8, step = cell / 4, reach = cell * 4, r = 4;
// A lake: everything west of x = 100 is water.
const lakeDry = (x) => x >= 100;
const dry = (x, y) => lakeDry(x, y);
const footprintDry = (s) => [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1], [0.7, 0.7], [-0.7, -0.7]].every(([a, b]) => dry(s.x + a * r, s.y + b * r));

// Dry hub: untouched, exactly.
let s = hubSpot(150.3, 40.7, r, dry, step, reach);
check(!s.moved && !s.afloat && s.x === 150.3 && s.y === 40.7, "a hub on dry ground stays where the sim put it");

// Hub in the lake, 10 units from shore: nearest spot whose whole footprint is dry, straight east.
s = hubSpot(90, 40, r, dry, step, reach);
check(s.moved && !s.afloat, "a hub in the water is moved");
check(footprintDry(s), `moved stall footprint is dry (got ${s.x},${s.y})`);
check(s.y === 40 && s.x >= 100 + r && s.x < 100 + r + step, `moved to the nearest dry spot (got ${s.x})`);

// Centre on land but the awning over the water: moved just far enough inland.
s = hubSpot(102, 40, r, dry, step, reach);
check(s.moved && footprintDry(s) && s.x - 102 <= r, `a stall straddling the shore steps inland (got ${s.x})`);

// Open water, no land within reach: keeps its position and floats.
s = hubSpot(40, 40, r, dry, step, reach);
check(!s.moved && s.afloat && s.x === 40 && s.y === 40, "a hub in open water stays put, afloat");

// Plate edge: water along y < 10 near a corner hub; the nearest dry spot by
// plain distance would be off the plate (x < 0), so with a world size the
// stall stays on it.
const edgeDry = (x, y) => y >= 10 && x < 1000;
s = hubSpot(2, 5, r, edgeDry, step, reach, 1000);
check(s.moved && s.x >= r && s.y >= 10 + r, `a moved stall stays on the plate (got ${s.x},${s.y})`);

// Deterministic: the same inputs give the same spot.
const a = hubSpot(95, 17, r, dry, step, reach), b = hubSpot(95, 17, r, dry, step, reach);
check(a.x === b.x && a.y === b.y, "placement is deterministic");

// Hubs layer: ids unknown at set() (isWater answers false) → drawn at the sim
// position; once ids exist, layout() re-places. Heights below the water plane
// count as wet, and an afloat stall rides y = 0 instead of the lake bed.
let idsKnown = false;
const isWater = (x) => idsKnown && x < 100;
const heightAt = (x) => (x < 100 ? -5 : 3);
const hubs = new Hubs();
const data = new Float32Array([90, 40, 7, 150, 40, 7, 20, 20, 7]);
hubs.set({ count: 3, data }, cell, heightAt, isWater);
const y = (k) => hubs.mesh.instanceMatrix.array[k * 16 + 13], x = (k) => hubs.mesh.instanceMatrix.array[k * 16 + 12];
// Heights alone already flag the lake before the ids arrive.
check(hubs.spots[0].moved && x(0) >= 100 && y(0) === 3, "a stall under the water plane is moved before ids are known");
check(hubs.spots[2].afloat && y(2) === 0, "an open-water stall floats at the water plane");
check(!hubs.spots[1].moved && x(1) === 150, "dry stall untouched");
// Flat world (heights 0): only the ids can tell, so layout() must re-run once they exist.
hubs.set({ count: 3, data }, cell, () => 0, isWater);
check(!hubs.spots[0].moved && x(0) === 90, "without ids a flat-world stall stays at the sim position");
idsKnown = true;
hubs.layout();
check(hubs.spots[0].moved && x(0) >= 100 + cell * 0.72 * STALL_FOOTPRINT, `layout() re-places once ids exist (got ${x(0)})`);
check(hubs.spots[2].afloat && x(2) === 20, "open-water hub stays put after re-layout");
data.fill(0);   // the source view may be reused; the layer kept its own copy
hubs.layout();
check(x(1) === 150, "Hubs keeps a copy of the hub rows");

console.log("hubs: ok");
