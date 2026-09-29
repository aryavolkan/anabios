#!/usr/bin/env node
// Unit test for layers.js's Villages: a village is pinned where it is founded
// and does not slide with the wandering anchor centroid; it moves only when
// the centroid has left for good, leaving the old huts to fade in place; hut
// counts do not flicker at a band edge.
//
//   node web/test/villages.mjs

import { Villages } from "../src/layers.js";

function check(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

const flat = () => 0;
const site = (sid, x, y, n) => ({ count: 1, data: new Float32Array([sid, x, y, n]) });
/** World positions of every drawn hut instance (x, z). */
function huts(v) {
  const out = [], a = v.mesh.instanceMatrix.array;
  for (let i = 0; i < v.mesh.count; i++) out.push([a[i * 16 + 12], a[i * 16 + 14]]);
  return out;
}

// World of 1024 units on a 128² grid: cell 8, hut scale 4.64, radius ≈ 9.3.
const v = new Villages();
v.setWorldSize(1024, 8);
const R = v.radius;

// Founded at (500, 500), then the centroid jitters and wanders inside the
// relocation reach (and spikes out past it for fewer than DWELL ticks, as a
// cohort dying does): the huts never move.
let tick = 0;
v.update(site(3, 500, 500, 40), tick, flat);
const founded = huts(v);
check(founded.length === 5, `40 members → 5 huts (got ${founded.length})`);
for (tick = 1; tick <= 600; tick++) {
  const spike = tick > 300 && tick < 300 + v.DWELL / 2;
  const x = 500 + Math.sin(tick * 0.37) * R * 0.8 + (spike ? R * 6 : 0);
  v.update(site(3, x, 500 + Math.cos(tick * 0.21) * R * 0.8, 40), tick, flat);
}
const later = huts(v);
check(later.length === founded.length, "jitter keeps the hut count");
check(later.every(([x, z], i) => x === founded[i][0] && z === founded[i][1]), "jitter and a short spike never move the huts");

// The people move 8 radii east and stay: the village follows once, after the
// dwell, and the old huts linger as a fading, abandoned copy.
const far = 500 + R * 8;
let movedAt = -1;
for (let t = 0; t < 600; t++, tick++) {
  v.update(site(3, far, 500, 40), tick, flat);
  if (movedAt < 0 && v.sites.get(3).x !== 500) movedAt = tick;
}
const live = v.sites.get(3);
check(movedAt > 0, "a sustained move relocates the village");
check(Math.abs(live.x - far) < R, `relocated near the new home (x=${live.x.toFixed(1)}, want ≈${far.toFixed(1)})`);
check([...v.sites.keys()].length === 1, "the abandoned copy has faded out by now");

// Relocation leaves a ghost that fades, never a second live village.
const w = new Villages();
w.setWorldSize(1024, 8);
w.update(site(1, 200, 200, 40), 0, flat);
let ghostSeen = false;
for (let t = 1; t < 400; t++) {
  w.update(site(1, 200 + R * 8, 200, 40), t, flat);
  for (const [k, s] of w.sites) if (typeof k === "string") { ghostSeen = true; check(s.retired >= 0 && s.x === 200, "the ghost stays where the old village stood"); }
}
check(ghostSeen, "a relocation leaves the old huts to fade in place");

// Hut-count hysteresis: 8 members per hut, but a count that dips one hut
// below the band edge and back keeps the hut.
check(Villages.hutsFor(40) === 5, "40 → 5 huts");
check(Villages.hutsFor(39, 5) === 5, "one member short keeps the fifth hut");
check(Villages.hutsFor(31, 5) === 3, "two huts short drops them");
check(Villages.hutsFor(48, 5) === 6, "growth adds a hut at once");

// 64× play advances ~100 ticks a frame: that is not a time jump, so a
// jittering centroid still leaves the huts where they stand.
const fast = new Villages();
fast.setWorldSize(1024, 8);
fast.update(site(5, 300, 300, 40), 0, flat);
const fastHuts = huts(fast);
for (let t = 100, k = 0; t <= 3000; t += 100, k++) fast.update(site(5, 300 + (k % 2 ? 1 : -1) * R * 3, 300, 40), t, flat);
check(huts(fast).every(([x, z], i) => x === fastHuts[i][0] && z === fastHuts[i][1]), "fast play does not drag the huts after the centroid");

// Time jumps (fast-forward / replay seek) snap the village to its current home.
const j = new Villages();
j.setWorldSize(1024, 8);
j.update(site(2, 100, 100, 16), 0, flat);
j.update(site(2, 400, 100, 16), 5000, flat);
check(j.sites.get(2).x === 400, "a time jump snaps the village to the live site");

// Huts never stand on water: a lakeside village drops the huts over the lake.
const lake = new Villages();
lake.setWorldSize(1024, 8);
lake.isWater = (x) => x > 600;
lake.update(site(4, 600, 300, 72), 0, flat);
check(huts(lake).every(([x]) => x <= 600), "no hut on a water cell");
check(huts(lake).length > 0 && huts(lake).length < 9, "shore huts kept, lake huts dropped");

console.log("villages: pinned placement, relocation, hysteresis and shoreline ok");
