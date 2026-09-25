// Entity layers drawn over the terrain: instanced agents, fading combat /
// trade segments, settlement villages, trade hubs, codex-event rings and the
// selection marker. Every layer is fed flat buffers from a source (sources.js)
// and a `heightAt(x, y)` sampler so things sit on the ground.

import * as THREE from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";
import { AGENT, AGENT_FLAG } from "./sim.js";
import { hsv, mix, MOOD_COLORS, KIND_COLOR, kindOf, speciesHue } from "./palette.js";

const _m = new THREE.Matrix4(), _p = new THREE.Vector3(), _q = new THREE.Quaternion(), _s = new THREE.Vector3();
const _c = new THREE.Color();
const Y_AXIS = new THREE.Vector3(0, 1, 0);

/** Figure part ids baked into `aPart` for the gait shader. */
const PART = Object.freeze({ BODY: 0, FL: 1, FR: 2, BL: 3, BR: 4, HEAD: 5, TAIL: 6 });

/** Tag every vertex of `geo` with a part id and the pivot it swings about. */
function tag(geo, part, pivot = [0, 0, 0]) {
  const n = geo.attributes.position.count;
  const parts = new Float32Array(n).fill(part);
  const pivots = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) { pivots[i * 3] = pivot[0]; pivots[i * 3 + 1] = pivot[1]; pivots[i * 3 + 2] = pivot[2]; }
  geo.setAttribute("aPart", new THREE.BufferAttribute(parts, 1));
  geo.setAttribute("aPivot", new THREE.BufferAttribute(pivots, 3));
  return geo;
}

/** Four legs under a body, each tagged with its corner so diagonal pairs swing together. */
function legs(spread, len, r = 0.075) {
  const out = [];
  for (const [x, z, part] of [[spread, 0.28, PART.FL], [spread, -0.28, PART.FR], [-spread, 0.28, PART.BL], [-spread, -0.28, PART.BR]]) {
    const l = new THREE.CylinderGeometry(r, r * 0.8, len, 5);
    l.translate(x, len / 2, z);
    out.push(tag(l, part, [x, len, z]));
  }
  return out;
}

/** Bake a soft top-down shade into vertex colours (multiplied by the instance colour). */
function shade(geo) {
  const n = geo.attributes.position.count, nrm = geo.attributes.normal.array, col = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    const v = 0.74 + 0.26 * Math.min(1, Math.max(0, nrm[i * 3 + 1] * 0.5 + 0.5));
    col[i * 3] = v; col[i * 3 + 1] = v; col[i * 3 + 2] = v;
  }
  geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
  return geo;
}

/** Grazer facing +x: a rounded body on legs, a lowered head with two ears, a stub tail. */
function grazerGeometry() {
  const body = new THREE.SphereGeometry(0.5, 10, 7);
  body.scale(1.3, 0.78, 0.88);
  body.translate(0, 0.72, 0);
  tag(body, PART.BODY);
  const neck = new THREE.CylinderGeometry(0.16, 0.22, 0.5, 6);
  neck.rotateZ(-Math.PI / 3);
  neck.translate(0.62, 0.78, 0);
  const head = new THREE.SphereGeometry(0.26, 8, 6);
  head.scale(1.25, 0.9, 0.85);
  head.translate(0.92, 0.9, 0);
  const earL = new THREE.ConeGeometry(0.09, 0.26, 4);
  earL.translate(0.82, 1.2, 0.16);
  const earR = earL.clone();
  earR.translate(0, 0, -0.32);
  const neckPivot = [0.5, 0.72, 0];
  for (const g of [neck, head, earL, earR]) tag(g, PART.HEAD, neckPivot);
  const tail = new THREE.SphereGeometry(0.11, 6, 5);
  tail.translate(-0.66, 0.8, 0);
  tag(tail, PART.TAIL, [-0.6, 0.8, 0]);
  return shade(mergeGeometries([body, neck, head, earL, earR, tail, ...legs(0.42, 0.45)], false));
}

/** Hunter facing +x: a long low body, a pointed muzzle, pricked ears, a trailing tail. */
function hunterGeometry() {
  const body = new THREE.SphereGeometry(0.5, 10, 7);
  body.scale(1.55, 0.6, 0.62);
  body.translate(0, 0.62, 0);
  tag(body, PART.BODY);
  const head = new THREE.SphereGeometry(0.24, 8, 6);
  head.scale(1.1, 0.95, 0.9);
  head.translate(0.78, 0.74, 0);
  const muzzle = new THREE.ConeGeometry(0.17, 0.5, 6);
  muzzle.rotateZ(-Math.PI / 2);
  muzzle.translate(1.08, 0.7, 0);
  const earL = new THREE.ConeGeometry(0.08, 0.24, 4);
  earL.translate(0.72, 0.98, 0.13);
  const earR = earL.clone();
  earR.translate(0, 0, -0.26);
  const neckPivot = [0.6, 0.68, 0];
  for (const g of [head, muzzle, earL, earR]) tag(g, PART.HEAD, neckPivot);
  const tail = new THREE.ConeGeometry(0.1, 0.8, 5);
  tail.rotateZ(Math.PI / 2 + 0.5);
  tail.translate(-0.98, 0.72, 0);
  tag(tail, PART.TAIL, [-0.7, 0.66, 0]);
  return shade(mergeGeometries([body, head, muzzle, earL, earR, tail, ...legs(0.5, 0.42, 0.065)], false));
}

/**
 * Gait shader: a vertex-shader patch on the figure material. Each instance
 * carries `aGait = (phase, moving)`; legs swing about their hips in diagonal
 * pairs, the head nods (or grazes when standing), the tail wags and the body
 * bobs — all in object space before the instance matrix, so 10k figures
 * animate for free.
 */
function gaitMaterial(uniforms) {
  const mat = new THREE.MeshStandardMaterial({ roughness: 0.72, metalness: 0.04, flatShading: true, vertexColors: true });
  mat.customProgramCacheKey = () => "atlas-gait";
  mat.onBeforeCompile = (shader) => {
    shader.uniforms.uTime = uniforms.uTime;
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", `#include <common>
        uniform float uTime; attribute float aPart; attribute vec3 aPivot; attribute vec2 aGait;
        vec3 swingXY(vec3 p, vec3 pv, float a) { vec2 d = p.xy - pv.xy; float c = cos(a), s = sin(a); return vec3(pv.x + c * d.x - s * d.y, pv.y + s * d.x + c * d.y, p.z); }
        vec3 swingXZ(vec3 p, vec3 pv, float a) { vec2 d = p.xz - pv.xz; float c = cos(a), s = sin(a); return vec3(pv.x + c * d.x - s * d.y, p.y, pv.z + s * d.x + c * d.y); }`)
      .replace("#include <begin_vertex>", `#include <begin_vertex>
        {
          float g = min(aGait.y, 1.0);                             // stride amplitude
          float ph = uTime * 11.0 * max(aGait.y, 1.0) + aGait.x;   // fleeing / fighting figures sprint
          float sw = sin(ph);
          if (aPart > 0.5 && aPart < 4.5) {
            float sgn = (aPart < 1.5 || aPart > 3.5) ? 1.0 : -1.0;      // FL + BR vs FR + BL
            transformed = swingXY(transformed, aPivot, sw * sgn * 0.55 * g);
          } else if (aPart > 4.5 && aPart < 5.5) {
            float nod = g > 0.5 ? sin(ph * 2.0) * 0.07 : (-0.22 + 0.18 * sin(uTime * 1.1 + aGait.x));   // trot nod, or graze
            transformed = swingXY(transformed, aPivot, nod);
          } else if (aPart > 5.5) {
            transformed = swingXZ(transformed, aPivot, sin(ph * 1.3 + aGait.x) * (0.15 + 0.3 * g));
          }
          if (aPart < 0.5 || aPart > 4.5) transformed.y += abs(sw) * 0.05 * g;
        }`);
  };
  return mat;
}

/** Fill a geometry's vertex colours with one tint (multiplied by the instance colour). */
function tint(geo, r, g, b) {
  const n = geo.attributes.position.count, col = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) { col[i * 3] = r; col[i * 3 + 1] = g; col[i * 3 + 2] = b; }
  geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
  return geo;
}

/** Hut: mud walls under a pitched thatch roof with a dark doorway. Unit footprint, ~1.1 tall. */
function hutGeometry() {
  const wall = tint(new THREE.BoxGeometry(0.9, 0.5, 0.75).translate(0, 0.25, 0), 0.82, 0.72, 0.58);
  const roof = new THREE.CylinderGeometry(0, 0.75, 0.55, 4, 1);
  roof.rotateY(Math.PI / 4);
  roof.scale(1, 1, 0.85);
  roof.translate(0, 0.5 + 0.275, 0);
  tint(roof, 0.62, 0.48, 0.26);
  const door = tint(new THREE.BoxGeometry(0.06, 0.32, 0.22).translate(0.44, 0.16, 0), 0.18, 0.13, 0.10);
  return mergeGeometries([wall, roof, door], false);
}

/** Market stall: a counter with goods under a sloped awning on four poles, and a pennant. */
function stallGeometry() {
  const counter = tint(new THREE.BoxGeometry(1.2, 0.4, 0.7).translate(0, 0.2, 0), 0.55, 0.38, 0.22);
  const awning = new THREE.BoxGeometry(1.5, 0.06, 1.1);
  awning.rotateX(0.28);
  awning.translate(0, 1.05, 0.1);
  tint(awning, 0.85, 0.42, 0.22);
  const poles = [[-0.62, 0.52], [0.62, 0.52], [-0.62, -0.42], [0.62, -0.42]]
    .map(([x, z]) => tint(new THREE.CylinderGeometry(0.04, 0.04, 1.0, 5).translate(x, 0.5, z), 0.45, 0.32, 0.2));
  const goods = [[-0.3, 0, 0.85, 0.65, 0.25], [0.05, 0.1, 0.6, 0.2, 0.2], [0.35, -0.08, 0.3, 0.55, 0.3]]
    .map(([x, z, r, g, b]) => tint(new THREE.SphereGeometry(0.13, 6, 5).translate(x, 0.5, z), r, g, b));
  const pole = tint(new THREE.CylinderGeometry(0.04, 0.04, 1.9, 5).translate(0.62, 0.95, -0.42), 0.45, 0.32, 0.2);
  const flag = tint(new THREE.BoxGeometry(0.4, 0.22, 0.03).translate(0.82, 1.75, -0.42), 0.9, 0.85, 0.7);
  return mergeGeometries([counter, awning, ...poles, ...goods, pole, flag], false);
}

// ---------------------------------------------------------------------------
export const COLOR_MODES = ["species", "diet", "dialect", "energy", "mood", "arousal", "infection"];

export class Agents {
  constructor(max = 16384) {
    this.max = max;
    this.uniforms = { uTime: { value: 0 } };
    const withGait = (geo) => {
      geo.setAttribute("aGait", new THREE.InstancedBufferAttribute(new Float32Array(max * 2), 2).setUsage(THREE.DynamicDrawUsage));
      return geo;
    };
    this.grazers = new THREE.InstancedMesh(withGait(grazerGeometry()), gaitMaterial(this.uniforms), max);
    this.hunters = new THREE.InstancedMesh(withGait(hunterGeometry()), gaitMaterial(this.uniforms), max);
    /** Both figure meshes; each carries its own `userData.ids` (instance → agent id) for picking. */
    this.meshes = [this.grazers, this.hunters];
    for (const m of this.meshes) {
      m.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
      m.count = 0;
      m.frustumCulled = false;
      m.castShadow = true;
      m.receiveShadow = true;
      m.name = "agents";
      m.userData.ids = new Int32Array(max);
    }
    this.mode = "species";
    this.baseScale = 1;
    /** Ids seen by the previous update (id → slot in `prevBuf`), for birth / death effects. */
    this.prev = null;
    this.spare = new Map();   // the other id map, reused frame to frame instead of reallocated
    this.buf = new Float32Array(max * 2);
    this.prevBuf = new Float32Array(max * 2);
    /** Flat [x, y, …] of agents that appeared / vanished since the previous update. */
    this.born = [];
    this.died = [];
    /** Selected agent id, or -1. */
    this.selected = -1;
    this.selectedPos = null;
    this.marker = new THREE.Mesh(
      new THREE.RingGeometry(1.25, 1.55, 28).rotateX(-Math.PI / 2),
      new THREE.MeshBasicMaterial({ color: 0xe9e1d2, transparent: true, opacity: 0.9, depthWrite: false }),
    );
    this.marker.visible = false;
  }

  /** Base scale follows the biome cell size (8 units on every standard tier), so a
   *  1024² and a 4096² world with the same cell draw the same figure. */
  setWorldSize(ws, cell = ws / 128) { this.baseScale = Math.max(1.6, cell * 0.39); this.marker.scale.setScalar(this.baseScale * 1.4); }

  color(row, o, live) {
    switch (this.mode) {
      case "diet": return mix(0x7fbf5a, 0xd64a3a, row[o + AGENT.DIET]);
      case "dialect": return live ? hsv(row[o + AGENT.DIALECT_HUE], 0.75, 0.95) : hsv(speciesHue(row[o + AGENT.SPECIES]), 0.6, 0.9);
      case "energy": { const e = Math.min(1, row[o + AGENT.ENERGY] / 120); return mix(0x3a2a22, 0xf2d27a, e); }
      case "mood": return MOOD_COLORS[row[o + AGENT.MOOD] | 0] ?? 0xb7ac98;
      case "arousal": return mix(0x4a6b8a, 0xff5a2a, Math.min(1, row[o + AGENT.AROUSAL]));
      case "infection": return mix(0xb7ac98, 0x77d64a, Math.min(1, row[o + AGENT.INFECTION]));
      default: {
        let c = hsv(row[o + AGENT.HUE], row[o + AGENT.SAT], row[o + AGENT.VAL]);
        if ((row[o + AGENT.FLAGS] & AGENT_FLAG.LIVESTOCK) !== 0) c = mix(c, 0xf6f2e6, 0.45); // tamed: bleached
        return c;
      }
    }
  }

  /**
   * @param {{count:number,data:Float32Array,stride:number}} a
   * @param {(x:number,y:number)=>number} heightAt
   * @param {boolean} live  whether genome colour columns are populated
   */
  /** Advance the gait clock (seconds). */
  setTime(t) { this.uniforms.uTime.value = t; }

  /** Forget the previous population (new world, or a jump in time) so nothing reads as born or dead. */
  reset() { if (this.prev) { this.prev.clear(); this.spare = this.prev; } this.prev = null; this.born.length = 0; this.died.length = 0; }

  update(a, heightAt, live) {
    const n = Math.min(a.count, this.max), d = a.data, s = a.stride;
    const counts = [0, 0];
    this.selectedPos = null;
    const gaits = this.meshes.map((m) => m.geometry.attributes.aGait.array);
    const prev = this.prev, cur = this.spare, buf = this.buf, born = this.born, died = this.died;
    cur.clear(); born.length = 0; died.length = 0;
    for (let k = 0; k < n; k++) {
      const o = k * s, x = d[o + AGENT.X], y = d[o + AGENT.Y];
      const h = heightAt(x, y);
      const sc = this.baseScale * (0.55 + 0.45 * d[o + AGENT.SIZE]);
      const id = d[o + AGENT.ID] | 0;
      const rot = d[o + AGENT.ROT];
      const asleep = (d[o + AGENT.FLAGS] & AGENT_FLAG.ASLEEP) !== 0;
      _p.set(x, Math.max(h, 0) + 0.02, y);
      _q.setFromAxisAngle(Y_AXIS, -rot);
      _s.set(sc, asleep ? sc * 0.6 : sc, sc);
      _m.compose(_p, _q, _s);
      const kind = d[o + AGENT.DIET] >= 0.5 ? 1 : 0;
      const mesh = this.meshes[kind], i = counts[kind]++;
      mesh.setMatrixAt(i, _m);
      mesh.setColorAt(i, _c.setHex(this.color(d, o, live)));
      mesh.userData.ids[i] = id;
      // Per-instance gait: a phase from the id (herds never march in step) and
      // whether the figure is moving (rotation is only written for movers).
      const mood = d[o + AGENT.MOOD] | 0;
      gaits[kind][i * 2] = (id * 1.7) % 6.283;
      gaits[kind][i * 2 + 1] = asleep || rot === 0 ? 0 : (mood === 4 || mood === 5 ? 1.7 : 1);   // flee / fight: sprint
      if (id === this.selected) this.selectedPos = _p.clone();
      cur.set(id, k); buf[k * 2] = x; buf[k * 2 + 1] = y;
      if (prev && !prev.has(id)) born.push(x, y);
    }
    if (prev) { const pb = this.prevBuf; for (const [id, j] of prev) if (!cur.has(id)) died.push(pb[j * 2], pb[j * 2 + 1]); }
    this.spare = prev || new Map(); this.prev = cur; this.buf = this.prevBuf; this.prevBuf = buf;
    for (let kind = 0; kind < 2; kind++) {
      const mesh = this.meshes[kind];
      mesh.count = counts[kind];
      mesh.instanceMatrix.needsUpdate = true;
      mesh.geometry.attributes.aGait.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    }
    if (this.selectedPos) {
      this.marker.position.copy(this.selectedPos).setY(this.selectedPos.y + 0.15);
      this.marker.visible = true;
    } else {
      this.marker.visible = false;
    }
  }
}

// ---------------------------------------------------------------------------
/** Fading line segments (combat streaks or trade routes) with a lifetime in ticks. */
export class Segments {
  constructor({ max = 6000, life = 14, sat = 0.7, additive = true, lift = 1.2, width = 1 } = {}) {
    this.max = max; this.life = life; this.sat = sat; this.lift = lift;
    this.items = []; // {x1,y1,x2,y2,hue,born}
    const pos = new Float32Array(max * 6), col = new Float32Array(max * 6);
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.BufferAttribute(pos, 3).setUsage(THREE.DynamicDrawUsage));
    geo.setAttribute("color", new THREE.BufferAttribute(col, 3).setUsage(THREE.DynamicDrawUsage));
    geo.setDrawRange(0, 0);
    this.lines = new THREE.LineSegments(geo, new THREE.LineBasicMaterial({
      vertexColors: true, transparent: true, depthWrite: false, linewidth: width,
      blending: additive ? THREE.AdditiveBlending : THREE.NormalBlending, opacity: additive ? 1 : 0.75,
    }));
    this.lines.frustumCulled = false;
  }
  /** Append this tick's segments (rows [x1,y1,x2,y2,hue]) stamped with `tick`. */
  push(seg, tick, stride = 5) {
    for (let k = 0; k < seg.count; k++) {
      const o = k * stride;
      if (this.items.length >= this.max) this.items.shift();
      this.items.push({ x1: seg.data[o], y1: seg.data[o + 1], x2: seg.data[o + 2], y2: seg.data[o + 3], hue: seg.data[o + 4], born: tick });
    }
  }
  update(tick, heightAt) {
    const items = this.items;
    while (items.length && tick - items[0].born > this.life) items.shift();
    const pos = this.lines.geometry.attributes.position.array, col = this.lines.geometry.attributes.color.array;
    let n = 0;
    for (const it of items) {
      const age = Math.max(0, tick - it.born) / this.life, fade = (1 - age) * (1 - age);
      if (fade <= 0) continue;
      _c.setHex(hsv(it.hue, this.sat, 1.0)).multiplyScalar(fade);
      const o = n * 6;
      pos[o] = it.x1; pos[o + 1] = heightAt(it.x1, it.y1) + this.lift; pos[o + 2] = it.y1;
      pos[o + 3] = it.x2; pos[o + 4] = heightAt(it.x2, it.y2) + this.lift; pos[o + 5] = it.y2;
      col[o] = col[o + 3] = _c.r; col[o + 1] = col[o + 4] = _c.g; col[o + 2] = col[o + 5] = _c.b;
      n++;
    }
    this.lines.geometry.setDrawRange(0, n * 2);
    this.lines.geometry.attributes.position.needsUpdate = true;
    this.lines.geometry.attributes.color.needsUpdate = true;
  }
  clear() { this.items.length = 0; this.lines.geometry.setDrawRange(0, 0); }
}

// ---------------------------------------------------------------------------
/** Hut clusters at settlement sites; villages grow in and linger/fade out. */
export class Villages {
  constructor(max = 1024) {
    this.max = max;
    this.mesh = new THREE.InstancedMesh(hutGeometry(), new THREE.MeshStandardMaterial({ roughness: 0.9, flatShading: true, vertexColors: true }), max);
    this.mesh.count = 0; this.mesh.frustumCulled = false; this.mesh.name = "villages";
    this.mesh.castShadow = true; this.mesh.receiveShadow = true;
    this.sites = new Map(); // sid → {x,y,n,born,seen}
    this.scale = 1;
    this.LINGER = 300; this.FADE = 100; this.GROW = 40;
    this.lastTick = -1;
  }
  setWorldSize(ws, cell = ws / 128) { this.scale = Math.max(2.6, cell * 0.58); }
  /** `instant`: sites first seen now are drawn fully grown (the capture harness's fast-forward). */
  update(sites, tick, heightAt, stride = 4, instant = false) {
    // After a jump in time (a fast-forward, or the first frame of a world) a
    // site that is already there was not founded this tick: draw it grown.
    const jump = instant || this.lastTick < 0 || tick - this.lastTick > 8;
    this.lastTick = tick;
    for (let k = 0; k < sites.count; k++) {
      const o = k * stride, sid = sites.data[o];
      const v = this.sites.get(sid) || { born: jump ? tick - this.GROW : tick };
      v.x = sites.data[o + 1]; v.y = sites.data[o + 2]; v.n = sites.data[o + 3]; v.seen = tick;
      this.sites.set(sid, v);
    }
    let i = 0;
    for (const [sid, v] of this.sites) {
      const stale = tick - v.seen;
      if (stale > this.LINGER || stale < 0) { this.sites.delete(sid); continue; }
      const fade = Math.min(1, (this.LINGER - stale) / this.FADE);
      const grow = Math.min(1, Math.max(0, (tick - v.born) / this.GROW));
      const ease = (1 - Math.pow(1 - grow, 3)) * fade;
      v.ease = ease;
      const huts = Math.max(1, Math.min(Math.floor(v.n / 8), 9));
      _c.setHex(mix(hsv(speciesHue(sid), 0.5, 0.8), 0x8a6a3c, 0.55));
      for (let h = 0; h < huts && i < this.max; h++) {
        const ang = sid * 2.39996 + h * 2.39996, r = (0.9 + (h % 3) * 0.55) * this.scale * (h === 0 ? 0 : 1);
        const x = v.x + Math.cos(ang) * r, y = v.y + Math.sin(ang) * r;
        _p.set(x, heightAt(x, y), y);
        _q.setFromAxisAngle(Y_AXIS, ang);
        const sc = this.scale * (h === 0 ? 1.35 : 1) * ease;
        _s.set(sc, sc, sc);
        _m.compose(_p, _q, _s);
        this.mesh.setMatrixAt(i, _m);
        this.mesh.setColorAt(i, _c);
        i++;
      }
    }
    this.mesh.count = i;
    this.mesh.instanceMatrix.needsUpdate = true;
    if (this.mesh.instanceColor) this.mesh.instanceColor.needsUpdate = true;
  }
  /** Live sites for the hearth-smoke emitter: `[{x, y, n, ease}]`. */
  centers() { return [...this.sites.values()].filter((v) => v.ease > 0.6); }
  clear() { this.sites.clear(); this.mesh.count = 0; this.lastTick = -1; }
}

// ---------------------------------------------------------------------------
/** Fixed trade hubs (markets) — static after load. */
export class Hubs {
  constructor(max = 128) {
    this.mesh = new THREE.InstancedMesh(stallGeometry(), new THREE.MeshStandardMaterial({ roughness: 0.8, flatShading: true, vertexColors: true }), max);
    this.mesh.count = 0; this.mesh.frustumCulled = false; this.mesh.name = "hubs";
    this.mesh.castShadow = true; this.mesh.receiveShadow = true;
    this.max = max;
  }
  set(hubs, cell, heightAt, stride = 3) {
    const sc = Math.max(3.5, cell * 0.72);
    const n = Math.min(hubs.count, this.max);
    for (let k = 0; k < n; k++) {
      const x = hubs.data[k * stride], y = hubs.data[k * stride + 1];
      _p.set(x, heightAt(x, y), y);
      _q.setFromAxisAngle(Y_AXIS, k * 1.3);
      _s.set(sc, sc, sc);
      this.mesh.setMatrixAt(k, _m.compose(_p, _q, _s));
    }
    this.mesh.count = n;
    this.mesh.instanceMatrix.needsUpdate = true;
  }
}

// ---------------------------------------------------------------------------
const hashf = (a, b) => { let h = (a * 374761393 + b * 668265263) | 0; h = Math.imul(h ^ (h >>> 13), 1274126177); return ((h ^ (h >>> 16)) >>> 0) / 4294967296; };

/** Two wing triangles meeting at the body, facing +x; `aWing` is the span fraction the flap lifts. */
function birdGeometry() {
  const g = new THREE.BufferGeometry();
  // Wing tips ride 0.3 above the body (a dihedral) so the V reads from a low camera too.
  g.setAttribute("position", new THREE.BufferAttribute(new Float32Array([
    0.18, 0, 0, -0.18, 0, 0, -0.05, 0.3, 0.55,
    0.18, 0, 0, -0.05, 0.3, -0.55, -0.18, 0, 0,
  ]), 3));
  g.setAttribute("aWing", new THREE.BufferAttribute(new Float32Array([0, 0, 1, 0, 1, 0]), 1));
  g.computeVertexNormals();
  return g;
}

/** Bird flocks: dark V shapes circling over the map, wings flapping in the vertex shader. */
export class Birds {
  constructor(max = 600) {
    this.max = max;
    this.uniforms = { uTime: { value: 0 } };
    const mat = new THREE.MeshStandardMaterial({ color: 0x6a5c50, roughness: 0.9, flatShading: true, side: THREE.DoubleSide });
    mat.customProgramCacheKey = () => "atlas-bird";
    mat.onBeforeCompile = (shader) => {
      shader.uniforms.uTime = this.uniforms.uTime;
      shader.vertexShader = shader.vertexShader
        .replace("#include <common>", "#include <common>\nuniform float uTime; attribute float aWing;")
        .replace("#include <begin_vertex>", `#include <begin_vertex>
          {
            #ifdef USE_INSTANCING
              float ph = instanceMatrix[3].x * 0.7 + instanceMatrix[3].z * 0.3;
            #else
              float ph = 0.0;
            #endif
            transformed.y += sin(uTime * 9.0 + ph) * aWing * 0.55;
          }`);
    };
    this.mesh = new THREE.InstancedMesh(birdGeometry(), mat, max);
    this.mesh.count = 0; this.mesh.frustumCulled = false; this.mesh.name = "birds";
    this.flocks = [];
    this.scale = 1;
  }
  /** Seed flocks for a world: a few per 128² of cells, each circling a spot above the map. */
  setWorld(ws, res, cell, heightAt) {
    const n = Math.max(3, Math.min(40, Math.round(7 * (res * res) / (128 * 128))));
    this.flocks.length = 0;
    this.scale = cell * 0.55;
    let birds = 0;
    for (let f = 0; f < n; f++) {
      const h = (k) => hashf(f + 1, k);
      const cx = h(1) * ws, cz = h(2) * ws;
      const size = 5 + Math.floor(h(3) * 7);
      if (birds + size > this.max) break;
      const flock = {
        cx, cz, base: Math.max(0, heightAt(cx, cz)) + cell * (2.5 + 1.5 * h(4)), r: cell * (1.6 + 1.6 * h(5)),
        w: (0.22 + 0.2 * h(6)) * (h(7) < 0.5 ? 1 : -1), a0: h(8) * 6.283, birds: [],
      };
      for (let b = 0; b < size; b++) flock.birds.push({ da: (b - size / 2) * 0.12 + (h(20 + b) - 0.5) * 0.05, dr: (h(40 + b) - 0.5) * 0.5 * cell, dy: (h(60 + b) - 0.5) * 0.5 * cell });
      birds += size;
      this.flocks.push(flock);
    }
  }
  update(t) {
    this.uniforms.uTime.value = t;
    let i = 0;
    for (const f of this.flocks) {
      const a = f.a0 + f.w * t;
      for (const b of f.birds) {
        const ang = a + b.da, r = f.r + b.dr;
        _p.set(f.cx + Math.cos(ang) * r, f.base + b.dy + Math.sin(t * 0.7 + b.da * 9.0) * 0.3 * this.scale, f.cz + Math.sin(ang) * r);
        _q.setFromAxisAngle(Y_AXIS, -(ang + (f.w > 0 ? Math.PI / 2 : -Math.PI / 2)));   // heading along the circle
        _s.setScalar(this.scale);
        this.mesh.setMatrixAt(i++, _m.compose(_p, _q, _s));
      }
    }
    this.mesh.count = i;
    this.mesh.instanceMatrix.needsUpdate = true;
  }
}

// ---------------------------------------------------------------------------
/** A column of light fading upward: an open cylinder with a vertical alpha ramp. */
function pillarMaterial() {
  return new THREE.ShaderMaterial({
    uniforms: { uColor: { value: new THREE.Color(0xffffff) }, uAlpha: { value: 0 } },
    transparent: true, depthWrite: false, blending: THREE.AdditiveBlending, side: THREE.DoubleSide,
    vertexShader: /* glsl */ `varying vec2 vUv; void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }`,
    fragmentShader: /* glsl */ `uniform vec3 uColor; uniform float uAlpha; varying vec2 vUv;
      void main() { float a = pow(1.0 - vUv.y, 1.6) * uAlpha; gl_FragColor = vec4(uColor, a);
        #include <tonemapping_fragment>
        #include <colorspace_fragment>
      }`,
  });
}

/** Expanding rings and light pillars at codex-event locations, coloured by narrative kind. */
export class EventFx {
  constructor(pool = 48) {
    this.group = new THREE.Group();
    this.rings = [];
    const pillarGeo = new THREE.CylinderGeometry(1, 1, 1, 18, 1, true).translate(0, 0.5, 0);
    for (let i = 0; i < pool; i++) {
      const m = new THREE.Mesh(
        new THREE.RingGeometry(0.82, 1.0, 40).rotateX(-Math.PI / 2),
        new THREE.MeshBasicMaterial({ transparent: true, opacity: 0, depthWrite: false, side: THREE.DoubleSide }),
      );
      m.visible = false;
      const pillar = new THREE.Mesh(pillarGeo, pillarMaterial());
      pillar.visible = false;
      this.group.add(m, pillar);
      this.rings.push({ mesh: m, pillar, t0: 0, life: 0, kind: "mind", size: 1 });
    }
    this.next = 0;
    this.scale = 1;
    this.enabled = true;
  }
  setWorldSize(ws, cell = ws / 128) { this.scale = Math.max(5, cell * 2.5); }
  spawn(ev, now, heightAt) {
    if (!this.enabled || (!ev.x && !ev.y)) return;
    const r = this.rings[this.next++ % this.rings.length];
    const kind = kindOf(ev.type);
    r.kind = kind; r.t0 = now; r.life = kind === "war" ? 2.2 : 3.0; r.size = kind === "war" || kind === "fire" ? 1.4 : 1;
    const h = heightAt(ev.x, ev.y);
    r.mesh.position.set(ev.x, h + 0.8, ev.y);
    r.mesh.material.color.setHex(KIND_COLOR[kind]);
    r.mesh.visible = true;
    r.pillar.position.set(ev.x, h, ev.y);
    r.pillar.material.uniforms.uColor.value.setHex(KIND_COLOR[kind]);
    r.pillar.visible = true;
  }
  update(now) {
    for (const r of this.rings) {
      if (!r.mesh.visible) continue;
      const t = (now - r.t0) / r.life;
      if (t >= 1) { r.mesh.visible = false; r.pillar.visible = false; continue; }
      const s = this.scale * r.size * (0.15 + 0.85 * Math.sqrt(t));
      r.mesh.scale.set(s, 1, s);
      r.mesh.material.opacity = (1 - t) * (1 - t) * 0.95;
      const pr = this.scale * r.size * 0.22 * (1 + 0.6 * t), ph = this.scale * r.size * (2.2 + 1.5 * t);
      r.pillar.scale.set(pr, ph, pr);
      r.pillar.material.uniforms.uAlpha.value = Math.min(1, t * 6) * (1 - t) * 0.55;
    }
  }
}
