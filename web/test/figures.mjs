#!/usr/bin/env node
// Unit test for the atlas's animal figures (layers.js Agents): the figure
// kind follows the diet bands (grazer / upright hominid / hunter), every
// figure's legs reach into its body instead of floating beside or below it,
// and in species colour mode each species wears the hue the species list
// shows — not the genome's neutral colour slots, which drew every species
// the same dull teal.
//
//   node web/test/figures.mjs

import { Agents, FIGURE, figureKind, figureFootprint } from "../src/layers.js";
import { AGENT } from "../src/sim.js";
import { hsv, speciesHue } from "../src/palette.js";

function check(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

// Kinds by diet: the omnivore band 0.34–0.66 is the hominid (the culture
// lineages run at diet 0.50 — the old `diet >= 0.5` cut drew them as hunters).
check(figureKind(0) === FIGURE.GRAZER && figureKind(0.33) === FIGURE.GRAZER, "herbivores are grazers");
check(figureKind(0.34) === FIGURE.HOMINID && figureKind(0.5) === FIGURE.HOMINID && figureKind(0.65) === FIGURE.HOMINID, "omnivores are hominids");
check(figureKind(0.66) === FIGURE.HUNTER && figureKind(1) === FIGURE.HUNTER, "carnivores are hunters");

const agents = new Agents(8);
check(agents.meshes.length === 3, "three figure meshes");

// Legs attach: each leg's top reaches up to body geometry right above it.
for (const [name, mesh] of [["grazer", agents.grazers], ["hunter", agents.hunters], ["hominid", agents.hominids]]) {
  const pos = mesh.geometry.attributes.position.array, part = mesh.geometry.attributes.aPart.array;
  const body = [];
  for (let i = 0; i < part.length; i++) if (part[i] === 0) body.push([pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]]);
  for (let p = 1; p <= 4; p++) {
    let top = -Infinity, cx = 0, cz = 0, n = 0;
    for (let i = 0; i < part.length; i++) if (part[i] === p) { top = Math.max(top, pos[i * 3 + 1]); cx += pos[i * 3]; cz += pos[i * 3 + 2]; n++; }
    cx /= n; cz /= n;
    const joined = body.some(([x, y, z]) => Math.hypot(x - cx, z - cz) < 0.15 && y <= top);
    check(joined, `${name} limb ${p} reaches into the body (top ${top.toFixed(2)} at ${cx.toFixed(2)},${cz.toFixed(2)})`);
  }
}

// The upright figure's close-range size counts its height, so a hominid
// clamped to the physical diameter is not drawn twice a quadruped's height.
for (const mesh of agents.meshes) {
  mesh.geometry.computeBoundingBox();
  const b = mesh.geometry.boundingBox, fp = figureFootprint(mesh.geometry);
  check((b.max.y - b.min.y) / fp <= 1.25 + 1e-9, `figure height at the physical clamp stays within 1.25 diameters (got ${((b.max.y - b.min.y) / fp).toFixed(2)})`);
}

// Species colour: a neutral genome (every colour slot 0.5) draws exactly the
// species list's swatch, and two species differ.
const row = (sid, hue = 0.5, sat = 0.5, val = 0.5) => {
  const r = new Float32Array(17); r[AGENT.SPECIES] = sid; r[AGENT.HUE] = hue; r[AGENT.SAT] = sat; r[AGENT.VAL] = val; return r;
};
for (const sid of [1, 2, 3, 7]) check(agents.color(row(sid), 0, true) === hsv(speciesHue(sid), 0.6, 0.9), `species ${sid} draws its swatch colour`);
check(agents.color(row(1), 0, true) !== agents.color(row(2), 0, true), "two species differ in colour");
check(agents.color(row(1, 0.6), 0, true) !== agents.color(row(1), 0, true), "the genome still varies individuals");

// Update sorts rows into the three meshes by diet.
const stride = 17, data = new Float32Array(stride * 3);
[[0.01, 1], [0.5, 2], [0.99, 3]].forEach(([diet, id], k) => { data[k * stride + AGENT.DIET] = diet; data[k * stride + AGENT.ID] = id; data[k * stride + AGENT.BODY] = 1.15; data[k * stride + AGENT.SIZE] = 0.5; });
agents.update({ count: 3, data, stride }, () => 0, true, 0.1);
check(agents.grazers.count === 1 && agents.hominids.count === 1 && agents.hunters.count === 1, "one figure per kind");
check(agents.hominids.userData.ids[0] === 2, "the omnivore is drawn as the hominid");

console.log("figures: diet kinds, attached legs, upright scale, species colours ok");
