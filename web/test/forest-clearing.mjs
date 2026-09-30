#!/usr/bin/env node
// Unit test for terrain.js's Forest clearings: trees ease out of a village's
// clearing and grow back when it is released instead of popping in one frame;
// only the items in transition are touched; the pace follows sim ticks within
// a wall-time band (so a paused toggle still animates and 64× does not pop);
// the scarred-bare size composes with clearing; time jumps, reduced motion
// and rebuilds leave a settled, consistent forest. Also: the plate's edge
// vertices clamp to the edge cells (no torus average with the far side), and
// no tree is planted where the relief ground lies under the water plane.
//
//   node web/test/forest-clearing.mjs

import { Forest, Terrain, T, TRANSITION_TICKS, TRANSITION_MIN_S, TRANSITION_MAX_S } from "../src/terrain.js";
import { Villages } from "../src/layers.js";

function check(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

// A 16² all-forest grid of 8-unit cells, flat.
const res = 16, cell = 8;
const terrain = { res, cell, terrainIds: new Uint8Array(res * res).fill(T.FOREST), heightAt: () => 0, uniforms: { uCloud: { value: 1 } } };
const forest = () => new Forest(terrain, 4000);
/** Drawn scale of an item, read back from its instance matrix. */
const drawn = (it) => { const a = it.mesh.instanceMatrix.array, o = it.index * 16; return Math.hypot(a[o], a[o + 1], a[o + 2]); };
const near = (a, b) => Math.abs(a - b) < 1e-6;
const circle = { x: 64, y: 64, r: 12 };
const inside = (it, c = circle) => (it.x - c.x) ** 2 + (it.z - c.y) ** 2 < c.r * c.r;
/** Colour grid: green everywhere, brown (scarred bare) on `bare` cells. */
function rgba(bare = []) {
  const a = new Uint8Array(res * res * 4);
  for (let k = 0; k < res * res; k++) { a[k * 4] = 40; a[k * 4 + 1] = 120; a[k * 4 + 2] = 40; a[k * 4 + 3] = 255; }
  for (const k of bare) { a[k * 4] = 150; a[k * 4 + 1] = 100; }
  return a;
}
/** Advance `frames` frames of `dt` seconds and `dTicks` ticks each. */
const run = (f, frames, dt = 1 / 60, dTicks = 1) => { for (let i = 0; i < frames; i++) f.update(dt, dTicks); };

check(TRANSITION_TICKS === new Villages().GROW, "the forest eases at the huts' own grow-in pace (Villages.GROW)");

// A village is founded: its trees shrink away over GROW ticks, not in one frame.
{
  const f = forest();
  f.refresh(rgba());
  const hit = f.items.filter((it) => inside(it));
  check(hit.length > 3 && hit.length < f.items.length / 4, `a clearing covers a few trees (${hit.length} of ${f.items.length})`);
  const v0 = f.leaf.instanceMatrix.version;
  run(f, 5);
  check(f.leaf.instanceMatrix.version === v0, "a settled forest is not re-uploaded");
  f.setClearings([circle]);
  check(f.active.size === hit.length, `only the cleared items are animated (${f.active.size} vs ${hit.length})`);
  check(hit.every((it) => near(drawn(it), it.base)), "nothing vanishes on the frame the clearing is set");
  let prev = hit.map(drawn);
  for (let frame = 1; frame <= TRANSITION_TICKS; frame++) {
    f.update(1 / 60, 1);
    const now = hit.map(drawn);
    if (frame < TRANSITION_TICKS) check(now.every((s, i) => s > 0 && s < prev[i]), `frame ${frame}: trees shrink steadily`);
    prev = now;
  }
  check(hit.every((it) => drawn(it) === 0), "gone after GROW ticks at 1×");
  check(f.active.size === 0, "settled items leave the active set");
  check(f.items.filter((it) => !inside(it)).every((it) => near(drawn(it), it.base)), "trees outside the clearing are untouched");

  // The village expires: the clearing is released and the trees grow back.
  f.setClearings([]);
  check(hit.every((it) => drawn(it) === 0), "no tree pops back on release");
  f.update(1 / 60, 1);
  check(hit.every((it) => drawn(it) > 0 && drawn(it) < it.base * 0.2), "regrowth starts small");
  run(f, TRANSITION_TICKS);
  check(hit.every((it) => near(drawn(it), it.base)) && f.active.size === 0, "fully regrown and settled");
}

// Paused (no ticks): a layer toggle still animates, on wall time.
{
  const f = forest();
  f.setClearings([circle]);
  const hit = f.items.filter((it) => it.cleared);
  run(f, Math.floor(TRANSITION_MAX_S / 0.1) - 2, 0.1, 0);
  check(hit.every((it) => drawn(it) > 0 && drawn(it) < it.base), "paused: shrinking, not frozen and not popped");
  run(f, 3, 0.1, 0);
  check(hit.every((it) => drawn(it) === 0), `paused: done within ${TRANSITION_MAX_S}s`);
}

// 64×: 40 ticks is under a frame, but the transition still takes TRANSITION_MIN_S.
{
  const f = forest();
  f.setClearings([circle]);
  const hit = f.items.filter((it) => it.cleared);
  f.update(1 / 60, 64);
  check(hit.every((it) => drawn(it) > 0), "64×: no one-frame pop");
  run(f, Math.ceil(TRANSITION_MIN_S * 60), 1 / 60, 64);
  check(hit.every((it) => drawn(it) === 0), "64×: done in the minimum wall time");
}

// Reversal mid-transition continues from where the tree stands.
{
  const f = forest();
  f.setClearings([circle]);
  const hit = f.items.filter((it) => it.cleared);
  run(f, 15);
  const mid = hit.map(drawn);
  f.setClearings([]);
  check(hit.every((it, i) => drawn(it) === mid[i]), "reversal does not jump");
  f.update(1 / 60, 1);
  check(hit.every((it, i) => drawn(it) > mid[i]), "and grows back from there");
  run(f, TRANSITION_TICKS);
  check(hit.every((it) => near(drawn(it), it.base)), "reversed trees regrow fully");
}

// Scarred bare composes with clearing: cleared ? 0 : bare ? 0.12·base : base.
{
  const f = forest();
  const k = 8 * res + 8;   // the cell under the circle's centre
  f.refresh(rgba([k]));
  const bare = f.items.filter((it) => it.cell === k && it.kind !== "rock");
  check(bare.length > 0 && bare.every((it) => near(drawn(it), it.base * 0.12)), "the first refresh seats bare trees at once");
  f.setClearings([circle]);
  run(f, TRANSITION_TICKS);
  const bareHit = bare.filter((it) => it.cleared);
  check(bareHit.length > 0 && bareHit.every((it) => drawn(it) === 0), "a bare tree in a clearing goes to 0");
  f.refresh(rgba());   // the cell greens again while cleared
  check(f.active.size === 0 && bareHit.every((it) => drawn(it) === 0), "greening under a clearing keeps it cleared");
  f.refresh(rgba([k]));
  f.setClearings([]);
  run(f, TRANSITION_TICKS);
  check(bare.every((it) => near(drawn(it), it.base * 0.12)), "released onto a bare cell: regrows to the bare size, not full");
  f.refresh(rgba());
  check(bare.every((it) => near(drawn(it), it.base * 0.12)), "a greening cell eases up too");
  run(f, TRANSITION_TICKS);
  check(bare.every((it) => near(drawn(it), it.base)), "and reaches full size");
  f.refresh(rgba([k]), true);
  check(bare.every((it) => near(drawn(it), it.base * 0.12)) && f.active.size === 0, "a snapped refresh (time jump) seats at once");
}

// Time jumps and reduced motion: settled at once.
{
  const f = forest();
  f.setClearings([circle], true);
  check(f.items.filter((it) => inside(it)).every((it) => drawn(it) === 0) && f.active.size === 0, "a snapped clearing is made at once");
  f.setClearings([]);
  f.update(Infinity, 0);
  check(f.items.every((it) => near(drawn(it), it.base)) && f.active.size === 0, "update(Infinity) (reduced motion) finishes every transition");
}

// A rebuild mid-transition (relief toggle) leaves a consistent, settled forest.
{
  const f = forest();
  const k = 2 * res + 2;
  f.refresh(rgba([k]));
  f.setClearings([circle]);
  run(f, 10);
  f.rebuild();
  check(f.active.size === 0, "rebuild drops stale transitions");
  check(f.items.every((it) => near(drawn(it), inside(it) ? 0 : it.cell === k && it.kind !== "rock" ? it.base * 0.12 : it.base)),
    "after rebuild: cleared at 0, bare at 0.12·base (scars carried over), the rest full");
  f.setClearings([]);
  run(f, TRANSITION_TICKS);
  check(f.items.every((it) => near(drawn(it), it.cell === k && it.kind !== "rock" ? it.base * 0.12 : it.base)), "and it regrows from there");
}

// Uploads: after the first full upload, a transition sends only its span.
{
  const f = forest();
  const m = f.leaf.instanceMatrix;
  f.setClearings([circle]);
  f.update(1 / 60, 1);
  check(m.updateRanges.length === 0, "a pending full upload is not narrowed");
  m.onUploadCallback();   // as the renderer does once it has sent the buffer
  m.clearUpdateRanges();
  f.update(1 / 60, 1);
  check(m.updateRanges.length === 1, "one range per mesh per step");
  const r = m.updateRanges[0], full = f.leaf.count * 16;
  check(r.start % 16 === 0 && r.count > 0 && r.count < full, `the range covers the clearing, not the forest (${r.count} of ${full} floats)`);
  f.rebuild();
  check(m.updateRanges.length === 0 && f.fullUpload.has(f.leaf), "rebuild asks for a full upload again");
}

// Relief shores and plate edges (a real Terrain): an 8² grid, taiga in
// columns 0-3 (column 3 barely above the sea), deep sea in columns 4-7.
{
  const r = 8, sea = 0.35, el = new Float32Array(r * r), ids = new Uint8Array(r * r);
  for (let k = 0; k < r * r; k++) {
    const i = k % r;
    el[k] = i < 3 ? 0.6 : i === 3 ? sea + 0.001 : 0.0;
    ids[k] = i < 4 ? T.TAIGA : T.WATER;
  }
  const t = new Terrain(r, 64, sea, el, ids), n = r + 1;
  // The plate is drawn unwrapped: the x = 0 edge is taiga ground, the x = max
  // edge is sea floor. A torus average would sink the one and raise the other.
  for (let j = 0; j < n; j++) {
    check(t.heights[j * n] > 0, `edge vertex (0,${j}) stays land (${t.heights[j * n].toFixed(2)})`);
    check(t.heights[j * n + r] < 0, `edge vertex (${r},${j}) stays sea floor (${t.heights[j * n + r].toFixed(2)})`);
  }
  const trees = t.forest.items;
  check(trees.length > 0, `the inland taiga is still planted (${trees.length} trees)`);
  check(trees.every((it) => t.heightAt(it.x, it.z) >= 0), "no tree stands under the water plane");
  check(t.forest.cone.count === trees.filter((it) => it.kind === "cone").length, "skipped trees leave no holes in the instance range");
  const onRelief = trees.length;
  t.setRelief(false);
  check(t.forest.items.length > onRelief, `a flat world plants the shore column too (${t.forest.items.length} vs ${onRelief})`);
  t.dispose();
}

console.log("forest-clearing: eased clear/regrow, tick pace in a wall band, bare composition, snaps, rebuild and shore seating ok");
