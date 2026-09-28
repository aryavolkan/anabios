#!/usr/bin/env node
// Unit test for the pure body-scale clamp used by layers.js's Agents.update:
// readable far away, clamped down toward the physical collision diameter as
// the camera closes in, never below it or above `readable`.
//
//   node web/test/body-scale.mjs

import { bodyScale } from "../src/layers.js";

function check(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

// Far camera: legible (LEGIBLE_PX · unitsPerPixel) is huge, so the clamp
// never bites and the current readable size wins.
check(bodyScale(2.4, 0.9, 1000) === 2.4, "far camera keeps the readable size");

// Close camera: legible shrinks below physDiam; the result still never drops
// under the physical body diameter (bodies must never look smaller than the
// collision resolve actually keeps them apart).
check(bodyScale(2.4, 0.9, 0.2) === 0.9, "close camera clamps to the physical diameter, not below it");

// Mid-range: legible sits between physDiam and readable — the legible pixel
// budget wins over both.
check(bodyScale(2.4, 0.9, 1.5) === 1.5, "mid camera clamps to the legible size");

// Degenerate: legible below physDiam even though readable is smaller than
// physDiam would never happen in practice (readable >= physDiam by
// construction), but the clamp must still never go under physDiam.
check(bodyScale(0.5, 0.9, 0.1) === 0.9, "never below the physical diameter even if readable is smaller");

console.log("body-scale test: OK");
