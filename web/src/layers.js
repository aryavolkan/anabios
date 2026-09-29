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

/** Omnivore diet band read as hominid (game/scripts/mammal_sprites.gd HERB_MAX / CARN_MIN): the upright figure, and the lineages that build huts. */
export const HOMINID_DIET = [0.34, 0.66];
/** Figure kinds, indexed like `Agents.meshes`. */
export const FIGURE = Object.freeze({ GRAZER: 0, HUNTER: 1, HOMINID: 2 });

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

/**
 * Four legs under a body, each tagged with its corner so diagonal pairs swing
 * together. Each leg runs from the ground up to `top` — inside the body, so
 * no gap opens between leg and belly at any stride — and swings about its hip
 * at `hip`. The corners (±`x`, ±`z`) must sit well inside the body's plan
 * outline: the old legs stood outside the hunter's and below the grazer's
 * belly, so figures read as bodies floating over loose posts.
 */
function legs(x, z, top, hip, r = 0.075) {
  const out = [];
  for (const [lx, lz, part] of [[x, z, PART.FL], [x, -z, PART.FR], [-x, z, PART.BL], [-x, -z, PART.BR]]) {
    const l = new THREE.CylinderGeometry(r, r * 0.75, top, 5);
    l.translate(lx, top / 2, lz);
    out.push(tag(l, part, [lx, hip, lz]));
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
  return shade(mergeGeometries([body, neck, head, earL, earR, tail, ...legs(0.38, 0.2, 0.74, 0.55)], false));
}

/** Hunter facing +x: a canine — a lean body, a raised head with a short snout and pricked ears, a drooping brush tail. */
function hunterGeometry() {
  const body = new THREE.SphereGeometry(0.5, 10, 7);
  body.scale(1.4, 0.62, 0.6);
  body.translate(0, 0.66, 0);
  tag(body, PART.BODY);
  const neck = new THREE.CylinderGeometry(0.12, 0.17, 0.38, 6);
  neck.rotateZ(-Math.PI / 3.2);
  neck.translate(0.62, 0.8, 0);
  const head = new THREE.SphereGeometry(0.2, 8, 6);
  head.scale(1.15, 0.95, 0.9);
  head.translate(0.84, 0.94, 0);
  const snout = new THREE.ConeGeometry(0.1, 0.3, 6);
  snout.rotateZ(-Math.PI / 2);
  snout.translate(1.1, 0.9, 0);
  const earL = new THREE.ConeGeometry(0.07, 0.2, 4);
  earL.translate(0.78, 1.14, 0.09);
  const earR = earL.clone();
  earR.translate(0, 0, -0.18);
  const neckPivot = [0.55, 0.72, 0];
  for (const g of [neck, head, snout, earL, earR]) tag(g, PART.HEAD, neckPivot);
  // Brush tail: thick at the rump, tapering back and down (the cylinder's +y
  // end is the tip once rotated, hence radiusTop < radiusBottom).
  const tail = new THREE.CylinderGeometry(0.035, 0.1, 0.6, 5);
  tail.rotateZ(Math.PI / 2 + 0.45);
  tail.translate(-0.97, 0.57, 0);
  tag(tail, PART.TAIL, [-0.68, 0.7, 0]);
  return shade(mergeGeometries([body, neck, head, snout, earL, earR, tail, ...legs(0.42, 0.15, 0.66, 0.5, 0.065)], false));
}

/**
 * Hominid facing +x: an upright biped — legs, a torso broad at the shoulders,
 * a head, and arms hanging at the sides. It reuses the quadruped part ids so
 * the same gait shader walks it: arms are the fore pair (FL/FR) and legs the
 * hind pair (BL/BR), and since diagonal pairs swing together each arm swings
 * with the opposite leg, as a walking person's do.
 */
function hominidGeometry() {
  const torso = new THREE.CylinderGeometry(0.2, 0.15, 0.62, 7);
  torso.scale(0.75, 1, 1.1);
  torso.translate(0, 0.97, 0);
  tag(torso, PART.BODY);
  const head = new THREE.SphereGeometry(0.15, 8, 6);
  head.translate(0.03, 1.45, 0);
  tag(head, PART.HEAD, [0, 1.3, 0]);
  const parts = [torso, head];
  for (const [z, arm, leg] of [[1, PART.FL, PART.BL], [-1, PART.FR, PART.BR]]) {
    const l = new THREE.CylinderGeometry(0.07, 0.055, 0.74, 5);
    l.translate(0, 0.37, 0.1 * z);
    parts.push(tag(l, leg, [0, 0.7, 0.1 * z]));
    const a = new THREE.CylinderGeometry(0.05, 0.04, 0.6, 5);
    a.translate(0, 0.96, 0.25 * z);
    parts.push(tag(a, arm, [0, 1.24, 0.25 * z]));
  }
  return shade(mergeGeometries(parts, false));
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

// ---------------------------------------------------------------------------
// Seating buildings on slopes. A building is one rigid instance but the ground
// under its footprint is not level: seated at its centre height, the downhill
// side hung in the air (up to 0.4 local units — most of a hut's 0.5-tall wall
// — on tribes hillsides; a stall's whole counter on a steep shore) while the uphill
// side sank. Each building now stands on an earth plinth reaching `depth`
// local units below its floor, and is seated as high as that plinth allows:
// at the highest ground under the footprint (a terrace cut into the hill —
// walls, doorway and counter stay whole, the plinth shows downhill as a
// retaining wall), but never so high that the plinth's foot clears the lowest
// ground. So nothing floats, and only on slopes steeper than the plinth spans
// does the uphill side dip into the hill. Tilting to the slope was rejected:
// tipped huts and stalls read as sliding downhill. On flat ground the plinth
// is buried and the look is unchanged.

/** Slack kept at a plinth's foot for ground the samples miss (the drawn
 *  terrain is triangulated, not bilinear, and dips between samples). */
const PLINTH_SLACK = 0.08;
/** Ground-contact footprints in local (unit-scale) units: half extents, z
 *  centre, and plinth depth below the floor (a hut's about its wall height, so
 *  a tall retaining wall never dwarfs it; an open stall gets a deeper deck, as
 *  markets sit on steeper shores). */
export const HUT_FOOT = Object.freeze({ hx: 0.52, hz: 0.45, cz: 0, depth: 0.6 });
export const STALL_FOOT = Object.freeze({ hx: 0.72, hz: 0.55, cz: 0.05, depth: 0.9 });

/**
 * Floor height for a building at (x, y), turned `ang` about +y at instance
 * scale `sc` (see the seating note above). Samples the footprint's corners,
 * edge midpoints and centre: the ground is bilinear per cell and a footprint
 * is about a cell wide, so a 3×3 grid catches its low and high points.
 */
export function seatY(heightAt, x, y, ang, sc, foot) {
  const c = Math.cos(ang), s = Math.sin(ang);
  let lo = Infinity, hi = -Infinity;
  for (let a = -1; a <= 1; a++) {
    for (let b = -1; b <= 1; b++) {
      // Local (lx, lz) → world, as `_q.setFromAxisAngle(Y_AXIS, ang)` turns it (three's z is the sim's y).
      const lx = a * foot.hx * sc, lz = (foot.cz + b * foot.hz) * sc;
      const h = heightAt(x + c * lx + s * lz, y - s * lx + c * lz);
      if (h < lo) lo = h;
      if (h > hi) hi = h;
    }
  }
  return Math.min(hi, lo + (foot.depth - PLINTH_SLACK) * sc);
}

/** Earth plinth under a footprint: top flush with the floor, `foot.depth` deep. */
function plinth(foot, r, g, b) {
  return tint(new THREE.BoxGeometry(foot.hx * 2, foot.depth, foot.hz * 2).translate(0, 0.01 - foot.depth / 2, foot.cz), r, g, b);
}

/** Hut: mud walls under a pitched thatch roof with a dark doorway, on an earth
 *  plinth (see `seatY`). Unit footprint, ~1.1 tall above the floor. */
function hutGeometry() {
  const base = plinth(HUT_FOOT, 0.58, 0.5, 0.4);
  const wall = tint(new THREE.BoxGeometry(0.9, 0.5, 0.75).translate(0, 0.25, 0), 0.82, 0.72, 0.58);
  const roof = new THREE.CylinderGeometry(0, 0.75, 0.55, 4, 1);
  roof.rotateY(Math.PI / 4);
  roof.scale(1, 1, 0.85);
  roof.translate(0, 0.5 + 0.275, 0);
  tint(roof, 0.62, 0.48, 0.26);
  const door = tint(new THREE.BoxGeometry(0.06, 0.32, 0.22).translate(0.44, 0.16, 0), 0.18, 0.13, 0.10);
  return mergeGeometries([base, wall, roof, door], false);
}

/** Market stall: a counter with goods under a sloped awning on four poles, and
 *  a pennant, on a trodden-earth plinth (see `seatY`). */
function stallGeometry() {
  const base = plinth(STALL_FOOT, 0.5, 0.46, 0.4);
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
  return mergeGeometries([base, counter, awning, ...poles, ...goods, pole, flag], false);
}

// ---------------------------------------------------------------------------
// Body draw scale: the figure geometries are stylised well above the physical
// collision body (see crates/anabios-core/src/collision.rs BODY_R_BASE/
// BODY_R_SIZE — radius = BODY_R_BASE + BODY_R_SIZE · Size, Size ∈ [0,1]), so a
// distant herd reads clearly but two agents the collision resolve has already
// separated can still look stacked on screen. `bodyScale` keeps the current
// readable size while the camera is far away and shrinks each body toward its
// physical diameter as the camera closes in, never below it (bodies never
// look more overlapped than the physics actually is) and never above the
// readable size (distant herds stay legible).
//
// The clamp is expressed in *footprint* units, not raw instance scale: a
// figure at scale 1 is ~2 units long (grazer) to ~2.7 (hunter) nose to tail
// (`figureFootprint`), so the scale at which its drawn body equals the
// physical diameter is `physDiam / footprint`, not `physDiam`. Clamping the
// raw scale to the diameter drew every close-up body about twice its
// physical size and a resolved herd still read as a pile.
// The physical diameter itself rides in the row (`AGENT.BODY`, the sim's
// `2 · live_body_radius`): a juvenile's under the sim's `growth_enabled`
// knob, where the display size can no longer be inverted to it.

/**
 * Pixels of instance scale a body keeps at minimum, regardless of camera
 * distance — chosen so the default framing (a whole herd) still reads
 * clearly. Below it the physical floor takes over: from roughly 27 px per
 * world unit on, a figure is drawn exactly its physical diameter long, so two
 * bodies 1.15 units apart (two default-size agents touching) meet nose to
 * tail on screen instead of overlapping.
 */
const LEGIBLE_PX = 14;

/** Draw scale: readable far away, clamped down to the physical body as the
 *  camera closes in, but never smaller than it or larger than `readable`.
 *  `physScale` is the instance scale at which the figure's footprint equals
 *  its physical diameter (`AGENT.BODY / figureFootprint(geo)`). */
export function bodyScale(readable, physScale, legible) {
  return Math.max(physScale, Math.min(readable, legible));
}

/** Extent (world units at scale 1) of a figure geometry: the larger of its
 *  length (x, the facing axis) and width (z), so a drawn body scaled by
 *  `physDiam / footprint` never reaches past its physical disc — or 0.8 of
 *  its height, so the upright hominid (a small disc under a tall body) is
 *  not drawn twice a quadruped's height at close range. */
export function figureFootprint(geometry) {
  geometry.computeBoundingBox();
  const b = geometry.boundingBox;
  return Math.max(b.max.x - b.min.x, b.max.z - b.min.z, 0.8 * (b.max.y - b.min.y));
}

/** Figure kind by diet: grazer below the omnivore band, hominid inside it, hunter above (same band as the huts). */
export function figureKind(diet) {
  return diet < HOMINID_DIET[0] ? FIGURE.GRAZER : diet < HOMINID_DIET[1] ? FIGURE.HOMINID : FIGURE.HUNTER;
}

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
    this.hominids = new THREE.InstancedMesh(withGait(hominidGeometry()), gaitMaterial(this.uniforms), max);
    /** The figure meshes, indexed by `FIGURE`; each carries its own `userData.ids` (instance → agent id) for picking. */
    this.meshes = [this.grazers, this.hunters, this.hominids];
    /** Per-kind footprint at scale 1 (see `figureFootprint`), indexed like `meshes`. */
    this.footprint = this.meshes.map((m) => figureFootprint(m.geometry));
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
        // Live rows: the species' own hue (the swatch the species list shows),
        // shaded per individual by the genome's colour slots. Those slots sit
        // at a neutral 0.5 in nearly every archetype, and drawn raw they gave
        // every species the same dull teal. Replay rows already carry the
        // species hue in HUE.
        let c = live
          ? hsv(speciesHue(row[o + AGENT.SPECIES]) + (row[o + AGENT.HUE] - 0.5) * 0.3,
            Math.min(0.85, Math.max(0.3, 0.6 + (row[o + AGENT.SAT] - 0.5) * 0.5)),
            Math.min(1, Math.max(0.5, 0.9 + (row[o + AGENT.VAL] - 0.5) * 0.5)))
          : hsv(row[o + AGENT.HUE], row[o + AGENT.SAT], row[o + AGENT.VAL]);
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

  /**
   * @param unitsPerPixel  world units spanned by one screen pixel at the
   *   camera's current distance to its target (computed once per frame by
   *   the caller — see `stage.unitsPerPixel` in scene.js — never per agent).
   */
  update(a, heightAt, live, unitsPerPixel = 0) {
    const n = Math.min(a.count, this.max), d = a.data, s = a.stride;
    const counts = this.meshes.map(() => 0);
    this.selectedPos = null;
    const gaits = this.meshes.map((m) => m.geometry.attributes.aGait.array);
    const prev = this.prev, cur = this.spare, buf = this.buf, born = this.born, died = this.died;
    const legible = LEGIBLE_PX * unitsPerPixel;
    cur.clear(); born.length = 0; died.length = 0;
    for (let k = 0; k < n; k++) {
      const o = k * s, x = d[o + AGENT.X], y = d[o + AGENT.Y];
      const h = heightAt(x, y);
      const kind = figureKind(d[o + AGENT.DIET]);
      const readable = this.baseScale * (0.55 + 0.45 * d[o + AGENT.SIZE]);
      const sc = unitsPerPixel > 0 ? bodyScale(readable, d[o + AGENT.BODY] / this.footprint[kind], legible) : readable;
      const id = d[o + AGENT.ID] | 0;
      const rot = d[o + AGENT.ROT];
      const asleep = (d[o + AGENT.FLAGS] & AGENT_FLAG.ASLEEP) !== 0;
      _p.set(x, Math.max(h, 0) + 0.02, y);
      _q.setFromAxisAngle(Y_AXIS, -rot);
      _s.set(sc, asleep ? sc * 0.6 : sc, sc);
      _m.compose(_p, _q, _s);
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
    for (let kind = 0; kind < this.meshes.length; kind++) {
      const mesh = this.meshes[kind];
      mesh.count = counts[kind];
      mesh.instanceMatrix.needsUpdate = true;
      mesh.geometry.attributes.aGait.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
      // three caches an InstancedMesh's bounding sphere the first time it is
      // needed and never refreshes it as instance matrices change; here it was
      // computed over zero instances (an empty sphere) before the first world
      // was attached, so `InstancedMesh.raycast` rejected every pick and
      // clicking an agent never opened its card. Drop the cache each update:
      // the next raycast (a click) recomputes it over the live instances.
      mesh.boundingSphere = null;
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
/** Share of a species' members in the band to become / stay a hominid. */
const HOMINID_IN = 0.5, HOMINID_OUT = 0.3;

/**
 * Hut clusters at settlement sites; villages grow in and linger/fade out.
 *
 * A site's position is the centroid of its members' home anchors, and that
 * centroid wanders every tick as anchors learn and members are born and die
 * (tens of world units over a run, with single-sample jumps when a cohort
 * dies). Drawing the huts at the live centroid slid whole villages across
 * the ground. A village is therefore pinned where it is founded and only
 * moves when its people have plainly left: the smoothed centroid has to sit
 * more than `RELOCATE` village radii away for `DWELL` ticks, and then the old
 * huts fade out where they stood while a new village grows at the new home.
 *
 * Huts belong to hominids only. The core's settlement latch fires for any
 * species whose home anchors cluster — grazing herds and hunting packs
 * included — so a site is drawn only while its species reads as a hominid
 * (see `classify`).
 */
export class Villages {
  constructor(max = 1024) {
    this.max = max;
    this.mesh = new THREE.InstancedMesh(hutGeometry(), new THREE.MeshStandardMaterial({ roughness: 0.9, flatShading: true, vertexColors: true }), max);
    this.mesh.count = 0; this.mesh.frustumCulled = false; this.mesh.name = "villages";
    this.mesh.castShadow = true; this.mesh.receiveShadow = true;
    // key → {sid, x, y (pinned), lx, ly (smoothed live centroid), n, huts, hutBorn[], born, seen, farSince, retired, ease}
    // Live villages are keyed by species id; a village left behind by a move is
    // re-keyed `r<sid>:<tick>` and only fades.
    this.sites = new Map();
    this.scale = 1;
    this.LINGER = 300; this.FADE = 100; this.GROW = 40;
    this.RELOCATE = 2.5;     // village radii the smoothed centroid must clear before the village moves
    this.DWELL = 150;        // ticks it must stay clear (a cohort dying is a jump, not a move)
    this.SMOOTH = 120;       // time constant (ticks) of the centroid smoothing
    this.RETIRE = 90;        // ticks an abandoned village takes to fade away
    this.lastTick = -1;
    this.jumped = false;     // the last update was a time jump (see update)
    this.isWater = () => false;
    this.clearingKey = "";
    this.hominids = new Set();
  }
  /**
   * Which species are hominids, from one frame of agent rows: those whose
   * members mostly sit in the omnivore diet band the Godot viewer draws as
   * primates (`MammalSprites.HERB_MAX`..`CARN_MIN`). Every culture-bearing
   * archetype (innovator, traditionalist, ape_hunter, cultural_forager) runs
   * at diet ≈ 0.5; grazers sit near 0 and hunters near 1. Body size is left
   * out: growing juveniles fall under the Godot size split. A species joins
   * at a `HOMINID_IN` share of its members and leaves below `HOMINID_OUT`,
   * so a lineage drifting across the band does not flicker its village.
   */
  classify(agents) {
    const { count, data, stride } = agents, tally = new Map();
    for (let k = 0; k < count; k++) {
      const o = k * stride, sid = data[o + AGENT.SPECIES], d = data[o + AGENT.DIET];
      let t = tally.get(sid);
      if (!t) tally.set(sid, (t = [0, 0]));
      t[1]++;
      if (d >= HOMINID_DIET[0] && d < HOMINID_DIET[1]) t[0]++;
    }
    const next = new Set();
    for (const [sid, [inBand, n]] of tally) {
      const share = inBand / n;
      if (share >= HOMINID_IN || (this.hominids.has(sid) && share >= HOMINID_OUT)) next.add(sid);
    }
    this.hominids = next;
  }
  setWorldSize(ws, cell = ws / 128) { this.scale = Math.max(2.6, cell * 0.58); }
  /** Outer hut ring radius in world units (huts h ≥ 1 sit at 0.9–2.0 scales out). */
  get radius() { return this.scale * 2.0; }
  /** Target hut count for `n` anchored members, with a two-hut hysteresis band on the way down. */
  static hutsFor(n, shown = 0) {
    const want = Math.max(1, Math.min(Math.floor(n / 8), 9));
    return want >= shown || shown - want >= 2 ? want : shown;
  }
  /** `instant`: sites first seen now are drawn fully grown (the capture harness's fast-forward). */
  update(sites, tick, heightAt, stride = 4, instant = false) {
    // After a jump in time (a fast-forward, a seek, or the first frame of a
    // world) a site that is already there was not founded this tick: draw it
    // grown, and snap a pinned village straight to where it now is. Plain
    // play at 64× advances ~64–128 ticks a frame, so only a gap longer than a
    // village lingers (or any step backwards) counts as a jump.
    const gap = tick - this.lastTick;
    const jump = instant || this.lastTick < 0 || gap < 0 || gap > this.LINGER;
    const reach = this.RELOCATE * this.radius;
    this.lastTick = tick;
    this.jumped = jump;   // read by the forest clearing: a jump clears at once
    for (let k = 0; k < sites.count; k++) {
      const o = k * stride, sid = sites.data[o], x = sites.data[o + 1], y = sites.data[o + 2], n = sites.data[o + 3];
      if (!this.hominids.has(sid)) continue;   // not reported ⇒ an existing village lingers and fades
      let v = this.sites.get(sid);
      if (!v) {
        const born = jump ? tick - this.GROW : tick;
        v = { sid, x, y, lx: x, ly: y, n, huts: 0, hutBorn: [], born, seen: tick, farSince: -1, fx: 0, fy: 0, fn: 0, retired: -1, ease: 0 };
        this.sites.set(sid, v);
      }
      const dt = Math.max(0, tick - v.seen);
      const k1 = jump ? 1 : 1 - Math.exp(-dt / this.SMOOTH);
      v.lx += (x - v.lx) * k1; v.ly += (y - v.ly) * k1;
      v.n = n; v.seen = tick;
      const far = Math.hypot(v.lx - v.x, v.ly - v.y) > reach;
      if (!far) v.farSince = -1;
      else if (jump) { v.x = v.lx; v.y = v.ly; v.farSince = -1; }
      else {
        // While away, average the raw centroid: the smoothed one lags a real
        // move, and the new village belongs where the people were meanwhile.
        if (v.farSince < 0) { v.farSince = tick; v.fx = v.fy = v.fn = 0; }
        v.fx += x; v.fy += y; v.fn++;
        if (tick - v.farSince >= this.DWELL) {
          // The people have moved on: leave the old huts to fade where they
          // stand and found the village again at the new home.
          this.sites.set(`r${sid}:${tick}`, { ...v, hutBorn: v.hutBorn.slice(), retired: tick });
          v.x = v.lx = v.fx / v.fn; v.y = v.ly = v.fy / v.fn;
          v.farSince = -1; v.born = tick; v.huts = 0; v.hutBorn.length = 0;
        }
      }
      const huts = Villages.hutsFor(n, v.huts);
      for (let h = v.huts; h < huts; h++) v.hutBorn[h] = v.huts === 0 ? v.born : tick;
      v.huts = huts;
    }
    this.layout(heightAt);
  }
  /** Rebuild the hut instances for the sites held, as of the last tick seen —
   *  also after the ground changes shape (relief toggled) while paused. */
  layout(heightAt) {
    const tick = this.lastTick;
    let i = 0;
    for (const [key, v] of this.sites) {
      let fade;
      if (v.retired >= 0) {
        const age = tick - v.retired;
        if (age > this.RETIRE || age < 0) { this.sites.delete(key); continue; }
        fade = 1 - age / this.RETIRE;
      } else {
        const stale = tick - v.seen;
        if (stale > this.LINGER || stale < 0) { this.sites.delete(key); continue; }
        fade = Math.min(1, (this.LINGER - stale) / this.FADE);
      }
      const grow = Math.min(1, Math.max(0, (tick - v.born) / this.GROW));
      v.ease = (1 - Math.pow(1 - grow, 3)) * fade;
      _c.setHex(mix(hsv(speciesHue(v.sid), 0.5, 0.8), 0x8a6a3c, 0.55));
      for (let h = 0; h < v.huts && i < this.max; h++) {
        const ang = v.sid * 2.39996 + h * 2.39996, r = (0.9 + (h % 3) * 0.55) * this.scale * (h === 0 ? 0 : 1);
        const x = v.x + Math.cos(ang) * r, y = v.y + Math.sin(ang) * r;
        if (this.isWater(x, y)) continue;   // a lakeside village keeps its huts on the shore
        const g = Math.min(1, Math.max(0, (tick - v.hutBorn[h]) / this.GROW));
        const sc = this.scale * (h === 0 ? 1.35 : 1) * fade * (1 - Math.pow(1 - g, 3));
        if (sc <= 0) continue;
        _p.set(x, seatY(heightAt, x, y, ang, sc, HUT_FOOT), y);
        if (h === 0) v.base = _p.y;   // hearth smoke rises from the centre hut's roof
        _q.setFromAxisAngle(Y_AXIS, ang);
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
  /** Live sites for the hearth-smoke emitter: `[{x, y, n, ease, base}]` (pinned positions; `base` the centre hut's floor). */
  centers() { return [...this.sites.values()].filter((v) => v.retired < 0 && v.ease > 0.6); }
  /**
   * Footprints the forest should stand back from, `[{x, y, r}]`, or null when
   * they have not changed since the last call (so the caller can skip the
   * forest update). Abandoned villages keep their clearing while they fade.
   */
  clearings() {
    const out = [], r = this.radius + this.scale * 0.9;
    for (const v of this.sites.values()) out.push({ x: v.x, y: v.y, r });
    const key = out.map((c) => `${c.x.toFixed(1)},${c.y.toFixed(1)}`).sort().join(";") + `|${r}`;
    if (key === this.clearingKey) return null;
    this.clearingKey = key;
    return out;
  }
  clear() { this.sites.clear(); this.hominids.clear(); this.mesh.count = 0; this.lastTick = -1; this.clearingKey = ""; }
}

// ---------------------------------------------------------------------------
/** Ground radius of a stall at scale 1 (the awning poles sit ~0.8 out). */
export const STALL_FOOTPRINT = 0.85;
const RING8 = Array.from({ length: 8 }, (_, a) => [Math.cos((a * Math.PI) / 4), Math.sin((a * Math.PI) / 4)]);

/**
 * Where to draw the stall of a sim hub at (x, y). Hub positions are the sim's
 * (fixed at load, never changed here) and some fall in a lake or straddle its
 * shore, so a hub whose footprint — the point and a ring of radius `r` — is not
 * all `dry` is drawn at the nearest spot within `reach` (searched on a `step`
 * grid, first in scan order on ties, so it is deterministic) whose footprint
 * is. With no dry spot that near the hub keeps its position and comes back
 * `afloat`: the stall is still a real market, so it rides the water surface as
 * a raft rather than vanishing or sinking to the lake bed. With `size` (world
 * units) a moved stall also stays on the plate: the sim's world wraps, the
 * drawn ground does not.
 */
export function hubSpot(x, y, r, dry, step, reach, size = 0) {
  const clear = (px, py) => dry(px, py) && RING8.every(([c, s]) => dry(px + c * r, py + s * r));
  if (clear(x, y)) return { x, y, moved: false, afloat: false };
  const onPlate = (v) => !size || (v >= r && v <= size - r);
  let best = null, bd = Infinity;
  const n = Math.ceil(reach / step);
  for (let j = -n; j <= n; j++) {
    for (let i = -n; i <= n; i++) {
      const d = Math.hypot(i, j) * step, px = x + i * step, py = y + j * step;
      if (d > reach || d >= bd || !onPlate(px) || !onPlate(py)) continue;
      if (clear(px, py)) { best = { x: px, y: py, moved: true, afloat: false }; bd = d; }
    }
  }
  return best || { x, y, moved: false, afloat: true };
}

/** Fixed trade hubs (markets) — static after load. */
export class Hubs {
  constructor(max = 128) {
    this.mesh = new THREE.InstancedMesh(stallGeometry(), new THREE.MeshStandardMaterial({ roughness: 0.8, flatShading: true, vertexColors: true }), max);
    this.mesh.count = 0; this.mesh.frustumCulled = false; this.mesh.name = "hubs";
    this.mesh.castShadow = true; this.mesh.receiveShadow = true;
    this.max = max;
    /** Drawn stall position per hub, `{x, y, moved, afloat}` (see hubSpot). */
    this.spots = [];
    this.src = null;
  }
  /** `isWater(x, y)` keeps stalls off water cells; it may answer false until the terrain ids are known, so call `layout()` again once they are. */
  set(hubs, cell, heightAt, isWater = () => false, worldSize = 0, stride = 3) {
    const n = Math.min(hubs.count, this.max);
    // Copy: the rows are a view into wasm memory, and layout() may run again later.
    this.src = { xy: Array.from({ length: n }, (_, k) => [hubs.data[k * stride], hubs.data[k * stride + 1]]), cell, heightAt, isWater, worldSize };
    this.layout();
  }
  layout() {
    if (!this.src) return;
    const { xy, cell, heightAt, isWater, worldSize } = this.src;
    const sc = Math.max(3.5, cell * 0.72);
    // Dry = not a water cell and not under the water plane: the drawn shore
    // follows the vertex-averaged heights, which dip below 0 on the edge of a
    // land cell that borders water. (Flat worlds read 0 everywhere, so only
    // the cell test bites there.)
    const dry = (x, y) => !isWater(x, y) && heightAt(x, y) >= 0;
    this.spots = xy.map(([x, y]) => hubSpot(x, y, sc * STALL_FOOTPRINT, dry, cell / 4, cell * 4, worldSize));
    this.spots.forEach(({ x, y }, k) => {
      // Terraced on its plinth (seatY); an afloat stall rides the water plane.
      _p.set(x, Math.max(0, seatY(heightAt, x, y, k * 1.3, sc, STALL_FOOT)), y);
      _q.setFromAxisAngle(Y_AXIS, k * 1.3);
      _s.set(sc, sc, sc);
      this.mesh.setMatrixAt(k, _m.compose(_p, _q, _s));
    });
    this.mesh.count = this.spots.length;
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
