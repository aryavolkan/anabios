#!/usr/bin/env node
// Unit test for layers.js's Villages: a village is pinned where it is founded
// and does not slide with the wandering anchor centroid; it moves only when
// the centroid has left for good, leaving the old huts to fade in place; hut
// counts do not flicker at a band edge; only hominids build huts; and huts and
// market stalls are seated on slopes without floating.
//
//   node web/test/villages.mjs

import * as THREE from "three";
import { Villages, Hubs, HUT_FOOT, STALL_FOOT } from "../src/layers.js";
import { AGENT } from "../src/sim.js";

function check(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

const flat = () => 0;
const site = (sid, x, y, n) => ({ count: 1, data: new Float32Array([sid, x, y, n]) });
/** Agent rows (AGENT layout) for `[species, diet]` pairs. */
function agents(pairs) {
  const stride = 17, data = new Float32Array(pairs.length * stride);
  pairs.forEach(([sid, diet], k) => { data[k * stride + AGENT.SPECIES] = sid; data[k * stride + AGENT.DIET] = diet; });
  return { count: pairs.length, data, stride };
}
/** A Villages layer on the 1024-unit / 128² world where `sids` are hominids. */
function villages(...sids) {
  const v = new Villages();
  v.setWorldSize(1024, 8);
  v.classify(agents(sids.map((sid) => [sid, 0.5])));
  return v;
}
/** World positions of every drawn hut instance (x, z). */
function huts(v) {
  const out = [], a = v.mesh.instanceMatrix.array;
  for (let i = 0; i < v.mesh.count; i++) out.push([a[i * 16 + 12], a[i * 16 + 14]]);
  return out;
}

// World of 1024 units on a 128² grid: cell 8, hut scale 4.64, radius ≈ 9.3.
const v = villages(3);
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
const w = villages(1);
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
const fast = villages(5);
fast.update(site(5, 300, 300, 40), 0, flat);
const fastHuts = huts(fast);
for (let t = 100, k = 0; t <= 3000; t += 100, k++) fast.update(site(5, 300 + (k % 2 ? 1 : -1) * R * 3, 300, 40), t, flat);
check(huts(fast).every(([x, z], i) => x === fastHuts[i][0] && z === fastHuts[i][1]), "fast play does not drag the huts after the centroid");

// Time jumps (fast-forward / replay seek) snap the village to its current home.
const j = villages(2);
j.update(site(2, 100, 100, 16), 0, flat);
j.update(site(2, 400, 100, 16), 5000, flat);
check(j.sites.get(2).x === 400, "a time jump snaps the village to the live site");

// Huts never stand on water: a lakeside village drops the huts over the lake.
const lake = villages(4);
lake.isWater = (x) => x > 600;
lake.update(site(4, 600, 300, 72), 0, flat);
check(huts(lake).every(([x]) => x <= 600), "no hut on a water cell");
check(huts(lake).length > 0 && huts(lake).length < 9, "shore huts kept, lake huts dropped");

// Huts stand apart: a hut's pyramid roof reaches ~0.70 scales from its
// centre and the 1.35× centre hut's ~0.94, so ring neighbours need ≥ 1.4
// scales between centres and the centre and a ring hut ≥ 1.64 (the old
// three-radius spiral put ring huts 0.9 scales from the centre, inside it).
for (let sid = 0; sid <= 40; sid++) {
  const sp = villages(sid);
  sp.update(site(sid, 500, 500, 80), 1000, flat, 4, true);
  const [c, ...ring] = huts(sp), S = sp.scale;
  check(ring.length === 8, `sid ${sid}: nine huts (got ${ring.length + 1})`);
  for (const [x, z] of ring) check(Math.hypot(x - c[0], z - c[1]) >= 1.64 * S, `sid ${sid}: a ring hut pierces the centre hut`);
  for (let a = 0; a < ring.length; a++) {
    for (let b = a + 1; b < ring.length; b++) {
      const d = Math.hypot(ring[a][0] - ring[b][0], ring[a][1] - ring[b][1]);
      check(d >= 1.4 * S, `sid ${sid}: ring huts ${a + 1} and ${b + 1} overlap (${(d / S).toFixed(2)} scales apart)`);
    }
  }
}

// Only hominids build huts: an omnivore lineage (diet ≈ 0.5, the culture
// archetypes) settles into a village, a settled grazing herd (diet ≈ 0) or
// hunting pack (≈ 1) does not.
const mixed = new Villages();
mixed.setWorldSize(1024, 8);
mixed.classify(agents([[1, 0.5], [1, 0.48], [1, 0.52], [2, 0.01], [2, 0.0], [3, 1.0], [3, 0.99]]));
const three = { count: 3, data: new Float32Array([1, 100, 100, 40, 2, 500, 500, 40, 3, 800, 800, 40]) };
mixed.update(three, 0, flat);
check([...mixed.sites.keys()].join() === "1", `only the hominid species gets a village (got ${[...mixed.sites.keys()]})`);
check(huts(mixed).every(([x]) => x < 200), "no huts at the herd or pack sites");

// A lineage drifting out of the omnivore band keeps its village until it has
// clearly left (hysteresis), then the village lingers and fades like any site.
const drift = villages(6);
const share = (inBand) => agents(Array.from({ length: 10 }, (_, k) => [6, k < inBand ? 0.5 : 0.05]));
drift.classify(share(4));
check(drift.hominids.has(6), "40% in band keeps an existing hominid");
drift.classify(share(2));
check(!drift.hominids.has(6), "20% in band drops it");
drift.classify(share(4));
check(!drift.hominids.has(6), "and 40% is not enough to rejoin");
drift.update(site(6, 300, 300, 40), 0, flat);
check(drift.sites.size === 0, "a non-hominid settlement founds no village");

// Seating on slopes: a building's lowest point (the foot of its plinth) sits
// at or below the ground everywhere under its footprint — nothing floats —
// its floor never rises above the highest ground there (no stilts), and on
// flat ground it sits exactly on the ground. Grounds: flat, a gentle and a
// steep plane (steeper than any plinth spans), and a bilinear grid like
// terrain.js's (cell 8, gradients up to ~0.7, the steepest found under a
// village or market in the shipped scenarios). The footprint is checked on a
// 9×9 grid, much finer than seatY's own 3×3.
{
  const CELL = 8, RES = 32, N = RES + 1, grid = new Float32Array(N * N);
  for (let j = 0; j < N; j++) for (let i = 0; i < N; i++) grid[j * N + i] = 5.5 * Math.sin(i * 0.9 + j * 0.5) + 2.5 * Math.cos(i * 0.4 - j * 1.3);
  const bilinear = (x, y) => {
    const fx = ((x / CELL) % RES + RES) % RES, fy = ((y / CELL) % RES + RES) % RES;
    const i = Math.floor(fx), j = Math.floor(fy), u = fx - i, w = fy - j;
    return (grid[j * N + i] * (1 - u) + grid[j * N + i + 1] * u) * (1 - w) + (grid[(j + 1) * N + i] * (1 - u) + grid[(j + 1) * N + i + 1] * u) * w;
  };
  const grounds = { flat: () => 2, gentle: (x, y) => 0.12 * x - 0.05 * y, steep: (x, y) => -0.7 * x + 0.4 * y, bilinear };

  /** Each instance of `mesh`: its floor, its lowest point, and local → world (x, z). */
  const placed = (mesh) => {
    const box = new THREE.Box3().setFromBufferAttribute(mesh.geometry.attributes.position), out = [];
    for (let i = 0; i < mesh.count; i++) {
      const m = new THREE.Matrix4(), p = new THREE.Vector3(), q = new THREE.Quaternion(), s = new THREE.Vector3();
      mesh.getMatrixAt(i, m);
      m.decompose(p, q, s);
      out.push({ floor: p.y, foot: p.y + s.y * box.min.y, at: (lx, lz) => new THREE.Vector3(lx, 0, lz).applyMatrix4(m) });
    }
    return out;
  };
  const under = (inst, foot, heightAt) => {
    let lo = Infinity, hi = -Infinity;
    for (let a = 0; a <= 8; a++) {
      for (let b = 0; b <= 8; b++) {
        const w = inst.at(foot.hx * (a / 4 - 1), foot.cz + foot.hz * (b / 4 - 1)), h = heightAt(w.x, w.z);
        lo = Math.min(lo, h); hi = Math.max(hi, h);
      }
    }
    return { lo, hi };
  };
  const seated = (label, list, foot, heightAt, flatGround) => {
    check(list.length > 0, `${label}: something was placed`);
    for (const inst of list) {
      const { lo, hi } = under(inst, foot, heightAt), eps = 1e-4 * Math.max(1, Math.abs(lo));
      check(inst.foot <= lo + eps, `${label}: plinth foot ${inst.foot.toFixed(3)} above the lowest ground ${lo.toFixed(3)} under it (floating)`);
      check(inst.floor <= hi + eps, `${label}: floor ${inst.floor.toFixed(3)} above the highest ground ${hi.toFixed(3)} under it (on stilts)`);
      if (flatGround) check(Math.abs(inst.floor - lo) < eps, `${label}: on flat ground the floor sits on the ground`);
      // A slope the plinth spans is terraced: the floor meets the uphill ground, so no wall is buried.
      if (label.endsWith("/gentle")) check(Math.abs(inst.floor - hi) < eps, `${label}: floor ${inst.floor.toFixed(3)} is terraced at the uphill ground ${hi.toFixed(3)}`);
    }
  };

  // Six nine-hut villages (different sids → different hut angles), fully grown.
  const sids = [3, 10, 17, 24, 31, 38];
  const many = { count: 6, data: new Float32Array(sids.flatMap((sid, k) => [sid, 40 + k * 37, 60 + k * 23, 80])) };
  for (const [name, heightAt] of Object.entries(grounds)) {
    const vv = villages(...sids);
    vv.update(many, 1000, heightAt, 4, true);
    check(vv.mesh.count === 6 * 9, `seat/${name}: nine huts a site (got ${vv.mesh.count})`);
    seated(`huts/${name}`, placed(vv.mesh), HUT_FOOT, heightAt, name === "flat");
    for (const c of vv.centers()) check(Number.isFinite(c.base), `huts/${name}: hearth smoke knows the centre hut's floor`);
    // Relief toggled while paused: layout() re-seats on the new ground.
    vv.layout(grounds.flat);
    seated(`huts/${name}→flat`, placed(vv.mesh), HUT_FOOT, grounds.flat, true);
  }
  // Market stalls: the same rule at stall scale.
  const hubs = { count: 5, data: new Float32Array([30, 200, 0, 75, 169, 0, 120, 138, 0, 165, 107, 0, 210, 76, 0]) };
  // Lifted clear of the water plane: ground below 0 is lake to the stall
  // placement (hubSpot), which would move the stall ashore or float it.
  for (const [name, ground] of Object.entries(grounds)) {
    const heightAt = (x, y) => ground(x, y) + 200;
    const hb = new Hubs(16);
    hb.set(hubs, CELL, heightAt);
    seated(`stalls/${name}`, placed(hb.mesh), STALL_FOOT, heightAt, name === "flat");
  }
  // The check has teeth: a hut seated at its centre height (the old rule) on
  // the gentle slope clears the downhill ground.
  const g = grounds.gentle, vv = villages(3);
  vv.update(site(3, 100, 100, 80), 1000, g, 4, true);
  const inst = placed(vv.mesh)[4], c = inst.at(0, 0);
  check(g(c.x, c.z) - under(inst, HUT_FOOT, g).lo > 0.25, "a centre-seated hut would float on the gentle slope");
}

console.log("villages: hominids only; pinned placement, relocation, hysteresis, hut spacing, shoreline and slope seating ok");
