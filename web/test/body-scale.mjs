#!/usr/bin/env node
// Unit test for the body-scale clamp used by layers.js's Agents.update:
// readable far away, clamped down toward the physical collision body as the
// camera closes in, never below it or above `readable` — measured in
// footprint units, so a figure that is two units long at scale 1 shrinks to
// scale ≈ 0.55 (not 1.1) when its physical diameter is 1.1.
//
//   node web/test/body-scale.mjs

import { bodyScale, figureFootprint, Agents, LEGIBLE_PX, BODY_DRAW } from "../src/layers.js";

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
// diameter 1.15) drawn at the close-camera scale span BODY_DRAW of their
// physical diameter nose to tail, so bodies the resolve keeps 1.15 apart
// show a margin on screen instead of overlapping by half a figure.
const physDiam = 2 * (0.4 + 0.35 * 0.5);
check(BODY_DRAW > 0.7 && BODY_DRAW <= 1, `BODY_DRAW is a margin just under the disc (got ${BODY_DRAW})`);
for (const [name, fp] of [["grazer", grazer], ["hunter", hunter]]) {
  const sc = bodyScale(2.4, BODY_DRAW * physDiam / fp, 0.2);
  check(Math.abs(sc * fp - BODY_DRAW * physDiam) < 1e-9, `${name} drawn footprint at close range is BODY_DRAW of the physical diameter (got ${(sc * fp).toFixed(3)})`);
  check(sc < physDiam, `${name} close-range scale is below the raw diameter (got ${sc.toFixed(3)})`);
}

// The legible floor is a floor on the drawn LENGTH (LEGIBLE_PX on screen),
// not on the instance scale: at 10 px per world unit two touching
// default-size bodies (1.15 units = 11.5 px apart) are drawn at their
// physical size and do not overlap, whichever figure they are. With the old
// scale floor (14 px of scale) they were drawn 28–38 px long at that zoom.
check(LEGIBLE_PX <= 12, `legible floor is a small figure, not a scale (got ${LEGIBLE_PX})`);
const unitsPerPixel = 0.1;
for (const [name, fp] of [["grazer", grazer], ["hunter", hunter]]) {
  const legible = LEGIBLE_PX * unitsPerPixel / fp;
  const sc = bodyScale(2.4, BODY_DRAW * physDiam / fp, legible);
  const drawnPx = sc * fp / unitsPerPixel;
  check(drawnPx <= physDiam / unitsPerPixel + 1e-9, `${name} at 10 px/unit is drawn no longer than its physical diameter (got ${drawnPx.toFixed(1)} px for ${(physDiam / unitsPerPixel).toFixed(1)} px of spacing)`);
  check(drawnPx >= LEGIBLE_PX - 1e-9, `${name} at 10 px/unit still spans at least LEGIBLE_PX (got ${drawnPx.toFixed(1)} px)`);
}
// Far away (1 px per world unit) the readable size still rules, capped by
// the legible floor from above: a body is drawn min(readable, LEGIBLE_PX)
// long — never a sub-pixel dot, never a figure larger than the floor.
for (const [name, fp] of [["grazer", grazer], ["hunter", hunter]]) {
  const legible = LEGIBLE_PX * 1.0 / fp;
  const sc = bodyScale(2.4, BODY_DRAW * physDiam / fp, legible);
  const want = Math.min(2.4 * fp, LEGIBLE_PX);
  check(Math.abs(sc * fp - want) < 1e-9, `${name} at 1 px/unit is drawn min(readable, LEGIBLE_PX) long (got ${(sc * fp).toFixed(1)}, want ${want.toFixed(1)})`);
}

console.log("body-scale test: OK");
