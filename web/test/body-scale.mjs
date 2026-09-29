#!/usr/bin/env node
// Unit test for the body-scale clamp used by layers.js's Agents.update:
// readable far away, clamped down toward the physical collision body as the
// camera closes in, never below it or above `readable` — measured in
// footprint units, so a figure that is two units long at scale 1 shrinks to
// scale ≈ 0.55 (not 1.1) when its physical diameter is 1.1.
//
//   node web/test/body-scale.mjs

import { bodyScale, figureFootprint, Agents } from "../src/layers.js";

function check(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

// Far camera: legible (LEGIBLE_PX · unitsPerPixel) is huge, so the clamp
// never bites and the current readable size wins.
check(bodyScale(2.4, 0.9, 1000) === 2.4, "far camera keeps the readable size");

// Close camera: legible shrinks below the physical scale; the result still
// never drops under it (bodies must never look smaller than the collision
// resolve actually keeps them apart).
check(bodyScale(2.4, 0.9, 0.2) === 0.9, "close camera clamps to the physical scale, not below it");

// Mid-range: legible sits between the physical scale and readable — the
// legible pixel budget wins over both.
check(bodyScale(2.4, 0.9, 1.5) === 1.5, "mid camera clamps to the legible size");

// Degenerate: legible below the physical scale even though readable is
// smaller than it would never happen in practice (readable >= physical by
// construction), but the clamp must still never go under the physical scale.
check(bodyScale(0.5, 0.9, 0.1) === 0.9, "never below the physical scale even if readable is smaller");

// Footprints: the figure geometries are stylised well past a unit disc, and
// the clamp has to know by how much. Both are at least 1.5 units long at
// scale 1 (the grazer ~2.0, the hunter ~2.7), and the hunter is the longer.
const agents = new Agents(4);
const [grazer, hunter] = agents.footprint;
check(grazer > 1.5 && grazer < 2.5, `grazer footprint is ~2 units at scale 1 (got ${grazer.toFixed(3)})`);
check(hunter > grazer && hunter < 3.2, `hunter footprint is longer than the grazer's (got ${hunter.toFixed(3)})`);
check(Math.abs(figureFootprint(agents.grazers.geometry) - grazer) < 1e-9, "figureFootprint matches the cached per-kind value");

// The property the clamp exists for: two default-size bodies (physical
// diameter 1.15) drawn at the close-camera scale span exactly their physical
// diameter nose to tail, so bodies the resolve keeps 1.15 apart touch on
// screen instead of overlapping by half a figure.
const physDiam = 2 * (0.4 + 0.35 * 0.5);
for (const [name, fp] of [["grazer", grazer], ["hunter", hunter]]) {
  const sc = bodyScale(2.4, physDiam / fp, 0.2);
  check(Math.abs(sc * fp - physDiam) < 1e-9, `${name} drawn footprint at close range equals the physical diameter (got ${(sc * fp).toFixed(3)})`);
  check(sc < physDiam, `${name} close-range scale is below the raw diameter (got ${sc.toFixed(3)})`);
}

console.log("body-scale test: OK");
